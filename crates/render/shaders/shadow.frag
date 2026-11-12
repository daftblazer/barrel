#version 450
// The sun's shadow map: depth only, with the colour texture's alpha test (leaves, fences).
layout(set = 0, binding = 0) uniform sampler2D tex;
layout(push_constant) uniform Push { mat4 view_proj; vec4 params; } pc;
layout(location = 1) in vec2 uv;
void main() {
    if (pc.params.x >= 0.0 && texture(tex, uv).a < pc.params.x) discard;
}
