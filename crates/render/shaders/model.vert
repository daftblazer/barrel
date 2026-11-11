#version 450
layout(location = 0) in vec3 in_pos;
layout(location = 1) in vec3 in_normal;
layout(location = 2) in vec2 in_uv;
layout(push_constant) uniform Push { mat4 mvp; vec4 params; vec4 scale_a; vec4 scale_b; } pc;
layout(location = 0) out vec3 normal;
layout(location = 1) out vec2 uv;
void main() {
    gl_Position = pc.mvp * vec4(in_pos, 1.0);
    normal = in_normal;
    uv = in_uv;
}
