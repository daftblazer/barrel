//! Where a game's files come from. Every original file Barrel reads goes through one layered
//! file system ([`GameFs`]): the game's install at the bottom, each enabled mod above it in load
//! order, the topmost root holding a path winning. The rest of this crate is what makes the
//! layers: the config folder and its small settings files ([`toml`], [`config`]), the mods
//! ([`mods`]), the search for the install ([`find`], with Steam's library files in [`vdf`]) and
//! which game to start ([`detect`]) and the folder list the "Locate game files" screen shows ([`browse`]).
//!
//! The paths a game's code gives are the install's own (`<install>/Data/x64/...`): [`read`] and
//! the other free functions take them, map the ones under the install to the game-relative path
//! and look that up through the layers; a path outside the install is an ordinary file's.

pub mod browse;
pub mod config;
pub mod detect;
pub mod find;
pub mod fs;
pub mod mods;
pub mod toml;
pub mod vdf;

pub use fs::{exists, install, is_dir, is_file, list, open, read, read_to_string, GameFs};
