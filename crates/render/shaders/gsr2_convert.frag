#version 450
// Snapdragon Game Super Resolution 2, its convert pass (sgsr2_convert.fs, 2-pass fragment
// variant), ported to Vulkan GLSL for Barrel: per render pixel, the motion (in NDC, current
// minus last) and how far the pixel's depth stands out from its neighbourhood (the depth
// clip: 0 on flat surfaces, towards 1 at depth edges). Barrel's motion vectors already hold
// the camera's and the objects' movement (texture units, half of NDC), so they are read
// directly where the original reprojects static pixels from depth.
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

precision highp float;
precision highp int;

#include "gsr2_common.glsl"

layout(set = 0, binding = 0) uniform highp sampler2D depth;
layout(set = 0, binding = 1) uniform highp sampler2D motion;
layout(location = 0) in highp vec2 uv;
layout(location = 0) out vec4 motion_depth_clip;

void main() {
    ivec2 pos = ivec2(uv * pc.render_size);
    vec2 gather = uv - vec2(0.5) * pc.render_size_rcp;
    // Gathers find the nearest depth (a b c d / e f g h / i j k l / m n o p).
    vec4 btm_left = textureGather(depth, gather, 0);
    vec4 btm_right = textureGather(depth, gather + vec2(pc.render_size_rcp.x * 2.0, 0.0), 0);
    vec4 top_left = textureGather(depth, gather + vec2(0.0, pc.render_size_rcp.y * 2.0), 0);
    vec4 top_right = textureGather(depth, gather + pc.render_size_rcp * 2.0, 0);
    float max_c = min(min(min(btm_left.z, btm_right.w), top_left.y), top_right.x);
    float depth_clip = 0.0;
    if (max_c < 1.0 - 1.0e-05) {
        float btm_left4 = min(min(min(btm_left.y, btm_left.x), btm_left.z), btm_left.w);
        float btm_right4 = min(min(min(btm_right.y, btm_right.x), btm_right.z), btm_right.w);
        float top_left4 = min(min(min(top_left.y, top_left.x), top_left.z), top_left.w);
        float top_right4 = min(min(min(top_right.y, top_right.x), top_right.z), top_right.w);
        float sep = 1.37e-05 * pc.fov_hor * length(pc.render_size) * (1.0 - max_c);
        const float EPSILON = 1.19e-07;
        float w = clamp(sep / (abs(max_c - btm_left4) + EPSILON), 0.0, 1.0)
            + clamp(sep / (abs(max_c - btm_right4) + EPSILON), 0.0, 1.0)
            + clamp(sep / (abs(max_c - top_left4) + EPSILON), 0.0, 1.0)
            + clamp(sep / (abs(max_c - top_right4) + EPSILON), 0.0, 1.0);
        depth_clip = clamp(1.0 - w * 0.25, 0.0, 1.0);
    }
    vec2 m = texelFetch(motion, pos, 0).xy * 2.0;
    motion_depth_clip = vec4(m, depth_clip, 0.0);
}
