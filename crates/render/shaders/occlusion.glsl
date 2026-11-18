// The screen-space ambient occlusion (ao.rs) as the main pass's fragments take it. Included by
// the fragment shaders after they define AO_SET, the set the map is bound as.
layout(set = AO_SET, binding = 0) uniform sampler2D ao_map;

// The occlusion at this fragment: the half-size map stretched bilinearly over the scene (the
// game's own upsampling); 1 where there is none (a one-texel white map).
float screen_occlusion() {
    return texture(ao_map, gl_FragCoord.xy * 0.5 / vec2(textureSize(ao_map, 0))).r;
}

// The lighting pass's `fOccAlbScl` for the field's stage draw (captured at Rasenboden).
const float STAGE_OCC_ALBEDO = 1.0;

// The occlusion `o` tinted by the surface's own colour `a`, as the lighting pass does for the
// stage: `a * s + o * (1 - a * s)`, which leaves a bright surface less occluded than a dark one.
vec3 occlusion_tint(vec3 a, float o) {
    vec3 s = a * STAGE_OCC_ALBEDO;
    return s + o * (1.0 - s);
}
