//! The layered file system: a list of roots (the game's install first, then each mod in load
//! order), a game-relative path looked up from the topmost root down, case-insensitively (the
//! game's paths are Windows paths: `Data/x64/Field` and `data/X64/field` are one folder).

use std::collections::HashMap;
use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

/// One root of the layers: the install, or a mod's folder.
#[derive(Clone, Debug)]
pub struct Layer {
    /// What messages call it ("game", or the mod's folder name).
    pub name: String,
    pub root: PathBuf,
}

/// A folder's entries by lower-case name, to their spelling on the disk.
type Names = Arc<HashMap<String, String>>;

pub struct GameFs {
    /// The install first, then the mods, the last one on top.
    layers: Vec<Layer>,
    /// The install's root as `canonicalize` spells it, for paths given another way.
    canonical: Option<PathBuf>,
    /// Folder listings for the case-insensitive lookup, by the folder's real path.
    folders: Mutex<HashMap<PathBuf, Option<Names>>>,
}

/// The components of a game-relative path (either slash, `.` and empty parts dropped), or `None`
/// when it climbs out with `..`.
fn components(rel: &str) -> Option<Vec<&str>> {
    let mut out = Vec::new();
    for c in rel.split(['/', '\\']) {
        match c {
            "" | "." => {}
            ".." => return None,
            c => out.push(c),
        }
    }
    Some(out)
}

fn not_found(rel: &str) -> io::Error {
    io::Error::new(io::ErrorKind::NotFound, format!("{rel}: not in the game's files"))
}

impl GameFs {
    /// The layers of one install and no mods.
    pub fn new(install: impl Into<PathBuf>) -> GameFs {
        let root: PathBuf = install.into();
        let canonical = root.canonicalize().ok().filter(|c| *c != root);
        GameFs { layers: vec![Layer { name: "game".into(), root }], canonical, folders: Mutex::default() }
    }

    /// A layer on top of those so far.
    pub fn with_layer(mut self, name: impl Into<String>, root: impl Into<PathBuf>) -> GameFs {
        self.layers.push(Layer { name: name.into(), root: root.into() });
        self
    }

    /// The install's folder, the bottom layer.
    pub fn install_root(&self) -> &Path {
        &self.layers[0].root
    }

    /// The layers, the install first.
    pub fn layers(&self) -> &[Layer] {
        &self.layers
    }

    /// The game-relative path (`Data/x64/a.g1t`) of `path` when it lies in the install's folder.
    pub fn relative(&self, path: &Path) -> Option<String> {
        let rest = path.strip_prefix(self.install_root()).ok().or_else(|| path.strip_prefix(self.canonical.as_deref()?).ok())?;
        Some(rest.components().map(|c| c.as_os_str().to_string_lossy()).collect::<Vec<_>>().join("/"))
    }

