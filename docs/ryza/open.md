# Ryza DX: open problems

Everything Barrel is stuck on for Atelier Ryza DX, has not worked out yet, or knows to be wrong
(addresses are in `Atelier_Ryza_DX.exe`, which is not DRM-wrapped). Same rules as
`docs/escha/open.md`; problems in the shared engine are in `docs/open.md`.

## Status (2026-11-20): paused

The Ryza port is paused. Its aim was to run the game clearly faster than the original on a
handheld (an Adreno 740 under Turnip), and the measurements put its ceiling at about the
original's speed. At Low 1080p the original runs at 37 to 45 fps; Barrel at the Rasenboden gate
went from 51.9 ms a frame to 34.9 (about 29 fps; a little over 30 in play), and a 720p render
scale was not much faster: most of what is left does not shrink with the resolution (the
shadow maps, binning and each draw's work), so the GPU limits both. What exists: the opening's
field (Rasenboden, its terrain, models, plants, water, sky, lighting, shadows, reflections),
Ryza walking in it, the title and the opening event, at the scores in `docs/ryza/parity.md`;
not the menus, the HUD, battles or the rest of the story.

Left untried, each exact (the same pixels), with its ceiling where measured (the entry "On the
RP6 (2026-11-20)" below has the breakdown): a position-only vertex stream for binning, the
depth part and the shadow casters (the vertex is 72 bytes); fewer bytes a pixel in the scene
pass (20, so 14 bins); a min/max map so lit or shadowed pixels skip the shadow's comparisons
(the lookup is 7.7 ms). Together they would bring Barrel to about the original's speed, perhaps
a little past. Barrel also does not yet read the original's `Setting.ini` from a Proton prefix
or have Ryza's graphics options screen: launched with neither, it runs at High.

The original Ryza DX is never run through Steam without asking first (its playtime and
achievements are the player's own, and the DX version has not been played there).

## Most pressing (worked first; each entry stands on its own)

Ranked by what it blocks. Each says what is known, what is ruled out, where to start, and when it
is solved. ryza-recomp is ../ryza-recomp (its CLAUDE.md has the build and run notes); Barrel's
checks are `runchecks docs/ryza/checks.txt` (44 rows; frames are the exact capture's) and
`docs/escha/checks.txt` (17).

- **The boot with saves present** (B1). Done in 2D (2026-11-20): with saves found (`--saves DIR`,
  or the Steam library's own folder in a window) `boot::quads_with_saves` runs the game's
  with-saves boot (ryza-recomp `boot-saves`, frames 0-760 every 5, no input): the loading screen at
  5 (the load screen's layer), the notice at 7-128 ("Processing...", the load screen's
  `loading_notice`: fade in from 7, out from 113), the logos from 161 and 341, a black fade from
  522 (the game's `sys_white` texture is drawn black), the title from 543. Without saves the
  schedule is the old one. Rows `boot-saves-N` (docs/ryza/parity.md) are at 0 except two with a
  stated cause. Left, all stand-ins named in `boot.rs`: (a) the loading screen's frames (shown from
  5; the capture samples 0 and 5 and 10 only, so its end between 5 and 8 is a guess); (b) the title
  scene's first frames (draws 2091 at 545, 1014 at 550-580, about 500 at 585-595, 1922 from 600)
  are not built up: at 545 the game draws one more 2D quad (a full-screen `[0,0,1,1]` from
  shader `shb323822a9767bc95`, a pass of the scene's first frames; row `boot-saves-545` has 1
  mismatch for it) and the 3D draw counts are not compared (not seen: needs
  `barrel-parity draws ... --summary` on 545-600 against the scene's own first frames); (c) the
  pulse of the prompt (`title.rs` `PULSE_FROM`) counts from the no-save boot's start, as the
  existing `menu-saves-*` rows pass with it; its phase over a real with-saves boot is unverified.
  (Fixed 2026-11-20, 14b16f2: the boot screens showed the field behind them; now over black.)

1. **The title backdrop's cast: what is left** (P1). Ryza, Lent and Tao and the three
   weapons lying by them are drawn from the event `tt01_010` (docs/renderer.md, "The
   backdrop"; docs/events.md, "Timing and loops"): diff3d at frame 1400 matches 615 of 995
   draws, all three characters' bones within 0.012 units (rows `title-3d-poses-1400`,
   `title-3d-props-1400`), the event's commands pair with the game's log. Left: the cloth
   submeshes of the title's characters hang at rest, carried by their parent joint (built from the
   model's control points, docs/formats.md "Cloth pieces"; the field's Ryza runs the game's
   cloth solver, `barrel_engine::cloth`, and the title's cast could take it the same way; the pieces of sections 0x30002 and 0x30005, in 8 of the 37
   character models with cloth, are still drawn rigid on one joint); a head turn
   (`s88`) takes its first angle only, at once (the title's `s88 0 -7 0 0` is 7 degrees about
   the model's x axis in the head's bind frame; the opening's `s88 0 0 56 -4` and the timed
   `s90` are the way to the other two); `npc34a`'s two draws have no Barrel bones to compare.
   Of the 995 game draws at frame 1135, 962 have a Barrel match (row `title-3d-1135`). The 32
   without are the terrain, triangle strips in 32 patches (the pass's first draws; 24 by
   `sh0cf28a`, 8 by `sh499eb4`): the game draws every draw record of a terrain patch (`.ktf`:
   a layer or a pair of layers each, as a strip) on its own, where Barrel draws a patch once,
   its layers blended in one pass (16 patches in view; the strips are not checked against
   the file's records index by index). The Barrel draws
   without a game match (row `title-3d-1135` counts the matched): those 16, 2 plant cells
   (below, "Grass"), 5 of the characters' (the cloth submeshes), and 46 of the field's models
   that the game leaves out because they stand behind other scenery (docs/renderer.md, "View
   culling"; how the game tests what is hidden is not read, and nothing shows of it). One
   game draw more has no match: a bush just outside the view, out of sight. The title is
   drawn without draw ranges, a stand-in: with the field's own ranges 400 game draws go
   missing, so the title's ranges are not the field camera file's.

2. **The title's sparkles** (P1). The menu's own quads match the game's on every frame checked
   (rows `menu-*`), but the game also draws particles Barrel does not: a burst at the button
   (frames P + 1 to P + 31, up to 96 quads) and a trail for about a second after each move of
   the cursor (20 to 105 quads). They come from the effect player (`Saves/ez_eff_player/title.xml`,
   effect 1663 for the burst, started in `0x1400d7640`), drawn with unnamed textures between
   the prompt and the cursor. Barrel reads no `.g1e` effect yet (docs/formats.md, "Effects"), and
   the particles are scattered by the game's random numbers. Start: effect 1663's `.g1e` and the
   effect player's update. Solved
   when frames P + 10 and a move's frame + 3 diff to 0.
3. **The title's menu: what is not read** (P1). Stand-ins in `title_menu.rs`: the cursor's
   six-frame travel and the held direction's 37 / 15 frame repeat (the list control's own
   values), the perspective's eye distance (fitted 1340 to 0.015 pixels; a 30 degree view over
   720 lines gives 1343.5), the fade's two steps ahead, and Episodes always showing
   (`0x1405a2350`, the ownership check). Not captured: confirming an item, whether a direction
   counts before the cursor shows, and the sequencer's state 0xc (60 s without a button at the
   prompt, `0x140de7824`).

## The light set and weather on loading a save

Barrel arrives at noon and sunny (its default; right for the load-path autosave). The game's rule,
read from ryza-recomp `load-path` with `scenarios/hooks/environment.hooks` (all at frame 1728):
`0x1404214xx` picks a key: 6 if byte `[obj+0x17c]` is 1, else `0x140594d80` when `0x140595080`
allows (a story-stage short at `0x141a6c9c0`, 0 to 4, through the table at `0x141aa87d0` and two
lists of the data at `0x140069c00` (+0x48, then +0x90 entries of 0x20 bytes, word +0x1c, at most
0x1d, else 6)). `0x1406a8c40 (key)` returns `{weather, time set}`; weather becomes 0 when
`0x1404c81b0` is 1; `0x140727680 (env, weather, set)` stores them (+0x574, +0x578) and calls
the setters. The load-path save: key 6, set 1 (noon), weather 0. The save's `time` word (493) is
not read on this path. Where to start: the table behind `0x1406a8c40` (a data file's records by
key) and the two lists behind `0x140069c00`; solved when a save at another story stage arrives
with the game's set and weather. A field whose info record (table `0x141a940f0`) has byte +0x44
or +0x45 set takes `0x14037b669`'s fixed (0, 1) on its own path (`0x14037b590`).

## Field HUD

Built (`crates/ryza/src/field_hud.rs`, its layer drawn over the field in a run with `--save` or
`--hud`, not over the title or the load screen): the minimap's grid, terrain, her arrow, glass
layer, frame and north mark (`uil_a21_fm_minimap.xml`, `a21_minimap_image_006`), the basket's disc,
picture and count with the game's nine-copy shadow (the same file's `cage`), the equipped tool's
disc (`uil_a21_collect_item.xml`) and the clock (`uil_a21_time_display.xml`: disc, period picture,
plate, AM or PM, hour), positions from the layouts and pictures from `uis_gen_a21_field`, `_icon`,
`_a21v2_common`, `_a21v2_various`. The minimap's markers (2026-11-20: exits, talkable townsfolk, the story's destination with its
two additive glows; see Markers below). Rows `hud-2030, 2230, 2530, 2830, 3330` (hud-walk, 61
quads) at 0 mismatches; `hud-3130` at 3 (three zero-size quads the game draws, see Left).

What the capture shows (read, not in a table):
- The terrain picture is drawn twice over (uv 0 to 2, 1,024 layout pixels) and slides under her
  fixed arrow: its corner moves 1 layout pixel per 125 game units in x and in z
  (`TERRAIN_PER_UNIT`, `TERRAIN_ORIGIN`: fitted to the six frames, Rasenboden only, x was 120
  until the markers showed 125: the six frames differ by one constant 10.73 layout pixels; the game's
  per-map bounds are not in its tables). With Map Rotation Off (the default) the map does not turn
  with the camera; only her arrow does.
- Map Rotation On (`option_map_rotation 0`, `Play::map_rotation`, live on Yes; ryza-recomp scenario
  `map-rotation`, six `maprot-*` rows at 0, 3750 at the same 3 zero-size quads as hud-3130): the
  map turns clockwise on the screen by 180 plus the camera's yaw in degrees (the game's, 0 looking
  along +z; `field_hud::minimap_turn`, read off the frame's corners: yaw -1.00 gives 179.0, to 0.05
  degrees over six frames). The terrain and the markers' places turn about her arrow's middle
  (1729, 886 on 1080 lines); the markers' pictures stay upright. The frame turns about its own
  middle and the north mark about the frame's (1728.5, 888). The glass layer and the grid stay.
  Her arrow keeps its middle and takes the map's turn on top of its own (the quarter-turn flip rule
  as before). Parity runs give the camera with `--camera-yaw DEG` (an offscreen run with `--ryza`
  has no camera of the game's); the glows' blink there runs on the capture's frame plus 9 (`--hud-at`,
  the loop of 71 frames started 9 frames before the HUD's first).
- Her arrow turns about its middle by 180 minus her turn, kept within a quarter either side and
  with the texture coordinates flipped for the other half (all six frames).
- The minimap's mask is a stencil (`hud-walk` 2230, draws 1946-1955): the grid writes stencil 255,
  the mask quad writes 1 with no colour, the terrain, markers and arrow test equal to 1; the 2D
  shader drops pixels under 1/255 alpha. The tool disc's mask goes to a target of its own
  (cleared to 0) that the tool's picture is multiplied by (and the clock's pieces by its
  inverse). Barrel: `barrel_render::text::Clip` (the mask not drawn, the clipped quads only
  where its picture is not clear); the tool disc's mask hides only (the tool sits inside it).

Markers (built `field_hud.rs`, `play::minimap_markers`; the order the game draws them: the exits and
the townsfolk with the terrain inside the mask clip, then her arrow, the frame's pieces and the
north mark, then the destination and its two glows, over the frame and not clipped):
- A marker's centre is `(1729, 886) + (thing - her) / 83.33` pixels on 1080 lines in x and z
  alike (`MARKER_UNITS_PER_PIXEL` 125 game units a layout pixel; all 18 exit markers of the six frames
  to under half a pixel). Pictures come from the minimap layout's `clone_targets` nodes
  (`icon_fm_change_img`, `icon_npc`, `icon_map_change_destination_img` and its `_eff_01`,
  `_eff_02`: a style and an `image_no`).
- Exits: `gimmick::exits`, the field's `FIELDMAP_CHANGE_EX` rows within their story span
  (Rasenboden at the capture's save: fixID 13, 6, 9; 15 waits for `MM00_030`, 43 for `MM02_100`).
- Townsfolk: the people standing at the hour of the save's clock (the clock's own periods, not
  the scene's `--time`) whose `npcpeople` row says `talk="1"` (`Npc::talk`): NPC24A_00 and the two
  boys, 3 of the 22 standing. The walkers are not marked, and the rule explains it: Rasenboden has
  10 walkers (`move_data`) and none has `talk="1"` (8 people do, all standing), so a marker for
  `talk` people alone gives the game's counts (the 58 frames diffed against the draws have the
  same townsfolk quads as Barrel, 3 each, 0 mismatches). Whether a walker with `talk="1"` would be marked is not
  testable in this field.
- Destination: `mapmarker::current` (docs/tables.md, "Ryza DX: map markers"): a place of this
  field when the row has one, else the exit toward the row's field: each exit's destination is
  the first `<Value>` of its row's `<Parameter>` (`FIELDMAP_S08_T_BODEN`; `gimmick::Exit::to`,
  read from the text since the table reader drops it; Rasenboden's exit 6 is Boden, as the
  capture had it; 13 Livingroom, 43 Secretbase 02, 15 Highway 01, 9 Pasture). It sits on the ring of 80 layout pixels (`DESTINATION_RING`) when
  farther than that from the arrow, else where it is.
