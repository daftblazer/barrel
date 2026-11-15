Barrel @VERSION@ (Linux)

Barrel plays Atelier Escha & Logy DX and Atelier Ryza DX on its own engine, from your copy of
the game's files (the Steam release). One program, `barrel`, works out which game to start.

Needs: a Vulkan driver (Mesa or the vendor's), Wayland or X11, and SDL3 (the system's on
x86_64: package "sdl3"; the aarch64 package carries its own in lib/).

Run it
------
  ./barrel

It finds the game in your Steam libraries (or the folder it saved before). If you own both
games it asks which one to play; if it finds none it shows a folder list to find the game's
folder (the one holding Atelier_Escha_and_Logy_EN.exe or Atelier_Ryza_DX.exe) and remembers it.

  ./barrel --game "/path/to/Atelier Ryza DX"     starts that game directly
  BARREL_GAME_DIR=/path/to/the/game ./barrel     the same, from the environment

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
