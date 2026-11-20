#version 450
#extension GL_GOOGLE_include_directive : require
#extension GL_EXT_control_flow_attributes : require
// Models. Characters (flags bit 0) are shaded the way the game's character pixel program
// (CharaMTC) does it, step by step as in docs/shading.md; the rest keep a plain light.
layout(set = 0, binding = 0) uniform sampler2D tex;
// The character's mask (its red channel says how much of the shading applies; white when the
// material has none).
layout(set = 2, binding = 0) uniform sampler2D mask;
// A field model's emission map (flag 32768; white otherwise).
layout(set = 3, binding = 0) uniform sampler2D emissive;
// A field model's specular map (flag 1 << 24: `sSpMap`, as stored; white otherwise).
layout(set = 4, binding = 0) uniform sampler2D spec_map;
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
    // Slots 95 and 96: a sky dome's sun, its colour and the way to it (flag 4096).
    vec4 dome_sun;
    vec4 dome_sun_way;
    // Slots 97 to 108: the clouds'. 109: the specular ambient's light scale and its mip range;
    // 110: its tint.
    vec4 cloud_pad[12];
    vec4 spec_amb;
    vec4 spec_tint;
    // Slots 111 to 191: the water's. Then the cascades' (lib.rs SHADOW_CASCADE_SLOT): the
    // second and third view-projections, the cascades' ends along the view and the fade width
    // into the static layer, the view's forward.
    vec4 water_pad[81];
    mat4 shadow_c1;
    mat4 shadow_c2;
    vec4 shadow_ends;
    vec4 shadow_fwd;
    // The characters' map (lib.rs SHADOW_SUB_SLOT): view-projection, {texel, share of the
    // layer, on, layer}.
    mat4 shadow_sub;
    vec4 shadow_sub_info;
    // The characters' specular ambient (lib.rs CHARA_SPECULAR_SLOT): the light's scale times
    // its tint (rgb), and the exposure the lights carry (w).
    vec4 chara_spec;
    // The cascades' and the static layer's maps within their layers (lib.rs SHADOW_SIDES_SLOT).
    vec4 shadow_sides[4];
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
// Set by the ambient occlusion's pipeline (ao.rs): the pixel's normal and depth are written, not
// its light.
layout(constant_id = 0) const bool AO_PASS = false;
// The motion vector (`--motion`): how far this point moved on screen since the last frame, in
// texture coordinates (no attachment takes it when motion is off).
layout(location = 9) in vec4 cur_clip;
layout(location = 10) in vec4 prev_clip;
layout(location = 1) out vec2 motion;
// The screen-space reflections' inputs (reflect.rs; no attachment takes them without the
// setting): the specular ambient's per-pixel factor (rgb: its Fresnel and occlusion terms times the
// fog's extinction; a: the gloss) and the scaled cube light it reflects, for the pass that
// mixes it toward the reflections after the scene is drawn.
layout(location = 2) out vec4 spec_light;
layout(location = 3) out vec4 spec_env;
// Where the occlusion's own pre-pass does not run, the world normal (octahedral, for the half-size
// normal and depth target, `gbuf_derive.frag`) takes the motion vectors' attachment (the scene pass
// has no motion vectors then); set by the scene pass's pipelines.
layout(constant_id = 1) const bool NORMAL_AS_MOTION = false;
// The field models' alpha test (off for draws whose textures have no texel under the reference:
// the program without a discard keeps the GPU's early depth test).
layout(constant_id = 2) const bool ALPHA_TEST = true;
// The alpha-tested draws' depth pre-pass: the alpha test alone, no colour (its pipeline writes
// none), so the shading pass after it runs the full program once per pixel.
layout(constant_id = 3) const bool DEPTH_ONLY = false;
// What the pipeline draws, so the compiler keeps only that program's paths (its registers are
// the largest path's): 0 anything (the flags decide), 1 the scene pass's field models, 2 its other
// models (characters, flag 1). Neither of the last two is the distant view or a character's hull.
layout(constant_id = 4) const uint KIND = 0u;
// Whether anything reads the motion vectors (model.vert's MOTION, the same for both).
layout(constant_id = 5) const bool MOTION = true;
bool is_chara() { return KIND == 1u ? false : KIND == 2u ? true : (pc.flags & 1u) != 0u; }
bool is_far() { return KIND != 0u ? false : (pc.flags & 8u) != 0u; }
bool is_hull() { return KIND != 0u ? false : (pc.flags & (1u << 25)) != 0u; }
layout(location = 11) flat in vec4 tint_mul;
layout(location = 12) flat in vec4 tint_add;
// The mesh's tangent (turned like the normal) and the bitangent's sign; zero without one.
layout(location = 13) in vec4 tangent;

