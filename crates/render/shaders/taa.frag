#version 450
// Temporal anti-aliasing (`taa.rs`): this frame's jittered scene blended into the history of
// the frames before it, the history reprojected by the motion vectors and clamped to this
// pixel's 3 × 3 neighbourhood (in YCoCg, where the clamp sits closest around the colours) so
// what is no longer there does not linger.
layout(set = 0, binding = 0) uniform sampler2D current;
layout(set = 0, binding = 1) uniform sampler2D history;
layout(set = 0, binding = 2) uniform sampler2D motion;
// texel: one scene pixel in texture coordinates; reset 1: no history to use (the first frame,
// a cut); blend: how much of this frame goes in.
layout(push_constant) uniform Push { vec2 texel; float reset; float blend; } pc;
layout(location = 0) in vec2 uv;
layout(location = 0) out vec4 color;

vec3 to_ycocg(vec3 c) {
    return vec3(0.25 * c.r + 0.5 * c.g + 0.25 * c.b, 0.5 * c.r - 0.5 * c.b, -0.25 * c.r + 0.5 * c.g - 0.25 * c.b);
}
vec3 from_ycocg(vec3 c) {
    return vec3(c.x + c.y - c.z, c.x + c.z, c.x - c.y - c.z);
}

void main() {
    vec3 here = texture(current, uv).rgb;
    if (pc.reset > 0.5) {
        color = vec4(here, 1.0);
        return;
    }
    vec3 lo = vec3(1e9), hi = vec3(-1e9);
    for (int y = -1; y <= 1; y++) {
        for (int x = -1; x <= 1; x++) {
            vec3 c = to_ycocg(texture(current, uv + vec2(x, y) * pc.texel).rgb);
            lo = min(lo, c);
            hi = max(hi, c);
        }
    }
    vec2 back = uv - texture(motion, uv).rg;
    if (any(lessThan(back, vec2(0.0))) || any(greaterThan(back, vec2(1.0)))) {
        color = vec4(here, 1.0);
        return;
    }
    vec3 before = from_ycocg(clamp(to_ycocg(texture(history, back).rgb), lo, hi));
    color = vec4(mix(before, here, pc.blend), 1.0);
}
