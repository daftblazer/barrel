#version 450
// Snapdragon Game Super Resolution 2, its upscale pass (sgsr2_upscale.fs, 2-pass fragment
// variant), ported to Vulkan GLSL for Barrel: per output pixel, a 5-tap Lanczos-like upsample of
// this frame's jittered render, the history reprojected by the motion and clamped to the
// samples' box (narrowed by their variance), then blended by how much the samples cover the
// pixel. The original's four corner taps sit behind `if (false)` and are left out; the taps'
// pixel positions are clamped to the render (an out-of-range texelFetch is undefined in Vulkan).
//
// Copyright (c) 2024, Qualcomm Innovation Center, Inc. All rights reserved.
//
// Redistribution and use in source and binary forms, with or without
// modification, are permitted provided that the following conditions are met:
//
// 1. Redistributions of source code must retain the above copyright notice,
//    this list of conditions and the following disclaimer.
//
// 2. Redistributions in binary form must reproduce the above copyright notice,
//    this list of conditions and the following disclaimer in the documentation
//    and/or other materials provided with the distribution.
//
// 3. Neither the name of the copyright holder nor the names of its contributors
//    may be used to endorse or promote products derived from this software
//    without specific prior written permission.
//
// THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
// AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
// IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE
// ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE
// LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR
// CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF
// SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS
// INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN
// CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
// ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE
// POSSIBILITY OF SUCH DAMAGE.
//
// SPDX-License-Identifier: BSD-3-Clause

precision mediump float;
precision highp int;

#include "gsr2_common.glsl"

layout(set = 0, binding = 0) uniform mediump sampler2D history;
layout(set = 0, binding = 1) uniform mediump sampler2D motion_depth_clip;
layout(set = 0, binding = 2) uniform mediump sampler2D scene;
layout(location = 0) in highp vec2 uv;
layout(location = 0) out mediump vec4 color;

float fast_lanczos(float base) {
    float y = base - 1.0;
    float y2 = y * y;
    return (0.75 * y + y2) * y2;
}

// Running sums over the taps.
vec4 upsampled = vec4(0.0);
vec3 box_center = vec3(0.0), box_var = vec3(0.0), box_min = vec3(1e9), box_max = vec3(-1e9);
float box_weight = 0.0;

void tap(ivec2 at, vec2 offset, float bias2, float curve) {
    vec3 c = texelFetch(scene, clamp(at, ivec2(0), ivec2(pc.render_size) - 1), 0).rgb;
    float d = dot(offset, offset);
    float weight = fast_lanczos(clamp(d * bias2, 0.0, 1.0));
    upsampled += vec4(c * weight, weight);
    float bw = exp(d * curve);
    box_min = min(box_min, c);
    box_max = max(box_max, c);
    vec3 wc = c * bw;
    box_center += wc;
    box_var += c * wc;
    box_weight += bw;
}

void main() {
    float bias_max = pc.scale_ratio.x;
    float scale_factor = pc.scale_ratio.y;
    highp vec2 jitter_uv = clamp(uv + pc.jitter * pc.render_size_rcp, 0.0, 1.0);
    highp ivec2 pos = ivec2(jitter_uv * pc.render_size);
    highp vec3 mda = textureLod(motion_depth_clip, jitter_uv, 0.0).xyz;
    highp vec2 m = mda.xy;
    highp vec2 prev_uv = clamp(uv - 0.5 * m, 0.0, 1.0);
    float depth_factor = mda.z;
    vec3 history_color = textureLod(history, prev_uv, 0.0).rgb;

    float bias_min = max(1.0, 0.3 + 0.3 * bias_max);
    float kernel_bias = mix(bias_max, bias_min, 0.25 * depth_factor) * 0.5;
    float motion_len = length(m * pc.output_size);
    float curve = mix(-2.0, -3.0, clamp(motion_len * 0.02, 0.0, 1.0));
    highp vec2 src = vec2(pos) + vec2(0.5) - pc.jitter - uv * pc.render_size;
    float bias2 = kernel_bias * kernel_bias;
    tap(pos + ivec2(0, 1), src + vec2(0.0, 1.0), bias2, curve);
    tap(pos + ivec2(1, 0), src + vec2(1.0, 0.0), bias2, curve);
    tap(pos + ivec2(-1, 0), src + vec2(-1.0, 0.0), bias2, curve);
    tap(pos, src, bias2, curve);
    tap(pos + ivec2(0, -1), src + vec2(0.0, -1.0), bias2, curve);

    float rw = 1.0 / box_weight;
    box_center *= rw;
    box_var = sqrt(abs(box_var * rw - box_center * box_center));
    upsampled.rgb = clamp(upsampled.rgb / upsampled.w, box_min - vec3(0.075), box_max + vec3(0.075));
    upsampled.w *= 1.0 / 3.0;

    float update = 1.0 - depth_factor;
    update = min(update, mix(update, upsampled.w * 10.0, clamp(10.0 * motion_len, 0.0, 1.0)));
    update = min(update, mix(update, upsampled.w, clamp(motion_len * 0.05, 0.0, 1.0)));
    float base_alpha = update;
    float box_scale = max(depth_factor, clamp(motion_len * 0.05, 0.0, 1.0));
    vec3 s = box_var * mix(scale_factor, 1.0, box_scale);
    vec3 hi = min(box_max, box_center + s);
    vec3 lo = max(box_min, box_center - s);
    vec3 clamped = clamp(history_color, lo, hi);
    float start_lerp = (abs(m.x) + abs(m.y)) > 0.000001 ? 0.0 : pc.min_lerp_contribution;
    float lerp_contribution = (any(greaterThan(lo, history_color)) || any(greaterThan(history_color, hi))) ? start_lerp : 1.0;
    history_color = mix(clamped, history_color, clamp(lerp_contribution, 0.0, 1.0));
    base_alpha = mix(min(base_alpha, 0.1), base_alpha, clamp(lerp_contribution, 0.0, 1.0));
    float alpha_sum = max(1.192e-07, base_alpha + upsampled.w);
    float alpha = clamp(upsampled.w / alpha_sum + pc.reset, 0.0, 1.0);
    color = vec4(mix(history_color, upsampled.rgb, alpha), 1.0);
}
