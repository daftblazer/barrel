Barrel @VERSION@ (Windows)

Barrel plays Atelier Escha & Logy DX and Atelier Ryza DX on its own engine, from your copy of
the game's files (the Steam release). One program, barrel.exe, works out which game to start.
Keep SDL3.dll beside it.

Needs: a Vulkan driver (any current graphics driver has one).

Run it
------
Unpack the zip anywhere (keep SDL3.dll beside barrel.exe). All the ways to start it:

  Double-click barrel.exe   (or from a command prompt in its folder: barrel.exe)
      Finds the game by itself: the folder it saved before, else your Steam libraries
      (C:\Program Files (x86)\Steam and every library it lists).
      - one game found:   it starts at the title screen
      - both games found: a "Choose a game" list (Up/Down, Enter or A)
      - none found:       a "Locate game files" folder list: open the folder holding
                          Atelier_Escha_and_Logy_EN.exe or Atelier_Ryza_DX.exe and press Enter
                          on it; Barrel remembers it (in games.toml, below)

  barrel.exe --game "D:\Games\Atelier Escha and Logy DX"
      Starts the game in that folder directly, skipping the search and the list. Use this for
      a copy outside Steam or to pick one game when you own both. If the folder does not hold
      the game, Barrel says so and stops. (A shortcut works too: add --game "..." after the
      program in the shortcut's Target.)

  set BARREL_GAME_DIR=D:\Games\Atelier Ryza DX
  barrel.exe
      The same as --game, from the environment.

  barrel.exe --preset handheld
      Starts with the handheld graphics preset for this run only (also: desktop,
      desktop-high). Settings you keep belong in the game's Options screen.

To make Barrel search again (another folder, a moved game), delete the game's line from
%APPDATA%\barrel\games.toml.

Display (window or fullscreen) is in Options > Graphics.

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
