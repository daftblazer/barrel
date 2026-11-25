//! Atelier Ryza DX on Barrel: its install (the `Data/*.PAK` archives), its fields, and later its
//! rules and play. Ryza 2 DX and Ryza 3 DX get crates of their own; what the games turn out to
//! share moves into `barrel_formats` and `barrel_engine` (`docs/ryza/`).

pub mod camera;
pub mod chara;
pub mod environment;
pub mod face;
pub mod flare;
pub mod gimmick;
pub mod mapmarker;
pub mod grass;
pub mod graphics;
pub mod presets;
pub mod river;
pub mod horizon;
pub mod install;
pub mod koc;
pub mod occluders;
pub mod light;
pub mod maps;
pub mod motion;
pub mod npc;
pub mod player;
pub mod walk;
pub mod solids;
pub mod save;
pub mod item_info;
pub mod library;
pub mod party_stats;
pub mod party_skills;
pub mod saves;
pub mod load;
pub mod load_view;
pub mod leave;
pub mod menu_input;
pub mod options;
pub mod options_run;
pub mod preferences;
pub mod options_view;
pub mod field_hud;
pub mod field_menu;
pub mod field_page;
pub mod story;
pub mod event;
pub mod talk;
pub mod telop;
pub mod font;
pub mod title;
pub mod title_menu;
pub mod new_game;
pub mod opening;
pub mod boot;
mod spinner;
pub mod music;
pub mod sound;

// The game's play (its program: `barrel-ryza`, and `barrel`, which picks the game).
// The play code names its own crate by its package name.
extern crate self as barrel_ryza;
pub mod play;
