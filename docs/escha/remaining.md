# Escha & Logy DX: what is left

Work on Escha & Logy DX paused on 2026-11-13 to start on a second game. This is where it
stands and what is left, grouped by area, roughly in the order a full playthrough would need
it. The detail for each open problem (what is known, what was tried, how to check a fix) is in
`docs/escha/open.md`; the topic docs hold what was worked out.

## Where it stands

Barrel plays Escha & Logy DX from its own title screen as a Kintsugi engine (`README.md`),
on the desktop and on an aarch64 handheld (Retroid Pocket 6, Turnip):

- The title screen, New Game with the lead chosen, Load Game over Barrel's own save slots,
  Options (the graphics page), Exit.
- The opening's events played in order, with voices, music, lines advanced by A or auto.
- Free play: walking with the play camera, map changes through doors, the town, the world map
  with trips that take days, area maps with their sectors, gathering, field enemies, battles
  with results, drops and game over, the cauldron (basic synthesis), shops, the main menu and
  its Equip, Status and Basket pages, the field HUD with the date and time dial, saving.
- A save from the original game imported into Barrel's format (`barrel import-save`).
- On the RP6 at native 1080p, 120 fps in the field (frames in flight, the distant view drawn
  after the field, view culling: `docs/renderer.md`).

## The story past the opening

The biggest gap: the opening plays, but the game's story as a whole has not been played
through in Barrel.

- Events through the whole story: their start conditions are read (`game::event_start`), but
  only the opening's chain has been checked in play. Every later event, cutscene and character
  event needs a pass against the running game.
- Story NPCs in events (characters 10048 and up, `docs/escha/open.md` "Field NPC spawns") and town
  people placed in free play (`clsFMDataNpc`, its 1,494 records unread).
- Assignments: their counters per condition (`docs/escha/open.md` "Assignment counters"), the
  assignment screen and its rewards at the end of each period.
- The request board: accepting and handing in requests (the session does not hold accepted
  requests yet), the board's points.
- Logy as the lead: the new game and the opening work, the rest of his route is untested.
- The world level's story opening of places and routes (place 5 behind route 3 on the user's
  save is unknown), the `[1, N]` place conditions read live.
- Endings (`u145` in a jump table, skipped), `u79` in `MM08_AS4_*`.
- `TOWN_03`'s event scenes render from inside the ground (parked).
- Movies: not played; native Rust decoders chosen (2026-11-14).

## Battle

- Rules: traits, states, support attacks, items in battle, guard and flee
  (`crates/game/src/duel.rs`'s header lists what the partial battle leaves out); item power,
  item level, the reaction queue's timing (`docs/escha/open.md`, "Battle").
- The game's battle screen: its command layout, target marker, turn order bar, damage numbers,
  effects and sounds, the battle camera and light (the stage centre is a guess), running to
  targets and per-skill motions.
- Formation, the battle item grid in play.

## Synthesis

- The game's own synthesis screens (layouts 720 to 851): choosing materials, the material
  loop, synthesis skills and CP, picking traits, the count.
- Recipe books and recipe ideas, the alchemy level and experience, refining, equipment
  synthesis, property points (`docs/escha/open.md`, "Property point spending").
- Made items keeping their effects and traits in the save; prices (`docs/escha/open.md`).

## Menus and screens

- Container, Journal, Formation, Request pages; the item card (layout 752), the Amount dialog
  (660), the drops screen's own grid and slot pictures.
- The Options screen's sound page (layout 408's volume rows), its title and help bar in the
  game's pieces; Change BGM.
- The title's Dressing Room and Extras.
- The game's own key configuration (Barrel's button map is its own).

## The maps (stage 15, in progress when work paused)

- The world map's compass cursor while moving, no player shadow on the maps, the scroll and
  label plates in the game's art (layout 641's sheet), the question box's bars.
- The gathering level icon's number (Barrel shows Lv1 where the game shows Lv3).
- Landing on the area map after a wipe (Barrel uses the touch point).

## Field details

- The play camera: `bumper_follow_speed` / `bumper_max_distance`, blending between camera
  areas (Barrel snaps), the zoom's way back.
- Gathering: picks per gathering, the reach of a point (`docs/escha/open.md`).
- Followers on escort requests (unconfirmed).
- The field HUD's remaining pieces: the dial's hand, the assignment badges, "Free Time", the
  gathering gauge and notices.

## Look

- Particle effects (`.g1e`, `eff_pack_list.xml`) and the character-bound effects that use them;
  the first fields with placed effects are `AREA_06_*` and `AREA_08_*`.
- Terrain: its specular term, the coarse patches at a distance, `battle`'s terrain, the open
  questions in `docs/escha/open.md` ("Terrain details").
- The draw priority of pieces (`xS1`-`xS3`, a live read of the draw queue).
- The cloud shadows' phase, the fog values' conversion, `cDiff`'s alpha.
- Plant sway and tree spring units against the game.

## Sound

- The sound effect id layer (event sound ids through the bank's group/cue entry).
- The effects' level against the game, the ambience volume's units.

## Saves

- Writing the game's own `GAMEDATA` saves (a record writer and a new game's full state as a
  template). Barrel saves its own text format.
- `barrel import-save` leaves out the container, the battle item grids, requests, shops and
  most of the library; it starts the save in a field where the game opens the area map.

## Performance and the handheld

- The opening's town (`TOWN_04` at the event's 20 s) is 8.5 ms of GPU time at native 1080p on
  the RP6 and did not gain from drawing the sky last: its cost is still to be found.
- The `handheld` preset still upscales with FSR 1 from 75%; GSR 1 does the same upscale for
  0.29 ms against FSR 1's 1.17 on the RP6 and should replace it.
- The GPU hang seen once on the RP6 and the per-draw name checks are engine problems
  (`docs/open.md`).
