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
the pixel program's v2.

**`litAtt` from the field's `POINT`** (the field loader, `0x140068cc4` on): `att.x` is `att0`
and `att.y` is `att1` as written, `att.w` is the range scaled by 100 (field units) and squared,
but the table's `att2` is not `att.z`. It is how much of the light is left at the range
(`fall`, plus 1.19e-7): `att.z = (1 - fall·att0) / (fall·R²)`, R the range in field units,
so that 1 / (att0 + att.z·R²) = `fall`. Missing values: `att0` 1, `att1` 0.001, `range` 1,
`att2` 0.1. `q54` makes its lights the same way (`field_look::point_attenuation`). Taking
`att2` as `att.z` had made every field point light all but vanish (TOWN_03's `att2` 0.9 over
d² in centimetres); with it, Colland's office and Escha's workshop come up toward the game's
(the workshop above the message window 49, 44, 31 before, 61, 55, 38 now, the game 77, 68,
43), nothing blown out. The directional lights (`litCol[0 .. nDirLit]`) do not light
characters. The renderer takes d in field units (centimetres) and `att.w` as the range, scaled to field
units, squared: with d in metres the town's street lamps (`TOWN_10`: ambient 4, `att0` 0.27)
would light the whole street several times over, in centimetres they glow near the lamp.

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

### The field constants as the game sets them

Read live in `TOWN_02` (the R&D room, end of `EVENT_MM01_OP1_131`): the engine keeps each
draw's shader parameters as a stream of `{id, values}` entries (the ids small integers, not
names), found by the field's `<light ambient>` (0.4, 0.45, 0.3, 0.6). In order: `litCol[0]`
(0.5, 0.3, 0.3, 1: the `PARALLEL` colour), the matrices, `cDiff` (1, 1, 1, 1), `cAmb` (1, 1,
1), `cSpec` per material ((0.3, 0.3, 0.3) power 4 in one, (0, 0, 0) power 25.5 or 10 in
others), `ambLit` (the `<light ambient>` itself, w included), `fog` (1/6800, -1200/6800),
`vEye` (675.3, 110.9, 551), `Scat0` to `Scat3` (as "Fog" gives them) and `scSM` (1/1024,
1/1024, 1024, 1024: the shadow map's texel and size). So the material colours are white: they
do not darken the outdoor walls, whose ambient stays unexplained; the specular term (per
material, not drawn yet) only adds light.

### Fields in the renderer

Field models now carry their vertex colours (`g1m` colour attribute, layer 0; the workshop
floor averages (0.35, 0.39, 0.29)) and are lit per vertex as `MapSTD`'s vertex program does:
ambient (the field's `<light ambient>`, plus each point light's ambient times its
attenuation) on the vertex colour, and the vertex colour times the fill's and the point
lights' diffuse and the sun's diffuse; the pixel shader multiplies that by the texture. The field models' own parameter sets (106 looked at in `TOWN_10`, `TOWN_04` and
`AREA_01_01`) hold only `atColor` (white), the wind's (`g_f3Wind`, `g_nWindFlag`,
`g_fWindTime`, `g_fInvHeight`, `g_f3Shift`, `g_fRootHeight`: plants sway) and, twice,
`emissiveColor`; `cDiff`, `cAmb` and `cSpec` are the engine's. Taken
for now: the material colours white no light texture or shadow (1), the
`PARALLEL` `rot` as pitch below level then yaw in degrees, and point light distances in
metres. Point light distances are in field units, as for characters. Checked again against
TOWN_03 (Colland's office, event `EVENT_MM01_OP1_222`): taking metres there makes the desk
lamp glow as in the game but blows Escha's workshop out to white (101, 65, 67 become 255,
242, 254), so field units stand. The quadratic term was the error (below: the table's `att2`
is the light left at the range). With it the desk lamp in `EVENT_MM01_OP1_222`'s line 10
(82.3 s on, the game's shot `logy_c/12`) is as the game's: shade 255, 255, 255 (game the
same, 129, 147, 111 before), brass base 226, 218, 96 (game 237, 220, 58).

**The light texture is the field's `<AREA>_pat.g1t`.** Outdoor fields carry a real one
(`TOWN_10_pat.g1t`: one 512 × 512 BC1 texture, grey, mean 0.61, black to white: drifting cloud
shadows), indoor ones a 64-byte stub (`TOWN_03`, `TOWN_07`, `TOWN_11`). The pixel program
multiplies it into the sun's part only (`s · lit · v6`), at the world position through
`mW2Pt` (its scale and scroll not read yet), so it dapples the sunlit ground; it cannot
explain shade where the sun's part is small (the TOWN_10 walls, mostly ambient).

### Wind (plants swaying)

`MapSTD`'s vertex programs bend a field model's vertices (`p` the world position;
`g_f3Wind` cb0[45].xyz, `g_nWindFlag` .w, `g_fWindTime` cb0[46].x, `g_fInvHeight` .y,
`g_f3Shift` cb0[47].xyz, `g_fRootHeight` .w; the model's world y is `mL2W`'s row 1 w):

```text
phase  = p.x · shift.x + time + p.z · shift.z
height = saturate((p.y - model y - root height) · invHeight)
p'     = p + wind · sin(phase) · height          (only when the flag is set)
```

The models' own values are placeholders (flag 0, wind and shift (1, 1, 1), invHeight 1, root
0: 67 submeshes in `AREA_01_01_Obj`), so the engine sets them from the field table's
`<wind_root>` (23 fields): `<wind dir power freq shift>` (global winds), `<v_shaker keyword
weight root_height height shake_time shift_x shift_z>` (vertex sway for the meshes whose
names hold the keyword, `xV1_` ...) and `<n_shaker keyword weight axis max_angle axis_power
char_range char_power>` (node sway: a spring per bone, `0x1402859xx`). The `v_shaker` parser
(`0x140285125` on) keeps height ×100 (+0x20) and root height ×100 (+0x24, field units), shake
time (+0x28, clamped), shift x and z (+0x2c, +0x30) and weight (+0x34); the `<wind>` parser
reads `freq` at `0x140285327`. How weight, the winds and the time become `g_f3Wind` and
`g_fWindTime` is not read yet, so the renderer does not sway plants.

