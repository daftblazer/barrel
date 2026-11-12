# How the game shades

Read from the game's own shader programs (`Data/x64/Shader/*.g1s`, Direct3D 11 bytecode;
`barrel g1s FILE N` prints program N's code with `barrel_formats::dxbc::disassemble`). The
engine's shaders are to do the same work, written anew.

## Characters (`CharaMTC.g1s`, pixel program 12)

Inputs from the vertex program (program 0 for the plain variant): the texture coordinates
(v1), the scene lights' sum (v2: `litCol`/`litAmb`/`litDir`/`litPos`/`litAtt` over `nDirLit`
lights, saturated), the fog or scattering colour and its weight (v3, v4; `fog`, `Scat`), the
directions of the rim light and the character light, negated (v5 = -`atCharaRimLightDir`, v6
= -`atCharaLightDir`), the view direction (v7) and the world normal (v8, through `mL2W`).
Constants (`$Globals`): `atColor` (cb0[0]), `HdrRangeInv` (cb0[1].x),
`atCharaPointLightStrength` (cb0[2].x), `atCharaLightColor` (cb0[3]: colour, weight in w),
`atCharaRimLightColor` (cb0[4]). Textures: the diffuse (t0) and a mask (t1).

The steps, with n the normalised normal and c the diffuse texture's colour:

1. **Toon step.** h = saturate((n·L + 1) / 2), with L = v6; t = saturate((h - 0.546875) ×
   10.6667), a hard edge 0.09375 wide. The shadow colour is max(c², c/2); the shaded colour is
   lerp(shadow, c, t).
2. **Light colour.** The light colour divided by its largest channel (at least 1) is laid over
   the shaded colour with a soft-light blend (the four-term formula of `round_ni(k + 0.5)`
   picking between two branches), weighted by its w.
3. **Mask.** The mask's red channel (t1 sampled `.yzwx`, the result's w) blends between c (0)
   and the lit result (1).
4. **Rim light.** r = (1 - |n·v|) × saturate((n·R + 1) / 2) with R = v5 and v the view
   direction; rim = saturate((r - 0.3) × 32) × 0.8, times c, coloured by the rim colour the
   same way as step 2, and added with a screen blend: 1 - (1 - a)(1 - b).
5. **Scene lights.** v2 is laid over with an overlay blend (`round_ne` picks the branch),
   weighted by its largest channel times `atCharaPointLightStrength`.
6. **Fog.** lerp(v3, colour, v4).
7. **Colour and range.** result × saturate(atColor) + saturate(atColor - 1), then × the HDR
   range's inverse; alpha from the same.

## Where the constants come from

Each field's table (`Saves/field/<FIELD>.xml`; 114 of the 144 have one) gives its look
(`barrel_formats::field_look`): `<chara_shader light_dir light_color rim_light_dir
rim_light_color point_light_strength>` is the character shader's constants (the colours'
fourth value is their weight: `TOWN_11` has light colour (0.65, 0.4, 0.3) at 0.3 and rim
colour (0.9, 0.9, 0) at 0.25); `<light ambient>` with `PARALLEL` (`rot` in degrees, `color`)
and `POINT` (`pos` in metres, `color`, `ambient`, `att0..2`, `range`) params is the scene's
lights; `<fog start end ...>` (metres; with Mie and Rayleigh scattering terms) and `<hdr
threshold glow_add_rate>` the fog and glow.
