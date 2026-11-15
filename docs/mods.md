# Running Barrel and modding it

## Running

Barrel is one program, `barrel` (`barrel.exe` on Windows), that reads the original games' files
and works out which game to start; it never needs a launcher. One release package per platform
(`tools/package.sh release`: `barrel-<version>-linux-x86_64.tar.gz`, `-linux-aarch64.tar.gz`,
`-windows-x86_64.zip`), each with a README.txt on how to run it. The games' own programs
(`barrel-escha`, `barrel-ryza`) stay as developer tools with every offscreen/debug flag
(`tools/parity.sh` uses `barrel-escha`); `barrel-tool` is the data inspector (it was `barrel`).

- **Which game** (`barrel_files::detect`). In order: `--game DIR` (the folder's contents say
  which game: its program file and a `Data` folder); a path among the arguments inside an
  install (Steam's `%command%`: on Windows the game's `.exe`, on Linux with Proton
  `.../proton waitforexitandrun .../Atelier_*.exe`; Kintsugi's `--title FOLDER`; a developer's
  `--field DIR`); the `SteamAppId` (else `SteamGameId`) variable Steam sets (Escha 1152310, Ryza
  3365030) with that game's install from the saved folder or the Steam search; `BARREL_GAME_DIR`;
  then the games the config saved (`games.toml`) and Steam's libraries hold: exactly one starts,
  several show the picker, none the Locate screen. When the arguments name the install through
  the game's program, Steam's own arguments are dropped; any other arguments that are no
  `--flag` are ignored, never errors. A `--game` or `BARREL_GAME_DIR` that holds no game is an error.
- **The Steam search** reads every `libraryfolders.vdf` under `~/.local/share/Steam`,
  `~/.steam/steam`, `~/.steam/root` and the Flatpak Steam folders (on Windows
  `C:\Program Files (x86)\Steam` and `%ProgramFiles%\Steam`, no registry), and in each library the
  game's `appmanifest_<app id>.acf` (its `installdir`) or the usual folder name (`Atelier Escha
  and Logy DX`, `Atelier Ryza DX`). A folder counts when it holds the game's program
  (`Atelier_Escha_and_Logy_EN.exe`, `Atelier_Ryza_DX.exe`) and a `Data` folder. With no scene
  on the command line the game starts itself (Escha at its title screen, Ryza on Rasenboden).
- **Picker.** When several games are found and nothing above chose: a screen in the Locate
  screen's flat style listing them by name and folder, Up/Down (D-pad), Enter (A) to start,
  Esc to quit. It remembers nothing.
- **Locate game files.** When no game is found: a window in a clean flat style of Barrel's own
  (the game's art is not loadable yet): a folder list, Up/Down (D-pad) to move, Enter (A) to
  open a folder or, on one that holds either game, to choose it, Backspace (B) up a folder,
  Page Up/Down, Esc to quit. The answer is saved in `games.toml` under that game.
- **Linux (Steam):** either Kintsugi (a compatibility tool that starts native engines; its
  engine folder's `run` calls `barrel --game FOLDER`), or the launch options
  `"/path/to/barrel" %command%` with any Proton selected as the compatibility tool (Steam only
  allows a launch command for a Windows build under Proton). Linux works like Windows: it can
  also be run directly.
- **Windows (Steam):** the launch options `"C:\path\barrel.exe" %command%` start Barrel instead
  of the game, keeping the Steam library entry, playtime and controller layout. Starting
  `barrel.exe` directly works too.
- **Handhelds without Steam (Rocknix and the like):** the aarch64 package (`barrel` with its SDL3
  in `lib/`, found through the binary's rpath) or the Kintsugi engine folder's `run` script.
- **Windows build:** `tools/package.sh release` builds the zip when the Rust target
  `x86_64-pc-windows-gnu`, `x86_64-w64-mingw32-gcc` and a Windows SDL3 prefix
  (`BARREL_WINDOWS_SDL3`) are present; otherwise it says why it skipped it. It has not been built
  on the development machine (no Windows target installed offline).

Every setting lives in the game's Options screen (the game's own page, then Barrel's graphics
page; a mods page later).

## Config files and data folders

Per game, so two games never share or overwrite each other's settings, saves or mods. In the
config folder (`$XDG_CONFIG_HOME/barrel`, `~/.config/barrel`; on Windows `%APPDATA%\barrel`):

- `games.toml` (shared): the game folders, `escha = "/path/to/Atelier Escha and Logy DX"`, `ryza = "..."`.
- `escha/` and `ryza/`, each with:
  - `graphics.conf`, `options.conf`: the settings (the Options screens write them);
  - `mods.toml`: the enabled mods in load order, `enabled = ["first", "second"]` (folder names
    under `mods/`; the last wins a path). No mods are enabled by default, and with none the
    reads are exactly the game's own;
  - `mods/<folder>/mod.toml` and the mod's files, below.

The data folder (`$XDG_DATA_HOME/barrel`, `~/.local/share/barrel`; on Windows
`%LOCALAPPDATA%\barrel`) holds `escha/saves/slotNN.save` (Ryza has none yet).

With `BARREL_CONFIG=FILE` (offscreen test runs) the graphics config is that file and the rest of
the config sits flat in its folder: no per-game subfolders, no migration.

**Older installs keep working.** Before this layout the settings sat directly in
`~/.config/barrel` (`graphics.conf`, `options.conf`, `mods.toml`, `mods/`) and the saves in
`~/.local/share/barrel/saves`. At its start a game copies the old `graphics.conf` into its own
folder when it has none (Escha also `options.conf` and `mods.toml`), never deleting the old file;
Escha keeps using the old `mods/` and `saves/` folders until its own `escha/mods` / `escha/saves`
exist. Nothing is moved, so nobody loses anything; new saves of a fresh install go to the new place.

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
