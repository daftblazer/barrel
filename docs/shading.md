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

A model names the shader each submesh draws with: its mesh groups (G1MG section 0x10009,
`Geometry::mesh_groups`, `barrel g1m-materials` lists them) give per pass a shader by the
`String` of `Saves/shader/shader.xml` and the submeshes it draws. Escha (`PC20_MODEL_default`):
`CharaEYEL`, `CharaEYER`, `CharaMTCN`, `CharaMTCNO`, `CharaMTDS`, `CharaMTDSO` in the colour pass,
`SM` in the shadow pass; her staff `CharaMTSO`, the hat `CharaMTDSO`. (An earlier note here
said the model names none: wrong, the section was not read.) Escha's body material (`PC20_MODEL`
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
`PARALLEL` `rot` as pitch below level then yaw in degrees (now read: the field loader,
`0x140068e6c` on, turns (0, 0, 1) by `rot.x` degrees about x and then `rot.y` about y,
right-handed (`0x14006bf70`), normalises it and keeps it as the light's `dir`, so the light
travels along (cos x·sin y, −sin x, cos x·cos y) and lights `max(n·−dir, 0)`; `rot.z` is
not used; no `rot` gives 30, 45; the table's own `dir` is read first and overwritten), and
point light distances in metres. Point light distances are in field units, as for characters. Checked again against
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

**Its mapping (`mW2Pt`) comes from the field's `<pattern_map speed_u speed_v area_width
area_height>`** (`TOWN_10`: -0.005, 0.01, 200, 200). The field loader (`0x14006937a`) reads
them (missing: speeds 0.0777 and 0.0333, sizes 50) and scales the sizes to field units
(`0x1404f1990`: +0x14 speed u, +0x18 speed v, +0x1c width, +0x20 height). Each frame
(`0x1404f1890`, the step in seconds) the offsets move by step × speed and keep their
fraction (+0xc u, +0x10 v), and the matrix is rebuilt (`0x1405eb280`) as three, multiplied
in turn (`D3DXMatrixMultiply`, row vectors):

1. `D3DXMatrixLookAtRH` from (0, 1000, 0) to the origin, +z up: a point goes to
   (−x, z, y − 1000).
2. A shear along the light (the direction the object was given, `0x1404f1980`; the sun's):
   rows (−1, 0, 0), (0, −1, 0), (l.x / l.z, l.y / l.z, ±1) with l the direction in the view's
   space, so each point slides along the light onto the plane 1000 units up.
3. Scale by 1 / width and 1 / −height, then add u + 0.5 and v + 0.5.

In the field's own space, d the sun's direction: U = (x − (y − 1000)·d.x / d.y) / width + u +
0.5 and V = (z − (y − 1000)·d.z / d.y) / height + v + 0.5. The cloud shadows fall along the
sunlight, one repeat of the texture per area, drifting at the speeds.

The renderer draws them so: each field's `_pat` texture (stubs left out) is bound with set 1
(binding 6, one set per field loaded), the mapping goes in the look block's slots 48 and 49,
and `model.frag` and `terrain.frag` multiply the sun's part by the texture's red at
`cloud_light(world)`. The drift runs from the scene's first frame (the game's from the
field's load, so where the clouds are in a given shot is not the game's). In `TOWN_10`'s
narration shot (`EVENT_MM01_OP1_111` at 58 s) the ground goes from 159, 127, 81 to 147, 120,
81 (the game 130, 107, 78); indoor fields are pixel-identical. The house front there is
still twice the game's (131 against 61): its shade is something else. It was the house's lighting material: `obj-a15_06house_body` is
the one model in `TOWN_10` whose g1m section 0x10001 is not all ones (diffuse 0.7, ambient
0.3: `cDiff` and `cAmb`; `docs/formats.md`), and the renderer takes them from it
(`Draw::light`, pushed at offset 124 as two unorm16; `model.vert` scales the ambient by `cAmb`
and the lights' diffuse, the sun's too, by `cDiff`). The wall behind Escha's close-up
(`EVENT_MM01_OP1_111` at 50 s) went from 94, 78, 73 to 29, 24, 23 (the game 34, 28, 26).
`cDiff`'s alpha (0.5 to 0.62 in a few models) is not applied yet.

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
reads `freq` at `0x140285327`. Each frame the field's wind clock `T`
(seconds, kept within 0 to 3600, `0x140286540`) gives each `<wind>` a strength from none to
full, `(sinf((T + shift) · 360 / freq °) + 1) / 2`, and the winds are summed, `power · dir`
each (`dir` normalised). Each `<v_shaker>` (`0x140285d80`) keeps its own clock, advanced by `2π
/ shake_time` a second (within 0 to 3600), and sets its meshes' constants: `g_nWindFlag` 1,
`g_fWindTime` its clock, `g_f3Wind` the summed wind over its `weight`, `g_fInvHeight` 1 over
its height (the kept height / 100), `g_fRootHeight` its root height (/ 100) and `g_f3Shift`
(`shift_x`, 0, `shift_z`). `game::wind` does the same (`wind`, `Shaker::step`); the
`<wind_root>` is read into `FieldLook::winds` and `v_shakers`. The keywords match the
placements' names (`.opl2`: `xV1_...`, `xS3_xV1_grass_1` in AREA_05_01; no model file names
them), and the renderer sways those objects: the look block's `sway` slots hold each group's
two vectors, a draw's flags bits 8 to 11 its group, and `model.vert` moves the vertices as
above. It takes the program's space as metres (the game's x, our units / 100: the parser keeps
height and root height × 100 and the frame divides them back, and AREA_01_01's wind 30 over
weight 800 sways 4 cm in metres, nothing visible in centimetres); not checked against the
game. The node shakers (`<n_shaker>`, trees and dense grass: placements named `xN1_...`) are a
spring per node. The parser (`0x140284f00` on) keeps `weight` (+0x20), the swing `axis`
normalised (+0x24 to +0x30, the rest direction), `axis_power` × 100 (+0x34), `max_angle`
(+0x38, degrees), `char_range` × 100 (+0x3c) and `char_power` (+0x40). Each node keeps a tip
direction `d` (+0x48, unit) and a velocity `v` (+0x58), and each frame (`0x1402856c0` on, `dt`
the frame's time):

1. the wind: the field's summed wind's length `s` and its direction turned into the node's
   space `u` (by the inverse of the node's matrix kept when it was set up, `0x1402855f0`; the
   frame's time is capped at 1/30 s): unless `|u · d| ≥ 0.9`, `v += u × s × dt / weight`;
2. within `char_range` of the player: `v += r × char_power × dt`, `r` a random unit vector
   (each part `(rand() % 100 - 50) / 50`, normalised);
3. `v += (rest − d) × axis_power × dt` (pulled back to the axis);
4. `d += v × dt`, normalised;
5. past `max_angle` from the rest direction (`cos(max_angle) > rest · d`), `d` is put back on
   that cone (`0x140285b7b` on, through the cross products of the two; not read further).

The node is then turned from the rest direction to `d`. `game::wind::NodeSpring` does these
steps (`FieldLook::n_shakers` the settings), its cone clamp in the plane of `d` and the axis
(the game's own, by cross products, is not read through). In the renderer the objects named or
tagged `xN<n>_` get their own instance (the rest of a field is baked into one), and each frame
each turns from its rest axis to its spring's tip under the field's wind (seven trees in
AREA_01_01, their leaves stirring frame to frame). The walking player pushes a tree within its
`char_range` (measured here from the tree's origin; the game measures from the tip in the
node's own space), along a random direction drawn as the game does (each part `(rand() % 100 -
50) / 50`, normalised; a small xorshift stands in for the C library's `rand`): from AREA_01_01's
start, walking north she brushes one a second in. A draw's name
is now its object's name and its placement's tags, since the sway keywords may be in either
(the grass's `xV1_` mostly is in the tags, which the grass sway had missed). The viewer
says how many sway at load: AREA_01_01 7 trees by node and 41 draws by vertex, AREA_05_01 10
and 37, TOWN_10 37 and 17.

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

## The shader table (`Saves/shader/shader.xml`)

74 `<Shader Name Id Autoload String EnableDrawOp DrawOpBit File>` rows: each name a material
can ask for, its id (0 to 73, in order), the short name the engine knows it by (`String`) and
its program file (`File`, a `.g1s`). The characters' are ids 0 to 18 (`CHARA_MT` ...
`CHARA_FRA_BODY`, `CharaMT.g1s` ...) and 72, 73 (the eyes); 19 the effects (`KTGLEffect.g1s`,
not loaded up front, `Autoload` 0); 20 `SWATER`; 21 to 29 the shadow-map programs (`SM_Psm`,
`SM-B0_Psm` ..., the terrain's `RenderTerrainShadowMap`); 30 to 36 the standard ones (`STD`,
`Standard`, `STD-B0`, `STD-T0`: several names share a file); 37 to 43 the 2D and screen ones
(`prim*`, `ipu`, `contrast`); 44, 45 refraction; 46 `battle_enter`; 47 to 51 `constantcolor*`
(again `Standard` and `STD-B0`/`-T0`); 52 to 65 the fields' (`MapSTD`, `MapSTDHS`, `MapSTDNS`,
`MapSTD-B0` to `-B2`, `-T0`, `-B0T0`, `MapSTDNF`, `MapSTDNA`, `MapSTDNA-B0`, `MapWater`,
`MapWaterNS`, `MapWater-B1`); 66 to 71 `E3dS*`, the only ones with `EnableDrawOp` 1 (draw-op
bit 8 for the `xAdd` ones, 16 for `xSub`: additive and subtractive blending, by the names).

## The field tables' own notes (`Saves/field/_def.xml`)

The game ships a commented template of the field tables (`Saves/field/<FIELD>.xml`), not read
by it. What its comments say, where it adds to the above:

- **`<camera>`**: `boundary_min`/`_max` bound where the eye may go (metres, y ignored);
  `center_position` locks the look-at point to a place, `pivot_position` locks it "reversed"
  (not both); `zoom_mode` is the movement that zooms out: `Y` vertical, `XZ` horizontal (the
  default), `XYZ` any. `<area tag>` picks how its points blend: `CLOSELY_X`, `_Y`, `_Z`, `_XZ`
  or `VERTEX_LENGTH`. A `<point>`'s `bumper_follow_speed` is the camera's base speed following
  its target, rising with the distance between them (negative turns it off; -1 if left out);
  `bumper_max_distance` the farthest they may be apart (metres); `zoom_time` the seconds to
  the widest zoom while the target moves, `zoom_distance` how far it pulls back then (metres,
  0 if left out).
- **`<fog>`**: `start` where fog begins, `end` where it is full, `ambient` the ambient
  scattering's strength, `mieG` the Mie scattering's eccentricity (its directionality, 0 ≤ g <
  1), `rayleighPow` the light's strength (overall scattering), `mieRGB` and `rayleighRGB` the
  Mie and Rayleigh parts.
- **`<light>`**: `PARALLEL` `rot` is the light's incoming direction as rotations in degrees.
  For `POINT` and `SPOT` the notes call `att0` the exponential, `att1` the linear and `att2` the
  quadratic falloff, and `range` "the square of the light's reach"; the game's code uses them
  otherwise (`att2` as the share left at the range, `range` × 100 as the reach: see
  "litAtt" above, checked against the game), so the code wins. `SPOT` adds `rot` (where it
  points), `inner` (the penumbra angle) and `outer` (the umbra angle), degrees.
- **`<camera_ct trans_speed camera_radius>`**: things in front of the camera turn see-through;
  **`<user_zoom relative_limit>`**: how far the player may zoom.
- **`<wind>`**: `dir` is normalised ("in the game y is up: y and z are swapped"), `power` its
  strength, `freq` the seconds from strongest to weakest and back, `shift` its phase in seconds.
- **`<n_shaker>`** (trees and dense grass, by node): `keyword` a part of the node names (the
  first matching `<n_shaker>` wins), `weight` (30 if left out; larger sways more), `axis` the
  swing axis in the node's own space ((0, 0, 1)), `max_angle` (10°), `axis_power` the pull back
  to rest (500), `char_range` the radius the player pushes it within (metres, 0) and
  `char_power` that push (10).
- **`<v_shaker>`** (low-polygon grass, by vertex, a sine sway): grass is modelled growing from
  the node's origin along +z; `weight` (200; larger sways less), `height` the grass's height
  (metres), `root_height` where it starts below the origin (-5.0 for 5 m down; 0),
  `shake_time` seconds a sway, `shift_x`/`_z` how much each vertex's world x and z shift the
  wave's phase (0).

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
0.5) and there is no tone curve: values over 2 clip. Only `glow_add_rate` comes from the
field (`<hdr threshold bloom_weight glow_add_rate>`, read at `0x1400690d3`; defaults 0.85, 0.4
and none): it is set by name (`fGlowAddRate`, `0x1403355c0`), while the threshold's setter
(`0x140287470`) stores it at +0x198 and ends in a stub (`0x1403355b0`, a bare `ret`), the
bloom weight is only stored, and `fHdrThreshold` is named nowhere in the executable. The
threshold is so the program's own default, 0.5 (its `RDEF`; `fGlowAddRate`'s default is 1.0),
in every field. Taking `TOWN_09`'s `threshold="0.0"` had made its glow a fifth of the whole
frame: against the reference recording's arch shot the right wall 107 became 89 (the game 88),
the sky 143 became 120 (123). `PostEffectGamma`, `Color`, `Hue`, `Sepia`, `Monotone`,
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
per-vertex light as the field's models on its blended layers, with the vertex's own colour
and normal (`docs/formats.md`, "Terrain"). Against the reference recording of
`EVENT_MM01_OP1_111`'s arch shot (`TOWN_09`), the baked colour darkened the band under the
houses from 169 to 134 (red; the game 107); the ground is still about 1.35 times the game's
(152, 118, 72 against 111, 89, 51) and the plaza's (`TOWN_04`) 1.15 times, evenly over the
channels: a light, not a texture. (`TOWN_09`'s models were 1.2 to 1.35 times too: its glow,
"After the scene"; with the glow's real threshold its ground is 1.14 times, 127 against 111,
like `TOWN_04`'s 145 against 127.) The terrain's vertex program has no point lights (only
`litDir[0..1]`, `litCol`, `ambLit`), so the renderer leaves them out of the terrain too (flag 64
in `model.vert`); it changed nothing at these spots, away from the lamps. Its pixel program 9
(`TOWN_09`) is the renderer's: shadow × `sLit` × the sun's part plus `v2`, times the blended
layers, plus the specular (`v8 + s·lit·v7`) × the layers' alpha (not drawn: it can only
brighten), the fog, × 0.5; program 8 (one layer, most of `TOWN_09`'s ground: kind 0x20
program 0 draws 5,146 of its lit indices) the same with `sLand0` alone. What is left is the
sun's part: with it off, the arch shot's near ground is 105, 85, 60 (the game 111, 89, 51, the
renderer with it 127, 99, 60) and the shaded middle ground, which takes no sun, 117, 97, 67
against 114, 94, 60. The game's near ground takes about a quarter of the renderer's sun there.
The sun's part is the one multiplied by the drifting light texture (cloud shadows), whose phase
is the game's from the field's load and the renderer's from its first frame, so where the
clouds fall in a given shot cannot match; that, not a lighting term, is taken as the cause.
(The renderer's ground is also 7 to 9 bluer, ambient or fog, small.)

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
soft-edged), unlit (texture on vertex colour), and keeps the far plane past the dome. The game draws it otherwise: by
their mesh groups the dome, far ground, plants and stones of `set-a15_01_sphere` are
`MapSTD` (the lit field program, with fog and scattering) and the clouds `MapSTD-T0`, so the
dome's brightness comes through the field's lights and scattering, which the unlit draw here
leaves out: the likely cause of the sky being too dark (`docs/renderer.md`).

## Ryza DX: the deferred G-buffer (`Data/x64/shader/pbb-*_gb.g1s`)

Atelier Ryza DX draws its field models deferred. A material's shader name (`PBB-N`,
`PBB-EmN`, ... in the model, `barrel g1m-materials`) names two packs in `\data\x64\shader`:
`pbb-<name>_gb.g1s` (or `_gba2`, `_dsgb`) writes the G-buffer, `pbb-<name>_cll.g1s` (about
190 KB) lights it. `pbb-n_gb.g1s` (Ryza DX), read with `barrel g1s`:

- **Vertex programs** (0 rigid, 1 skinned, 2 one joint) take position, normal, UV and tangent:
  **no `COLOR0`**. So the models' vertex colours do not reach these materials, and
  `barrel` draws them white (the attic's cabinet holds masks there for other passes).
- **The alpha test** of the programs that have one: `saturate(alpha × vSAMap.y + vSAMap.z)`
  (1 and 0 on every draw of the capture) below `vATest` is discarded. `vATest` is 50.1 / 255
  on every model's draw (the field's and the characters') and 63.1 / 255 on the plants'
  (`play::MODEL_ALPHA_REF`, `PLANT_ALPHA_REF`; the renderer's swaying draws take the second).
- **Two-sided materials.** The raster state of a G-buffer draw (the capture's state blobs,
  D3D11's `CullMode`) is back-face culling for the opaque materials (`PBB-N`), front-face
  culling for the grass cards and none for the alpha-tested ones (`PBBA2-N`, the program with
  the `discard`): all 47 of the gate's, the trees' leaf cards among them, except five that are
  culled as the opaque ones (flowerbeds and ivy: their lighting material, G1MG section
  0x10001, has the flags `0x133` alone, `g1m::Light::flags`; the leaf cards, grapes, signs and
  house trims have `0x111` or `0x1111` first). The program reads no face, so a back face is lit
  with the vertex's normal as it is. Barrel's `Draw::two_sided` follows that rule (`PBBA2-N`,
  the first lighting flags' low nibble not 3), with a pipeline of its own and the same in the
  occlusion's normal pass; the casters' passes never culled. Without it the cards that face up
  were culled from below, and 1.7% of the picture showed sky where the game has leaves. What
  the game keys the culling on is not read: the rule is the capture's, checked on its 47 draws.
- **Pixel program 3**, textures `sStage0` (the colour tile, kind 1), `sSpMap` (kind 2), `sNMap`
  (kind 3); constants `vDifScl`, `vSpcScl`, `atColor`. With `m` = `sSpMap.g` and metal =
  `sSpMap.b` ≥ 0.5:
  - target 0, diffuse: the colour tile (metal: × 0.2 m), × `vDifScl`, then `atColor` as
    `c × sat(atColor) + sat(atColor - 1)`;
  - target 1, specular colour: `m` as gray (metal: the colour tile × (1 - m)), × `vSpcScl.xyz`;
  - target 2: the normal (normal map red × alpha and green to -1..1, z rebuilt, in the tangent
    frame, turned to view space by `mWV`, octahedral-encoded in xy), and `sSpMap.a ×
    vSpcScl.w` (roughness or gloss) packed in w;
  - targets 3 and 4: 1.
  `barrel` draws the normal map (`model.frag`'s flag 8192, the map in the mask slot) in the
  game's frame: the mesh's tangent (semantic 6, four floats, in the vertex as a packed
  10-10-10-2 turned like the normal, `render::pack_tangent`), `N = normalize(normal)`,
  `T = normalize(tangent.xyz)`, `B = cross(T, N) * w` (the program's `r4 = T.yzx*N.zxy -
  N.yzx*T.zxy`: T crossed with N, not N with T; in Barrel's mirrored render space the same
  vector is `cross(N, T) * w` with the stored w unchanged) and the result `T y + B x + N z`,
  none of them orthogonalised. Every normal-mapped submesh at the gate has a tangent; a mesh
  without one takes a frame from the screen derivatives of the position and UV (the map's
  red along u, its green against v), which stands within a degree of it over the stage
  (the median angle to the game's normal target 5.7 degrees against 5.6 with the tangent). With the mapped normal the sun's Lambert term and the cube
  map's light are taken again per pixel (the vertex's light otherwise). Every field draw at
  Rasenboden has one; it shows as relief on the rocks and bark, the shade included, for about
  0.07 ms at 1280×720 on the PC.
  The red maps of the attic's furniture (green and blue near 0) give the colour tile as the
  diffuse and no specular: albedo times light, as `barrel` draws them.

**The lighting pass** (`pbb-n_cll.g1s`, pixel program 3, 168 KB: a forward pass that samples the
colour, specular and normal maps itself, with the shadow maps, the light grid `sLit`, the
light-propagation volume `sLPVBuffer`, and the field's cube maps `sAmbDif` (`_ad`) and
`sAmbSpc0` (`_as`)). Its constants: `litDir`, `litCol` (two directional lights), `ambLit`,
`fDiffuseScale` (cb0[11].y), `vAmdLerpParam` (cb0[71]), `vAmbParam` (cb0[72]), `vAmbDifScl`
(cb0[73]), `vAmbSpcWgt` (cb0[74]), `atColor` (cb0[81]). The end of the program:

- **Diffuse ambient**: `A = sAmbDif(n) × vAmbParam.x × vAmbDifScl.rgb`; with the volume's
  light `L = LPV(n) × fDiffuseScale`, `vAmdLerpParam.w` picks 0: `L + A`, 1: a lerp from `L` to
  `A` over distance (`vAmdLerpParam.y`, `.z`), 2: `L + A × that lerp`; then `+ ambLit` and `+`
  the direct lights. All of it times the diffuse colour.
- **Specular ambient**: `sAmbSpc0` at the reflected view direction, mip level from the
  roughness (`vAmbParam.z`), × `vAmbSpcWgt.x × vAmbParam.y`, × a Schlick term (`(1 - x)^5`) on
  the specular colour, plus the direct specular.
- Two height-fog layers (constants from cb0[12] on, each with five-term wave sums), then
  `atColor` as in the G-buffer program.

So the cube map's light is scaled, not multiplied by an ambient colour, and an ambient colour is
added. The executable names the constants in tables of `{name pointer, type word}` (the ambient
group `vAmbParam`, `vAmbOccRat`, `vAmbSpcWgt`, `nAmbSpcNum`, `vAmbDifScl`, `vAmbSpcScl` from
0x1413f9c40; `vDiffSrc`, `vMatSrc`, `bSpec`, `ambLit`, `fog`, `cFog`, `litDir`, `litPos`,
`litCol`, `litAmb` around 0x1417b23b0), set through those tables by index, so no code refers
to the names. The environment's `indirectStage_` block (structure +0xda0) reads, by its
reader's member offsets, `{IDLScalerOfAmbientDiffuseMap @0?, ISLScalerOfAmbientSpecularMap @4,
the two ColorRGB @8 and @0x14?, IDLFade @0x20, ISLFade @0x24, IDLScalerOfLPV @0x28,
AmbientLightScale @0x30, KeyLight0Scale @0x34, ...}`. Which environment values become `vAmbParam`, `vAmbDifScl` and `ambLit` is not placed:
the default XML's `indirectStage_IDLScalerOfAmbientDiffuseMap` (2.5) and its `ColorRGB` (1.64,
1.98, 4.0) fit `vAmbParam.x` and `vAmbDifScl`, and the file's 0x484 values (2 to 4 in the
attic) fit that RGB rather than an ambient colour.

## Ryza DX: the character lighting pass (`Data/x64/shader/dschara-cll.g1s`)

`shader.xml`'s `DSChara-Cll` (header variant `PBDS-CllEEmIra3lnhIriLr`) lights the G-buffer's
character pixels: a full-screen vertex program and one pixel program (8480 bytes) reading depth
(`t4`, 1 is the sky: discarded), `sGBuf0..5` (`t5`..`t10`), the ramp (`sRamp`, `t3`) and the
clustered lights (`tCllLightIndices/Positions/Attributes`). What it does, from the code
(`barrel g1s dschara-cll.g1s 1`):

- **Normal**: GBuf2's `xy`, octahedral (`2v - 1`, the folded lower half), turned to world by
  `mV2W`; the position from depth through `mP2W`. GBuf1 is the specular colour (`xyz`) and a
  byte of flags in `w` (bit 0: a lit emissive in GBuf3, times 32); GBuf0 the albedo (`xyz`) and
  an occlusion (`w`); GBuf4 `x, y` select the rim row (`x × 255`, at most 15) and the ramp row
  (`y`), `z` a back-light amount, `w` an occlusion; GBuf5 an occlusion and gloss (`w`, `2^(10w+1)`).
- **Rim**: `cRimTbl[16]` (one float4 each): `pow(1 - N·V, rim.w) × rim.x`, added to each
  directional light's clamped `N·L` weighted by how much the eye faces into the light
  (`sat(L·V)`), so it shows when she is backlit.
- **Lights**: the two directional lights (`litDir[2]`, `litCol[2]`) and the cluster's point
  lights (an exponential fall-off, `exp(-d² × a) × b`), each with a Blinn-Phong specular
  (Schlick Fresnel from GBuf1's colour, normalised by `gloss × 0.125 + 0.25`). The diffuse is
  `litCol × (rim + N·L) + litCol × N·L × back-light`, times the albedo mixed toward white by the
  occlusion (`fOccAlbScl`).
- **Ramp** (toon): `u = clamp(max(s.x × h0 × m, s.y × h1), range.x, range.y)`, where `h0` and `h1`
  are the two lights' half-Lambert terms (`sign(N·L) |N·L|^p × 0.5 + 0.5` taken with the
  light's luminance pulled toward 1 by `rampCoeAdj`: with the captured `p` and `rampCoeAdj`
  of 1, just `N·L × 0.5 + 0.5`), `m = min(occlusion, shadow)` (the occlusion GBuf0's `w`, the
  shadow the ambient pass's alpha: the cascades' shadow times the cloud pattern; GBuf4's `w`
  is 1 on her), `s` is `rampCoordScaler` (0.27, 0.116) and `range` `rampCoordRange` (0.2,
  0.5); `u` is `occlusion × 0.2` where the occlusion is under `occThresForIRShadow` (0.2). So
  `u` runs from 0.2 (shade) to 0.27 (facing the sun, unshadowed): the short run of a row
  where its shade tint turns white. v is `(rampIndex - 0.5) / rampTexHeight` (256): the
  texture's row before the index (index 1 takes row 0), and index 0 takes no ramp. The ramp
  multiplies the diffuse (albedo times all the light, ambient included), not the specular;
  the output is specular plus ramped diffuse. (Values as captured at Rasenboden's noon; where
  the game keeps them is not found. The G-buffer's colour targets are read through the sRGB
  view: GBuf0's albedo is in gamma space in the dump.) With the capture's G-buffer (her
  pixels' GBuf0, 1, 2, 4, 5 and the ramp) the program above reproduces the game's lit target
  over her to a mean absolute difference of 0.0085 (2.8% of the light) and a mean within 0.2%.

The ramp texture (`Data/x64/shader/texture/chara_rmp.g1t`, 256 × 256 BC7) is white but for a
few thin rows going from a dark tint to white: one row per kind of material. Row 0 is a light
red (242, 172, 164), row 1 a dark brown (90, 46, 32), row 3 a pale pink (247, 199, 193) that turns
white within a quarter of the row, row 4 a long warm brown, row 9 a dark blue-grey; most rows
are white.

Ryza's materials name their row: her submeshes' parameter sets (G1MG 0x10003) carry
`rampIndex` (an integer in a type-1 word: 3 on submeshes 0 to 4, her coat's cloth pieces,
which take row 2, a grey pink; 1 on the rest, which take row 0, the light red), `rampRimColorAdj` (1, 1, 1), `rampRimThres` (1), `rmIndex` (0:
likely her rim row), the wetness (`wtWet`, `wtBldPrms`, `wtDtFade`, `wtDtMkFade`: all 0) and on
her hair (`@C86A7C58`, `@E905E6C7`) `alr2HAdj` (-0.11) and `alr2HDir` (0, 1, 0, 6: an
anisotropic highlight along the hair, "Alr2h" in the `PBB-Alr2h…` program names); her eyes
(`@E24D26E6`) have texture transforms and a parallax (`vPlx` 0, -0.05), her face's outline-like
part (`@6679C989`) `ocrAdj`, `ocrBDir`, `ocrUDir` ("Ocr" programs). The pass's constants
(`rampCoord*`, `fOccAlbScl`, `cRimTbl`) are set at run time and not found; the capture's
are in `model.frag` and on the constants board (`CHARA_PASS` in
`crates/ryza/src/play/constants.rs`). Her G-buffer on the capture: GBuf4 is (rim row 0, ramp
row 1 or 3, back-light 0, occlusion 1) on every pixel, GBuf1's flags are 0 (no emissive) and
its specular colour small (mean 0.03, 4,430 pixels over 0.1), GBuf2's gloss 0.09 on average.

`barrel` lights her (`model.frag` flag 128) as the pass does, per pixel, with the field's two
lights at the characters' scales (the sun's and the second light's, `look.sun_*`,
`look.fill_*`), the ambient flat, her occlusion map's green times the screen occlusion for
`o`, and the shadow `s`. With `m = min(o, s)` the sun's diffuse weight is `sat(rim ×
sat(litDir · V) + sat(N·L))` with `rim = (1 - N·V)^2` (the capture's `cRimTbl`: every entry
strength 1, power 2, row 0 on her), and the diffuse light is `(sun × w0 × m + second × w1)
× mix(albedo × fOccAlbScl, 1, o')` plus the ambient × `mix(albedo × fOccAlbScl, 1, 0.7527 o +
0.2473)`, with `o' = 1 + IRAFade (o - 1)` (0.2497); a material without a ramp row takes `o`
for `o'` and the shadow alone for `m`. The result times the ramp's colour at `u` above. Not
done: the direct specular term (0.4% of her light on the capture), the back-light amount (0 on her),
the point lights' share of the pass, and the normal map (the G-buffer's normal carries it; the
vertex normal is used). Her forward passes follow.

