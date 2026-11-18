#version 450
// Models. Characters (flags bit 0) are shaded the way the game's character pixel program
// (CharaMTC) does it, step by step as in docs/shading.md; the rest keep a plain light.
layout(set = 0, binding = 0) uniform sampler2D tex;
// The character's mask (its red channel says how much of the shading applies; white when the
// material has none).
layout(set = 2, binding = 0) uniform sampler2D mask;
// A field model's emission map (flag 32768; white otherwise).
layout(set = 3, binding = 0) uniform sampler2D emissive;
layout(push_constant) uniform Push { mat4 view_proj; vec4 params; vec4 scale_a; vec4 scale_b; uint instance; uint flags; float outline_colour;
    // A field model's lighting material: cDiff and cAmb as unorm16 (Draw::light).
    uint light; } pc;
// The field's character lighting (barrel_formats::field_look), in render space: the
// directions as the field gives them (the shader uses their negation), the colours with their
// weight in w; the camera's eye.
layout(std140, set = 1, binding = 4) uniform Look {
    vec4 light_dir;
    vec4 light_color;
    vec4 rim_dir;
    vec4 rim_color;
    vec4 eye;
    // x: the point lights' strength (atCharaPointLightStrength); y: how many point lights.
    vec4 misc;
    // Up to four point lights (render space, field units): position and range² (w), colour,
    // attenuation {att0, att1, att2} over field units, and ambient.
    vec4 point_pos[4];
    vec4 point_color[4];
    vec4 point_att[4];
    vec4 point_ambient[4];
    // The field's lights for its own models: the first directional light (the sun: direction,
    // colour), the second (the fill), and the ambient.
    vec4 sun_dir;
    vec4 sun_color;
    vec4 fill_dir;
    vec4 fill_color;
    vec4 ambient;
    // The sun's shadow map: world (render space) to its clip space, and {texel size, on}.
    mat4 shadow_vp;
    vec4 shadow_info;
    // The field's fog: {1 / (end - start), -start / (end - start)} in field units, and the
    // scattering constants (docs/shading.md, "Fog": read from the game's memory).
    vec4 fog;
    vec4 scat[4];
    // Slots 37 to 47: the billboards' axes and the view-projections (model.vert's).
    vec4 vertex_only[11];
    // The field's light texture (docs/shading.md, "The light texture"): {1 / width,
    // 1 / height (field units), u + 0.5, v + 0.5}, and {on}.
    vec4 pattern;
    vec4 pattern_on;
    // The grass sway groups (`<v_shaker>`, game::wind): {wind.xyz, clock}, {1 / height,
    // root height, shift x, shift z} each, metres.
    vec4 sway[16];
    // Slot 66: the cube map's light on (x > 0.5); 67 to 75 its spherical-harmonic
    // coefficients (model.vert's `environment_light`); 76 to 86 the sky plane's.
    vec4 sh_on;
    vec4 sh[9];
    vec4 others[11];
    // The far shadow square (lib.rs SHADOW_FAR_SLOT): view-projection, {texel, layer (0:
    // none), fade start, fade end}.
    mat4 shadow_far;
    vec4 shadow_far_info;
    // Slots 92 and 93: the vertex program's; 94: the emission maps' strength (flag 32768).
    vec4 frag_pad[2];
    vec4 emission;
} look;
layout(location = 0) in vec3 normal;
layout(location = 1) in vec2 uv;
layout(location = 2) in vec3 world;
// The point lights' sum at this vertex (the vertex program's v2).
layout(location = 3) in vec3 scene_light;
layout(location = 4) in vec3 field_light;
layout(location = 5) in vec2 screen_uv;
layout(location = 6) in vec3 sun_part;
layout(location = 7) in vec3 fog_add;
layout(location = 8) in vec3 fog_keep;
layout(set = 1, binding = 5) uniform sampler2DArrayShadow shadow_map;
layout(location = 0) out vec4 color;
// The motion vector (`--motion`): how far this point moved on screen since the last frame, in
// texture coordinates (no attachment takes it when motion is off).
layout(location = 9) in vec4 cur_clip;
layout(location = 10) in vec4 prev_clip;
layout(location = 1) out vec2 motion;
layout(location = 11) flat in vec4 tint_mul;
layout(location = 12) flat in vec4 tint_add;

// The instance's colour transform on its lit colour, in display (sRGB) terms; none unless on.
vec3 tinted(vec3 c) {
    if (tint_add.w < 0.5) return c;
    vec3 s = mix(c * 12.92, 1.055 * pow(c, vec3(1.0 / 2.4)) - 0.055, step(0.0031308, c));
    s = clamp(s * tint_mul.rgb + tint_add.rgb, 0.0, 1.0);
    return mix(s / 12.92, pow((s + 0.055) / 1.055, vec3(2.4)), step(0.04045, s));
}

