#version 450
// A glyph takes its coverage from the atlas's alpha; a negative u fills with the colour.
layout(set = 0, binding = 0) uniform sampler2D atlas;
layout(location = 0) in vec2 v_uv;
layout(location = 1) in vec4 v_color;
layout(location = 0) out vec4 out_color;
void main() {
    vec4 t = v_uv.x < 0.0 ? vec4(1.0) : texture(atlas, v_uv);
    out_color = vec4(v_color.rgb * t.rgb, v_color.a * t.a);
}
