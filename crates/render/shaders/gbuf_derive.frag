#version 450
// The half-size normal and view depth target (`ao::GBUF_FORMAT`: the world normal in rgb, the
// depth along the view in a, `CLEAR_DEPTH` where nothing is drawn) built from the scene pass's own
// outputs: its depth buffer and its octahedral world normal. The reflections' march, the depth of
// field and the water's refraction read it where the occlusion's half-size geometry pre-pass does
// not run (it runs only with the occlusion, `ao.rs`).
#include "normal_octahedral.glsl"
layout(set = 0, binding = 0) uniform sampler2D scene_depth;
layout(set = 0, binding = 1) uniform sampler2D scene_normal;
// x, y: the view depth as 1 / (x + y * d) for the device depth d; z: the depth the target holds
// where nothing is drawn.
layout(push_constant) uniform Params { vec4 depth; } pc;
layout(location = 0) out vec4 gbuf;

void main() {
    // The scene's texel at the half-size texel's top-left: the same one for both inputs.
    ivec2 px = min(ivec2(gl_FragCoord.xy) * 2, textureSize(scene_depth, 0) - 1);
    float d = texelFetch(scene_depth, px, 0).r;
    if (d >= 1.0) {
        gbuf = vec4(0.0, 0.0, 1.0, pc.depth.z);
        return;
    }
    gbuf = vec4(octahedral_decode(texelFetch(scene_normal, px, 0).rg), 1.0 / (pc.depth.x + pc.depth.y * d));
}