// The game's soft-light blend of a colour k over a, weighted by w (k first divided by its
// largest channel, at least 1).
vec3 soft_light(vec3 a, vec4 kw) {
    vec3 k = kw.rgb / max(max(max(kw.r, kw.g), kw.b), 1.0);
    vec3 lo = clamp((a * k - (1.0 - 2.0 * k)) / max(2.0 * k, vec3(0.001)), 0.0, 1.0);
    vec3 hi = clamp((a * k) / (2.0 - 2.0 * k), 0.0, 1.0);
    vec3 pick = mix(lo, hi, floor(k + 0.5));
    vec3 blended = a * k + (pick - a * k) * 0.25;
    return mix(a, blended, kw.w);
}

// The sun's shadow at a render-space point: MapSTD's 16 taps (four 2 × 2 comparisons a texel
// and a half apart), 1 lit to 0 shadowed (atColor white: the full strength).
layout(set = 1, binding = 6) uniform sampler2D pattern_map;

// The field's light texture at world point p (render space): the point slid along the
// sunlight onto the plane 1000 units up, in the field's own space (x mirrored back), then
// scaled to one repeat per area and moved by the drift. 1 without one.
float cloud_light(vec3 p) {
    vec3 d = vec3(-look.sun_dir.x, look.sun_dir.y, look.sun_dir.z);
    if (look.pattern_on.x == 0.0 || abs(d.y) < 1e-4) {
        return 1.0;
    }
    vec3 g = vec3(-p.x, p.y, p.z);
    float k = (g.y - 1000.0) / d.y;
    vec2 uv = vec2((g.x - k * d.x) * look.pattern.x + look.pattern.z, (g.z - k * d.z) * look.pattern.y + look.pattern.w);
    float light = texture(pattern_map, uv).r;
    // pattern_on.y: the texture's strength (the Ryza games' `fPtn`: the light is 1 less that
    // much of the texture's shade); 0: the texture as it is.
    return look.pattern_on.y > 0.0 ? clamp(1.0 + look.pattern_on.y * (light - 1.0), 0.0, 1.0) : light;
}

// One shadow layer's light at p (16 taps over 1.5 texels), or -1 outside it.
float shadow_layer(mat4 vp, float layer, float t, vec3 p) {
    vec4 c = vp * vec4(p, 1.0);
    vec3 s = c.xyz / c.w;
    vec2 at = s.xy * 0.5 + 0.5;
    if (any(lessThan(at, vec2(0.0))) || any(greaterThan(at, vec2(1.0))) || s.z > 1.0) return -1.0;
    float lit = 0.0;
    for (int i = 0; i < 4; i++) {
        vec2 o = vec2(float(i & 1) * 2.0 - 1.0, float(i >> 1) * 2.0 - 1.0) * t * 0.75;
        for (int k = 0; k < 4; k++) {
            vec2 q = vec2(float(k & 1), float(k >> 1)) * t * 0.5;
            lit += texture(shadow_map, vec4(at + o + q, layer, s.z));
        }
    }
    return lit / 16.0;
}

