#version 450
#extension GL_GOOGLE_include_directive : require
// Field terrain, as the field's own terrain programs draw it (docs/formats.md, "Patches and
// their draws"): each triangle belongs to one draw that mixes one or two tiled layers by a pair
// of one blend map's channels, each divided by their sum.
layout(set = 0, binding = 0) uniform sampler2D blend[8];
layout(set = 0, binding = 1) uniform sampler2D layers[20];
// Each layer's normal map (Ryza's, as stored: red, green and blue the normal's parts in the
// ground's frame, alpha the gloss), or flat ones where the game has none.
layout(set = 0, binding = 2) uniform sampler2D normals[20];
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
    // Slot 66: the cube map's light on (x > 0.5); 67 to 75 its spherical-harmonic
    // coefficients (model.vert's `environment_light`); 76 to 86 the sky plane's.
    vec4 sh_on;
    vec4 sh[9];
    vec4 others[11];
    // The far shadow square (lib.rs SHADOW_FAR_SLOT): view-projection, {texel, layer (0:
    // none), fade start, fade end}.
    mat4 shadow_far;
    vec4 shadow_far_info;
    // Slots 92 to 108: the vertex program's, the emission's, the sky's.
    vec4 frag_pad[17];
    // 109: the specular ambient's (model.frag); z: the game's per-pixel lighting of the
    // ground (its normal maps and direct specular) on, for the Ryza games' fields.
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
    vec4 chara_spec; // CHARA_SPECULAR_SLOT, not read here
    // The cascades' and the static layer's maps within their layers (lib.rs SHADOW_SIDES_SLOT).
    vec4 shadow_sides[4];
} look;
layout(set = 1, binding = 5) uniform sampler2DArrayShadow shadow_map;
// The sun's shadow at a render-space point: MapSTD's 16 taps (four 2 × 2 comparisons a texel
// and a half apart), 1 lit to 0 shadowed (atColor white: the full strength).
layout(set = 1, binding = 6) uniform sampler2D pattern_map;

#include "sun_light.glsl"

#define AO_SET 2
#include "occlusion.glsl"
#include "normal_octahedral.glsl"
#include "environment_light.glsl"
#include "specular_direct.glsl"

layout(location = 0) out vec4 color;
// Set by the ambient occlusion's pipeline (ao.rs): the normal and depth are written, not the light.
layout(constant_id = 0) const bool AO_PASS = false;
// The motion vector (`--motion`): how far this point moved on screen since the last frame, in
// texture coordinates (no attachment takes it when motion is off).
layout(location = 9) in vec4 cur_clip;
layout(location = 10) in vec4 prev_clip;
layout(location = 1) out vec2 motion;
// The world normal takes the motion vectors' attachment where there is no pre-pass (as in model.frag).
layout(constant_id = 1) const bool NORMAL_AS_MOTION = false;
// Whether anything reads the motion vectors (model.vert's MOTION, the same for both).
layout(constant_id = 5) const bool MOTION = true;
// The ground's normal from the blended normal map `nm`, in the frame of the vertex normal (the
// game's space, x not mirrored; the map's red along the side axis, green along the slope, blue
// along the normal), back in the render space.
vec3 ground_normal(vec4 nm) {
    vec3 up = normalize(vec3(-normal.x, normal.y, normal.z));
    vec3 slope = normalize(vec3(0.0, up.z, -up.y));
    vec3 side = cross(slope, up);
    vec3 t = normalize(nm.rgb - 0.5);
    vec3 n_game = up * t.z + side * t.x + slope * t.y;
    return normalize(vec3(-n_game.x, n_game.y, n_game.z));
}

