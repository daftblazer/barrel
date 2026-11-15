Barrel @VERSION@ (Linux)

Barrel plays Atelier Escha & Logy DX and Atelier Ryza DX on its own engine, from your copy of
the game's files (the Steam release). One program, `barrel`, works out which game to start.

Needs: a Vulkan driver (Mesa or the vendor's), Wayland or X11, and SDL3 (the system's on
x86_64: package "sdl3"; the aarch64 package carries its own in lib/).

Run it
------
Unpack it anywhere and start `barrel` from that folder. All the ways to start it:

  ./barrel
      Finds the game by itself: the folder it saved before, else your Steam libraries
      (~/.local/share/Steam, ~/.steam, Flatpak Steam and every library they list).
      - one game found:   it starts at the title screen
      - both games found: a "Choose a game" list (Up/Down, Enter or A)
      - none found:       a "Locate game files" folder list: open the folder holding
                          Atelier_Escha_and_Logy_EN.exe or Atelier_Ryza_DX.exe and press Enter
                          on it; Barrel remembers it (in games.toml, below)

  ./barrel --game "/path/to/Atelier Escha and Logy DX"
      Starts the game in that folder directly, skipping the search and the list. Use this for
      a copy outside Steam, an SD card, or to pick one game when you own both. If the folder
      does not hold the game, Barrel says so and stops.

  BARREL_GAME_DIR="/path/to/Atelier Ryza DX" ./barrel
      The same as --game, from the environment (handy in a start script).

  ./barrel --preset handheld
      Starts with the handheld graphics preset for this run only (also: desktop,
      desktop-high). Settings you keep belong in the game's Options screen.

  ./barrel --game "/path/to/the/game" --preset handheld
      Options can be combined.

To make Barrel search again (another folder, a moved game), delete the game's line from
~/.config/barrel/games.toml.

The window follows your desktop; Display (window or fullscreen) is in Options > Graphics.

From Steam
----------
Put the program somewhere permanent. In Steam, open the game's Properties > Compatibility and
tick "Force the use of a specific Steam Play compatibility tool" (any Proton: Steam only lets
a game start under Proton for the Windows build), then set Properties > General > Launch
Options to:

  "/path/to/barrel" %command%

Steam then starts Barrel in the game's place (Steam still counts playtime and shows the
overlay); Barrel finds out which game Steam meant from what Steam passes it.

With Kintsugi
-------------
Kintsugi (a Steam compatibility tool that starts native engines) can start Barrel instead of
Proton: install the engine folder from the project's tools/package.sh for the game, then select
Kintsugi as the game's compatibility tool. No launch options are needed.

Settings, saves, mods
---------------------
  ~/.config/barrel/games.toml             the game folders (shared)
  ~/.config/barrel/<game>/                graphics.conf, options.conf, mods.toml, mods/   (<game> = escha or ryza)
  ~/.local/share/barrel/<game>/saves      save slots

Every setting is in the game's own Options screen.
