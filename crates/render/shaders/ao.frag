#version 450
// The screen-space ambient occlusion, as the game's pass computes it (docs/shading.md, "The
// ambient occlusion"; its pixel program, read from the capture): at half the scene's size,
// eight directions round each pixel's surface, each marched in rounded pixel steps along the
// surface's tangent plane; the horizon's rise inside a radius, weighted by the distance,
// is summed and scaled. The depth and normals come from ao_gbuffer*.frag.
layout(set = 0, binding = 0) uniform sampler2D gbuf;
layout(push_constant) uniform Push {
    // The world to view rows (the game's axes: x right, y up, z toward the viewer), without the
    // translation.
    vec4 rot_x;
    vec4 rot_y;
    vec4 rot_z;
    // vViewParam: view position = (uv * xy + zw) * depth.
    vec4 view;
    // vResolutionParam: the target's size and its inverse.
    vec4 res;
} pc;
layout(location = 0) out vec4 occlusion;

// The constants of the captured frame (Rasenboden at noon, the game's 960 by 540 target).
const float DEPTH_FAR = 299910.0;                              // vDepthParam.z: nothing past it is shaded
const float MAX_PIXELS = 54.0;                                 // vRadiusParam.w: the march's longest reach
const vec4 RADIUS = vec4(50.0, 200.0, -0.00066666666, -0.33333334); // vRadiusParam2: the radius, near and far, and its slope with depth
const vec4 RAY = vec4(8.0, 6.0, 0.125, 0.17632698);            // vRayParam: directions, steps, turn a direction, bias (tan 10 degrees)
const vec2 OCC = vec2(2.5, 0.8);                               // vOccParam: scale and ceiling of the occlusion

ivec2 last;

// The depth (alpha) at a pixel; the edge repeats.
float depth_at(ivec2 p) {
    return texelFetch(gbuf, clamp(p, ivec2(0), last), 0).a;
}

vec3 view_at(ivec2 p, float d) {
    return vec3((vec2(p) + 0.5) * pc.res.zw * pc.view.xy + pc.view.zw, -1.0) * d;
}

// A neighbour's position, slid along its ray onto the centre's tangent plane when it is in front
// of the plane's far side.
vec3 neighbour(ivec2 p, vec3 n, float n_dot_p) {
    vec3 q = view_at(p, depth_at(p));
    float nq = dot(n, q);
    return nq < 0.0 ? q * (n_dot_p / nq) : q;
}

// STAND-IN: the game's 4 by 4 noise texture (`sNoise`, tiled over the half-size target: x
// jitters the directions' start angle, y the march's start along its first step) is filled by
// the game at run time and is not in the capture. x is a Bayer matrix; y is 0, which brings the
// regions' mean occlusion nearer the game's (mid tree 0.479 against its 0.457, where a Bayer y
// gives 0.524): a start off the pixel shortens every march. With a temporal resolve after it the
// pattern moves by a whole texel a frame through its 16 places (the rows' w: Barrel's choice, so
// the resolve averages the pattern as the game's own picture shows none), else it stays put.
vec2 noise(ivec2 p) {
    p += ivec2(pc.rot_x.w, pc.rot_y.w);
    const int bayer[16] = int[16](0, 8, 2, 10, 12, 4, 14, 6, 3, 11, 1, 9, 15, 7, 13, 5);
    return vec2(bayer[(p.y & 3) * 4 + (p.x & 3)] / 16.0 + 1.0 / 32.0, 0.0);
}

void main() {
    last = textureSize(gbuf, 0) - 1;
    ivec2 px = ivec2(gl_FragCoord.xy);
    vec4 g = texelFetch(gbuf, px, 0);
    float d = g.a;
    occlusion = vec4(1.0);
    if (d >= DEPTH_FAR) return;
    // The radius in the world (50 within 500 of the camera, growing to 200 by 2000), in pixels,
    // and how many steps of it (at most six, and no more than 54 pixels in all).
    float radius = clamp(-d * RADIUS.z + RADIUS.w, 0.0, 1.0) * (RADIUS.y - RADIUS.x) + RADIUS.x;
    float reach = radius * pc.res.x / pc.view.x / d;
    float steps = min(reach, RAY.y);
    if (steps < 1.0) return;
    vec2 nz = noise(px);
    float step_len = reach / (steps + 1.0);
    if (MAX_PIXELS / step_len < steps) {
        steps = max(floor(nz.y + MAX_PIXELS / step_len), 1.0);
        step_len = MAX_PIXELS / steps;
    }
    vec3 p = view_at(px, d);
    float r2 = radius * radius;
    vec3 wn = g.xyz;
    vec3 n = normalize(vec3(dot(pc.rot_x.xyz, wn), dot(pc.rot_y.xyz, wn), dot(pc.rot_z.xyz, wn)));
    float n_dot_p = dot(n, p);
    // The tangent plane's axes along the screen's, each the shorter of the two sides.
    vec3 left = neighbour(px + ivec2(-1, 0), n, n_dot_p), right = neighbour(px + ivec2(1, 0), n, n_dot_p);
    vec3 up = neighbour(px + ivec2(0, -1), n, n_dot_p), down = neighbour(px + ivec2(0, 1), n, n_dot_p);
    vec3 tx = right - p, tl = p - left;
    if (dot(tx, tx) >= dot(tl, tl)) tx = tl;
    vec3 ty = down - p, tu = p - up;
    if (dot(ty, ty) >= dot(tu, tu)) ty = tu;
    ty *= pc.res.z * pc.res.y;
    float turn = 6.283185 * RAY.z;
    float sum = 0.0, count = 0.0;
    for (int i = 0; i < int(RAY.x); i++) {
        float angle = turn * float(i) + nz.x * turn;
        vec2 dir = vec2(cos(angle), sin(angle));
        // The step and the start in whole pixels.
        vec2 stride = roundEven(vec2(step_len) * dir);
        ivec2 q = px + ivec2(roundEven(nz.y * step_len * dir));
        vec2 step_uv = stride * pc.res.zw;
        vec3 along = tx * step_uv.x + ty * step_uv.y;
        float tangent = along.z / length(along.xy) + RAY.w;
        float horizon = tangent, horizon_sin = tangent * inversesqrt(tangent * tangent + 1.0);
        float rise = 0.0, seen = 0.0;
        for (float s = 0.0; s < steps; s += 1.0) {
            q += ivec2(stride);
            vec3 delta = view_at(q, depth_at(q)) - p;
            float dist2 = dot(delta, delta);
            float slope = delta.z / max(length(delta.xy), 1e-6);
            bool inside = dist2 < r2;
            if (inside || horizon >= slope) seen += 1.0;
            if (inside && horizon < slope) {
                float slope_sin = slope * inversesqrt(slope * slope + 1.0);
                rise += (slope_sin - horizon_sin) * (dist2 * (-1.0 / r2) + 1.0);
                horizon = slope;
                horizon_sin = slope_sin;
            }
        }
        count += clamp(seen, 0.0, 1.0);
        sum += clamp(rise, 0.0, 1.0);
    }
    float amount = count != 0.0 ? min(sum / count * OCC.x, OCC.y) : 0.0;
    occlusion = vec4(vec3(clamp(1.0 - amount, 0.0, 1.0)), 1.0);
}
