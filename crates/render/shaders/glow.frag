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
    // The Ryza games' bloom (docs/shading.md, "the frame as the game draws it"). Mode 2 its
    // bright pass on the scene (stored at half, as its result is): c × saturate((luma − T) /
    // (white − T)), T and the white point in `step`, the scene taken down by four taps.
    if (pc.mode == 2u) {
        vec2 t = 1.0 / vec2(textureSize(src, 0));
        vec3 s = (texture(src, uv + t * vec2(-1.0, -1.0)).rgb + texture(src, uv + t * vec2(1.0, -1.0)).rgb
            + texture(src, uv + t * vec2(-1.0, 1.0)).rgb + texture(src, uv + t * vec2(1.0, 1.0)).rgb) * 0.5;
        float luma = dot(s, vec3(0.29891, 0.58661, 0.11448));
        color = vec4(s * clamp((luma - pc.step.x) / (pc.step.y - pc.step.x), 0.0, 1.0) * 0.5, 1.0);
        return;
    }
    // Mode 3: its 13-tap diamond, `step` one texel.
    if (pc.mode == 3u) {
        vec4 d = texture(src, uv) * 0.18385795;
        d += (texture(src, uv + pc.step * vec2(1.0, 0.0)) + texture(src, uv + pc.step * vec2(-1.0, 0.0))
            + texture(src, uv + pc.step * vec2(0.0, 1.0)) + texture(src, uv + pc.step * vec2(0.0, -1.0))) * 0.11151548;
        d += (texture(src, uv + pc.step * vec2(1.0, 1.0)) + texture(src, uv + pc.step * vec2(-1.0, 1.0))
            + texture(src, uv + pc.step * vec2(1.0, -1.0)) + texture(src, uv + pc.step * vec2(-1.0, -1.0))) * 0.067637555;
        d += (texture(src, uv + pc.step * vec2(2.0, 0.0)) + texture(src, uv + pc.step * vec2(-2.0, 0.0))
            + texture(src, uv + pc.step * vec2(0.0, 2.0)) + texture(src, uv + pc.step * vec2(0.0, -2.0))) * 0.024882466;
        color = d;
        return;
    }
    // Mode 4: its Gaussian along `step` (one texel), seven taps each between two texels
    // (1.40733, 3.29421 and 5.20181 texels out).
    if (pc.mode == 4u) {
        vec4 g = texture(src, uv) * 0.19967563;
        g += (texture(src, uv + pc.step * 1.40733) + texture(src, uv - pc.step * 1.40733)) * 0.2973225;
        g += (texture(src, uv + pc.step * 3.29421) + texture(src, uv - pc.step * 3.29421)) * 0.091848336;
        g += (texture(src, uv + pc.step * 5.20181) + texture(src, uv - pc.step * 5.20181)) * 0.010991331;
        color = g;
        return;
    }
    // Mode 5: a copy (the quarter taken down to the eighth).
    if (pc.mode == 5u) {
        color = texture(src, uv);
        return;
    }
    vec4 c = texture(src, uv) * 0.36363637;
    c += (texture(src, uv - pc.step) + texture(src, uv + pc.step)) * 0.18181819;
    c += (texture(src, uv - 2.0 * pc.step) + texture(src, uv + 2.0 * pc.step)) * 0.09090909;
    c += (texture(src, uv - 3.0 * pc.step) + texture(src, uv + 3.0 * pc.step)) * 0.045454547;
    color = c;
}