### Fog (read from the running game)

The constants a field's `<fog start end ambient mieG rayleighPow mieRGB rayleighRGB>` becomes,
read from the game's memory in an oracle run (`TOWN_11`: copies of the per-draw constant
buffer found by their `-2g`, `1 + g²` pair; start 19, end 50, ambient 1, mieG 0.5,
rayleighPow 20, mieRGB (0, 0.2, 0), rayleighRGB (0.1, 0.1, 0.7), the first `PARALLEL`
colour (0.7, 0.5, 0.4), the `<light ambient>` (0.45, 0.5, 0.3)), with βR = rayleighRGB,
βM = mieRGB, and E the first directional light's colour:

```text
fog   = (1 / (end - start), -start / (end - start))    start, end × 100 (field units)
Scat0 = -(βR + βM) × log2 e                            (0.000323, -0.6129) / (-0.144, -0.433, -1.010)
Scat1 = <light ambient> × ambient                      (0.45, 0.5, 0.3)
Scat2 = 3 / (16π) × rayleighPow × E × βR / (βR + βM), w = -2g     (0.8356, 0.1989, 0.4775, -1)
Scat3 = E × βM / (βR + βM) × rayleighPow × (1 - g)² / (4π), w = 1 + g²   (0, 0.1326, 0, 1.25)
```

so the extinction is `exp(-(βR + βM) s)` with s the eye distance over the fog's span (0 at
the start), Rayleigh in-scattering `(1 + cos² θ) Scat2`, Mie (Henyey-Greenstein)
`Scat3 (1 + g² - 2g cos θ)^-1.5`, and the ambient. Checked in a second field, `TOWN_10` (g 0, rayleighPow 15, E
(0.3, 0.2, 0), light ambient (0.7, 0.6, 0.6, 0.5), fog ambient 0.6): Scat0 (-6.059, -10.96,
-8.656), Scat1 (0.42, 0.36, 0.36, 0.5: Scat1.w is the light ambient's w), Scat2 (0.0767,
0.0141, 0, -0), Scat3 (0.2558, 0.2199, 0, 1): Mie's factor is rayleighPow (1 - g)² / 4π
(Henyey-Greenstein's numerator, with the same sun strength as Rayleigh's), 3.75/π there and
1.25/π in `TOWN_11`. A third field, `TOWN_07` (g 0.1, rayleighPow 20, βM (0.2, 0.3, 0.4),
βR (0.5, 0.8, 0.7), E (0.25, 0.3, 0.2), light ambient (0.5, 0.3, 0.4, 0.7), fog ambient 0.7),
read live in Logy's opening, matches to four places: Scat1 (0.35, 0.21, 0.28, 0.7), Scat2
(0.2132, 0.2604, 0.1519, -0.2), Scat3 (0.0921, 0.1055, 0.0938, 1.01). The renderer computes these per
vertex for the field's models, the terrain, the characters and the distant view (θ taken
between the way to the eye and the sun), and mixes each colour toward the in-scattered light
by the extinction. In `TOWN_10`'s town shot the distance turns warm as in the game, if
duller.

