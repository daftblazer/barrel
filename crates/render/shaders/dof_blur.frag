#version 450
// The depth of field's blur (pass 31, docs/shading.md, "The depth of field (type 0)"): 16 taps
// of the two layers (the program's eight pairs), each weighted by how far its pixel is from the
// focus, the layers' alphas rewritten as a coverage.
layout(set = 0, binding = 0) uniform sampler2D layer0;
layout(set = 0, binding = 1) uniform sampler2D layer1;
layout(push_constant) uniform Push {
    // The layers' size inverse.
    vec2 inverse_size;
} pc;
layout(location = 0) in vec2 uv;
layout(location = 0) out vec4 out0;
layout(location = 1) out vec4 out1;

// The taps' offsets in texels (the program's constants: two thirds, 2.4 and 1.3).
const vec2 OFFSETS[16] = vec2[16](
    vec2(0.6666667, 0.6666667), vec2(-0.6666667, 0.6666667),
    vec2(0.6666667, -0.6666667), vec2(-0.6666667, -0.6666667),
    vec2(2.4, 0.0), vec2(0.0, 2.4),
    vec2(-2.4, 0.0), vec2(0.0, -2.4),
    vec2(1.3, 2.4), vec2(2.4, 1.3),
    vec2(-1.3, 2.4), vec2(-2.4, 1.3),
    vec2(1.3, -2.4), vec2(2.4, -1.3),
    vec2(-1.3, -2.4), vec2(-2.4, -1.3));
// A row for each four taps (the program's immediate table): the first term subtracted from the
// distance, the tap's weight, the term added to the distance, the distance's scale.
const vec4 TABLE[4] = vec4[4](
    vec4(3.7712362, 3.0, 4.0, 0.25),
    vec4(10.0, 2.0, 4.5, 0.22222222),
    vec4(10.917875, 2.5, 5.5, 0.18181819),
    vec4(10.917875, 2.5, 5.5, 0.18181819));
const float EDGE = 0.49803922;  // 127 / 255: the base of the weight's curve
const float EPS = 0.00001;

float cover(float alpha) {
    return max(1.0 - 2.0 * abs(alpha - 0.5), 0.0) + EPS;
}

void main() {
    vec4 sum0 = vec4(0.0), sum1 = vec4(0.0);
    vec2 total = vec2(EPS);
    bool back = false;
    for (int i = 0; i < 16; i++) {
        vec4 row = TABLE[i / 4];
        vec2 at = uv + OFFSETS[i] * pc.inverse_size;
        vec4 a = texture(layer0, at);
        vec4 b = texture(layer1, at);
        back = back || a.a < EDGE;
        float ca = cover(a.a), cb = cover(b.a);
        float w0 = clamp((1.0 / min(cb, ca) - 1.0 + row.z - row.x) * row.w, 0.0, 1.0);
        float w1 = clamp((1.0 / cb - 1.0 + row.z - row.x) * row.w, 0.0, 1.0);
        a.a = 1.0 / ca;
        b.a = 1.0 / cb;
        sum0 += a * w0 * row.y;
        sum1 += b * w1 * row.y;
        total += vec2(w0, w1) * row.y;
    }
    sum0 /= total.x;
    sum1 /= total.y;
    out0 = vec4(sum0.rgb, back ? min(1.0 / sum0.a, 1.0) * 0.5 : 0.5);
    out1 = vec4(sum1.rgb, (1.0 - min(1.0 / sum1.a, 1.0)) * 0.5 + 0.5);
}
