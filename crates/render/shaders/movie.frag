#version 450
// I420 to RGB: limited range, BT.709 (the movies carry no colour description; Media Foundation
// takes HD video without one as BT.709), times `level` for fades.
layout(set = 0, binding = 0) uniform sampler2D plane_y;
layout(set = 0, binding = 1) uniform sampler2D plane_u;
layout(set = 0, binding = 2) uniform sampler2D plane_v;
layout(push_constant) uniform Push { vec2 screen; float level; float pad; vec4 rect; } p;
layout(location = 0) in vec2 v_uv;
layout(location = 0) out vec4 out_color;
void main() {
    float y = (texture(plane_y, v_uv).r * 255.0 - 16.0) / 219.0;
    float cb = (texture(plane_u, v_uv).r * 255.0 - 128.0) / 224.0;
    float cr = (texture(plane_v, v_uv).r * 255.0 - 128.0) / 224.0;
    vec3 rgb = vec3(y + 1.5748 * cr, y - 0.1873 * cb - 0.4681 * cr, y + 1.8556 * cb);
    out_color = vec4(clamp(rgb, 0.0, 1.0) * p.level, 1.0);
}
