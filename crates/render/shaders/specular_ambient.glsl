// The game's specular ambient pass (docs/shading.md), for the field's models and terrain: the
// light the environment's specular cube map reflects off a surface, under the screen-space
// occlusion `occ` (1: none). Included by the fragment
// shaders after they declare `look` (with `eye`, `spec_amb` and `spec_tint`), `spec_cube`
// (the cube map, in the game's axes: x not mirrored), `world`, `fog_add` and `fog_keep`, and
// after specular_direct.glsl.

// The light reflected off a pixel with normal `n`, specular colour `f0` and gloss `gloss`: the
// cube map along the reflected view at the mip the gloss picks, fogged as the scene is, times
// a Fresnel term on the specular colour. With the screen-space reflections off (or in the
// shaders that have none) it is the cube map alone.
vec3 specular_light(vec3 n, vec3 f0, float gloss, float occ, vec3 scale, out vec3 factor, out vec3 env_light) {
    vec3 v = normalize(look.eye.xyz - world);
    float nv = clamp(dot(n, v), 0.0, 1.0);
    // The pass's occlusion is the screen-space one as it is (the stage's vAmbOccRat is (1, 0, 1, 0): no rescale).
    float spec_occ = clamp(occ + (occ + nv) * (occ + nv) - 1.0, 0.0, 1.0);
    vec3 f = f0 + (max(f0, vec3(gloss)) - f0) * pow(1.0 - nv, 5.0);
    vec3 r = reflect(-v, n);
    vec3 env = textureLod(spec_cube, vec3(-r.x, r.y, r.z), look.spec_amb.y * (1.0 - gloss)).rgb * scale;
    // The reflections' pass (reflect.rs) mixes `env` toward the screen-space reflections: it
    // takes it and the term the light is added by (without the fog's extinction).
    env_light = env;
    factor = occ * f * (occ + spec_occ * (1.0 - occ));
    // (The scene's fog light is at the stage's key scale, look.fog.z; this pass's own is at 1.)
    env = mix(fog_add / max(look.fog.z, 1e-3), env, fog_keep);
    return occ * f * env * (occ + spec_occ * (1.0 - occ));
}

vec3 specular_light(vec3 n, vec3 f0, float gloss, float occ, vec3 scale) {
    vec3 factor, env;
    return specular_light(n, f0, gloss, occ, scale, factor, env);
}

// The same with the stage's light scale and tint (the field's models and the terrain).
vec3 specular_light(vec3 n, vec3 f0, float gloss, float occ) {
    return specular_light(n, f0, gloss, occ, look.spec_amb.x * look.spec_tint.rgb);
}
