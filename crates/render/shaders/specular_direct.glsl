// The game's direct specular (docs/shading.md, "The lighting pass"), for the field's models and
// terrain: each directional light's highlight on a surface of specular colour `f0` and gloss.
// Included by the fragment shaders after they declare `look` (with `eye`, `sun_dir`,
// `sun_color`, `fill_dir`, `fill_color`) and `world`.

// One light's highlight: `way` is the direction the light travels, `colour` its colour.
vec3 highlight(vec3 n, vec3 v, vec3 way, vec3 colour, vec3 f0, float power, float visible) {
    if (dot(way, way) == 0.0) return vec3(0.0);
    vec3 l = -normalize(way);
    vec3 h = normalize(v + l);
    float vh = clamp(dot(v, h), 0.0, 1.0);
    vec3 fresnel = f0 + (1.0 - f0) * pow(1.0 - vh, 5.0);
    float nh = clamp(dot(n, h), 0.0, 1.0);
    return colour * clamp(dot(n, l), 0.0, 1.0) * ((power + 2.0) * 0.125) * fresnel * pow(nh, power) * visible;
}

// The field's two directional lights' highlights on pixel normal `n`: the sun times `sun_lit`
// (its shadow and light texture, the lighting pass's alpha), the second light as it is. Gloss
// `gloss` sets the power 2^(10 gloss + 1); the screen-space `occlusion` (1: none) narrows it by
// the view angle.
vec3 direct_specular(vec3 n, vec3 f0, float gloss, float sun_lit, float occlusion) {
    vec3 v = normalize(look.eye.xyz - world);
    float nv = clamp(dot(n, v), 0.0, 1.0);
    float visible = clamp((nv + occlusion) * (nv + occlusion) + occlusion - 1.0, 0.0, 1.0);
    float power = exp2(10.0 * gloss + 1.0);
    return highlight(n, v, look.sun_dir.xyz, look.sun_color.rgb, f0, power, visible * sun_lit)
        + highlight(n, v, look.fill_dir.xyz, look.fill_color.rgb, f0, power, visible);
}