// The instance's colour transform on its lit colour, in display (sRGB) terms; none unless on.
// HALF (the program's second build, `FRAG_HALF`): the colour arithmetic at 16 bits where the
// GPU has them (RelaxedPrecision: Adreno's twice-as-fast half floats, half the registers). The
// positions, texture coordinates, derivatives, shadows and the highlights' powers stay 32-bit.
#ifdef HALF
#define HALF_ON precision mediump float;
#define HALF_OFF precision highp float;
#else
#define HALF_ON
#define HALF_OFF
#endif
HALF_ON
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
HALF_OFF
layout(set = 1, binding = 6) uniform sampler2D pattern_map;
// The field's specular environment cube map (the game's axes: x not mirrored).
layout(set = 1, binding = 7) uniform samplerCube spec_cube;
#define AO_SET 5
#include "occlusion.glsl"
#include "normal_octahedral.glsl"
void put_normal(vec3 n) {
    if (NORMAL_AS_MOTION) motion = octahedral_encode(n);
}
HALF_ON
#include "environment_light.glsl"
HALF_OFF
#include "specular_direct.glsl"
#include "specular_ambient.glsl"

#include "sun_light.glsl"
// A field model's specular colour and gloss from its specular map as the G-buffer program
// writes them (green: reflectance, blue: metal; alpha: gloss).
HALF_ON
void specular_surface(vec3 albedo, out vec3 f0, out float gloss) {
    vec4 sp = texture(spec_map, uv);
    f0 = sp.b >= 0.5 ? albedo * (1.0 - sp.g) : vec3(sp.g);
    gloss = sp.a;
}

