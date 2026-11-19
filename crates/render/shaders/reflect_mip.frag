#version 450
// One level of the reflections' mip chain: a bilinear sample of the level above (the game's
// own, docs/shading.md, "The screen-space reflections").
layout(set = 0, binding = 0) uniform sampler2D above;
layout(push_constant) uniform Push {
    // The level's size inverse.
    vec2 inverse_size;
} pc;
layout(location = 0) out vec4 level;

void main() {
    level = texture(above, gl_FragCoord.xy * pc.inverse_size);
}