    /// `dir`'s entries by lower-case name; `None` for what is not a folder. Remembered.
    fn names(&self, dir: &Path) -> Option<Names> {
        if let Some(n) = self.folders.lock().unwrap().get(dir) {
            return n.clone();
        }
        let names = std::fs::read_dir(dir).ok().map(|rd| {
            Arc::new(rd.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).map(|n| (n.to_lowercase(), n)).collect::<HashMap<_, _>>())
        });
        self.folders.lock().unwrap().insert(dir.to_path_buf(), names.clone());
        names
    }

    /// The path in `root` that `parts` name, in whatever case the disk spells it.
    fn find_in(&self, root: &Path, parts: &[&str]) -> Option<PathBuf> {
        // The exact spelling first: one stat, and the only lookup a case-sensitive-correct path needs.
        let direct = parts.iter().fold(root.to_path_buf(), |p, c| p.join(c));
        if std::fs::symlink_metadata(&direct).is_ok() {
            return Some(direct);
        }
        let mut at = root.to_path_buf();
        for c in parts {
            at = at.join(self.names(&at)?.get(&c.to_lowercase())?);
        }
        Some(at)
    }

    /// The real path of `rel` in the topmost layer that holds it.
    pub fn resolve(&self, rel: &str) -> Option<PathBuf> {
        let parts = components(rel)?;
        self.layers.iter().rev().find_map(|l| self.find_in(&l.root, &parts))
    }

    /// The name of the layer `rel` comes from.
    pub fn layer_of(&self, rel: &str) -> Option<&str> {
        let parts = components(rel)?;
        self.layers.iter().rev().find(|l| self.find_in(&l.root, &parts).is_some()).map(|l| l.name.as_str())
    }

    pub fn exists(&self, rel: &str) -> bool {
        self.resolve(rel).is_some()
    }

    pub fn is_dir(&self, rel: &str) -> bool {
        self.resolve(rel).is_some_and(|p| p.is_dir())
    }

    pub fn is_file(&self, rel: &str) -> bool {
        self.resolve(rel).is_some_and(|p| p.is_file())
    }

    pub fn open(&self, rel: &str) -> io::Result<File> {
        File::open(self.resolve(rel).ok_or_else(|| not_found(rel))?)
    }

    pub fn read(&self, rel: &str) -> io::Result<Vec<u8>> {
        std::fs::read(self.resolve(rel).ok_or_else(|| not_found(rel))?)
    }

    pub fn read_to_string(&self, rel: &str) -> io::Result<String> {
        std::fs::read_to_string(self.resolve(rel).ok_or_else(|| not_found(rel))?)
    }

    /// The names in folder `rel`: those of every layer that has it, once each (the topmost
    /// layer's spelling), in name order (ignoring case).
    pub fn list(&self, rel: &str) -> io::Result<Vec<String>> {
        let parts = components(rel).ok_or_else(|| not_found(rel))?;
        let mut merged: HashMap<String, String> = HashMap::new();
        let mut found = false;
        for layer in &self.layers {
            if let Some(names) = self.find_in(&layer.root, &parts).and_then(|d| self.names_fresh(&d)) {
                found = true;
                merged.extend(names.iter().map(|(k, v)| (k.clone(), v.clone())));
            }
        }
        if !found {
            return Err(not_found(rel));
        }
        let mut out: Vec<(String, String)> = merged.into_iter().collect();
        out.sort();
        Ok(out.into_iter().map(|(_, name)| name).collect())
    }

    /// A folder's listing as the disk has it now (a listing the game asks for is of what is there).
    fn names_fresh(&self, dir: &Path) -> Option<Names> {
        self.folders.lock().unwrap().remove(dir);
        self.names(dir)
    }
}

static GLOBAL: OnceLock<GameFs> = OnceLock::new();

/// Makes `fs` the layers every game file is read through. Once a run; false when one is
/// installed already (and then `fs` is dropped).
pub fn install(fs: GameFs) -> bool {
    GLOBAL.set(fs).is_ok()
}

/// The installed layers, if any.
pub fn installed() -> Option<&'static GameFs> {
    GLOBAL.get()
}

/// The installed layers and `path`'s game-relative name, when `path` is in the install.
fn mapped(path: &Path) -> Option<(&'static GameFs, String)> {
    let g = GLOBAL.get()?;
    g.relative(path).map(|r| (g, r))
}

/// The bytes of the game file at `path` (the install's path, `<install>/Data/...`), from the
/// topmost layer that has it; a path outside the install is read as it is.
pub fn read(path: impl AsRef<Path>) -> io::Result<Vec<u8>> {
    let path = path.as_ref();
    match mapped(path) {
        Some((g, rel)) => g.read(&rel),
        None => std::fs::read(path),
    }
}

pub fn read_to_string(path: impl AsRef<Path>) -> io::Result<String> {
    let path = path.as_ref();
    match mapped(path) {
        Some((g, rel)) => g.read_to_string(&rel),
        None => std::fs::read_to_string(path),
    }
}

/// The game file at `path` opened for reading.
pub fn open(path: impl AsRef<Path>) -> io::Result<File> {
    let path = path.as_ref();
    match mapped(path) {
        Some((g, rel)) => g.open(&rel),
        None => File::open(path),
    }
}

pub fn exists(path: impl AsRef<Path>) -> bool {
    let path = path.as_ref();
    match mapped(path) {
        Some((g, rel)) => g.exists(&rel),
        None => path.exists(),
    }
}

pub fn is_dir(path: impl AsRef<Path>) -> bool {
    let path = path.as_ref();
    match mapped(path) {
        Some((g, rel)) => g.is_dir(&rel),
        None => path.is_dir(),
    }
}

pub fn is_file(path: impl AsRef<Path>) -> bool {
    let path = path.as_ref();
    match mapped(path) {
        Some((g, rel)) => g.is_file(&rel),
        None => path.is_file(),
    }
}

