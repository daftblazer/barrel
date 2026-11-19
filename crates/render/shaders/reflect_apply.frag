#version 450
// The reflections' share of the specular ambient (docs/shading.md, "The specular ambient pass"):
// the game mixes the scaled cube light toward the reflection by its alpha; the scene here
// already holds the cube light (model.frag adds it), so this adds the difference: the light's
// per-pixel factor times the weight times (reflection - cube light), over the scene by addition.
// spec_light: rgb the factor, a the gloss; spec_env: rgb the scaled cube light.
layout(set = 0, binding = 0) uniform sampler2D spec_light;
layout(set = 0, binding = 1) uniform sampler2D spec_env;
layout(set = 0, binding = 2) uniform sampler2D reflections;
layout(push_constant) uniform Push {
    // The scene's size inverse.
    vec2 inverse_size;
} pc;
layout(location = 0) out vec4 added;

// The constants of the captured frame (Rasenboden at noon).
const float MERGE_X = 1.0;     // vMerge.x: the reflection's weight times its alpha
const float MIP_LAST = 5.0;    // vAmbParam.z: the cube map's last mip, the chain's too

void main() {
    ivec2 px = ivec2(gl_FragCoord.xy);
    vec4 factor = texelFetch(spec_light, px, 0);
    if (factor.rgb == vec3(0.0)) discard;
    vec2 uv = (vec2(px) + 0.5) * pc.inverse_size;
    float weight = textureLod(reflections, uv, 0.0).a * MERGE_X;
    vec3 colour = textureLod(reflections, uv, MIP_LAST * (1.0 - factor.a)).rgb;
    added = vec4(factor.rgb * weight * (colour - texelFetch(spec_env, px, 0).rgb), 0.0);
}
