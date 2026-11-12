#version 450
// Field terrain: two blend maps over the whole field weight six tiled ground layers.
layout(set = 0, binding = 0) uniform sampler2D blend[2];
layout(set = 0, binding = 1) uniform sampler2D layers[6];
layout(push_constant) uniform Push { mat4 view_proj; vec4 params; vec4 scale_a; vec4 scale_b; } pc;
layout(location = 0) in vec3 normal;
layout(location = 1) in vec2 uv; // 0..1 across the field
layout(location = 0) out vec4 color;
void main() {
    vec3 w0 = texture(blend[0], uv).rgb;
    vec3 w1 = texture(blend[1], uv).rgb;
    float w[6] = float[6](w0.r, w0.g, w0.b, w1.r, w1.g, w1.b);
    float s[6] = float[6](pc.scale_a.x, pc.scale_a.y, pc.scale_a.z, pc.scale_a.w, pc.scale_b.x, pc.scale_b.y);
    vec2 world = uv * pc.params.y;
    vec3 c = vec3(0.0);
    float total = 0.0;
    for (int i = 0; i < 6; i++) {
        c += w[i] * texture(layers[i], world * s[i]).rgb;
        total += w[i];
    }
    c = total > 0.001 ? c / total : texture(layers[0], world * s[0]).rgb;
    vec3 n = normalize(normal);
    float light = 0.45 + 0.55 * max(dot(n, normalize(vec3(-0.3, 0.6, 0.7))), 0.0);
    color = vec4(c * light * pc.params.w, 1.0);
}
