//! Where Ryza DX's saves are and what their first 0x100 plain bytes say (what the load screen
//! lists: `docs/formats.md`, "Ryza DX saves"). Read only: nothing here creates, writes or locks a
//! file or folder.

use std::path::{Path, PathBuf};

use barrel_files::find::Title;

/// What the load screen shows of one save, from the file's plain header.
#[derive(Clone, Debug, PartialEq)]
pub struct Header {
    /// When it was saved, as a Windows `FILETIME` (100 ns ticks since 1601), at +0x30.
    pub saved: u64,
    /// Seconds played, at +0x3c.
    pub play_seconds: u32,
    pub money: u32,
    /// The alchemy level, at +0x44.
    pub alchemy_level: u8,
    /// Gold coins, at +0x48.
    pub gold_coins: u32,
    /// The party in order: (character, level), a pair of bytes each from +0x4e, `0xff` for an
    /// empty place (ten of them).
    pub party: Vec<(u8, u8)>,
}

const PARTY_AT: usize = 0x4e;
const PARTY_PLACES: usize = 10;

impl Header {
    pub fn parse(file: &[u8]) -> Option<Header> {
        let head = file.get(..crate::save::HEADER)?;
        let u32_at = |o: usize| u32::from_le_bytes(head[o..o + 4].try_into().unwrap());
        let party = (0..PARTY_PLACES).map(|i| (head[PARTY_AT + i * 2], head[PARTY_AT + i * 2 + 1])).take_while(|&(id, _)| id != 0xff).collect();
        Some(Header {
            saved: u64::from_le_bytes(head[0x30..0x38].try_into().unwrap()),
            play_seconds: u32_at(0x3c),
            money: u32_at(0x40),
            alchemy_level: head[0x44],
            gold_coins: u32_at(0x48),
            party,
        })
    }

    /// The play time as the screen writes it: `000:08:13`.
    pub fn play_time(&self) -> String {
        let s = self.play_seconds;
        format!("{:03}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
    }

    /// The time of saving in the machine's time zone as the screen writes it: `4/4/2026 6:57`.
    pub fn saved_at(&self) -> String {
        const EPOCH_DIFF: u64 = 11_644_473_600;
        let secs = (self.saved / 10_000_000).saturating_sub(EPOCH_DIFF) as libc::time_t;
        // SAFETY: `localtime_r` writes only the `tm` it is given.
        let tm = unsafe {
            let mut tm: libc::tm = std::mem::zeroed();
            libc::localtime_r(&secs, &mut tm);
            tm
        };
        format!("{}/{}/{} {}:{:02}", tm.tm_mon + 1, tm.tm_mday, tm.tm_year + 1900, tm.tm_hour, tm.tm_min)
    }
}

/// One save on the load list.
#[derive(Clone, Debug, PartialEq)]
pub struct Slot {
    /// The folder's name (`AutoSaveData`, `GameData00`).
    pub name: String,
    pub file: PathBuf,
    pub header: Header,
}

/// The folder holding `GameData<NN>` / `AutoSaveData` for the player: `root` itself, else its
/// first sub-folder (the user's id, in name order) that does.
pub fn user_folder(root: &Path) -> Option<PathBuf> {
    let holds = |d: &Path| std::fs::read_dir(d).is_ok_and(|e| e.flatten().any(|e| is_slot_name(&e.file_name().to_string_lossy()) && e.path().join("data.dat").is_file()));
    if holds(root) {
        return Some(root.to_path_buf());
    }
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(root).ok()?.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
    dirs.sort();
    dirs.into_iter().find(|d| holds(d))
}

fn is_slot_name(name: &str) -> bool {
    name == "AutoSaveData" || name.strip_prefix("GameData").is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
}

/// The saves in a user folder, as the load list shows them: the autosave first, then the slots
/// by number. A file whose header does not read is left out.
pub fn slots(folder: &Path) -> Vec<Slot> {
    let Ok(entries) = std::fs::read_dir(folder) else { return Vec::new() };
    let mut out: Vec<Slot> = entries
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            if !is_slot_name(&name) {
                return None;
            }
            let file = e.path().join("data.dat");
            let header = Header::parse(&read_head(&file)?)?;
            Some(Slot { name, file, header })
        })
        .collect();
    out.sort_by(|a, b| (a.name != "AutoSaveData", &a.name).cmp(&(b.name != "AutoSaveData", &b.name)));
    out
}

