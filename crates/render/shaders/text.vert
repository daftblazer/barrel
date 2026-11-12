#version 450
// Screen-space text and panels: positions in pixels, y down.
layout(location = 0) in vec2 pos;
layout(location = 1) in vec2 uv;
layout(location = 2) in vec4 color;
layout(push_constant) uniform Push { vec2 screen; } p;
layout(location = 0) out vec2 v_uv;
layout(location = 1) out vec4 v_color;
void main() {
    v_uv = uv;
    v_color = color;
    gl_Position = vec4(pos / p.screen * 2.0 - 1.0, 0.0, 1.0);
}