## Billboards (`MapSTD-B0.g1s`, `-B1`, `-B2`)

Leaf sprays (`obj-xN1_a15_05tree_a_b`, `obj-a15a01_balloon_space_low_town07_leaf` in TOWN_07)
are billboards: their layout has a PSIZE attribute (g1m semantic 4: the codes are Direct3D's
declaration usages, so 4 is PSIZE, not a tangent), every quad's four vertices sit at its
centre and PSIZE holds each corner's offset in the quad's own plane (±199.7, ±199.7, 0), with
the normal (0, 0, 1). `MapSTD-B0`'s vertex program 0 places them:

```text
world  = mL2W · position + mBbL2W · psize        (cb0[29..31], cb0[32..34])
normal = mBbL2W · normal
```

`mBbL2W` comes from the engine: the renderer takes it as the camera's right, up and back times
the model's scale, so each quad faces the camera. `-B1` reads the same constants (another
facing, likely upright only, not seen yet); `-B2` builds an axis billboard in the shader from
`BbPos`, `BbAxisY` and `BbL2WScl`. Drawn as stored, every quad collapses to a point, which is
why trees looked bare before (TOWN_07, TOWN_10).

## After the scene (`Data/x64/PostEffect/pe_pack.elixir.gz`, `PostEffectGlow.kps`)

