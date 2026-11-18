// The sun's light at a render-space point: the field's light texture (the clouds' shadow) and
// the shadow cascades. Included after `look`, `shadow_map` and `pattern_map` are declared.

// The field's light texture at world point p (render space): the point slid along the
// sunlight onto the plane 1000 units up, in the field's own space (x mirrored back), then
// scaled to one repeat per area and moved by the drift. 1 without one.
float cloud_light(vec3 p) {
    vec3 d = vec3(-look.sun_dir.x, look.sun_dir.y, look.sun_dir.z);
    if (look.pattern_on.x == 0.0 || abs(d.y) < 1e-4) {
        return 1.0;
    }
    vec3 g = vec3(-p.x, p.y, p.z);
    float k = (g.y - 1000.0) / d.y;
    vec2 uv = vec2((g.x - k * d.x) * look.pattern.x + look.pattern.z, (g.z - k * d.z) * look.pattern.y + look.pattern.w);
    float light = texture(pattern_map, uv).r;
    // pattern_on.y: the texture's strength (the Ryza games' `fPtn`: the light is 1 less that
    // much of the texture's shade); 0: the texture as it is.
    return look.pattern_on.y > 0.0 ? clamp(1.0 + look.pattern_on.y * (light - 1.0), 0.0, 1.0) : light;
}

// One shadow layer's light at p (16 taps over 1.5 texels), or -1 outside it.
float shadow_layer(mat4 vp, float layer, float t, vec3 p) {
    vec4 c = vp * vec4(p, 1.0);
    vec3 s = c.xyz / c.w;
    vec2 at = s.xy * 0.5 + 0.5;
    if (any(lessThan(at, vec2(0.0))) || any(greaterThan(at, vec2(1.0))) || s.z > 1.0) return -1.0;
    float lit = 0.0;
    for (int i = 0; i < 4; i++) {
        vec2 o = vec2(float(i & 1) * 2.0 - 1.0, float(i >> 1) * 2.0 - 1.0) * t * 0.75;
        for (int k = 0; k < 4; k++) {
            vec2 q = vec2(float(k & 1), float(k >> 1)) * t * 0.5;
            lit += texture(shadow_map, vec4(at + o + q, layer, s.z));
        }
    }
    return lit / 16.0;
}

#include "shadow_cascades.glsl"

float sun_shadow(vec3 p) {
    if (look.shadow_info.y < 0.5) return 1.0;
    if (look.shadow_info.z > 0.5) return sun_shadow_cascades(p);
    float near = shadow_layer(look.shadow_vp, 0.0, look.shadow_info.x, p);
    if (near >= 0.0) return near;
    // Past the near square: the far layer, if on, fading to lit with the distance.
    if (look.shadow_far_info.y < 0.5) return 1.0;
    float far = shadow_layer(look.shadow_far, look.shadow_far_info.y, look.shadow_far_info.x, p);
    if (far < 0.0) return 1.0;
    float fade = clamp((distance(p, look.eye.xyz) - look.shadow_far_info.z) / max(look.shadow_far_info.w - look.shadow_far_info.z, 1.0), 0.0, 1.0);
    return mix(far, 1.0, fade);
}
