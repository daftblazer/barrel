#version 450
layout(location = 0) in vec3 in_pos;
layout(location = 1) in vec3 in_normal;
layout(location = 2) in vec2 in_uv;
layout(location = 3) in uvec4 in_joints;
layout(location = 4) in vec4 in_weights;
// Morph targets: where this vertex's offsets begin, and (count | weight offset << 16).
layout(location = 5) in uvec2 in_morph;
layout(location = 6) in vec4 in_color;
// A billboard corner (MapSTD-B*'s PSIZE), turned by the camera's axes below; zero otherwise.
layout(location = 7) in vec3 in_corner;
// view_proj: the camera; instance: which of the instances places this draw.
layout(push_constant) uniform Push { mat4 view_proj; vec4 params; vec4 scale_a; vec4 scale_b; uint instance; uint flags; float outline_colour;
    // A field model's lighting material: cDiff and cAmb as unorm16 (Draw::light).
    uint light; } pc;
// Each instance's model matrix (render space, x already mirrored) and where its joints'
// matrices begin in the palette. Instance 0 is the identity, for geometry baked in place.
// info: x the palette's first joint, y the first morph weight, z last frame's first joint;
// prev_model last frame's model matrix (both for the motion vectors).
struct Instance { mat4 model; uvec4 info; mat4 prev_model; vec4 tint_mul; vec4 tint_add; };
layout(std430, set = 1, binding = 0) readonly buffer Instances { Instance inst[]; } instances;
// Joint matrices (bind pose to posed, render space) of every posed model.
layout(std430, set = 1, binding = 1) readonly buffer Palette { mat4 joint[]; } palette;
// Morph target offsets, vertex by vertex and target by target, and each instance's weights.
layout(std430, set = 1, binding = 2) readonly buffer Deltas { vec4 delta[]; } deltas;
layout(std430, set = 1, binding = 3) readonly buffer Weights { float weight[]; } weights;
layout(location = 0) out vec3 normal;
layout(location = 1) out vec2 uv;
layout(location = 2) out vec3 world;
layout(location = 3) out vec3 scene_light;
// A field model's light at this vertex (MapSTD's v1 + v6, with no light texture or shadow
// yet: docs/shading.md, "Fields").
layout(location = 4) out vec3 field_light;
// The outline texture's coordinates: the screen position over 5 (CharaFra*'s o1.zw).
layout(location = 5) out vec2 screen_uv;
// The sun's part of a field model's light, kept apart for its shadow (MapSTD's v6).
layout(location = 6) out vec3 sun_part;
// The fog at this vertex (MapSTD's and CharaMTC's v3 and v4): the colour it adds and how much
// of the surface's own colour is left.
layout(location = 7) out vec3 fog_add;
layout(location = 8) out vec3 fog_keep;
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
    // The billboards' axes (MapSTD-B0's mBbL2W): the camera's right, up and back.
    vec4 bb_right;
    vec4 bb_up;
    vec4 bb_back;
    // Last frame's view-projection (this frame's when there was none), for motion vectors.
    mat4 prev_view_proj;
    // This frame's view-projection without the jitter (the drawn one, pc.view_proj, has it).
    mat4 view_proj;
    // The field's light texture's mapping (model.frag).
    vec4 pattern;
    vec4 pattern_on;
    // The grass sway groups (`<v_shaker>`, game::wind): {wind.xyz, clock}, {1 / height,
    // root height, shift x, shift z} each, metres.
    vec4 sway[16];
    // The ambient from an environment light map (the Ryza games' `_ad` cube map) as nine
    // spherical-harmonic coefficients over the game's axes (x not mirrored), when x of `sh_on`
    // is 1; otherwise `ambient` alone.
    vec4 sh_on;
    vec4 sh[9];
    // Slots 76 to 91: the sky plane's and the far shadow square's.
    vec4 vert_pad[16];
    // The light an upward-facing surface takes ({rgb}, lib.rs DOME_LIGHT_SLOT): flag 4096.
    vec4 dome_light;
    // The fade range from the eye {begin, end} (lib.rs FADE_SLOT): flag 16384.
    vec4 fade;
} look;
// This frame's and last frame's clip positions, for the motion vectors (`--motion`).
layout(location = 9) out vec4 cur_clip;
layout(location = 10) out vec4 prev_clip;
// The instance's colour transform (lib.rs `Instance::tint_mul` / `tint_add`).
layout(location = 11) flat out vec4 tint_mul;
layout(location = 12) flat out vec4 tint_add;
// The environment light map's light on a surface facing n (render space), from its nine
// coefficients (game space: x mirrored back).
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

