#version 450
// The exposure's metering (docs/shading.md, "The exposure"), as the game's chain does it: the
// log of the scene's luminance averaged down through 64, 16 and 4 pixels to one, the average
// undone, then eased toward and turned into an exposure. Every target holds four floats.
layout(set = 0, binding = 0) uniform sampler2D src;
layout(set = 0, binding = 1) uniform sampler2D prior;
// a: mode, fresh (no earlier exposure), the adaptation's rate toward a darker and toward a
// brighter scene. b: the middle gray, the exposure's range, the exposure the scene was lit with.
// c: the scene's gain to its true brightness (it is stored at half).
layout(push_constant) uniform Push { vec4 a; vec4 b; vec4 c; } pc;
layout(location = 0) in vec2 uv;
layout(location = 0) out vec4 color;
const vec3 LUMA = vec3(0.29891, 0.58661, 0.11448);
// How many taps across and down one 64 x 64 texel's part of the scene.
const int TAPS = 8;
void main() {
    int mode = int(pc.a.x);
    ivec2 at = ivec2(gl_FragCoord.xy);
    if (mode == 0) {
        // The mean of log2(luminance + 1) over the texel's part of the scene.
        float sum = 0.0;
        for (int j = 0; j < TAPS; j++) {
            for (int i = 0; i < TAPS; i++) {
                vec2 p = (vec2(at) + (vec2(i, j) + 0.5) / float(TAPS)) / 64.0;
                sum += log2(dot(textureLod(src, p, 0.0).rgb * pc.c.x, LUMA) + 1.0);
            }
        }
        color = vec4(sum / float(TAPS * TAPS), 0.0, 0.0, 0.0);
    } else if (mode == 1 || mode == 2) {
        // The mean of four by four texels; mode 2 (the last) takes the average back out of the log.
        float sum = 0.0;
        for (int j = 0; j < 4; j++) {
            for (int i = 0; i < 4; i++) {
                sum += texelFetch(src, at * 4 + ivec2(i, j), 0).r;
            }
        }
        sum /= 16.0;
        if (mode == 2) {
            sum = exp2(sum) - 1.0;
            if (isnan(sum) || isinf(sum)) {
                sum = 0.0;
            }
        }
        color = vec4(sum, 0.0, 0.0, 0.0);
    } else if (mode == 3) {
        // The scene's luminance without the exposure it was lit with; the one it is eased from
        // (the middle gray over the last exposure), or itself at the start.
        float key = pc.b.x;
        float scene = texelFetch(src, ivec2(0), 0).r / pc.b.w;
        float last = pc.a.y > 0.5 ? scene : key / (texelFetch(prior, ivec2(0), 0).r + 0.00001);
        float rate = scene < last ? pc.a.z : pc.a.w;
        float e = clamp(key / (last + (scene - last) * rate + 0.00001), pc.b.y, pc.b.z);
        color = vec4(e, e / pc.b.w, scene, 0.0);
    } else {
        color = texelFetch(src, ivec2(0), 0);
    }
}
