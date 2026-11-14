//! A Ryza DX install: its archives (`Data/*.PAK`, `barrel_formats::pak::Archives`) and the
//! files the game reads from them.

use std::path::{Path, PathBuf};

use barrel_formats::game::Game;
use barrel_formats::pak::Archives;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

pub struct Install {
    pub root: PathBuf,
    pub archives: Archives,
}

/// A field's containers, read from the archives: the field (`<field>.elixir.gz`: terrain,
/// placements, textures, shaders, heightmap, light, collision, grass), its objects' models
/// (`<field>_obj`) and its environment (`<field>_env`: `<FIELD>_env.bin` and the `_ad` /
/// `_as` light probes, `docs/ryza/open.md`).
pub struct FieldFiles {
    /// The field's name as its containers' entries spell it (`S01_I_ATTIC`).
    pub area: String,
    pub main: Vec<u8>,
    pub objects: Vec<u8>,
    pub env: Option<Vec<u8>>,
}

impl Install {
    /// The install at `root` (the folder holding `Atelier_Ryza_DX.exe` and `Data`).
    pub fn open(root: &Path) -> Result<Install> {
        let archives = Archives::open_dir(&root.join("Data"))?;
        if archives.game() != Game::RyzaDx {
            return Err(format!("{}: not Atelier Ryza DX's archives ({:?})", root.display(), archives.game()).into());
        }
        Ok(Install { root: root.to_path_buf(), archives })
    }

    /// A file from the archives by its name there (`\data\x64\...`, any case).
    pub fn read(&self, name: &str) -> Result<Vec<u8>> {
        Ok(self.archives.read(name).ok_or_else(|| format!("{name}: not in the archives"))??)
    }

    /// The fields the archives hold (`s01_i_attic`, ...), in name order.
    pub fn fields(&self) -> Vec<String> {
        let mut out: Vec<String> = self
            .archives
            .names()
            .filter_map(|n| {
                let rest = n.strip_prefix("\\data\\x64\\field\\")?;
                let (dir, file) = rest.split_once('\\')?;
                (file == format!("{dir}.elixir.gz")).then(|| dir.to_string())
            })
            .collect();
        out.sort();
        out
    }

    /// A field's containers by its name (`s01_i_attic`, any case).
    pub fn field(&self, name: &str) -> Result<FieldFiles> {
        let n = name.to_ascii_lowercase();
        let path = |suffix: &str| format!("\\data\\x64\\field\\{n}\\{n}{suffix}.elixir.gz");
        Ok(FieldFiles {
            area: n.to_ascii_uppercase(),
            main: self.read(&path(""))?,
            objects: self.read(&path("_obj"))?,
            env: self.archives.read(&path("_env")).transpose()?,
        })
    }
}
