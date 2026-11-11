#version 450
layout(location = 0) in vec3 in_pos;
layout(location = 1) in vec3 in_normal;
layout(location = 2) in vec2 in_uv;
// view_proj: the camera; instance: which of the instances places this draw.
layout(push_constant) uniform Push { mat4 view_proj; vec4 params; vec4 scale_a; vec4 scale_b; uint instance; } pc;
// Each instance's model matrix (render space, x already mirrored). Instance 0 is the
// identity, for geometry baked in place (the field); characters each have their own, so they
// can move between frames without rebuilding the vertex buffer.
layout(std430, set = 1, binding = 0) readonly buffer Instances { mat4 model[]; } instances;
layout(location = 0) out vec3 normal;
layout(location = 1) out vec2 uv;
void main() {
    mat4 m = instances.model[pc.instance];
    gl_Position = pc.view_proj * m * vec4(in_pos, 1.0);
    // Instances are rotations and translations (no scale), so their upper 3x3 turns normals.
    normal = mat3(m) * in_normal;
    uv = in_uv;
}