The post effects are `_SPK4100` packs of DXBC programs (`barrel g1s` reads them too). The glow
pass is the game's whole HDR treatment: the scene is drawn at `HdrRangeInv` into an ordinary
8-bit target; program 0 takes `saturate(scene - fHdrThreshold)`; programs 1 to 14 blur it in x
and y over several smaller targets (7 taps, weights 0.0455, 0.0909, 0.1818, 0.3636, ...);
program 15 finishes with `scene × 2 + glow × fGlowAddRate`. So the range is 2 (`HdrRangeInv`
0.5) and there is no tone curve: values over 2 clip. The field's `<hdr threshold
glow_add_rate>` are the two constants. `PostEffectGamma`, `Color`, `Hue`, `Sepia`, `Monotone`,
`Fade`, `CrossFade`, `Blur`, `Wave` and others are separate effects the events can turn on.

The glow's targets, from the blur programs' tap offsets (one texel each): level 0 is 480×272
(a quarter of 1920×1088), then 240×136, 120×68, 60×34 and 30×17. Programs 1 to 10 blur across
then down at each level (each level's first blur reads the level above, so it also halves);
11 to 14 blur each level back up into the one above it.

The renderer draws the scene into its own 8-bit target at half brightness (`HDR_RANGE_INV`);
`glow.rs` then takes the bright parts into level 0 (a quarter of the screen), blurs across and
down through the five levels and back up, averaging each level into the one above (the blend the
programs do not show: plain addition summed five levels and made the glow three to five times
the oracle close-up's; averaging puts Escha's face at 246, 218, 183 against the game's 245,
219, 186, the workshop unchanged), and the post pass doubles the scene, adds level 0 at the
field's `glow_add_rate` and draws the text over it. A field without `<hdr>` gets no glow.

## Terrain (the field's own `<AREA>.g1s`)

The ground's programs are not in `Data/x64/Shader` (the engine's `CBlendMapTerrainShader`
classes) but in each field's container, `<AREA>.g1s` beside its `.ktf` (`TOWN_10.g1s`: 16
programs). Vertex program 2 lights the ground like `MapSTD`'s (`litDir`, `litCol`, `ambLit`,
`cDiff`, `cAmb`, `cSpec`, the light texture's projection `mW2Pt`, the shadow map's `mW2S`,
fog). Pixel programs 9 to 11 (two layers; 12 to 15 with a render-to-texture layer) take the
same shadow lookup, `sLit`, and two layers `sLand0`/`sLand1` weighted by the blend map's red
and green (each divided by their sum plus 1/1024), at the world position's x and z times
`texsize0`; colour = `(v2 + s·lit·v6) × layers + (v8 + s·lit·v7) × layers.a`, then the fog,
and × 0.5 written in (the HDR range's inverse). The renderer's terrain takes the same
per-vertex light as the field's models (with a white vertex colour) on its blended layers.

## Shadows (what is known)

The fields receive one shadow map, the first directional light's: `MapSTD` and the terrain
programs sample `sShadow` at `mW2S` (world → shadow map) with 16 taps (4 × 4) averaged and
scaled by `max((atColor.x - 0.7) × 3.33, 0)`, and multiply only the sun's diffuse and
specular by it (above). Casters are drawn with their own programs: `Saves/shader/shader.xml`
lists `standard_shadowmap`, `terrain_shadowmap` and the character variants `b0_shadowmap`,
`b0c_shadowmap`, `b0t0_shadowmap`, `b1_shadowmap`, `b1c_shadowmap`, `b2_shadowmap`,
`b2c_shadowmap` (by bone count and cut-out). The game's classes are
`EXSceneCoordinatorShadow` (vtable `0x14095a3f8`, constructor near `0x1404d75cf`; its methods
`0x14050d880`, `0x14050de00`, `0x14050de10`, `0x14050e190` only bind shader parameters),
`EXShadowMapShader` (`0x14095dec8`) and `CBlendMapTerrainShadowMapShaderBase` for the
ground. The engine (`ktgl`) carries cascaded, light-space-perspective, cube, dual-paraboloid
and plain depth mappers (their names are in the executable, no vtables are referenced from
code by RTTI), so which one the game runs, and how `mW2S` is built, are not read yet.

The casters (`SM_Psm.g1s`, `SM-B0_Psm.g1s` ...: "Psm", perspective shadow maps) write depth
only, through `mL2P` (statics) or `mW2P` and the skinning palette `mL2W` (characters). The
receivers' lookup (`MapSTD` pixel program 36, the terrain's alike) divides `v2.xyz` (from
`mW2S`) by `v2.w`, so the map's projection may be perspective; clamps the depth to 0..1; and
takes four bilinear 2 × 2 depth comparisons (`sample < depth` is shadowed) at the coordinates
plus the four offsets in `scSM2` (xy, zw and their mixes), with `scSM` = (texel size x, y,
map size x, y) placing the bilinear weights: 16 taps, averaged. The shader parameters' names are
in one table from `0x140952ca0`, 16-byte `{name, u32 type}` rows, split into groups by
`{u32, count, u64 rows}` records at `0x140953130` on (group 0: `mL2W`, `mW2P`, `mW2S`,
`mP2S`, `mL2WPrev`; `fog`, `litDir`, `vEye`, `Scat`, `scSM` ... in later ones). The type packs
`{array count, kind, flags}` bytes from the top: `mW2S` (`0x4070b`) and `mP2S` (`0x40703`) are
**arrays of four matrices**, so the game's shadows are cascaded (up to four cascades; the
engine's `CCascadedLSPShadowMapperImpl` is the likely mapper), `fog` (`0x433`) and `Scat`
(`0x40433`, four rows) vectors. The renderer object built at `0x140404500` on keeps each
group's values in blocks (`0x1403f9d20` lays them out: +0x3d0 group 0, +0x670, +0x9f0,
+0xec0, +0x1010 ...), so the code computing `mW2S` and `Scat` is what writes into those blocks
(not found yet).

## Eyes (`CharaEYEL.g1s`/`CharaEYER.g1s`, pixel program 12)

Four textures: `sDiffusePupWH` (the white, at the model's UVs), `sDiffusePup` (the iris, at
UVs moved by `g_UVOffsetScaleL`/`R`: `(uv - offset - 0.5) / scale + 0.5`, how the eyes look
about), `sSpecularPup` and `sDiffusePupHI` (the highlights, at the iris's UVs). The iris plus
`specular × sSpecularPup` (Blinn-Phong toward the view turned by `atSpecularLightOffset`, power
`g_specularPower`, only where the character light reaches) is laid over the white by the
iris's alpha; the light colour's soft light follows at 0.65 of its weight; the highlights,
under the rim colour's soft light at no more than 0.2, are laid over by their alpha; then the
point lights, fog, `atColor` and the HDR range as for the body. An eye material's textures are
in that order (white, iris, specular, highlights).

The renderer lays the iris over the white when it loads them (the iris's UVs are the white's
while the offset is 0 and the scale 1) and shades eyes (the `eye` flag) with the light colour
and the highlights; the specular is not done (`atSpecularLightOffset` and `g_specularPower`
are set by the engine, not read yet), nor the look-about offset.

## Outlines (`CharaFraHAIR`, `CharaFraBODY`, `CharaFraSKIRT`)

"Fra" is the characters' outline pass, not their shading: an inverted hull, the model drawn a
second time pushed out along its normals. The vertex programs (24 variants: skinning, wind for
hair and skirts) take the normal into clip space (through `mL2W` and `mW2P`), normalise it as
a 4-vector and push the position out by it times `0.0025 × (1.75 - 0.75 × vertex colour red) ×
min(w, 350) × (1 - 0.2 × saturate((distance - 190) × 0.05)) × g_outlineThickness` (y kept as
the normal's): about a constant width on screen, thinner where the artists painted the vertex
colour's red, and a little thinner far off. The pixel program colours it: the diffuse texture
under the light colour's soft light, blended half multiply, half overlay with `sOutline`
(`Data/x64/Shader/texture/outline.g1t`, at the second UV pair), its saturation pushed a little
(the channels within 1/255 of the brightest kept, the rest lowered 10%), then the point lights,
fog, `atColor` and the HDR range as for the body. `g_outlineThickness` is the engine's (not
read yet). In full, the push is `(n.x·0.0025, n.y·0.0044, n.z·0.0025, n.w·0.0025) × k` added to
the clip position, with n the normalised clip-space normal and k the factor above: the y
scale is 0.0025 × 16/9, so the width is even on a 16:9 screen, and with w capped at 350 it is
2.4 to 4.2 pixels at 1080 lines (times the thickness). The outline texture is sampled at the
clip position's x and y over 5w: a pattern fixed to the screen.

The renderer draws the outlines after the scene's models with a second pipeline that culls
front faces, for every character material with an `atEdge` parameter (not the eyes), in
outline mode (flag bit 2), coloured by the material's `atOutCol`, the y scale following the
screen's own aspect, and `g_outlineThickness` taken as 1.

## The materials' own parameters (`g1m` section 0x10003)

Each submesh names a set of shader parameters in its model (`barrel g1m-materials FILE#N`
lists them). Escha (`PC20_MODEL_default`): the eyes' sets have `g_UVOffsetScaleL`/`R` (0, 0, 1,
1), `atColor` white, `g_ambientColor` white and `g_specularPower` 64; the body's have `atAtten`
1, `atDark` (0.2, 0.2, 0.2), `atColor` white, `atOutCol` (0.8, 0.8, 0.8) and `atEdge` 0.2 (the
outline's colour and thickness, for the `CharaFra*` pass), `g_FalloffGradVals` (0.3, 0.2, 32,
32) and `g_ToonGradVals` (0.546875, 0.421875, 10.66667, 10.66667) - the rim and toon steps'
constants that `CharaMTC`'s pixel program has compiled in (0.3 × 32, 0.546875 × 10.66667) -
and `g_SphereCenter`. `Saves/shader/shader.xml` lists the shader packs by id (`CHARA_MTC` 1,
`CHARA_FRA_HAIR` 17, `CHARA_FRA_BODY` 18, ...) and `chara_shader_param.xml` the shared
textures (`sEnv` `defaultref.g1t`, `sOutline` `outline.g1t`, `sAlphamask` `alphamask.g1t`).

