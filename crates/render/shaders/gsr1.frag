#version 450
// Snapdragon Game Super Resolution 1 (sgsr1_shader_mobile.frag, RGBA mode), ported to Vulkan
// GLSL for Barrel: one pass that scales the scene to the output and sharpens its edges (a
// 12-tap Lanczos-like filter on green as the luma, applied only where the input has an edge).
// The viewport vector is a push constant: 1/width, 1/height, width, height of the input.
//
// Copyright (c) 2023, Qualcomm Innovation Center, Inc. All rights reserved.
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

const float EDGE_THRESHOLD = 8.0 / 255.0;
const float EDGE_SHARPNESS = 2.0;

layout(set = 0, binding = 0) uniform mediump sampler2D scene;
layout(push_constant) uniform Push { highp vec4 viewport; } pc;
layout(location = 0) in highp vec2 uv;
layout(location = 0) out vec4 color;

float fast_lanczos2(float x) {
    float wa = x - 4.0;
    float wb = x * wa - wa;
    wa *= wa;
    return wb * wa;
}

vec2 weight_y(float dx, float dy, float c, float std_dev) {
    float x = (dx * dx + dy * dy) * 0.55 + clamp(abs(c) * std_dev, 0.0, 1.0);
    float w = fast_lanczos2(x);
    return vec2(w, w * c);
}

void main() {
    vec4 c = vec4(textureLod(scene, uv, 0.0).rgb, 0.0);
    highp vec2 img = uv * pc.viewport.zw + vec2(-0.5, 0.5);
    highp vec2 pixel = floor(img);
    highp vec2 coord = pixel * pc.viewport.xy;
    vec2 pl = img - pixel;
    vec4 left = textureGather(scene, coord, 1);
    float edge_vote = abs(left.z - left.y) + abs(c.g - left.y) + abs(c.g - left.z);
    if (edge_vote > EDGE_THRESHOLD) {
        coord.x += pc.viewport.x;
        vec4 right = textureGather(scene, coord + vec2(pc.viewport.x, 0.0), 1);
        vec4 up_down;
        up_down.xy = textureGather(scene, coord + vec2(0.0, -pc.viewport.y), 1).wz;
        up_down.zw = textureGather(scene, coord + vec2(0.0, pc.viewport.y), 1).yx;
        float mean = (left.y + left.z + right.x + right.w) * 0.25;
        left -= vec4(mean);
        right -= vec4(mean);
        up_down -= vec4(mean);
        c.w = c.g - mean;
        float sum = abs(left.x) + abs(left.y) + abs(left.z) + abs(left.w)
            + abs(right.x) + abs(right.y) + abs(right.z) + abs(right.w)
            + abs(up_down.x) + abs(up_down.y) + abs(up_down.z) + abs(up_down.w);
        float std_dev = 2.181818 / sum;
        vec2 a = weight_y(pl.x, pl.y + 1.0, up_down.x, std_dev);
        a += weight_y(pl.x - 1.0, pl.y + 1.0, up_down.y, std_dev);
        a += weight_y(pl.x - 1.0, pl.y - 2.0, up_down.z, std_dev);
        a += weight_y(pl.x, pl.y - 2.0, up_down.w, std_dev);
        a += weight_y(pl.x + 1.0, pl.y - 1.0, left.x, std_dev);
        a += weight_y(pl.x, pl.y - 1.0, left.y, std_dev);
        a += weight_y(pl.x, pl.y, left.z, std_dev);
        a += weight_y(pl.x + 1.0, pl.y, left.w, std_dev);
        a += weight_y(pl.x - 1.0, pl.y - 1.0, right.x, std_dev);
        a += weight_y(pl.x - 2.0, pl.y - 1.0, right.y, std_dev);
        a += weight_y(pl.x - 2.0, pl.y, right.z, std_dev);
        a += weight_y(pl.x - 1.0, pl.y, right.w, std_dev);
        float y = a.y / a.x;
        float hi = max(max(left.y, left.z), max(right.x, right.w));
        float lo = min(min(left.y, left.z), min(right.x, right.w));
        y = clamp(EDGE_SHARPNESS * y, lo, hi);
        // Smooths high-contrast input.
        float dy = clamp(y - c.w, -23.0 / 255.0, 23.0 / 255.0);
        c.rgb = clamp(c.rgb + dy, 0.0, 1.0);
    }
    color = vec4(c.rgb, 1.0);
}
