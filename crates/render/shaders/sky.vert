#version 450
// The sky plane (the Ryza games' `skyplane.g1s`, docs/shading.md "Ryza DX: the sky"): a
// triangle over the screen on the far plane; each corner's view ray from the eye, in render
// space (x mirrored from the game's, y up). The look block's members as model.vert has them,
// up to the sky's (slots 76 on).
layout(std140, set = 1, binding = 4) uniform Look {
    vec4 head[4];
    vec4 eye;
    vec4 middle[39];
    mat4 view_proj;
    vec4 tail[28];
    vec4 sky[11];
} look;
layout(location = 0) out vec3 ray;
void main() {
    vec2 p = vec2((gl_VertexIndex << 1) & 2, gl_VertexIndex & 2) * 2.0 - 1.0;
    gl_Position = vec4(p, 1.0, 1.0);
    vec4 w = inverse(look.view_proj) * vec4(p, 1.0, 1.0);
    ray = w.xyz / w.w - look.eye.xyz;
}
