# Escha parity scoreboard

Each row is a scenario. Its game capture comes from `../escha-recomp/scenarios/<name>.sh`
(`tools/scenario.sh`), and Barrel's trace from the matching Barrel run, diffed with
`barrel-parity diff` (docs/parity.md). Mismatches count the diff's lines, game frame against
Barrel frame.

| Scenario | Game frame | Barrel | Mismatches | Notes |
|---|---|---|---|---|
| boot-title | 1400 | `--title`, frame 0 | 0 (+1 phase) | the one difference is the "press" pulse's phase (Barrel at 0 s, the game 15 s in); its curve already matches (2.4 s, low 0.2). Animated items need time alignment |
| load-field (area map) | 3000 | `--load barrel-user.save --area-map`, frame 0 | 0 | at parity (the gauge fills in steps of 2: 45 shows 44) |
| battle-walk (first turn's command row) | 3000 | `--field AREA_03_02 --load GAMEDATA00 --battle 25 --frames 120`, frame 119 | 8 (196 at frame 0) | the game's frame is its first turn with the row open; Barrel opens it at 1 s (frame 63), so frame 119 is the same moment (`tools/parity.sh` diffs the last frame of `--frames N`). Left: the extra fourth monster in the bar (Barrel draws A, A, B, B where the game had three, its draw is random) with the order that follows (3 differences, the bar's faces), the chosen command's flipbook pieces caught mid-animation in the game (2 game-only, 1 Barrel-only, no clock) |
| area-requests | 2220 | `--field TOWN_04 --load GAMEDATA00 --area-map --requests`, frame 0 | 2 (1 phase) | the cursor glow (part 0x9f) shows its cut by time (the game cycles 8 cuts a second: 4, 7, 1, 4 at frames 2200 to 2260; Barrel draws cut 0) and the chosen row's plate pulses (white at 2220); the rest at parity: three monster requests, rank panel, page |
| field-enter (field HUD) | 3100 | `--field AREA_03_02 --load GAMEDATA00 --frames 120`, PARITY_SKIP=Data/x64/Effect/, `BARREL_ENTRY_DRAW=5` (the line is a random draw from the executable's field talk; `BARREL_ENTRY_LINE` still overrides it) | 0 | at parity with the draw fixed (the gauge in steps of 2 as on the area map) |
| area-menu (main menu, first page) | 2220 | `--field TOWN_04 --load GAMEDATA00 --area-map --menu`, frame 0 | 6 (3 at the other frames) | what moves: the chosen button's glow cut and the Journal badge's strip (by time), the field effect's glow at (74, 219) (a flipbook by time; not drawn), and the name plate (layout 369, drawn tilted about 13 degrees and scaled to 88%, 2 quads: Barrel draws it and its text upright) |
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

Result of the first run of each time-varying row: **none of them can be aligned yet, because
Barrel has no clock on these screens**, not because the offset is unknown.

| Row | Game frames | Barrel frames | Aligned result |
|---|---|---|---|
| boot-title | 1400-1480/20 | 0-239 | Barrel's title renders one frame offscreen and takes its time from the wall clock (`BARREL_TITLE_FADE_AT` seconds, `title.rs`), so its trace has a single frame and no offset can be searched (1 mismatch at every game frame, the pulse). Aligned by hand: the game's prompt alpha (0.455 at 1400, 0.231 at 1420, 0.376 at 1440, 0.6 at 1460, 0.82 at 1480) is Barrel's triangle wave (2.4 s, low 0.2) with the title starting at game frame about 1351. `BARREL_TITLE_FADE_AT=(frame-1351)/60` gives 0 mismatches at 1400 and 1 at 1460 (alpha 0.612 vs 0.600, about one frame of drift: the game's 60 frames are slightly longer than Barrel's) |
| area-requests | 2200-2260/20 | 0 (only) | 3, 2, 3, 3 mismatches per game frame against Barrel's single frame; the area map renders one frame offscreen and `--frames` does not loop it, so there is nothing to align. The game's glow cut changes at 8 cuts a second (4, 7, 1, 4 at 2200 to 2260); Barrel draws cut 0 only |
| area-menu | 2200-2260/20 | 0 (only) | 5, 6, 6, 7 per game frame, same cause: one static frame; Barrel has no cycling for the chosen button's glow or the Journal badge strip |
| battle-walk | 3000 | 0-239 | best Barrel frame 239 (the last), 8 mismatches, the scoreboard's count (20 at frame 0): the HUD's quad list is set at frame 0 and 63 and never changes after, so Barrel's chosen-command flipbook (a15_battle cuts #3 and #5) shows one cut where the game has caught another; later game frames (3200, 3400) are other battle states, not comparable |

So `align` is ready; what the animation rows need is Barrel-side time: a per-frame clock on the
area map, menu and battle HUD (offscreen `--frames N` looping the screen, and the cycling parts
following it at the game's rate: 8 cuts a second for the glow), after which the offset is
searchable. Until then the "phase" differences stay counted in the rows above.
