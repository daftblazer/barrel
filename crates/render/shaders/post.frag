#version 450
// The scene as the game finishes it (PostEffectGlow's last program): it was drawn at
// HdrRangeInv (0.5) into an 8-bit target and is doubled here, plus the glow at the field's
// rate; during a cross-fade (u13 kind 3) the frame frozen at its start lies over it, fading.
layout(set = 0, binding = 0) uniform sampler2D scene;
layout(set = 0, binding = 1) uniform sampler2D glow;
layout(set = 0, binding = 2) uniform sampler2D frozen;
layout(push_constant) uniform Push { float rate; float frozen_weight; } pc;
layout(location = 0) in vec2 uv;
layout(location = 0) out vec4 color;
void main() {
    vec3 live = texture(scene, uv).rgb * 2.0 + texture(glow, uv).rgb * pc.rate;
    vec3 before = texture(frozen, uv).rgb * 2.0;
    color = vec4(mix(live, before, pc.frozen_weight), 1.0);
}
