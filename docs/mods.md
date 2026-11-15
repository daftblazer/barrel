# Running Barrel and modding it

## Running

Barrel is a standalone program that reads the original game's files; it never needs a launcher.

- **Finding the game.** In order: `--game DIR`; the install the command line's own paths are
  in (Kintsugi's `--title FOLDER`, or `--field`/`--event` paths inside an install); the
  `BARREL_GAME_DIR` variable; the folder saved in the config (`games.toml`); Steam's libraries;
  else the "Locate game files" screen. The Steam search reads every `libraryfolders.vdf` under
  `~/.local/share/Steam`, `~/.steam/steam`, `~/.steam/root` and the Flatpak Steam folders (on
  Windows `C:\Program Files (x86)\Steam` and `%ProgramFiles%\Steam`, no registry), and in each
  library the game's `appmanifest_<app id>.acf` (Escha 1152310, Ryza 3365030; its `installdir`)
  or the usual folder name (`Atelier Escha and Logy DX`, `Atelier Ryza DX`). A folder counts
  when it holds the game's program (`Atelier_Escha_and_Logy_EN.exe`, `Atelier_Ryza_DX.exe`) and
  a `Data` folder. A `--game` or `BARREL_GAME_DIR` that does not hold the game is an error, not
  a fall-through. With no scene on the command line Barrel starts the game itself (Escha at its
  title screen, Ryza on Rasenboden), as Kintsugi's `run` asks for. Steam's `%command%` on
  Windows hands over the game's own `.exe`: its folder is the install.
- **Locate game files.** A window in a clean flat style of Barrel's own (the game's art is not
  loadable yet): a folder list, Up/Down (D-pad) to move, Enter (A) to open a folder or, on one
  that holds the game, to choose it, Backspace (B) up a folder, Page Up/Down, Esc to quit. The
  answer is saved in `games.toml`.
- **Linux (Steam):** Kintsugi, a Steam compatibility tool, starts Barrel in the game's place.
- **Windows (Steam):** the game's launch options `"C:\path\barrel-escha.exe" %command%` start
  Barrel instead of the game, keeping the Steam library entry, playtime and controller layout.
  Starting the .exe directly, or as a non-Steam game, works too.
- **Handhelds without Steam (Rocknix and the like):** the binary plus a start script, as the
  Kintsugi engine folder already is.

Every setting lives in the game's Options screen (the game's own page, then Barrel's graphics
page; a mods page later).

## Config files

All in the config folder (`$XDG_CONFIG_HOME/barrel`, `~/.config/barrel`; on Windows
`%APPDATA%\barrel`; with `BARREL_CONFIG` set, the folder of that file):

- `graphics.conf`, `options.conf`: the settings (the Options screen writes them).
- `games.toml`: the game folders, `escha = "/path/to/Atelier Escha and Logy DX"`, `ryza = "..."`.
- `mods.toml`: the enabled mods in load order, `enabled = ["first", "second"]` (folder names
  under `mods/`; the last wins a path). No mods are enabled by default, and with none the
  reads are exactly the game's own.
- `mods/<folder>/mod.toml` and the mod's files, below.

## Mods (plan)

Item 1 is built; the layout it reads:

```text
mods/better-items/mod.toml                  name = "Better Items"
                                            version = "1.0"
                                            needs = ["base-pack"]   # optional
mods/better-items/Saves_EN/item/itemData.xml   replaces the game's file of that path
mods/better-items/Data/ZZ_extra.PAK            an extra archive (a same-named one replaces)
```

Files sit at the game's own paths under the mod's folder (any letter case, either slash). A mod
whose folder is missing, has no `mod.toml` or needs a mod not enabled before it is skipped with
a line on stderr. Two mods that give the same path: the later in `mods.toml` wins.


1. **Layered files (done).** Every game file Barrel reads goes through one layered file system: the
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

Order: the layered file system and the game-files search are done (no visible change); the rest
after the games reach parity.