void main() {
    motion = NORMAL_AS_MOTION || !MOTION ? vec2(0.0) : (cur_clip.xy / cur_clip.w - prev_clip.xy / prev_clip.w) * 0.5;
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
    // The colour tiles (alpha: the specular colour) and, with the per-pixel lighting, the
    // normal maps (alpha: the gloss).
    bool per_pixel = look.spec_amb.z > 0.5;
    vec4 ta = vec4(0.0), tb = vec4(0.0), na = vec4(0.0), nb = vec4(0.0);
    vec3 m = vec3(0.0);
    for (int i = 0; i < 20; i++) {
        if (i == la) {
            ta = textureGrad(layers[i], ua, ua_dx, ua_dy);
            if (per_pixel) na = textureGrad(normals[i], ua, ua_dx, ua_dy);
        }
        if (i == lb) {
            tb = textureGrad(layers[i], ub, ub_dx, ub_dy);
            if (per_pixel) nb = textureGrad(normals[i], ub, ub_dx, ub_dy);
        }
    }
    // Every map once (the pair's and the bake's channels read from them).
    vec3 maps[8];
    for (int i = 0; i < 8; i++) maps[i] = textureGrad(blend[i], map_uv, m_dx, m_dy).rgb;
    m = maps[mix_code >> 2];
    // The bake (the game's screen-sized target, sRtt): each entry's tile times its map channel,
    // summed, and the weights summed (twice the target's alpha in the game).
    vec4 bake = vec4(0.0), bake_n = vec4(0.0);
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
            vec4 t = vec4(0.0), tn = vec4(0.0);
            for (int i = 0; i < 20; i++) {
                if (i == layer) {
                    t = textureGrad(layers[i], uk, uk_dx, uk_dy);
                    if (per_pixel) tn = textureGrad(normals[i], uk, uk_dx, uk_dy);
                }
            }
            bake += t * w;
            bake_n += tn * w;
            bake_w += w;
        }
    }
    vec4 c = ta, nm = na;
    if ((mix_code & 3) != 3) {
        // The pairs: red and green, green and blue, blue and red.
        int p = mix_code & 3;
        vec2 w = p == 0 ? m.rg : (p == 1 ? m.gb : m.br);
        // Programs 9-11: the pair divided by its sum; 13-15: by the sum with the bake's.
        float total = w.x + w.y + bake_w + 1.0 / 1024.0;
        c = (ta * w.x + tb * w.y + bake) / total;
        nm = (na * w.x + nb * w.y + bake_n) / total;
    } else if (baked) {
        // Program 12: the tile weighs what the bake leaves.
        float wa = clamp(1.0 - bake_w, 0.0, 1.0);
        c = bake / max(bake_w + wa, 1.0 / 1024.0) + ta * wa;
        nm = bake_n / max(bake_w + wa, 1.0 / 1024.0) + na * wa;
    }
    // The ambient occlusion's pass (ao.rs, the pipeline made with AO_PASS): the ground's normal
    // and depth in place of its light.
    {
        vec3 n = per_pixel ? ground_normal(nm) : normalize(normal);
        if (NORMAL_AS_MOTION) motion = octahedral_encode(n);
        if (AO_PASS) {
            color = vec4(n, 1.0 / gl_FragCoord.w);
            return;
        }
    }
    float sun_lit = sun_shadow(world) * cloud_light(world);
    // The stage dump of the sun's share (`dump::Stop::Shadow`).
    if (look.shadow_info.w > 0.5) {
        color = vec4(vec3(look.shadow_info.w > 1.5 ? 0.0 : sun_lit) * pc.params.w, 1.0);
        return;
    }
    if (per_pixel) {
        // The ground as the game's passes light it (docs/shading.md): the normal from the
        // blended normal maps in the frame of the vertex normal (the game's space, x not
        // mirrored; the map's red along the side axis, green along the slope, blue along the
        // normal), the specular colour from the colour tiles' alpha and the gloss from the
        // normal maps'; no vertex colour. The diffuse light takes (1 - F0) of itself, the two
        // lights' highlights add.
        vec3 n = ground_normal(nm);
        vec3 ambient = look.sh_on.x > 0.5 ? environment_light(n) : look.ambient.rgb;
        vec3 fill = (dot(look.fill_dir.xyz, look.fill_dir.xyz) > 0.0 ? max(dot(n, -normalize(look.fill_dir.xyz)), 0.0) : 0.0) * look.fill_color.rgb;
        vec3 sun = (dot(look.sun_dir.xyz, look.sun_dir.xyz) > 0.0 ? max(dot(n, -normalize(look.sun_dir.xyz)), 0.0) : 0.0) * look.sun_color.rgb;
        vec3 f0 = vec3(c.a);
        float occlusion = screen_occlusion();
        vec3 lit = (ambient + fill + sun * sun_lit) * (1.0 - f0) * occlusion_tint(c.rgb, occlusion);
        vec3 highlights = direct_specular(n, f0, nm.a, sun_lit, occlusion);
        color = vec4(mix(fog_add, lit * c.rgb + highlights, fog_keep) * pc.params.w, 1.0);
        return;
    }
    // Lit as the field's own terrain program does (its pack's pixel program 9 in TOWN_10):
    // the same per-vertex light as the field's models (the vertex colour white), on the
    // blended layers; the light texture, shadow and specular are not done yet.
    color = vec4(mix(fog_add, (field_light + sun_part * sun_lit) * c.rgb, fog_keep) * pc.params.w, 1.0);
}
