#version 450
// One line of the blurred screen picture's blur (`screen_blur.rs`): seven taps along `step`, bilinear, with the
// near-flat weights the game's passes carry. The offsets are in units of the pass's tap spacing; the positive side is
// the game's (0, 1, 2, 3 spacings), the negative side the one the capture shows (docs/renderer.md, "The blurred screen").
layout(location = 0) in vec2 uv;
layout(location = 0) out vec4 color;
layout(set = 0, binding = 0) uniform sampler2D src;
layout(push_constant) uniform Pass {
    vec2 step; // one spacing, in uv
} pass;

const float WEIGHT[7] = float[7](0.1404, 0.1428, 0.1443, 0.1448, 0.1443, 0.1428, 0.1404);
const float OFFSET[7] = float[7](-1.5385, -0.7692, -0.7692, 0.0, 1.0, 2.0, 3.0);

void main() {
    vec3 sum = vec3(0.0);
    for (int i = 0; i < 7; i++) {
        sum += WEIGHT[i] * texture(src, uv + OFFSET[i] * pass.step).rgb;
    }
    color = vec4(sum, 1.0);
}
