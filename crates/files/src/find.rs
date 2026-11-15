//! Finding the game's install. In order:
//!
//! 1. `--game DIR`, or `BARREL_GAME_DIR` (the caller passes the command line's);
//! 2. the folder saved in the config (`games.toml`, written when the user locates the game);
//! 3. Steam's libraries: every `libraryfolders.vdf` under the usual Steam folders, and in each
//!    library the game's `appmanifest_<app id>.acf` (its `installdir`) or its usual folder name;
//! 4. otherwise nothing: the caller shows the "Locate game files" screen.

use std::path::{Path, PathBuf};

use crate::vdf;

/// A game Barrel plays.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Title {
    Escha,
    Ryza,
}

impl Title {
    /// Every game, in the order the picker lists them.
    pub const ALL: [Title; 2] = [Title::Escha, Title::Ryza];

    /// The game whose install `dir` is.
    pub fn of_install(dir: &Path) -> Option<Title> {
        Title::ALL.into_iter().find(|t| is_install(*t, dir))
    }

    /// The key of the game in `games.toml`.
    pub fn key(self) -> &'static str {
        match self {
            Title::Escha => "escha",
            Title::Ryza => "ryza",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Title::Escha => "Atelier Escha & Logy DX",
            Title::Ryza => "Atelier Ryza DX",
        }
    }

    /// The Steam app.
    pub fn app_id(self) -> u32 {
        match self {
            Title::Escha => 1152310,
            Title::Ryza => 3365030,
        }
    }

    /// The folder the Steam release installs to, under `steamapps/common`.
    pub fn folder(self) -> &'static str {
        match self {
            Title::Escha => "Atelier Escha and Logy DX",
            Title::Ryza => "Atelier Ryza DX",
        }
    }

    /// The file whose presence says a folder holds the game (a program Barrel reads tables from).
    pub fn marker(self) -> &'static str {
        match self {
            Title::Escha => "Atelier_Escha_and_Logy_EN.exe",
            Title::Ryza => "Atelier_Ryza_DX.exe",
        }
    }
}

/// Whether `dir` holds `title`'s files: its marker program and a `Data` folder.
pub fn is_install(title: Title, dir: &Path) -> bool {
    let fs = crate::GameFs::new(dir);
    fs.is_file(title.marker()) && fs.is_dir("Data")
}

/// Where Steam may be installed on this system.
pub fn steam_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    #[cfg(not(windows))]
    if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
        roots.push(home.join(".local/share/Steam"));
        roots.push(home.join(".steam/steam"));
        roots.push(home.join(".steam/root"));
        roots.push(home.join(".var/app/com.valvesoftware.Steam/.local/share/Steam"));
        roots.push(home.join(".var/app/com.valvesoftware.Steam/.steam/steam"));
    }
    #[cfg(windows)]
    for var in ["ProgramFiles(x86)", "ProgramFiles"] {
        // No registry: Steam's default folders.
        if let Some(p) = std::env::var_os(var) {
            roots.push(PathBuf::from(p).join("Steam"));
        }
    }
    #[cfg(windows)]
    roots.push(PathBuf::from(r"C:\Program Files (x86)\Steam"));
    roots
}

/// The library folders (each holding a `steamapps`): `roots` that have one, and every path
/// the `libraryfolders.vdf` files under them list. Each once.
pub fn libraries(roots: &[PathBuf]) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();
    let mut add = |p: PathBuf| {
        let key = p.canonicalize().unwrap_or_else(|_| p.clone());
        if p.join("steamapps").is_dir() && !out.iter().any(|o| o.canonicalize().unwrap_or_else(|_| o.clone()) == key) {
            out.push(p);
        }
    };
    for root in roots {
        add(root.clone());
        for rel in ["steamapps/libraryfolders.vdf", "config/libraryfolders.vdf"] {
            let Ok(text) = std::fs::read_to_string(root.join(rel)) else { continue };
            let vdf = vdf::parse(&text);
            let Some(list) = vdf.get("libraryfolders") else { continue };
            for (_, entry) in list.children() {
                // The current format: a block with a "path"; the old one: the path as the value.
                let path = entry.get("path").and_then(vdf::Node::text).or_else(|| entry.text());
                if let Some(p) = path.filter(|p| !p.is_empty()) {
                    add(PathBuf::from(p));
                }
            }
        }
    }
    out
}

