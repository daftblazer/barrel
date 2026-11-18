// The game's specular ambient pass (docs/shading.md), for the field's models and terrain: the
// light the environment's specular cube map reflects off a surface, under the screen-space
// occlusion `occ` (1: none). Included by the fragment
// shaders after they declare `look` (with `eye`, `spec_amb` and `spec_tint`), `spec_cube`
// (the cube map, in the game's axes: x not mirrored), `world`, `fog_add` and `fog_keep`, and
// after specular_direct.glsl.

// The light reflected off a pixel with normal `n`, specular colour `f0` and gloss `gloss`: the
// cube map along the reflected view at the mip the gloss picks, fogged as the scene is, times
// a Fresnel term on the specular colour.
vec3 specular_light(vec3 n, vec3 f0, float gloss, float occ) {
    vec3 v = normalize(look.eye.xyz - world);
    float nv = clamp(dot(n, v), 0.0, 1.0);
    // The pass's occlusion is the screen-space one as it is (the stage's vAmbOccRat is (1, 0, 1, 0): no rescale).
    float spec_occ = clamp(occ + (occ + nv) * (occ + nv) - 1.0, 0.0, 1.0);
    vec3 f = f0 + (max(f0, vec3(gloss)) - f0) * pow(1.0 - nv, 5.0);
    vec3 r = reflect(-v, n);
    vec3 env = textureLod(spec_cube, vec3(-r.x, r.y, r.z), look.spec_amb.y * (1.0 - gloss)).rgb * look.spec_amb.x * look.spec_tint.rgb;
    // STAND-IN: the game mixes this toward its screen-space reflections by their alpha; this renderer has none, so the cube map alone.
    // (The scene's fog light is at the stage's key scale, look.fog.z; this pass's own is at 1.)
    env = mix(fog_add / max(look.fog.z, 1e-3), env, fog_keep);
    return occ * f * env * (occ + spec_occ * (1.0 - occ));
}
