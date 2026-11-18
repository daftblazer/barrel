// The sun's shadow from the cascades (Ryza's fields; docs/shading.md, "The ambient-and-shadow
// pass"). Included after `look` (with shadow_vp, shadow_info, shadow_far, shadow_far_info,
// shadow_c1, shadow_c2, shadow_ends, shadow_fwd) and `shadow_map` are declared.

// One layer's light at p: the game's sixteen taps, four by four a texel apart from 1.5 texels
// before p, each a bilinear depth comparison; or -1 outside the layer's box.
float cascade_light(mat4 vp, float layer, float t, vec3 p) {
    vec4 c = vp * vec4(p, 1.0);
    vec3 s = c.xyz / c.w;
    vec2 at = s.xy * 0.5 + 0.5;
    if (any(lessThan(at, vec2(0.0))) || any(greaterThan(at, vec2(1.0))) || s.z > 1.0) return -1.0;
    float lit = 0.0;
    for (int i = 0; i < 4; i++) {
        for (int k = 0; k < 4; k++) {
            lit += texture(shadow_map, vec4(at + (vec2(float(i), float(k)) - 1.5) * t, layer, s.z));
        }
    }
    return lit / 16.0;
}

// The sun's light at p: the cascade of p's distance along the view (the next one where p
// falls outside its box), fading before the last cascade's end into the static layer
// (the game's semi-static map), which holds the rest.
float sun_shadow_cascades(vec3 p) {
    float t = look.shadow_info.x;
    float z = dot(p - look.eye.xyz, look.shadow_fwd.xyz);
    int k = z < look.shadow_ends.x ? 0 : (z < look.shadow_ends.y ? 1 : 2);
    float lit = -1.0;
    for (; k < 3 && lit < 0.0; k++) {
        lit = cascade_light(k == 0 ? look.shadow_vp : (k == 1 ? look.shadow_c1 : look.shadow_c2), float(k), t, p);
    }
    if (lit < 0.0) lit = 1.0;
    float w = clamp((look.shadow_ends.z - z) / look.shadow_ends.w, 0.0, 1.0);
    if (w < 1.0) {
        float fixed_lit = 1.0;
        if (look.shadow_far_info.y > 0.5) {
            fixed_lit = cascade_light(look.shadow_far, look.shadow_far_info.y, look.shadow_far_info.x, p);
            if (fixed_lit < 0.0) fixed_lit = 1.0;
        }
        lit = mix(fixed_lit, lit, w);
    }
    return lit;
}
