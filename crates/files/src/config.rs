//! Barrel's config folder and the settings files in it that decide where files come from:
//!
//! - `games.toml`: the game folders found or chosen, `escha = "/path"`, `ryza = "/path"`;
//! - `mods.toml`: the enabled mods in load order, `enabled = ["first", "second"]` (the last
//!   one wins a path); none by default;
//! - `mods/<folder>/mod.toml`: each installed mod ([`crate::mods`]).
//!
//! The graphics and option settings live in the same folder (`graphics.conf`, `options.conf`).

use std::path::PathBuf;

use crate::toml::{self, Table, Value};

/// The config folder: beside `BARREL_CONFIG` (a file) when that is set, else
/// `$XDG_CONFIG_HOME/barrel` (`~/.config/barrel`), on Windows `%APPDATA%\barrel`.
pub fn dir() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("BARREL_CONFIG") {
        return PathBuf::from(p).parent().map(|d| d.to_path_buf());
    }
    #[cfg(windows)]
    let base = std::env::var_os("APPDATA").map(PathBuf::from);
    #[cfg(not(windows))]
    let base = std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from).or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")));
    Some(base?.join("barrel"))
}

fn load(name: &str) -> Table {
    dir().and_then(|d| std::fs::read_to_string(d.join(name)).ok()).map(|t| toml::parse(&t)).unwrap_or_default()
}

/// The game folder `key` (`escha`, `ryza`) saved in `games.toml`.
pub fn saved_game(key: &str) -> Option<PathBuf> {
    load("games.toml").text(key).map(PathBuf::from)
}

/// Saves the game folder for `key`, keeping the other games' lines.
pub fn save_game(key: &str, folder: &std::path::Path) -> std::io::Result<PathBuf> {
    let dir = dir().ok_or_else(|| std::io::Error::other("no place for the config (no HOME)"))?;
    std::fs::create_dir_all(&dir)?;
    let path = dir.join("games.toml");
    let mut table = std::fs::read_to_string(&path).map(|t| toml::parse(&t)).unwrap_or_default();
    table.set(key, Value::Text(folder.to_string_lossy().into_owned()));
    std::fs::write(&path, table.to_text())?;
    Ok(path)
}

/// The folders of the enabled mods, in load order (`mods.toml`).
pub fn enabled_mods() -> Vec<String> {
    load("mods.toml").list("enabled")
}

/// Where the installed mods' folders are (`mods/` in the config folder).
pub fn mods_dir() -> Option<PathBuf> {
    dir().map(|d| d.join("mods"))
}
