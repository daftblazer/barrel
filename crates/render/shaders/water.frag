#version 450
#extension GL_GOOGLE_include_directive : require
// The Ryza games' water, shaded forward (docs/shading.md, "Ryza DX: the water"): a wave normal,
// the lighting pass's light and highlights on the body's albedo, the refracted opaque scene seen
// through it, then the field's fog over the distance.
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
    // The field's fog: {1 / (end - start), -start / (end - start)} in field units, and the
    // scattering constants (docs/shading.md, "Fog": read from the game's memory).
    vec4 fog;
    vec4 scat[4];
    // Slots 37 to 47: the billboards' axes and the view-projections (model.vert's).
    vec4 vertex_only[11];
    // The field's light texture (docs/shading.md, "The light texture"): {1 / width,
    // 1 / height (field units), u + 0.5, v + 0.5}, and {on}.
    vec4 pattern;
    vec4 pattern_on;
    // The grass sway groups (`<v_shaker>`, game::wind): {wind.xyz, clock}, {1 / height,
    // root height, shift x, shift z} each, metres.
    vec4 sway[16];
    // Slot 66: the cube map's light on (x > 0.5); 67 to 75 its spherical-harmonic
    // coefficients (model.vert's `environment_light`); 76 to 86 the sky plane's.
    vec4 sh_on;
    vec4 sh[9];
    vec4 others[11];
    // The far shadow square (lib.rs SHADOW_FAR_SLOT): view-projection, {texel, layer (0:
    // none), fade start, fade end}.
    mat4 shadow_far;
    vec4 shadow_far_info;
    // Slots 92 to 108: the vertex program's, the emission's, the sky's.
    vec4 frag_pad[17];
    // 109: the specular ambient's (model.frag); z: the game's per-pixel lighting of the
    // ground (its normal maps and direct specular) on, for the Ryza games' fields.
    vec4 spec_amb;
    vec4 spec_tint;
    // Slots 111 to 191: the water's. Then the cascades' (lib.rs SHADOW_CASCADE_SLOT): the
    // second and third view-projections, the cascades' ends along the view and the fade width
    // into the static layer, the view's forward.
    vec4 water_pad[81];
    mat4 shadow_c1;
    mat4 shadow_c2;
    vec4 shadow_ends;
    vec4 shadow_fwd;
    // The characters' map (lib.rs SHADOW_SUB_SLOT): view-projection, {texel, share of the
    // layer, on, layer}.
    mat4 shadow_sub;
    vec4 shadow_sub_info;
    vec4 chara_spec; // CHARA_SPECULAR_SLOT, not read here
    // The cascades' and the static layer's maps within their layers (lib.rs SHADOW_SIDES_SLOT).
    vec4 shadow_sides[4];
} look;
layout(set = 1, binding = 5) uniform sampler2DArrayShadow shadow_map;
layout(set = 1, binding = 6) uniform sampler2D pattern_map;
// The opaque scene's colour (the first part of the scene pass, copied), and its view depth (the
// occlusion pass's, in alpha, at half size: it holds the scene without the water). One texel
// wide when there is none: the water is then opaque.
layout(set = 0, binding = 0) uniform sampler2D refraction_map;
layout(set = 2, binding = 0) uniform sampler2D scene_depth;
layout(location = 0) in vec3 world;
layout(location = 1) flat in int piece;
layout(location = 2) in float fog_depth;
layout(location = 0) out vec4 colour;
layout(location = 1) out vec4 motion;

const float HDR_RANGE_INV = 0.5;

#include "environment_light.glsl"
#include "specular_direct.glsl"
#include "sun_light.glsl"

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

