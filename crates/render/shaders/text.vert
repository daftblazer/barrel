#version 450
// Screen-space text and panels: positions in pixels, y down.
layout(location = 0) in vec2 pos;
layout(location = 1) in vec2 uv;
layout(location = 2) in vec4 color;
// A clip's mask (`text::Clip`): where the vertex falls in the mask's rectangle (0 to 1) and the
// mask picture's texture coordinates; a negative first one: no clip.
layout(location = 3) in vec2 mask_at;
layout(location = 4) in vec4 mask_uv;
layout(push_constant) uniform Push { vec2 screen; } p;
layout(location = 0) out vec2 v_uv;
layout(location = 1) out vec4 v_color;
layout(location = 2) out vec2 v_mask_at;
layout(location = 3) flat out vec4 v_mask_uv;
void main() {
    v_uv = uv;
    v_color = color;
    v_mask_at = mask_at;
    v_mask_uv = mask_uv;
    gl_Position = vec4(pos / p.screen * 2.0 - 1.0, 0.0, 1.0);
}