- Glows: `a21_field.g1t#1` pictures 2 and 3 (80 and 72 pixels), additive (`Layer::set_blended`, the
  render layer's second pipeline: source alpha, destination one), colour (155, 255, 255), both with
  alpha 0 until frame 5 of the loop, 1 at 10, 0 at 70 (`anm_icon_eff_blink_01` and `_02`); the first
  also scales 0.8 until 10 then eases out to 1 at 70. The loop is 71 capture frames
  (`GLOW_PERIOD`) with its start at frame -49 (`GLOW_PHASE`): fitted to the 75 glow quads of
  1740-2560, alpha within 1/255. `--hud-at FRAME` starts the pulse at a capture frame (the rows
  use it); a free run starts at 0, the game's own phase being the loop's arbitrary place.
- hud-3130 has three more game quads (64): zero-width rects at (1590.6, 557.9) of a texture
  the index does not name (`texb30d0f0369bea579`, uv 21 to 33 wide), drawn with a 3D matrix
  behind them: a 3D draw the diff counts as a quad, invisible. Not built.

Left:
- STAND-INs: the basket's count (225 in the capture), the clock hour from the save's `time` word
  (50 to the hour fits the one capture: 493 shows AM 10; the game's clock does not advance in the
  50 seconds walked), the clock picture by period (picture 2 for 10 AM; the others guessed), the
  tool (the first) and the arrow's middle (1729, 886 on 1080 lines, read off the capture).
- The popups are built (below); their STAND-INs are the field's names, the help bar's glyphs and
  its first showing.

The popups (2026-11-23, `field_hud`: rows `hud-popups-1750` ... `3710`, 23 frames of `hud-walk`, at
0 mismatches; 1740 differs only by the game's scene fade and lens flares; `--hud-arrival 1728
--hud-stick SPANS` give a still its arrival frame and the pad script's stick spans, the help
bar being replayed from them):
- The area name's banner (`uil_a21_fm01.xml`, `current_place`: `area_neme_bg` and `map_name_bg`
  plates of `a21_field#3`, the texts centred, `MainFont01` 23 and 20, colour 25 0 0 with the
  layout's `bold_size` 1 as nine copies at half alpha, then the text; drawn first of the HUD).
  Alpha is `anm_common_juvenile_fade_in` (`uia_common.xml`: 60 frames, cubic in, in whole 1/255
  steps) from the arrival frame (1728, the frame of the field's environment set), held to 151
  frames, then the same animation backwards; the map plate 30 frames behind (the layout's
  `fade_in_with_area` `delay` 0.5 s). The texts are the string table's: the area's
  (`2228225 + n`, "Kurken Island") and the map's (`917505 + n`, "Rasen District").
  STAND-IN: `place_names` has Rasenboden only (area 0, map 5). The game's own record is the info
  object at `0x141a940f0` (a run-time table: no file of the archives names a field's area or map):
  72-byte map records (area index at +0, the name's string number at +0x38) and 8-byte area
  records (string number at +0), read by `0x14019bbf0`; the file that fills it is not found, and
  the map names are not simply `917505 + field` after the town (917601 ... are others).
- The button help bar (the load screen's `uil_a21_shade.xml` and `gen_a21_cmn03_help`, a tiled
  plate of `a21_common#3`, right aligned at the screen's lower right): 7 entries "glyph + words"
  joined by spaces: Event Log, Full Map, Run, Photo Mode, Memo, Swing, "Menu / PC Menu" (the
  menu-define strings `4194353`, `4194351`, `4194338`, `4194341`, `4194451`, `4194559`; Photo
  Mode `720900`, one of three equal strings). It comes up 121 frames after the move stick's
  last frame outside its dead zone (`MOVE_DEAD_ZONE`; the pad script's frame reaches the field 2
  frames later), fading 1/15 a frame (`anm_common_fade_in`), and goes at the stick's first frame
  at the same rate, from wherever it is. The camera stick does not count; a pad-script stick
  of 6000 (inside the dead zone) does not either. At the arrival it comes up 6 frames after the
  banner (STAND-IN: the idle count starts at 114) with a short line ("Run Swing") and the
  shortest plate (376.5 px), the full line from 11 frames in and the plate grown over 16 more
  (STAND-IN: fitted to 3 frames, 1740/1750/1760; the game's own tween is not read: denser frames
  1735-1765 would settle it); later showings are full at once. Glyph characters are the
  capture's (STAND-IN, as the load screen's).
- Also: the glows are not drawn while their pulse is at 0 (`hud-walk` 1800).

## Field menu (Y in the field)

Built (2026-11-23, `crates/ryza/src/field_menu.rs`; its layer replaces the HUD's while the menu is
up). Answer key: ryza-recomp scenario `field-menu` (Y at 1900 after the arrival of `hud-walk`, five
D-pad downs 2000 to 2160, A on Settings 2220, B 2400, B 2550). Barrel runs it offscreen with
`--menu-keys 1900:y,2000:down,...,2220:a,2400:b,2550:b` (the pad's game frames, with `--hud-at`; keys
`y up down a b`); in a window Tab or Y opens it, the arrows or the D-pad move, Enter or A chooses,
Backspace, Escape or B closes (Escape quits only with the menu down), and the field stands still under
it. From the layouts `uil_a21_main_menu.xml` (buttons, level plate, money, corner picture),
`uil_a21_main_menu_chara.xml` (photo, cards), `uia_a21_main_menu.xml` (animations),
`uis_gen_a21_main_menu.xml`, `uis_a21_main_menu_image.xml` (the photo), `uis_gen_a21_mix.xml` and
`uis_gen_a21_icon.xml` (money plate and icon), `uil_a21_shade.xml` (the help bar).

What the capture shows (all read, not in a table):
- The menu's first frame is 6 frames after Y (`OPEN_LAG`; 1906). From it: the background fades in
  over 15 frames, the photo over 20 and the cards and the mask over 30 (`anm_common_juvenile_fade_in`,
  cubic, at speeds 3 and 2), the buttons slide in 12 frames each 2.4 frames after the one before
  (`anm_menu_slide_in`, `get_in`), the level plate and money 20 frames and the corner picture 30
  (`anm_common_slide_in_right`, outcubic, speeds 1.5 and 1), the help bar fades in from 34.
- The picture is `a21_main_menu_image_00` (image 0 of 26 by story progress: STAND-IN, the rule that
  picks it is not read) turned 2.01 degrees, the three cards at the layout's `list_0` places 0, 2, 3
  (a party of three), each `status_bg`, `battle_join`, HP, name and level text; level from the save's
  header, HP maximum is the capture's (STAND-IN `MAX_HP`: Ryza's save does not store it, 37, 57, 31 at
  level 3). The money is the header's. Draw order: background, photo, shadow, mask, the cards, the light,
  the buttons (base, glow, glow on, ring, name, icon), level plate, money, corner picture, help bar.
- The cursor: on the first button 30 frames after the menu's first frame; a move at the pad frame itself
  (no lag) takes the glow off the old button (9 frames) and on the new one (18, `anm_cursor_on`, then
  `anm_cursor_on_loop`: opacity 255 to 50 to 255 and scale 1 to 1.05 over 120 frames, which run 121:
  STAND-IN as the Settings blink). The ring (`bg_hilight_scale`) runs from the menu's first frame whichever
  button has the glow, times the glow's opacity. A on a button runs `anm_cursor_select` (speed 1.5) over
  the loop. Nodes at no opacity are not drawn. The cursor's order is the layout's (Container, Equip,
  Gathering Tools, Status, Guide, Settings; pictures 1, 3, 4, 6, 7, 8 of the name and icon lists: read off
  the uv), a move wraps (not tried: up, left and right).
- B closes with 1 frame of lag (2551): the background fades out over 15 frames, the cards and the photo
  back over 30 (cubic), the buttons go as they came in, one every 1.2 frames, the plates at 2.25 and the
  corner picture at 3 times the pace. The menu is gone when the background has faded (2566).
- The field's clock stands still under the menu: its banner (fading out when Y was pressed) goes on
  where it was (`FIELD_RESUME_LAG`, 2 frames after the menu's last), and the help bar comes up again as
  at an arrival (idle 114), its full line at once.
- A on Settings and B on the Options screen (2220 to 2440): B brings the cards and buttons in again from
  2402 (2 frames of lag), over the background and the photo, and the cursor's glow starts again 30 frames
  later (`Nav::back_from_settings`).

Rows (`field-menu-F`, local file `checks-menu.txt`, `PARITY_SKIP=tex` leaves out the sparks the game's
effect system draws from textures it builds at run time, the glow's ~40 sprites a frame; the parity
tool names an unindexed texture `tex<hash>`): 22 frames at 0 mismatches: 1930 to 2220 (opening done,
the five moves, A on Settings), 2500, 2570, 2590 to 2650 (the field back).

Settings from the menu (2026-11-23, `crates/ryza/src/options_run.rs`): A on Settings starts a `Run`
of the Options screen on a clock of its own (its first frame `SETTINGS_AT` = 43 frames after A: 2263 in
the capture), with the title's own `State`, `Leave` and `Blinks`: up and down (a held one repeats
30, then 8 frames: `RepeatUp`/`RepeatDown`), left and right, Q/E or LB/RB with the tab ask, B with the
leave dialog, A on Yes saving and applying live (the same `save_options` and `apply_settings`). The
menu's keys are the one log (`menu_keys`); the run takes the ones from its first frame on and tells
the menu when it is left (`Key::Return`, at its last frame; the menu's cards and buttons come in 2
frames later). Offscreen `--menu-keys` also takes `left,right,lb,rb`. The title still runs its own
copy of this key code (`play::run_once`, on the title's frame clock): the two are not yet one.

Settings from the menu, drawn as the game's (2026-11-23, rows `field-menu-2225` to `-2445`, `-2460` to `-2550` at 0):
- The menu goes out 41 frames after A (`SETTINGS_OUT_AT`, 2 frames before the screen's first): the buttons, plates
  and cards as on B (the background, the photo and the bottom-right picture `mm_put` stay), the Settings glow by
  itself over 9 frames (1 - out / 9), the help bar from A itself (`Bar::out`: words gone at +2.6, the plate
  shrinks to its shortest over 6.4 frames, the fade holds 5 frames then runs 15; the same on B, so one rule).
  Nodes at no opacity are not drawn.
- Over it the game draws the blurred picture of the menu: two passes on 960 x 540 targets (a 7-tap line blur each way,
  taps 1.33 px and 1.3 px apart, near-flat weights 0.1448, 0.1443, 0.1428, 0.1404), then a full-screen `sys_white`
  quad (shader `shbd81156d`) whose colour is that picture at the screen's pixels, white times a fade of 7.5 frames
  (fitted to 2265's 8/15), and the two vignettes (`a21_common` 1, alphas 0.59 and 0.35 at full, `field_menu::VIGNETTES`).
  Out again over the same 7.5 frames from the frame after the screen's last. Barrel draws these as the `backdrop`
  layer (made between the menu's and the screen's: the layers draw and the trace lists them in the order they were made).
  The layer's first quad is drawn as the blurred picture of what the layers before it drew (the renderer's `screen_blur`,
  `docs/renderer.md` "The blurred screen": the target so far scaled to half, blurred along x then y); its trace row is still
  the plain `sys_white` quad. A window whose swapchain cannot be copied from leaves the quad out (vignettes only).
  Pictures: Barrel's 2310 and 2370 frames against the game's backbuffers differ by a mean of 0.00033 of full scale
  (rows `field-menu-blur-2310` and `-2370`, local `checks-blur.txt`, tolerance 0.001), and Barrel's two 960 x 540 targets
  at 2310 against the game's dumps (`rt960x540`) by a mean of about 2.0 and 1.55 bytes (`BARREL_DUMP_STAGES` writes
  `screen_blur_x` and `screen_blur`). The Options screen's own background covers the picture at those frames; where it shows
  is the fade in (2263 to 2270) and out (2405 to 2412), which the capture has no backbuffers of. The taps' negative side is
  a STAND-IN (-2.0, -1.0, -1.0 px along y; the game's are not read).
- The screen over the menu differs from the title's: its caption comes down 30 layout pixels as it fades in (outcubic over
  30 frames from 1 frame before the screen's first) and is drawn after the page (`Moment::from_menu`); its help bar fades in
  from the screen's frame 3 (the plate 0.2 frames later); after B (nothing changed) the cursor's blink stands still on the
  key's frame, and after the last frame the caption goes out the way it came (`View::remnants`) with the bar going out
  as the menu's (until ~2435); the menu's bar comes in again from 33 frames after its return (plate 0.2 frames later).
  The ring's loop runs 120.75 frames (fitted: a frame ahead after 4 loops).

A window run (2026-11-23, a scripted pad: Y, five downs, A, down, right twice, B, down, A on Yes, B): the log reads
`menu: open`, `settings: opened from the menu`, the screen's keys (`[Down]`, `[Right]`, `[Right]` with `changed true`,
`[Back]` asking, `[Down]`, `[Confirm]`), `settings: saved from the menu's screen`, `settings: left`, `menu: closed`, and
the config file (`BARREL_CONFIG`) holds the values. Two bugs it found, fixed: a key a window stamped on a game frame the
screen had stepped already (it renders faster than 60 Hz) was dropped (keys are now taken by their stamps), and the
screen's last key with a fractional stamp sorted after the `Return` and closed the menu too (keys now sort by frame).

Help bar transients (2026-11-23, rows `field-menu-1910` to `-1920`, `-1940` to `-1950`, `-2575` to `-2585` at 0):
- The field's own bar (seven entries, its whole plate) stays under the opening menu for 3 frames after its first, then
  fades out over 15 (`FIELD_BAR_HOLD`; 14/15 on 1910, 4/15 on 1920).
- The menu's bar starts one frame sooner than the fade the rule had (`HELP_AT` 33: 1/15 on 1940, 6/15 on 1945, 11/15 on
  1950) and comes in with a plate wider than it stays: 198.3 layout pixels extra on its first frame, 33.1 on the sixth, none
  on the eleventh (`OPEN_PLATE_EXTRA`: STAND-IN, the three samples joined by lines; the game's tween is not read).
- The field's bar back after the menu closes: its fade comes a frame later than at an arrival (2/15 on 2575) with a
  plate that grows over 6.4 frames from 0.4 before the fade (0.375 of the way on 2575, whole on 2580;
  `HelpBar::returning`).
- The Settings glow as the menu closes on B goes out linearly from 1.17 frames after the buttons began over 8.5
  (`CLOSE_GLOW_AT`, `CLOSE_GLOW_FRAMES`: STAND-IN fitted to 170/255 on 2555 and 20/255 on 2560; no button's slide fits both).
  The glow's other parts fit exactly; the cursor part (the pulse's own alpha and scale) is one byte off at 2555 (the game
  has 0.667, Barrel 0.533: the game applies no pulse alpha then) and at 2560 (0.078 against 0.067, and its rectangle 0.9
  px wider): 1 mismatch each. Not read: what the pulse does in the game when the menu closes.
- 2150 to 2160 and 2510 show one mismatch each (a glow byte, 0.031 against 0.039; not rows).

Left (each a listed cause):
- The title's Options key code is still its own copy in `play::run_once` (not done in this pass: the title's offscreen
  frames replay scripted keys through `leave::replay` and keep their cursor holds in the title's own input record, which
  `options_run::Run` does not model; the 100 `checks-settings` rows hang on it, so it needs a pass of its own); only the
  menu uses `options_run::Run`.
- The other five buttons: A runs the select pulse and does nothing else (Container, Equip, Gathering
  Tools, Status, Guide are not built; the game's "not available" cue is not read). Left, right,
  X, Y, LB, RB and the other photos (ids by story) are not tried in the game.
- The photo image 0 only; the cards for a party of three only (`list_0`); no Gold Coin plate (the
  capture draws none); money and level from the save, HP a stand-in.
- A window's Tab and Y key are Barrel's choice (the game's keyboard map is not read, `docs/engine.md`).

## The load screen (title menu, Load Game)

Built (2026-11-20, `crates/ryza/src/{load,saves}.rs`, `title_menu.rs`, `play/mod.rs`): the button on
Load Game (`--title-keys F:choose`, Enter in a window) fades the item (196/255 to 0 in 13 frames),
turns the cursor one frame on and covers the screen black (1/18 a frame, full by 20); 160 frames on
a list of the saves comes up; a button on one (`F:ok`, Enter) starts loading it and 130 frames on the
run goes into the field from that file in the same window (`Handoff`: `run_once` returns the
chosen file and the window, `run` starts again with `<install> - - --save FILE --window`). Saves:
`--saves DIR` (else, in a window, the original's own folder in a Steam library's Proton prefix);
`barrel_ryza::saves` only reads. Checked: capture `load-path` frames 1400-1415 at 0 mismatches; the list at 1575-1600 (rows `load-1575` ... `load-1600` in checks.txt) at 2.

Not done, in order of what it blocks:
- **The load screen's drawing: done for 1560-1600** (`load_view.rs`, from `uil_a21_saveload.xml`,
  `uis_gen_a21_saveload/common/caption.xml`, `uil_a21_shade.xml` and the string table
  `strcombineall.xml`; the 2160-line style pixels are three to a layout pixel, twice a texture's).
  Frames 1575, 1585, 1595 and 1600 draw all 268 quads (233 before the help bar comes) at 0 mismatches but
  the game's textured full-screen quad (`fm_d02_barrel_02#1`) against Barrel's plain fill (2). Read off
  the capture: the background (3840 x 2160, uv 0..2) and the screen fade in over 15 frames from 1556, the
  info panel's own fade multiplies it (alpha = fade squared while the screen comes in); the cursor row's
  blink is `anm_common_blink` (120/255 times it) counted from 1556; the help bar fades in from 1576; on a
  button (1600) the screen fades out over 15 frames and the help bar from 1605. Fitted: the help
  bar's plate is one atlas pixel of its text wider than the advances (`help_bar`).
  `MainFont00` (heading, figures) is `font::MAIN_FONT_00_HD1080`, 1,774 records ending 8 bytes before
  the other's; party names are the string table's 262148 + the character's id (0 Ryza, 2 Lent, 3 Tao).
  Left of it:
  - **The help bar's plate moves** (done as a fit): the plate's width runs linearly between its shortest
    (376.5 px, 251 layout px) and the words' width over 6.4 frames from the bar's start (1576) (533 px
    at 1580 is 4/6.4 of the way, full by 1585); after the choice (1600) the words vanish at 2.6 frames
    and the plate shrinks by the same rule (533 at 1605, 376.5 at 1610), the bar outliving the screen's
    own fade by 5 frames (`PLATE_*` in `load_view.rs`). The layout files (`uia_*.xml`) hold no such
    animation: it is the game's code (a tween on the bar's width as its text changes); a one-line
    fit stands in. The words now come from the string tables (`str_menu_define`: Select, Submenu, Back,
    Confirm; the button glyphs stay the capture's), the caption's "Load" from `str_saveload_define`
    (4456470): the layout's `STR_UI_0028` / `STR_UI_0262` are empty in every table (the `.xml.e` twin
    decrypts to the same bytes), the game sets those texts in code.
  - **The cursor's alpha at the fade-out** (1605: 14/255 in the capture) looks truncated to bytes at each
    step; Barrel multiplies floats.
  - **A one-frame fade of the plate** and rows other than 2 are untested: the capture has two saves.
- **The notice (done, `Boot::loading_notice(word, t, alpha, screen)`)**: it is `icon_loading` of
  `uil_a21_saveload_progress.xml` (14 quads: the icon from `gen_a21_saveload02_load` and 13 letters; no
  pen) with the word "Please wait..." (`text_anime_waiting`) while the saves are read and
  "Processing..." (`text_anime_loading`) once one is chosen; the boot's "creating system data" is
  `icon_saving` and shares the code (the hop, `Boot::word`). A reusable piece: it takes the
  seconds since the notice appeared and its opacity. What drives it: the letters hop from the notice's
  first frame (`anm_hopping`, one letter every 0.1 s, the ten dots last; no restart between the two
  runs); the picture list steps on a schedule that is not in the layout (the icon has no
  `flick_time`): picture 1 to 23.5 frames, picture 2 to 54.25, then 3, 4, 5, 2 in turn every 12.25
  frames (`LOADING_PIC*`, fitted to the capture's two runs at 5-frame samples, so each edge is good
  to a frame). Its fade: in from 1448 over 15 frames, out from 1555 (a frame before the list)
  and from 1707; drawn under the help bar and over the rows. Not matched: at 1610 two letters
  near a quarter turn are drawn by the game with the quad's corners in another order (the same
  picture) and one is 1 px off; 1620 has 3 mismatches likewise.
- **The loading screen (1725)**: three quads at 0 mismatches against the game's (outside frame, inside
  frame, rolling text, from `uil_a21_loading.xml`); Barrel adds its black fill (1 mismatch). Left out
  after the first frame: the glows and the light ring (drawn additively, which the layer has no mode
  for) and the inside frame's swing about the vertical axis. Not built: the capture holds one frame
  of it (1725, then the field), with none of the glows in it, so there is nothing to match against; an
  additive mode would need a per-quad blend option in the render crate's text layer.
- **The black frames (1420-1445)**: the game draws a 9600 x 3240 quad of `fm_d02_barrel_02#1` (white
  colour, black texel) after a black fill the trace hides; Barrel draws the fill only: 2 mismatches a
  frame, the same pixels. The same two sit in every screen frame above.
- Which header bytes are the party: from +0x4e ten (id, level) byte pairs, `ff` for empty (the
  capture's three, Ryza 0 / Lent 2 / Tao 3 at level 3, fit; both saves hold the same party, so the
  id order is a guess). Names for the ids: the string table has them (not read).
- Selecting New Game, Extra, Episodes and End Game does nothing (Settings opens its own screen: the next section).

## Settings screen (title menu, Settings)

The packet named `uil_a21_system_option.xml` (textures a21_option); the screen the DX title menu
opens is not that layout but the PC game's **quick menu** (`ui_cmn/quick_menu/`): the list and the
description panel are `uil_quick_menu_settings.xml`, the tabs `uil_quick_menu_tab.xml` (its Steam
set of six tabs), the background `uil_quick_menu_background.xml`, styles `uis_gen_a21v2_common`,
`uis_gen_a21v2_various`, `uis_gen_a21_common/option/caption`, strings `strcombineall.xml` (the
rows' words are `str_option_define.xml`, `str_ui.xml`, `str_quick_menu_define.xml`).

- **Built (`crates/ryza/src/options_view.rs` draws, `options.rs` keeps the state, wired in
  `play/mod.rs`).** The button on Settings fades the chosen item as on Load Game, the black cover
  rises, and from 20 frames after the button the screen fades in (the background and the caption
  with the common fade; the rest is at full opacity by then): the two backgrounds, the caption
  "Options", the six tabs (Game opens; the lit tab is a second picture over the tab's own),
  the shoulder keys, the description panel (heading, text wrapped at 39 half-width characters,
  `<CLGR>`/`<CLNR>` colours), the list's window with its six lines, the cursor and the twelve
  rows (a choice with arrows on the cursor's row, a gauge with its number in a text ring), the
  help bar. The game builds two lists and draws both: the graphics list first, with the rows'
  template words ("Ambient Occlusion", "1920x1080", "0") and its scroll bar, then the plain list
  over it; Barrel draws both for the same trace. The parity tool now drops Barrel's quads that
  a later opaque quad covers whole, as it did the game's (without that every quad of the first
  list showed as an extra).
- **Scoreboard** (`options-N` rows, docs/ryza/parity.md): 0 mismatches at 1435 to 2100.
  `settings-tabN` rows: every tab from ryza-recomp `scenarios/settings-tabs.sh` (RB every 60
  frames from the open screen, pictures and draws at 1495 + 60 N: Game, Audio, Graphics,
  Keyboard, Controller, Camera; RB on Controller wraps to Camera). Barrel's `--title-keys F:rb`.
- **Two fixes read off the pictures (2026-11-21).** Each row's `sys_no_image` quad goes to a
  mask target in the game (rt `#7`), which the row's name is drawn through; Barrel drew it on the
  screen ("NO IMAGE" over every name) and the draw-list checks could not tell. It is now a
  `text::Clip` (kept in the trace, not drawn). And the parity decoder took a glyph's colour from
  the first copy of its corner in the joined strip, which still carries the glyph before's: the
  "a glyph late" colour rules were that. A button's picture is white, everything else the text's
  colour, at once.
- **Read off the other tabs (2026-11-21):** description lines wrap by width (`LINE_WIDTH`,
  fitted), `<KEY_ASSIGN_FIELD_CAMERA_MOVE>` is the right stick's picture, the Camera tab has the
  DX names and order (Y-Axis, X-Axis, Zoom, the speeds, Autocorrect), the cursor's blink starts
  again on a tab change, "Change" shows in the help bar only on a tab with a choice row, and the
  volumes start at 5. Camera, Game and Audio match (`settings-tab0`, `1`, `5`).
- **Graphics, Keyboard and Controller (2026-11-21), all 25 rows of `checks-settings.txt` at 0
  mismatches** (`settings-tab2` to `4`, `settings-scroll1620` to `2150`).
  - *Graphics*: 22 rows (Screen Resolution, Window Settings, Frame Rate Limit, Vertical Sync, then
    Overall Quality to Grass Density, Texture Filtering, Shadow Draw Distance, Water Quality, LOD
    Distance; no Motion Blur). The list scrolls: `State::follow` keeps the cursor inside the
    twelve rows shown, and the plain list draws the scroll bar's thumb (`a21_mix` texture 2,
    style `gen_a21_mix02_flame01_sb`) over its track when it has more rows than it shows. The
    four last items are `graphics::Item`s the config already had; the game's High preset has the
    anti-aliasing at FXAA, so `Settings::default` does (it matters only once the quality is Custom).
    A held direction repeats every 8 frames after 30 and stops at the last row (a press wraps).
  - *Keyboard*: the ten commands (strings 655914 to 655923) in the template window of the key
    assignment layout, a line under each, the cyan cursor (`gen_a21v2_cmn_cursor_s`) blinking
    twice as fast as the others', the scroll bar's bare track, and the description panel drawn
    after the list, empty (its heading's mask goes to the mask target as a clip with nothing under it, so the
    heading is plain brown as in the game's picture).
    A (open a key assignment) is not built.
  - *Controller*: the pad's picture (`a24_option_steam` texture 0) with the sixteen buttons'
    pictures (font glyphs U+04D8 + 51 pixels each in the atlas) and the cursor's glow on the first.
    The glow is added over the picture, as the game's (`Layer::set_blended`). The cursor moves over the
    sixteen buttons; nothing can be assigned.
  - *The cursor's blink* is the frame count since a frame before the last tab change, wrapping
    every 121 frames (`options::BLINK_LOOP`, replacing the 0.8% slow rate), with the clock at
    119.06 on the wrapping frame itself (`blink_clock`); on the Keyboard tab it runs at twice
    the speed. The rows' directions held before a scripted tab change count for the tab left.
  - *Display rows* (resolution, window mode, frame rate limit, vertical sync) are stored as
    `option_` keys; once the resolution or the window mode is moved off its default they also
    write the window's own `window WxH` and `fullscreen on|off` lines (Borderless and
    Fullscreen both mean on); back at the defaults the lines go back to the default window (1280x720,
    off). The frame rate limit and vertical sync are Barrel's own `frame_limit N|off` and `vsync on|off`
    (`barrel_render::settings::Pacing`), always written with the options (the game starts at 60 frames and
    vsync on); a config without them gets the rows' defaults (`options::GAME_PACING`: 60 frames, vsync on), so the rows always show what runs; Unlimited is one step on the row.
  - *Fitted* (each `// STAND-IN:` in the code): the thumb's height and travel
    (`options_view::View::thumb`), the Keyboard and Controller pages' places (`KEYS_*`, `PAD_*`),
    the help plate's extra atlas pixel(s) (one, and one more where the last glyph reaches past its
    advance), the zero-height middle's 1.5-pixel tiles (`segments`), the key list panel's one
    pixel shift, the draw distances' words (Long is the capture's; Close and Normal are the
    string table's nearest, unseen), and the names DOF (3.7 px) and Bloom (3.0 px) dropping like
    Vibration, Dash and Autosave.
- **Working.** Up and Down move the cursor (the title's holds, as the load list), Left and Right
  change the row (a choice wraps, a number stops at its ends), Q and E change the tab, Escape
  leaves for the title menu (after the "apply the changes?" dialog when something changed) and
  writes the changes to Barrel's own config (the graphics config
  file: `option_<key> N` lines, and the game's graphics keys once a graphics row was changed).
  The screen holds Camera, Game, Audio and Graphics rows; the graphics rows are
  `graphics::Settings` (changing an item makes the quality Custom, as the game's). **The
  render settings apply at once on Yes** (2026-11-23; below, "Live apply"): the renderer is rebuilt in the same process.
  The plants' density and the textures' mip bias are live too, and so are the display rows (2026-11-23, below).
  Unit test: `options::tests::a_change_goes_to_the_config_and_back`.
- **Left over (each kept and stored, not honoured, or not built):**
  - Keyboard and Controller tabs show the game's pages but assign nothing (A on a command, the
    pad's buttons), and show no key names. Texture Filtering, Shadow Draw Distance, Water Quality and LOD Distance are
    `graphics::Item`s the presets read. Frames the capture did not cover (the list's scroll back
    up, a tab's cursor after Escape) follow the same rules, unchecked.
  - **Audio, Camera and Game values that act (2026-11-23, `preferences.rs`; saved on Yes they
    apply at once, and at a start):** the BGM, Sound Effects and Voice volumes set the mixer's
    gain of their category (`Sound::set_volumes`, logged as `sound: volumes ...`; the effects
    play nothing yet, their codec being silent), the Movie volume is held for the video player
    (the game's videos do not play yet); the two camera inversions flip and the two speeds scale
    the right stick's turn and pitch (`walk::Feel`; the follow after walking is not scaled);
    the Confirm Button's second choice swaps the pad's A and B in the menus. Map Rotation On turns the field HUD's minimap with the camera (see Field HUD). Defaults change
    nothing. STAND-IN: the volumes' curve (row 5 = today's level, 6 dB a step down to -24 dB at
    1, silence at 0, 1.2 dB a step up to +6 dB at 10) and the speeds (linear, 5 = the measured
    rate) are Barrel's, the game's are not read; the volumes' range 0-10 and defaults remain guesses.
    Tests: `preferences::tests`, `walk::tests::the_camera_rows_flip_and_scale_the_right_stick`,
    `tools/options-effects.sh ENV_FILE` (a field walk with the right stick held, and the title's volumes line).
    Not live-checked through the screen's Yes (the apply path is the start path's `Play::from_config`).
  - Still stored only: Camera Auto-correct (its description, string 4522104: "Set to On to
    automatically reset the camera behind your character"). Measured (ryza-recomp scenario
    autocorrect-off, the hud-walk route with Map Rotation On so the minimap shows the camera's yaw,
    against map-rotation): with it Off the camera's yaw is the same to the last digit at nine frames
    over the walk, the follow's full sideways lean included, so Off does not stop the follow (Barrel's
    `walk::Camera::follow` stays on for both values). The reset behind her never happens on that route
    with it On either; what sets it off (standing still? a button?) is not found, so Barrel has none
    and both values draw the same as the game on every route measured. Zoom Controls Inverted swaps
    Barrel's zoom keys (= and -; the game's own zoom input is not read, `preferences::Play::zoom_inverted`),
    live on Yes like the camera rows.
  - Still stored only: Difficulty,
    Dash (string 4522107: hold the dash button to sprint, or press it to toggle sprinting; Barrel has no
    sprint yet), Event Autoplay, Subtitle Size, Autosave, Vibration, Item Auto Select, Save Order, Toggle
    Icons, Cursor Speed (each belongs to code the field or the events do not have yet). Their
    order and descriptions on the Camera and Audio tabs are the string tables', not captured
    (the capture shows the Game tab only); Difficulty offers five levels of the seven strings
    (Charismatic and Legendary left out, unverified).
  - Leaving: see "Leaving the screen" below (built).
  - Unexplained, fitted (named constants in `options_view.rs`): the names Vibration, Dash and
    Autosave sit 3.0 px (Autosave 3.7) lower than the others in the rows and in the description's
    heading (`NAME_DROPS`); the rows' pitch (`ROW_PITCH` 34.27 layout px) and the plain list's
    shift (`PLAIN_SHIFT`); the centred texts' fits (`CENTER_FIT`, `KEY_FIT`, `NUM_FIT`); the cursor's
    blink loop of 121 frames (`options::BLINK_LOOP`; in a window it is the frame count itself); the
    help plate's extra atlas pixels.
  - The title menu's chosen-item fade is now 0.878 of the pulse's opacity at the button times
    1 - frames / 13 (it was 196/255 flat): the Settings capture starts at 188 where the pulse
    stood lower; Load Game's three rows still match.

### Leaving the screen (2026-11-22)

B after a value changed since the screen opened opens the dialog "Do you want to apply the
changes?" (`barrel_ryza::leave`; layout `uil_quick_menu_notification.xml`, its `top` dialog: the
panel is 633 x 267, not the 695 x 382 of the `option` dialog; string 655372) with a Yes / No
box beside it (the balloon select: `uil_mt_balloon_select.xml`, window `gen_a21_window01_sub4`, its
cursor with the two cursor animations). The cursor starts on No; Up and Down flip it; B slides the dialog
out and stays on the list; A on No drops the changes and leaves; A on Yes writes them (`save_options`),
slides the dialog out, covers the screen in black over 30 frames (`options_view` draws the list under it),
shows the "Saving..." notice (the load screen's book and hopping word: `Boot::saving_notice`, drawn in
the load layer over the Settings layer) and leaves. B with nothing changed leaves at once. All of it
is read off ryza-recomp `settings-leave`, `settings-leave-gfx` and `settings-leave-no`; every time is
a title frame from the key (`leave.rs`):

- The dialog's panel and words fade in and rise by `anm_common_slide_in_bottom` (30 frames, an out-cubic
  of alpha and of a 30-pixel rise; the first frame is 1/30), the box fades in over 15 frames, and
  closing plays the same backwards (the box fades out over 15). The cursor's colour is
  `anm_common_cursor_base` from the dialog's opening (its glow `anm_common_cursor_add_75` is added
  over it) and goes plain white once the dialog starts to leave.
- The list's cursor holds its opacity while the dialog is up and goes on from there when it is gone.
  The help bar: the list's fades out from 4 frames after the key and the dialog's (Select, Back,
  Confirm) fades in from 33 after it; closing, the dialog's bar loses its words, shrinks and fades
  (from 3 and 5 frames after the key) and the list's comes back 33 frames after it. These delays
  are fitted to the capture's alphas (`LIST_HELP_OUT`, `LIST_HELP_BACK`, `DIALOG_HELP_IN`,
  `DIALOG_HELP_OUT`); the original's help bar keeps its own timer, and one reopening came a
  frame earlier than the other two.
