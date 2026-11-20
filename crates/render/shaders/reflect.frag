#version 450
// The screen-space reflections' march, as the game's pass computes it (docs/shading.md, "The
// screen-space reflections"; its pixel program, read from the capture): at half the scene's
// size, each pixel's view-space reflection ray is stepped in half-size pixels along the screen
// until it passes behind the depth the scene shows; the lit scene's colour where it does, and
// how far to trust it in alpha. The depth and normals come from ao_gbuffer (model.frag's
// `AO_PASS`): the world normal in rgb, the view depth in a.
layout(set = 0, binding = 0) uniform sampler2D gbuf;
layout(set = 0, binding = 1) uniform sampler2D scene;
// The view depth alone (gbuf's alpha, in red): the steps read it, 4 bytes where gbuf has 16.
layout(set = 0, binding = 2) uniform sampler2D march_depth;
layout(push_constant) uniform Push {
    // The world to view rows (the game's axes: x right, y up, z toward the viewer), without the
    // translation.
    vec4 rot_x;
    vec4 rot_y;
    vec4 rot_z;
    // vViewParam: view position = (uv * xy + zw) * depth.
    vec4 view;
    // The target's size and its inverse.
    vec4 res;
    // The quality level (rlrpreset.xml): MaxRaySamples, PixelsPerStep.
    vec4 level;
} pc;
layout(location = 0) out vec4 reflection;

// The constants of the captured frame at the game's High level (Rasenboden at noon, its 960 by 540 target);
// the lower levels scale the step and the steps' fade by pc.level (reflect.rs, `Rlr`).
const int MAX_STEPS = 256;                                      // nMaxRaySmpl
const vec4 REFL = vec4(0.0037037037, 1.7777778, 1.0, 0.5);     // vReflParam: a half-size pixel's step (2 / 540), the aspect, the direction's scale and bias
const vec2 DEPTH_PARAM = vec2(-0.02, 0.02);                     // vDepthParam.xy: linear depth = 1 / (d * x + y)
const vec3 DEPTH_FADE = vec3(100.0, 0.01, 0.0);                 // vDepthFadeParam: the thickest hit, its fade's scale
const vec4 EDGE_FADE = vec4(0.0078125, -1.0, 20.0, -9.0);       // vSampleEdgeFadeParam: the steps' fade (from step 128) and the screen edge's
const float SKY_DEPTH = 299910.0;                               // the occlusion's far limit: nothing past it is a surface

ivec2 last;

float lin(float d) {
    return 1.0 / (d * DEPTH_PARAM.x + DEPTH_PARAM.y);
}

// The depth buffer's value (0 near, 1 far) of a view depth.
float depth_of(float view_depth) {
    return (1.0 / view_depth - DEPTH_PARAM.y) / DEPTH_PARAM.x;
}

// The depth buffer value at a uv: the texel it falls in (a bilinear read agreed less with the
// capture); the sky is 1.
float depth_at(vec2 uv) {
    float d = texelFetch(march_depth, clamp(ivec2(floor(uv * pc.res.xy)), ivec2(0), last), 0).r;
    return d >= SKY_DEPTH ? 1.0 : depth_of(d);
}

void main() {
    last = textureSize(gbuf, 0) - 1;
    ivec2 px = ivec2(gl_FragCoord.xy);
    reflection = vec4(0.0);
    vec4 g = texelFetch(gbuf, px, 0);
    if (g.a >= SKY_DEPTH) return;
    vec2 uv0 = (vec2(px) + 0.5) * pc.res.zw;
    float d0 = depth_of(g.a);
    vec3 p = vec3(uv0 * pc.view.xy + pc.view.zw, -1.0) * g.a;
    vec3 n = normalize(vec3(dot(pc.rot_x.xyz, g.xyz), dot(pc.rot_y.xyz, g.xyz), dot(pc.rot_z.xyz, g.xyz)));
    vec3 r = p - 2.0 * dot(p, n) * n;
    // A ray toward the viewer shows nothing.
    float f_dir = r.z / length(r) * REFL.z + REFL.w;
    if (f_dir >= 1.0) return;
    // The ray's end on the screen: (ndc.xy, depth buffer value).
    vec3 q = p + r;
    float end_depth = -q.z;
    if (end_depth <= 0.0) return;
    vec2 end_uv = (q.xy / end_depth - pc.view.zw) / pc.view.xy;
    vec3 step_ndc = vec3(end_uv.x * 2.0 - 1.0, 1.0 - end_uv.y * 2.0, depth_of(end_depth)) - vec3(uv0.x * 2.0 - 1.0, 1.0 - uv0.y * 2.0, d0);
    // One half-size pixel along the longer screen axis, as a step in (uv, depth).
    vec3 s = step_ndc * (REFL.x * pc.level.y / max(abs(step_ndc.y), abs(step_ndc.x) * REFL.y)) * vec3(0.5, -0.5, 1.0);
    vec3 ray = vec3(uv0, d0) + 2.0 * s;
    float prev = d0;
    int steps = int(pc.level.x);
    float fade_rate = EDGE_FADE.x * float(MAX_STEPS) / pc.level.x;
    for (int i = 0; i < steps; i++) {
        if (max(abs(ray.x - 0.5), abs(ray.y - 0.5)) > 0.5 || ray.z > 1.0) return;
        float sd = depth_at(ray.xy);
        if (ray.z >= sd) {
            float thick = lin(clamp(ray.z, 0.0, 1.0)) - lin(sd);
            vec3 back = ray - s;
            vec2 hit = ray.xy;
            // Crossing the depth seen the step before: the hit is where, between the steps, with no thickness.
            if (back.z < prev && prev <= ray.z) {
                float a = lin(prev) - lin(clamp(back.z, 0.0, 1.0));
                hit = back.xy + (a / (thick + a)) * (ray.xy - back.xy);
                thick = 0.0;
            }
            if (thick < DEPTH_FADE.x) {
                float edge = max(abs(hit.x - 0.5), abs(hit.y - 0.5)) * EDGE_FADE.z + EDGE_FADE.w;
                float fade = max(max(clamp(thick * DEPTH_FADE.y, 0.0, 1.0), clamp(float(i) * fade_rate + EDGE_FADE.y, 0.0, 1.0)), max(clamp(edge, 0.0, 1.0), clamp(f_dir, 0.0, 1.0)));
                reflection = vec4(texture(scene, hit).rgb, 1.0 - fade);
                return;
            }
            ray.xy = hit;
        }
        prev = sd;
        ray += s;
    }
}