/// The first 0x100 bytes of a file (no more is read).
fn read_head(file: &Path) -> Option<Vec<u8>> {
    use std::io::Read;
    let mut buf = vec![0u8; crate::save::HEADER];
    std::fs::File::open(file).ok()?.read_exact(&mut buf).ok()?;
    Some(buf)
}

/// Where the Windows game keeps its saves inside a Proton prefix: under the Steam library's
/// `steamapps/compatdata/<app id>/pfx`.
const PREFIX_SAVES: &str = "pfx/drive_c/users/steamuser/AppData/Local/KoeiTecmo/Atelier Ryza DX";

/// The original's own save folder in one of the Steam `libraries`, the user's folder inside it.
pub fn in_libraries(libraries: &[PathBuf]) -> Option<PathBuf> {
    libraries.iter().find_map(|lib| user_folder(&lib.join("steamapps/compatdata").join(Title::Ryza.app_id().to_string()).join(PREFIX_SAVES)))
}

/// The saves to list: `--saves DIR` when given, else the original's own folder in a Steam library
/// on this system. `None` when there are none (the title then has no Load Game).
pub fn find(given: Option<&Path>) -> Option<PathBuf> {
    match given {
        Some(dir) => user_folder(dir),
        None => in_libraries(&barrel_files::find::libraries(&barrel_files::find::steam_roots())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(saved: u64, seconds: u32) -> Vec<u8> {
        let mut b = vec![0u8; 0x100 + 8];
        b[0x30..0x38].copy_from_slice(&saved.to_le_bytes());
        b[0x3c..0x40].copy_from_slice(&seconds.to_le_bytes());
        b[0x40..0x44].copy_from_slice(&20200u32.to_le_bytes());
        b[0x44] = 1;
        b[0x48] = 40;
        b[PARTY_AT..PARTY_AT + 20].copy_from_slice(&[0, 3, 2, 3, 3, 3, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff]);
        b
    }

    fn temp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("barrel-saves-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn a_header_reads_what_the_screen_shows() {
        let h = Header::parse(&file(0x01dd_73e5_52d0_5bf4, 493)).unwrap();
        assert_eq!((h.money, h.alchemy_level, h.gold_coins), (20200, 1, 40));
        assert_eq!(h.party, vec![(0, 3), (2, 3), (3, 3)]);
        assert_eq!(h.play_time(), "000:08:13");
        assert!(Header::parse(&[0u8; 16]).is_none());
    }

    #[test]
    fn the_list_has_the_autosave_first_and_finds_the_users_folder() {
        let root = temp("list");
        let user = root.join("76561197960265729");
        for (n, s) in [("GameData01", 5), ("AutoSaveData", 9), ("GameData00", 7), ("SystemData", 1)] {
            std::fs::create_dir_all(user.join(n)).unwrap();
            std::fs::write(user.join(n).join("data.dat"), file(0x01dd_73e5_52d0_5bf4, s)).unwrap();
        }
        assert_eq!(user_folder(&root), Some(user.clone()));
        assert_eq!(user_folder(&user), Some(user.clone()));
        let names: Vec<String> = slots(&user).into_iter().map(|s| s.name).collect();
        assert_eq!(names, ["AutoSaveData", "GameData00", "GameData01"]);
        assert_eq!(find(Some(&root)), Some(user));
        assert_eq!(find(Some(&root.join("nothing"))), None);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn the_prefix_is_found_under_a_steam_library() {
        let lib = temp("prefix");
        let user = lib.join("steamapps/compatdata/3365030").join(PREFIX_SAVES).join("42");
        std::fs::create_dir_all(user.join("GameData00")).unwrap();
        std::fs::write(user.join("GameData00/data.dat"), file(0, 0)).unwrap();
        assert_eq!(in_libraries(&[lib.clone()]), Some(user));
        let _ = std::fs::remove_dir_all(lib);
    }
}