/// `title`'s install in one of `libraries`.
pub fn in_libraries(title: Title, libraries: &[PathBuf]) -> Option<PathBuf> {
    for lib in libraries {
        let steamapps = lib.join("steamapps");
        let installdir = std::fs::read_to_string(steamapps.join(format!("appmanifest_{}.acf", title.app_id())))
            .ok()
            .and_then(|t| vdf::parse(&t).get("AppState").and_then(|s| s.get("installdir")).and_then(vdf::Node::text).map(str::to_string));
        for name in installdir.iter().map(String::as_str).chain([title.folder()]) {
            let dir = steamapps.join("common").join(name);
            if is_install(title, &dir) {
                return Some(dir);
            }
        }
    }
    None
}

/// `title`'s install in Steam's libraries on this system.
pub fn in_steam(title: Title) -> Option<PathBuf> {
    in_libraries(title, &libraries(&steam_roots()))
}

/// Where the game's install was found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Found {
    /// Given on the command line or by `BARREL_GAME_DIR`.
    Given(PathBuf),
    /// Saved in the config by an earlier run.
    Saved(PathBuf),
    Steam(PathBuf),
}

impl Found {
    pub fn path(&self) -> &Path {
        match self {
            Found::Given(p) | Found::Saved(p) | Found::Steam(p) => p,
        }
    }
}

/// `title`'s install by the search order above: `given` is the command line's `--game`
/// (the `BARREL_GAME_DIR` variable stands behind it). `Err` when a folder that was given does
/// not hold the game; `Ok(None)` when nothing was found, and the user must be asked.
pub fn find(title: Title, given: Option<PathBuf>) -> Result<Option<Found>, String> {
    if let Some(dir) = given.or_else(|| std::env::var_os("BARREL_GAME_DIR").map(PathBuf::from)) {
        return if is_install(title, &dir) { Ok(Some(Found::Given(dir))) } else { Err(format!("{}: not a folder with {} (no {})", dir.display(), title.name(), title.marker())) };
    }
    if let Some(dir) = crate::config::saved_game(title.key()).filter(|d| is_install(title, d)) {
        return Ok(Some(Found::Saved(dir)));
    }
    Ok(in_steam(title).map(Found::Steam))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fs::tests::Scratch;

    #[test]
    fn steam_libraries_lead_to_the_game() {
        let steam = Scratch::new("steam-root");
        let lib = Scratch::new("steam-lib");
        steam.file("steamapps/libraryfolders.vdf", format!("\"libraryfolders\"\n{{\n \"0\" {{ \"path\" \"{}\" }}\n \"1\" {{ \"path\" \"{}\" }}\n}}\n", steam.0.display(), lib.0.display()).as_bytes());
        lib.file("steamapps/appmanifest_3365030.acf", b"\"AppState\" { \"appid\" \"3365030\" \"installdir\" \"Ryza Folder\" }");
        lib.file("steamapps/common/Ryza Folder/atelier_ryza_dx.exe", b"MZ").file("steamapps/common/Ryza Folder/Data/a.pak", b"");
        lib.file("steamapps/common/Atelier Escha and Logy DX/Atelier_Escha_and_Logy_EN.exe", b"MZ").file("steamapps/common/Atelier Escha and Logy DX/Data/a.pak", b"");
        let libs = libraries(&[steam.0.clone(), PathBuf::from("/no/such/steam")]);
        assert_eq!(libs, [steam.0.clone(), lib.0.clone()]);
        // By the manifest's folder (any case of the exe), and by the usual folder name.
        assert_eq!(in_libraries(Title::Ryza, &libs), Some(lib.0.join("steamapps/common/Ryza Folder")));
        assert_eq!(in_libraries(Title::Escha, &libs), Some(lib.0.join("steamapps/common/Atelier Escha and Logy DX")));
    }

    #[test]
    fn a_folder_without_the_game_is_no_install() {
        let d = Scratch::new("not-install");
        d.file("Data/a.pak", b"");
        assert!(!is_install(Title::Escha, &d.0));
        d.file("Atelier_Escha_and_Logy_EN.exe", b"MZ");
        assert!(is_install(Title::Escha, &d.0));
        assert!(!is_install(Title::Ryza, &d.0));
        assert!(find(Title::Ryza, Some(d.0.clone())).is_err());
        assert_eq!(find(Title::Escha, Some(d.0.clone())), Ok(Some(Found::Given(d.0.clone()))));
    }
}
