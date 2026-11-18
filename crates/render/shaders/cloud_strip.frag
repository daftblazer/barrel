#version 450
// The sky's large clouds, shaded (docs/shading.md, "Ryza DX: the sky, the clouds"): the
// cloud map's value is the cloud's opacity here; how much cloud lies between the point and
// the light is marched along the light's way through the map (15 steps; each takes the map's
// value there less its height up the march, to a sixteenth), and lights the point through
// Beer's law. The colour is fogged with the sky's scattering. The game's height fog layers are
// not drawn (the set's are off at noon).
// The look block as cloud_strip.vert has it.
layout(std140, set = 1, binding = 4) uniform Look {
    vec4 head[4];
    vec4 eye;
    vec4 middle[39];
    mat4 view_proj;
    vec4 tail[28];
    vec4 sky[11];
    vec4 other[10];
    vec4 cloud[12];
} look;
layout(set = 0, binding = 0) uniform sampler2D cloud_map;
layout(location = 0) in vec3 world;
layout(location = 1) in vec2 map_uv;
layout(location = 2) in vec4 tint;
layout(location = 3) in vec2 fog_height;
layout(location = 0) out vec4 colour;
layout(location = 1) out vec4 motion;

const int STEPS = 16;
// The scene is stored at half (`HDR_RANGE_INV`, as water.frag and the models have it).
const float HDR_RANGE_INV = 0.5;

void main() {
    vec3 to_eye = normalize(look.eye.xyz - world);
    // Which layer's channel this point's edge fade picks (tents a quarter wide).
    vec4 pick = max(1.0 - 4.0 * abs(vec4(tint.w) - vec4(0.25, 0.5, 0.75, 1.0)), 0.0);
    float alpha = dot(pick, texture(cloud_map, map_uv));

    // The light's way down through the layer, in the map's coordinates (the game's x is the
    // render space's -x).
    vec3 way = look.cloud[0].xyz;
    vec3 down = way * look.cloud[6].x / min(way.y, -0.0001);
    vec2 step_uv = vec2(-down.x, down.z) * look.cloud[7].xy;
    float len = length(step_uv);
    step_uv *= len > 0.0 ? min(len, look.cloud[6].y) / len : 0.0;
    step_uv /= float(STEPS - 1);
    float depth = min(alpha, 1.0 / float(STEPS));
    for (int i = 1; i < STEPS; i++) {
        float here = dot(pick, texture(cloud_map, map_uv + step_uv * float(i)));
        depth += clamp(here - float(i) / float(STEPS), 0.0, 1.0 / float(STEPS));
    }
    // 2^-(path x depth x extinction): the game's exp is base 2.
    float lit = exp2(-length(down) * depth * look.cloud[6].z);
    vec3 light = (look.cloud[1].rgb * lit + look.cloud[2].rgb) * tint.rgb;

    // The sky's scattering between the point and the eye, the cloud's own in-scattering taking
    // the place of the light the path takes away.
    vec3 transmit = min(exp2(look.cloud[8].xyz * exp2(-fog_height.y * 1.442695) * fog_height.x), vec3(1.0));
    float c = dot(to_eye, way);
    float mie = pow(max(c * look.cloud[10].w + look.cloud[11].w, 0.0001), -1.5);
    vec3 scatter = ((1.0 + c * c) * look.cloud[10].xyz + look.cloud[11].xyz * mie + look.cloud[9].xyz) * (lit * look.cloud[6].w);
    colour = vec4((transmit * (light - scatter) + scatter) * HDR_RANGE_INV, alpha);
    motion = vec4(0.0);
}
