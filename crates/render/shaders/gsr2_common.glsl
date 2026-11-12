// The push constants both GSR 2 passes share (`gsr2.rs`, Gsr2Push).
layout(push_constant) uniform Push {
    highp vec2 render_size;
    highp vec2 output_size;
    highp vec2 render_size_rcp;
    highp vec2 output_size_rcp;
    // This frame's jitter in render pixels.
    highp vec2 jitter;
    // x: the kernel's widest bias (output / render width, at most 1.99); y: the history box's
    // scale (min(20, (area ratio)^3)).
    highp vec2 scale_ratio;
    // tan(horizontal field of view / 2).
    highp float fov_hor;
    highp float min_lerp_contribution;
    highp float reset;
    highp float pad;
} pc;
