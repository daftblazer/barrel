//! Atelier Ryza DX on Barrel: its install (the `Data/*.PAK` archives), its fields, and later its
//! rules and play. Ryza 2 DX and Ryza 3 DX get crates of their own; what the games turn out to
//! share moves into `barrel_formats` and `barrel_engine` (`docs/ryza/`).

pub mod camera;
pub mod chara;
pub mod environment;
pub mod face;
pub mod grass;
pub mod presets;
pub mod install;
pub mod koc;
pub mod light;
pub mod maps;
pub mod motion;
pub mod player;
pub mod solids;
pub mod event;
pub mod telop;
pub mod font;
pub mod title;
pub mod boot;

// The game's play (its program: `barrel-ryza`, and `barrel`, which picks the game).
// The play code names its own crate by its package name.
extern crate self as barrel_ryza;
pub mod play;