// A directional light's Lambert term on normal n; a light with no direction (one the field
// does not have) gives none, rather than NaN.
float lambert(vec3 n, vec3 dir) {
    return dot(dir, dir) > 0.0 ? max(dot(n, -normalize(dir)), 0.0) : 0.0;
}

void main() {
    Instance i = instances.inst[pc.instance];
    vec4 pos = vec4(in_pos, 1.0);
    // Facial morphs first, in the bind pose, as the game does before skinning.
    uint count = in_morph.y & 0xffffu;
    uint first_weight = i.info.y + (in_morph.y >> 16);
    for (uint k = 0u; k < count; k++) {
        // Offsets are stored in game space: mirror x like the vertices.
        pos.xyz += weights.weight[first_weight + k] * deltas.delta[in_morph.x + k].xyz * vec3(-1.0, 1.0, 1.0);
    }
    vec3 n = in_normal;
    float total = dot(in_weights, vec4(1.0));
    // Last frame's position too, for the motion vectors: the same vertex on last frame's joints
    // and model matrix (its morphs taken as this frame's).
    vec4 prev_pos = pos;
    if (total > 0.0) {
        uint base = i.info.x;
        mat4 skin = (in_weights.x * palette.joint[base + in_joints.x] + in_weights.y * palette.joint[base + in_joints.y]
            + in_weights.z * palette.joint[base + in_joints.z] + in_weights.w * palette.joint[base + in_joints.w]) / total;
        uint prev = i.info.z;
        mat4 prev_skin = (in_weights.x * palette.joint[prev + in_joints.x] + in_weights.y * palette.joint[prev + in_joints.y]
            + in_weights.z * palette.joint[prev + in_joints.z] + in_weights.w * palette.joint[prev + in_joints.w]) / total;
        pos = skin * pos;
        prev_pos = prev_skin * prev_pos;
        n = mat3(skin) * n;
    }
    vec4 w = i.model * pos;
    vec4 prev_w = i.prev_model * prev_pos;
    // Grass swaying in the wind (MapSTD's vertex programs, docs/shading.md "Wind"), for the
    // field models of a sway group (flags bits 8-11: its number + 1), in the game's metres and
    // with its x (render x mirrored): phase = x·shift.x + time + z·shift.z, height =
    // saturate((y - the model's y - root) / height), moved by wind · sin(phase) · height.
    uint group = (pc.flags >> 8) & 15u;
    if (group != 0u && (pc.flags & 1u) == 0u) {
        vec4 a = look.sway[2u * (group - 1u)];
        vec4 b = look.sway[2u * (group - 1u) + 1u];
        vec3 g = vec3(-w.x, w.y, w.z) / 100.0;
        float phase = g.x * b.z + a.w + g.z * b.w;
        float h = clamp((g.y - i.model[3].y / 100.0 - b.y) * b.x, 0.0, 1.0);
        vec3 move = a.xyz * sin(phase) * h * 100.0;
        w.xyz += vec3(-move.x, move.y, move.z);
        prev_w.xyz += vec3(-move.x, move.y, move.z);
    }
    // Billboards: the corner, and the normal, in the camera's frame (MapSTD-B0 program 0).
    mat3 bb = mat3(look.bb_right.xyz, look.bb_up.xyz, look.bb_back.xyz);
    if (in_corner != vec3(0.0)) {
        // Flag 16384 (a field's plants, `ObjectGrassPBS`): the card shrinks to nothing from
        // the fade range's begin to its end, by its foot's distance from the eye.
        float shrink = 1.0;
        if ((pc.flags & 16384u) != 0u) {
            shrink = clamp(1.0 - (distance(w.xyz, look.eye.xyz) - look.fade.x) / max(look.fade.y - look.fade.x, 1.0), 0.0, 1.0);
            // And its top corners lean with the wind as `ObjectGrassPBS`'s program 0 leans
            // them (sway slot 0 `g_vAttCache0`, slot 1 {`g_fAttCache1`, `g_fPhase`,
            // `g_fOscPhase`}; the plant's own phase is its position's fraction): a lean of
            // the card's height times 0.01 of the wind, normalised back to the height, so the
            // top swings on an arc and the foot stays put. In game space, x mirrored here.
            float hh = in_corner.y * shrink;
            if (hh > 0.01) {
                vec4 c = look.sway[0];
                vec4 p = look.sway[1];
                float f = fract(-w.x) * p.x * 6.283185;
                float k = 1.0 - p.x;
                float swing = sin(f + p.y) * 0.5 + 1.0;
                float sx = sin(f + p.z) - cos(p.z) * k;
                float cz = cos(f + p.z) - sin(p.z) * k;
                vec4 r = hh * 0.01 * c;
                vec3 lean = normalize(vec3(r.x * swing + r.z * sx, hh, r.y * swing + r.w * cz)) * hh;
                vec3 move = vec3(-lean.x, lean.y - hh, lean.z);
                w.xyz += move;
                prev_w.xyz += move;
            }
        }
        w.xyz += bb * in_corner * shrink;
        prev_w.xyz += bb * in_corner * shrink;
        n = bb * n;
    }
    world = w.xyz;
    // The point lights at this vertex, as the game's character vertex program sums them:
    // (colour + ambient) × (1 - saturate(d² / range²)) / (att0 + att1·d + att2·d²), d in field
    // units (centimetres) and the range scaled to them.
    vec3 sum = vec3(0.0);
    for (int k = 0; k < int(look.misc.y); k++) {
        vec3 d = look.point_pos[k].xyz - w.xyz;
        float d2 = dot(d, d);
        float falloff = look.point_pos[k].w > 0.00001 ? 1.0 - clamp(d2 / look.point_pos[k].w, 0.0, 1.0) : 0.0;
        float denom = look.point_att[k].x + look.point_att[k].y * sqrt(d2) + look.point_att[k].z * d2;
        sum += denom > 0.00001 ? (look.point_color[k].rgb + look.point_ambient[k].rgb) * falloff / denom : vec3(0.0);
    }
    scene_light = clamp(sum, 0.0, 1.0);
    // Fields: ambient (and each point light's ambient) on the vertex colour, the fill and the
    // point lights' diffuse, and the sun's diffuse; the material colours are taken as white.
    vec3 wn = normalize(mat3(i.model) * n);
    // (A Ryza character, flag 128, takes the flat ambient: its own, not the stage's.)
    vec3 amb = look.sh_on.x > 0.5 && (pc.flags & 128u) == 0u ? environment_light(wn) : look.ambient.rgb;
    vec3 diffuse = lambert(wn, look.fill_dir.xyz) * look.fill_color.rgb;
    // The terrain (flag 64) takes no point lights: its vertex program (the field's `.g1s`
    // program 2) has only the two directional lights and the ambient.
    int points = (pc.flags & 64u) != 0u ? 0 : int(look.misc.y);
    for (int k = 0; k < points; k++) {
        vec3 d = look.point_pos[k].xyz - w.xyz;
        float d2 = dot(d, d);
        if (d2 < look.point_pos[k].w) {
            float att = 1.0 / max(look.point_att[k].x + look.point_att[k].y * sqrt(d2) + look.point_att[k].z * d2, 0.00001);
            diffuse += att * max(dot(wn, normalize(d)), 0.0) * look.point_color[k].rgb;
            amb += att * look.point_ambient[k].rgb;
        }
    }
    vec3 sun = lambert(wn, look.sun_dir.xyz) * look.sun_color.rgb;
    // MapSTD: d = vertex colour x cDiff takes the lights' diffuse, the ambient takes the vertex
    // colour x cAmb (the model's lighting material, 1 in nearly every field model).
    vec2 material = unpackUnorm2x16(pc.light);
    field_light = amb * in_color.rgb * material.y + in_color.rgb * material.x * diffuse;
    sun_part = in_color.rgb * material.x * sun;
    // The distant view's dome and clouds are not lit: their texture on their vertex colour
    // (flag 16: its solid pieces, lit as the field's models are); nor are the field's xEF_
    // models (flag 32: window light).
    if ((pc.flags & 24u) == 8u || (pc.flags & 32u) != 0u) {
        // Flag 4096: lit by the light on an upward-facing surface (Ryza's sky dome).
        field_light = (pc.flags & 4096u) != 0u ? in_color.rgb * look.dome_light.rgb : in_color.rgb;
        sun_part = vec3(0.0);
    }
    gl_Position = pc.view_proj * w;
    tint_mul = i.tint_mul;
    tint_add = i.tint_add;
    cur_clip = look.view_proj * w;
    prev_clip = look.prev_view_proj * prev_w;
    // Fog: extinction exp2(s × Scat0) (Scat0 holds -(βR + βM) log2 e), s the eye distance mapped
    // over the fog's start and end; in-scattering Mie (Henyey-Greenstein) plus Rayleigh plus
    // the ambient, θ between the way to the eye and the sun.
    {
        vec3 e = look.eye.xyz - w.xyz;
        float s = look.fog.x * length(e) + look.fog.y;
        // The distant view's solid pieces (flag 16: ground ring, stones, plants) stand in for
        // what lies beyond the field: as far as the fog reaches. The dome and the clouds take
        // the fog at their own distance, as `MapSTD`/`MapSTD-T0` do: 30,000 units and more,
        // nearly all in-scattered light, which is the game's sky colour.
        if ((pc.flags & 24u) == 24u) {
            s = 1.0;
        }

        fog_keep = min(exp2(s * look.scat[0].xyz), vec3(1.0));
        // No sun (a field whose lights are not read): no Mie lobe toward it, rather than NaN.
        float c = dot(look.sun_dir.xyz, look.sun_dir.xyz) > 0.0 ? dot(normalize(e), normalize(look.sun_dir.xyz)) : 0.0;
        fog_add = look.scat[3].xyz * pow(max(look.scat[2].w * c + look.scat[3].w, 0.0001), -1.5) + (1.0 + c * c) * look.scat[2].xyz + look.scat[1].xyz;
    }
    if ((pc.flags & 4u) != 0u) {
        // The outline pass (CharaFra*): pushed out along the clip-space normal, about the same
        // width on screen (w capped at 350), thinner where the vertex colour's red says and a
        // little beyond 190 units from the eye; g_outlineThickness taken as 1.
        vec4 cn = pc.view_proj * vec4(mat3(i.model) * n, 0.0);
        cn = cn / max(length(cn), 0.00001);
        float clip_w = gl_Position.w;
        float dist = length(look.eye.xyz - w.xyz);
        float k = (1.75 - 0.75 * in_color.r) * sign(clip_w) * min(abs(clip_w), 350.0) * (1.0 - 0.2 * clamp((dist - 190.0) * 0.05, 0.0, 1.0));
        gl_Position += vec4(cn.x * 0.0025, cn.y * 0.0025 * pc.params.z, cn.z * 0.0025, cn.w * 0.0025) * k;
    }
    screen_uv = gl_Position.xy / (gl_Position.w * 5.0);
    // Instances are rotations and translations (no scale), so their upper 3x3 turns normals.
    normal = mat3(i.model) * n;
    uv = in_uv;
}
