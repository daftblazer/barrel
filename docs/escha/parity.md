# Escha parity scoreboard

Each row is a scenario. Its game capture comes from `../escha-recomp/scenarios/<name>.sh`
(`tools/scenario.sh`), and Barrel's trace from the matching Barrel run, diffed with
`barrel-parity diff` (docs/parity.md). Mismatches count the diff's lines, game frame against
Barrel frame.

| Scenario | Game frame | Barrel | Mismatches | Notes |
|---|---|---|---|---|
| boot-title | 1400 | `--title`, frame 0 | 0 (+1 phase) | the one difference is the "press" pulse's phase (Barrel at 0 s, the game 15 s in); its curve already matches (2.4 s, low 0.2). Animated items need time alignment |
| load-field (area map) | 3000 | `--load barrel-user.save --area-map`, frame 0 | 1 | the field gauge reads 45 (the save) where the game draws 44; rest at parity |
| battle-walk (first turn's command row) | 3000 | `--field AREA_03_02 --load GAMEDATA00 --battle 25 --frames 120`, frame 119 | 8 (196 at frame 0) | the game's frame is its first turn with the row open; Barrel opens it at 1 s (frame 63), so frame 119 is the same moment (`tools/parity.sh` diffs the last frame of `--frames N`). Left: the extra fourth monster in the bar (Barrel draws A, A, B, B where the game had three, its draw is random) with the order that follows (3 differences, the bar's faces), the chosen command's flipbook pieces caught mid-animation in the game (2 game-only, 1 Barrel-only, no clock) |
| area-requests | 2220 | `--field TOWN_04 --load GAMEDATA00 --area-map --requests`, frame 0 | 2 (1 phase) | the cursor glow (part 0x9f) shows its cut by time (the game cycles 8 cuts a second: 4, 7, 1, 4 at frames 2200 to 2260; Barrel draws cut 0) and the chosen row's plate pulses (white at 2220); the rest at parity: three monster requests, rank panel, page |

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
