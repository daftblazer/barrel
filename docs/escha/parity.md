# Escha parity scoreboard

Each row is a scenario. Its game capture comes from `../escha-recomp/scenarios/<name>.sh`
(`tools/scenario.sh`), and Barrel's trace from the matching Barrel run, diffed with
`barrel-parity diff` (docs/parity.md). Mismatches count the diff's lines, game frame against
Barrel frame.

| Scenario | Game frame | Barrel | Mismatches | Notes |
|---|---|---|---|---|
| boot-title | 1400-1480/20 | `BARREL_TITLE_FADE_AT=<(frame - 478.67) / 60> --title`, frame 0 | 0 at 1400, 1420, 1440, 1460, 1480 (was 1) | the prompt's pulse is a clipped triangle: straight lines of 204 bytes in 72 frames (2.833 a frame) between 0.2 and 1, peaks 145.33 frames apart on average (`title::prompt_at`), the first at the title's start, frame 478.67 of the recording (the Gust logo has faded out at 477, the title's first frame is 479; the captured frames 480 to 1499 give the peaks at 624, 769.5, 915, 1060, 1204.5, 1350.5, 1496). The game's period wanders by a frame from cycle to cycle, so the fit is within 1 byte at 481-700 and 1300-1499 and within 3 elsewhere (the checker's colour tolerance is 2.55 bytes). Check: `BARREL_TITLE_FADE_AT=15.3555 PARITY_OUT=$OUT/t tools/parity.sh .../traces/boot 1400 --field auto $OUT/t/f.png --size 1920x1080 --title $G` (15.3555 = (1400 - 478.67) / 60; 16.3555 for 1460) |
| load-field (area map) | 3000 | `--load barrel-user.save --area-map`, frame 0 | 0 | at parity (the gauge fills in steps of 2: 45 shows 44) |
| battle-walk (first turn's command row) | 3000 | `--field AREA_03_02 --load GAMEDATA00 --battle 25 --frames 235`, frame 234 (the flipbook phase that fits best), `BARREL_BATTLE_COUNT=3 BARREL_BATTLE_ROLLS=24,33,31,13,23,10` (rolls that give the game's turn bar; the bar shows the order, not the draws) (the group draws 3 to 4 monsters at random; the game's battle had 19, 19, 13; the variable fixes only the count, `BARREL_ENTRY_DRAW`'s way) | 0 (196 at frame 0, 8 → 7 → 3 → 0) | none; the edge-on card turn (`battlehud::TURN_KEYS`) holds the chosen place's frame and diamond as the game does (open.md for the curve between the keys) |
| area-requests | 2220 | `--field TOWN_04 --load GAMEDATA00 --area-map --requests`, frame 0 | 1 (was 2; 0 with `BARREL_CLOCK_AT=2424`) | the cursor glow (part 0x9f) turns with the clock (eight cuts of 8 ticks; 4, 7, 1, 4 at frames 2200 to 2260) and is drawn last, after the rows (the old second line of the pair was this order); the chosen row's plate pulses (white at 2220, a tan at 2200, 2240, 2260: its curve is not read, four captured frames); the rest at parity: three monster requests, rank panel, page |
| field-enter (field HUD) | 3100 | `--field AREA_03_02 --load GAMEDATA00 --frames 120`, PARITY_SKIP=Data/x64/Effect/, `BARREL_ENTRY_DRAW=5` (the line is a random draw from the executable's field talk; `BARREL_ENTRY_LINE` still overrides it) | 0 | at parity with the draw fixed (the gauge in steps of 2 as on the area map) |
| area-menu (main menu, first page) | 2220 | `--field TOWN_04 --load GAMEDATA00 --area-map --menu`, frame 0 | 6 (0 with `BARREL_CLOCK_AT=2424`) | the chosen button's frame (part 0x1e), the Journal badge's strip (0x4a) and the cursor glow over the chosen button (74, 219; it was taken for the field effect's glow, it is the cursor's) turn with the clock; the name plate (layout 369) is drawn tilted 9 degrees clockwise about its node (the kind 7 node in 368, word 5 = -9) with its text (`Element::Tilted`), no scale |
| menu-cmd-0 (main menu, Basket page) | 2560 | `--field TOWN_04 --load GAMEDATA00 --area-map --menu basket`, frame 0 (366 before) | 2 (was 1: the lit frame's cut 1 matched at 2560 by chance while it stood still; 0 with `BARREL_CLOCK_AT=2764`) | the cursor glow (part 0x9f) and the lit cell frame (part 0x44, five steps of 10 ticks over cuts 0, 1, 2, 1, 0) turn with the clock. Items, qualities and order of the 34 held items equal the game's; the rest at parity: grid 568, card 752, Possessed box 706, empty places 26 (limit 60 less 34) |
| menu-cmd-1 (main menu, Adventure Equip. picker) | 2500-2560 | `--field TOWN_04 --load GAMEDATA00 --area-map --menu picker-adventure`, frame 0, `BARREL_CLOCK_AT=<frame + 204> BARREL_PAGE_AGE=<frame - 2400>` | 58 quads: 0 at all four with `BARREL_PAGE_AGE=f-2410` (0, 1, 1, 1 with `f-2400`) | everything equal; the chosen row's frame (part 0x2, five cuts of 10 ticks, flipbook of layout 362) runs on the picker's own clock from its opening (see Page clocks; this run opened 10 ticks late) |
| menu-cmd-2 (main menu, Battle Equip. picker) | 2500-2560 | `--menu picker-battle`, same | 130 quads: 0, 0, 0, 0 with `BARREL_PAGE_AGE=f-2400` | five members (Escha, Katla, Wilbell, Awin, Logy) from the save's squad; at parity |
| menu-cmd-4 (main menu, Status picker) | 2500-2560 | `--menu picker-status`, same | 130 quads: 0, 1, 0, 1 with `f-2400`; 0, 0, 1, 0 with `f-2410` | the same list as Battle Equip. under another tab; the frame's cuts 0, 1, 0, 0 fit no page age at all four frames (see Page clocks); the every-frame `picker-open` (the same picker, every frame 2390 to 2600) is 0 at 11 of 13 frames tried with `f-2400`, the two others the cursor glow (global clock) one cut off |
| menu-cmd-3 (main menu, Formation page) | 2500-2560 | `--menu formation`, same | 216 quads: 0 at 2560 with `BARREL_CLOCK_AT=2764` (was 10) | slots, plates, tabs, Status panel numbers (RES from the save's in-use block), cursor frame and glow at parity at all four frames; the bookmark's five pieces (layout 393, node kind 7 in 431, word 5 = 4) are tilted 4 degrees anticlockwise about the node, as `Placed::tilt` carries it |
| menu-status-page (Status page of the picker) | 2700-2860 every 20 | `--field TOWN_04 --load GAMEDATA00 --area-map --menu status-page`, frame 0, `BARREL_CLOCK_AT=<frame + 204>` | 80 quads: 0 at all nine frames (99 before: the older lead-only page) | desk cut 1, the figure 361, notebook, right page values (stats with equipment, RES from the save), the left page's slot icons, title and twelve empty lines, the "1/2" pager, footer, header and tab. The page is still (no flipbook, no glow). Escha's figure is drawn for every member |
| menu-equip-battle (Battle Equip. page of the picker) | 2700-2860 every 20 | `--menu equip-battle`, `BARREL_CLOCK_AT=<frame + 204> BARREL_PAGE_AGE=<frame - 2600>` | 329 quads: 0 at 2740, 2760, 2780, 2820, 2860; 1 at 2700, 2720, 2800, 2840 (`BARREL_PAGE_AGE=f-2600`) | left: the first slot's pulsing bar (`battleequip::pulse`, the page's own clock, see Page clocks: the triangle wave read off `equip-open`; the nine captured frames come from another run whose timer is a few ticks off) |
| menu-equip-adventure (Adventure Equip. page of the picker) | 2700-2860 every 20 | `--field TOWN_04 --load GAMEDATA00 --area-map --menu equip-adventure`, frame 0, `BARREL_CLOCK_AT=<frame + 204>` | 162 quads: 0 at 2700, 2720, 2780, 2820 and 1 at the other five (`BARREL_PAGE_AGE=f-2600`) | the one difference is the lit frame's pulse colour (`battleequip::pulse`, the Battle page's: the two pages' colours agree at 2780 and 2820 and are within 3 levels elsewhere, so one page clock); everything else equal: container grid 456 at (44, 252), Escha's three pieces of gear packed by equip cost, name tab 457, figure, card 752 with its "Equip. Cost" plate and seal, red effect lines |
| menu-cmd-5 (main menu, Assignment page) | 2560 | `--field TOWN_04 --load GAMEDATA00 --area-map --menu assignment`, frame 0 (173 before) | 2 (was 1, as the Basket row; 0 with `BARREL_CLOCK_AT=2764`) | the cursor glow, the cursor frame (part 0xd) and the "Report!" glows (part 0x9c) turn with the clock; cards, percents, stamps, details and rank box at parity. State 5's "new" mark (part 0xd cut 15) is read from one square |
| menu-cmd-7 (main menu, Journal page) | 2560 | `--field TOWN_04 --load GAMEDATA00 --area-map --menu journal`, frame 0, `BARREL_CLOCK_AT=2764` | 0 at 2520 and 2560 (the page was not built before: the list stood in) ; 1 at 2500 and 2540 | five library tabs from layouts 485 to 491, desk, panel, header, footer, title tab, cursor glow: 26 quads. What is left is the chosen tab's pulsing plate (part 0x1a cut 4): its colour runs white to tan, four captured frames (0.91, 0, 0.82, 1.0 of the way to tan at 2500 to 2560) fit a cosine to a power only at the white frame and at 2560 (`journal::pulse`) |
| menu-cmd-8 (main menu, Option command) | 2560 | `--field TOWN_04 --load GAMEDATA00 --area-map --menu option`, frame 0, `BARREL_CLOCK_AT=2764` | 0 (was 4: the name plate's tilt; 475 before: the Save/Load list stood in; 13 before the rules' cut-short pieces took their ink, `Element::InkedSlice`), the same at 2500, 2520, 2540 with their clocks | the command opens the Options screen (layout 511: volume sliders, Fast Forward Event, Voice Language, Default, Confirm) over the overview the game has drawn that frame; the page is at parity, the name plate tilted as in area-menu. The glow cut is the only thing that changes with time |
| battle-hits (damage per hit, rules) | 14 hits | `cargo test -p barrel-escha --test battle_hits` | 0 (14/14 at the rate the log's flags imply) | stats and skill power equal the game's; flank step unconfirmed |

## Battle HUD facts found by the battle-walk capture

- The squad fights: `m_squad`'s first three (the lead first = the centre plate, then left, then
  right), at their saved HP and MP (the plates' bars are the share in whole percent, rounded
  down; their numbers are the HP and MP now, right-aligned on the layout's digit nodes). The
  save's maxima with equipment stand for the kit's HP that Barrel does not compute yet (Escha:
  35, Barrel 30). The squad's others are the reserve row (layout 868), in the plates' order,
  face part 0x42 cut = the member number.
- The layout's draw order is not the game's: the capture draws the bar (frame k, then the slot
  hanging from it), the plates and reserve panels, the Support gauge and Fast Forward, the Action
  slot's overlay, the command row's places, the field information, then the chosen command's
  arrows and label. `battlehud::elements` sorts into that order.
- Offsets from the layout's own places: the turn bar +1, +1 (its slots +1, 0; the Action slot's
  corner +1, +1, its pieces scaled about it), the Support gauge, Fast Forward and the button
  under it 0, -1; layout texts sit `TEXT_RISE` higher as in the maps.
- Left out when a turn starts: the layout's template command row (887), layout 876's marker,
  the bar's two frame ends (862's cuts 0 and 1), the plates' action-percentage badge and state
  flipbooks, the next-turn mark while the cursor is on Attack.
- Row: 105 pixels between places (the layout's nine are that rounded), centred at x 961, icons at
  y 585; the third squad member (not an alchemist) has Attack, Skill, Move, Wait and Run only.

## Time alignment (`barrel-parity align`)

`PARITY_ALIGN=a-b/step tools/parity.sh <capture> <frame> <barrel-escha args> --frames N` diffs every
captured game frame of the range against each of Barrel's frames 0..N-1 and reports the best
Barrel frame per game frame and the one offset (game frame - Barrel frame) with the fewest
total mismatches (docs/parity.md). Full arguments of a run (the rows above abbreviate them):
`--field <game>/Data/x64/Field/TOWN_04 out.png --size 1920x1080 --load GAMEDATA00.pcsave ...`
(`--field auto out.png --size 1920x1080 --title <game>` for the title).

The 2D screens (the menu and its pages, Requests, the title) run a clock: `screen::run_clocked`
calls the screen's `tick` with a 60 Hz tick count (in a window the elapsed time, offscreen the
frame of `--frames N`, from `BARREL_CLOCK_AT`, default 0), and every frame whose picture changed
reaches the draw trace. A flipbook is a layout node of kind 36 (`layout::Node::flip`): word 5 =
the cuts, word 6 = **ticks each cut stays** (not cuts a second: the captures fit 8 ticks for the
cursor glow, 10 for the frames, badges and glows, and the battle HUD's "10 cuts a second" in
`battlehud::FLIPBOOK_RATE` is 10 ticks a cut), cuts in the sheet's order from the node's cut (the
frames' five cuts list the pulse there and back; the badge's fifth step holds its fourth picture).

What the captures say (area-menu, party status, the Basket page: 12 frames over three scenarios): all
the flipbooks of every screen run off one global frame counter, `G = game frame + 204` (the
captures fit 204 or 205, and further values every 448 or so frames up to 1600: 652, 653, 908,
909, 1100, 1101, 1356, 1357 mod 1600; the glow alone fixes it mod 64, the frames mod 50). So
`BARREL_CLOCK_AT=<game frame + 204>` puts Barrel's flipbooks in the game's phase (the plain
checks above with it), and an align over a Barrel run of 1700 frames from clock 0 (needs
`--frames 1700 --save-every 100`) finds an offset equal to -204 (or the other values) mod 1600.

| Row | Game frames | Barrel frames | Aligned result |
|---|---|---|---|
| boot-title | 1400-1480/20 | 0-239 (`--frames 240`, `BARREL_TITLE_FADE_AT` unset) | aligned: offset 1352, 2 mismatches over 5 game frames (plain: 1 at every frame). The title starts at game frame about 1352 (the hand fit said 1351); the pulse's rate and curve match, the two left (1400, 1420) are one alpha step (0.012, a frame's slope) off: the game's pulse starts about half a frame between 1351 and 1352. The per-frame best frames differ (the pulse repeats every 144 frames), the single offset decides |
| area-requests | 2200-2260/20 | 0-1699 (`--frames 1700 --save-every 100`) | aligned: offset 691, 3 mismatches over 4 game frames (plain, 1 frame: 3, 2, 3, 3 before, now 2, 1, 2, 2): the three are the chosen row's plate tan at 2200, 2240, 2260 (white at 2220 matches); every glow cut matches. 691 is -909 mod 1600, one of the fitted clock values. At `BARREL_CLOCK_AT=2404` (the clock at game frame 2200) and `--frames 61` the offset is 2200 |
| area-menu | 2200-2260/20 | 0-1699 (same) | aligned: offset 691, 16 mismatches over 4 game frames = 4 at each: the name plate's tilt (2 quads, out of the clocks' scope) at every frame; the cursor glow, the chosen button's frame and the Journal badge match at all four frames (plain: 5, 6, 6, 7) |
| menu-cmd-0 | 2500-2560/20 | 0-1699 (same) | aligned: offset 945, 0 mismatches over 4 game frames (plain, frame 0: 1, 2, 1, 2): glow and lit cell frame |
| menu-cmd-5 | 2500-2560/20 | 0-1699 (same) | aligned: offset 1393, 0 mismatches over 4 game frames (plain: 2 at each): glow, cursor frame and "Report!" glows |
| battle-walk | 3000 | 0-239 | the battle HUD now has a clock: the command row's flipbooks (diamond cuts 2 to 5, arrows 6 to 9 and 10 to 13) step at 10 cuts a second (node word 6; the area map's glow has word 6 = 8 and cycles at the 8 cuts a second the captures show), so a trace lists the row every 6 frames. `BARREL_BATTLE_COUNT=3`: best Barrel frame 234 (offset 2766 by the loose search), 7 mismatches at every frame: the leftovers above do not depend on the cut (the game's pieces are mid-animation, flipped and squashed, whatever cut). The phase is not resolved: one captured game frame holds the row (3200 and later are other states), so the offset (game frame - Barrel frame) fits any value; the game's cut there is 4 (uv x 1056) (Barrel's cut 4 shows on frames 12-17 + 24k of its clock) |

### Page clocks

A page's own animations count ticks since the page opened (`Menu::age`, the tick at the press that
opened it), not the global clock (`game frame + 204`) the glows and badges keep. Offscreen checks
give the age at the first frame with `BARREL_PAGE_AGE=<ticks since the page opened>`, next to
`BARREL_CLOCK_AT`: the game frame less the frame of the opening press. Pickers (A at 2400):
`f - 2400`; member pages (Status, Battle Equip., Adventure Equip., A at 2600): `f - 2600`.

```
BARREL_CLOCK_AT=$((f+204)) BARREL_PAGE_AGE=$((f-2400)) PARITY_INDEX=... tools/parity.sh .../menu-cmd-4/draws $f --field ... --load g00.pcsave --area-map --menu picker-status
BARREL_CLOCK_AT=$((f+204)) BARREL_PAGE_AGE=$((f-2600)) ...                .../menu-equip-battle/draws $f ... --menu equip-battle
```

**Picker row frame** (part 0x2, flipbook of layout 362: five cuts of 10 ticks, cut 0, 1, 2, 1, 0 of
the sheet): the every-frame capture `picker-open` (frames 2390 to 2600, A at 2400) shows the row
from 2410 at the sheet's v 0.070, 0.141, 0.070, 0, 0, 0.070 ..., ten frames each: exactly the
flipbook's cut at `age` = frame - 2400 (cut 1 from tick 10; its cut 0 is not seen, the page fades in
at 2410 to 2412). The "opening cut at 2405-2409" is the main menu's command plates fading out (the
page's own list), not the row. The three picker captures (frames 2500, 2520, 2540, 2560, other
runs, one per picker) do not all open at tick 0 of the frame counter: Battle Equip. (cmd-2) is at
`f - 2400`, Adventure Equip. (cmd-1) at `f - 2410` (the page's opening is 10 ticks late in that
run), the Status picker (cmd-4) at `f - 2410` gives 0, 1, 1, 0 for the captured 0, 1, 0, 0: one
frame (2540) off at either base, and no uniform stepping fits all four (the timer is time-based
and wanders between runs, like the equip pulse below). With the plain rule `f - 2400` the four frames give 0 mismatches for cmd-2, 3 for cmd-1 and 2 for cmd-4; with `f - 2410` cmd-1 is 0 at all four and cmd-4 has 1.

