#version 450
// A movie picture over `rect` (pixels, y down), its three planes sampled at uv.
layout(push_constant) uniform Push { vec2 screen; float level; float pad; vec4 rect; } p;
layout(location = 0) out vec2 v_uv;
void main() {
    vec2 corner[6] = vec2[](vec2(0, 0), vec2(1, 0), vec2(1, 1), vec2(0, 0), vec2(1, 1), vec2(0, 1));
    vec2 c = corner[gl_VertexIndex];
    v_uv = c;
    vec2 pos = mix(p.rect.xy, p.rect.zw, c);
    gl_Position = vec4(pos / p.screen * 2.0 - 1.0, 0.0, 1.0);
}