**Her forward passes.** Two things happen to her pixels between the lighting pass and the tone
pass (the draws `1927` to `2031` of the capture; the dump brackets them as one range), and the
larger is not the hull.

- *The fog and specular ambient pass* (`1927` and `1928`, one program told apart by stencil: the
  stage, then the characters) adds the cube map's light reflected off her by Fresnel on her
  G-buffer's specular colour, as for the stage (the paragraph on the specular ambient pass
  above), with her block of the set: `vAmbParam` (1.5, 6, 5, 1/256), `vAmbSpcScl` 0.6,
  `vAmbOccRat` (0.7527, 0.2473, 1, 0): the light scale 6 x 0.6 = 3.6, against the stage's 1. Her
  specular colour and gloss are written by her G-buffer program (`shb62cde394912a5a7`) as the
  field models' are: `F0` the specular map's green (a metal, blue 0.5 or more, takes the colour
  tile x (1 - green)), the gloss its alpha, and the metal's diffuse is the tile x 0.2 x green. This
  is the red sheen on her stockings and boots (the stockings are metal in the map): the thigh goes
  from 0.0096 to 0.080 in red over the passes, the boots from 0.019 to 0.042, the coat not at all.
  `barrel` does it in `model.frag` (flag 128 with 1 << 24: her specular map in set 4, her
  metal's diffuse, the cube map at her block's scale, `look.chara_spec`) with her own occlusion
  as `o`. Left out as in the stage's: the half-size screen-space reflections the pass mixes
  toward by their alpha (they stand in for 15 to 20% of the sheen: the sheen is that much
  too bright on the boots and the thigh).
