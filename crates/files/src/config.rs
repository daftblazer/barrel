//! Barrel's config and data folders and the settings files in them that decide where files come
//! from.
//!
//! The config folder (`$XDG_CONFIG_HOME/barrel`, `~/.config/barrel`; on Windows `%APPDATA%\barrel`)
//! holds what the games share and, in a subfolder per game (`escha/`, `ryza/`), what is the
//! game's own:
//!
//! - `games.toml`: the game folders found or chosen, `escha = "/path"`, `ryza = "/path"`;
//! - `<game>/graphics.conf`, `<game>/options.conf`: the settings the Options screens write;
//! - `<game>/mods.toml`: the enabled mods in load order, `enabled = ["first", "second"]` (the
//!   last one wins a path); none by default;
//! - `<game>/mods/<folder>/mod.toml`: each installed mod ([`crate::mods`]).
//!
//! The data folder (`$XDG_DATA_HOME/barrel`, `~/.local/share/barrel`; on Windows
//! `%LOCALAPPDATA%\barrel`) holds `<game>/saves`.
//!
//! The game is named once ([`use_game`], when the install is chosen); the per-game paths
//! ([`game_dir`], [`saves_dir`], ...) are those of that game. Before that, [`game_dir`] is the
//! shared folder itself (the "Locate game files" screen reads the old, shared `graphics.conf`).
//!
//! Before Barrel ran more than one game everything sat directly in the config folder and the saves
//! in `barrel/saves`. [`use_game`] copies the old settings into Escha's folder when it has none
//! yet, and Escha keeps finding its old mods and saves where they are until its own folders exist.
//! With `BARREL_CONFIG` (a file, for test runs) the config is flat in that file's folder: no
//! subfolders, no migration.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use crate::find::Title;
use crate::toml::{self, Table, Value};

static GAME: OnceLock<Title> = OnceLock::new();

/// Names the game this process plays. The first call wins.
pub fn use_game(title: Title) {
    if GAME.set(title).is_ok() {
        migrate(title);
    }
}

/// The game named by [`use_game`].
pub fn game() -> Option<Title> {
    GAME.get().copied()
}

/// The shared config folder: beside `BARREL_CONFIG` (a file) when that is set, else
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

/// The game's own config folder (`<config>/escha`); the shared folder when no game is named
/// yet or `BARREL_CONFIG` is set.
pub fn game_dir() -> Option<PathBuf> {
    let shared = dir()?;
    match game() {
        Some(t) if std::env::var_os("BARREL_CONFIG").is_none() => Some(shared.join(t.key())),
        _ => Some(shared),
    }
}

/// The graphics settings file: `BARREL_CONFIG`, else `graphics.conf` in the game's config folder.
pub fn graphics_path() -> Option<PathBuf> {
    match std::env::var_os("BARREL_CONFIG") {
        Some(p) => Some(p.into()),
        None => Some(game_dir()?.join("graphics.conf")),
    }
}

/// The data folder: `$XDG_DATA_HOME/barrel` (`~/.local/share/barrel`), on Windows
/// `%LOCALAPPDATA%\barrel`.
fn data_root() -> Option<PathBuf> {
    #[cfg(windows)]
    let base = std::env::var_os("LOCALAPPDATA").or_else(|| std::env::var_os("APPDATA")).map(PathBuf::from);
    #[cfg(not(windows))]
    let base = std::env::var_os("XDG_DATA_HOME").map(PathBuf::from).or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")));
    Some(base?.join("barrel"))
}

/// The folder of the game's save slots: `<data>/<game>/saves`. Escha keeps using the old shared
/// `<data>/saves` while its own folder does not exist yet.
pub fn saves_dir() -> Option<PathBuf> {
    let root = data_root()?;
    let Some(title) = game() else { return Some(root.join("saves")) };
    let own = root.join(title.key()).join("saves");
    let old = root.join("saves");
    Some(if title == Title::Escha && !own.exists() && old.is_dir() { old } else { own })
}

/// Copies the pre-per-game settings into the game's folder, once.
fn migrate(title: Title) {
    if std::env::var_os("BARREL_CONFIG").is_some() {
        return;
    }
    let (Some(shared), Some(own)) = (dir(), game_dir()) else { return };
    let files: &[&str] = if title == Title::Escha { &["graphics.conf", "options.conf", "mods.toml"] } else { &["graphics.conf"] };
    for name in files {
        let (old, new) = (shared.join(name), own.join(name));
        if old.is_file() && !new.exists() && std::fs::create_dir_all(&own).is_ok() {
            let _ = std::fs::copy(&old, &new);
        }
    }
}

fn load_in(dir: Option<PathBuf>, name: &str) -> Table {
    dir.and_then(|d| std::fs::read_to_string(d.join(name)).ok()).map(|t| toml::parse(&t)).unwrap_or_default()
}

/// The game folder `key` (`escha`, `ryza`) saved in `games.toml`.
pub fn saved_game(key: &str) -> Option<PathBuf> {
    load_in(dir(), "games.toml").text(key).map(PathBuf::from)
}

/// Saves the game folder for `key`, keeping the other games' lines.
pub fn save_game(key: &str, folder: &Path) -> std::io::Result<PathBuf> {
    let dir = dir().ok_or_else(|| std::io::Error::other("no place for the config (no HOME)"))?;
    std::fs::create_dir_all(&dir)?;
    let path = dir.join("games.toml");
    let mut table = std::fs::read_to_string(&path).map(|t| toml::parse(&t)).unwrap_or_default();
    table.set(key, Value::Text(folder.to_string_lossy().into_owned()));
    std::fs::write(&path, table.to_text())?;
    Ok(path)
}

/// The folders of the enabled mods, in load order (the game's `mods.toml`).
pub fn enabled_mods() -> Vec<String> {
    load_in(game_dir(), "mods.toml").list("enabled")
}

/// Where the game's installed mods' folders are (`mods/` in its config folder). Escha keeps
/// using the old shared `mods/` while its own does not exist yet.
pub fn mods_dir() -> Option<PathBuf> {
    let own = game_dir()?.join("mods");
    let old = dir()?.join("mods");
    Some(if game() == Some(Title::Escha) && !own.exists() && old.is_dir() { old } else { own })
}
