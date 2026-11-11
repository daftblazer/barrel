# Barrel

A native engine for Gust's Atelier games, written in Rust. The player's Steam copy supplies
the data, and nothing from it is shipped here. There is one engine for all the games: the
format readers, renderer, audio, the event interpreter and the Squirrel VM are shared. Each
game adds its own rules on top: synthesis, battle, calendar and menus.

| Game | Steam release | Base |
|---|---|---|
| Atelier Ayesha DX | 2021 | Older PSSG data with KTGL, Squirrel 2.2.5 |
| Atelier Escha & Logy DX | 2021 | KTGL + elixir, Squirrel 2.2.5 |
| Atelier Shallie DX | 2021 | KTGL + elixir, Squirrel 2.2.5 |
| Atelier Sophie 2 | 2022 | KTGL + elixir, PAK archives, Squirrel 2.2.5 |
| Atelier Ryza DX | 2023 | KTGL + elixir, PAK archives, no Squirrel |
| Atelier Ryza 2 DX | 2023 | KTGL + elixir, PAK archives, no Squirrel |
| Atelier Ryza 3 DX | 2024 | KTGL + elixir, PAK archives, no Squirrel |

The goals:
- run natively on Linux, including aarch64 handhelds (Vulkan);
- mod support built in (a load order of loose files that override any table, event,
  script or asset);
- graphics beyond the DX11 originals once the original look is reproduced;
- uncapped frame rates, any aspect ratio, and UI scaling for handhelds.

Status: started, with Escha & Logy DX first. `crates/formats` reads the elixir containers,
the event bytecode, the XML tables, the textures and the models' geometry; every one of Escha's files of both kinds reads without error.
`crates/cli` is the `barrel` tool for looking at the data. See `docs/survey.md` for the
games and `docs/formats.md` for the formats.

```sh
cargo build --release
cargo test --release
target/release/barrel game "<steam>/steamapps/common/Atelier Escha and Logy DX"
target/release/barrel scan  "<install>/Data"           # every container and what it holds
target/release/barrel ls     FILE.elixir.gz
target/release/barrel unpack FILE.elixir.gz OUTDIR
target/release/barrel ebd-scan "<install>/Event"       # every event and its commands
target/release/barrel ebd-decode "<install>/Event"     # split every event into commands
target/release/barrel ebd    FILE.ebd
target/release/barrel xml-scan "<install>"            # every XML table and its rows
target/release/barrel xml    FILE.xml
target/release/barrel g1t-scan "<install>/Data"       # every texture
target/release/barrel g1t    FILE.g1t OUTDIR           # a texture pack as PNG files
target/release/barrel g1m-scan "<install>/Data"       # every model
target/release/barrel g1m-obj FILE.elixir.gz#N OUT.obj  # a model as OBJ
```

Extracted data goes in `work/`, which git ignores. No game data belongs in the repository.

The name comes from the series' running "Barrel!" joke.
