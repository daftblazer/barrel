#version 450
// The game's glow (PostEffectGlow.kps): mode 0 the bright pass, saturate(scene - threshold);
// mode 1 a 7-tap blur along `step` (one texel of the target written), weights 0.0455, 0.0909,
// 0.1818, 0.3636, 0.1818, 0.0909, 0.0455.
layout(set = 0, binding = 0) uniform sampler2D src;
layout(push_constant) uniform Push { vec2 step; float threshold; uint mode; } pc;
layout(location = 0) in vec2 uv;
layout(location = 0) out vec4 color;
void main() {
    if (pc.mode == 0u) {
        color = vec4(clamp(texture(src, uv).rgb - pc.threshold, 0.0, 1.0), 1.0);
        return;
    }
    vec4 c = texture(src, uv) * 0.36363637;
    c += (texture(src, uv - pc.step) + texture(src, uv + pc.step)) * 0.18181819;
    c += (texture(src, uv - 2.0 * pc.step) + texture(src, uv + 2.0 * pc.step)) * 0.09090909;
    c += (texture(src, uv - 3.0 * pc.step) + texture(src, uv + 3.0 * pc.step)) * 0.045454547;
    color = c;
}
