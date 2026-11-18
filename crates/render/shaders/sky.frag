#version 450
// The sky plane's colour: single scattering along the view ray out through the atmosphere,
// mixed toward the set's haze colour around a direction (`skyplane.g1s` pixel program 2,
// docs/shading.md).
// Look slots 76 on (`sky`): [0] {on}, [1..4] Scat, [5] skyPos, [6] fog {x, y}, [7] the way the
// sun's light travels (render space), [8] the haze's direction, [9] its colour (w: strength),
// [10] hGlowParam.
layout(std140, set = 1, binding = 4) uniform Look {
    vec4 head[4];
    vec4 eye;
    vec4 middle[39];
    mat4 view_proj;
    vec4 tail[28];
    vec4 sky[11];
} look;
layout(location = 0) in vec3 ray;
layout(location = 0) out vec4 colour;
layout(location = 1) out vec4 motion;
void main() {
    vec3 d = normalize(ray);
    vec4 sp = look.sky[5];
    float l = abs(-d.y * sp.x - sqrt(max(d.y * d.y * sp.y + sp.z, 0.0))) * look.sky[6].x + look.sky[6].y;
    vec3 t = vec3(1.0) - min(exp2(l * look.sky[1].xyz), vec3(1.0));
    float c = dot(d, -look.sky[7].xyz);
    vec3 s = (1.0 + c * c) * look.sky[3].xyz + pow(max(c * look.sky[3].w + look.sky[4].w, 0.0001), -1.5) * look.sky[4].xyz + look.sky[2].xyz;
    vec3 in_scattered = s * t;
    float x = clamp(dot(d, look.sky[8].xyz) * look.sky[10].x + look.sky[10].y, 0.0, 1.0);
    float k = (1.0 - exp(-x * x)) * clamp(look.sky[9].w, 0.0, 1.0);
    colour = vec4(mix(in_scattered, look.sky[9].rgb, k), 1.0);
    // The far plane does not move with the scene: no motion of its own.
    motion = vec4(0.0);
}
