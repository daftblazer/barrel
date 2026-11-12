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

## Which texture is which

Neither the model nor the tables name a material's shader (no shader names in the g1m, none
in `CharaBase.xml`); the executable picks it, not read yet. Escha's body material (`PC20_MODEL`
material 0, `barrel g1m-materials FILE#N`) lists textures #0 (2048², subtype 1: the colour,
with a cut-out alpha), #2 (1024×512, subtype 1: laid out for other UVs, presumably the second
set) and #1 (1024², subtype 0: grey, the shape of the colour texture's islands: the mask that
`CharaMTC`'s pixel program samples as `t1`). Taken for now: the colour is the material's
first texture, the mask its first subtype-0 texture after it (none: full shading).

## In the renderer

`model.frag` shades characters (draws that belong to no field) with steps 1 to 4 and 7: the
toon step toward the field's character light, the light colour's soft-light blend, the mask
(set 2; white when the material has none), the rim light and its screen blend. The field's
`<chara_shader>` values go to the fragment shader in a uniform block (set 1, binding 4) each
frame, the directions mirrored into render space, with the camera's eye for the view
direction; a field without them gets a light from above and in front and no colour or rim.
Step 5's point lights are summed per vertex as the game's vertex program does (below), from
the field's first four `POINT` lights. Not done yet: step 6 (fog), `atColor` (taken as white)
and the HDR range (1); fields' own shaders (`MapSTD*`).

## The character vertex program (`CharaMTC.g1s` program 0)

Point lights: for each of four, with d the vector from the vertex to `litPos[i]`, the light
adds `(litAmb[i] + litCol[nDirLit + i]) × (1 - saturate(d² / att.w)) / (att.x + att.y·|d| +
att.z·d²)` (`litAtt[i]`; a range or divisor near zero drops it), and the sum is saturated:
the pixel program's v2. The directional lights (`litCol[0 .. nDirLit]`) do not light
characters. The renderer takes d in metres and `att.w` as the range squared (the field gives
`range` in metres; the game's own conversion is not read).

Fog and scattering: with e the vector from the vertex to the eye (`vEye`), the extinction is
`min(exp((fog.x·|e| + fog.y) × Scat[0].xyz), 1)` (v4) and the in-scattered colour, with θ the
angle between the view direction and `litDir[0]`, is `Scat[3].xyz × (Scat[2].w·cos θ +
Scat[3].w)^-1.5 + (1 + cos² θ) × Scat[2].xyz + Scat[1].xyz` (v3): Mie and Rayleigh terms.
How `fog` and `Scat` come from the field's `<fog>` is not read yet. The field loader
(`0x140068566` on) reads `start` and `end` (both scaled, to field units), `ambient`, `mieG`,
`rayleighPow`, `mieRGB` and `rayleighRGB` and stores them by field (`0x1404d6df0`, a map entry:
+0x20 start, +0x24 end, +0x28 ambient, +0x2c mieG, +0x30 rayleighPow, +0x38 mieRGB, +0x48
rayleighRGB); the shader constants are filled through the engine's parameter-name table
(`Scat` at `0x140953378`), whose code is not found yet. The pixel program's form is
Henyey-Greenstein for Mie (`Scat[2].w` and `Scat[3].w` presumably -2g and 1 + g²) and
`1 + cos² θ` for Rayleigh.

## Fields (`MapSTD.g1s`: 36 vertex programs, pixel program 36)

**Vertex program 0** (inputs: position, normal, vertex colour, one UV set; `$Globals`:
`litDir[2]`, `litPos[4]`, `litCol`, `litAmb`, `litAtt`, `ambLit`, `vEye`, `cDiff`, `cAmb`,
`cSpec`, the matrices, `fog`, `Scat`, and wind for swaying plants). Lighting is per vertex,
Blinn-Phong with the specular power in `cSpec.w`:

- the first directional light (`litDir[0]`, `litCol[0]`): diffuse `max(n·-L, 0)` and specular
  `(n·h)^power`, kept apart (the pixel program shadows and lightmaps them);
- the second directional light and the point lights (each within its range, attenuated by
  `1 / (att.x + att.y·d + att.z·d²)`): diffuse and specular summed;
- the ambient: `ambLit` plus each point light's `litAmb` times its attenuation.

With `d = vertex colour × cDiff`: v6 = d × the first light's diffuse; v7 = its specular ×
`cSpec`; v1 = ambient × vertex colour × `cAmb` + d × the other diffuse (alpha d.a); v8 = the
other specular × `cSpec`. v3.xy is the world position through a projection (`mW2Pt` rows,
the light texture's coordinates over the field), v3.zw the texture coordinates, v2 the shadow
map's (`mW2S`), v4/v5 the fog colour and extinction as for characters.

**Pixel program 36** (`sStage0` the colour, `sShadow` a shadow map, `sLit` the light texture;
`nStageNum`, `vATest`, `scSM`, `scSM2`, `atColor`, `HdrRangeInv`): `lit` = `sLit` at v3.xy;
`tex` = the colour at v3.zw (white when `nStageNum` is 0), alpha-tested; `s` = a 4×4
filtered shadow-map lookup (16 taps averaged, then scaled by `max((atColor.x - 0.7) × 3.33,
0)`); colour = `(v1 + s·lit·v6) × tex + (v8 + s·lit·v7)`; then the fog, `atColor` and the HDR
range as for characters.

So fields have a shadowed, light-textured "sun" (the first directional light) and an
unshadowed fill (the second), point lights and ambient, all on the vertex colours. Still to
pin down before the renderer can follow it: the `rot` convention of the field's `PARALLEL`
lights, the fields' vertex colours (not read from the g1m yet), `cDiff`/`cAmb`/`cSpec` (the
material's, presumably), and where the light texture and its projection come from.
