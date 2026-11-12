#version 450
// FSR 1's edge-adaptive spatial upscale (EASU), the 32-bit path of AMD FidelityFX Super
// Resolution 1.0 (ffx_fsr1.h, FsrEasuF and its helpers), ported to plain GLSL for Barrel:
// draws the output from the scene at a lower resolution. Constants from fsr::easu_constants.
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

layout(set = 0, binding = 0) uniform sampler2D scene;
layout(push_constant) uniform Push {
    vec4 con0;
    vec4 con1;
    vec4 con2;
    vec4 con3;
} pc;
layout(location = 0) out vec4 color;

// The approximations of ffx_a.h (APrxLoRcpF1, APrxLoRsqF1).
float rcpLo(float a) { return uintBitsToFloat(0x7ef07ebbu - floatBitsToUint(a)); }
float rsqLo(float a) { return uintBitsToFloat(0x5f347d74u - (floatBitsToUint(a) >> 1u)); }

void tap(inout vec3 aC, inout float aW, vec2 off, vec2 dir, vec2 len, float lob, float clp, vec3 c) {
    vec2 v = vec2(off.x * dir.x + off.y * dir.y, off.x * (-dir.y) + off.y * dir.x);
    v *= len;
    float d2 = min(v.x * v.x + v.y * v.y, clp);
    float wB = (2.0 / 5.0) * d2 - 1.0;
    float wA = lob * d2 - 1.0;
    wB *= wB;
    wA *= wA;
    wB = (25.0 / 16.0) * wB - (25.0 / 16.0 - 1.0);
    float w = wB * wA;
    aC += c * w;
    aW += w;
}

void set(inout vec2 dir, inout float len, vec2 pp, int corner, float lA, float lB, float lC, float lD, float lE) {
    float w = corner == 0 ? (1.0 - pp.x) * (1.0 - pp.y) : corner == 1 ? pp.x * (1.0 - pp.y) : corner == 2 ? (1.0 - pp.x) * pp.y : pp.x * pp.y;
    float dc = lD - lC;
    float cb = lC - lB;
    float lenX = rcpLo(max(abs(dc), abs(cb)));
    float dirX = lD - lB;
    dir.x += dirX * w;
    lenX = clamp(abs(dirX) * lenX, 0.0, 1.0);
    len += lenX * lenX * w;
    float ec = lE - lC;
    float ca = lC - lA;
    float lenY = rcpLo(max(abs(ec), abs(ca)));
    float dirY = lE - lA;
    dir.y += dirY * w;
    lenY = clamp(abs(dirY) * lenY, 0.0, 1.0);
    len += lenY * lenY * w;
}

