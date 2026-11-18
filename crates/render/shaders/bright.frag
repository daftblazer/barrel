#version 450
// The Ryza games' bloom's bright pass (docs/shading.md, "The bloom's bright pass"): the scene
// taken down by four taps to a quarter, as stored at half, c x saturate((luma - T) / (white -
// T)). T and the white point are in `step`; `metered` 1: the scene is first taken to the
// metered exposure (the second channel of `exposure`, its ratio to the one the scene was lit with).
layout(set = 0, binding = 0) uniform sampler2D src;
layout(set = 1, binding = 0) uniform sampler2D exposure;
layout(push_constant) uniform Push { vec2 step; float metered; uint pad; } pc;
layout(location = 0) in vec2 uv;
layout(location = 0) out vec4 color;
void main() {
    vec2 t = 1.0 / vec2(textureSize(src, 0));
    vec3 s = (texture(src, uv + t * vec2(-1.0, -1.0)).rgb + texture(src, uv + t * vec2(1.0, -1.0)).rgb
        + texture(src, uv + t * vec2(-1.0, 1.0)).rgb + texture(src, uv + t * vec2(1.0, 1.0)).rgb) * 0.5;
    if (pc.metered > 0.5) {
        s *= texelFetch(exposure, ivec2(0), 0).g;
    }
    float luma = dot(s, vec3(0.29891, 0.58661, 0.11448));
    color = vec4(s * clamp((luma - pc.step.x) / (pc.step.y - pc.step.x), 0.0, 1.0) * 0.5, 1.0);
}
