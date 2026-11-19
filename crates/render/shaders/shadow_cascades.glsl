// The sun's shadow from the cascades (Ryza's fields; docs/shading.md, "The ambient-and-shadow
// pass"). Included after `look` (with shadow_vp, shadow_info, shadow_far, shadow_far_info,
// shadow_c1, shadow_c2, shadow_ends, shadow_fwd) and `shadow_map` are declared.

// A layer's map within the layer (`shadow_sides`): the share of the layer's u and v it covers
// and its texel; the whole layer at texel t when the game gives no sizes.
vec4 layer_side(int k, float t) {
    vec4 side = look.shadow_sides[k];
    return side.x > 0.0 ? side : vec4(1.0, 1.0, t, t);
}

// One layer's light at p: the game's sixteen taps, four by four a texel apart from 1.5 texels
// before p, each a bilinear depth comparison (kept in the layer's map); or -1 outside the
// layer's box.
float cascade_light(mat4 vp, float layer, float t, vec3 p) {
    vec4 c = vp * vec4(p, 1.0);
    vec3 s = c.xyz / c.w;
    vec2 at = s.xy * 0.5 + 0.5;
    if (any(lessThan(at, vec2(0.0))) || any(greaterThan(at, vec2(1.0))) || s.z > 1.0) return -1.0;
    vec4 side = layer_side(int(layer), t);
    float lit = 0.0;
    for (int i = 0; i < 4; i++) {
        for (int k = 0; k < 4; k++) {
            vec2 q = clamp(at + (vec2(float(i), float(k)) - 1.5) * side.zw, vec2(0.0), vec2(1.0));
            lit += texture(shadow_map, vec4(q * side.xy, layer, s.z));
        }
    }
    return lit / 16.0;
}

// The characters' map's light at p (the game's `mSubW2S` and `sSubShadow`): sixteen point taps, four
// by four a texel apart from 1.5 texels before p, each a plain depth comparison (the game's
// sampler is a point one: the tap is moved to its texel's centre for the comparison sampler),
// the depth saturated; 1 outside the map's box. The map takes `shadow_sub_info.y` of the
// layer's side, from its corner.
float sub_light(vec3 p) {
    vec4 c = look.shadow_sub * vec4(p, 1.0);
    vec2 at = c.xy * 0.5 + 0.5;
    float depth = clamp(c.z, 0.0, 1.0);
    float t = look.shadow_sub_info.x;
    if (any(greaterThanEqual(at - 1.5 * t, vec2(1.0))) || any(lessThanEqual(at + 1.5 * t, vec2(0.0)))) return 1.0;
    float texels = 1.0 / t;
    float lit = 0.0;
    for (int i = 0; i < 4; i++) {
        for (int k = 0; k < 4; k++) {
            vec2 q = at + (vec2(float(i), float(k)) - 1.5) * t;
            q = (floor(clamp(q, vec2(0.0), vec2(0.999999)) * texels) + 0.5) * t;
            lit += texture(shadow_map, vec4(q * look.shadow_sub_info.y, look.shadow_sub_info.w, depth));
        }
    }
    return lit / 16.0;
}

// The static layer's light at p (the game's `sSemiStaticShadow`, mode 2): sixteen point taps
// four by four a texel apart from 1.5 texels before p, each a plain depth comparison with the
// depth saturated; 1 when the taps' square leaves the map.
float static_light(vec3 p) {
    vec4 c = look.shadow_far * vec4(p, 1.0);
    vec2 at = (c.xy / c.w) * 0.5 + 0.5;
    float depth = clamp(c.z / c.w, 0.0, 1.0);
    vec4 side = layer_side(3, look.shadow_far_info.x);
    vec2 t = side.zw;
    vec2 first = at - 1.5 * t;
    if (any(greaterThanEqual(first, vec2(1.0))) || any(lessThanEqual(first + 3.0 * t, vec2(0.0)))) return 1.0;
    vec2 texels = 1.0 / t;
    float lit = 0.0;
    for (int i = 0; i < 4; i++) {
        for (int k = 0; k < 4; k++) {
            vec2 q = first + vec2(float(i), float(k)) * t;
            q = (floor(clamp(q, vec2(0.0), vec2(0.999999)) * texels) + 0.5) * t;
            lit += texture(shadow_map, vec4(q * side.xy, look.shadow_far_info.y, depth));
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
    // The characters' own map shades what the cascades light (the larger shadow kept).
    if (look.shadow_sub_info.z > 0.5) lit = min(lit, sub_light(p));
    float w = clamp((look.shadow_ends.z - z) / look.shadow_ends.w, 0.0, 1.0);
    if (w < 1.0) {
        float fixed_lit = 1.0;
        if (look.shadow_far_info.y > 0.5) {
            fixed_lit = static_light(p);
        }
        lit = mix(fixed_lit, lit, w);
    }
    return lit;
}
