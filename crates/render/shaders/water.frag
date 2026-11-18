#version 450
// The Ryza games' water, shaded forward (docs/shading.md, "Ryza DX: the water"): a wave normal,
// Fresnel between the body's lit colour and the sky it reflects, the sun's highlight, then the
// sky's scattering over the distance as the clouds'.
layout(std140, set = 1, binding = 4) uniform Look {
    vec4 head[4];
    vec4 eye;
    vec4 middle[39];
    mat4 view_proj;
    vec4 tail[28];
    vec4 sky[11];
    vec4 other[10];
    vec4 cloud[12];
    vec4 specular[2];
    vec4 water[81];
} look;
layout(location = 0) in vec3 world;
layout(location = 1) flat in int piece;
layout(location = 2) in float fog_depth;
layout(location = 0) out vec4 colour;
layout(location = 1) out vec4 motion;

const float HDR_RANGE_INV = 0.5;

// The wave normal's gradient: a few travelling sines across the surface, each a steeper, shorter
// swell than the last.
// STAND-IN: the game's waves are the wave volume texture (`wavevolume.g1t`) sampled five times at offsets along the view and scrolled; these are sines of a similar scale.
vec3 wave_normal(vec2 p, float time, float pixel) {
    const vec4 DIR[4] = vec4[](vec4(0.8, 0.6, 0.011, 0.9), vec4(-0.6, 0.8, 0.017, 1.3), vec4(0.3, -0.95, 0.029, 1.9), vec4(-0.95, -0.3, 0.043, 2.7));
    vec2 slope = vec2(0.0);
    for (int i = 0; i < 4; i++) {
        float phase = dot(p, DIR[i].xy) * DIR[i].z + time * DIR[i].w;
        // A swell much shorter than a few pixels is left out (it would only shimmer).
        float seen = clamp(1.0 - pixel * DIR[i].z * 1.0, 0.0, 1.0);
        slope += DIR[i].xy * (cos(phase) * DIR[i].z * 9.0 / float(i + 1) * seen);
    }
    return normalize(vec3(-slope.x, 1.0, -slope.y));
}

// The sky plane's colour along d (sky.frag).
vec3 sky_colour(vec3 d) {
    vec4 sp = look.sky[5];
    float l = abs(-d.y * sp.x - sqrt(max(d.y * d.y * sp.y + sp.z, 0.0))) * look.sky[6].x + look.sky[6].y;
    vec3 t = vec3(1.0) - min(exp2(l * look.sky[1].xyz), vec3(1.0));
    float c = dot(d, -look.sky[7].xyz);
    vec3 s = (1.0 + c * c) * look.sky[3].xyz + pow(max(c * look.sky[3].w + look.sky[4].w, 0.0001), -1.5) * look.sky[4].xyz + look.sky[2].xyz;
    float x = clamp(dot(d, look.sky[8].xyz) * look.sky[10].x + look.sky[10].y, 0.0, 1.0);
    float k = (1.0 - exp(-x * x)) * clamp(look.sky[9].w, 0.0, 1.0);
    return mix(s * t, look.sky[9].rgb, k);
}

void main() {
    vec3 to_eye = normalize(look.eye.xyz - world);
    vec4 body = look.water[1 + piece * 5 + 4];
    vec3 n = wave_normal(world.xz, look.water[0].y, fog_depth * 0.0016 * 6.0 / 6.2832);
    // Seen from low, the swell's slope flattens (it is far and small on the screen).
    n = normalize(mix(vec3(0.0, 1.0, 0.0), n, clamp(to_eye.y * 4.0, 0.0, 1.0)));
    float cos_v = max(dot(n, to_eye), 0.0);
    float fresnel = body.w + (1.0 - body.w) * pow(1.0 - cos_v, 5.0);
    // The field's sun (render space, as the terrain takes it): the way its light travels, its colour, and the ambient.
    vec3 way = normalize(look.middle[17].xyz);
    vec3 sun = look.middle[18].rgb;
    vec3 ambient = look.middle[21].rgb;
    vec3 reflected = reflect(-to_eye, n);
    reflected.y = abs(reflected.y);
    vec3 lit = body.rgb * (sun * max(dot(n, -way), 0.0) + ambient);
    vec3 spec = sun * pow(max(dot(n, normalize(to_eye - way)), 0.0), 80.0) * fresnel;
    vec3 c = mix(lit * HDR_RANGE_INV, sky_colour(normalize(reflected)), fresnel) + spec * HDR_RANGE_INV;
    // The field's fog over the distance, as the models take it (model.vert), the lit part at the
    // scene's half brightness (the post pass doubles it; the sky plane's colour is already there).
    float s = look.middle[27].x * length(look.eye.xyz - world) + look.middle[27].y;
    vec3 keep = min(exp2(s * look.middle[28].xyz), vec3(1.0));
    float cs = dot(to_eye, way);
    vec3 add = look.middle[31].xyz * pow(max(look.middle[30].w * cs + look.middle[31].w, 0.0001), -1.5) + (1.0 + cs * cs) * look.middle[30].xyz + look.middle[29].xyz;
    colour = vec4(mix(add * HDR_RANGE_INV, c, keep), 1.0);
    motion = vec4(0.0);
}
