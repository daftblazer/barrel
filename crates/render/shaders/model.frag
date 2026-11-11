#version 450
layout(set = 0, binding = 0) uniform sampler2D tex;
layout(push_constant) uniform Push { mat4 mvp; vec4 params; vec4 scale_a; vec4 scale_b; } pc;
layout(location = 0) in vec3 normal;
layout(location = 1) in vec2 uv;
layout(location = 0) out vec4 color;
void main() {
    vec4 c = texture(tex, uv);
    if (c.a < pc.params.x) discard;
    vec3 n = normalize(normal);
    float light = 0.45 + 0.55 * max(dot(n, normalize(vec3(-0.3, 0.6, 0.7))), 0.0);
    color = vec4(c.rgb * light, 1.0);
}
