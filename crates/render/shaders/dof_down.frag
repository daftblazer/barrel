#version 450
// The depth of field's two quarter-size layers (pass 30, docs/shading.md, "The depth of field
// (type 0)"): the scene and its weight over a 4 x 4 block (four bilinear taps one pixel from the
// centre), each tap going to the layer its weight is on, by the ramp of the weight. Layer 0
// takes the taps with a weight under 1/2 (the near view: the game's front layer), layer 1 the rest
// (the far view: the back layer).
layout(set = 0, binding = 0) uniform sampler2D scene;
layout(set = 0, binding = 1) uniform sampler2D weight;
layout(push_constant) uniform Push {
    // The scene's size inverse.
    vec2 inverse_size;
} pc;
layout(location = 0) in vec2 uv;
layout(location = 0) out vec4 front;
layout(location = 1) out vec4 back;

// The program's constants: the ramp is 1 / (1 - 2|w - 1/2|) less this, at most 1; the weight a
// tap keeps on the layer it is not on.
const float RAMP_BIAS = 0.99218;
const float OFF_LAYER = 0.00001;

void main() {
    vec4 sum0 = vec4(0.0), sum1 = vec4(0.0);
    float weight0 = 0.0, weight1 = 0.0;
    for (int i = 0; i < 4; i++) {
        vec2 offset = vec2(i & 1, i >> 1) * 2.0 - 1.0;
        vec2 at = uv + offset * pc.inverse_size;
        vec4 tap = vec4(texture(scene, at).rgb, texture(weight, at).r);
        float ramp = min(1.0 / (max(1.0 - 2.0 * abs(tap.a - 0.5), 0.0) + OFF_LAYER) - RAMP_BIAS, 1.0);
        bool low = tap.a < 0.5;
        float w0 = low ? ramp : OFF_LAYER;
        float w1 = low ? OFF_LAYER : ramp;
        sum0 += tap * w0;
        sum1 += tap * w1;
        weight0 += w0;
        weight1 += w1;
    }
    front = vec4((sum0 / weight0).rgb, min(sum0.a / weight0, 0.5));
    back = vec4((sum1 / weight1).rgb, max(sum1.a / weight1, 0.5));
}