- *The hull* (draws `1929` to `2024`, `sh470bcc97a7144d67` and `sh0d2367f637ea8f6a`, one per
  submesh of every character: her, the townsfolk): the model again with front faces culled,
  inflated along the clip-space normal, alpha-blended (source alpha) over the scene with the
  depth tested (less or equal) and written, stencil 8 written. Only the pushed-out back faces
  that reach past the body show, as a dark line of one to two pixels on her silhouette and the
  seams of her overlapping pieces (the cause of the dark rings on her coat's loops and
  hood). The vertex program: the skinned position and normal; the thickness is
  `((1 - sat(z / w))^fOLZDecayRatio x (vOLIntensity.y - vOLIntensity.x) + vOLIntensity.x) x
  vertex colour alpha x (1 + fOLThicknessConstantRatio (w - 1))` (her vertex colour's alpha is
  0.4 on every vertex of her body), the clip-space normal's direction (normalised over its four
  components) times it, x by `fAdjByAspect`, z (kept positive, away from the eye) by `fOLZScale`,
  added to the clip position. The constants at the gate: `vOLIntensity` (0, 0.6),
  `fOLZDecayRatio` 0.35, `fOLThicknessConstantRatio` 0.01, `fAdjByAspect` 1, `fOLZScale` 0.1,
  `vOLColorScaler` 0.5, `fOLLtEfFade` 0.5, `fOLCSatFade` 0, `vOLAmbEfScaler` 0 (on the constants
  board as `forward.chara`). The pixel program takes the colour tile's alpha as the blend and
  the alpha test (`vATest` 50.1 / 255), the colour tile plus (with a specular map, `vSpMap` (1, 0,
  1, 0)) its red times the two lights' Blinn-Phong highlights (`cSpec` power 20, the lights at
  the set's own colours, the sun's 4.78, 5, 3.5 and the second light's 0.15, 0.29, 0.5), scaled by
  `vOLColorScaler` and by `1 + sat(fOLLtEfFade)(ambLit x vOLAmbEfScaler - 1)` (0.5), then
  fogged with the height-fog layers and the scattering as the other forward draws (nothing
  at her distance here: the extinction is 1). A submesh without a specular map takes the
  same program less the highlights (`sh718056b9b500791f`). The programs of her face and the
  townsfolk's faces (`sh6ff58774ff997f98`, `shf9b98df9548ba6fa`, draws 2025 to 2031, 4,800 lines)
  are not read. `barrel` draws the hull in `renderer.rs` after the scene (flag 1 << 25 of
  `model.frag` and `model.vert`, a pipeline of its own that culls front faces and blends, depth
  not written), for the draws that take the toon ramp, the colour in the game's units
  times the scene's exposure.

