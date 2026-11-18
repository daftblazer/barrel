#version 450
// The scene as the game finishes it (PostEffectGlow's last program): it was drawn at
// HdrRangeInv (0.5) into an 8-bit target and is doubled here, plus the glow at the field's
// rate; during a cross-fade (u13 kind 3) the frame frozen at its start lies over it, fading.
layout(set = 0, binding = 0) uniform sampler2D scene;
layout(set = 0, binding = 1) uniform sampler2D glow;
layout(set = 0, binding = 2) uniform sampler2D frozen;
// The screen filter (Escha's PostEffectTexFilter): its colour overlay-blended over the frame,
// by its alpha times screen.x (overlayModifier; 0: none).
layout(set = 0, binding = 3) uniform sampler2D screen_filter;
// The metered exposure (render::meter) when screen.y is 1: its second channel is the ratio of
// the exposure to the one the scene was lit with.
layout(set = 0, binding = 4) uniform sampler2D exposure;
// The fade's colour (u22), laid over by its alpha. saturation: a scale per channel about the
// BT.601 luma (0: none), and in w a tone curve's white point (0: none): extended Reinhard,
// c (1 + c / w²) / (1 + c), or with a gamma in pad.y the Ryza games' filmic curve over the
// white point's, then that gamma.
layout(push_constant) uniform Push { float rate; float frozen_weight; vec2 pad; vec4 overlay; vec4 saturation; vec4 screen; vec4 grade; } pc;
layout(location = 0) in vec2 uv;
layout(location = 0) out vec4 color;
// The Ryza games' tone curve (the inverse of what `effect2dpreinvtonemapandgamma.g1s` undoes,
// docs/shading.md): Hable's filmic curve, shoulder 0.22, linear 0.30 at angle 0.10, toe 0.20
// with numerator 0.01 and denominator 0.30.
vec3 filmic(vec3 x) {
    return (x * (0.22 * x + 0.03) + 0.002) / (x * (0.22 * x + 0.3) + 0.06) - 0.0333;
}
void main() {
    // pad.x 1: the motion vectors shown instead (`--show-motion`: binding 0 holds them),
    // their size times 40 in red and green.
    if (pc.pad.x > 0.5) {
        color = vec4(abs(texture(scene, uv).rg) * 40.0, 0.0, 1.0);
        return;
    }
    float metered = pc.screen.y > 0.5 ? texelFetch(exposure, ivec2(0), 0).g : 1.0;
    vec3 live = texture(scene, uv).rgb * 2.0 * metered + texture(glow, uv).rgb * pc.rate;
    vec3 before = texture(frozen, uv).rgb * 2.0 * metered;
    vec3 c = mix(live, before, pc.frozen_weight);
    if (pc.screen.x > 0.0) {
        vec4 f = texture(screen_filter, uv);
        vec3 over = mix(2.0 * c * f.rgb, 1.0 - 2.0 * (1.0 - c) * (1.0 - f.rgb), step(0.5, c));
        c += (over - c) * f.a * pc.screen.x;
    }
    if (pc.pad.y > 0.0) {
        // The Ryza games' last pass: the colour scale (grade.rgb, `vColorScale`), the curve
        // over the white point's, the gamma, then `fSaturationScaleEx` (grade.a): a
        // saturation about the luma, 1 at white and grade.a at black.
        if (pc.grade.x > 0.0) {
            c *= pc.grade.rgb;
        }
        c = pow(max(filmic(c) / filmic(vec3(pc.saturation.w)), 0.0), vec3(1.0 / pc.pad.y));
        if (pc.grade.a > 1.0) {
            float mid = 0.5 * (min(c.r, min(c.g, c.b)) + max(c.r, max(c.g, c.b)));
            float y = dot(c, vec3(0.29891, 0.58661, 0.11448));
            c = y + (c - y) * (1.0 + (1.0 - mid) * (pc.grade.a - 1.0));
        }
    } else if (pc.saturation.w > 0.0) {
        float w2 = pc.saturation.w * pc.saturation.w;
        c = c * (1.0 + c / w2) / (1.0 + c);
    }
    if (pc.saturation.x > 0.0) {
        float y = dot(c, vec3(0.299, 0.587, 0.114));
        c = max(mix(vec3(y), c, pc.saturation.xyz), 0.0);
    }
    color = vec4(mix(c, pc.overlay.rgb, clamp(pc.overlay.a, 0.0, 1.0)), 1.0);
}
