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
// Ryza's sun flare and paraffin wash (flare.rs): its constants, and the sun's visibility (a
// one-pixel value, 1 where the sun is blocked).
layout(set = 0, binding = 5) uniform Wash {
    vec4 rotation;       // the screen's tilt: row 0 then row 1 of the 2x2
    vec4 flare_color;    // vFlareColor (xyz, weight in w)
    vec4 paraffin_color; // vParaffinColor
    vec4 flare;          // fFlareAmount, fFlareCurveCenter, fFlareCurveCoefficient
    vec4 paraffin;       // fParaffinAmount, fParaffinCurveCenter, fParaffinCurveCoefficient
    vec4 extra;          // the two layers' shadow influence ratios, on (0: no wash), fIntensity
} wash;
layout(set = 0, binding = 6) uniform sampler2D sun_visibility;
// Ryza's depth of field (dof.rs; screen.z 1: on): the weight (red), then the blurred near-view
// and far-view layers at quarter size, in the scene's own units (the alpha of each a coverage).
layout(set = 0, binding = 7) uniform sampler2D dof_weight;
layout(set = 0, binding = 8) uniform sampler2D dof_front;
layout(set = 0, binding = 9) uniform sampler2D dof_back;
layout(push_constant) uniform Push { float rate; float frozen_weight; vec2 pad; vec4 overlay; vec4 saturation; vec4 screen; vec4 grade; } pc;
layout(location = 0) in vec2 uv;
layout(location = 0) out vec4 color;
// The Ryza games' tone curve (the inverse of what `effect2dpreinvtonemapandgamma.g1s` undoes,
// docs/shading.md): Hable's filmic curve, shoulder 0.22, linear 0.30 at angle 0.10, toe 0.20
// with numerator 0.01 and denominator 0.30.
vec3 filmic(vec3 x) {
    return (x * (0.22 * x + 0.03) + 0.002) / (x * (0.22 * x + 0.3) + 0.06) - 0.0333;
}
// How much of a blur mixes in for a coverage `w` (0.5: none; toward 0 and 1: more): the game's
// ramp, saturate((1 / max(1 - 2|w - 1/2|, 0) - 1) / 4).
float blur_ramp(float w) {
    return clamp((1.0 / (max(1.0 - 2.0 * abs(w - 0.5), 0.0) + 0.00001) - 1.0) * 0.25, 0.0, 1.0);
}
// The depth of field, type 0, on the scene `c` (already at its exposure `k`, the layers' to be
// brought to it): the far view's layer by the weight where that is 0.5 or more, then the near
// view's by its own alpha.
vec3 soften(vec3 c, float k) {
    float w = texture(dof_weight, uv).r;
    if (w >= 0.5) {
        c += blur_ramp(w) * (texture(dof_back, uv).rgb * k - c);
    }
    vec4 front = texture(dof_front, uv);
    return c + blur_ramp(front.a) * (front.rgb * k - c);
}
void main() {
    // pad.x 1: the motion vectors shown instead (`--show-motion`: binding 0 holds them),
    // their size times 40 in red and green.
    if (pc.pad.x > 0.5) {
        color = vec4(abs(texture(scene, uv).rg) * 40.0, 0.0, 1.0);
        return;
    }
    float metered = pc.screen.y > 0.5 ? texelFetch(exposure, ivec2(0), 0).g : 1.0;
    vec3 lit = texture(scene, uv).rgb * 2.0 * metered;
    if (pc.screen.z > 0.5) {
        lit = soften(lit, 2.0 * metered);
    }
    vec3 live = lit + texture(glow, uv).rgb * pc.rate;
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
    if (wash.extra.z > 0.5) {
        // The wash lies on the finished picture (after the tone curve), as the game's pass does
        // on its 8-bit result. p: the screen's uv tilted about its centre.
        vec2 q = uv - 0.5;
        vec2 p = q.x * wash.rotation.xy + q.y * wash.rotation.zw + 0.5;
        float vis = texelFetch(sun_visibility, ivec2(0), 0).r;
        float dp = wash.paraffin.y - p.x;
        float df = wash.flare.y - p.x;
        float lit = clamp(wash.paraffin.z * dp * dp + p.y - (1.0 - wash.paraffin.x), 0.0, 255.0);
        lit *= wash.extra.w * clamp(wash.extra.y, 0.0, 1.0) * vis * wash.paraffin_color.w;
        float glow = clamp(wash.flare.x - (wash.flare.z * df * df + p.y), 0.0, 255.0);
        glow *= wash.extra.w * (1.0 - vis * clamp(wash.extra.x, 0.0, 1.0));
        c = (c + wash.flare_color.w * wash.flare_color.xyz * glow) * (1.0 + lit * (wash.paraffin_color.xyz - 1.0));
    }
    color = vec4(mix(c, pc.overlay.rgb, clamp(pc.overlay.a, 0.0, 1.0)), 1.0);
}
