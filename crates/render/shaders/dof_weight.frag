#version 450
// The depth of field's weight (pass 29 of the game's chain, docs/shading.md, "The depth of field
// (type 0)"): the game writes a 512 x 1 curve looked up by the raw depth (linear filter, v 0.5)
// into the scene's alpha; here it is its own target, from the occlusion's linear depth.
layout(set = 0, binding = 0) uniform sampler2D gbuf;
layout(location = 0) out vec4 weight;

// The raw depth's planes (the capture's camera): 0 at the near plane, 1 at the far.
const float NEAR = 50.0;
const float FAR = 300000.0;
// The curve's texels, in 255ths. STAND-IN: the game's CPU writes the curve out of sight; these
// values are read off the capture (docs/ryza/open.md). Texels 0 to 495 are the base, 496 to 511
// the tail.
const float CURVE_BASE = 127.0;
const float CURVE_TAIL[16] = float[16](129.0, 132.0, 134.0, 136.0, 139.0, 141.0, 143.0, 145.0, 147.0, 148.0, 150.0, 152.0, 154.0, 155.0, 157.0, 158.0);
const int CURVE_TAIL_START = 496;
const int CURVE_SIZE = 512;

float texel(int k) {
    return (k < CURVE_TAIL_START ? CURVE_BASE : CURVE_TAIL[k - CURVE_TAIL_START]) / 255.0;
}

void main() {
    // The occlusion's target is half size; its depth is along the view, 1e9 where nothing is.
    float z = texelFetch(gbuf, ivec2(gl_FragCoord.xy) / 2, 0).a;
    float raw = clamp((1.0 / z - 1.0 / NEAR) / (1.0 / FAR - 1.0 / NEAR), 0.0, 1.0);
    // The linear filter at u = raw, the texels' centres at (k + 1/2) / 512.
    float t = clamp(raw * float(CURVE_SIZE) - 0.5, 0.0, float(CURVE_SIZE - 1));
    int k = int(floor(t));
    float w = mix(texel(k), texel(min(k + 1, CURVE_SIZE - 1)), t - float(k));
    weight = vec4(w);
}