**Equip. pages' bar pulse** (`battleequip::pulse`, the Battle slot bar and the Adventure lit
frame: one function): the every-frame capture `equip-open` (frames 2590 to 2800, A at 2600, the
first slot's bar quad (164, 212, 548, 256)) shows a triangle wave: constant colour slope
(0.0256 of the range per frame), white held 4 to 5 frames and the tan (219, 166, 64) 1 to 2, half
period 30.4: white at 2659.5, 2721, 2780.5, tan at 2628.5, 2690, 2749.5 (period 60.7). A
clipped triangle (`white hold` 4 %, `tan hold` 2 % of the rise) fits the 192 frames to 0.024 RMS of
the range; a cosine does not (the old fit, period 62.125, was one). The nine frames of
menu-equip-battle / menu-equip-adventure (every 20, other runs) fit the same shape at a period
of 62.15 (`PULSE_PERIOD`, `PULSE_WHITE_AT` -2.9: fitted to those nine and 18 samples within 8
levels): the game's timer is time-based and wanders by about 2 % between runs, so checks sit 0 to
4 mismatches of nine whatever the constants (the check's tolerance is 1.5 levels of 255, the
bar's blue moves 6 levels a tick).

The offsets differ between the rows because they tie: the flipbooks repeat every 64, 50 and 320 frames,
so every offset equal to the best one modulo 1600 (or one of the other fitted values) gives the same
count and `align` reports one of them.

Not resolved: the Requests plate's colour pulse (four captured frames; its blue channel reads
0.51, 1.0, 0.53, 0.46 at 2200 to 2260, between white and a tan (0.89, 0.75, 0.45); it is not a
flipbook (node words 5 and 6 are 0), a denser capture, every 2 frames from 2200, would give its
curve); the Assignment squares' own plate pulse likely the same; the menu name plate's tilt.
The battle HUD's command row flipbooks (`battlehud.rs`) step every 10 ticks, not 10 times a
second (this section's finding; left to the battle's owner).
