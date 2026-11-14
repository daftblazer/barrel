#version 450
// The scene as the game finishes it (PostEffectGlow's last program): it was drawn at
// HdrRangeInv (0.5) into an 8-bit target and is doubled here, plus the glow at the field's
// rate; during a cross-fade (u13 kind 3) the frame frozen at its start lies over it, fading.
layout(set = 0, binding = 0) uniform sampler2D scene;
layout(set = 0, binding = 1) uniform sampler2D glow;
layout(set = 0, binding = 2) uniform sampler2D frozen;
// The fade's colour (u22), laid over by its alpha. pad.y: a saturation scale about the
// BT.601 luma (0: none; Ryza's `trueHDR_vSaturationScale`).
layout(push_constant) uniform Push { float rate; float frozen_weight; vec2 pad; vec4 overlay; } pc;
layout(location = 0) in vec2 uv;
layout(location = 0) out vec4 color;
void main() {
    // pad.x 1: the motion vectors shown instead (`--show-motion`: binding 0 holds them),
    // their size times 40 in red and green.
    if (pc.pad.x > 0.5) {
        color = vec4(abs(texture(scene, uv).rg) * 40.0, 0.0, 1.0);
        return;
    }
    vec3 live = texture(scene, uv).rgb * 2.0 + texture(glow, uv).rgb * pc.rate;
    vec3 before = texture(frozen, uv).rgb * 2.0;
    vec3 c = mix(live, before, pc.frozen_weight);
    if (pc.pad.y > 0.0) {
        float y = dot(c, vec3(0.299, 0.587, 0.114));
        c = max(mix(vec3(y), c, pc.pad.y), 0.0);
    }
    color = vec4(mix(c, pc.overlay.rgb, clamp(pc.overlay.a, 0.0, 1.0)), 1.0);
}