void main() {
    vec2 pp = floor(gl_FragCoord.xy) * pc.con0.xy + pc.con0.zw;
    vec2 fp = floor(pp);
    pp -= fp;
    vec2 p0 = fp * pc.con1.xy + pc.con1.zw;
    vec2 p1 = p0 + pc.con2.xy;
    vec2 p2 = p0 + pc.con2.zw;
    vec2 p3 = p0 + pc.con3.xy;
    vec4 bczzR = textureGather(scene, p0, 0), bczzG = textureGather(scene, p0, 1), bczzB = textureGather(scene, p0, 2);
    vec4 ijfeR = textureGather(scene, p1, 0), ijfeG = textureGather(scene, p1, 1), ijfeB = textureGather(scene, p1, 2);
    vec4 klhgR = textureGather(scene, p2, 0), klhgG = textureGather(scene, p2, 1), klhgB = textureGather(scene, p2, 2);
    vec4 zzonR = textureGather(scene, p3, 0), zzonG = textureGather(scene, p3, 1), zzonB = textureGather(scene, p3, 2);
    vec4 bczzL = bczzB * 0.5 + (bczzR * 0.5 + bczzG);
    vec4 ijfeL = ijfeB * 0.5 + (ijfeR * 0.5 + ijfeG);
    vec4 klhgL = klhgB * 0.5 + (klhgR * 0.5 + klhgG);
    vec4 zzonL = zzonB * 0.5 + (zzonR * 0.5 + zzonG);
    float bL = bczzL.x, cL = bczzL.y, iL = ijfeL.x, jL = ijfeL.y, fL = ijfeL.z, eL = ijfeL.w;
    float kL = klhgL.x, lL = klhgL.y, hL = klhgL.z, gL = klhgL.w, oL = zzonL.z, nL = zzonL.w;
    vec2 dir = vec2(0.0);
    float len = 0.0;
    set(dir, len, pp, 0, bL, eL, fL, gL, jL);
    set(dir, len, pp, 1, cL, fL, gL, hL, kL);
    set(dir, len, pp, 2, fL, iL, jL, kL, nL);
    set(dir, len, pp, 3, gL, jL, kL, lL, oL);
    vec2 dir2 = dir * dir;
    float dirR = dir2.x + dir2.y;
    bool zro = dirR < 1.0 / 32768.0;
    dirR = zro ? 1.0 : rsqLo(dirR);
    dir.x = zro ? 1.0 : dir.x;
    dir *= dirR;
    len = len * 0.5;
    len *= len;
    float stretch = (dir.x * dir.x + dir.y * dir.y) * rcpLo(max(abs(dir.x), abs(dir.y)));
    vec2 len2 = vec2(1.0 + (stretch - 1.0) * len, 1.0 - 0.5 * len);
    float lob = 0.5 + ((1.0 / 4.0 - 0.04) - 0.5) * len;
    float clp = rcpLo(lob);
    vec3 f = vec3(ijfeR.z, ijfeG.z, ijfeB.z), g = vec3(klhgR.w, klhgG.w, klhgB.w);
    vec3 j = vec3(ijfeR.y, ijfeG.y, ijfeB.y), k = vec3(klhgR.x, klhgG.x, klhgB.x);
    vec3 min4 = min(min(min(f, g), j), k);
    vec3 max4 = max(max(max(f, g), j), k);
    vec3 aC = vec3(0.0);
    float aW = 0.0;
    tap(aC, aW, vec2(0.0, -1.0) - pp, dir, len2, lob, clp, vec3(bczzR.x, bczzG.x, bczzB.x));
    tap(aC, aW, vec2(1.0, -1.0) - pp, dir, len2, lob, clp, vec3(bczzR.y, bczzG.y, bczzB.y));
    tap(aC, aW, vec2(-1.0, 1.0) - pp, dir, len2, lob, clp, vec3(ijfeR.x, ijfeG.x, ijfeB.x));
    tap(aC, aW, vec2(0.0, 1.0) - pp, dir, len2, lob, clp, j);
    tap(aC, aW, vec2(0.0, 0.0) - pp, dir, len2, lob, clp, f);
    tap(aC, aW, vec2(-1.0, 0.0) - pp, dir, len2, lob, clp, vec3(ijfeR.w, ijfeG.w, ijfeB.w));
    tap(aC, aW, vec2(1.0, 1.0) - pp, dir, len2, lob, clp, k);
    tap(aC, aW, vec2(2.0, 1.0) - pp, dir, len2, lob, clp, vec3(klhgR.y, klhgG.y, klhgB.y));
    tap(aC, aW, vec2(2.0, 0.0) - pp, dir, len2, lob, clp, vec3(klhgR.z, klhgG.z, klhgB.z));
    tap(aC, aW, vec2(1.0, 0.0) - pp, dir, len2, lob, clp, g);
    tap(aC, aW, vec2(1.0, 2.0) - pp, dir, len2, lob, clp, vec3(zzonR.z, zzonG.z, zzonB.z));
    tap(aC, aW, vec2(0.0, 2.0) - pp, dir, len2, lob, clp, vec3(zzonR.w, zzonG.w, zzonB.w));
    color = vec4(min(max4, max(min4, aC / aW)), 1.0);
}
