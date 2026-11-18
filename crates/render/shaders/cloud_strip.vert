#version 450
// The sky's large clouds (the Ryza games' strip, docs/shading.md "Ryza DX: the sky, the
// clouds"): a 16 by 16 grid of cells centred on the eye, its height a parabola falling away
// with the square of the distance, so it bowls over the whole view. Each cell is two triangles
// split along the diagonal from its (0, 0) corner to its (1, 1) corner, as the game's grid (one
// triangle strip down each column, its index buffer's order) splits them: the height's parabola
// and the edge fade are only linear across a triangle, so the diagonal shows in the picture.
// Render space mirrors x against the game's, so the
// grid's x runs the other way; the map's coordinates run with the game's.
// Look slots 97 on (`cloud`): [0] the light's way, [1] its colour, [2] the ambient, [3] the
// clouds' colour and density, [4] {grid width, -, the surface's fall, its height at the eye's
// column}, [5] {the map's tiling x z, its offset x z}, [6] {thickness, the most a step moves the
// map's coordinate, extinction, scattering scale}, [7] {the step's scale x z, fog x y},
// [8..11] Scat.
layout(std140, set = 1, binding = 4) uniform Look {
    vec4 head[4];
    vec4 eye;
    vec4 middle[39];
    mat4 view_proj;
    vec4 tail[28];
    vec4 sky[11];
    vec4 other[10];
    vec4 cloud[12];
} look;
layout(location = 0) out vec3 world;
layout(location = 1) out vec2 map_uv;
layout(location = 2) out vec4 tint;
layout(location = 3) out vec2 fog_height;

const int CELLS = 16;
const ivec2 CORNER[6] = ivec2[](ivec2(0, 0), ivec2(1, 0), ivec2(1, 1), ivec2(0, 0), ivec2(1, 1), ivec2(0, 1));

void main() {
    int cell = gl_VertexIndex / 6;
    vec2 g = vec2(ivec2(cell % CELLS, cell / CELLS) + CORNER[gl_VertexIndex % 6]) / float(CELLS);
    vec2 d = (g - 0.5) * look.cloud[4].x;
    float y = dot(d, d) * look.cloud[4].z + look.cloud[4].w;
    world = vec3(look.eye.x - d.x, y, look.eye.z + d.y);
    gl_Position = look.view_proj * vec4(world, 1.0);
    map_uv = g * look.cloud[5].xy + look.cloud[5].zw;
    // The colour, and an alpha that fades the grid out toward its edge.
    vec2 e = g * 2.0 - 1.0;
    float edge = min((1.0 - min(dot(e, e), 1.0)) * 1.5, 1.0);
    tint = vec4(look.cloud[3].rgb, edge * look.cloud[3].w);
    fog_height = vec2(gl_Position.w * look.cloud[7].z + look.cloud[7].w, y / look.cloud[4].w);
}
