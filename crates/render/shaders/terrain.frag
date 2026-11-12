#version 450
// Field terrain: two blend maps over the whole field weight six tiled ground layers.
layout(set = 0, binding = 0) uniform sampler2D blend[2];
layout(set = 0, binding = 1) uniform sampler2D layers[6];
layout(push_constant) uniform Push { mat4 view_proj; vec4 params; vec4 scale_a; vec4 scale_b; } pc;
layout(location = 0) in vec3 normal;
layout(location = 1) in vec2 uv; // 0..1 across the field
layout(location = 2) in vec3 world;
// The field light at this vertex (model.vert), the sun's part apart.
layout(location = 4) in vec3 field_light;
layout(location = 6) in vec3 sun_part;
layout(std140, set = 1, binding = 4) uniform Look {
    vec4 light_dir;
    vec4 light_color;
    vec4 rim_dir;
    vec4 rim_color;
    vec4 eye;
    // x: the point lights' strength (atCharaPointLightStrength); y: how many point lights.
    vec4 misc;
    // Up to four point lights (render space, field units): position and range² (w), colour,
    // attenuation {att0, att1, att2} over field units, and ambient.
    vec4 point_pos[4];
    vec4 point_color[4];
    vec4 point_att[4];
    vec4 point_ambient[4];
    // The field's lights for its own models: the first directional light (the sun: direction,
    // colour), the second (the fill), and the ambient.
    vec4 sun_dir;
    vec4 sun_color;
    vec4 fill_dir;
    vec4 fill_color;
    vec4 ambient;
    // The sun's shadow map: world (render space) to its clip space, and {texel size, on}.
    mat4 shadow_vp;
    vec4 shadow_info;
} look;
layout(set = 1, binding = 5) uniform sampler2DShadow shadow_map;
// The sun's shadow at a render-space point: MapSTD's 16 taps (four 2 × 2 comparisons a texel
// and a half apart), 1 lit to 0 shadowed (atColor white: the full strength).
float sun_shadow(vec3 p) {
    if (look.shadow_info.y < 0.5) return 1.0;
    vec4 c = look.shadow_vp * vec4(p, 1.0);
    vec3 s = c.xyz / c.w;
    vec2 at = s.xy * 0.5 + 0.5;
    if (any(lessThan(at, vec2(0.0))) || any(greaterThan(at, vec2(1.0))) || s.z > 1.0) return 1.0;
    float t = look.shadow_info.x, lit = 0.0;
    for (int i = 0; i < 4; i++) {
        vec2 o = vec2(float(i & 1) * 2.0 - 1.0, float(i >> 1) * 2.0 - 1.0) * t * 0.75;
        for (int k = 0; k < 4; k++) {
            vec2 q = vec2(float(k & 1), float(k >> 1)) * t * 0.5;
            lit += texture(shadow_map, vec3(at + o + q, s.z));
        }
    }
    return lit / 16.0;
}

layout(location = 0) out vec4 color;
void main() {
    vec3 w0 = texture(blend[0], uv).rgb;
    vec3 w1 = texture(blend[1], uv).rgb;
    float w[6] = float[6](w0.r, w0.g, w0.b, w1.r, w1.g, w1.b);
    float s[6] = float[6](pc.scale_a.x, pc.scale_a.y, pc.scale_a.z, pc.scale_a.w, pc.scale_b.x, pc.scale_b.y);
    vec2 field_xz = uv * pc.params.y;
    vec3 c = vec3(0.0);
    float total = 0.0;
    for (int i = 0; i < 6; i++) {
        c += w[i] * texture(layers[i], field_xz * s[i]).rgb;
        total += w[i];
    }
    c = total > 0.001 ? c / total : texture(layers[0], field_xz * s[0]).rgb;
    // Lit as the field's own terrain program does (its pack's pixel program 9 in TOWN_10):
    // the same per-vertex light as the field's models (the vertex colour white), on the
    // blended layers; the light texture, shadow and specular are not done yet.
    color = vec4((field_light + sun_part * sun_shadow(world)) * c * pc.params.w, 1.0);
}
