#version 450
// The sun's visibility (Ryza's sun flare, docs/renderer.md, "The sun flare"): a one-pixel value
// the sun's shadow nudges every frame, up while the reference point is in shadow and down while
// it is not, kept in [0, 1]. Mode 0 draws the next value from the one before it; mode 1 copies
// a value on, so the pair of one-pixel targets can swap without swapping their sets.
layout(set = 0, binding = 0) uniform sampler2DArrayShadow shadow_map;
layout(set = 0, binding = 1) uniform sampler2D before;
// reference: the point in the first cascade's map (uv, then depth). rates: the step up and the
// step down per frame, whether this is the first frame (the value before it is 0), the mode.
layout(push_constant) uniform Push { vec4 reference; vec4 rates; } pc;
layout(location = 0) out float result;
void main() {
    float last = pc.rates.z > 0.5 ? 0.0 : texelFetch(before, ivec2(0), 0).r;
    if (pc.rates.w > 0.5) {
        result = last;
        return;
    }
    // The comparison sampler answers 1 where the point is lit.
    bool shadowed = texture(shadow_map, vec4(pc.reference.xy, 0.0, pc.reference.z)) < 0.5;
    result = clamp(last + (shadowed ? pc.rates.x : -pc.rates.y), 0.0, 1.0);
}