## The distant view (`Data/x64/Field/C_SPHERE/set-*.elixir.gz`)

Outdoor fields have a distant view around them: a sky dome (about 35,000 units across), far
ground, stones, plants and clouds (`set-a15_01_sphere`: `obj-a15a01_Sphere_sky_hi`,
`..._ground_hi`, `..._stones_hi`, `..._plants_hi`, `obj-xEF_a15a01_Sphere_cloud_hi`, each
model with its own texture pack). Which set a field has is the executable's table at
`0x14076eff0` (English): 122 records of 32 bytes, `{u64 field name, u64 set file, f32 × 4}`
(`TOWN_07` the airship dock: `set-a15_01_sphere_tw07`; `TOWN_10` and `TOWN_04`:
`set-a15_01_sphere`; indoors such as `TOWN_11`: `set-black`; of the four floats the first, 75 in nearly every
record, is the steepest walkable slope in degrees, whichever of it and the character's
`CharaBase` `FMslp` is larger (`0x14026c3c0` on the copy at the field object's +0x1a8); the
other three are not read).
The field's table places it: `<sphere offset tracking_camera>` (metres from the camera,
`TOWN_07` 20 below it). The renderer loads each event field's set into one instance that
follows the eye, draws it first with no depth test or write, alpha-blended (the clouds are
soft-edged), unlit (texture on vertex colour), and keeps the far plane past the dome.