/// The names in the game folder at `path`, merged over the layers, in name order.
pub fn list(path: impl AsRef<Path>) -> io::Result<Vec<String>> {
    let path = path.as_ref();
    match mapped(path) {
        Some((g, rel)) => g.list(&rel),
        None => {
            let mut names: Vec<String> = std::fs::read_dir(path)?.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
            names.sort_by_key(|n| n.to_lowercase());
            Ok(names)
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A scratch folder that goes away with the test.
    pub struct Scratch(pub PathBuf);

    impl Scratch {
        pub fn new(tag: &str) -> Scratch {
            let p = std::env::temp_dir().join(format!("barrel-files-{tag}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&p);
            std::fs::create_dir_all(&p).unwrap();
            Scratch(p)
        }

        pub fn file(&self, rel: &str, bytes: &[u8]) -> &Scratch {
            let p = self.0.join(rel);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, bytes).unwrap();
            self
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn the_topmost_layer_with_a_path_wins() {
        let game = Scratch::new("order-game");
        game.file("Saves/a.xml", b"game").file("Saves/b.xml", b"game b");
        let one = Scratch::new("order-one");
        one.file("Saves/a.xml", b"one");
        let two = Scratch::new("order-two");
        two.file("Saves/a.xml", b"two").file("Saves/c.xml", b"two c");
        let fs = GameFs::new(&game.0).with_layer("one", &one.0).with_layer("two", &two.0);
        assert_eq!(fs.read("Saves/a.xml").unwrap(), b"two");
        assert_eq!(fs.read("Saves/b.xml").unwrap(), b"game b");
        assert_eq!(fs.read("Saves/c.xml").unwrap(), b"two c");
        assert_eq!(fs.layer_of("Saves/a.xml"), Some("two"));
        assert_eq!(fs.layer_of("Saves/b.xml"), Some("game"));
        assert_eq!(fs.list("Saves").unwrap(), ["a.xml", "b.xml", "c.xml"]);
    }

    #[test]
    fn lookup_ignores_case_and_slash_kind() {
        let game = Scratch::new("case");
        game.file("Data/x64/Field/Town.G1T", b"x");
        let fs = GameFs::new(&game.0);
        assert_eq!(fs.read("data\\X64\\field\\town.g1t").unwrap(), b"x");
        assert!(fs.is_dir("DATA/x64"));
        assert!(fs.exists("./Data//x64/Field/TOWN.g1t"));
        assert_eq!(fs.list("data/X64").unwrap(), ["Field"]);
    }

    #[test]
    fn a_missing_file_is_a_not_found_error() {
        let game = Scratch::new("missing");
        game.file("a", b"x");
        let fs = GameFs::new(&game.0);
        let e = fs.read("Data/none.bin").unwrap_err();
        assert_eq!(e.kind(), io::ErrorKind::NotFound);
        assert!(e.to_string().contains("Data/none.bin"));
        assert_eq!(fs.list("Data").unwrap_err().kind(), io::ErrorKind::NotFound);
        assert!(fs.read("../outside").is_err());
        assert!(!fs.exists("a/b"));
    }

    #[test]
    fn a_mod_overrides_one_file_and_leaves_the_rest() {
        let game = Scratch::new("mod-game");
        game.file("Saves_EN/item/itemData.xml", b"<Items a='1'/>").file("Saves_EN/shop/ShopData.xml", b"<Shop/>");
        let m = Scratch::new("mod-mod");
        m.file("saves_en/ITEM/itemdata.xml", b"<Items a='2'/>");
        let fs = GameFs::new(&game.0).with_layer("m", &m.0);
        assert_eq!(fs.read("Saves_EN/item/itemData.xml").unwrap(), b"<Items a='2'/>");
        assert_eq!(fs.read("Saves_EN/shop/ShopData.xml").unwrap(), b"<Shop/>");
        // The folders the mod spells differently are one folder with the game's (its spelling on top).
        assert_eq!(fs.list("Saves_EN").unwrap(), ["ITEM", "shop"]);
    }

    #[test]
    fn install_paths_map_to_relative_names() {
        let game = Scratch::new("relative");
        let fs = GameFs::new(&game.0);
        assert_eq!(fs.relative(&game.0.join("Data/x64/a.g1t")).as_deref(), Some("Data/x64/a.g1t"));
        assert_eq!(fs.relative(&game.0), Some(String::new()));
        assert_eq!(fs.relative(Path::new("/elsewhere/Data")), None);
    }
}