- After A on Yes the black rises 1/30 a frame from 29 frames after the key, the notice fades in from
  31 and out from 137 (the common fade), and the list shows again until the screen is left 163
  frames after the key (`SAVED_AT`, the capture's). Its hopping word loops (the control's period is its
  last letter's delay plus the hop's second). After A on No the dialog slides out for 30 frames and
  the screen is left at once.
- The title comes back from black at 1/18 a frame, its cursor coming in (alpha 1 frame later) and
  turning on from the clock it had at the button on Settings plus 20 frames (`RETURN_SPIN`: both captures'
  icons fit it); the chosen item's pulse restarts.
- The keys offscreen: `--title-keys` takes `F:right`, `F:left` (change the row), `F:back` (B) and
  `F:ok` (A, also the load list's) beside the tab and direction keys; the screen's state is a function of
  the frame (`leave::replay` runs the keys again from the screen's first frame, the directions pressed while
  the dialog is up going to its box).
- **Parity**: `settings-leave*` (17 rows), `settings-leavegfx*` (6) and `settings-leaveno*` (4) at 0
  mismatches, but `settings-leave1800` (3): the hopping "Saving..." word's first two letters sit
  at exactly 45 and 135 degrees there, where the captured quad's corners tie and the capture's float noise
  picks one (the parity tool settles such ties as Barrel's trace writes them only with
  `PARITY_TURNED_TIES=left`, which the boot rows set and these rows do not); with it set the row is 0 (the
  other three notice rows, 1840, 1880 and 1910, are 0 either way). Add it to that row's command (or `checks.env`).

### The tab change asks too (2026-11-23)

LB or RB with a value changed since the screen opened opens the same dialog first (`Leave::tab`,
`Then::Tab`; the cursor starts on No); nothing changed turns the tab at once as before. Read off ryza-recomp
`settings-tabask` (`--title-keys` as in the `settings-tabask*` rows):

- **No** (A on No): the dialog slides out as in the leave, the changes are dropped at the turn and the tab
  turns 31 frames after the key (`TAB_AFTER_NO`: the frame after the dialog's last, as the title follows a
  leave). The changed row is back at its first value when the old tab is shown again.
- **Yes**: written and applied as the leave's Yes does (`apply_settings`), then the black, the notice and the
  list again as in the leave, and the tab turns 164 frames after the key (`TAB_AFTER_SAVE`, the frame after the
  leave's last). What was pressed while the dialog or the save was up (a tab button, a change) is dropped.
  The saved values are the screen's new first values: a later B asks nothing until something changes again.
- The turned-to tab's help bar fades in from 2 frames after the turn (`TAB_HELP_IN`, one alpha); a plain
  tab change leaves the bar's fade where it was.
- Each tab's cursor blinks on its own clock (`leave::Blinks`): it runs while the tab is shown and the dialog is
  not up, starts with the tab's first visit and goes on from where it was when the tab comes back (Game tab
  left at 1530 and shown again at 1700: clock 82). This replaces the earlier "restart at every tab change"
  (the old rows still fit it: they only visit each tab once).
- B in the tab dialog closes it and stays on the tab with the change kept; a second RB asks again (ryza-recomp
  scenario settings-tabask-b, rows settings-tabaskb1600-1930 at 0).
- A B with nothing changed runs the title's fade, cursor and spin one frame ahead of the screen's last frame
  (`Leave::lead`, from the capture's 2130 to 2160; the dialogs' leaves are fitted without it); the list's help
  plate of the screen also goes on fading over the title for 15 frames there (not drawn: `settings-tabask2130`
  has those three quads).
- Also fixed: a screen opened again after a session that changed something began with "changed" still set.
- **Parity**: `settings-tabask*` (25 rows, 1520 to 2200) at 0 mismatches. Not rows: the key frames themselves
  (1500, 1530, 1700, 1760, 1790, 2060, 2120: the original changes a few quads a frame after the key), 1570
  (the dialog's own help bar one frame behind the leave's fit, as the leave's one reopening), 1930, 1960 and 2030
  (one quad of the notice's hopping letters, probably the tie of `settings-leave1800` above: unchecked) and 2130.

### Barrel's own rows on the Graphics tab (2026-11-22)

Barrel's engine options sit in the game's page, among the rows they belong with (the user's choice:
integrated, not a launcher): Render Scale, Upscaler (Off, FSR 1, GSR 1, GSR 2, FSR 3.1) and Sharpening
after Window Settings, and the game's Anti-Aliasing and Texture Filtering rows extended with Barrel's
choices (MSAA 2x, 4x, 8x and FSR native AA after the game's Off, FXAA and TAA; 2x, 4x, 8x and 16x after
Linear). The rules are `barrel_render::graphics_options`' (Escha's page): a choice that would conflict
with another is closed (Left and Right skip it) and the description panel names the closed choices with
their reasons in red after the row's description (the game's own words for the game's rows, Barrel's
`graphics_options::description` for its own; Escha reads the same text). The mapping to the game's items
and the config: Off, FXAA and TAA are the game's item (its preset draws TAA from it); MSAA and native AA
set the item to Off and the render keys (`msaa`, `fsr3`); choosing a game level or an Overall Quality preset
clears Barrel's (msaa 1, no native AA). The filtering is the game's Anisotropic from 2x on, the count in
`anisotropy` (`presets::Graphics::apply` keeps it; it forced 16). `scale`, `upscaler`, `msaa`, `taa`,
`fsr3`, `gsr2`, `sharpen` and `anisotropy` go to the config once one of Barrel's choices was changed,
and the screen reads them back (the run's render settings before the presets). Like the game's rows they
apply at once on Yes (see "Live apply"). `--settings-as-game` (parity only) lists the game's 22 rows with the game's own
choices, as the captures have them. Unit tests: `options::tests` (the rows, skip-closed stepping, the
descriptions, the AA and filtering mapping), `presets::tests`.

### Live apply (2026-11-23)

After A on Yes (a player's, or the replayed key offscreen: `Leave::saved_at`) the run resolves the saved config
again through the one function the start uses (`play::Resolved::from`: render settings, the game's settings, the
tables' presets) and, when the render settings changed, `Renderer::apply_settings` (barrel-render) rebuilds what they
shape: render passes, pipelines, targets and their sets, occlusion, glow, upscale and temporal passes, the shadow
target, the sampler's filtering. The command pool, the descriptor layouts and the lighting ring are kept, the post
pass stays compatible with the text layers, and `SceneGpu::rebind` writes the sampler and the shadow map into the
scene's sets again. Temporal history starts over. The presets' per-frame values (shadow reach, bloom, level-of-detail
distances, draw ranges, grass fade) are read from a cell each frame, so they change at once too.
`tools/live-settings.sh ENV_FILE [CASE]` compares a live change with a cold start of the saved config (see its header).
When the shadows' settings stay, the rebuild keeps the shadow map, its static far layer (drawn when its square
last moved, the game's semi-static map) and the sun's share (eased from the value kept the frame before): made
again they started over and the picture differed from a run that never changed. Results (2026-11-23): the same
settings applied again mid-run, Render Scale 75%, MSAA 4x and FSR 1 at 75% draw exactly the cold start's picture;
the temporal ones (FSR native AA, TAA, FSR 3.1: history restarts) 43-47 dB. The title's event draws its waits and
actions from the clock as the game does, so the check fixes them (`BARREL_EVENT_DRAWS`); without that about one run
in five differs whatever the settings.
The plants' density: the cards are the scene's last vertices and indices (`PlantField`), laid again for the new
density by the function the load uses (`plant_cards`) and put in place by `SceneGpu::replace_geometry_tail` (new
buffers: the earlier geometry copied on the device, the tail uploaded); their draws replace the old ones at the same place
in the list and the cells' boxes follow. The mip bias: the whole textures are kept as views of the packs' bytes
(nothing is copied on a cold start), uploaded again at the new bias (`SceneGpu::replace_textures`, same count and
order) and the sets written again. Grass density and texture quality one step lower draw exactly what a cold start
does (AE 0), and so do two steps. One more thing a live change must reproduce: the static shadow layer is drawn once, in
the frame it is first asked for, with the casters the view and levels of detail of that frame show (`shown`); on a
change of the presets it is drawn again from that frame's view (`far_view`, `Frame::far_shown`; the title's boot has the scene
off in that frame). What is left of Quality Low (a level of detail change) is a few pixels in the trees' shadows
(51 of 518,400 pixels, 54.6 dB; tools/live-settings.sh passes `low` at 50 dB): the layer drawn again has the models in the poses of now,
not those of that frame.

### Display rows live (2026-11-23)

On Yes in a window run the display rows reach the open window: `Pump::set_display` gives it the new size
(`request_inner_size`) or borderless fullscreen (`set_fullscreen`, for Borderless and Fullscreen both), the
swapchain and targets follow through the existing resize path. Wayland has a catch: a configure the compositor sent
before it saw the request sets the old size back, so the pump asks again (up to three times) until the size it
asked for arrives. Vertical Sync and Frame Rate Limit are `Presenter::set_pacing` (vsync off remakes the swapchain
in MAILBOX, else IMMEDIATE, else FIFO, with no hold to the refresh; the limit sleeps the loop to its interval in
every mode). Both are read at each scene's start too. Offscreen runs have no window and ignore all of it.
`tools/live-display.sh ENV_FILE [CASE]` opens a real window and checks each case from the log line printed 120
frames after the apply (`display: inner, fullscreen, swapchain, present mode, mean frame`). Results (2026-11-23,
KDE Wayland): resolution 1600x900 -> inner and swapchain 1600x900; Borderless -> fullscreen, 3440x1440 (the
screen); back to Window -> 1280x720 windowed; vsync off with the limit Unlimited -> MAILBOX, 2.6 ms a frame; limit
30 (vsync off) -> 33.1 ms a frame. Note: with vsync on this desktop's FIFO swapchain ran the title at about 50 ms a
frame (the compositor's frame callbacks; refresh unknown on Wayland, so no MAILBOX), as before this change. The runs
were made with the session locked (`loginctl` LockedHint=yes), where the compositor throttles a window's frame
callbacks; the pace on an unlocked screen is not measured yet.

## Controller (Ryza)

Ryza reads the pad through a newer input layer than Escha's (the XInput calls are at
`0x140a3ee5b` / `0x140a3fc9d`; the pad object's vtable is `0x14178e8d8`; Escha's
`{slot, mask}` table is not in this executable). What each button does is read off the game
instead: ryza-recomp `scenarios/settings-pad.sh` (Settings opened from the title at 1460,
captures every 5 frames, `barrel-parity quads`):

- **RB / LB** change the tab right / left (1500 and 1560: Game → Graphics → next; 1620 back).
  The help bar slides its text over 10 frames after each change.
- **Left stick** (pushed past its dead zone) moves the cursor as the D-pad does: a tap at 1680
  moved it one row.
- **D-pad held** (1860 for 60 frames): the first step at once (it wrapped from the last row to
  the first), the next 30 frames later (1890), then every 10 (1900, 1910); a held step stops at
  the last row (no wrap). The stick held (1740) stepped the same way.
- **B** leaves Settings (2000: the title from 2005). A confirms (the title's menu, the load
  screen's save, as before).
- Not tried in the game yet: D-pad / stick left and right (they change a value, and the run's
  saves must stay as they are), X and Y on this screen.

Barrel's Ryza screens before the field read the keyboard and the pad as one menu input
(`barrel_ryza::menu_input`, `Pump::set_menu_pad`: the pad does not stand in for keys there). A
scripted pad (`BARREL_PAD_SCRIPT=file`, lines `<60 Hz game frame> <button> <frames held>`, buttons
A B X Y LB RB LT RT UP DOWN LEFT RIGHT START BACK and `LX=v` / `LY=v` stick axes) feeds the same
place as the real one; Settings prints `settings: tab N row M` (N from the Game tab) when either
changes, and `settings: left` on leaving.

| Where | Button | Does | Barrel |
|---|---|---|---|
| Title | any pad button, Enter | leaves "Press any button" | yes |
| Title menu, load list | D-pad, left stick, arrows | move (a press wraps; held 37 frames, then every 15, stops at the ends) | yes |
| Title menu, load list | A, Enter | choose / load the save | yes |
| Load list | B | back to the menu | no (the keyboard has none either) |
| Settings | D-pad, left stick, arrows | move (a press wraps; held 30 frames, then every 10, stops at the end) | yes |
| Settings | left, right, arrows | change the row's value (a press only; a held repeat is not read) | yes |
| Settings | LB / RB, Q / E | previous / next tab (stops at the ends) | yes |
| Settings | B, Backspace, Escape | leave: asks "apply the changes?" first if something changed (B closes the dialog, A answers, up and down move its cursor; Yes writes Barrel's config) | yes |
| Settings | X, Y | not tried in the game | no |
| Field | left stick, right stick, buttons | as before (`take_input`, `sticks`) | unchanged |

The stick counts as a direction from 0.4 and until it falls under 0.2257 (Barrel's choice, see
`docs/engine.md`, "Deliberate differences"); the game's threshold is not read.

## Sound cues (the game's own requests)

ryza-recomp `scenarios/load-cues.sh` with `scenarios/hooks/sound.hooks` logs the game's sound
requests over load-rasen's path; `tools/ryza_cues.py HOOKS_LOG stream.xml sfx.xml` turns the
log into cue lines (`<game frame> <kind> <hash>`), the answer key (`work/scenarios/load-cues/cues.txt`):

```text
545 music a5ba0cbd    BGM00054_SY_02_TITLE01 (title from 543 with saves: title frame 2)
641 voice 8c7effb2    the title call, asked for at 545 with a 1.6 s delay
1235 effect a699d027  SE_ETC_008_PRESS_START
1400 effect 09c3999b  SE_SYS_001_CLICK (Continue)
1600 effect 09c3999b  SE_SYS_001_CLICK (the save)
1725 stop -           the load stops every channel
1751 music 03bea3b2   BGM00003_FM_03_ISLAND01, Rasenboden by day
```

- **The sound manager** is the object at `[0x141b7a588]` (vtable `0x1413da2e0`). `+0x20`
  (`0x140762860`) plays a request: word 0 the hash, word 2 a delay in seconds, word 3 or 4 a
  volume. `+0x38` stops (first argument a channel, 0 to 7). Every hash is a stream id (the
  KTSR chunk id, `barrel-tool ktsr-ls`) or an sfx.xml row; voice clips have no table.
- **Music numbers.** Music is asked for by number (`0x14031f700`, `0x1403200a0`): 5000 + the
  tag's number, so 5054 is `BGM00054_*` and 5003 `BGM00003_*`. The number's hash is the
  stream.xml row whose tag starts `BGM<number - 5000, 5 digits>`.
- **A field's music** is the field's row of a static table in `.rdata` (rows of 0x48 bytes,
  one per `FIELDMAP_*` number from 0: 101 rows from `0x1411e01f0`, found by their ids): +0x04 the
  day music, +0x08 the night music (numbers, -1 none), +0x38 `0x000e0000 | (map + 1)`, +0x40 a
  pointer to the field's Shift-JIS name (`☆ラーゼン地区`). Rasenboden (map 5): 5003 by day, 5004
  by night. The field picks the night one when `0x1405a0b80` (the time of day's period) is 2
  to 4, at half volume for period 4 (`0x1404d38d0`). Map 12 becomes 13 when flag `0x6e` is set.
- **Effects** come from the `SE_*.ktsl2asbin` banks, loaded whole at the start (so file
  reads never show them).

Barrel's side (2026-11-20, `crates/ryza/src/{sound,music}.rs`, `crates/formats/src/ktsr.rs`):
`BARREL_TRACE_CUES=FILE` writes the same lines. One offscreen run steps the whole path and
matches the key exactly (`--title --title-saves --saves DIR --title-press 692 --title-keys
857:choose,1057:ok --into-field --frames 1260`; the log has the 7 lines above, plus a
`<frame> effect 5e3dae22` per cursor step). With saves the title's music and voice are at 2 and
98 (without, 18 and 106). The press, the button on Load Game and on a save, and the load's stop
are the menus' own frames (`load::confirmed`, `load::stop_frame`); the field music follows 26
frames after the stop (the key's 1725 to 1751), read from the executable's table (found by its
ids: `music::field_number`) and `stream.xml`. In a window the music and the voice play
(`Library` finds the stream by id in `bgm` / `voice`); effects are looked up in their bank
(sample, rate, channels, length) and stay silent. Left:

- **Effects are silent.** The banks' codec `0xd2db2960` is not decoded (docs/formats.md).
- **Day and night** is hours 6 to 17 against the rest (`music::is_night`, a STAND-IN for the
  period rule at `0x1405a0b80`, and the half volume of period 4); map 12 to 13 with flag `0x6e`
  is not done; the music's volume and fade are the mixer's plain ones.
- The window sound (`SE_SYS_005_WINDOW`) and the other menus' sounds are not placed; the cancel
  and page effects are asked for in the Settings screen only, in a window (not in the key).

## Event commands against the game's own log

The game runs every command through per-subject queues (`0x140489410` steps a runner's queue:
the event's own `u`/`w`/`p` words, characters' `s`, the camera's `t`, the `q` loads), and
ryza-recomp logs them with their frames (`scenarios/hooks/commands.hooks`,
`tools/cmdlog.py`). `barrel-tool ryza-commands INSTALL EVENT [DRAWS]` prints what Barrel's
player takes and `tools/cmddiff.py` pairs the two per group (docs/parity.md, "Event commands").
Read from the game and now Barrel's (docs/events.md, "Atelier Ryza DX's events"): waits
count down in 32-bit floats (some a frame short), the title's loop (a label, a jump, a number
drawn from the clock, a jump by it), the random wait, and an event waiting on its fades.
With the game's two draws given (`18.1,2`) the title's event pairs 66 of 69 commands and the
opening's first 132 of 135, each group at one offset. Left:

- **Loading is not timed.** The game's event runs its first commands, then loads its
  characters behind `p1 10` polls (title: 874 to 917; opening: 1601 to 1667, its clock from
  1699), and only then places them. Barrel places them at once, so its early commands sit 40
  (title) and 64 to 95 (opening) frames from the game's, a constant after that
  (`actors::EVENT_START` is fitted to it).
- **How long a fade holds the event** is fitted to three fades (`event::fade_hold`: 3, 32 and
  94 frames for 0, 0.5 and 1.5 s; the picture's own fade is 30 and 90). The game's test is
  behind `0x14050fdb0` (to `0x140509f30`), not read. Rules tried that give 93 for 1.5 s: the
  fade's frames plus a fixed 2 or 3, and a float sum against the length plus 1/30 s.
- **The prompt's slide-in starts a frame late, for a reason not read.** Frame by frame the
  game's prompt is the layout's curve a frame after its `delay` (`title::SLIDE_LATE`, rows
  `title-prompt-in-1000` and `1010` at 0, with the fade's rows `title-fade-900`, `930`, `950`).
  What makes the layout's animations start a frame after the title's first frame is not found.
- **The draws are the game's only when given.** The pick is seeded by the clock's seconds, as
  the game's; the wait's generator is the game's shared one (`0x1406ae6b0`), which Barrel
  does not follow, so a parity run gives the numbers (`BARREL_EVENT_DRAWS`).
- **`s69`** carries -4300 in its 14th operand in the game's queue where the script has 0, and
  `s36` (wait for the motion) takes no time in `ryza-commands`, which gives the player no
  motion lengths (the play does).

## File bytes Barrel never uses

`barrel-tool unread` over a sample of the archives (`tools/unread-survey.sh`, docs/parity.md)
and one field's files (`s01_i_attic`):

- **The environment file (`<field>_env.bin`): 97% unused** (95,232 of 97,920 bytes, over
  every time of day and weather Barrel reads). Barrel takes the fog, the stage light, the
  character light and the sun; the rest holds what several stand-ins guess at (exposure,
  saturation, the sky's scales, emission by time of day, the wind: `tools/standins.sh`). The
  largest known gap between the game's data and what Barrel reads. Nearly all of the file is
  now placed against the game's structure (docs/formats.md, "Ryza field environment": the
  running game's 12 sets read by a hook and matched to the file's by value), so each block
  the XML reader names has its place in the file. The wind is read (`gust_`, `environment::Gust`:
  docs/shading.md). Left: the members' names and types inside
  each block (the XML reader's per-block functions name them, from `0x14071d000`), then
  reading them in `environment.rs` in place of the stand-ins; and the pieces the values could
  not place (file 0x614 to 0x674, the shadow block's single words).
- **The placement list (`.opl2`): 68% unused**, nearly all empty slots (the 8 collision records
  of a type with no collision files); really unread are 28 of the 32 bytes after an instance's
  tags and 3 floats a collision record (docs/formats.md).
- **The field's shapes (`_coll.bin`): 18%** (32 of 180 bytes in 4 ranges, not looked at).
- Animations (`.g1a`, 45 files): 12% unused, all of it padding to 16 bytes and header words.
  Models, textures, events, tables, the terrain (`.ktf` 1.3%, `.khm` 1.3%): under 2%.
- No parser to ask: `.g1s`, `.g1pf`, `.trmd`, `Oid.bin`, and the collision primitives
  (`_cbox`, `_ccap`, `_cplh.g1co`: the shared `g1co` reader takes only their header).

## The answer key's runs repeat exactly

ryza-recomp's scenarios run on virtual clocks by default (RYZA_DETERMINISTIC): each frame is
1/60 s and every read a thread makes within a frame returns the frame's time (only past 64
reads in one frame does a thread's clock creep, 1 µs a read, so a loop waiting on the clock
still ends); the null XAudio2's passes follow the frames presented; and Present waits until no
other game thread is at work, so what a frame set going (a load) is finished before the next
frame. A thread in a blocking wait is not at work, nor is one spinning on SwitchToThread: the
game's wait for a resource (0x140728dc0, 83 callers) loops on a yield until another thread,
often the presenting one, has done its part. Two title-cycle runs and two opening probes give
the same captures frame for frame, paced or not, so scenarios run unpaced: the 14,000-frame
opening in about 2.5 minutes for 4.5, a boot to the title in seconds. The boot is 265 frames
shorter than in the first captures (its loading no longer runs across frames in real time).
Timeouts (Sleep, timed waits) end on a clock of their own (ryza-recomp d155a88): in Present
up to the next display refresh, else when every thread is idle, the earliest first and one
thread at a time; a thread woken (an event, a lock, a futex, a start) counts as at work until
it runs. On real time they ended wherever the host got to them, which moved loads by a frame
between runs. Load Game (scenarios/load-rasen.sh, from a save in Rasenboden) and the opening
now repeat frame for frame too. The recapture starts the boot a frame later (the notice at
12) with the title where it was; it also ended the one glow quad at frame 825 that no time
could match (the game's value there came with the old timing, not from a curve Barrel lacks).
Left: the runtime's timer and thread-pool threads still fire in real time (none seen to matter
yet), and a thread that computes for a minute without a wait makes the frames stop waiting
(a line in the log names it).

## Loops that take 50 or 51 frames

The game adds `speed x frame time` to a looping animation's time each frame, in 32-bit floats,
and restarts it once the sum reaches the length (`boot::loop_time`). For the boot spinner's
glow (0.6667 s at speed 0.8, 50 frames' worth) the sum after 50 frames sits within a few
units in the last place of the length: at a frame time of 16,666,670 ns or less the loop
takes 51 frames, at 16,666,671 or more it takes 50 (the same for the title prompt's 0.75 s
blink: 46 or 45). The game's frame time is `now - last` of QueryPerformanceCounter (read at
0x1406b94ab through 0x140b8c9f0, stored at +0x80 and +0x84 of the object in rbx, capped at
+0x88). ryza-recomp's old virtual clock added 1 µs for each read by any thread, so the frame
time wandered by a microsecond either way (16,665.7 to 16,667.7 µs over frames 900-1105,
`RYZA_TRACE_CLOCK`) and the loops took 50 or 51 frames by turns: the "one frame ahead each 105"
of `boot::GLOW_DRIFT` and the title rows' times 9.30 and 10.9667 s (for frames 1700 and 1800
after a start at 1140) are fits to that. On real hardware the same comparison goes either way
with the display's timing; the exact clock gives 51 every time, and 91 for the prompt's whole
1.5 s pulse. The reference capture is made on the exact clock now and `GLOW_DRIFT` is gone:
`boot::loop_time` adds the game's float frame time as the game does.

## Fields

- **Point lights** (P2). Barrel's Ryza has none: the shaders declare `point_pos`,
  `point_color` and `point_ambient` but never read them, and the field fills none. The game
  gathers them in clusters (pass 12's `fClstScl` 0.0003, `nClstPtch`, `vClstZPrm`: the grid,
  plumbing Barrel need not copy) out to `pointlightpreset.xml`'s `ClusterFarLimitZ` 100000,
  fading over its `LightFadeOutRange` 2000 (`vFadeDist` -98000, -100000); `IRLocalLightParam`
  (1, 0.33) on the characters' draw. Which lights the field has, and whether any are lit at
  noon, not read. Off at the gate but not sourced (so not dumped as zeros): the light shafts'
  `vLightShaftPower` (it may follow the sun on screen, not a switch) and the vignette's
  `fLimbDarkeningWeight`.
- **The characters' map's box** (P2; `mSubW2S`, `SUB_BOX_PAD`). The game's box in the sun's
  view is a world box's (its v extent equals its depth: the sun at 45 degrees) that follows
  her pose (gate 144.5 by 220.8 in all three idle captures to 0.6; the turn 136.8 by 231.1).
  Barrel's is her draws' box (76.2 by 163.1 at the gate) grown 25.0 sideways and 6.7 up and
  down: the gate's matrix agrees to 1e-3 (one even pad of 17.3 gave v but u 123.5). In the
  turn Barrel's is wider in u (`mSubW2S[0]` 0.00553 against 0.00633), her pose there not the
  game's; the pixels do not move either way. The real rule (her joints grown by a radius?)
  not found; position and facing ruled out.
- **The post constants Barrel does not dump** (P2; `rasen-constants`: 1171 compared, 0 differ,
  131 (pass, name) pairs not dumped at the gate; `barrel-parity constants ... consts.json`
  lists them by pass). They split three ways:
  - Passes Barrel draws, names not yet dumped (passes 11 to 22 and the tone pass 69): the
    shadow terms of the ambient-and-shadow pass (`mW2S`, `mSemiStaticW2S`, `mSubW2S`, `scSMD`,
    `scSM2D`, `vSMOf`; `vS2VZ` and `sdplane` are dumped, the planes of the first cascade's
    map), the lighting pass's other draws (`litDir`/`litCol` 1 and 2, `emScl`,
    `vAmbOccRat`), the water's own ambient and lighting (passes 14 to 16), the reflections'
    step count (pass 21: `nMaxRaySmpl`, an integer the board does not compare; the rest is dumped). The height fog (`cHFog`, `hFogParam`, `hFogFlucPrms`, sky pass 17 and fog pass
    18) is off: the fog shader (`sh09135ab09a4edc1e`) weights both layers by `cHFog`'s w, and
    the game passes all of `cHFog` as 0 although the set holds the second layer's colour
    (0.543, 1.41, 2 at 0x110), so a switch not read turns it off; the pass's visible term is
    the scattering (`Scat`, compared).
  - Off at the gate (weight or switch 0; only their source to read): the depth of field
    (`fKIDSDOFType` 0, `fHexScale` 0, passes 31 to 37), the star filter's result
    (`fStarWeight` 0 in the tone pass, though its chain 49 to 60 runs), the light shafts
    (`vLightShaftPower` 0, 63 to 68), the lens flare (`fLensFlareWeight` 0), the vignette
    (`fLimbDarkeningWeight` 0). Their values sit in the set's `trueHDR_` block and its
    neighbours (`docs/formats.md`, the environment table).
  - On at the gate, and now drawn (docs/renderer.md, "Depth of field"): the depth of field's
    softening. The tone pass
    (`sh54b7a1dac8c7a203`) has three branches on `fKIDSDOFType`; the distant blur
    (`fDistantBlurIntensity` past `fDistantBlurZThreshold`) is only type 2's, and the gate is
    type 0: it mixes in pass 31's quarter-size blurs by the weight in alpha, a ramp
    `saturate((1 / max(1 - 2|w - 0.5|, 0) - 1) / 4)` of `w`, the front blur (`rt480x270 #4`)
    where `w` (`rt1920x1080 #17`'s alpha) is 0.5 or more, the back (`#3`, its own alpha) over
    that. The chain: pass 29 (`sh7b41f1dd6f2b7f36`) writes the scene with `w` = a 512 × 1
    curve looked up by the raw depth (`smplWeight`, `rt512x1 #5`); pass 30 makes the two
    quarter-size layers, pass 31 blurs them in 8 taps; 32 to 37 (the hexagonal blur) feed only
    type 1. The game's pixels at the gate (pixel mirror, scratch scenario `pix-gate`): the
    back weight is nil, the front reaches 0.078 and averages 0.05 to 0.07 over the upper half
    of the screen (the far view), 0 below the middle. The curve is written by the CPU at load
    (no draw writes it in frames 1700 to 2600; the capture logs no texture uploads): what
    builds it (the field's or camera's focus settings) is not read.
    Its values, read off the capture (pixels-rasen frame 2550: draw 2038's alpha against the
    raw depth of `1927-2031 rt1920x1080.19#15`, at the texel centres `u = (k + ½) / 512`): 127/255
    for texels 0 to 495 (depth to about 1,550 units), then 129, 132, 134, 136, 139, 141, 143,
    145, 147, 148, 150, 152, 154, 155, 157, 158 for texels 496 to 511 (the last, the far
    plane and the sky, 0.619). The capture's runtime ignores texture uploads
    (`UpdateSubresource` writes buffers only, `Dispatch` is a no-op), so the CPU or a compute
    pass fills it out of sight.
    Built: the weight, the two layers, the blur and the tone pass's mix (`dof.rs`, `dof_*.frag`,
    `post.frag`); the board's `dof_weight` agrees to 0.0004 at the gate (0.002 turn, 0.004 left),
    the blurred layers to 0.008 to 0.013 (colour, at the game's exposure), the tone 3.89 to 3.67
    (turn 4.57 to 4.39, left 4.74 to 4.50). Left: the curve is a stand-in (`dof::CURVE_*`): its
    source (what the CPU builds it from, so what other fields and cameras give) is not read;
    the weight takes the occlusion's half-size depth (no full-size depth on every setting); the
    hexagonal blur (type 1, passes 32 to 37) and the distant blur (type 2) are not built, no
    field the board knows uses them.
  - Also on, and now drawn: the sun flare and paraffin wash (docs/renderer.md, "The sun flare").
    Its numbers come from the environment template's `flare_paraffin` block (a row per time
    and weather; the noon, sunny row gives the gate's constants, all 738 compared agree). The
    game's two draws are not swapped: the first (intensity 20) covers exactly the pixels with
    depth below 1, the second (`sky_intensity` 15) exactly the sky (pixel mirror at the gate).
    Open: Barrel's post pass has no depth, so its sky takes 20 (9% of the gate's pixels, the
    top of the screen, 25% too much wash there; giving it depth means storing the depth on every
    setting, not only GSR 2 and FSR 3 (`keeps_depth`), and a `sampler2DMS` variant of post.frag
    for MSAA; the scene's alpha is no mask, blended models write it); the colours' and sky intensity's defaults (rows
    without them) are not read; the screen's tilt is fitted to three views; the reference
    point is the camera's focus (the player plus her look height, 128) to within the cascade fit;
    the step 1/60 a frame is not sourced.
    What is left of the gate's pixel score (3.96 of 255 over the frame): fine detail, not
    colour (over 120-pixel blocks the error is 1.27); Barrel's picture has 25 to 35% more edge
    contrast than the game's in every band of rows, the bottom (no depth of field) included, so
    most of it is the game's TAA softening. Barrel's own TAA (`--preset desktop-taa`) scores 6.72
    on the board's one still frame (no history, and it sharpens), so the board stays without it.
    Also on: the game's TAA (pass 72, `g_cbTemporalAAParam` 960, 0.1, 0.05, 0.2, `jitterScale`
    0.5; Barrel's TAA is an option of its own) and two textured screen quads (pass 76,
    `materialColor` 1, not identified).

- **Water** (P2). Barrel draws Rasenboden's river and sea (`docs/shading.md`, "Ryza DX: the
  water"; the file `_river.bin`, `docs/formats.md`): the ribbons of its two bodies from the file's
  points and widths, lit by the field's sun, mixed with the sky's colour by the Fresnel term, fogged
  as the models; the sea runs to the horizon. Left: the game's wave volume normals, the refraction
  and the depth fade (the sea is opaque), shore bubbles, the planar reflection, and the cubic the
  game makes of each piece from the points (Catmull-Rom stands in: the draw's own stream is not in
  the capture). Barrel's pieces are in its draw trace and pair with the game's 10 at the gate (`diff3d`
  pairs water with water); which pieces are drawn and the grid's levels of detail are in
  docs/shading.md ("Ryza DX: the water"); at the gate the sea lies behind the camera, so the horizon's white haze
  there is the sky's, not the water's.
- **Rasenboden against the game's draws** (P2). From the game's own camera at the gate after
  loading a save (ryza-recomp's `load-rasen`, frame 2550: eye 32526.8, 6363.3, 29944.7;
  `barrel FIELD --look-at 32509.512,6224.147,30934.779 --distance 1000 --pitch 8 --yaw 180.99981
  --fov 45 --ryza 32517,6180,30336 --done MM00_010`, row `rasen-3d-2550`), 856 of the game's 913
  draws have a Barrel match. The 55 without: the terrain's 42 strips (Barrel draws a patch at
  once), the water's 10 (below), one `npc23a` draw and 2 of the field's models. Barrel
  draws 113 the game does not: 62 of the field's models and some 20 townsfolk draws, in view
  and hidden behind the town's buildings and ground (the game's own test of what is hidden
  is not read; Barrel's stand-in, the ground's skyline, leaves out 211 draws the game also
  leaves out and none it draws: `docs/renderer.md`, "Ryza DX: objects behind the ground").
  The "title backdrop" draws are Rasenboden's own cliffs and rocks (their textures share
  names with the backdrop's in the index; `PARITY_LIST=1` now names each extra draw's model,
  object number and place). Of the 47 field objects still drawn only by Barrel at the gate,
  34 are on screen and 13 straddle the eye, at all distances (157 to 44,000 units) and sizes,
  in all four classes. Ruled out as the game's rule: the view (none outside it), distance and
  size (the drawn ones span the same), the GPU's queries (none), and the finished picture's
  depth: against the game's own depth target (`1 - 50 / view depth`), 75 objects it draws are
  wholly behind it and some it hides are mostly in front (a fence's box 89%). Where to start:
  the code, not the data: hook the field object's per-frame draw (find it from a hidden
  object's record by RYZA_FIND on its position, `+0x230`, then RYZA_WATCH the flag words near
  `+0x5a0` over a frame), and read what sets the skip. Tried since: the placement word's
  unread bits (0x08 to 0x10000) do not split hidden from drawn (both carry 0x28, 0x68, 0xa8,
  0x120a8); a trace object's number is its placement's index plus one. Searching the running
  game's memory at frame 2550 for the objects' positions (RYZA_FIND) finds their records; a
  byte 0x727 past the position was 1 on two hidden rocks and 0 on two drawn ones, but over
  107 records it is 1 on 52 drawn and 0 on 10 hidden: not the flag. (RYZA_FIND stops at 2000
  hits; leave out an object placed at the origin, whose zeros match everywhere.) **The rule
  is occluders**: the field's `_le.koc` quads (docs/formats.md, "Ryza field walls"). Each frame
  (`0x140a3cac0`, called from `0x140725b50`) the game keeps the quads in view (each as two
  triangles against the frustum, `0x14099aa20`; a quad's byte `+0x45` turns it on), sorts them
  by their nearest corner's depth (`0x140a3b7d0`), gives each five planes (its face turned
  away from the eye and its four edges through the eye, `0x140985080`), drops quads wholly
  behind a nearer one (flag `+0x5d` of the 0x60-byte records at `+0x30`, count `+0xc8`), then
  measures the kept ones' corners in a 2D camera basis (from `0x140a3d183`; use not read).
  Tested with Barrel's draw boxes at the gate (scratch `koc2.py`), per model part, as `diff3d`
  pairs repeated baked models by count and not by place: a box wholly inside one quad's
  five planes accounts for 45 of the 53 parts the game hides, but 3 the game draws would be
  hidden (`S06-HOUSE-01`, `COMMON-STUMP-01`, `S09-STAIR-02`) and 11 it hides are not
  (cliff blocks, `CLIFF-15M-01`, `CLIFF-30M-02`, a mountain). A bounding sphere fits worse (12).
  The per-object query (`0x140a39fc0`, from `0x1407282ce`) and Barrel's version of it:
  docs/renderer.md, "Ryza DX: objects behind the field's occluders" (99fe8e5): at the gate
  Barrel's draws 975 -> 934, field parts the game does not draw 78 -> 38, matched 859 -> 858.
  The stump was a false lead: the game hides stump 1147 too (its own query on its own kept
  records, a hook `flt*20:rdx u32:rcx+0xc8 flt*768:[rcx+0x30]` at frame 2550, scratch
  `occr.py`: 12 records, no zero planes, 97 of the 364 boxes hidden, 1147 among them). The
  game's fourth stump draw is stump 1145, which it queries and draws and Barrel leaves out
  by the view: its centre is at x -1.11 in Barrel's view (box to -1.06), the paired draw by count
  made 1147 look wrong. The view test's planes are the camera's (logged at `[r14]`: 72.7 by
  45 degrees, near 50, far 300,000, inside negative), but an object takes one of two tests
  by its word `+0x250`: 0, its oriented box (`0x14099a3d0` on `+0x1d0`); else the world box
  around that box (`0x14099a3b0`, a jump to `0x14099c050`, on `+0x220`: max then min corner).
  At the gate 463 take the first and 760 the second (scratch `views.log`); 1145's world box
  reaches 2 units inside the left plane where Barrel's (its vertices' box, 45 to 187 units
  smaller a side) is outside. What sets `+0x250` is not read: not the model (both tests
  have deserts, cliffs, trees, fences), not the tilt, nor size or distance alone (the second
  test's objects are smaller, median half diagonal 194 against 1,495, but overlap). Left:
  (1) the `+0x250` flag (Barrel would test those objects on the box around their oriented
  box; one draw at the gate). (2) The rest of the extra parts
  (20 objects at the gate) are not the occluders' doing. The game keeps quads 98, 8, 3, 6,
  2, 1, 0, 4, 9, 5, 11, 14 (its records matched to the file's by plane); the dropped ones
  in view are mostly inside a nearer kept quad as Barrel's rule has it, but 15, 16, 92, 95,
  96 and 97 are not. Both programs take the same 26 quads in view, in the same order (the
  update `0x140a3cac0`'s stale records past its count hold all 26, depths Barrel's less the
  near distance 50), and the game's count is already 12 right after its covered-quad drop
  (hook on its `sqrt` call, `0x140d70164` from `0x140a3d284`, reading `rdi+0xc8`). That drop
  as read (`0x140a3d090`..`0x140a3d17d`: a later quad goes when its four corners lie strictly
  inside all five planes of a nearer kept one) keeps those six: none of the 12 holds one by
  85 to 2,300 units. So the drop does more than read (2 tries; the view test `0x14099c7f0`,
  the enable byte `+0x45` (all 1), the frustum (the camera's) and a count cap are ruled out).
  Barrel keeping quad 15 hides 5 objects the game draws (899 to 901, 958, 1137).
  The mountain (8), the cliffs 78, 95, 96, 586, 587 and the kudzu 972 reach the game's query
  and its own records leave them visible (the best quad misses each by 397 to 3,108 units),
  yet it draws none of them: something after the query leaves them out (per part, or a
  draw-stage test; not read). The far trees (785 to 1183, `-FAR`/`-FAREND`) and object 168
  never reach the game's view test: left out before it (a far-tree switch, not read). (3) Occluders are off on the title (`a.title`): the title's field
  has a `_le.koc` too; whether the game uses it there is not checked. What got it here from 809: objects are
  in or out of view whole, by a sphere (`docs/renderer.md`, "Ryza DX: which field models are
  drawn"), and have no draw ranges or lower levels of detail at the top settings, as the
  game's code and capture say ("Ryza DX: object draw ranges"). The story's progress decides
  who and what a field places (`docs/tables.md`, "Ryza DX: story spans"; this save has only
  `MM00_010` of the main story, which leaves the same 22 people and 4 boards as before).
  The townsfolk stand where `npcmarker.xml` puts them, turned as the capture's root matrices
  (`npc25c`: within 6 units and 0.01 of the game's). The walkers are where the game has them only at the
  game's own field time: row `rasen-3d-2550` runs with `BARREL_SKY_CLOCK=13.7166` (the pixel
  board's clock for that frame; at 0 the two walkers in view stood at their paths' starts,
  1,200 and 3,600 units off). At that clock the townsfolk's bones are within 55 units of the
  game's (`npc29a` running on `s06_move_data_003`, 3 m/s), 41 (`npc22a/b` walking
  `s06_move_data_005`) and under 9 for those standing; walkers now stand on the
  ground under their path (the field's heightmap and floors, as Ryza's; `WALKER_STEP`, a
  stand-in, though 50 to 1,000 give the same): `npc22a/b` 41 and 40 -> 11 and 6. Left:
  `npc29a` stays 55 off (not height: `DIFF3D_POSES=root` has its root 28 and 50 off in x
  and z, along its path: at clock 13.5 instead of 13.7166 it is 7.6 off, but `npc22a` is
  then 40; no one clock fits all three, so its pace, its pauses' lengths (random between
  `interval_min` and `interval_max`) or its start differ; the capture's frames 1750 to 2550
  (every 50) settle it: the game runs it at 301 units a second (3 m/s, as Barrel) and covers
  140 fewer units between 1750 and 1850 than its pace gives: a pause at a point, whose length
  the game draws at random between the row's bounds; Barrel cannot match it without the
  game's random numbers, so this one stays), the game's own
  rule for its walkers' ground (not read), and walkers on the title (no ground without
  Ryza's rig), and the pace of their motions. The story comes from a save with `--save` (`docs/formats.md`,
  "Ryza DX saves"). Left: the object ranges' own measure and classes below
  the top setting (the placement word's bits 0-1 as the class is still a guess: nothing at
  High tells), the game's switch for levels of detail, the walkers' pauses, motions and
  ground between points (`docs/tables.md`), the people's drop to the ground when
  `no_gravity` is 0 (ten of Rasenboden's rows have it 0, with motion 0 and no turning), the
  people's and gimmicks' own ranges below the top setting, and the bits of `timezone` and
  `weather` (stand-in
  order). The windmills' sails turn and the boat rocks by the motions the object
  container has under their names (`<model>.g1a`, looped: the game turns the sails 0.5° a
  frame, all windmills together); where in the motion the game is when a field appears is
  not read (Barrel starts it with the field; the board's `BARREL_SKY_CLOCK` sets it, and the capture's
  sail joints sit where the sky's clock puts them: 7.05 s at frame 2150). The game's projection is a
  vertical field of 45.0 degrees at 16:9 (`mW2P`: y scale 2.4142 over the view's pitch cosine, x
  1.3578): the 45.4 the gate's row used was 1% too wide (read off the culling, not the matrix).
  With the exact camera (eye from `mW2P`'s zero point, the yaw and pitch from `mWV`'s third row,
  the field of view from its y scale) the clip matrix equals the game's to 5 digits and the
  game does not jitter its projection at these frames (the temporal anti-aliasing's constants
  are not dumped, but `mW2P` has no sub-pixel offset). The cull stand-in (docs/renderer.md, "which
  field models are drawn") was measured under the 45.4 view: 858 pairs then, 856 with the exact
  one (2 parts of objects at the view's edge).
- **Ryza 3 DX's placement lists** read neither as Escha's nor as Ryza DX's (they fail at the
  first type's instance count): for when Ryza 3 starts (`docs/formats.md`, "Placement lists").
- **Field lighting.** `barrel --time T --weather W` lights a field with the game's own
  constants where they are read (docs/shading.md, "Ryza DX: the frame as the game draws it":
  every pass, its formula and its constants at Rasenboden's noon, from the answer key's
  capture): colour textures sampled as sRGB, the sun at the stage's key scale (3 at noon) and
  the second light from the set, the cloud pattern on the sun (strength 0.7), the stage's
  ambient from the diffuse light map (7 × (0.1, 0.2602, 1)), the exposure metered (0.9 at
  noon), the colour scale (1.27, 1.3, 1.2), the filmic curve with the set's
  `fLumiMax` and `fGamma`, and `SaturationScaleEx` (1.6) after the gamma. Against the game's
  noon still at the gate (`work/ryza-ref/rasen-gate-noon.png`, fourteen spots): sunlit sand
  237, 217, 144 against 246, 232, 175, stone 226, 227, 209 against 236, 236, 210, a path in
  shade 25, 57, 117 against 25, 57, 115, a rock in shade 46, 73, 119 against 56, 82, 118;
  mean error 12.8 levels a channel (14.5 before these). Left, largest first:
  (1) **dark surfaces in shade** come out dimmer and less blue than the still's (a trunk 32,
  32, 31 against 44, 53, 63, dark leaves 23, 65, 74 against 44, 81, 78): the game adds the
  specular ambient in a pass of its own and Barrel's models do it as the pass does (the `_as`
  cube map by the reflected view, from the set's scale and tint, fogged; docs/shading.md),
  but the bark's specular map is dull (`F0` 0.02, gloss 0.2), so it adds a few levels;
  what is left is the screen-space reflections (the pass mixes the cube map toward them),
  and the terrain's, the plants' and
  the characters' specular ambient (not drawn; the terrain's tiles are as dull). (2) **sunlit grass**
  lacks red and blue (150, 194, 47 against 171, 210, 64; 133, 190, 2 before the direct
  specular and the terrain's normal maps): the plants' own ambient (the pass's
  second kind, a flat (3, 6, 4), is a guess) and the bloom. Sunlit sand is 193, 195, 94
  against 217, 219, 114: a gap of about a tenth in every channel that the ground's
  light does not explain yet. (3) **the ambient occlusion**
  and its tint: drawn (docs/shading.md, "The ambient occlusion"), in the direct and ambient
  specular too. The direct specular is drawn for
  the models with a specular map and for the terrain (docs/shading.md, "What Barrel does of
  the lighting pass"); the characters' and the plants' are not. (4) **the exposure** is metered by the
  game's rule (docs/shading.md, "The exposure"): daylit views and the attic's backdrop stay at
  the set's floor (0.9), as the game's noon still's own brightness says it does, so the
  sunlit sand's tenth is not the exposure; the night view rises to 1.14. Left of it: the depth
  of field's weighting of the metered layer, and the adaptation's base rate and time (0.95
  and 3 s, from the capture: no lighting set carries them). (5) **the bloom**: the game's bright pass and chain are done, its star
  filter is not. (6) **the cloud pattern's
  phase** starts with the field (the game's is not read), and the pattern's switch for the
  other times of day is a reading of one word (stand-in). (7) **the sky**: the plane and the dome take the game's own constants (below). Evening,
  morning and night have no reference frame. Ryza takes the characters' light (`indirectChara_`:
  the sun at 0.8, the set's ambient at 0.2 plus the light map's mean at 1.5 × (0.8, 1, 1),
  flat rather than by her normals), her second light at 0.487 (`KeyLight1Scale`); the game also adds
  the light volume, which Barrel does not.
  (8) **The constants board** (`barrel-parity constants`, docs/parity.md, "Constants parity"):
  397 constants of the lighting and post draws at the gate against what Barrel uses, 0 differ
  (17 before the board's own conventions and Barrel's two wrong values were put right: the
  scene fog's in-scattering was the sun at 1 where the game's fog pass takes it at the stage's key
  scale 3, and the characters' second light was at 1 and 0.267 where the game takes 0.487).
  So the lighting set's values are not what separates the pictures at the gate (the pale
  distant hills, the white horizon band, the sand's tenth): the cause is in what Barrel has no
  counterpart for, the 240 constants the board lists as not dumped (height fog parameters read as
  zero at noon, the ambient occlusion, the reflections, the cascades' matrices, the light
  volume's clusters, the depth of field, the star filter, the light shafts, the temporal
  anti-aliasing), or in a formula. The board's values that are stand-ins (the dome's and the
  strip's wind offsets are left out: time-dependent) are marked in `play/constants.rs`.
  (9) **The halo and the gate's picture against the game's pixels.** Two views are on the board
  (`docs/ryza/views`): the gate and the turn view (frame 2150, toward the windmill tower and a
  house). The camera first: the gate's row had `--fov 45.4` and a yaw of 181, 1% wider than the
  game's projection and 0.0002 degrees off; with the matrix's own 45.0 and the exact yaw the gate's
  tone error went from 15.87 to 9.02 (stage 15.2 to 8.0, the `normal` median from 4.3 to 1.5
  degrees) and the turn view's from 19.5 to 10.9, all of the "thin edge offset". The turn view
  then lost its windmill-tower band when the field clock turned the sails to the capture's phase
  (7.05 s; the sail shadow had fallen on the tower's other side): tone 10.12 (stage 9.11, sky
  16.4, characters 26.4 over her pose). With the dome's layers slid as the game's
  (`BARREL_DOME_CLOCK`) the gate reads 8.12 (sky 3.7) and the turn view 9.05 (sky 4.9); with
  Ryza where the capture has her, at her idle's phase (docs/parity.md), 6.79 (characters 9.5 from
  36.9, stage 7.05) and 8.69 (characters 13.9 from 26.4, stage 8.94); with the field models'
  metal darkened as the G-buffer program writes it (colour times 0.2 times the specular map's
  green: the mailbox's ornament read 0.74 0.54 0.16 against the game's 0.25 0.15 0.06 lit, now
  0.29 0.19 0.11) 6.77 and 8.53; with the cloud pattern where the game's matrix puts it (it
  stood 500 and 866 units off: docs/shading.md) 6.19 and 5.48, the sun's share on open ground
  0.456 against 0.458 and 0.446 against 0.448 (so the shares below, measured before, are
  mostly gone). Shares left on the turn view (lit, game against Barrel):
  the tower's sunlit side 1.70 against 1.86 and shaded 0.86 against 0.93 (the sun's share 0.43
  against 0.47 and 0.45 against 0.48: the cloud pattern's level; the occlusion and normals
  agree, the normals within 3 to 4 degrees), the house's white wall 2.41 against 2.77 (share
  0.425 against 0.492, flat in the game and banded in Barrel's: the pattern's lookup on a
  vertical wall), the planter 0.20 against 0.26 (share 0.45 against 0.56), the rock 1.04
  against 0.84 (Barrel darker, share 0.54 against 0.51, normals 3.5 degrees), the crates and the
  grass and the path within 3% (grass 3%). The pattern's level is the open term at both views:
  the gate's ground has Barrel 7% under the game's share, the walls 16% over.
  Earlier measures, before the camera was exact: The game's own render
  targets for the gate (frame 2550; the pixel board, docs/parity.md) replaced the still as the
  measure. The halo was Barrel's, not the game's: the cloud strip's colour was not stored at
  half like the rest of the scene, so the clouds were twice as bright (fixed:
  docs/shading.md, "Ryza DX: the sky, the clouds"), and the bloom's bright pass three times the
  game's. Tone error against the game's tone pass (mean absolute, 0 to 255): 24.63 at the
  start, 24.13 with the strip's fix, 23.85 with the clouds given the capture's phases
  (`BARREL_SKY_CLOCK`, `BARREL_PATTERN_PHASE`, `BARREL_CLOUD_THRESHOLD`; the game's cloud map is
  noise from a lattice it shuffles each run, so its clouds' places cannot be reproduced). The
  camera is within a pixel or two (the best shift of the picture is (-1, 2) px: 22.97 against
  23.34). What the 23.85 is made of at the gate (shares of the summed error; the picture's
  error is mostly broad, not fine: still 16.4 after a 32 by 32 box blur of both):
  - **Leaves missing in the trees' crowns: solved.** The alpha-tested field materials
    (`PBBA2-N`: the trees' leaf cards, house trims, signs) are drawn by the game with no face
    culling, and Barrel culled their back faces: from below, the cards that face up were gone
    (docs/shading.md, "Two-sided materials"). Sky is 9.10% of Barrel's half-size depth against
    the game's 9.15% (10.40% before; barrel-only 0.51%, game-only 0.56%), the crowns'
    cells (2,0) and (3,0) hold 1.4% and 3.9% Barrel-only sky (12.1% and 14.3% before), and the
    tone error is 17.36 (20.44 before). What is left of the sky's edge is leaf detail (the
    game-only share is as large). The sway's phase and the alpha the G-buffer program reads
    are the game's, so nothing there was checked further. Not found: what the game keys the
    single-sided `PBBA2-N` draws on (flowerbeds, ivy: lighting-material flags `0x133`).
  - **The ambient occlusion: about 13%** (the pixels with an occlusion below 0.85 are 35% of
    the picture at a mean error of 27.4 against 15.0 elsewhere; the game's own occlusion laid
    over Barrel's scene took the tone error from 23.2 to 20.1). The feature is built from the
    game's program (the tone error is 20.6 on the board: 22.8 before) with a stand-in noise
    texture, so the occlusion's pattern differs (mean abs 0.128 against the half size picture, its
    regions' means 0.02 to 0.08 above the game's: the left canopy 0.741 against 0.657); the tree
    regions of the lit stage are still 1.5 to 2 times the game's brightness (item below).
    **The far hills** (the strip (1500, 330, 1800, 350), lit and fogged 0.150, 0.387, 0.535
    against the game's fogged 0.128, 0.318, 0.427): two causes found and fixed. Their models
    stand at 0.47 of their height and the game turns normals by the placement's own matrix
    (Barrel turned them by the inverse transpose), and the normal map's green runs against the
    texture's v (Barrel's frame from the screen derivatives had it along v: over the stage the
    median angle to the game's normal target went from 14.6 to 11.8 degrees, the stone wall
    from 24.5 to 16.3). The meshes' own tangents (the game's frame, `B = cross(T, N) w`) take the
    median angle over the stage (the board's `normal` line, half size) from 5.7 to 5.6 degrees,
    the far hills' from 24.0 to 22.8, the wall's from 7.6 to 7.0, the trunk's from 10.9 to 10.7: the
    derived frame was nearly the same. The grass is the biggest region left (the game's normals
    there vary with a standard deviation of 0.17 in y, Barrel's plant cards' 0.01). What is left
    on the hills is about 20%.
  - **The sky's pixels: 9.5%** (9% of the picture, mean abs 24.9; with the game's own cloud map
    (`BARREL_CLOUD_MAP`, docs/parity.md) 16.9 and the tone error 15.76 against 17.01 without it). Measured on
    the board's stages (sky pixels, game units): the plane 0.091, the plane and the dome's first
    layer 0.106, both layers 0.416 (the dome's second layer is the clouds on the dome: its
    contribution to the sky is a quarter weaker than the game's in the rows 0 to 180, 0.47, 1.0 against 0.63, 1.33 in
    green and blue; the game's sun term there is about 0.37 stronger; the mesh's own tangent, now in
    Barrel's vertices, changed the `dome` stage by less than 0.001, so that is not the cause),
    the strip's contribution 0.106 (0.241 with the grid cells split along the other diagonal; the rest is in
    the rows 0 to 60), the finished sky 0.51. Those were measured with the dome's layers standing still: with their
    scroll (`txTrs01`, docs/shading.md) the second layer's weakness is gone (it was the clouds
    standing elsewhere), the `dome` stage's sky reads 0.079 and the finished picture's sky 3.7
    of 255 at the gate and 4.9 in the turn view (12.5 and 16.4 before). Not drawn are the cloud particles: the game's draw `1916` leaves the picture as it is at
    this frame (its target equals the strip's, every pixel exactly), so they are not built.
  - **The characters: 3.7%** (2.2% of the picture, mean abs 39.4): Ryza's pose differs by
    10 to 15 pixels at her feet (the idle's phase), her shaded side and outlines. The
    characters' own shadow map (docs/shading.md, "The characters' map": her draws are in it and
    not in the cascades; 1024², its box around her) is built: her shadow on the ground has the
    game's crisp shape and is cut at the box's edge, and the `shadow` stage's characters mask is
    0.099 mean abs (0.114 before; the game's sun share on her pixels is 0.086, Barrel's 0.092
    against 0.110). The tone error did not follow: 15.81 to 15.85 over the picture, the `ryza
    shadow` region's tone 29.96 to 31.03, because the crisp shadow now lies where her pose differs
    from the game's (a blurred one overlapped more). Left: the box's rule (the game's grows with
    her pose; Barrel's is her box on the starting joints plus a constant, a stand-in), which
    shaders take the second bias set (a stand-in list).
  - **The rest, about a third**, spread over the stage at 15 a pixel: the shadows' edges (the
    sun's share against the game's alpha: 0.072 mean abs over the stage, a third of it from the
    stage past 10000 units, where the game reads its static map and the clouds' pattern,
    random per run; the characters' own shadow map is built, below),
    the reflections, the depth of field's softness, the temporal anti-aliasing, the lantern's
    glass (lit white in the game, dark in Barrel).
  Where the game's occlusion is 1 and the sun's share 0, Barrel's scene is within 3 to 5% of
  the game's, the stage's ambient equals the game's ambient pass to 1%, and the sunlit sand and
  stone are within 6% (docs/shading.md, "Against the game's own pixels"): the lighting set's
  values and the exposure are right; the picture's error is coverage and occlusion.
  Ryza herself, seen from behind at the gate (her shaded side), with the toon ramp sampled
  as the game's pass does (docs/shading.md, "Ramp"): coat 161, 146, 69 against the still's
  162, 150, 75, thigh 154, 138, 142 against 156, 143, 102, hat 149, 142, 138 against 162,
  160, 161; left: the frill under the coat (142, 119, 61 against 90, 75, 48: the occlusion,
  which also drives the ramp), the boot calves (95, 60, 27 against 60, 40, 25: her forward outline pass, docs/shading.md), the
  still's dark creases on the coat and red sheen on the boots (occlusion and specular).
  The ramp's constants are the ones captured at Rasenboden's noon, taken for every field.
- **Model lighting material.** Ryza's models' G1MG section 0x10001 reads as in Escha but holds
  diffuse 0.8 and ambient 0 (`barrel g1m-materials`): their shaders (`PBB-N`, `PBB-EmN`, ...)
  take the ambient from the environment, not `cAmb`. `barrel` gives every draw the full
  material until those shaders are read.
- **Emissive and odd materials.** `PBB-EmN` submeshes (the attic wall's windows; at
  Rasenboden three houses' windows, a bridge pillar and the light poles) have an emission map
  (texture kind 19) and flicker parameters (`emFrq` 1, `emRange` (0, 1) or (1, 1), `emStart`
  0 in Ryza's house). The lighting pass adds `sEmi × |emScl|`: to the light (times the colour)
  when `emScl` ≥ 0, to the finished colour when negative; `emScl` is not among the material's
  parameters (set at run time). `barrel` draws the map as light (`model.frag` flag
  32768, the map bound as the draw's set 3) at a stand-in strength by time of day
  (`EMISSION_BY_TIME`: none by day, 2 at evening, 3 at night): the house's window glows at
  night. The flicker is not done: no pass of `PBB-EmN` (`_gb`, `_gba2`, `_cll`) reads
  `emFrq`, `emRange` or `emStart`; all three take only `emScl` (+16 in the G-buffer passes'
  block, +96 in the lighting pass's), so the game works the flicker out on the CPU into
  `emScl` each frame. Its curve (a wave between `emRange`'s two ends at `emFrq`, offset by
  `emStart`, would fit the names; `emRange` (1, 1) would then hold still) is a guess until a
  run reads `emScl` over a few seconds (tools/oracle/README.md). Not drawn yet from the G-buffer model
  (docs/shading.md, "Ryza DX: the deferred G-buffer"; normal maps are drawn now, their
  green's sign unchecked): the specular map (metal flag, specular amount, roughness), and the lighting pass (`pbb-*_cll.g1s`). Attic view inside: `--look-at 3905,2250,3914 --distance 300 --pitch 10`.
- **Terrain against the models.** Rasenboden's terrain (grass, dirt paths, cobbled road; its
  heights match the heightmap) lies at grass level over the sandy canyon path the cliff models
  make in the village close-up (`--look-at 31261,5466,30776 --distance 6000 --pitch 25`). Every
  Ryza lit terrain draw has kind bit 0, so that bit hides nothing; program bit 4 (on some
  draws) is unread. Needs a frame of the game at that spot, or the field shader pack's terrain
  programs. The layers' normal maps are not drawn yet.
- **`obj-A21-S06-FLOOR-01`** (placed at the origin, its geometry in field coordinates, x 1210
  to 56805, z 33342 to 53823) is the stone paving along the canal and its bridge: hidden
  (`--hide FLOOR-01`), the overview shows a hole there. It draws where it belongs; only its
  collision meshes are off (below). Not a stray "sand square", as first noted.
- **Other field files** not in Escha: `.trmd` (`TRMD`, 5 MB, 1024 × 1024 header words then
  runs of `24 ff ff ff ff`), `.g1pf` (`FP1G`, a grid over 128,000 units, likely navigation),
  `.koc` (read: the field walls) next to the containers.

## Against the game

A recorded run of the original (a new game: the opening, the attic event at dusk, free play
from the path outside Ryza's house; kept outside the repository) is the reference. Ryza's house
is `obj-A21-S06-HOUSE-01` (32538, 6194, 28804); free play starts on the path down from its gate,
past the signpost (32148, 6003, 31151) toward the light poles. Close to the game's first view:
`barrel INSTALL s06_t_rasen OUT --look-at 32400,6350,30600 --distance 800 --pitch 12 --yaw 150
--time morning`. What differs there, largest first:

- **Shadows.** The game's path and rocks lie in the trees' shade, lit blue by the sky.
  `barrel` casts the sun's shadow from every placed model and Ryza (`--no-shadows` to
  compare) as the game's three cascades (boxes round the view's slices to 1600, 3600 and
  10000 units, with the game's depth bias and filter) and a static layer over the whole field
  past them (docs/shading.md, "The cascades"; docs/renderer.md, "Shadows"). Measured against
  the game's sun share at the gate (`barrel-parity pixels`, the `shadow` stage): the stage 0.0785
  mean abs, the middle tree 0.112 (the game's 0.117), the left canopy 0.010 (the game's 0.010,
  0.034 before the leaf cards were two-sided: the leaks were the cards' missing back faces,
  not the sun), the dark canopy 0.012 against 0.009, the trunk 0.002 against 0.001, the grass
  0.402 against 0.428, the sand 0.345 against 0.380. Left: (1) where the game takes its cascades' ends
  (1600, 3600 and 10000 and the fade's 3500 are one frame's constants, `CASCADE_ENDS`), the
  depth plane 36755 before the camera and the third cascade's box (11672 wide, no slice from
  3600 fits it); (2) which draws cast (a placed model casts here; Barrel's cascades hold 160,
  235 and 687 draws against the game's 104, 177 and 669: the choice is not found); (3) the
  characters' map (`mSubW2S`, 25 draws: built, with a stand-in box); (4) the static map's contents (drawn once at load in the game: it is
  not in the capture; its box and its reading are the game's, docs/shading.md "The cascades",
  and its bias is the far cascade's as a stand-in; where its top 5,000 and its depth 173,439
  come from is not found). The lit canopy's brightness (0.05 against 0.022) is not the sun's share
  but the occlusion: regressing the lit stage on the share per pixel, Barrel's ambient part is
  0.061 against the game's 0.039 in the canopy and 0.070 against 0.031 on the middle tree, the
  game's sun slope half Barrel's. With the occlusion built (docs/shading.md, "What is left in
  the lit scene") the canopy's lit colour is 0.044 against the game's 0.022: the sun's share
  was the part left (0.034 against 0.010): two-sided leaf cards close it (lit stage: left canopy
  0.023, 0.044, 0.034 against the game's 0.022, 0.042, 0.032; dark canopy 0.032, 0.059, 0.045
  against 0.030, 0.053, 0.032; trunk 0.024, 0.034, 0.038 against 0.019, 0.028, 0.031). The shadow pass
  is 1.9 ms (6.9 before the geometry moved into device memory, docs/renderer.md); which draws
  cast is still the open part (160, 235 and 687 against the game's 104, 177 and 669).
  The gate's river (region `river`, 48 pixels at the screen's left edge) is 7% off at tone, but a
  wider region of it (`river_wide`) agrees (0.565 0.726 0.813 against 0.563 0.718 0.801): the
  small one is the ripples' highlights, the wave normal Barrel's water does not draw yet
  (docs/shading.md, the water's normal).
  The turn view's far tree crown (region `crown`, about 7,500 units out: inside the fade from
  the third cascade to the static map) has the game's sun share 0.151 against Barrel's 0.085,
  and its final colour is 20% darker (0.152 0.330 0.396 against 0.219 0.397 0.419): it waits on
  the third cascade's box and the static map's contents above. The draw trace names each
  shadow layer (`shadow0`..`shadow2`, `shadow-static`, `shadow-sub`) to compare casters per map.
- **The sky.** `barrel` draws the sky plane with the game's own constants (the set's
  `skyFog_`, the sun times its power, the path through the atmosphere, the horizon haze of
  the glow block) and the set's cloud dome where the set places it, lit by the set's ambient
  and sun for it and fogged with the sky's scattering (docs/shading.md, "the sky"). The game's large white clouds
  are drawn too (a strip showing a 1024 × 1024 cloud map ray-marched from the set's `cloud_`
  block, docs/shading.md "the clouds"; where they fall differs from the game's, whose noise
  table is reshuffled each run, so the gate's sky mean runs 130 to 149 in red over three tables
  tried, the game's still 153). Left, largest first: **the cloud particles** (`cloudParticle_`: a few dozen
  billboards from a CPU system whose rules are not read), the clouds' stand-ins (their
  threshold moves with a height noise and the sharpness follows it, `3 / (1 - threshold)`; the grid's height and fall are read from
  one frame, so other eye heights and sets are unchecked), how long the dome's layers have
  slid when the sky's clock starts (the game's dome runs some 14.7 seconds ahead in the captured runs; Barrel's starts with the sky's).
  At the gate's noon a gap in the leaves at the top of the view shows bright sky where the
  still shows leaves: the tree's foliage there, not the sky. Morning, evening and night have
  no reference frame.
- **Which set at which hour.** The game's first view of free play
  (`work/ryza-ref/rasen-path-start.png`, the clock at 10 AM) looks like our noon set there:
  deep shade, blue-lit path and rocks, a clear blue sky. Our morning set comes out pale and
  hazy with hardly any shade: its ambient is ten times noon's (19.7, 29.6, 35.6) and its
  `skyFog_` a thick grey haze (mie 4, 3.4, 2.5; rayleigh 1.5, 2, 3). The HUD's clock shows which:
  its icon is one of four (`gen_a21_field01_clock_icon` in `ui/a21_field.g1t`: rising sun,
  full sun, setting sun, moon, in the set order morning, noon, evening, night), and at 10 AM
  in every still it is the full sun, so 10 AM is already the noon set. The hours where the
  others begin are not read yet (the executable names `time_zone`, `EVENT_SET_TIMEZONE`). Night has no reference
  frame yet, so its colour (too bright and green here) cannot be checked.
- **Grass.** The plants (flowers, weeds, tufts, wheat) come from `_grass.bin` / `_grass.g1t`
  (docs/formats.md): which kinds grow in a cell, how many, their cards and their sizes' ranges
  are the file's, and `barrel` draws them as the game does, one draw a 500-unit cell of upright
  cards turning about the vertical (look slots 37 to 39), on the heightmap: the title's 348
  plant draws all have their match by card atlas and plant count. Which cells are drawn is
  the game's own test (docs/renderer.md, "Ryza DX: which plant cells are drawn"): read from
  `0x140a78dd0` and its planes hooked at `0x1409990c0` (the call from `0x140a79304`), it gives
  the game's cells but for 1 on the title and 2 at the gate; Barrel's left-over plant draws
  are 1 on the title and 3 at the gate (they were 2 and 15 with the measured limits it
  replaced). Not modelled: the test box's top uses the tallest plant of the cell's kind
  (`+0x48` of the kind), where Barrel takes the cell's tallest plant. Where a plant stands in its
  cell and where in its ranges its size falls are the game's own numbers
  (`barrel_ryza::grass::Scatter`, from the game's cell builder `0x140a77060`, which fills a
  cell's vertices the first time the cell is drawn: a generator `s × 69069 + 1` seeded from
  the cell's grid index, each member's run seeded from the cell's, four numbers a plant):
  all 574 plant cells of the pixel board's three captures come out plant for plant, place
  and size. The title's 348 cells' first plants stand where the rule puts them too. Not read in the builder: a second run of numbers that can drop plants (none is
  dropped in these captures), and a branch for cells at the edge of a neighbour with
  another setting (flags at the cell's +0x100, +0x104, +0x124). The row length in the seed
  (256) is the builder's on Rasenboden; whether it follows the grid's size is not checked on
  another field. Stand-ins: how a lower grass quality thins the plants, a sway of the
  cards' tops by stand-in wind values (`PLANT_SWAY`: the game's `g_vAttCache0` / `g_fPhase`
  are set at run time; sway slots 0 and 1, free in Ryza's fields), no shadows cast, lit as
  the field's models with an upward normal, normal-mapped by the cards' own normal maps
  (`_grass.g1t`'s third texture, flag 8192, as the game's plant program does). In the shade
  they come out teal where the game's are green: the cards are green (the grass tuft's opaque
  pixels average 60, 83, 25) but the sky light from above is blue (noon's 2.7, 3.0, 5.2 ×
  the cube map); the game's pigment mix most likely carries the green. They shrink away
  over the game's grass preset's range as the game's do (`model.vert` flag 16384, look slot
  93); the near clip and the far scale-up are not done. The preset follows the graphics'
  `foliage` (full: Middle, density 1, 8000 to 15000; reduced, the handheld preset: Low, 0.75,
  3000 to 3100; least: VeryLow, 0.5, 2000 to 2100; `barrel_ryza::grass::Preset`). Rasenboden
  at the gate: 120,884 plants in 1,432 draws, 4.5 ms a frame on the PC at 1280 × 720. The
  game's plant program mixes the vertex colour with a field-wide pigment map by
  `g_vPigmentMapScale` (docs/shading.md, "Ryza DX: the plants"); which texture is the pigment
  and the scale's values are set at run time, not read. What is left of the grass on the pixel board is fine detail: the
  left view's grass strip (y 480 to 600) is 12.3 of tone error a pixel but 3.6 with both pictures
  blurred by 6 pixels, the two line up to the pixel (no half-pixel shift does better), and their
  sharpness (mean Laplacian 63.1 against 64.1) and spread (standard deviation 54.3 against 54.0)
  agree: the blades differ one by one, not in shade. Over the whole board half the error is
  such detail (4.08 against 2.02 blurred on the left view). The title's capture has no
  other grass draws: whatever grass shows on the ground itself is the terrain's own shading.
- **Things in front of the camera.** The game leaves a placed object that stands right in
  front of the camera out of the view, while it still casts its shadow: in the left view of
  the pixel board (`docs/ryza/views/left.txt`) the roofed post `obj-A21-S06-POST`, 125 units
  from the eye over the ground and under the eye's height, has no draw in the G-buffer pass
  and three in the nearest shadow cascade; in the turn view, 355 units away, it is drawn.
  `barrel` draws it (it fills a quarter of the picture), so the left view's run leaves it
  out of the view by name (`--unseen`). The rule is not read: the camera file has no setting for it (the
  program knows no `camera_ct` name; the field template's note calls it "things in front of
  the camera turn see-through", with a speed and a radius), and whether it fades or cuts is
  not known (one frame, 300 frames after the camera stopped).
  Read so far (pass `0x140728150`, per object `rsi`): drawn (`+0x247`) needs `+0x245` set,
  the float `+0x1cc` (and its parent's at `+0x2c8`) non-zero, byte `+0x249` clear, the
  frustum test (`0x14099a3d0` on the box at `+0x1d0`) and the occluders; the shadow flag
  (`+0x248`) is worked out after, apart from all that (`+0x246`, then the shadow view's planes
  at `[r14+0x8]`). In the left view's frame 2150 (hook on the frustum test) 463 objects reach
  that test, none with `+0x249` set, and the post is not among them: it is stopped before,
  by `+0x245` clear or `+0x1cc` zero, which leaves its shadow. Who sets it is not found: the
  thirteen functions that store to `+0x1cc` and the small setter `0x140a67960` do not touch
  it in that frame (it was set earlier, or it is `+0x245`). Next: a hook over several frames
  as the camera turns toward the post (frames 1850 to 2150), or the stores to `+0x245`.
- **What moves in the wind.** The trees' leaves are not physics: each tree is a field object
  that plays a looping motion (docs/formats.md, "Shared field motions"), 4 or 8 s long,
  turning its leaf joints by up to about a degree; the leaf meshes are skinned one joint a
  vertex and the trunks are not. Barrel plays them (`field::Moving`), and the joints agree
  with the capture's (rotation 0.0000, translation within 0.1). All the field's motions,
  the windmills' too, run on one clock that starts with the field and is a frame ahead of
  the sky's. With them the pixel board went from 5.82 / 4.71 / 7.61 to 3.53 / 3.89 / 5.08.
  How the game does it, for the next thing that moves: the objects are `EXFieldObjectMotion`
  (update 0x1407509e0, player advance 0x1406c2870, evaluate 0x1406c4280 into the animation
  evaluator 0x140ac01b0, joints to world in 0x1406eb430). The engine's swing class with
  named parameters (0x14086f2e0: Gravity, SpringCoef, Damping, WindResistance, ...) never
  runs in the field. The plants lean as the game's now that their cards are anchored at the
  foot (docs/shading.md, "the plants"): with a capture's wind (`BARREL_PLANT_WIND`) the
  left view's grass strip went from 23.5 to 12.3 and no other sway phase comes near (24),
  the board to 3.30 / 3.75 / 4.38. Ryza's cloth pieces (NUNO: skirt, jacket hem, hair) run the
  game's solver, read and matched stage by stage against a capture (docs/formats.md "Cloth
  pieces"): the board to 3.12 / 3.63 / 4.21. The hanging lamps do not move in the game: between the
  capture's frames 2500 and 2550 only the four skinned field objects (Barrel's within 0.1),
  the grass and the water change their draws (scratch `moving.py`).
- **Brightness and bloom.** The brightness and saturation are the game's now (above,
  "Field lighting": the sun's key scale, the colour scale, `SaturationScaleEx`). The bloom
  is the game's bright pass and chain (docs/shading.md, "the frame as the game draws it")
  with the set's `fLumiThreshold` and `fBloomWeight`; its star filter is not done. Only
  `s02_l_hap_main` and `s04_l_sad_main` have their own overrides (`env_<field>.xml`): every
  field takes `env_default.xml` here.

## Ryza in the field

- **Ground and walls.** `--ryza` stands her on the field's heightmap (`<FIELD>.khm`, through the
  shared `walk::Ground::of_heightmap`): at the garden gate she stands on the stepping stones as
  in the game. The fields have no `_col.g1co` as Escha's do; their collision is likely in
  `<FIELD>_coll.bin` (224 bytes in Rasenboden), `.g1pf` or `_le.koc`. The `.koc` walls are
  read (`barrel_ryza::koc`, docs/formats.md) and stop her in a window (run from the garden
  gate toward the house, she stops at the quad across its front, z 28966, her radius short of
  it). Some quads there cross each other in an X behind that one, so not every quad may be a
  plain wall (volumes outlined by quads?). Its sloped and flat quads are not used; `_coll.bin`'s boxes and cylinder are
  (half-sizes or full ones, and where a cylinder starts, not checked).
  The placed models' collision meshes (`_cbvh`, docs/formats.md) are: walls and floors on
  models, and she steps over a wall whose top is within a step (60) of her feet. The boxes and capsules are
  used too (boxes as boxes, capsules as eight-sided prisms). Every shape is put where the
  placement list says within its model (`opl2::CollisionFile`: a rotation and an offset for each
  collision file, docs/formats.md) before the model's placement, as the game does. The hulls (`_cplh`) are read
  and used the same way (Rasenboden: 449 of them; with the meshes, boxes and capsules 1,617
  placed shapes, 293,000 triangles; the game builds 1,628, the 11 more being kinds not read). The camera keeps out of
  the ground behind her (`camera::clear_distance`: pulled in where terrain or a floor comes
  within 30 below to 400 above its line, no nearer than 60, easing back out): banks no longer
  swallow it; it also stops short of the first wall its line passes through
  (`walk::Walls::first_hit`). It has no size: a rock just beside the line can still fill the
  view's side. The game's own camera collision and how it pulls in (pitching up?) are not read. Heights look only at the floors in
  their 256-unit cell (`walk::Ground`): 0.6 µs a height in Rasenboden on the PC, from 1.8 ms
  searching all 282,000 triangles.
- **Walking her** (`barrel ... --ryza X,Y,Z --window`, `barrel_ryza::player`). Stand-ins,
  not read from the game: the lean that starts a run (0.6), how fast she turns and the step she climbs (60). The camera looks 128 above her feet
  (measured: the game's eye and her position at the gate with the field camera's 8° and 400;
  about 0.8 of `chara.xml`'s `CamH` 160, the rule not read). With it
  and the field camera's 45°, 400 and 8° (vertical field of view), the scene at the garden gate
  frames as in the game's still (the fence, the path and its steps, the light pole, the trees in
  the same places); she stands nearer the camera in the save than at the spot used here. Her speeds (walk 71, run 432 units a second) are those her
  feet keep still at (docs/formats.md, "Ryza's speeds"): whether the game moves her that fast
  is unchecked. Her size against the walls (28 by 150)
  and her model's scale (1.04) are the character tables' (`barrel_ryza::chara`). The camera's field of view, distance and pitch are
  the field's (`barrel_ryza::camera`); `at_diff_y` and the look-up adjustment are not placed,
  and the field of view taken as vertical frames the gate as the game does. Setting off plays the gait's
  start motion (10, 20) then its loop, letting go its stop motion (12, 22) then the stand;
  walk and run change loop to loop, each change mixing the last motion out over 0.2 s. How
  the game moves her during the start and stop (here eased in and out) and its mixing time
  are not read.
- **Walking her, measured (2026-11-19, ryza-recomp `walk-30`, `tools/walkstate.py`).** Frames
  are drawn frames; the stick acts on the frame it is read (no lag in position).
  - She moves along the stick's direction against the camera from the first frame, at full
    speed at once (no easing in or out); only her facing turns toward it, 0.2 rad a frame
    (11.46 degrees; 12 rad a second, Barrel's stand-in) until within a step, then it keeps with the stick. Barrel moves her along
    her facing: wrong.
  - Speeds by the left stick's lean (of 32767): full 600 units a second, 12000 (0.37) 108.9,
    6000 (0.18) none (a dead zone). The curve between is not sampled.
  - Against a wall she slides (423 to 458 along it at full lean), and stuck in a corner jitters
    10 units a second.
  - The camera: right stick x full turns it 1.5 degrees a frame (90 a second), -20000 (0.61) 0.8
    (a dead zone near 0.165 if linear). Right stick y 20000 pitches 0.38 a frame, rising to 0.5
    and easing to a stop at -9.12 (its limit or the stick's release, not split); the distance
    shortens as it pitches down (400 at 8 degrees, 330.7 at -9.12: the ground pull-in or a
    pitch rule). While she runs it turns toward where she goes by 1.01 degrees a frame times
    the sine of the angle between (0.71 at 45 degrees); how it depends on speed is not sampled.
  - The eye at pitch 8 is 184.2 above her feet, 396.1 out: 400 along the view from a point
    128.5 above her feet.
  - **Built (`barrel_ryza::walk`, the window and `barrel-tool walk` share its `step`).** Dead zones
    0.2257 (left, from 108.9 at 12000) and 0.1651 (right), linear; speed 600 times the lean; she
    moves along the stick against the camera as the frame began and faces where she went (a
    slide along a wall included; still when held), 0.2 rad a frame, the short way round however
    near half a turn (179.2 degrees went the negative way, 179.99 the positive); the camera turns 1.5 a frame, follows where she went at 1.0125 a frame times the
    sine of the angle across its view (0.7125 measured at 45 degrees against 0.716 here: the
    last half percent is not explained); pitch 0.9375 a frame times the stick for 8 frames at
    75%, floor -9.12; below 5 degrees the eye comes in 4.6 and the look point rises 1.71 a degree
    (fitted at the end of the walk: the data's both are linear in time, not in the pitch, and
    the pitch slows over its last 5 frames: not explained). `tools/walkdiff.py OURS KEY`
    diffs a headless walk (`barrel-tool walk`) with the key per frame, a line a segment.
    Left (walk-30), largest first:
    1. Solved: a sloping wall triangle spans only its own heights where she is
       (`walk::Wall::span`). The stair's side, `A21-S07-STAIR-01_cbvh2.g1co`, falls from 6184 to
       5808 along its 1,100 units and had blocked her at frame 2178, about 186 below its top. The walk now
       holds to 11 units and 0.34 degrees through frame 2779 (`walk-30-to-2779`).
    2. The fence keeps her further off than Barrel does: from frame 2917 she slides along
       `A21-COMMON-FENCE-02_cplh1.g1co` (x 31454 to 31797, z 33856 to 33910) about 78 units
       from its face, and Barrel's 30-unit radius lets her 48 closer (52 units off by 2921, 342
       at most by 3079). Open: whether `_cplh` shapes (as against `_cbvh`) are pushed out, or
       she is wider against them, or another shape in front of the fence is not loaded.
    3. The stairs' heights (7 up at the top step, 3.4 along), the camera starting at 393.25 and
       400 from her first step, the pitch's transient (0.6 degrees).
- **Collision shapes outside their models** (small). With the placement list's per-file
  rotation and offset the stairs, floors, tunnel rocks and light poles lie on their models
  (down the garden stairs' middle the floors are within 8 of the terrain; before, the stair
  mesh lay 105 too high). Left: 26 of Rasenboden's 1,617 placed shapes, 1 of the attic's 9 and
  1 of the title field's 1,212 still lie mostly outside their model's bounds, all boxes or
  capsules on props from older games (`a13-case`, `a14-box2`, `a17_...`, `02bag`, a trash can;
  `BARREL_VERBOSE=1` lists them). Each record's last three floats are not read (zero where
  looked). Not checked by a driven walk: Barrel's Ryza has no scripted input yet.
- **Her shading.** Drawn as a field model with the characters' lighting pass as the program
  has it (`model.frag` flag 128; docs/shading.md, "Ryza DX: the character lighting pass": two
  lights with the rim weight, the occlusion's and the shadow's `min`, the ramp's `u` from the
  two lights' half-Lambert terms), the pass's constants the capture's at Rasenboden's noon
  (a stand-in on the constants board: where the game keeps them is not found). Over her
  pixels the game's lit target is reproduced from its own G-buffer to 2.8%; Barrel's differs
  by its inputs (the board's `chara` lines: mean colour within 1%, the sorted luminance
  distributions 2.8% apart). Her forward passes are built (docs/shading.md, "Her forward
  passes"): the specular ambient with her own block and her metal's diffuse (the red sheen
  on her stockings and boots, which the previous reading took for the hull's), and the hull
  (a dark line on her silhouette and seams). The half-size screen-space reflections
  the specular ambient mixes toward are built (`reflect.rs`; the march's weight over her thighs
  0.25 against the game's 0.26, the boots' 0.44 against 0.44; the board's `reflections` and
  `reflections_alpha` stages). They explain only part of the excess: where the specular
  ambient is brightest its weight is low (the factor-weighted weight 0.16, so the reflections
  take 8% of her thighs' red at the gate), and her stockings and
  boots at tone are still +8% and +4% at the gate (red; 55.0 against 50.9 and 35.9 against
  34.4), -7% and -8% at the turn (the reflections take more there than the game's own light
  leaves), +6% and +2% at the left. What is left is in the specular ambient's own terms, not
  the reflections (stand-ins: the scene the march reads holds the cube light, the terrain takes
  none). Left: the hull's own
  fog (nothing at her distance), the face's and the townsfolk's faces' forward programs
  (draws 2025 to 2031, not read), the normal map in her lighting (her G-buffer program reads it as `rgb x 2 - 1`, normalised, in the
  mesh's tangent frame, not the field models' red x alpha; her draws have no free texture slot for it: the
  mask slot holds the ramp, set 3 her occlusion map, set 4 her specular map; the vertex normal is
  used), the direct specular term (0.4% of her light), the back-light amount, the second light on
  fields where it differs, the hair highlight is laid on fixed (the game slides it with the
  view's pitch). Which override a field uses (most have none: `env_default`) and the point
  lights' share of the pass are not done. A field's gimmicks are drawn as the stage's models,
  as the game does (the notice board at the gate's left; `actors::add_character` without the ramp
  rows). Her own G-buffer programs are not read: her mesh names them by hashes that match no
  shader pack's name (docs/formats.md, "Ryza's character files"); her materials' ramp rows
  (`rampIndex` 3 for her coat's cloth, 1 elsewhere) are read. Her face takes its default expression
  and blinks (every 4 s, a stand-in timing); the other expressions and the eyes' directions
  are not used. Her skirt, jacket hem and hair move by the game's cloth solver (the model's NUNO
  pieces, docs/formats.md "Cloth pieces"). Measured against ryza-recomp's `walk-rasen` (her cloth's
  control points in the capture's `vCtrl`, every 5th frame 1880 to 2100, in each piece's parent joint's
  space; `tools/ryza_cloth.sh`, `barrel-parity cloth`, docs/parity.md): the hair's two pieces
  (parent joint 133) were 13.7 and 13.3 cm rms off over the run (31 cm at worst) and are 2.1
  and 2.0 (9.3 at worst); the skirt was 15.8 cm rms and is 3.1; the whole of her cloth was 10.5
  and is 3.2. Three causes: (1) the pins of the hair pieces were carried by the wrong joints (the
  palette's bone ids, 331 and up, were used as joint numbers: the head pieces' pins ran through
  the hat; the skirt's low ids hid it); (2) the parent's velocity and spin were remembered
  clamped, so the drag and the spin term never saw the run (docs/formats.md, step 2); (3) her
  run loop's phase against the start motion (pelvis pins 3.1 cm off over the run, 1.0 with a
  fitted 0.35 s, `player::RUN_LOOP_PHASE`, a stand-in; docs/engine.md). The game's hair points
  lie inside the piece's colliders (Barrel's, deeper than 0.05) 17 and 18 times in the 44 frames,
  by up to 2.1 cm; Barrel's, 0. Pictures of her head at frames 1915 to 1960 from both sides,
  before and after (`tools/ryza_cloth_view.sh`): the strand through the hat is gone. Left: the
  skirt and the jacket's flaps are still 3.0 to 4.2 cm rms (the skirt's pelvis pins 1.4 of it,
  the rest the solver in motion; the skirt's worst is 28 cm at the stop, frame 2025, where
  Barrel stops her at once and the game's pose settles over some frames) and the first frames of
  the run (hair 9 cm at 1915). The parent's rotation limit is the jump test only (`rot` above
  pi/8, docs/formats.md); nothing more of it is read. The pieces' collider groups hold no hat
  collider (the hair's is the head's), and with the pins right the hair clears the hat. The
  `.swg` settings (10 records over 652 swing bones, docs/formats.md "Swing settings") are a
  separate system the field does not run for her: not built, and nothing in the capture moves
  that the pieces above do not. The loop phase of her other gaits is not fitted (the capture
  has only the run). Check rows likely to move: the `walk-*` rows compare her path only and do
  not; the rows that draw her (`rasen-3d-*`, the pixel board's character pixels) see the
  hair at its pinned place now.

## On handhelds

- **Geometry, not pixels.** Rasenboden from the garden gate draws 2,087 draws and 1.85 million
  triangles, every placed object at full detail. On the PC (RX 9070 XT, 1920x1080, offscreen,
  60 frames, median) the frame is 5.97 ms at the desktop preset and 5.89 ms at the handheld one
  (75 % scale): the scene pass barely changes with the resolution, so it is bound by vertices
  and draws. The field's draw distances count only below the top object-distance setting,
  as in the PC game (`docs/renderer.md`, "Ryza DX: object draw ranges"; `barrel --draw-ranges
  pc|nx` forces them on, and these timings were taken with them on). Choosing the Switch's
  ranges belongs with the handheld preset (a graphics setting, not yet). The timings below
  are from before objects were culled whole and before levels of detail were tied to the
  setting: to be taken again.
  Where the frame goes at the gate (1920x1080, desktop preset, 57 frames, median 5.83 ms): the
  shadow map 1.70 ms, the scene 4.04, the rest 0.08. After view culling, by placement class
  (draws / triangles):

  | pass | near | middle | far | landmark |
  |---|---|---|---|---|
  | scene | 39 / 51k (with Ryza) | 62 / 67k | 146 / 67k | 472 / 430k |
  | shadow map | 72 / 54k | 27 / 21k | 53 / 20k | 142 / 160k |

  The shadow map (3000 units square around her, ±8000 along the sun) draws 255k triangles,
  the scene 615k: about the same cost a triangle, and two thirds of both are landmarks (the
  canyon's cliffs). The game's own shadow range (`disp_range` `shadow`, 300 m) cuts nothing
  here. For the RP6: the landmarks' triangles, then the shadow map's casters. The scene's
  landmark triangles at the gate by model: `a21-common-cliff-block-30m-02` 110k (80 draws),
  `a21-common-desert-cliff-06` 52k, `a21-s06-windmill-01` 35k (95 draws), the houses
  (`a21-s06-house-01`, `-03`, `-04`) 20k to 26k each, `a21-common-mountain-02` 18k, the rest
  under 15k. None of these has a `_mid` or `_low` model, and a `_hi` model holds one level (its
  mesh groups are all LOD 0), so the game's LOD would not lighten them; whether 615k triangles
  and 717 draws are too many for the RP6 is for its timings. The placements'
  classes (docs/formats.md, "Placement lists") put the cost in the landmark class: leaving each
  class out in turn (1080p, 37 frames), near saves 0.36 ms, middle 0.39, far 0.51, landmark 3.9
  of 5.9 (646 cliffs and mountains, 1.26 of the 1.85 million triangles). Landmarks are drawn to
  3000 (the farthest), so distance culling saves little; lower-detail models exist for few
  objects (26 `_mid`, 14 `_low` in Rasenboden's objects, mostly flowers, carts and the woods; the
  cliffs have only `_hi`), and the PC game runs with `use_lod` off. Whether the RP6 needs more
  than the culling we have is for its own timings.
- **On the RP6 itself** (Adreno 740, Turnip, Mesa 26.2.4; 2026-11-14; the aarch64 package
  from `tools/package.sh aarch64 … ryza`, the game read from the device's own install on its SD
  card; offscreen, 1920x1080, the garden gate at noon, 240 frames, median of 237). It runs
  unchanged and draws as on the PC. Desktop preset: 18.8 ms (shadow map 2.67, scene 15.73, glow
  and AA 0.24, post 0.18), 225 MB of textures, 44,015 plants: short of 60 fps. Handheld preset
  (75 % scale, Middle tables, mip bias 1): 14.5 ms (shadow map 1.25, scene 11.83, glow, AA and
  upscale 1.20, post 0.19), 59 MB of textures, 32,980 plants: 60 fps with about 2 ms to spare
  at the gate. The scene pass is three times the PC's and still most of the frame; the
  landmarks' triangles (above) are where it goes next, and a walk through the village (not
  only the gate) is still to be timed.
- **Floors on models**: read, see "Ryza in the field".
- **The game's own graphics options (2026-11-19).** In play on the RP6 Barrel ran at 13 fps:
  not the handheld preset (an old shared `graphics.conf` of Escha's was copied in, fixed), full
  plants (120,884), 3,974 draws, and Ryza's reflections, depth of field and HDR scene forced on
  whatever the preset (`crates/ryza/src/play/mod.rs`, the `hdr_scene: true, reflections: true`
  line). The original on the same device runs with its own `Setting.ini` `[Graphics]`:
  `GRAPHICS_QUALITY_ALL=3`, `GRAPHICS_QUALITY_TEX=1`, `GRAPHICS_ANTIAILIASING=1`, every other
  item 0 (shadow, RLR, effects, DOF, AO, bloom, light shafts, draw range, point lights, grass
  range and density, filtering, shadow distance, water, LOD). A fair timing needs Barrel's
  Options to be the game's: the same items, the same values, the presets' tables (docs/formats.md,
  "Ryza DX: the graphics presets"). Not read yet: what each value means (which index is High,
  whether 0 is off for the switches, what QUALITY_ALL 3 is).
  Timed on the RP6 the same day (offscreen, 1920x1080, the gate after the opening,
  `--done MM00_010`, medians): the settings it played at (scale 1, full plants, AO, reflections
  and depth of field) 87.5 ms: AO 15.3, reflections and depth of field 24.4 together (they run
  only with AO on), the rest 44.4. The handheld preset 34.9 ms (it was 14.1 on 2026-11-14's
  build: that run had no `--done`, 32,980 plants in 66 draws against 90,462 in 1,432 after the
  opening). Bisected on the device at the handheld preset: the plants after the opening +1.8 ms;
  the three cascades and static layer (21ba11a) +6.4 (shadow +3.4); 20bcaef..688b35c +2.4; the
  characters' map's 16 taps over the whole view (bcfe208) +4.8; 5204466..eb78689 +4.4, not
  split yet. Each is the game's own pass; whether the game's Low settings skip or shrink them is
  what reading its settings answers.
  The original's values, read off the answer key (2026-11-19: the gate at frame 2550 with
  `Setting.ini` variants, runs `gs-q0`..`gs-q3`, `gs-x-*` one item at a time, `gs-rp6` the
  handheld's file): `GRAPHICS_QUALITY_ALL` 0 Low, 1 Middle, 2 High, 3 Custom; below 3 it decides
  everything and the items are ignored; at 3 the items are used, 0 the lowest. The handheld's
  file is Custom, nearly all Low: 1,346 draws against High's 2,149, without AO, depth of field,
  bloom and star, light shafts and TAA; the three cascades and the characters' map keep their
  sizes at every level (2000x2400, 2000x2400, 1600, 1024) and the reflections still run. A
  structural cost of Barrel's: its AO, reflections and depth of field need a second geometry
  pass (normals and depth at half size, `ao.rs`) where the game's deferred G-buffer already
  holds them; on the RP6 that pre-pass and AO are 15 ms of the played 87.
- **Optimised on the PC, not yet timed on the RP6 (2026-11-19).** Four changes, the gate after
  the opening at 1920x1080 with the handheld's own `Setting.ini` (`--setting-ini`): the shadow
  lookups as gathers (54f6857), the reflections' and depth of field's normals and depth from the
  scene pass so the second geometry pass runs only with AO (9d3ce8d), the bloom's passes
  skipped with a glow rate of 0 (c1da9fd), the casters' bias in the raster state so the shadow
  pass writes no depth per pixel (1a0af32). The GPU's stages went from 3.68 to 2.66 ms (shadow
  1.62 to 0.91, occlusion 0.22 to 0.01); High from 5.27 to 4.34. A tiler should gain more (the
  geometry no longer drawn twice, the casters' depth test early). Next on the RP6: time the
  handheld's file against the 87.5 ms played and the bisect's numbers, then split
  5204466..eb78689 (+4.4 ms there). Left on the PC: the casters' alpha test (0.55 ms with none:
  a texture with no texel under the reference could skip it, the plants cannot); the scene
  stage grew 0.06 ms (0.3 at 4K) with the normals written in the scene pass, not explained.
- **Timed on the RP6 after the four (2026-11-19, d27631b).** The gate after the opening,
  offscreen at 1920x1080, 240 frames, medians; GPU pinned at 680 MHz, the device cool. The
  game's Low (`GRAPHICS_QUALITY_ALL=0`, the file the original had just played with): 51.9 ms at
  scale 1 (shadow 6.2, scene 44.9), 40.5 at 0.75 with FSR 1. The handheld's earlier Custom
  file: 52.4 / 37.2 (0.75, shadows low) / 27.8 (0.5). The original on the device at Low,
  1080p (the user's counter, through FEX and Proton, not at the gate exactly): 37 fps, about
  27 ms. So at the same settings the original is about twice as fast, and nearly all of
  Barrel's frame is the scene pass. The pass fits about 15.6 ms that no scale removes plus
  about 30 ms per full 1080p frame of pixels. Not the draw range: 3,974 is what is loaded,
  the scene draws 697 after culling (plus the plants' 1,432). Suspected: overdraw of the full
  forward shader (the plants' alpha-tested layers) on a tiler, where the original is
  deferred. Next, a few runs to split it: no plants, a trivial fragment shader, a depth
  pre-pass.
- **For the tiler, on the PC (2026-11-19, d6fd963 to 1992738).** What the pass's cost was
  on the PC: 9.0 million pixel-program runs for 2.07 million pixels, 5.2 million of them the
  far trees' alpha-tested cards. Now the scene pass is in three parts (opaque textures without
  the alpha test; the tested draws' depth alone; their shading over it, no depth write) with
  1.21 + 1.05 million runs of the whole program and 4.94 million of the test alone; the field
  models and the others have their own pipelines (`KIND`: on Turnip for the 740 the field
  models' about 5,100 instructions and 19 vec4 registers, the others' about 1,900 and 12); the scene keeps 20 bytes a pixel
  where it kept 28 (the reflections' inputs at 4 bytes each); shadow casters with opaque
  textures are drawn without a fragment program; each shadow pass covers only its map; textures
  are bound only on a change (about half the binds, which a tiler replays in every bin); without
  motion vectors the vertex program writes no clip positions and skins once. On the PC the
  gate is 1.92 ms where it was about 2.7 (shadow 0.76, scene 0.68, reflections and water
  0.38). Every check holds; the format change moves 1.2% of the pixels by at most 4 levels but
  5 (docs/engine.md). Turnip's sources (26.2.4): the statistics query does not turn GMEM off
  (only primitives-generated queries do), and the depth part's draws do not cost LRZ its test.
  Not tried on the device yet: the model program's 16-bit colour build (`BARREL_HALF=1`; RADV
  ignores RelaxedPrecision, so only the device can time it and show its pixels). Next on the
  device: `work/rp6-time.sh` (Low, Low with HALF and the two images compared, the handheld
  preset), then `MODE=trace work/rp6-time.sh 30` (Turnip's u_trace per render pass: GMEM or
  sysmem and why, bins, LRZ, GPU time; `work/rp6-trace.py`). That says whether the 15.6 ms no
  scale removes is the bins replaying draws, the vertex program, or a pass Turnip runs in
  sysmem.
- **On the RP6 (2026-11-20, a1f0c1a to 7860ae8).** The gate at Low 1080p: 51.9 ms (d27631b),
  46.3 with the night's PC work, 34.9 now; the game's own is about 27. The trace showed two
  passes out of proportion: the reflection march (10.3 ms: each step read the whole rgba32f
  texel for its depth; now an r32f target of its own, 1.8 ms) and the forward pass over the
  characters (4.1 ms: every toon-ramp draw of the area, 358 draws; now the scene's drawn
  ones). The HALF build: 1.3 ms off the scene, 4% of the pixels by up to 11 levels (not
  taken). A d16 shadow map: 0.5 ms (not taken). Left, by turning parts off: the scene 23.7
  ms, of which the models 18.7 (their shading 10.5, the sun's shadow lookup 7.7 of the
  scene with the terrain's), the terrain 3.9, binning 3.9 (888 draws, 14 bins of 960 by 160
  at 20 bytes a pixel); the shadow maps 6.3 (587 casters). Next: a position-only vertex
  stream for binning and the depth part (the vertex is 72 bytes), and fewer bytes a pixel in
  the scene pass.
- **The files.** A Rasenboden run (with her) reads 22 files from four archives (2026-11-14):
  `PACK00_01.PAK` (3.06 GB: only `shader\texture\chara_rmp.g1t`), `PACK00_02.PAK` (2.04 GB:
  her model, face and motions), `PACK00_03.PAK` (13.69 GB: the field, its objects, environment
  and walls, the sky dome) and `PACK02.PAK` (0.05 GB: the tables under `saves`), 18.8 GB of
  the install's 21. The voice archives `PACK00_04_*` (3.6 GB) and `Data/x64` (6.1 GB: sound
  and UI resources) are not read yet.

## Running the original

- **Without the Steam client**: the executable imports `steam_api64.dll` (callbacks,
  achievements, Steam saves) but no restart-through-Steam check was found among its strings;
  whether it starts without the client is unchecked.
- **Levels of detail.** Rasenboden's object container holds 138 `_hi` models, 26 `_mid` and
  14 `_low`. `barrel` loads all three (`Placement::lods`, `Draw::lod`) and shows each by
  the distance from the eye to its box's centre at the game's High LOD preset (10000, 30000;
  docs/formats.md, "the graphics presets"). In a far view of the village it saves about
  0.08 ms of 7.2 on the PC with 12 pixels changed; the landmarks (cliffs, windmills) have no
  lower levels. The graphics' `foliage` picks the game's general preset
  (`barrel_ryza::presets::Graphics`: full High, reduced Middle, least Low), and the plants and
  these distances come from the tables it names (Middle 8000 / 25000 on the handheld preset).
  The general preset's object draw ranges (`DispRangeType`) take the place of the field's
  own below the top setting and are not used at it (`docs/renderer.md`, "Ryza DX: object
  draw ranges"): the handheld's Middle (7000, 15000, 30000) drops far small objects. The shadow distance (`ShadowDistType`: 1, 0.7, 0.5) scales the
  cascades' ends (1600, 3600, 10000 on High; 0.7 times them on the handheld's Middle): a
  guess at what it scales, as the game's own use of it is not read.
  In the far view 0.5% of the pixels change and the time does not (the far layer is redrawn
  only when it moves). The texture mip bias (`TextureType`) leaves out each texture's largest
  levels at upload (`g1t::Texture::without_top_levels`, kept at 4 × 4 or more): Rasenboden's
  526 textures take 224.9 MB on the desktop preset and 59.3 MB on the handheld's (bias 1), at
  the gate 5.73 against 5.60 ms (PC); the picture softens as the game's Middle does.
- **`SaturationScaleEx`** (`env_override.xml`, docs/formats.md): if Rasenboden is
  `LGT01_DEFAULT_01`, a saturation of 1.6 at noon on top of (or instead of) the environment's
  `vSaturationScale` (1.27, 1.3, 1.2), which matched the noon still worse when laid on as
  saturation (so Barrel applies none). Which set a field uses and where the game lays the
  scale (everything, or the characters: `S999_CHARA_LIGHT`) are unknown; a reference frame of
  a saturated object at noon against Barrel's with each guess would tell. Also the fifth time
  zone, `TIME_ZONE_MIDNIGHT`, and which of the four sets it shows.
- **Plants against the measured constants** (docs/shading.md, "the plants' constants as the
  game sets them"): the sway is now the game's own formula with its measured `g_vAttCache0` and
  `g_fOscPhase` speed; only the gusts' curve over time is not read (the strength holds 9.6, the
  middle of the 8.7 to 10.5 seen), and the plants' own phases differ from the game's since
  Barrel places its plants itself. The plants sample no pigment map at the gate, so their
  teal is not a missing pigment: the pixel side (`cDiff`, `fPtn`, the ambient terms of
  `objectgrasspbs`'s 416-byte block) is not in the saved ring where looked: the lit blocks
  beside the plants' vertex blocks (sun (−0.354, −0.707, −0.612), `litCol` (4.78, 5.0, 3.5),
  `ambLit` (3, 3, 4), `cDiff` 1 or 0.8, `fPtn` (1, 1, 1, 20)) end in matrices, not the ambient
  terms, so they are other programs'. Against the matched still (`rasen-gate-noon.png`) the
  game's plants at the gate are dark green (about 12, 44, 36 in a clean patch) where Barrel's
  are lighter and bluer: too much of the bluish ambient, or a darker card texture. A run that
  reads the block by its own layout (the 416 bytes after `vEye` at +80 with `vATest` at +176)
  would settle it. `emScl` by evening and its
  flicker need a run at evening.

## Outfits (B4)

Step 1 of the outfit mods: `barrel-tool chara-gltf` (docs/formats.md, "glTF export") and
`tools/blender/render_glb.py`. Checked in Blender 5.3 (Workbench, flat): every costume exports
with every vertex weighted, and the glb of `pc00a` posed with motion 0 (idle, frame 20) and
motion 21 (run, frame 8) shows no torn or stretched mesh (hat, vest, sleeves, skirt panels, boots
all follow; looked at front, side and back). Every submesh of every costume is drawn by the
colour pass at LOD 0 (the models have no other LODs; no shadow-only submeshes were skipped:
`skipped []` in each export). Model sets that exist: `pc00a` to `pc00i`, `pc00k`, `pc00l`,
`pc00a_wet`; `pc00j`, `pc00m` ... and other `_wet` sets are not in the archives.

Pieces by shader hash, the same in every costume: `E24D26E6` eyes (2), `6679C989` face and mouth
(2), `B82041B9` eyebrows, `C86A7C58` hair strands, `E905E6C7` the hair, `9767D5FD` a 290-vertex
bowl (the cap under a hat), `339F19F6` the outfit and the skin pieces (arms, legs, neck, hands),
`26EDF6BF` more skin or small accessories, `CB0965E5` and `4196223C` the cloth-simulated pieces
(cape, sleeve ties, skirt panels). Piece columns: judged from `--each` sheets for `pc00a`, `pc00i`
and `pc00l`; the rest from the same hashes and vertex counts (marked ~).

| Costume | Looks like | Submeshes | Pieces (submesh numbers) |
|---|---|---|---|
| pc00a | default: yellow hooded vest, cream top, red shorts, thigh boots, beret | 25 | cloth: vest 0, pendant 1, flap 2, sleeve ties 3-4 and 6-7, frill 5; eyes 8-9; face 10-11; outfit (top, shorts, belt, boots in one mesh) 12; accessories (necklace, clips, sleeve pieces) 13; ribbon/glove bits 14; body skin 15-17; inner top 18; cap 19; hair 20-22; bottle 23; brows 24 |
| pc00b | blue smiley tee, black shorts, sandals, blue slime beret | 15 | eyes 0-1; face 2-3; tee, shorts, sandals 4; skin 5-9 (~); cap 10; hair 11-13; brows 14 |
| pc00c | white graphic tee, red shorts, sandals, cream beret | 15 | as pc00b (tee 4, skin 5-9 ~) |
| pc00d | white cropped blouse with bow, frilled bloomers, grey stockings, brown boots, no hat | 25 | cloth (blouse/cape and ties) 0-9; eyes 10-11; face 12-13; outfit 14-15 (~); skin 16-19 (~); cap 20; hair 21-23; brows 24 |
| pc00e | default in black and white, black beret | 25 | as pc00a (outfit 12, skin 14-18 ~) |
| pc00f | orange vest, light-blue sleeves, blue shorts, pale-blue beret | 25 | as pc00a |
| pc00g | pink vest, yellow shorts, pale-blue beret | 25 | as pc00a |
| pc00h | default without the vest: camisole, red shorts, thigh boots | 18 | cloth 0-1; eyes 2-3; face 4-5; outfit 6; skin 7-11 (~); cap 12; hair 13-15; bottle 16; brows 17 |
| pc00i | white bikini and sandals | 16 | halter ties (cloth) 0; eyes 1-2; face 3-4; bikini top and bottom, headband, sandals 5; body skin 6-9; cloth patch 10; cap 11; hair 12-14; brows 15 |
| pc00k | light-blue party dress, lace shoulders, heels | 16 | eyes 0-1; face 2-3; dress, shoes 4-5 (~); skin 6-10 (~); cap 11; hair 12-14; brows 15 |
| pc00l | red and black dress, stockings, white gloves, hair bun | 29 | cloth skirt panels 0-13; eyes 14-15; face 16-17; stockings, shoes, skirt 18; bodice, cuffs, chain 19; gold pins 20; gloves 21-22; body skin 23; dress top 24 ~; cap 25; hair 26-27; brows 28 |
| pc00a_wet | default, wet look (muted colours, hair stuck down) | 25 | as pc00a |

Swimsuit: only `pc00i`. Its bikini is (c): its own submesh (5, with the headband and sandals)
over a complete skin body (6: torso, arms, hips, with the swimsuit's area tinted); the skin is
not cut away under it. (`pc00b` and `pc00c` are tees over shorts, not swimsuits.)

Left over, not solved:
- `pc00l`'s cloth submeshes (0-13: the red skirt panels, ribbons) hang as flat fans at her feet in
  the bind pose: placed from the cloth control points at rest (`place_cloths`), which the game's
  simulation then moves; the export carries them rigid on the cloth's parent joint. Her other
  costumes' cloth pieces (vest, sleeve ties) sit right.
- The game's shader hashes have no readable names, so a material's `extras.shader` is the hash.
- Eye overlays, hair highlights and the toon ramp are carried only as extra images; the base
  colour is the albedo alone.
- Costume `pc00a_wet` and the others were not looked at per submesh (marked ~ above).

### Field menu, part 5: the character pick (A on Equip or Status), and the way back from a page

Built (rows in the local `field-menu-rows-pages.txt`, not in checks.txt; `field-menu-equip` and `field-menu-status`, offscreen with
`--menu-keys`, `left` and `right` move the pick):
- A on Equip or Status starts a pick whose first frame is 42 frames later (`PICK_AT`; the pad's A on 2060, first frame 2102): the menu
  stays as the A pulse had it until then, with the bar going out as on the Settings button (`Bar::out`), so 2070 to 2100 are the old rules.
- From the first frame: the buttons darken (`anm_select_blackout` run in 20 frames, 255 to 179, rounded per channel), each button's
  glow goes, each button's name plate and base are drawn once more through the game's blurring shader (alpha cubic over 20 frames; in
  the trace only, `Pen::hidden` makes them the mask of a clip that draws nothing), the level plate and money go out as on B (out cubic,
  `CLOSE_MONEY_SPEED`), the photo's blurring quad (`blur_bg`, `sys_white`, cubic over 30 frames) is in the trace over the photo and
  the character's sharp figure (`a21v2_main_menu_image`, the style of its `list_0` image) is drawn after the mask.
- The picked card: its glow (`bg_hilight_on`), the ring (`anm_cursor_scale`) and the turning ring (`anm_cursor_rolling`, 0.75 degrees
  a frame, `a21_common` 5) behind its plate and the blinking glow (`anm_status_blink_loop`) over it, all added; the loops start on the pick's
  second frame, or a frame before the key that moved the pick to the card. Left and right go along the cards left to right and stop at
  the ends (STAND-IN, an end not tried). The party's order is `View::card_order`.
- The pick's bar: from 4 frames after the first frame, three entries ("Select Character", "Back", "Confirm"), the plate growing over
  6.4 frames as it fades in over 15.
- B in the pick: back to the menu at once (STAND-IN, not captured). A on the card (`Pick::confirmed`): the page opens; B on the page: the
  menu is back 24 frames later (`PAGE_OUT`): the cards and the buttons come in as after the Settings screen, the photo's blur goes out
  over 30 frames, the blurred picture and the vignettes of the page go out over 7.5 (the `backdrop` layer), the menu's bar from 33 frames
  with its plate 0.2 frames early (STAND-IN, one sample).
- The menu ring's loop is 121 frames (it was 120.75); after the Settings screen it runs a frame ahead (`Nav::ring_skew`).

Left, with causes:
- The Equipment and Status pages are NOT drawn: while a page is open Barrel shows the pick (the frames from the second A to B). The Equip
  frame 2150 on has ~860 quads (the layouts `uil_a21_menu_equip`, `uil_a21_item_status`, `uil_a21_menu_chara_status`, text with its
  nine-copy outline, the item icons, the stat panel); stats need the character growth table (not found, the cards' HP stays the STAND-IN
  `MAX_HP`).
- Status 2260 to 2320 (4 mismatches each): after LEFT back to the first card the game's ring and turning ring are 17 frames into their
  loops (a start at 2243), not at the key's frame; no rule found (not a pause: 97 frames of run before the move).
- Equip 2590 (3): the cursor glow's closing fade after B at 2580 is 9/255, the fitted `CLOSE_GLOW_*` gives 20/255 (the 2560 fit was
  taken after a B straight out of the menu).
- The photo's blur is not drawn (the quad stays in the trace); the page's own backdrop in is not drawn either. A window run was not made.
- Offscreen, the photo shows white in the menu's pictures already (main too); not looked into.

### Field menu, part 6: the stats rule and the Equipment page

Built:
- The numbers (`crates/ryza/src/party_stats.rs`, rule in `docs/tables.md` "Ryza DX: a party member's numbers", tests with the
  capture's Lv 3 numbers): the cards' HP is the rule's (`MAX_HP` is gone) and the page's panel shows HP, attack, defence, speed,
  equip quality and dexterity (`save::members` reads the worn items from the save's `party` record, `docs/formats.md`).
  STAND-IN: the +3 on attack, defence and speed is read as the equip quality (level 3 gives the same number).
- The Equipment page (`crates/ryza/src/field_page.rs`; Equipment only, A on Status still shows the pick): from 10 frames after A
  on a card (`PAGE_AT`, STAND-IN) the menu draws only its background, the photo, the mask and the corner picture, the `backdrop`
  layer comes up over them and the page's own layer draws the character's figure (`etc_a21_chara_stand_card`), the four slots with
  the worn items' pictures and the cursor's glow, and the stat panel (window, name, lines, labels and numbers). A small generic
  walker reads the layouts (`uil_a21_menu_equip`, `uil_a21_menu_chara`, `uil_a21_menu_chara_status`): nodes by place, turn and
  scale, images by style (the style files name their texture and place in it), texts with the outline of `bold_size`, clones by
  `copy` and `command`. DOWN goes from the weapon to the first accessory (the slots are two rows of two), RT and LT (offscreen
  `rt`, `lt`; Q and E in a window) go round the party.
- Nothing is a frame at 0: the whole page is not drawn (below), so no row was added (the frames are 800 or so mismatches each).

Left, with causes:
- The item card (`uil_a21_item_status`: the big paper `a21_item_info`, the item's name, level, quality bar, Effect, Status, Trait,
  Change Role and Equippable panels, about 600 of the page's 860 quads), the title plate "Equipment" (`a21_caption`), the party
  icons with LT and RT, the Element Resist icons, the help bar and the lines' nine-slice caps are not drawn. The Status page (tabs,
  skills list, name plate) is not drawn at all.
- No animation: the page appears whole from its first frame (`PAGE_AT`), its blur backdrop comes in over 7.5 frames; the slot
  cursor and the figure change at once (the game slides the card and the figure and fades the names), the cursor's blink
  (`anm_common_blink`) is not run, the glow is steady. B on the page takes the page away at once.
- The figure's quad: the capture's trace has it 1240 x 2078 pixels with uv 0 to 2.07 (twice the file's
  size over the texture's: not explained); Barrel draws it at the style's size, which is what the pixels of the
  game's picture show (Ryza's face at (200, 185) of 720): the rows would disagree on this quad's rectangle.
- A window run was not made (this session had no window): the log of each page is unverified; offscreen `--menu-keys` runs show the
  Equipment page at 2290 and 2410.


### Field menu, part 7: the rest of the Equipment page's static parts

Built (`crates/ryza/src/field_page.rs`):
- The figure's quad is explained: `uis_a21_chara_stand_card.xml` (like `uis_a21_bg.xml`) says `resolution_hd="1080"`, so its style pixels are
  screen pixels of 1080 lines (1.5 to a layout pixel) and its uvwh over the texture's own size, not the 2160-line files' 3 and 2 (the
  capture's 1240 x 2078 quad with uv to 2.07: the picture's own 600 pixels and the clamped margin). `Pic` carries the file's scale.
- `wrap` pictures (`prim_type`, `edge_margin`) are nine-sliced with `options_view::segments`: the stat panel's lines (caps of 25, tiles of
  140 along the 324 layout pixels).
- The element resists (`att_d`: six `icon_off` pictures, the arrows and the lit `icon` are for the game's code), the title plate
  (`uil_a21_caption.xml`, "Equipment" = string 6684752), the party icons with the shoulder buttons' glyphs (`uil_a21_menu_faceicon.xml`: the
  last places of the nine, the left button's node at the first icon) and the help bar (`field_menu::View::page_help`: the menu's plate and
  words, entries "Select", "Back", "Confirm", drawn into the page's layer after the page).
- Text with a node scale of two values (`label_all` "Equip Quality" is 0.92 across): glyph width and advance by the first, height by the second;
  `align_v="center"` lifts the line.
- Animations: the cursor's blink (`anm_common_blink`, restarts when the cursor moves to a slot) and the party icon highlight's turn
  (360 frames, from A on the card) and glow (`expandEff`, 100 frames).
- 2330 to 2370, 2410 and 2420 of `field-menu-equip` are at 0 (rows in the loop's scratch file, not yet in checks.txt).

STAND-INs: the icons fill the last places of the nine; the button glyphs (U+04E6, U+04EA) are the capture's; `CENTER_LIFT` (0.6875 of the
font size) is fitted to the two shoulder glyphs; the spacing of a squeezed label takes the square of the squeeze again (fitted to "Equip
Quality"); the glow's 2.1 frame lag and the blink's 1 frame lag are fitted to 2410.

Left, with causes:
- The item card (`uil_a21_item_info.xml`, `equip_arms` at the item-info position (354, 7), the paper `gen_a21_item_info02_win_l`, the
  equipped-by panel `fl_sub2` at (38, 568), the name, level, quality bar, Effect, Status, Role and Potential panels from
  `uil_a21_item_info_clone_targets.xml`): about 650 of 2290's quads. Not built: it needs the item table's effects, status, roles and the
  save's item unit (potentials), none of which are read yet; the layouts and styles (`uis_gen_a21_item_info.xml`, loaded) are ready.
- The cursor blink's clock is one frame behind Barrel's from 2440 on (1 mismatch each: 2430 to 2490), the game's own hitch or a clock rule not
  found; the character change (2380 to 2400: the figure and name slide, 2 or 3 quads), the page coming in and the card sliding out
  (2300 to 2320, 2150 to 2200) and going out (2500+) are not animated.
- The Status page is not drawn.