void main() {
    motion = NORMAL_AS_MOTION || !MOTION ? vec2(0.0) : (cur_clip.xy / cur_clip.w - prev_clip.xy / prev_clip.w) * 0.5;
    spec_light = vec4(0.0);
    spec_env = vec4(0.0);
    vec4 c = texture(tex, uv);
    if (DEPTH_ONLY) {
        if (c.a < ((pc.flags & 0xf00u) != 0u ? pc.params.y : pc.params.x)) discard;
        return;
    }
    // The stage dump of the sun's share (`dump::Stop::Shadow`): what the shading multiplies the sun by.
    if (look.shadow_info.w > 0.5 && (pc.flags & 4096u) == 0u && (!is_far() || (pc.flags & 16u) != 0u)) {
        if (ALPHA_TEST && c.a < ((pc.flags & 0xf00u) != 0u ? pc.params.y : pc.params.x)) discard;
        // (Stage 2: the coverage of a Ryza character, flag 128.)
        float share = look.shadow_info.w > 1.5 ? float((pc.flags & 128u) != 0u) : sun_shadow(world) * cloud_light(world);
        color = vec4(vec3(share) * pc.params.w, 1.0);
        return;
    }
    if (is_far()) {
        // The distant view, blended by its alpha (clouds over the dome); its solid pieces
        // (flag 16: ground, stones, trees, cliffs) alpha-tested like the field's models.
        if ((pc.flags & 16u) != 0u) {
            if (ALPHA_TEST && c.a < pc.params.x) discard;
            c.a = 1.0;
            if (AO_PASS) {
                color = vec4(normalize(normal), 1.0 / gl_FragCoord.w);
                return;
            }
        }
        // Flag 4096: a sky dome lit by its own ambient (`field_light`) and sun, by the pixel's
        // normal: the vertex's, or with flag 8192 the normal map in the mask slot (Ryza's
        // `PBSkyDome`; the map read as the field models' is, below).
        if ((pc.flags & 4096u) != 0u) {
            vec3 dn = normalize(normal);
            // The game's `PBSkyDome` program: the normal map's x (red times alpha) runs along
            // the bitangent `cross(N, T) * w`, its y along the mesh's tangent, and the sum is not
            // normalised; the sky's maps are flat black (x = y = -1, z = 0), so the normal is
            // -(T + B), a vector of length 1.41, and its sun term is that much stronger. The
            // specular map's green is the reflectance and its alpha the gloss (a shininess of
            // 2^(10 gloss + 1)).
            vec3 sn = dn;
            if ((pc.flags & 8192u) != 0u) {
                vec4 t = texture(mask, uv);
                vec2 xy = vec2(t.r * t.a, t.g) * 2.0 - 1.0;
                vec3 tg, bt;
                if (dot(tangent.xyz, tangent.xyz) > 0.0) {
                    tg = normalize(tangent.xyz);
                    bt = cross(dn, tg) * tangent.w;
                } else {
                    // A mesh without a tangent: the frame from the screen derivatives.
                    highp vec3 dp1 = dFdx(world), dp2 = dFdy(world);
                    highp vec2 du1 = dFdx(uv), du2 = dFdy(uv);
                    highp vec3 t0 = cross(dp2, dn) * du1.x + cross(dn, dp1) * du2.x;
                    tg = normalize(t0 - dn * dot(dn, t0));
                    bt = cross(tg, dn);
                }
                sn = tg * xy.y + bt * xy.x + dn * sqrt(max(1.0 - dot(xy, xy), 0.0));
            }
            // (a dome without a specular map has none: no reflectance, a dull gloss)
            vec4 sp = (pc.flags & (1u << 24)) != 0u ? texture(spec_map, uv) : vec4(0.0);
            float f0 = sp.g;
            vec3 to_eye = normalize(look.eye.xyz - world);
            vec3 sun_way = look.dome_sun_way.xyz;
            float n_dot_l = clamp(dot(sn, sun_way), 0.0, 1.0);
            vec3 half_way = normalize(to_eye + sun_way);
            float shine = exp2(sp.a * 10.0 + 1.0);
            float fresnel = f0 + (1.0 - f0) * pow(1.0 - clamp(dot(to_eye, half_way), 0.0, 1.0), 5.0);
            float highlight = pow(clamp(dot(sn, half_way), 0.0, 1.0), shine) * (shine * 0.125 + 0.25) * fresnel;
            vec3 sun = look.dome_sun.rgb * n_dot_l;
            vec3 dome = (field_light + sun) * c.rgb * (1.0 - f0) + sun * highlight;
            color = vec4(tinted(mix(fog_add, dome, fog_keep)) * pc.params.w, c.a);
            return;
        }
        color = vec4(tinted(mix(fog_add, (field_light + sun_part * sun_shadow(world) * cloud_light(world)) * c.rgb, fog_keep)) * pc.params.w, c.a);
        return;
    }
    // The alpha test: a swaying draw (a plant) against its own reference (params.y).
    if (ALPHA_TEST && c.a < ((pc.flags & 0xf00u) != 0u ? pc.params.y : pc.params.x)) discard;
    if (is_hull()) {
        // The game's forward pass over a character (`sh0d2367f637ea8f6a`, docs/shading.md, "Her
        // forward pass"): the colour tile plus, with a specular map, its red times the two
        // lights' Blinn-Phong highlights (`cSpec` power 20), at `vOLColorScaler` and faded by
        // `fOLLtEfFade` toward the ambient term (`vOLAmbEfScaler` 0), fogged as the scene is, over
        // the light by the tile's alpha. The values are the capture's, in the game's units (the
        // scene's exposure back in).
        const float OL_COLOUR = 0.5;      // vOLColorScaler
        const float OL_LIGHT_FADE = 0.5;  // fOLLtEfFade
        const float OL_AMBIENT = 0.0;     // vOLAmbEfScaler
        const float OL_SHINE = 20.0;      // cSpec.w
        vec3 fn = normalize(normal);
        vec3 fv = normalize(look.eye.xyz - world);
        vec3 spec = vec3(0.0);
        if ((pc.flags & (1u << 24)) != 0u) {
            vec3 ways[2] = vec3[2](look.sun_dir.xyz, look.fill_dir.xyz);
            vec3 cols[2] = vec3[2](look.sun_color.rgb, look.fill_color.rgb);
            for (int k = 0; k < 2; k++) {
                if (dot(ways[k], ways[k]) == 0.0) continue;
                vec3 l = -normalize(ways[k]);
                float nl = dot(fn, l);
                float nh = dot(fn, normalize(fv + l));
                if (nl >= 0.0 && nh >= 0.0) spec += cols[k] * pow(nh, OL_SHINE);
            }
            spec *= texture(spec_map, uv).r;
        }
        vec3 fc = (c.rgb + spec / look.chara_spec.w) * OL_COLOUR * (1.0 - OL_LIGHT_FADE + OL_LIGHT_FADE * OL_AMBIENT);
        color = vec4(tinted(mix(fog_add, fc * look.chara_spec.w, fog_keep)) * pc.params.w, clamp(c.a, 0.0, 1.0));
        // Blended over the lit scene: what the specular ambient left under it shows by 1 - alpha.
        spec_light.a = color.a;
        return;
    }
    vec3 n = normalize(normal);
    if (is_chara()) {
        put_normal(n);
    }
    if (AO_PASS && is_chara()) {
        color = vec4(n, 1.0 / gl_FragCoord.w);
        return;
    }
    if (!is_chara()) {
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
            vec3 nm;
            if (dot(tangent.xyz, tangent.xyz) > 0.0) {
                // The game's frame: T the mesh's tangent, B = cross(N, T) * w, N the normal, none
                // of them orthogonalised; the game turns the sum by the view and encodes it
                // without normalising, so only its direction counts.
                vec3 tg = normalize(tangent.xyz);
                nm = normalize(tg * tn.y + cross(n, tg) * tangent.w * tn.x + n * tn.z);
            } else {
                // A mesh without a tangent: the frame from the screen derivatives of the
                // position and UV, green against the texture's v.
                highp vec3 dp1 = dFdx(world), dp2 = dFdy(world);
                highp vec2 du1 = dFdx(uv), du2 = dFdy(uv);
                highp vec3 p2 = cross(dp2, n), p1 = cross(n, dp1);
                highp vec3 tg = p2 * du1.x + p1 * du2.x;
                highp vec3 bt = p2 * du1.y + p1 * du2.y;
                highp float inv = inversesqrt(max(max(dot(tg, tg), dot(bt, bt)), 1e-20));
                nm = normalize(mat3(tg * inv, -bt * inv, n) * tn);
            }
            float after = max(dot(nm, -normalize(look.sun_dir.xyz)), 0.0);
            vec2 material = unpackUnorm2x16(pc.light);
            sun_now = look.sun_color.rgb * material.x * after;
            // The ambient, too, taken again on the mapped normal (the vertex's replaced).
            if (look.sh_on.x > 0.5) {
                field_now = max(field_now + (environment_light(nm) - environment_light(n)) * material.y, 0.0);
            }
            n = nm;
        }
        put_normal(n);
        if (AO_PASS) {
            color = vec4(n, 1.0 / gl_FragCoord.w);
            return;
        }
        float sun_lit = sun_shadow(world) * cloud_light(world);
        float occlusion = screen_occlusion();
        vec3 lit = field_now + sun_now * sun_lit;
        // A model with a specular map (flag 1 << 24): its specular colour takes its share of the
        // diffuse light (1 - F0), and the lights' highlights are added.
        vec3 f0 = vec3(0.0);
        float gloss = 0.0;
        vec3 highlights = vec3(0.0);
        bool specular = (pc.flags & (1u << 24)) != 0u;
        // A Ryza character (flag 128) takes the specular ambient only: its direct specular is
        // the lighting pass's own term (not done), not the field models' rule.
        bool chara = (pc.flags & 128u) != 0u;
        float spec_occlusion = occlusion;
        // The diffuse colour: metal (the specular map's blue at 0.5 or more) takes the colour
        // tile times 0.2 times the map's green, as the G-buffer programs write it, the field
        // models' and the characters' alike (the colour left over is its specular colour).
        vec3 albedo = c.rgb;
        if (specular) {
            if (texture(spec_map, uv).b >= 0.5) {
                albedo = c.rgb * (texture(spec_map, uv).g * 0.2);
            }
            specular_surface(c.rgb, f0, gloss);
            if (!chara) {
                lit *= 1.0 - f0;
                highlights = direct_specular(n, f0, gloss, sun_lit, occlusion);
            }
        }
        // The lighting pass's occlusion on all the diffuse light, tinted by the colour. (A Ryza
        // character's own lighting below takes it with its occlusion map.)
        if ((pc.flags & 128u) == 0u) {
            lit *= occlusion_tint(c.rgb, occlusion);
        }
        // Flag 32768: the emission map as light of its own (PBB-EmN's `sEmi × emScl`, added to
        // the light before the colour multiplies it).
        if ((pc.flags & 32768u) != 0u) {
            lit += texture(emissive, uv).rgb * look.emission.x;
        }
        // A Ryza character (flag 128), lit as the game's character lighting pass lights it
        // (`DSChara-Cll`, docs/shading.md): two directional lights, each with its diffuse weight
        // `sat(rim * sat(litDir . V) + sat(N . L))`, the occlusion map's and the shadow's `min`
        // on the sun, the albedo's own colour toward the light by the occlusion, the ambient,
        // then the toon ramp (the mask slot holds `chara_rmp`, its row in bits 16-23). The
        // specular term and the back-light amount (the G-buffer's, 0 and 0 here) are not done.
        if ((pc.flags & 128u) != 0u) {
            const float OCC_COLOUR = 0.50278664;              // fOccAlbScl, as captured at Rasenboden
            const float OCC_ON_SUN = 0.24965833;              // IRAFade, as captured there
            const vec2 OCC_ON_AMBIENT = vec2(0.75268143, 0.24731857); // vAmbOccRat, as captured there
            const float OCC_DEEP = 0.2;                       // occThresForIRShadow, as captured there
            const vec2 RAMP_SCALE = vec2(0.27, 0.116);        // rampCoordScaler, as captured there
            const vec2 RAMP_RANGE = vec2(0.2, 0.5);           // rampCoordRange, as captured there
            const vec2 RIM = vec2(1.0, 2.0);                  // cRimTbl's entry (strength, power), as captured there
            vec2 material = unpackUnorm2x16(pc.light);
            vec3 to_eye = normalize(look.eye.xyz - world);
            float rim = pow(1.0 - clamp(dot(n, to_eye), 0.0, 1.0), RIM.y) * RIM.x;
            // The two lights: the sun, and the second light at its own scale (sh_on.y over the draw's).
            vec3 way0 = -normalize(look.sun_dir.xyz);
            vec3 way1 = dot(look.fill_dir.xyz, look.fill_dir.xyz) > 0.0 ? -normalize(look.fill_dir.xyz) : vec3(0.0);
            vec3 col0 = look.sun_color.rgb * material.x;
            vec3 col1 = look.fill_color.rgb * material.x * look.sh_on.y;
            float nl0 = dot(n, way0), nl1 = dot(n, way1);
            float weight0 = clamp(rim * clamp(dot(-way0, to_eye), 0.0, 1.0) + clamp(nl0, 0.0, 1.0), 0.0, 1.0);
            float weight1 = clamp(rim * clamp(dot(-way1, to_eye), 0.0, 1.0) + clamp(nl1, 0.0, 1.0), 0.0, 1.0);
            // The occlusion map (the emissive slot holds it, white without one; its green) times
            // the screen occlusion; the sun takes the smaller of it and the shadow (a character's
            // row 0 takes the shadow alone).
            float row = float((pc.flags >> 16) & 255u);
            float occ = texture(emissive, uv).g * occlusion;
            spec_occlusion = occ;
            float shadow = sun_shadow(world) * cloud_light(world);
            float lit_part = row > 0.0 ? min(occ, shadow) : shadow;
            float occ_light = row > 0.0 ? 1.0 + OCC_ON_SUN * (occ - 1.0) : occ;
            vec3 own = albedo * OCC_COLOUR;
            vec3 toward_light = mix(own, vec3(1.0), occ_light);
            lit = field_now * mix(own, vec3(1.0), occ * OCC_ON_AMBIENT.x + OCC_ON_AMBIENT.y)
                + (col0 * weight0 * lit_part + col1 * weight1) * toward_light;
            // The ramp: u the larger of 0.27 times the sun's half-Lambert term times the lit part
            // and 0.116 times the second light's, kept inside `rampCoordRange`, so only the row's
            // short run from its shade tint toward white is used; under it where the occlusion is
            // deep. v is the row before the material's `rampIndex` (index 0: no ramp).
            if (row > 0.0) {
                float u = clamp(max(RAMP_SCALE.x * clamp(nl0 * 0.5 + 0.5, 0.0, 1.0) * lit_part, RAMP_SCALE.y * clamp(nl1 * 0.5 + 0.5, 0.0, 1.0)), RAMP_RANGE.x, RAMP_RANGE.y);
                if (occ < OCC_DEEP) {
                    u = occ * RAMP_RANGE.x;
                }
                lit *= textureLod(mask, vec2(u, (row - 0.5) / 256.0), 0.0).rgb;
            }
        }
        vec3 rgb = tinted(mix(fog_add, lit * albedo * at_color + highlights, fog_keep));
        if (specular && look.spec_amb.x > 0.0) {
            vec3 factor, env;
            rgb += chara ? specular_light(n, f0, gloss, spec_occlusion, look.chara_spec.rgb, factor, env) : specular_light(n, f0, gloss, occlusion, look.spec_amb.x * look.spec_tint.rgb, factor, env);
            spec_light = vec4(factor * fog_keep, gloss);
            spec_env = vec4(env * pc.params.w, 0.0);
        }
        color = vec4(rgb * pc.params.w, 1.0);
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