float sun_shadow(vec3 p) {
    if (look.shadow_info.y < 0.5) return 1.0;
    float near = shadow_layer(look.shadow_vp, 0.0, look.shadow_info.x, p);
    if (near >= 0.0) return near;
    // Past the near square: the far layer, if on, fading to lit with the distance.
    if (look.shadow_far_info.y < 0.5) return 1.0;
    float far = shadow_layer(look.shadow_far, look.shadow_far_info.y, look.shadow_far_info.x, p);
    if (far < 0.0) return 1.0;
    float fade = clamp((distance(p, look.eye.xyz) - look.shadow_far_info.z) / max(look.shadow_far_info.w - look.shadow_far_info.z, 1.0), 0.0, 1.0);
    return mix(far, 1.0, fade);
}
// The field's light from its cube map on normal n (as model.vert's `environment_light`).
vec3 environment_light(vec3 n) {
    vec3 g = normalize(vec3(-n.x, n.y, n.z));
    float b[9] = float[9](0.282095, 0.488603 * g.y, 0.488603 * g.z, 0.488603 * g.x, 1.092548 * g.x * g.y,
        1.092548 * g.y * g.z, 0.315392 * (3.0 * g.z * g.z - 1.0), 1.092548 * g.x * g.z, 0.546274 * (g.x * g.x - g.y * g.y));
    vec3 c = vec3(0.0);
    for (int k = 0; k < 9; k++) {
        c += look.sh[k].rgb * b[k];
    }
    return max(c, vec3(0.0));
}
void main() {
    motion = (cur_clip.xy / cur_clip.w - prev_clip.xy / prev_clip.w) * 0.5;
    vec4 c = texture(tex, uv);
    if ((pc.flags & 8u) != 0u) {
        // The distant view, blended by its alpha (clouds over the dome); its solid pieces
        // (flag 16: ground, stones, trees, cliffs) alpha-tested like the field's models.
        if ((pc.flags & 16u) != 0u) {
            if (c.a < pc.params.x) discard;
            c.a = 1.0;
        }
        color = vec4(tinted(mix(fog_add, (field_light + sun_part * sun_shadow(world) * cloud_light(world)) * c.rgb, fog_keep)) * pc.params.w, c.a);
        return;
    }
    if (c.a < pc.params.x) discard;
    vec3 n = normalize(normal);
    if ((pc.flags & 1u) == 0u) {
        // A field model: its light (from the vertex program) on its colour; specular, the
        // light texture and shadows are not done yet, nor fog. Its atColor (a run-time tint,
        // packed as unorm8 in the outline colour's word; 0: none) multiplies the colour.
        uint tint = floatBitsToUint(pc.outline_colour);
        vec3 at_color = tint != 0u && (pc.flags & 128u) == 0u ? unpackUnorm4x8(tint).rgb : vec3(1.0);
        vec3 sun_now = sun_part;
        vec3 field_now = field_light;
        // Flag 8192: a normal map in the mask slot (Ryza's `sNMap`: x = red × alpha, y =
        // green, to -1..1, z rebuilt), in a tangent frame from the screen derivatives of the
        // position and UV; the sun's Lambert term taken again per pixel with it.
        if ((pc.flags & 8192u) != 0u) {
            vec4 t = texture(mask, uv);
            vec2 xy = vec2(t.r * t.a, t.g) * 2.0 - 1.0;
            vec3 tn = vec3(xy, sqrt(max(1.0 - dot(xy, xy), 0.0)));
            vec3 dp1 = dFdx(world), dp2 = dFdy(world);
            vec2 du1 = dFdx(uv), du2 = dFdy(uv);
            vec3 p2 = cross(dp2, n), p1 = cross(n, dp1);
            vec3 tg = p2 * du1.x + p1 * du2.x;
            vec3 bt = p2 * du1.y + p1 * du2.y;
            float inv = inversesqrt(max(max(dot(tg, tg), dot(bt, bt)), 1e-20));
            vec3 nm = normalize(mat3(tg * inv, bt * inv, n) * tn);
            float after = max(dot(nm, -normalize(look.sun_dir.xyz)), 0.0);
            vec2 material = unpackUnorm2x16(pc.light);
            sun_now = look.sun_color.rgb * material.x * after;
            // The ambient, too, taken again on the mapped normal (the vertex's replaced).
            if (look.sh_on.x > 0.5) {
                field_now = max(field_now + (environment_light(nm) - environment_light(n)) * material.y, 0.0);
            }
            n = nm;
        }
        vec3 lit = field_now + sun_now * sun_shadow(world) * cloud_light(world);
        // Flag 32768: the emission map as light of its own (PBB-EmN's `sEmi × emScl`, added to
        // the light before the colour multiplies it).
        if ((pc.flags & 32768u) != 0u) {
            lit += texture(emissive, uv).rgb * look.emission.x;
        }
        // A Ryza character (flag 128): the rim (its word in the tint's place: sharpness / 64
        // and strength / 4 as unorm8), where the eye looks into the sun, `pow(1 - N·V,
        // sharpness) × strength` of the sun's colour, up to the full sun; then the toon ramp (the mask slot holds
        // `chara_rmp`) on the light, its u the sun's half-Lambert term and its row (bits 16-23)
        // the material's `rampIndex` (DSChara-Cll's, docs/shading.md).
        if ((pc.flags & 128u) != 0u) {
            vec2 rp = unpackUnorm4x8(tint).xy * vec2(64.0, 4.0);
            vec3 to_eye = normalize(look.eye.xyz - world);
            float backlit = clamp(dot(normalize(look.sun_dir.xyz), to_eye), 0.0, 1.0);
            float rim = pow(1.0 - clamp(dot(n, to_eye), 0.0, 1.0), max(rp.x, 1.0)) * rp.y * backlit;
            // As in the game, the rim and the sun's N·L are saturated together: the rim adds
            // only what N·L leaves of the sun.
            float nl = max(dot(n, -normalize(look.sun_dir.xyz)), 0.0);
            lit += look.sun_color.rgb * unpackUnorm2x16(pc.light).x * min(rim, 1.0 - nl);
            // The ramp as the game's character lighting pass samples it (docs/shading.md):
            // u is the sun's half-Lambert term times its shadow (the cascades' and the cloud
            // pattern's) times `rampCoordScaler`, kept inside `rampCoordRange`, so only the
            // row's short run from its shade tint (0.2) toward white is used; v is the row
            // before the material's `rampIndex` (index 0: no ramp).
            const float RAMP_SCALE = 0.27;            // rampCoordScaler.x, as captured at Rasenboden
            const vec2 RAMP_RANGE = vec2(0.2, 0.5);   // rampCoordRange, as captured there
            float h = clamp(dot(n, -normalize(look.sun_dir.xyz)) * 0.5 + 0.5, 0.0, 1.0);
            float row = float((pc.flags >> 16) & 255u);
            if (row > 0.0) {
                float u = clamp(RAMP_SCALE * h * sun_shadow(world) * cloud_light(world), RAMP_RANGE.x, RAMP_RANGE.y);
                lit *= textureLod(mask, vec2(u, (row - 0.5) / 256.0), 0.0).rgb;
            }
        }
        color = vec4(tinted(mix(fog_add, lit * c.rgb * at_color, fog_keep)) * pc.params.w, 1.0);
        return;
    }
    vec3 col;
    if ((pc.flags & 4u) != 0u) {
        // An outline (CharaFra*'s pixel program): the colour under the light colour, half
        // multiplied and half overlaid with the outline texture, its saturation pushed (the
        // channels below the brightest lowered 10%), times the material's outline colour.
        vec3 a = soft_light(c.rgb, look.light_color);
        vec3 o = texture(mask, screen_uv).rgb;
        vec3 ovl = mix(2.0 * a * o, 1.0 - 2.0 * (1.0 - a) * (1.0 - o), round(o));
        vec3 b = mix(ovl, a * o, 0.5);
        float top = max(max(b.r, b.g), b.b) - 0.003921569;
        b = b * 0.9 + b * clamp((b - top) * 255.0, 0.0, 1.0) * 0.1;
        col = b * pc.outline_colour;
    } else if ((pc.flags & 2u) != 0u) {
        // An eye (CharaEYE): the white and iris (already laid together, the iris by its
        // alpha; the specular is not done) under the light colour at 0.65 of its weight, then
        // the highlights (the mask slot) tinted by the rim colour at up to 0.2, by their alpha.
        vec4 kw = look.light_color;
        vec3 lit = soft_light(c.rgb, vec4(kw.rgb, kw.w * 0.65));
        vec4 hl = texture(mask, uv);
        vec4 rw = look.rim_color;
        col = mix(lit, soft_light(hl.rgb, vec4(rw.rgb, min(rw.w, 0.2))), hl.a);
    } else {
    // 1. The toon step on a half-Lambert term, toward the character light.
    float h = clamp((dot(n, -normalize(look.light_dir.xyz)) + 1.0) * 0.5, 0.0, 1.0);
    float t = clamp((h - 0.546875) * 10.66667, 0.0, 1.0);
    vec3 shaded = mix(max(c.rgb * c.rgb, c.rgb * 0.5), c.rgb, t);
    // 2. The light colour, then 3. the mask.
    vec3 lit = clamp(mix(c.rgb, soft_light(shaded, look.light_color), texture(mask, uv).r), 0.0, 1.0);
    // 4. The rim light, screen-blended.
    vec3 v = normalize(look.eye.xyz - world);
    float r = max((1.0 - abs(dot(n, v))) * clamp((dot(n, -normalize(look.rim_dir.xyz)) + 1.0) * 0.5, 0.0, 1.0), 0.0);
    r = clamp((r - 0.3) * 32.0, 0.0, 1.0) * 0.8;
    vec3 rim = clamp(soft_light(c.rgb * r, look.rim_color), 0.0, 1.0);
    col = 1.0 - (1.0 - lit) * (1.0 - rim);
    }
    // 5. The point lights, laid over (overlay blend), weighted by their brightest channel
    // times the field's point light strength.
    vec3 L = scene_light;
    vec3 over = mix(2.0 * col * L, 1.0 - 2.0 * (1.0 - col) * (1.0 - L), round(col));
    float w = max(max(L.r, L.g), L.b) * look.misc.x;
    col = clamp(col + w * (over - col), 0.0, 1.0);
    // (6. fog is not done yet; 7. atColor is 1 for characters and the HDR range 1.)
    // The scene is drawn at the HDR range's inverse (params.w), doubled back after.
    color = vec4(tinted(mix(fog_add, col, fog_keep)) * pc.params.w, 1.0);
}