void main() {
    vec3 to_eye = normalize(look.eye.xyz - world);
    vec4 body = look.water_pad[1 + piece * 5 + 4];
    vec3 n = wave_normal(world.xz, look.water_pad[0].y, fog_depth * 0.0016 * 6.0 / 6.2832);
    // Seen from low, the swell's slope flattens (it is far and small on the screen).
    n = normalize(mix(vec3(0.0, 1.0, 0.0), n, clamp(to_eye.y * 4.0, 0.0, 1.0)));
    // The lighting pass's: the body's albedo takes (1 - reflectance) of the diffuse light (the
    // ambient from the environment's cube map, the fill light, the sun times its shadow and the
    // light texture), no occlusion (the program writes 1); the highlights of both lights add,
    // at the gloss the program writes (0). The scene stores it at half (`HDR_RANGE_INV`).
    vec3 f0 = vec3(body.w);
    vec3 ambient = look.sh_on.x > 0.5 ? environment_light(n) : look.ambient.rgb;
    vec3 fill = (dot(look.fill_dir.xyz, look.fill_dir.xyz) > 0.0 ? max(dot(n, -normalize(look.fill_dir.xyz)), 0.0) : 0.0) * look.fill_color.rgb;
    vec3 way = normalize(look.sun_dir.xyz);
    float sun_lit = sun_shadow(world) * cloud_light(world);
    vec3 sun = max(dot(n, -way), 0.0) * look.sun_color.rgb;
    vec3 lit = (ambient + fill + sun * sun_lit) * (1.0 - f0) * body.rgb + direct_specular(n, f0, 0.0, sun_lit, 1.0);
    vec3 c = lit * HDR_RANGE_INV;
    // The refraction (the lighting pass's): the way the view ray goes on through the surface
    // (the ray bent by the index less 1 along the way the normal leans off the vertical), as far
    // as the scene is behind the surface (at most 8 transparencies), and the scene's colour
    // where that ends; the light through the water thins as 2^(-depth behind / transparency).
    // STAND-IN: the caustics' light on what is seen through (`caustics.g1t`) and the shore bubbles are left out.
    float seen_through = 0.0;
    vec3 below = vec3(0.0);
    if (textureSize(scene_depth, 0).x > 1) {
        float index = look.water_pad[1 + piece * 5 + 2].w;
        float transparency = look.water_pad[1 + piece * 5 + 3].w;
        vec2 here = gl_FragCoord.xy / vec2(textureSize(refraction_map, 0));
        // The nearest of the depth map's four texels around the pixel (its edge may be a bank).
        vec4 around = textureGather(scene_depth, here, 3);
        float behind = min(min(around.x, around.y), min(around.z, around.w));
        float thickness = max(behind - fog_depth, 0.0);
        vec3 ray = normalize((vec3(0.0, 1.0, 0.0) - n) * index - to_eye);
        vec3 end = world + ray * min(thickness, 8.0 * transparency);
        mat4 view_proj = mat4(look.vertex_only[7], look.vertex_only[8], look.vertex_only[9], look.vertex_only[10]);
        vec4 clip = view_proj * vec4(end, 1.0);
        vec2 there = clamp(clip.xy / clip.w * 0.5 + 0.5, vec2(0.0), vec2(1.0));
        vec4 around_end = textureGather(scene_depth, there, 3);
        float behind_end = min(min(around_end.x, around_end.y), min(around_end.z, around_end.w));
        // Something in front of the surface there (a bank, a plant): the scene where the pixel is.
        if (clip.w <= 0.0 || behind_end < fog_depth) {
            there = here;
            behind_end = behind;
        }
        below = textureLod(refraction_map, there, 0.0).rgb;
        seen_through = min(exp2(-(behind_end - fog_depth) / transparency), 1.0);
        c *= 1.0 - seen_through;
    }
    // The field's fog over the distance, as the models take it (model.vert), the lit part at the
    // scene's half brightness (the post pass doubles it; the sky plane's colour is already there).
    // What is seen through the water has its own fog already.
    float s = look.fog.x * length(look.eye.xyz - world) + look.fog.y;
    vec3 keep = min(exp2(s * look.scat[0].xyz), vec3(1.0));
    float cs = dot(to_eye, way);
    vec3 add = look.scat[3].xyz * pow(max(look.scat[2].w * cs + look.scat[3].w, 0.0001), -1.5) + (1.0 + cs * cs) * look.scat[2].xyz + look.scat[1].xyz;
    colour = vec4(mix(add * HDR_RANGE_INV, c, keep) + below * seen_through, 1.0);
    motion = vec4(0.0);
}
