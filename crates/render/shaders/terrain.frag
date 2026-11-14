#version 450
// Field terrain, as the field's own terrain programs draw it (docs/formats.md, "Patches and
// their draws"): each triangle belongs to one draw that mixes one or two tiled layers by a pair
// of one blend map's channels, each divided by their sum.
layout(set = 0, binding = 0) uniform sampler2D blend[8];
layout(set = 0, binding = 1) uniform sampler2D layers[20];
layout(push_constant) uniform Push { mat4 view_proj; vec4 params; vec4 scale_a; vec4 scale_b; } pc;
layout(location = 0) in vec3 normal;
// x: layer a + 32 × layer b + 1024 × blend (map × 4 + channel pair, 3 one layer alone)
// + 32768 when the draw adds the bake; y: layer a's repeat + 256 × layer b's, in cells
// (Scene::add_terrain). The same over a draw. The patch's bake list is in scale_a.zw and
// scale_b, as bits: the count, then two entries a word (scene_gpu::bake_entry).
layout(location = 1) in vec2 uv;
layout(location = 2) in vec3 world;
// The field light at this vertex (model.vert), the sun's part apart.
layout(location = 4) in vec3 field_light;
layout(location = 6) in vec3 sun_part;
layout(location = 7) in vec3 fog_add;
layout(location = 8) in vec3 fog_keep;
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
    // Slots 66 to 86: the vertex program's and the sky plane's.
    vec4 others[21];
    // The far shadow layer (lib.rs SHADOW_FAR_SLOT): view-projection, {texel, on, fade
    // start, fade end}.
    mat4 shadow_far;
    vec4 shadow_far_info;
} look;
layout(set = 1, binding = 5) uniform sampler2DArrayShadow shadow_map;
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
    return texture(pattern_map, uv).r;
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
    float far = shadow_layer(look.shadow_far, 1.0, look.shadow_far_info.x, p);
    if (far < 0.0) return 1.0;
    float fade = clamp((distance(p, look.eye.xyz) - look.shadow_far_info.z) / max(look.shadow_far_info.w - look.shadow_far_info.z, 1.0), 0.0, 1.0);
    return mix(far, 1.0, fade);
}

layout(location = 0) out vec4 color;
// The motion vector (`--motion`): how far this point moved on screen since the last frame, in
// texture coordinates (no attachment takes it when motion is off).
layout(location = 9) in vec4 cur_clip;
layout(location = 10) in vec4 prev_clip;
layout(location = 1) out vec2 motion;
void main() {
    motion = (cur_clip.xy / cur_clip.w - prev_clip.xy / prev_clip.w) * 0.5;
    int code = int(uv.x + 0.5), reps = int(uv.y + 0.5);
    int la = code & 31, lb = (code >> 5) & 31, mix_code = (code >> 10) & 31;
    bool baked = (code & 32768) != 0;
    float cell = pc.params.y;
    // The field's own coordinates (x mirrored back), across the field for the maps.
    vec2 field_xz = vec2(-world.x, world.z);
    vec2 map_uv = field_xz / pc.scale_a.xy;
    // The layer and map numbers vary by triangle, so each slot is read by a constant index
    // (no descriptor indexing needed), with gradients taken outside the branches.
    vec2 ua = field_xz / (cell * float(reps & 255)), ub = field_xz / (cell * float(reps >> 8));
    vec2 ua_dx = dFdx(ua), ua_dy = dFdy(ua), ub_dx = dFdx(ub), ub_dy = dFdy(ub);
    vec2 m_dx = dFdx(map_uv), m_dy = dFdy(map_uv);
    vec3 ta = vec3(0.0), tb = vec3(0.0), m = vec3(0.0);
    for (int i = 0; i < 20; i++) {
        if (i == la) ta = textureGrad(layers[i], ua, ua_dx, ua_dy).rgb;
        if (i == lb) tb = textureGrad(layers[i], ub, ub_dx, ub_dy).rgb;
    }
    // Every map once (the pair's and the bake's channels read from them).
    vec3 maps[8];
    for (int i = 0; i < 8; i++) maps[i] = textureGrad(blend[i], map_uv, m_dx, m_dy).rgb;
    m = maps[mix_code >> 2];
    // The bake (the game's screen-sized target, sRtt): each entry's tile times its map channel,
    // summed, and the weights summed (twice the target's alpha in the game).
    vec3 bake = vec3(0.0);
    float bake_w = 0.0;
    if (baked) {
        uint words[5] = uint[5](floatBitsToUint(pc.scale_a.w), floatBitsToUint(pc.scale_b.x), floatBitsToUint(pc.scale_b.y), floatBitsToUint(pc.scale_b.z), floatBitsToUint(pc.scale_b.w));
        int count = int(floatBitsToUint(pc.scale_a.z));
        for (int k = 0; k < 10; k++) {
            if (k >= count) break;
            uint e = (words[k >> 1] >> (16 * (k & 1))) & 0xffffu;
            int layer = int(e & 31u), map = int((e >> 5) & 7u), channel = int((e >> 8) & 3u), reps_k = int((e >> 10) & 15u);
            vec3 mm = maps[map];
            float w = channel == 0 ? mm.r : (channel == 1 ? mm.g : mm.b);
            vec2 uk = field_xz / (cell * float(max(reps_k, 1)));
            vec2 uk_dx = dFdx(uk), uk_dy = dFdy(uk);
            vec3 t = vec3(0.0);
            for (int i = 0; i < 20; i++) {
                if (i == layer) t = textureGrad(layers[i], uk, uk_dx, uk_dy).rgb;
            }
            bake += t * w;
            bake_w += w;
        }
    }
    vec3 c = ta;
    if ((mix_code & 3) != 3) {
        // The pairs: red and green, green and blue, blue and red.
        int p = mix_code & 3;
        vec2 w = p == 0 ? m.rg : (p == 1 ? m.gb : m.br);
        // Programs 9-11: the pair divided by its sum; 13-15: by the sum with the bake's.
        float total = w.x + w.y + bake_w + 1.0 / 1024.0;
        c = (ta * w.x + tb * w.y + bake) / total;
    } else if (baked) {
        // Program 12: the tile weighs what the bake leaves.
        float wa = clamp(1.0 - bake_w, 0.0, 1.0);
        c = bake / max(bake_w + wa, 1.0 / 1024.0) + ta * wa;
    }
    // Lit as the field's own terrain program does (its pack's pixel program 9 in TOWN_10):
    // the same per-vertex light as the field's models (the vertex colour white), on the
    // blended layers; the light texture, shadow and specular are not done yet.
    color = vec4(mix(fog_add, (field_light + sun_part * sun_shadow(world) * cloud_light(world)) * c, fog_keep) * pc.params.w, 1.0);
}
