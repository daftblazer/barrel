# Running Barrel and modding it

## Running

Barrel is a standalone program that reads the original game's files; it never needs a launcher.

- **Finding the game.** Barrel looks for the game in the usual Steam library folders (each
  `libraryfolders.vdf` it finds), then in the folder its config names. With neither, it opens a
  one-time "Locate game files" screen drawn in the game's own style and remembers the answer in
  its config. The command-line `--game DIR` (or `BARREL_GAME_DIR`) overrides all of it.
- **Linux (Steam):** Kintsugi, a Steam compatibility tool, starts Barrel in the game's place.
- **Windows (Steam):** the game's launch options `"C:\path\barrel-escha.exe" %command%` start
  Barrel instead of the game, keeping the Steam library entry, playtime and controller layout.
  Starting the .exe directly, or as a non-Steam game, works too.
- **Handhelds without Steam (Rocknix and the like):** the binary plus a start script, as the
  Kintsugi engine folder already is.

Every setting lives in the game's Options screen (the game's own page, then Barrel's graphics
page; a mods page later).

## Mods (plan)

1. **Layered files.** Every game file Barrel reads goes through one layered file system: the
   game's own files at the bottom, each enabled mod above them in load order; the topmost copy of
   a path wins. A mod is a folder with a manifest (`mod.toml`: name, version, what it needs) and
   files at the game's own paths.
2. **Standard formats.** A mod may give a file in a common format in place of the game's: PNG for
   a texture, glTF for a model, XML or JSON for a table, plain text for lines. Barrel converts it
   on load, so mods are made with ordinary tools (an image editor, Blender, a text editor).
3. **`barrel-mod`**, a command-line tool: `extract` (the game's files to the standard formats, as
   a starting point), `new` (a mod folder from a template), `check` (errors, missing files,
   conflicts between mods), `pack` (a zip to share). A graphical editor only if people want one.
4. **Mods page.** The Options screen gains a page listing the installed mods: on/off and load
   order, applied on restart.
5. **Logic mods (after the games are complete).** A scripting layer (likely Lua) over Barrel's
   own systems, never the original program's memory layout.

Order: the layered file system and the game-files search come now (no visible change); the rest
after the games reach parity.
