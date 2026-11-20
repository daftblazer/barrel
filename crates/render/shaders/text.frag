#version 450
// A glyph takes its coverage from the atlas's alpha; a negative u fills with the colour.
layout(set = 0, binding = 0) uniform sampler2D atlas;
// A clipped quad draws only inside its mask's rectangle where the mask picture is not clear
// (the games' stencil: their 2D shader drops a pixel under 1/255 alpha).
layout(set = 1, binding = 0) uniform sampler2D mask;
layout(location = 0) in vec2 v_uv;
layout(location = 1) in vec4 v_color;
layout(location = 2) in vec2 v_mask_at;
layout(location = 3) flat in vec4 v_mask_uv;
layout(location = 0) out vec4 out_color;
void main() {
    if (v_mask_uv.x >= 0.0) {
        if (any(lessThan(v_mask_at, vec2(0.0))) || any(greaterThan(v_mask_at, vec2(1.0)))) discard;
        if (texture(mask, mix(v_mask_uv.xy, v_mask_uv.zw, v_mask_at)).a < 1.0 / 255.0) discard;
    }
    vec4 t = v_uv.x < 0.0 ? vec4(1.0) : texture(atlas, v_uv);
    out_color = vec4(v_color.rgb * t.rgb, v_color.a * t.a);
}
