#version 450
layout(location = 0) in vec3 in_pos;
layout(location = 1) in vec3 in_normal;
layout(location = 2) in vec2 in_uv;
layout(location = 3) in uvec4 in_joints;
layout(location = 4) in vec4 in_weights;
// Morph targets: where this vertex's offsets begin, and (count | weight offset << 16).
layout(location = 5) in uvec2 in_morph;
layout(location = 6) in vec4 in_color;
// view_proj: the camera; instance: which of the instances places this draw.
layout(push_constant) uniform Push { mat4 view_proj; vec4 params; vec4 scale_a; vec4 scale_b; uint instance; uint flags; } pc;
// Each instance's model matrix (render space, x already mirrored) and where its joints'
// matrices begin in the palette. Instance 0 is the identity, for geometry baked in place.
struct Instance { mat4 model; uvec4 info; };
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
} look;
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
    if (total > 0.0) {
        uint base = i.info.x;
        mat4 skin = (in_weights.x * palette.joint[base + in_joints.x] + in_weights.y * palette.joint[base + in_joints.y]
            + in_weights.z * palette.joint[base + in_joints.z] + in_weights.w * palette.joint[base + in_joints.w]) / total;
        pos = skin * pos;
        n = mat3(skin) * n;
    }
    vec4 w = i.model * pos;
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
    vec3 amb = look.ambient.rgb;
    vec3 diffuse = max(dot(wn, -normalize(look.fill_dir.xyz)), 0.0) * look.fill_color.rgb;
    for (int k = 0; k < int(look.misc.y); k++) {
        vec3 d = look.point_pos[k].xyz - w.xyz;
        float d2 = dot(d, d);
        if (d2 < look.point_pos[k].w) {
            float att = 1.0 / max(look.point_att[k].x + look.point_att[k].y * sqrt(d2) + look.point_att[k].z * d2, 0.00001);
            diffuse += att * max(dot(wn, normalize(d)), 0.0) * look.point_color[k].rgb;
            amb += att * look.point_ambient[k].rgb;
        }
    }
    vec3 sun = max(dot(wn, -normalize(look.sun_dir.xyz)), 0.0) * look.sun_color.rgb;
    field_light = amb * in_color.rgb + in_color.rgb * diffuse + in_color.rgb * sun;
    gl_Position = pc.view_proj * w;
    // Instances are rotations and translations (no scale), so their upper 3x3 turns normals.
    normal = mat3(i.model) * n;
    uv = in_uv;
}
