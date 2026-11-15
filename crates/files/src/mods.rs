//! Mods: a folder under the config folder's `mods/` holding a `mod.toml` and, at the game's own
//! paths, the files it replaces or adds:
//!
//! ```text
//! mods/better-items/mod.toml
//! mods/better-items/Saves_EN/item/itemData.xml
//! ```
//!
//! ```toml
//! name = "Better Items"
//! version = "1.0"
//! needs = ["base-pack"]   # optional: mods (by folder or name) enabled before this one
//! ```
//!
//! `mods.toml` lists the enabled folders in load order; the last wins a path. None are enabled
//! by default, and with none the game's files are read exactly as they are.

use std::path::{Path, PathBuf};

use crate::fs::GameFs;
use crate::toml;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModInfo {
    /// The mod's folder name (what `mods.toml` lists).
    pub folder: String,
    pub name: String,
    pub version: String,
    pub needs: Vec<String>,
    pub root: PathBuf,
}

/// The mod in `root` (its `mod.toml`).
pub fn read_info(root: &Path) -> Result<ModInfo, String> {
    let folder = root.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let text = std::fs::read_to_string(root.join("mod.toml")).map_err(|e| format!("{folder}: mod.toml: {e}"))?;
    let t = toml::parse(&text);
    Ok(ModInfo {
        name: t.text("name").map_or_else(|| folder.clone(), str::to_string),
        version: t.text("version").unwrap_or("0").to_string(),
        needs: t.list("needs"),
        folder,
        root: root.to_path_buf(),
    })
}

/// Every mod installed in `mods_dir`, in folder order, and the problems with folders that are
/// not mods.
pub fn installed(mods_dir: &Path) -> (Vec<ModInfo>, Vec<String>) {
    let mut folders: Vec<PathBuf> = std::fs::read_dir(mods_dir).map(|rd| rd.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect()).unwrap_or_default();
    folders.sort();
    let (mut mods, mut problems) = (Vec::new(), Vec::new());
    for f in folders {
        match read_info(&f) {
            Ok(m) => mods.push(m),
            Err(e) => problems.push(e),
        }
    }
    (mods, problems)
}

/// The enabled mods, in load order. A mod that is missing, has no `mod.toml` or needs one that is
/// not enabled before it is left out, with a problem line saying why.
pub fn load(mods_dir: &Path, enabled: &[String]) -> (Vec<ModInfo>, Vec<String>) {
    let mut chosen: Vec<ModInfo> = Vec::new();
    let mut problems = Vec::new();
    for folder in enabled {
        let root = mods_dir.join(folder);
        let info = match read_info(&root) {
            Ok(i) => i,
            Err(e) => {
                problems.push(format!("mod skipped: {e}"));
                continue;
            }
        };
        if let Some(missing) = info.needs.iter().find(|n| !chosen.iter().any(|c| c.folder == **n || c.name == **n)) {
            problems.push(format!("mod {} skipped: it needs {missing}, which is not enabled before it", info.folder));
            continue;
        }
        chosen.push(info);
    }
    (chosen, problems)
}

/// The layers for the game installed in `install`: the install, then the mods the config enables.
/// Problems with mods are printed and the mods left out.
pub fn game_fs(install: &Path) -> GameFs {
    game_fs_with(install, crate::config::mods_dir().as_deref(), &crate::config::enabled_mods())
}

/// [`game_fs`] with the mods folder and the enabled list given.
pub fn game_fs_with(install: &Path, mods_dir: Option<&Path>, enabled: &[String]) -> GameFs {
    let mut fs = GameFs::new(install);
    let Some(mods_dir) = mods_dir.filter(|_| !enabled.is_empty()) else { return fs };
    let (mods, problems) = load(mods_dir, enabled);
    for p in problems {
        eprintln!("mods: {p}");
    }
    for m in mods {
        eprintln!("mods: {} {} ({})", m.name, m.version, m.folder);
        fs = fs.with_layer(m.folder, m.root);
    }
    fs
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fs::tests::Scratch;

    #[test]
    fn enabled_mods_stack_in_list_order() {
        let mods = Scratch::new("mods-list");
        mods.file("a/mod.toml", b"name = \"Alpha\"\nversion = \"1.1\"\n").file("a/Saves/x.xml", b"a");
        mods.file("b/mod.toml", b"needs = [\"Alpha\"]\n").file("b/Saves/x.xml", b"b");
        mods.file("c/mod.toml", b"needs = [\"zzz\"]\n");
        mods.file("off/mod.toml", b"").file("off/Saves/x.xml", b"off");
        let game = Scratch::new("mods-game");
        game.file("Saves/x.xml", b"game");
        let enabled: Vec<String> = ["a", "b", "c", "gone"].map(String::from).to_vec();
        let (loaded, problems) = load(&mods.0, &enabled);
        assert_eq!(loaded.iter().map(|m| m.folder.as_str()).collect::<Vec<_>>(), ["a", "b"]);
        assert_eq!((loaded[0].name.as_str(), loaded[0].version.as_str()), ("Alpha", "1.1"));
        assert_eq!(loaded[1].name, "b");
        assert_eq!(problems.len(), 2);
        let fs = game_fs_with(&game.0, Some(&mods.0), &enabled);
        assert_eq!(fs.read("Saves/x.xml").unwrap(), b"b");
        // Disabled by default: no list, no mods.
        let plain = game_fs_with(&game.0, Some(&mods.0), &[]);
        assert_eq!(plain.read("Saves/x.xml").unwrap(), b"game");
        assert_eq!(plain.layers().len(), 1);
        assert_eq!(installed(&mods.0).0.len(), 4);
    }
}
