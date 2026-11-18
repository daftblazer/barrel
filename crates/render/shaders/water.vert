#version 450
// The Ryza games' water (docs/shading.md, "Ryza DX: the water"): ribbons along cubic centre
// lines, drawn from no vertex buffer. A piece is a grid of CELLS_T by CELLS_L cells; its curve,
// widths and look come from the look block's water slots (WATER_SLOT on): [0] {pieces, seconds},
// then five slots a piece: the curve's a, b, c, d (xyz; a.w and b.w the widths at its ends; c.w and
// d.w the body's refraction, water.frag's) and {albedo rgb, the reflectance at normal incidence}. Render space has x mirrored.
layout(std140, set = 1, binding = 4) uniform Look {
    vec4 head[4];
    vec4 eye;
    vec4 middle[39];
    mat4 view_proj;
    vec4 tail[28];
    vec4 sky[11];
    vec4 other[10];
    vec4 cloud[12];
    vec4 specular[2];
    vec4 water[81];
} look;
layout(location = 0) out vec3 world;
layout(location = 1) flat out int piece;
layout(location = 2) out float fog_depth;

const int CELLS_T = 24;
const int CELLS_L = 2;
const int CELL_VERTICES = 6;
const ivec2 CORNER[6] = ivec2[](ivec2(0, 0), ivec2(1, 0), ivec2(0, 1), ivec2(1, 0), ivec2(1, 1), ivec2(0, 1));

void main() {
    int per_piece = CELLS_T * CELLS_L * CELL_VERTICES;
    piece = gl_VertexIndex / per_piece;
    int cell = (gl_VertexIndex % per_piece) / CELL_VERTICES;
    ivec2 corner = ivec2(cell % CELLS_T, cell / CELLS_T) + CORNER[gl_VertexIndex % CELL_VERTICES];
    float t = float(corner.x) / float(CELLS_T);
    float v = float(corner.y) / float(CELLS_L) - 0.5;
    int s = 1 + piece * 5;
    vec3 a = look.water[s].xyz, b = look.water[s + 1].xyz, c = look.water[s + 2].xyz, d = look.water[s + 3].xyz;
    vec3 centre = ((a * t + b) * t + c) * t + d;
    vec3 along = (3.0 * a * t + 2.0 * b) * t + c;
    vec2 flat_along = normalize(along.xz + vec2(1e-6, 0.0));
    vec2 side = vec2(-flat_along.y, flat_along.x);
    float width = mix(look.water[s].w, look.water[s + 1].w, t);
    world = centre + vec3(side.x, 0.0, side.y) * (v * width * 2.0);
    gl_Position = look.view_proj * vec4(world, 1.0);
    // Past the far plane the sheet goes on to the horizon: its depth stays on the plane.
    gl_Position.z = min(gl_Position.z, gl_Position.w);
    fog_depth = gl_Position.w;
}
