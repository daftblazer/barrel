// Included by the fragment shaders after they declare `look` (with `sh`).

// The field's light from its cube map on normal n (as model.vert's `environment_light`).
vec3 environment_light(vec3 n) {
    vec3 g = normalize(vec3(-n.x, n.y, n.z));
    float b[9] = float[9](0.282095, 0.488603 * g.y, 0.488603 * g.z, 0.488603 * g.x, 1.092548 * g.x * g.y,
        1.092548 * g.y * g.z, 0.315392 * (3.0 * g.z * g.z - 1.0), 1.092548 * g.x * g.z, 0.546274 * (g.x * g.x - g.y * g.y));
    vec3 c = vec3(0.0);
    for (int k = 0; k < 9; k++) {
        c += look.sh[k].rgb * b[k];
    }
    return max(c, vec3(0.0));
}
