#version 450
// The sky clouds' density map (docs/shading.md, "Ryza DX: the sky, the clouds"): tileable
// four-dimensional Perlin noise. The pixel's place on the map is two circles (its two axes
// each turned into an angle), so the map joins at its edges; each octave doubles the
// frequency and scales the amplitude by the persistence. Gradients are picked from a
// permutation table by the low bits of the lattice hash, the corner's offset taken in the
// order x, y, z, w.
layout(set = 0, binding = 0) uniform sampler2D perm;
layout(push_constant) uniform Push {
    // The two sampling ranges (round each circle), the persistence and the scale.
    vec4 shape;
    vec4 offset;
    uint octaves;
} p;
layout(location = 0) in vec2 uv;
layout(location = 0) out vec4 colour;

uint table(int i) {
    return uint(texelFetch(perm, ivec2(i & 255, 0), 0).r * 255.0 + 0.5);
}

// A corner's gradient dotted with its offset from the point.
float grad(uint h, vec4 f) {
    h &= 31u;
    float u = h < 24u ? f.x : f.y;
    float v = h < 12u ? f.y : f.z;
    float w = h < 8u ? f.z : f.w;
    return ((h & 1u) != 0u ? -u : u) + ((h & 2u) != 0u ? -v : v) + ((h & 4u) != 0u ? -w : w);
}

float noise(vec4 x) {
    vec4 fl = floor(x);
    ivec4 i0 = ivec4(fl);
    vec4 f0 = x - fl;
    vec4 f1 = f0 - 1.0;
    // The quintic fade.
    vec4 w = f0 * f0 * f0 * (f0 * (f0 * 6.0 - 15.0) + 10.0);
    // The hash of each corner, built a coordinate at a time (bit 0 of a corner is its x).
    uint hx[2];
    uint hy[4];
    uint hz[8];
    uint hw[16];
    for (int a = 0; a < 2; a++) {
        hx[a] = table(i0.x + a);
    }
    for (int a = 0; a < 4; a++) {
        hy[a] = table(i0.y + (a >> 1) + int(hx[a & 1]));
    }
    for (int a = 0; a < 8; a++) {
        hz[a] = table(i0.z + (a >> 2) + int(hy[a & 3]));
    }
    float n[16];
    for (int a = 0; a < 16; a++) {
        hw[a] = table(i0.w + (a >> 3) + int(hz[a & 7]));
        vec4 f = vec4((a & 1) != 0 ? f1.x : f0.x, (a & 2) != 0 ? f1.y : f0.y, (a & 4) != 0 ? f1.z : f0.z, (a & 8) != 0 ? f1.w : f0.w);
        n[a] = grad(hw[a], f);
    }
    for (int a = 0; a < 8; a++) {
        n[a] = mix(n[2 * a], n[2 * a + 1], w.x);
    }
    for (int a = 0; a < 4; a++) {
        n[a] = mix(n[2 * a], n[2 * a + 1], w.y);
    }
    for (int a = 0; a < 2; a++) {
        n[a] = mix(n[2 * a], n[2 * a + 1], w.z);
    }
    return mix(n[0], n[1], w.w);
}

void main() {
    vec2 t = uv * 6.2831853;
    vec4 q = vec4(sin(t.x), cos(t.x), sin(t.y), cos(t.y)) * vec4(p.shape.xx, p.shape.yy) * 0.15915494 + p.offset;
    float sum = 0.0;
    float amp = 1.0;
    for (uint o = 0u; o < p.octaves; o++) {
        sum += noise(q) * amp;
        q += q;
        amp *= p.shape.z;
    }
    colour = vec4(vec3(sum * p.shape.w * 0.5 + 0.5), 1.0);
}
