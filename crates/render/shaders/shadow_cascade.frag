#version 450
// A cascade's caster as the game's shadow pass writes it (docs/shading.md, "The ambient-and-shadow
// pass"): a texel under `sbias.w` of alpha casts nothing (leaves, fences). The game's depth
// bias (its slope times the depth's slope, plus a constant) is not written here: the constant
// is added in the vertex shader (`CASCADE_BIAS`) and the slope term is the pipeline's depth bias,
// so the depth test runs early. params: {alpha reference, slope, constant}.
layout(set = 0, binding = 0) uniform sampler2D tex;
layout(push_constant) uniform Push { mat4 view_proj; vec4 params; } pc;
layout(location = 1) in vec2 uv;
void main() {
    if (pc.params.x >= 0.0 && texture(tex, uv).a < pc.params.x) discard;
}
