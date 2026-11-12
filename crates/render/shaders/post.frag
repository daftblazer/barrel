#version 450
// The scene as the game finishes it (PostEffectGlow's last program): it was drawn at
// HdrRangeInv (0.5) into an 8-bit target and is doubled here; the glow is added later.
layout(set = 0, binding = 0) uniform sampler2D scene;
layout(location = 0) in vec2 uv;
layout(location = 0) out vec4 color;
void main() {
    color = vec4(texture(scene, uv).rgb * 2.0, 1.0);
}
