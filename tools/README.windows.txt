Barrel @VERSION@ (Windows)

Barrel plays Atelier Escha & Logy DX and Atelier Ryza DX on its own engine, from your copy of
the game's files (the Steam release). One program, barrel.exe, works out which game to start.
Keep SDL3.dll beside it.

Needs: a Vulkan driver (any current graphics driver has one).

Run it
------
Double-click barrel.exe.

It finds the game in your Steam libraries (or the folder it saved before). If you own both
games it asks which one to play; if it finds none it shows a folder list to find the game's
folder (the one holding Atelier_Escha_and_Logy_EN.exe or Atelier_Ryza_DX.exe) and remembers it.

From a command prompt:
  barrel.exe --game "D:\Games\Atelier Ryza DX"     starts that game directly

From Steam
----------
Put barrel.exe and SDL3.dll in a permanent folder. In Steam, open the game's Properties >
General > Launch Options and enter:

  "C:\path\to\barrel.exe" %command%

Steam then starts Barrel in the game's place, keeping the library entry, playtime and controller
layout. Barrel finds out which game Steam meant from what Steam passes it.

Settings, saves, mods
---------------------
  %APPDATA%\barrel\games.toml             the game folders (shared)
  %APPDATA%\barrel\<game>\                graphics.conf, options.conf, mods.toml, mods\   (<game> = escha or ryza)
  %LOCALAPPDATA%\barrel\<game>\saves      save slots

Every setting is in the game's own Options screen.