The pass's constants come from the environment overrides (`saves\fieldmap\override_env\
env_<name>.xml`, one `<root>` of `group_name` attributes; `env_default.xml` and a few fields'
own): the `ir_` group is the character lighting ("illustration render"). The defaults:
`ir_firstLightRampCoordinateScaler` 1 and `ir_parallelSubLightsRampCoordinateScaler` 0 (the
signed power on the sun's half-Lambert, so u is that term as is, the second light not
ramped), the ramp coefficient adjusters 1, `ir_rampCoordRangeMin/Max` 0 and 1, `ir_IRDFade`
and `ir_IRAFade` 1, the rim (`ir_rimScaler` 15, `ir_rimSharpness` 10, `ir_rimColorScaler` 0.1;
the hair's the same with colour 0: no rim on hair; `ir_rimElevation/Azimuth` 0), the hair's
highlight (`ir_hairHighlight*`: reflection ratio 0.4, scaler 0.3) and a mask
(`ir_maskScale` 20, rotation 45). (The captured `cRimTbl` holds strength 1 and power 2 on every entry, not these rim settings: the rim the pass draws is the table's.) Next to it the `albedoRetouch_` group (toon, rim and specular
light directions, a shadow colour limit 0.5), `outline_`, `indirectChara_` and the field's
lights, fog, HDR and shadow cascades (`shadowMap_ShadowCascadeStart` 1200, 6000, 13000; erased
from 20000 to 30000). The hair's G-buffer program (`_DSGb` with `alr2H`) lays the
highlight map (`sAlbedoRetouch2HighlightMap`, the material's kind-66 texture: Ryza's
`pc00a_model.g1t` #16, 512×512 BC7, orange bands with their strength in alpha) over the albedo
by its alpha, at the hair's UV shifted along v by `dot(V, alr2HDir) × alr2HAdj` (`alr2HDir`
(0,1,0), `alr2HAdj` -0.11: the band slides up and down the hair with the view's pitch).
`barrel` composites it unshifted at load (`composite_overlays`, as Escha's eyes).
Ryza's sky dome (`data\x64\field\skydome\<time>_<weather>.elixir.gz`, Rasenboden's noon
takes `morning_sunny`: two submeshes, a band of thin clouds and one of big ones, 2048 × 1024
and 2048 × 512 BC3, scrolled by `txC0` 0.004 and 0.002) draws with `PBSkyDome-NT0Tn`
(`skydomepbs-nt0tn_cll.g1s`, program 2): the cloud texture is lit as a surface, (the ambient
light map along the normal × `vAmbParam.x` × `vAmbDifScl` + `ambLit` + `litCol` × N·L) ×
albedo, plus a specular term from the kind-3 map and the ambient specular map, then the
height fog layers (`cHFog`, `hFogParam`, a sum of 20-odd sines for their wavy top). The
dome's normals point inward and down (y -0.5 to -0.76), so the sun's N·L is 0 overhead and
the clouds take mostly the ambient. Its vertex program (`shfcc3b7e2d2785fde`) takes no
vertex colour and fogs the dome with the sky's scattering, not the field's: the sky plane's
`Scat` rows and its `fog` range (noon: 1000 to 300,000), extinction `min(exp(s × Scat[0]),
1)`, the pixel's colour mixed with the in-scattered light by it. As the game sets them at
Rasenboden's noon: `litCol` (4.78, 5, 3.5) along (-0.354, -0.707, -0.612), `ambLit` (3, 6,
4), `vAmbParam.x` 0 (no ambient map), `cHFog` zero (no height fog), `atColor` 1; the model
stands at (30000, 0, 30000) three times its size, its two layers scrolled by `txTrs01`
(0.0568 and 0.0284 in that frame). The set holds these (docs/formats.md, "The sky").

`barrel` lights the dome so: (`ambLit` + `litCol` × N·L, the normal from the dome's normal
map where it has one) × the exposure × the cloud texture (look slots 92, 95 and 96,
`model.vert`'s and `model.frag`'s flag 4096), fogged with the sky's scattering, placed where
the set says, with the program's specular term, and each layer's texture slid along u by the
game's `txTrs01`: half the material's `txC0` a second (0.0568 and 0.0435 on two frames 400
apart, for the first layer's 0.004; the second layer's is half that), counted from when the
field was entered (`play::dome_shift`; the push constants' last float carries it for a dome's
draw). The dome is the thin, pale layer of
the sky only: the game's large white clouds are two more draws of its sky pass, a strip
showing a 1024 × 1024 target the clouds are ray-marched into (the set's `cloud_` block) and
the cloud particles (`\data\x64\sky\cloudpiece.g1t`, the set's `cloudParticle_` block),
both lit by the second light after the dome's in the set ((35, 48.8, 70) at noon). Without
them the sky's pixels at the gate's noon average 123, 226, 253 against the still's 153, 242,
252.

## Ryza DX: the tone curve, saturation and fog passes

**The tone curve.** `effect2dpreinvtonemapandgamma.g1s` (pixel program 1, constants `fGamma`
and `fInvToneMapParam`) takes a finished pixel back to the scene's light for 2D effects: it
raises the colour to `fGamma`, multiplies by `fInvToneMapParam`, adds 0.0333 and solves a
quadratic with the coefficients 0.22, 0.30, 0.03, 0.06 and 0.002. That is the inverse of
Hable's filmic curve, `f(x) = (x (A x + C B) + D E) / (x (A x + B) + D F) - E / F` with A 0.22
(shoulder), B 0.30 (linear), C 0.10, D 0.20 (toe), E 0.01, F 0.30, divided by its value at a
white point (`fInvToneMapParam`), followed by a gamma of `1 / fGamma`. So the game lights in
linear light and finishes with `pow(f(x) / f(W), 1 / fGamma)`; the set's `trueHDR_fLumiMax`
(4.3 at Rasenboden's noon) taken as W and its `fGamma` (1.8 at noon, 2.2 otherwise) match the
game's own constants (below, "the frame as the game draws it"), with the colour textures
sampled as sRGB (as stored values the sand's blue comes out twice too high). `barrel` does
this (`post.frag`'s `filmic`, `Frame::tone_gamma`; `upload_texture`'s `linear`).

**`SaturationScaleEx`** (`saturationscaleex.g1s`, constant `fScale`; the per-set values of
`env_override.xml`): `c = y + s (c - y)` about the BT.601 luma `y` (0.29891, 0.58661,
0.11448), where for a scale above 1 `s = 1 + (1 - L)(fScale - 1)` with `L = (min + max) / 2`
(dark pixels take the whole scale, white ones none), and below 1 `s = fScale`. The game's
last pass does the same after its gamma (`fSaturationScaleEx`: 1.6 at Rasenboden's noon and
on the title, `LGT01_DEFAULT_01`'s row for a clear noon), and `barrel` lays it on there
(`Frame::tone_vivid`, `environment::saturation_ex`).

**Height fog and scene fog** (`heightfog.g1s`, pixel program 1, on the scene's depth and
colour; the lighting passes carry the same constants). The point's world position comes from
the depth. Each of two layers: `h = y + Σ amp_i (sin(2π (x × xs_i + xo_i)) + cos(2π (z × zs_i +
zo_i)))` over four waves (`hFogFlucPrms`, five floats each in a 16-byte slot: `effectHeight`,
`xScaler`, the x offset (`xVelocity` × time), `zScaler`, the z offset), `t = saturate(h ×
hFogParam.x + hFogParam.y)`, `f = saturate((1 - e^(-t²)) × cHFog.a) × saturate(distance ×
hFogParam.z + hFogParam.w)`, and the colour goes `f` of the way to `cHFog.rgb`. Measured at
the gate (noon): `hFogParam[0]` = (-1/1500, 4.2227, 1/8000, 0), a layer from height 6334 down
to 4834 fading in over 8000 units; `hFogParam[1]` = (-0.004, -0.8, 0, 1), the default
override's -200 to -450; both `cHFog` zero there (no height fog at noon), so `x = 1 / (end -
start)`, `y = -start / (end - start)`. Then the scene fog, as Escha's: extinction `min(exp2(L
× Scat[0]), 1)` with `L = distance × fog.x + fog.y`, the colour mixed toward the in-scattered
light `(1 + cos²θ) Scat[2] + Scat[3] (cosθ Scat[2].w + Scat[3].w)^-1.5 + Scat[1]`.
`barrel` draws the scene fog from the set's `fog_` (Rasenboden's noon: from 5000 to
700,000 units, so only the far hills take it), built as the sky's `Scat` is; the height fog is
not drawn. The fog pass of the stage and characters takes the sun at the stage's key scale: its
`Scat[2]` is three times the specular ambient pass's own (0.4077, 2.046, 5.648 against 0.1359,
0.6821, 1.8828 at noon), the lighting pass's `litCol` scale (`KeyLight0Scale`, 3); `barrel`
builds the scene's `Scat` with it (`look[32].z` holds the scale, and the specular ambient divides
it out for its own fog). The sky's `Scat` takes the sun at 1.

**The characters' light scales.** The lighting blocks read at the gate hold the sun at 0.8 of
its colour and the set's ambient at 0.2 for one kind of draw (the characters'), which are the
words at +0x34 and +0x30 of a block at 0x5d0 of the noon set, right after `skyFog_`:
`indirectChara_` (`KeyLight0Scale` 0.8, `AmbientLightScale` 0.2, by the reader's member
offsets; its first words 1.5 and 6 are the diffuse and specular map scales, then the diffuse
map's colour (0.8, 1, 1) and the specular's). The stage's block, `indirectStage_`, is laid out
the same at 0x54c, between `fog_` and `skyFog_`: Rasenboden's noon holds a diffuse map scale
of 7 and colour (0.1, 0.2602, 1) with `AmbientLightScale` 0 (+0x30) and `KeyLight0Scale` 3
(+0x34: the game's lighting pass takes the sun at three times the set's; the word at +0x2c,
`KeyLight1Scale`, scales the second light: 1 on the stage, 0.4874 on the characters, whose draw
holds the sun at 0.8 and the second light at 0.4874 of the set's), so the stage's shade is
the light map tinted deep blue and the set's ambient colour is left out; a colour fitted to
the game's still before the block was found came out (0.8, 2.0, 6.5) against its (0.7, 1.82,
7). The attic's sets hold 1 × (1.9, 2.3, 4) in the morning and 1 × (2.0, 2.2, 2.7) in the
evening. Before it, at 0x528, ten words: a colour, then a third directional light (35, 48.8,
70 at 30°, 30° at noon: not placed).

## Ryza DX: the frame as the game draws it (measured)

From the answer key's captures (Rasenboden at the gate, noon, frame 2550; the title's frame
1135 holds the same constants). The capture names each bound view's format, so what is
decoded as sRGB is known, not guessed.

**The passes**, in order: a depth pre-pass (9 draws); the G-buffer (903 draws into four
targets: colour, specular colour, normal and gloss, a fourth); the ambient occlusion (the
depth made linear at half size, the occlusion from depth and normals, the result stretched
bilinearly over the colour target's alpha; below); four shadow maps (two cascades of 2000 × 2400, one
of 1600 × 1600, one of 1024 × 1024); the ambient-and-shadow pass (3 draws, below); the
lighting pass (4 draws, below); the water (10 draws) and its own ambient and lighting; the
sky (the scattering plane, the dome's two layers, the clouds); the scene fog; screen-space
reflections at half size and the pass that adds the specular ambient; 105 forward draws
(transparent things, the characters' outlines and faces); then the post chain: temporal
anti-aliasing, depth of field, the luminance metering (64, 16, 4, 1 pixels), the bloom and
star chains, light shafts, the tone pass, the interface.

**What is sRGB.** Every game texture has two views, one sRGB and one not. The colour map is
sampled through the sRGB one, the specular and normal maps through the other; the G-buffer's
colour target is written and read through sRGB views (so it holds linear colour at sRGB
precision), the specular target as stored. The cloud pattern is sampled as sRGB. The lit
scene is a float target; the tone pass writes its own gamma into a plain 8-bit target.

**The ambient-and-shadow pass** (one program, drawn three times with different constants:
the stage, a second kind, the characters) writes the ambient light in rgb and the sun's
share in alpha:

- rgb = `sAmbDif(n) × vAmbParam.x × vAmbDifScl.rgb + LPV(n) × fDiffuseScale + ambLit`
  (the two `ilaScale` words, 0 here, would scale the parts by the normal target's low byte).
  The stage: 7 × (0.1, 0.2602, 1) on the diffuse map, no LPV, `ambLit` 0. The second kind:
  no map, `ambLit` (3, 6, 4). The characters: 1.5 × (0.8, 1, 1), LPV at 0.0637, `ambLit`
  (0.6, 0.6, 0.8) = the set's ambient × 0.2.
- alpha = the cascades' shadow × `saturate(1 + fPtn × (pattern − 1))`: the cloud pattern
  (`<field>_pat.g1t`'s red, at the world position through `mW2Pt`) takes away up to `fPtn`
  (0.7) of the sun. `mW2Pt` at the gate: u = (x − y d.x / d.y) / 50000 + o, v = (z − y d.z /
  d.y) / 50000 + o with d the sun's direction, o growing 0.005 a second (-0.138 at frame
  1750). The plane the points slide onto is at height 0 here (the translation is `o`
  alone, on both axes), not Escha's 1000 up: `play` takes the 1000-plane's share off the
  drift it hands the shader. Checked on the turn view's dumps: the pattern's red (as sRGB)
  through `mW2Pt` at each pixel's world position gives the pass's alpha on open ground to
  0.002; Barrel's reads 0.456 against 0.458 there (0.533 before, the pattern standing 500 and
  866 units off). The strength, a switch, the size and the speed are the words at +0x38 to +0x4c of
  the set's `indirectStage_` block (0.7, 1, 50000, 50000, 0.005, 0.005 at noon; the other
  times hold strength 1 and switch 0, taken as off).

**The cascades** (the pass's shadow term, one frame at the gate; docs/ryza/open.md). The
pass reads three cascades (`mW2S`, an array of three orthographic matrices: u along the
horizontal, v up the slope, depth along the sun), the static map (`mSemiStaticW2S`, 2048², mode
`iSemiStaticSMMode` 2) and a small map round the characters (`mSubW2S`, 1024², `nSubSdw` 1).
- **Picking.** The distance along the view `z` is `vS2VZ` on the first cascade's coordinates
  minus 1600: cascade 0 up to 1600, cascade 1 to 1600 + `vSMDF.x` = 3600, cascade 2 beyond.
  `sdplane` is (10000 - z) / 3500 on the same coordinates, saturated: the cascades hold to z
  6500, fade to the static map by 10000 (`1 - shadowed × weight`, where weight 0 reads the
  static map alone).
- **The boxes.** Cascades 0 and 1 are the smallest box in the sun's view (up along +y) round the
  view's frustum from z 0 to 1600 and 1600 to 3600 (widths 2115 and 4758, to 0.4% in both
  directions), their depth from one plane 36755 before the camera toward the sun (the same
  for all three: 36900 to 37500 deep) to the slice's far side. Cascade 2 is 11672 wide, which
  no slice from 3600 reaches (a slice to 10000 is 13209): not found. The boxes are not
  snapped to texels. The maps are 2000 × 2400, 2000 × 2400 and 1600 × 1600 (the texels
  1.06 × 0.80, 2.38 × 1.98 and 7.3 world units); the sub map 1024².
- **The static map** (`mSemiStaticW2S`, `sSemiStaticShadow`, 2048² at `scSemiStaticSM`). Its box
  in the sun's view (u horizontal, v up the slope, the cascades' way) spans exactly the
  heightmap's square (0 to 64,000 in x and z on Rasenboden) at heights 0 to 5,000 (not the
  map's 1,766 to 7,000), its depth 173,439 from the square's far corner toward the sun: 42.7 ×
  31.9 units a texel. Mode 2 (`iSemiStaticSMMode`; 1 reads it alone, others not at all) reads
  it where the cascades' fade weight is below 1: sixteen point taps four by four a texel apart
  from 1.5 texels before the point, each `sample < depth` shadowed with the depth saturated, 1
  when the taps' square leaves the map, mixed with the cascades' light by the fade weight. Its
  contents are drawn at load (not in the capture); Barrel draws the field's casters into it
  with the far cascade's bias (a stand-in). Past z 10000 the sun's share then matches the
  game's on average (gate 0.352 against 0.350, turn 0.497 against 0.443, left 0.384 against
  0.372; it was 0.42 to 0.53 with a box round the whole scene, distant view included, some
  450,000 units across).
- **The casters' pass.** Back faces culled, no raster bias; the pixel program writes the depth
  plus `sbias.x` × (|ddx| + |ddy|) plus `sbias.y` to a float target (the depth buffer holds
  the nearest), and the cut-out variant drops texels under `sbias.w` of alpha. `sbias`: (1,
  0.0001), (0.5, 0.0003) and (0.05, 0.001) for the three cascades (the constants a fraction
  of the box's depth: 4, 11 and 37 units), (2.5, 0.01) and (5, 0.07) for the sub map; the
  alpha 0.392 (100 / 255) in all. The draws: 104, 177 and 669 (the cascades), 25 (the sub map),
  Barrel's: 160, 235 and 687.
- **The characters' map** (`mSubW2S`, `scSubSM`, `nSubSdw`, `sSubShadow`; pass 10, 25 draws
  into the 1024² map, all of the player's submeshes of her first level of detail, one draw each):
  the characters are in this map and not in the two near cascades (none of her 25 draws' index
  counts and shaders occurs among passes 7's and 8's draws; five of them (9402, 1530, 1098, 240,
  144) also occur in pass 9, the far cascade, which was not looked into). Its box is the smallest in
  the sun's view around a world-space box: its depth and its v extents are equal (220.8 at the
  gate) and its u extent smaller (144.5), the box's centre about 92 units above her feet, and the
  extents change with her pose (143.9 by 220.1 at the start of the walk capture, 175 by 273 at its
  later frames; the sun's way is the cascades'). The pass writes the depth as the cascades do, with
  `sbias` (2.5, 0.01) for the shadow shaders `@00012B6D` and `@03C5C6E3` and (5, 0.07) for
  `@D0A7806D` and `@456FF036` (17 draws and 8: the shader groups of the model's shadow pass),
  alpha 0.392. The pass reads it as 16 point taps (the sampler is a point one, clamped), four by
  four a texel apart from 1.5 texels before the point, each `sample < depth` shadowed with the
  depth saturated; outside the box (tested on the first tap's corner and the last's) it shades
  nothing; `nSubSdw` (1) layers are read and the larger shadow kept (`max`). It is read where the
  cascades' share is not already shadow, and in the fade into the static map it is mixed in with the
  cascades' term (`min` of the two lights before the mix). So her ground shadow is the sub map's
  alone, cut at the box's edge (the game's is shorter than the cascades' was), and her own pixels
  are shaded by her own map: the sun's share on them is 0.086 (the board's `characters` mask of
  the `shadow` stage: 0.114 mean abs against 0.099 with the map). Barrel's box is her draws'
  own box on the scene's starting joints grown by a constant (a stand-in: the rule that gives
  the game's pose-dependent extents is not found).
- **The filter.** Sixteen taps four by four, one texel apart from 1.5 texels before the
  point, each a bilinear depth comparison (`sample < depth` shadowed, no receiver bias); the
  sub map's loop is `nSubSdw` times, the larger shadow kept.
- **The share.** The pass's alpha is `saturate(lit - fOverallShadow)` times the cloud
  pattern's term; at the gate the trees come out almost fully shaded (0.01 on the left canopy,
  0.00 on the trunk, 0.12 on the middle tree, 0.43 on the grass in the clouds' pattern). Barrel
  matches that once the leaf cards are two-sided (above, "Two-sided materials"): 0.010, 0.002,
  0.112, 0.402.

**The far hills' fog.** The game fogs the backdrop's mountains as stage pixels (stencil 2) at
their own distance in the scene fog's first draw: at the gate they stand 12000 to 90000 units
away (mean 44900 over the right-hand strip), `s` = 1.44e-6 × distance - 0.00719 stays under
0.13, the extinction `exp2(s × Scat0)` keeps 0.89, 0.90 and 0.94, and the in-scattering adds
(0.04, 0.20, 0.33) to their (0.085, 0.118, 0.093) (the formula `fog_add × (1 - keep)`
reproduces it to 10%: 0.129, 0.343, 0.476 against 0.124, 0.314, 0.424). Barrel's far hills were
not haze: the projection's far plane was 80000 and the mountains past it were cut, so the sky
showed through where the game has hills (`look_for`: near 50 and far 300000 now, the
game's, from its depth terms). The hills are 0.45, 0.81, 0.74 in Barrel's scene against the
game's 0.13, 0.32, 0.43 over the strip: their unfogged colour is about 5 times the game's, so
what is left is not their fog. The hills are field models, `obj-A21-COMMON-MOUNTAIN-*` (class
3, the baked-in-place instance), not the sky pass's draws: the game draws them in its G-buffer
with the field models' own pixel program (draws 552 and 719 at the gate: colour, specular map
and normal map), so they are lit by the lighting pass like the rest, with the occlusion (0.79
there; Barrel's 0.82 now), and Barrel lights them as it lights a field model (flags: normal
mapped, specular map). Measured over the strip (1500, 330, 1800, 350), Barrel against the game:
ambient (0.235, 0.628, 2.91 against 0.212, 0.541, 2.47), colour (0.132, 0.133, 0.049 against the
G-buffer's 0.11, 0.108, 0.035), the sun's share 0.484 against 0.411, and the sun's Lambert term
0.49 against 0.10: that is the difference (the sun, 14.3 15 10.5, is behind the hills, the
game's normals face the camera: world (-0.09, 0.29, -0.64) on average, Barrel's mapped normals
(0.18, 0.74, -0.20) in game axes and its vertex normals' mean 0.82 up). The ambient and colour
are 10 to 40% high, the sun's term four times. Not the tangent frame's axes (swapping red and
green moves the strip by 3%); not found: where the game's normals come from (the vertex
tangent, which Barrel's vertices do not carry, or the normal map's decoding on these large
UVs).

**The lighting pass** (stage draws: `litCol` = the sun × 3, the second light × 1; the
characters' draw: the sun × 0.8, the second light × 0.487; the scales are the blocks' words
at +0x34 and +0x2c). With albedo `a`, specular colour `F0`, occlusion `o` (the colour
target's alpha), gloss `g`:

- `occ = a × fOccAlbScl + o (1 − a × fOccAlbScl)`: the occlusion tinted by the surface's
  own colour (1 where nothing occludes);
- diffuse = `a (1 − F0) × occ × (ambient + sun × N·L × alpha + second × N·L₂ + point lights)`;
- specular, per light = `light × N·L × (p + 2) / 8 × (F0 + (1 − F0)(1 − V·H)⁵) × (N·H)^p ×
  saturate((N·V + o)² + o − 1)` with `p = 2^(10 g + 1)`; the sun's also times alpha;
- the point lights come from a clustered list (16 depth slices, 64-pixel tiles), each with
  an exponential falloff.

No division by π anywhere: the set's light is the surface's brightness for a white albedo.

**The specular ambient pass** (after the screen-space reflections; one program, two draws
told apart by stencil: the stage's models, the characters). It reads depth, the three G-buffer
targets (specular colour `F0` in GBuf1, the normal and gloss `g` in GBuf2, the occlusion `o` in
GBuf0's alpha), the reflections at half size (`sRLR`) and the field's specular cube map
(`<FIELD>_as.g1t`, `sAmbSpc0`), and adds to the lit scene:

- `NV = sat(N·V)`, `specOcc = sat(o + (o + NV)² − 1)`, `F = F0 + (max(F0, g) − F0)(1 − NV)⁵`;
- `env = cube(reflect(−V, N), lod = vAmbParam.z (1 − g)) × vAmbSpcWgt.x × vAmbParam.y ×
  vAmbSpcScl`, then toward `sRLR.rgb` by `sRLR.a × vMerge.x`;
- `env` fogged as the scene (the height fog, then `mix(inscatter, env, extinction)`);
- `o' = o × vAmbOccRat.z + vAmbOccRat.w`, the added light `o' F env (o' + specOcc (1 − o'))`.

The constants at Rasenboden's noon: the stage's draw has `vAmbParam` (7, 1, 5, 1/256),
`vAmbSpcScl` (1, 1, 1), `vAmbOccRat` (1, 0, 1, 0), `vAmbSpcWgt.x` 1; the characters' draw
(1.5, 6, 5, 1/256), (0.6, 0.6, 0.6), (0.7527, 0.2473, 1, 0), 1. They are words of the set's
`indirectStage_` and `indirectChara_` blocks: `vAmbParam.y` is the block's word at +4 (the
specular map's scale: 1 and 6 at noon, 0.1 at night), `vAmbSpcScl` its
colour at +0x14, `vAmbOccRat.x` the word at +0x20 (the characters' 0.7527; `.y` is its
complement); `vAmbParam.z` is the cube map's mip count less one. `vAmbSpcWgt.x` is 1 in the
capture and not found in the set; `vAmbOccRat.zw` and `vMerge` are as captured. A field
model's `F0` and `g` are the G-buffer program's: `pbb-n_gb` writes `F0` = the specular map's
green (a metal, blue ≥ 0.5, takes the colour tile × (1 − green) instead, and its diffuse
colour is the tile × 0.2 × green: `shb629ba7124d0dec8`, as the characters') and `g` = its alpha,
each times `vSpcScl` (1 on every field draw of the capture). The terrain's program writes
the blended colour tiles' alpha as `F0` and the blended normal maps' alpha as `g`: the
ambient term adds almost nothing there, but the direct term (the sun is ten times the
ambient) does. The terrain's G-buffer program also takes its normal from the blended normal
maps: the map's `rgb - 0.5`, normalised, in the frame of the vertex normal `N` (the terrain
vertex program rebuilds `N` from two bytes and takes no vertex colour), with `T = (0, N.z,
-N.y)` normalised and `B = T × N`: `n = N × blue + B × red + T × green`. The bark's specular
map has `F0` 0.02 and `g` 0.2.

**The ambient occlusion** (passes 2 to 6, draws 912 to 916 of the capture, after the G-buffer:
they read this frame's depth and normals, not the frame before's). Draw 912 makes the full
size depth linear (`1 / (depth × vDepthParam.x + vDepthParam.y)`: near 50, far 300000), 913 takes
every other pixel of it into a half size target (960 × 540), and 914 is the occlusion
(`sh2c8cde6c41b0e437`) into an 8-bit half size target from that depth, the normal target
(octahedral, view space) and a 4 × 4 noise texture the game fills at run time (it is not in the
capture). Per pixel with view position `P = (uv × vViewParam.xy + vViewParam.zw, -1) × d` (d the
linear depth):

- the radius is `50 + 150 × saturate(d / 1500 - 1/3)` world units, which is `radius × 960 /
  vViewParam.x / d` pixels; steps = min(that, 6), the step `pixels / (steps + 1)`; where `54
  / step < steps` the steps become `floor(54 / step)` (at least 1) and the step `54 / steps`
  (`vRadiusParam.w` is the longest reach); under one step, and past the far plane (299910), the
  pixel is not shaded (occlusion 1);
- the tangent plane: each neighbour's position (the four, one pixel away) is slid along its ray
  onto the plane through `P` with the normal `N` when it lies in front of that plane's far
  side (`N·Q < 0`), and the screen axes' tangents are the shorter of the two sides of each
  pair (`Tx`, and `Ty` scaled by `540 / 960`);
- eight directions (`vRayParam.x`) round the pixel, the first turned by the noise's x times a
  direction's share of the turn (`2π × 0.125`); a direction's step in whole pixels is `step ×
  (cos, sin)` rounded to even, its start `noise.y × step` rounded likewise; its tangent
  `T = Tx × step.x + Ty × step.y` gives the surface's horizon, `tan = T.z / |T.xy| + 0.1763`
  (`vRayParam.w`: ten degrees' bias);
- along the direction, for each step: the sample's view position `S`, `d = S - P`, its slope
  `d.z / |d.xy|`; a sample inside the radius that rises above the horizon so far raises it, adding
  `(sin(new) - sin(old)) × (1 - |d|² / radius²)`; a direction counts when any sample was inside
  the radius or under the horizon; each direction's sum is saturated;
- the occlusion is `min(sum / counted × 2.5, 0.8)` (`vOccParam.xy`), written as `1 - it`.

Draw 915 copies the colour target (albedo and the material's own occlusion, from the G-buffer
program, in alpha) and 916 writes it back with alpha times the half size result, read through
a bilinear sampler: no blur of its own. The lighting pass (above) takes this alpha as `o`.
Measured at the gate: the occlusion is 0.66 on the left canopy, 0.46 on the middle tree, 0.74 on
the trunks, 0.92 on the grass, 0.91 on the sand, 0.79 on the far hills, 0.96 on the shaded
path: the pass's own AO is mild on the ground and strongest in foliage.

**What Barrel does of the occlusion** (Ryza's frames; `crates/render/src/ao.rs`,
`ao.frag`): the program is read as above (constants as captured, named in `ao.frag` and
`ao.rs`, which a test keeps equal), fed by a pass that draws the terrain, the field's models
and the distant view's solid pieces at half size into a world normal and linear depth target
(the models' and terrain's own fragment programs specialised to write them, so the normal
maps the programs read give the same normals; the game's G-buffer is that pass's counterpart).
The main pass reads the occlusion through the pixel's position and takes it as the lighting
pass does for the stage: all the diffuse light (ambient, sun, second light) times `a + o (1 -
a)` (`fOccAlbScl` 1 for the stage), the direct and ambient specular's `o` as it is, and a
character's own occlusion map times it. Stand-in: the noise texture (a Bayer matrix for the
angle, the start offset 0); at the gate the regions' mean occlusion is 0.74 against the
game's 0.66 on the left canopy, 0.48 against 0.46 on the middle tree, 0.76 against 0.74 on the
trunk, 0.94 against 0.92 on the grass, 0.92 against 0.91 on the sand, 0.82 against 0.79 on the
hills, mean abs 0.128 (0.157 of the game's own); the picture's structure matches and the
pattern of the noise is the rest.

**What Barrel does of the lighting pass** (Ryza fields; Escha's are as before): the field
models with a specular map and the terrain take the pass's rule per pixel: diffuse times
`(1 - F0)`, and the highlights of the sun (times its shadow and cloud light, the pass's
alpha) and of the second light, with `o` the screen-space occlusion (above). The
ground takes its normal, `F0` and gloss from the layers' normal maps and colour tiles as the
G-buffer program does, blended by the same weights (the bake's entries included), and its
ambient, second light and sun per pixel on that normal with no vertex colour; the colour
tiles' alpha is read as `F0` (they are sRGB views, so the alpha stays linear). The
characters, and the models without a specular map (the plants), keep the vertex light.
At the gate: sunlit sand 193, 195, 94 against the still's 217, 219, 114 (187, 188, 70
before), grass 150, 194, 47 against 171, 210, 64 (133, 190, 2); mean error 23.2 (25.9).

**Against the game's own pixels** (the pixel board, docs/parity.md; the gate at noon, the
frame's stages as the game wrote them):

- The units: the game's finished scene (the target before the tone pass) times the exposure
  0.9 goes through the tone pass to the game's picture (sunlit sand 179.8, 194.0, 83.7
  against the picture's 180.0, 194.1, 84.1), so the game's scene is Barrel's stored scene
  times 2 over 0.9.
- **The stage's ambient is the game's**: the nine harmonics of the diffuse light map times 7 and
  (0.1, 0.2602, 1), evaluated at the G-buffer's normals (octahedral, view space, turned by the
  capture's view-to-world matrix), equal the game's ambient pass (its output, rgb) to 1% in
  every band of the normal's height (0.176, 0.517, 2.765 mean against 0.173, 0.511, 2.763).
  The set's ambient colour is not added (`StageLight::ambient` is 0 at noon).
- Where the game's occlusion is 1 (the colour target's alpha after the ambient occlusion pass)
  and the sun's share (the ambient pass's alpha) is 0, Barrel's finished scene is within 3 to 5%
  of the game's (specular ambient included: it is a later pass of the game's, so the board's
  `lit` stage, which has it in already, reads 12 to 20% high in the shade). Where the sun
  reaches (alpha above 0), the sunlit sand and stone are within 6%.
- What is left in the lit scene, measured over the stage's pixels: the occlusion is built
  (tone error 22.8 to 20.6 on the board), the leaves (below), and the shadows' edges. The tree
  regions of the lit stage, with the occlusion: left canopy 0.044, 0.068, 0.054 against the
  game's 0.022, 0.042, 0.032, middle tree 0.071, 0.105, 0.057 against 0.064, 0.089, 0.033,
  trunk 0.036, 0.047, 0.049 against 0.019, 0.028, 0.031 (with the leaf cards two-sided, above:
  0.023, 0.044, 0.034; 0.061, 0.089, 0.037; 0.024, 0.034, 0.038; before: 0.053, 0.086, 0.067; 0.134,
  0.191, 0.102; 0.043, 0.059, 0.061). What the occlusion leaves is the sun's share (the canopy's
  0.034 against the game's 0.010, the trunk's 0.012 against 0.001: about a third of the
  canopy's red) and an ambient and albedo 15 to 20% above the game's there (the ambient on the
  leaf cards' vertex normals, the game's on its normal maps); the ground came 4% down (sand
  1.169 against 1.274, grass 0.552 against 0.570), as its occlusion (0.92) matches the game's.

**The tone pass** (the last full-screen pass): `c = scene × E` (the metered exposure), the
depth of field and distant blur, the 2D effects taken back through the curve and laid over,
`+ bloom × fBloomWeight` (0.4) `+ star + flare + light shafts`, `× vColorScale` (the set's
(1.27, 1.3, 1.2), the words `barrel` once read as a saturation), a saturation about the
luma by `vSaturationScale` (1 here), the filmic curve times `1 / f(fLumiMax)`, the gamma
(`1 / fGamma`), then `fSaturationScaleEx`.

**The exposure** is metered, from the program of each pass (read in the capture's frame, the
chain before the tone pass): the scene (the lit scene as it stands before the tone pass: sky,
fog and transparent draws in, no bloom) goes through the depth-of-field pass, which copies
it with the circle of confusion in alpha, and a pass that takes it to a quarter size
(480 × 270) times the last frame's E (one pixel read; E is held in a one-pixel float target),
two layers weighted by the circle of confusion (the first is metered). A
pass makes a 64 × 64 target of `log2(lum + fLumDelta)` (`fLumDelta` 1; `lum` by (0.29891,
0.58661, 0.11448); four bilinear taps a pixel), two passes average it by four taps (a plain
mean of four by four) to 16 × 16 and 4 × 4, and the last takes the mean of the 4 × 4 and
returns it as `exp2(mean) − fLumDelta` (a NaN is 0). A last pass eases and turns it into the
next E: `L = metered / E` (the scene's own luminance), the eye's `Lp = fMiddleGray / E`, the
rate `1 − (1 − fAdaptBaseRate)^(fFrameTime / fAdaptTime)` (0.95, 1/60 s, 3 s in the capture:
0.0165 a frame) times `fLightAdaptedScale` when `L < Lp` and `fDarkAdaptedScale` otherwise
(the set's, 0.5 and 0.5 at noon, the evening's second 0.3), `Lp += (L − Lp) × rate`, and
`E = fMiddleGray / Lp` (0.18 over it) clamped to the set's `vExposureRange` (0.9 to 2.4
at noon; 0.7 to 2.4 in the morning). The clamped E is what is kept, and what the tone
pass multiplies the scene by (the E from before the frame's own metering).
The still of the gate gives the rule's outcome: undoing its tone curve, gamma and colour
scale on its pixels, the metered average `exp2(mean(log2(lum + 1))) − 1` of the frame is
0.47, against the 0.18 an unclamped E would hold it to, so E is at the range's bottom, 0.9
(the same figure from Barrel's own frame at E = 0.9 is 0.47: the two frames are as bright
overall, and the sunlit sand's tenth is not the exposure). Only dark scenes leave the
range's floor.

**The bloom's bright pass**: `c × saturate((lum − T) / (fLumiMax − T))` on the metered
quarter-size scene, `T = fLumiThreshold × fLumiMax` (1.29 at noon), `lum` by (0.29891,
0.58661, 0.11448). **Its chain** (a 1920 × 1080 frame): a 13-tap diamond blur at a quarter
of the screen (weights: centre 0.18385795, the four at one texel 0.11151548, the diagonals
0.067637555, the four at two texels 0.024882466), a copy down to an eighth, the same
diamond there, then a Gaussian across and down (centre 0.19967563; pairs at 1.40733,
3.29421 and 5.20181 texels weighing 0.2973225, 0.091848336 and 0.010991331). The tone pass
adds the result times `fBloomWeight`. A star filter (twelve passes at a quarter) follows.

**What `barrel` does of this**: the sun at the stage's key scale, the cloud pattern
(`environment::Pattern`; its phase starts with the field), the exposure metered by the rule
above (`render::meter`: the scene is lit at the range's bottom, and the tone pass and the
bloom's bright pass take it to the metered E by the ratio of the two; a frame meters its own
scene, so no frame waits on the last, and the first frame of a view, and every offscreen
frame, takes the exposure its scene calls for, where a window eases toward it over the
adaptation's seconds; the pass is four small draws and a one-pixel copy; the depth of field's
weighting of the metered layer is not done: a plain mean of the scene's quarter). The gate
at noon, evening and in the attic's title backdrop meters to the range's bottom (the scene's
luminance 0.52, 0.50 and 0.93 where 0.18 would leave it), the morning to its 0.7 and the
night to 1.14 (luminance 0.157); the colour scale, the curve, the gamma, the last saturation, and the bloom's bright
pass and chain (`render::glow`). Against the gate's
still, fourteen spots' mean error falls from 14.5 to 12.8 levels a channel; the sunlit sand
and stone and the path in shade come within 10 (a shaded path 25, 57, 117 against 25, 57,
115). The field's models add the specular ambient as the pass does, in their own fragment
program (`model.frag`, `specular_ambient.glsl`; flag 1 << 24 on a model with a specular map):
the cube map from the set (`_as`, scale and tint of the stage's block), the specular map in a
set of its own, the fog on the light. Left out of it: the screen-space reflections (the cube
map alone), the occlusion (the screen-space one, above, as the pass's `o`), the terrain
and the plants. The characters take it too, with their own block of the set and their
own block's scale ("Her forward passes"; the metal rule is the same). Its light on bark and leaves is small, as the specular maps are dull
(`F0` 0.02), and the shade stays well below the still's. What it does not, largest first by
what shows: the reflections and what makes the shade bluer, the ambient occlusion's tint (built), the direct specular, the second
kind's ambient (which draws take it is not read; plants are likely), the bloom's star filter, the metering (night and interiors would rise above 0.9), the light volume,
the point lights, the height fog.

## Ryza DX: the sky

`skyplane.g1s` draws the sky as a full-screen plane at the far plane. Vertex program 0: the
corner `v0.xy` is the position (depth `fProjFarZ`), the view ray is `mC2W` applied to
(x, y, far, 1), and `fog.xy` passes on. Pixel programs 1 to 4 share a single-scattering sky
(program 1 without the glow, 2 to 4 with it; their differences are not read). With `d` the
normalized view ray (game space, y up) and `cosθ = d · -litDir`:

- **Path length**: `L = |-d.y × skyPos.x - sqrt(d.y² × skyPos.y + skyPos.z)| × fog.x + fog.y`
  (the distance out through a sphere of atmosphere from a point inside it).
- **Extinction**: `T = 1 - min(exp(L × Scat[0].rgb), 1)` (`Scat[0]` negative).
- **In-scattering**: `S = (1 + cos²θ) × Scat[2].rgb` (Rayleigh) `+ (cosθ × Scat[2].w +
  Scat[3].w)^-1.5 × Scat[3].rgb` (Mie, Henyey-Greenstein's form with the g terms folded into
  `.w`) `+ Scat[1].rgb`.
- **Glow**: `x = saturate(d · vGlowDir × hGlowParam.x + hGlowParam.y)`,
  `k = (1 - exp(-x²)) × saturate(cGlow.w)`.
- **Colour**: `S × T` mixed `k` of the way to `cGlow.rgb`; alpha 1. `litCol` is not used here.

**The constants, measured** (read from the running game's memory at Rasenboden's sunny noon,
where they sit in every lit draw's constants too; checked channel by channel):

- `Scat[0] = (-(rayleigh + mie) × log2(e), 1)`: the extinction, scaled for the shader's `exp`,
  which is base 2 (noon: rayleigh (0.02, 0.118, 1.0), mie 0.713 each, gives (-1.0576, -1.1990,
  -2.4715), as read).
- `Scat[1] = (0, 0, 0, 1)` there (the term added to the in-scattering).
- `Scat[2] = (rayleigh × 3/(16π) × c / (rayleigh + mie), -2g)`,
  `Scat[3] = (mie × (1 - g)²/(4π) × c / (rayleigh + mie), 1 + g²)`, with `c` the sun's colour
  (`parallelLight00`) times `skyFog_.power`: noon's (4.781, 5, 3.5) × 40 gives the read
  (0.3114, 1.6948, 4.8775) and (0.01025, 0.00945, 0.00321) to four figures; g = 0.9737.
  (The builder at 0x140b08b10 computes these; its Mie factor reads as `1 - g²` in the
  disassembly above, but the values need `(1 - g)²`, so a step was misread there.)

The coefficients come straight from `skyFog_` (docs/formats.md, "Fog and sky scattering"),
unscaled. The rest, as the game sets them at Rasenboden's noon (a captured frame's sky
plane draw):

- `fog = (1 / (end - start), -start / (end - start))` of `skyFog_` (1000 and 300,000).
- `skyPos = (-R, R², 2RH + H²)` with R = 6,378,000 and H = 10,000: the ground's radius and
  the atmosphere's height, the same in every set.
- `vGlowDir` is the direction a light at the set's `SkyPlaneGlowElevation` and `Azimuth`
  shines along (noon: straight down, so the glow is a haze along the horizon), `cGlow` the
  glow block's colour with `SkyPlaneGlowFade` in `w` ((2.46, 6.84, 10), 0.5), and
  `hGlowParam = (1 / (sin(EndAngle) - sin(StartAngle)), -sin(StartAngle) × that)` (25 and
  -6 degrees give the captured (-1.897, 0.8017)).

`barrel` sets all of these so (`environment::SkyGlow`, `play::look_for`), times its exposure.
The plane is stored at half like the rest of the scene (`sky.frag`'s `HDR_RANGE_INV`); against
the game's own pixels after that draw alone it is the game's to a few hundredths from the top
of the view to the horizon (unhalved it was twice the game's, a white band along the horizon
wherever the dome's clouds did not cover it).

The executable's builder (0x140b08b10, found by the phase
constants 3/(16π) at 0x1417927d0 and 1/(4π) at 0x1417a7378) fills four float4s at its render
object's +0x64a0 from a scattering object (`o`) and a light's colour `c` (zero without a
light), with `βR = o+0x160`, `βM = o+0x170` (RGB each), `g = o+0x17c`, `s = o+0x16c`; the object keeps its live values
at +0x58..+0x84 and copies them to +0x128..+0x17c each update (0x14035b200, 0x140543de0,
0x140546e20).


### Ryza DX: the sky, the clouds

Pass 17 of the game's frame draws the sky in five steps: the plane, the dome's two layers, then
**the large clouds** and the cloud particles. The clouds are a strip over the sky that shows a
cloud map ray-marched from the light, and they are what makes the sky white in places. Barrel
draws the strip (`clouds.rs`, `cloud_*.vert/frag`, the look block's slots from `CLOUD_SLOT`);
the particles are not drawn (below).

**The density map** (once per field, `cloud_density.frag`; the game's pixel program
`sh24132e3c`): a 1024 × 1024 map of tileable 4D Perlin noise. The pixel's `uv` becomes two
circles, `p = (sin 2πu, cos 2πu, sin 2πv, cos 2πv)` each pair scaled by its sampling range
over 2π (`vSamplingRangeU`, `V`), so the map joins at its edges. Eleven octaves (`nOctave`):
`sum += noise(p) × a; p *= 2; a *= vPersistence`; the output is `sum × vNoiseScale × 0.5 + 0.5`.
The noise is Perlin's improved one: a quintic fade, the lattice hash `P[P[P[P[x] + y] + z] + w]`
over a permutation table `P` (the game's is a 256 × 1 texture it reshuffles each run; Barrel's
is one fixed shuffle), and from the hash's low five bits a gradient: `u = h < 24 ? x : y`,
`v = h < 12 ? y : z`, `w = h < 8 ? z : t`, with the signs from bits 0, 1 and 2. The noise's
members come from the set's `cloud_` block: `vPersistence` is `CloudHeightNoiseSeedPersistence`
(0.56), `vNoiseScale` `CloudHeightNoiseSeedScale` (1.5), and the ranges `SeedFreqScale ×
SeedOctave` (0.96 × 8 = 7.68, as captured). The octave count 11 is the game's, not the set's
(`SeedOctave` is 8).

**The cloud map** (`cloud_map.frag`, `sh03e41bad`): per channel (four layers, alike at noon),
`x = saturate(d - vThreshold) × vSharpness`, `c = (1 - 2^-x)^vThinness × vHeightScale` (the
game's `exp` and `log` are base 2). The game makes it every frame; Barrel when its constants
change. Noon: threshold 0.6414, sharpness 8.365 (both read from a captured frame, see below),
thinness and height scale 1.

**The strip** (`cloud_strip.vert/frag`; `sh8a4c237c`, `shf2d21137`): a grid of 16 × 16 cells
(574 vertices as the game's one triangle strip, which splits each cell along the diagonal from its
(0, 0) corner to its (1, 1) corner; Barrel's is a list built in the vertex shader with the same
split, which matters because the parabola and the edge fade are only linear across a triangle: with
the other diagonal the strip's contribution to the sky was 0.24 from the game's, with this one 0.11),
`vPosOf` wide, centred on the eye in x and z; the height is a parabola in the distance `r` from
the eye's column, `y = vRange.w + vRange.z r²`. The map's coordinate is the grid's `(u, v)` times
the tiling (`CloudDensityMapFrequencyX/Z`, 1.2) plus the wind's offset, `-Wind × t × tiling /
gridWidth` (`CloudWindX/Z`, t the seconds since the field came up). Each vertex also carries
`cDif` (the set's cloud colour) with an alpha `min(1.5 (1 - min(|2(u,v) - 1|², 1)), 1) ×
CloudDensity`, which fades the grid out toward its edge; the pixel program takes tents
`max(1 - 4|a - c|, 0)` of it at 0.25, 0.5, 0.75 and 1 as weights over the map's four channels
(so a thin edge picks a faint value), and `alpha = dot(weights, map)`.

The march: the light's way `L` (the set's cloud light, below) takes the thickness (`CloudThickness`,
1050) down through the layer, `down = L × thickness / L.y`, whose x and z, scaled to the map's
coordinates (`tiling / gridWidth`) and limited to `CloudMaxRayTraceDeltaUV` (0.02) long, are the
whole step; 15 taps at 1/15 of it each add `clamp(map - i/16, 0, 1/16)` to a depth that starts at
`min(alpha, 1/16)` (the map is the cloud's height field: what stands above a ray at its height).
`lit = 2^(-|down| × depth / CloudAttenuationDistance)` (the extinction is the reciprocal of
`CloudAttenuationDistance`, 500), the light on the cloud `(litCol × lit + ambLit) × cDif`.
Scattering is the sky's (`Scat`, built as the sky plane's, below) with `cosθ = to-eye · L`:
`in = (1 + cos²θ) Scat2 + Scat3 × (cosθ Scat2.w + Scat3.w)^-1.5 + Scat1`, `s = in × lit ×
CloudScatteringAttenuationScale`, `T = min(2^(Scat0 × e^(-height / top) × fogDistance), 1)` with
`fogDistance` the sky's fog ramp on the clip `w`, and the colour is `T (light - s) + s` with the
alpha above, blended over the dome. (The game's pixel program also mixes in two height fog
layers from `cHFog`; both are zero at noon and Barrel does not draw the scene's height fog
either, so they are left out.)

**Where each constant comes from** (`environment::Clouds`, `Clouds::shader`; the test
`the_clouds_constants_are_the_games` compares them to a captured frame at Rasenboden's noon,
to four figures):

- the light is the set's second light, at 0x538: colour (35, 48.84, 70) and, from 30 degrees up
  and 30 round, `litDir`; `Scat` is the sky plane's with this colour times `skyFog_.power` as
  the sun (so the clouds' Scat rows 2 and 3 are not the plane's); the fog ramp (`fog`) is the
  sky's `1 / (end - start)`, `-start / (end - start)`.
- `cDif` and `vRange.y` are the block's colour and `CloudDensity`; `vTraceParams` is
  (`CloudThickness`, `CloudMaxRayTraceDeltaUV`, 1 / `CloudAttenuationDistance`,
  `CloudScatteringAttenuationScale`); `vTexOf` the tilings (`CloudDensityMapFrequency*`, then
  `CloudDensityNoiseMapFrequency*`, 4, whose layer the strip's program does not read) and the
  offsets above (the capture is at t = 1/60 s); `vTraceDeltaScale` the tilings over 64,000.
- **Stand-ins**: the grid's width 64,000 and its middle height `eye.y + 6000 + CloudOffsetY` (12,000
  over the eye at noon; the offset could also count twice), its fall `5/6` of that height at
  `CloudViewRange` (a ratio read from one frame; 5/6 is also 1 / 1.2, the tiling), the cloud map's
  threshold 0.6414 (a frame's value: the game moves it with a height noise,
  `CloudHeightNoiseChangeSpeed` 0.1 and `Scale` 0.05, whose rule is not found and whose series
  differs by run: it ran 0.6413 to 0.6560 over the capture's 800 frames; 1 - `CloudDensity` is
  0.65), the clouds' ambient light (zero in the frame). The sharpness is not a stand-in: it
  is `3 / (1 - threshold)` in all seventeen captured frames (0.6413145 gives 8.3638735,
  0.6560234 gives 8.721523).
- The sky's exposure folds into the colours as the sky plane's does.
- **The scene is stored at half** (`HDR_RANGE_INV`): the strip's colour is written times 0.5
  (`cloud_strip.frag`) as the models' and the water's are. It was written whole, which made the
  clouds twice as bright against the game's pixels (the sky's brightest pixel at the stage after the strip, in the game's
  units: Barrel 4.5, 10.0, 25.0 against the game's 2.6, 5.9, 14.0; with the factor 2.7, 6.2,
  14.7) and the bloom's bright pass three times the game's (its mean 0.067, 0.165, 0.398
  against 0.022, 0.052, 0.120; with the factor 1.6 to 1.8 times, the rest being the sky seen
  through the trees' leaves, docs/ryza/open.md). The clear sky (the plane and the dome) was
  right to within a few percent.

**The dome's program** (`sh1cd42d63d688dec3`, draws `1913` and `1914`): the diffuse light is
`albedo (1 - F0) (ambLit + ambient map vAmbParam.x vAmbDifScl + litCol N.L)` with the specular
map's green as `F0` and its alpha as gloss (shininess `2^(10 gloss + 1)`, scaled `(n + 2) / 8`,
Fresnel `F0 + (1 - F0)(1 - V.H)^5`, times the sun's light), the ambient map and the specular
ambient scaled by `vAmbParam` x and y, both 0 at noon. The normal is `T n.y + B n.x + N n.z`
(`B = cross(T, N) w`) and is not normalised; the sky's maps are black, so it is `-(T + B)`, 1.41
long. The blend is the plain alpha one, the depth test on and not written. Barrel's frame for
it is the mesh's tangent as the game's (it matches the screen derivatives' frame on the dome's
regular UVs to the fourth digit of the board's `dome` stage), and it read
a scene texture as the dome's normal map until the draw's normal-map index was moved with the
rest of the dome's. The stage dumps `plane`, `dome1` (the first layer) and `dome` take the
sky's draws one at a time.

**Not drawn: the cloud particles** (`shb50b0ba6`, `sh040b16cf`, `cloudpiece.g1t`, set block
`cloudParticle_`): billboards a CPU system places, in the frame read 49 quads (294 indices)
around 50,000 to 60,000 units out, a unit or so across (just spawned), lit by the same light
through their normal map. Their spawning, growth and fade rules are not read.

## Ryza DX: the water

The game draws a field's water in one pass after the scene (Rasenboden at the gate, frame 2550:
pass 13, 10 draws of 8,710 indices, vertex program `sh3b59ff800bd4a704`, pixel program
`sh307bbe2b1903717c`) into the G-buffer, then a full-screen ambient draw (the second kind, flat
`ambLit`) and the lighting draw (`sh3f2bb7102c69409c`) for the stencil-marked pixels. The ten draws
are the pieces of the field's water file (`docs/formats.md`, "Ryza field water"): 3 of the river and
7 of the sea (the draws' two constant sets match the two bodies').

The pixel program: the wave normal is the wave volume texture's (`wavevolume.g1t`, a volume)
sampled five times, offsets along the view, summed and tilted by the vertex normal; the
refraction direction takes the surface up with `vRrParam.y`; the water's thickness is the scene's
depth along it less the surface's (read at the refracted point from the depth map), and the
colour fades from the body's albedo toward the refracted scene by `1 - exp2(-thickness |dir.y|
vRrParam.w)`; the shore bubbles (`wavebubbles-sequence-a01.g1t`, by the depth over the shore)
are laid over it. It writes the albedo, the reflectance (`vReflectance`, gloss 0.125), the
octahedral normal and the fade; the lighting pass then adds the sun, the reflection
(`mRrW2T`, `vRrCameraPos`: a reflection target) and the caustics.

The lighting pass has no reflection of its own: `mRrW2T` is the camera's projection, used to find
where the refracted ray ends on the screen, and the highlights are the two directional lights'
(Schlick, power 2 at the program's gloss 0). What the pass writes for a water pixel is the
highlights plus `(1 - k)` of the body's albedo taking `(1 - F0)` of the diffuse light plus `k` of
the refracted scene, with `k = 2^(-depth behind the surface / fRrTransparency)`.

Barrel draws it forward (`water.vert`, `water.frag`; look slots from `WATER_SLOT`): each piece a
24 by 2 grid made in the vertex program from the piece's cubic and widths, with the body's
refraction (index less 1, transparency) in the `w` of the curve's last two terms. It is drawn
after the field's opaque scene with depth, and again in the distant view's pass, after the sky
plane and before the backdrop, with its depth pulled onto the far plane past it (the sea runs to
the horizon, far beyond the camera's far plane).

The scene pass is drawn in two parts when a frame has water and the occlusion pass ran (`ScenePart`
in `renderer.rs`): the opaque scene, then a copy of it (the refraction map) and the second part,
which loads the kept colour and depth again and draws the water and everything after it. The
water reads the occlusion pass's view depth (its alpha, at half size, the scene without the
water) as the game reads its depth map: the nearest of four texels, the thickness behind the
surface, the refracted ray (the view ray bent by `index - 1` along `up - normal`) walked that far
(at most 8 transparencies), the depth and the colour there (the copy; the surface's own pixel when
something is in front of the ray's end), and `k` from it. The lit part is the lighting pass's:
the cube map's ambient on the wave normal, the second light, the sun times the shadow cascades and
the light texture (`sun_light.glsl`, shared with the models and terrain), the body's albedo taking
`1 - F0`, the two highlights (`specular_direct.glsl`), at the scene's half brightness. The field's
fog takes the lit part; what is seen through the water has its own fog already. Without the
occlusion pass the water is opaque.

On the pixel board the water's scene values (game, Barrel) are: left view, the river's pixels
0.55 1.18 3.21 and 0.67 1.21 2.78, its region 0.281 0.496 0.829 and 0.323 0.527 0.764; the turn
view's sea region 0.335 0.333 0.471 and 0.325 0.309 0.441; the gate view's river region
0.259 0.760 2.295 and 0.270 0.735 2.149. The wave normal's shape does not move these means.

Not done: the wave volume's normals (a few travelling sines stand in), the shore bubbles
(`wavebubbles-sequence-a01.g1t`), the caustics' light on what is seen through, the piece's skirt,
the surface's scroll.

## Ryza DX: the plants (`ObjectGrassPBS.g1s`)

The field's plants (`_grass.bin`, docs/formats.md) are quads the game builds on the CPU,
drawn with `ObjectGrassPBS`. Vertex program 1 (the one with the pigment map):

- **Size and fade.** The vertex's `PSIZE` (the card's corner) is scaled by `g_fWidthScale` /
  `g_fHeightScale` (cb0[36]) and by a factor going from 1 down to 0 between
  `g_fFarClipBegin` and `g_fFarClipEnd` (cb0[39].x, .y) of distance from the eye; nearer than
  `g_fNearClip` (cb0[39].z) the card collapses to nothing. Between `g_fFarScaleBegin` and
  `End` (cb0[40].y, .z) it grows by `g_fWidthScale`-like factors (cb0[39].w, cb0[40].x): far
  plants drawn larger. `barrel` shrinks them the same way (`model.vert` flag 16384).
- **Sway** (program 0's own code). With f = 2π · frac(POSITION.x) · `g_fAttCache1` (the
  plant's phase, kept in its position's fraction), h the corner's height after the fade, k = 1
  − `g_fAttCache1`, s = sin(f + `g_fPhase`) · 0.5 + 1 and r = h · 0.01 · `g_vAttCache0`: the
  lean L = (r.x · s + r.z · (sin(f + Osc) − cos(Osc) · k), h, r.y · s + r.w · (cos(f + Osc) −
  sin(Osc) · k)), Osc = `g_fOscPhase`; the corner moves by normalize(L) · h − (0, h, 0), only
  where h > 0.01 (the top corners): the card bends on an arc and keeps its height. With the
  measured (a, a, a/2, −a/2) the top leans steadily toward +x +z (about 0.01 · a · √2 of the
  height: some 8°) and circles by a/2 · 0.01 as Osc turns. `barrel` does exactly this
  (`model.vert` flag 16384, sway slots 0 and 1), the strength held at 9.6. The height is the
  whole card's: the game's vertices all sit at the plant's foot, their `PSIZE` half the width
  to either side and 0 or the height up (64 bytes a vertex: the foot, a 15000 the program
  does not use, `PSIZE`, normal, tangent, the card's place in the atlas), and Barrel's cards
  are built the same (anchored at their middles they leaned half as far).
- **Pigment.** `sPigmentMap` (t9) is sampled at the plant's ground point, `(x, z) -
  g_vFieldRange.xy` over `g_vFieldRange.zw` (cb0[41]), clamped: a map over the whole field.
- **Billboard.** `mBbL2W` (cb0[12] to cb0[14]) turns the corner; the card's normal and tangent
  go out turned by it too.

The pixel program (3) alpha-tests the card (`vATest`), and its colour is the card texture
times `(COLOR0 × fPtn.x + fPtn.w) × cDiff × g_vPigmentMapScale.y + pigment ×
g_vPigmentMapScale.x` (cb0[7], cb0[6], cb0[23]): the vertex colour and the pigment map
weighted together, the pigment's share set by the game. Which texture is the pigment map and
the scale's values are set at run time (not read): `_grass.g1t`'s second texture, a flat
red, may be it (or a placeholder). It then normal-maps (`sNMap`, the cards' normal maps) and
lights as the field's models do. `barrel` draws the plants without the pigment, and sways their
tops by stand-in values (docs/ryza/open.md).

### Ryza DX: the plants' constants as the game sets them (measured)

Read from the running game at Rasenboden's garden gate, 10 AM, the High preset (2026-11-14,
`<reference>/ryza-gate-constants-60hz.mkv`; a 32 MB snapshot of the
constant ring is kept locally). The plants draw with `objectgrasspbs`'s 656-byte vertex block,
the variant without `g_vFieldRange`: no pigment map is sampled at the gate. Per draw:

- `g_vAttCache0` = (a, a, a/2, −a/2) with a, the sway's strength, 5 sqrt(2 p) for the wind's
  power p; `g_fAttCache1` 1.
- `g_fPhase` 0; `g_fOscPhase` turns sqrt(p) radians a second and wraps at 2π.
- The power is the field's wind, the environment set's `gust_` block (file 0x308 of a set:
  azimuth, elevation, `power`, `powerRandom`, `powerToMaxTime`, `powerToIntervalTime`,
  `powerMaxTime`, `intervalPower`, `intervalTime`, `intervalTimeRandom`, three more): it
  blows at `intervalPower`, after a pause of `intervalTime` and up to `intervalTimeRandom`
  more rises over `powerToMaxTime` seconds to a gust of `power` give or take `powerRandom`,
  holds `powerMaxTime` and falls back over `powerToIntervalTime`, moving between the two
  levels as half a cosine wave. Measured in Rasenboden at noon (1.5 between gusts, 2 and 0.5,
  2 s, 8 s, 1 s): a 8.660 between gusts, a rise of 120 frames fitted to 0.0001 at 30 of them,
  10.313 held for 480 frames, 60 frames down; the phase 0.024308 a frame at 10.313 and
  0.022368 at 9.490. `barrel_ryza::environment::Gust` plays it; which gust strength and
  pause the game draws, and where the cycle stands when a field comes up, are Barrel's own.
- `g_vEyePosition` the eye (32527, 6363, 29945 here).
- `g_fFarClipBegin` / `End` 8000 / 15000 (the Middle grass preset: as Barrel has it),
  `g_fNearClip` 10.
- `g_fWidthScale` / `g_fHeightScale` 0 (no growth far off), `g_fFarScaleBegin` / `End` 1000 /
  2000.

The lighting pass's blocks (`litDir`, `litCol`, `ambLit`, `vEye`, then `emScl` at +96) hold
the sun's direction (−0.354, −0.707, −0.612), `litCol` (4.781, 5.0, 3.5) with `ambLit`
(3, 3, 4) in one kind and (3.825, 4, 2.8) with (0.6, 0.6, 0.8) in another, and `emScl` 0 at
10 AM in those blocks. The models' own emission is another `emScl`, the G-buffer program's of
a material with an emission map (`sh8b7f9717e671dddc`: the map times `|emScl|` goes out as
light of its own, kept to 32): at noon it is 1 on the houses' window glass (0.8 to 0.85 on
five draws of one other material), so the glass glows a little by day too: in a wall's shade
the game's lit glass is (0.19, 0.28, 0.52), Barrel's (0.21, 0.30, 0.46) with the map at 1
and (0.04, 0.10, 0.38) without. `EMISSION_BY_TIME` takes the 1 at noon; the other times' are
a stand-in.

## Escha's maps: the screen texture filter

The world and area maps finish with `PostEffectTexFilter` (`Data/x64/PostEffect/pe_pack`,
entry 14): `s = scene × HdrRange`, `f` = `PostEffect/texture/overlay.g1t` (512 × 256 BC3:
black, its alpha a soft frame up to 83/255 at the edges, 0 inside), then
`s + (overlay(s, f.rgb) − s) × f.a × overlayModifier`, times `HdrRangeInv`, with the usual
overlay blend (`2sf` below 0.5, `1 − 2(1 − s)(1 − f)` above). With black, the overlay is 0 below
0.5 and `2s − 1` above, so the edges darken and spread apart in colour: the maps' red-orange
rim. `overlayModifier` is `A15_WMAP.xml`'s `<tex_filter_world>` (2) on the world map and
`<tex_filter_area>` (3) on an area map. Barrel lays it over in `post.frag` (binding 3,
`Frame::filter`; 0 for every other scene, Ryza included). On top of it the maps draw layouts
649 (a dark red vignette) and 650 (the diamond border). The map's own colour is its sun: `A15_WMAP.xml`'s
PARALLEL light reads `color="0.8, 0.7, 0.5` + line break + `5, 1.0"`, which the game's reader
(C `atof` per comma-separated part: the number at the start, the rest ignored) takes as
(0.8, 0.7, 0.5). Barrel's reader failed on it and fell back to white, leaving the maps paler
by about that colour (0.84, 0.75, 0.53); it now reads numbers the same way
(`field_look::leading_number`), and the area map's clouds and ground match the recording
within 3 levels. Other field files with such slips, read the game's way now: AREA_02_01
`pivot_position` (`140md` → 140), AREA_10_01, AREA_10_05 and EX_AREA_01–03
`center_position2_` (`0m` → 0), AREA_12_01 `boundary_max` (`0z.0` → 0).
