#version 450
// The cloud map (docs/shading.md, "Ryza DX: the sky, the clouds"): the density map's value
// past a threshold, shaped to a soft edge, a channel for each layer (all alike at noon).
layout(set = 0, binding = 0) uniform sampler2D density;
layout(push_constant) uniform Push {
    vec4 threshold;
    vec4 sharpness;
    vec4 thinness;
    vec4 height;
} p;
layout(location = 0) in vec2 uv;
layout(location = 0) out vec4 colour;
void main() {
    vec4 x = clamp(vec4(texture(density, uv).r) - p.threshold, 0.0, 1.0) * p.sharpness;
    // 1 - 2^-x, to the power of the thinness (the game's programs' exp and log are base 2).
    vec4 e = 1.0 - exp2(-x);
    vec4 thin = exp2(p.thinness * log2(max(e, 1e-30)));
    colour = thin * p.height * vec4(greaterThan(e, vec4(0.0)));
}
