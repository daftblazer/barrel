#version 450
// A cascade's depth as the game's shadow pass writes it (docs/shading.md, "The ambient-and-shadow
// pass"): the depth plus its slope times `sbias.x` plus `sbias.y`, and a texel under `sbias.w`
// of alpha casts nothing (leaves, fences). params: {alpha reference, slope, constant}.
layout(set = 0, binding = 0) uniform sampler2D tex;
layout(push_constant) uniform Push { mat4 view_proj; vec4 params; } pc;
layout(location = 1) in vec2 uv;
// The bias only pushes the depth deeper, so the depth test and culling can still run early.
layout(depth_greater) out float gl_FragDepth;
void main() {
    if (pc.params.x >= 0.0 && texture(tex, uv).a < pc.params.x) discard;
    float z = gl_FragCoord.z;
    gl_FragDepth = z + pc.params.y * (abs(dFdx(z)) + abs(dFdy(z))) + pc.params.z;
}
