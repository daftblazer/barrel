#version 450
// FSR 1's robust contrast-adaptive sharpening (RCAS), the 32-bit path of AMD FidelityFX Super
// Resolution 1.0 (ffx_fsr1.h, FsrRcasF, without the optional denoise), ported to plain GLSL for
// Barrel: sharpens the upscaled image. `sharpness` from fsr::rcas_sharpness.
//
// Copyright (c) 2021 Advanced Micro Devices, Inc. All rights reserved.
// 
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
// 
// The above copyright notice and this permission notice shall be included in
// all copies or substantial portions of the Software.
// 
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT.  IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN
// THE SOFTWARE.

layout(set = 0, binding = 0) uniform sampler2D upscaled;
layout(push_constant) uniform Push {
    float sharpness;
} pc;
layout(location = 0) out vec4 color;

const float LIMIT = 0.25 - 1.0 / 16.0;

// ffx_a.h's APrxMedRcpF1.
float rcpMed(float a) {
    float b = uintBitsToFloat(0x7ef19fffu - floatBitsToUint(a));
    return b * (-b * a + 2.0);
}

void main() {
    ivec2 sp = ivec2(gl_FragCoord.xy);
    ivec2 last = textureSize(upscaled, 0) - 1;
    vec3 b = texelFetch(upscaled, clamp(sp + ivec2(0, -1), ivec2(0), last), 0).rgb;
    vec3 d = texelFetch(upscaled, clamp(sp + ivec2(-1, 0), ivec2(0), last), 0).rgb;
    vec3 e = texelFetch(upscaled, sp, 0).rgb;
    vec3 f = texelFetch(upscaled, clamp(sp + ivec2(1, 0), ivec2(0), last), 0).rgb;
    vec3 h = texelFetch(upscaled, clamp(sp + ivec2(0, 1), ivec2(0), last), 0).rgb;
    vec3 mn4 = min(min(min(b, d), f), h);
    vec3 mx4 = max(max(max(b, d), f), h);
    vec2 peakC = vec2(1.0, -4.0);
    vec3 hitMin = min(mn4, e) / (4.0 * mx4);
    vec3 hitMax = (peakC.x - max(mx4, e)) / (4.0 * mn4 + peakC.y);
    vec3 lobeRGB = max(-hitMin, hitMax);
    float lobe = max(-LIMIT, min(max(max(lobeRGB.r, lobeRGB.g), lobeRGB.b), 0.0)) * pc.sharpness;
    float rcpL = rcpMed(4.0 * lobe + 1.0);
    color = vec4((lobe * (b + d + h + f) + e) * rcpL, 1.0);
}
