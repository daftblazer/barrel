#version 450
// Models. Characters (flags bit 0) are shaded the way the game's character pixel program
// (CharaMTC) does it, step by step as in docs/shading.md; the rest keep a plain light.
layout(set = 0, binding = 0) uniform sampler2D tex;
// The character's mask (its red channel says how much of the shading applies; white when the
// material has none).
layout(set = 2, binding = 0) uniform sampler2D mask;
layout(push_constant) uniform Push { mat4 view_proj; vec4 params; vec4 scale_a; vec4 scale_b; uint instance; uint flags; } pc;
// The field's character lighting (barrel_formats::field_look), in render space: the
// directions as the field gives them (the shader uses their negation), the colours with their
// weight in w; the camera's eye.
layout(std140, set = 1, binding = 4) uniform Look {
    vec4 light_dir;
    vec4 light_color;
    vec4 rim_dir;
    vec4 rim_color;
    vec4 eye;
    // x: the point lights' strength (atCharaPointLightStrength); y: how many point lights.
    vec4 misc;
    // Up to four point lights (render space, field units): position and range² (w), colour
    // (its own plus its ambient), attenuation {att0, att1, att2} per metre.
    vec4 point_pos[4];
    vec4 point_color[4];
    vec4 point_att[4];
} look;
layout(location = 0) in vec3 normal;
layout(location = 1) in vec2 uv;
layout(location = 2) in vec3 world;
// The point lights' sum at this vertex (the vertex program's v2).
layout(location = 3) in vec3 scene_light;
layout(location = 0) out vec4 color;

// The game's soft-light blend of a colour k over a, weighted by w (k first divided by its
// largest channel, at least 1).
vec3 soft_light(vec3 a, vec4 kw) {
    vec3 k = kw.rgb / max(max(max(kw.r, kw.g), kw.b), 1.0);
    vec3 lo = clamp((a * k - (1.0 - 2.0 * k)) / max(2.0 * k, vec3(0.001)), 0.0, 1.0);
    vec3 hi = clamp((a * k) / (2.0 - 2.0 * k), 0.0, 1.0);
    vec3 pick = mix(lo, hi, floor(k + 0.5));
    vec3 blended = a * k + (pick - a * k) * 0.25;
    return mix(a, blended, kw.w);
}

void main() {
    vec4 c = texture(tex, uv);
    if (c.a < pc.params.x) discard;
    vec3 n = normalize(normal);
    if ((pc.flags & 1u) == 0u) {
        float light = 0.45 + 0.55 * max(dot(n, normalize(vec3(-0.3, 0.6, 0.7))), 0.0);
        color = vec4(c.rgb * light, 1.0);
        return;
    }
    // 1. The toon step on a half-Lambert term, toward the character light.
    float h = clamp((dot(n, -normalize(look.light_dir.xyz)) + 1.0) * 0.5, 0.0, 1.0);
    float t = clamp((h - 0.546875) * 10.66667, 0.0, 1.0);
    vec3 shaded = mix(max(c.rgb * c.rgb, c.rgb * 0.5), c.rgb, t);
    // 2. The light colour, then 3. the mask.
    vec3 lit = clamp(mix(c.rgb, soft_light(shaded, look.light_color), texture(mask, uv).r), 0.0, 1.0);
    // 4. The rim light, screen-blended.
    vec3 v = normalize(look.eye.xyz - world);
    float r = max((1.0 - abs(dot(n, v))) * clamp((dot(n, -normalize(look.rim_dir.xyz)) + 1.0) * 0.5, 0.0, 1.0), 0.0);
    r = clamp((r - 0.3) * 32.0, 0.0, 1.0) * 0.8;
    vec3 rim = clamp(soft_light(c.rgb * r, look.rim_color), 0.0, 1.0);
    vec3 col = 1.0 - (1.0 - lit) * (1.0 - rim);
    // 5. The point lights, laid over (overlay blend), weighted by their brightest channel
    // times the field's point light strength.
    vec3 L = scene_light;
    vec3 over = mix(2.0 * col * L, 1.0 - 2.0 * (1.0 - col) * (1.0 - L), round(col));
    float w = max(max(L.r, L.g), L.b) * look.misc.x;
    col = clamp(col + w * (over - col), 0.0, 1.0);
    // (6. fog is not done yet; 7. atColor is 1 and the HDR range 1.)
    color = vec4(col, 1.0);
}
