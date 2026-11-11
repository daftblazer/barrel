#version 450
layout(location = 0) in vec3 in_pos;
layout(location = 1) in vec3 in_normal;
layout(location = 2) in vec2 in_uv;
layout(location = 3) in uvec4 in_joints;
layout(location = 4) in vec4 in_weights;
// view_proj: the camera; instance: which of the instances places this draw.
layout(push_constant) uniform Push { mat4 view_proj; vec4 params; vec4 scale_a; vec4 scale_b; uint instance; } pc;
// Each instance's model matrix (render space, x already mirrored) and where its joints'
// matrices begin in the palette. Instance 0 is the identity, for geometry baked in place.
struct Instance { mat4 model; uvec4 info; };
layout(std430, set = 1, binding = 0) readonly buffer Instances { Instance inst[]; } instances;
// Joint matrices (bind pose to posed, render space) of every posed model.
layout(std430, set = 1, binding = 1) readonly buffer Palette { mat4 joint[]; } palette;
layout(location = 0) out vec3 normal;
layout(location = 1) out vec2 uv;
void main() {
    Instance i = instances.inst[pc.instance];
    vec4 pos = vec4(in_pos, 1.0);
    vec3 n = in_normal;
    float total = dot(in_weights, vec4(1.0));
    if (total > 0.0) {
        uint base = i.info.x;
        mat4 skin = (in_weights.x * palette.joint[base + in_joints.x] + in_weights.y * palette.joint[base + in_joints.y]
            + in_weights.z * palette.joint[base + in_joints.z] + in_weights.w * palette.joint[base + in_joints.w]) / total;
        pos = skin * pos;
        n = mat3(skin) * n;
    }
    gl_Position = pc.view_proj * i.model * pos;
    // Instances are rotations and translations (no scale), so their upper 3x3 turns normals.
    normal = mat3(i.model) * n;
    uv = in_uv;
}
