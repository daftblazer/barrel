//! Which game to start, for the one `barrel` program that runs them all. In order:
//!
//! 1. `--game DIR`: the folder's contents say which game it is;
//! 2. a path on the command line inside an install: Steam's `%command%` (on Windows the game's
//!    `.exe`; with Proton `.../proton waitforexitandrun .../Atelier_*.exe`), Kintsugi's
//!    `--title FOLDER`, a developer's `--field DIR`. Steam's own arguments are then dropped, so
//!    nothing it adds can be an error (arguments left that are no `--flag` are dropped too);
//! 3. the `SteamAppId` (or `SteamGameId`) variable Steam sets, with that game's install from the
//!    config's saved folder or the Steam search;
//! 4. `BARREL_GAME_DIR`;
//! 5. the games the config saved (`games.toml`) and Steam's libraries hold: exactly one starts,
//!    several ask the user (the picker), none the "Locate game files" screen.

use std::path::{Path, PathBuf};

use crate::find::Title;

/// What [`detect`] decided.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Detected {
    /// Start this game from this install.
    One(Title, PathBuf),
    /// Nothing above chose; the user picks one of these.
    Several(Vec<(Title, PathBuf)>),
    /// No game found anywhere.
    None,
}

/// The places a game's install may come from besides the command line.
pub struct Sources<'a> {
    /// `SteamAppId`, else `SteamGameId`.
    pub app_id: Option<u32>,
    /// `BARREL_GAME_DIR`.
    pub env_dir: Option<PathBuf>,
    /// The folder saved in the config for a game.
    pub saved: &'a dyn Fn(Title) -> Option<PathBuf>,
    /// The game's install in Steam's libraries.
    pub steam: &'a dyn Fn(Title) -> Option<PathBuf>,
}

impl Sources<'_> {
    /// The real environment, config and Steam.
    pub fn system() -> Sources<'static> {
        let id = |k: &str| std::env::var(k).ok().and_then(|v| v.trim().parse().ok());
        Sources {
            app_id: id("SteamAppId").or_else(|| id("SteamGameId")),
            env_dir: std::env::var_os("BARREL_GAME_DIR").filter(|v| !v.is_empty()).map(PathBuf::from),
            saved: &|t| crate::config::saved_game(t.key()),
            steam: &crate::find::in_steam,
        }
    }

    /// `title`'s install: the saved folder when it still holds the game, else Steam's.
    fn install(&self, title: Title) -> Option<PathBuf> {
        (self.saved)(title).filter(|d| crate::find::is_install(title, d)).or_else(|| (self.steam)(title))
    }
}

/// Removes `flag VALUE` from `args` and gives the value.
pub fn take_option(args: &mut Vec<String>, flag: &str) -> Option<String> {
    let at = args.iter().position(|a| a == flag)?;
    let value = args.get(at + 1).cloned();
    args.drain(at..(at + 2).min(args.len()));
    value
}

/// The install an argument's path lies in (itself or a folder above it).
fn install_around(arg: &str) -> Option<(Title, PathBuf)> {
    // A Windows path on any host: both slashes end a folder name.
    if !arg.contains(['/', '\\']) {
        return None;
    }
    let mut dir = PathBuf::from(arg);
    loop {
        if let Some(t) = Title::of_install(&dir) {
            return Some((t, dir));
        }
        if !dir.pop() || dir.as_os_str().is_empty() {
            return None;
        }
    }
}

/// The game and its install, and the arguments left for the game's own command line.
pub fn detect(mut args: Vec<String>, src: &Sources) -> Result<(Detected, Vec<String>), String> {
    if let Some(dir) = take_option(&mut args, "--game") {
        let dir = PathBuf::from(dir);
        let title = Title::of_install(&dir).ok_or_else(|| format!("{}: not a folder with a game Barrel plays (no {} or {})", dir.display(), Title::Escha.marker(), Title::Ryza.marker()))?;
        return Ok((Detected::One(title, dir), args));
    }
    // Steam's `%command%`: the game's own program among the arguments.
    for a in &args {
        if a.to_ascii_lowercase().ends_with(".exe")
            && let Some(dir) = a.rsplit_once(['/', '\\']).map(|(d, _)| Path::new(d)).filter(|d| !d.as_os_str().is_empty())
            && let Some(title) = Title::of_install(dir)
        {
            return Ok((Detected::One(title, dir.to_path_buf()), Vec::new()));
        }
    }
    if let Some((title, dir)) = args.iter().find_map(|a| install_around(a)) {
        return Ok((Detected::One(title, dir), args));
    }
    // Nothing in the arguments is a path of ours: what Steam or a launcher added is ignored,
    // unless it is a flag for the game.
    if !args.iter().any(|a| a.starts_with("--")) {
        args.clear();
    }
    if let Some(title) = src.app_id.and_then(|id| Title::ALL.into_iter().find(|t| t.app_id() == id))
        && let Some(dir) = src.install(title)
    {
        return Ok((Detected::One(title, dir), args));
    }
    if let Some(dir) = &src.env_dir {
        let title = Title::of_install(dir).ok_or_else(|| format!("BARREL_GAME_DIR {}: not a folder with a game Barrel plays", dir.display()))?;
        return Ok((Detected::One(title, dir.clone()), args));
    }
    let mut found: Vec<(Title, PathBuf)> = Title::ALL.into_iter().filter_map(|t| src.install(t).map(|d| (t, d))).collect();
    let detected = match found.len() {
        0 => Detected::None,
        1 => {
            let (t, d) = found.remove(0);
            Detected::One(t, d)
        }
        _ => Detected::Several(found),
    };
    Ok((detected, args))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fs::tests::Scratch;

    /// A scratch folder with an Escha install and a Ryza one.
    fn installs(tag: &str) -> (Scratch, PathBuf, PathBuf) {
        let root = Scratch::new(tag);
        root.file("e/Atelier_Escha_and_Logy_EN.exe", b"MZ").file("e/Data/a", b"");
        root.file("r/Atelier_Ryza_DX.exe", b"MZ").file("r/Data/a", b"");
        let (e, r) = (root.0.join("e"), root.0.join("r"));
        (root, e, r)
    }

    fn strings(a: &[&str]) -> Vec<String> {
        a.iter().map(|s| s.to_string()).collect()
    }

    fn sources<'a>(app_id: Option<u32>, env_dir: Option<PathBuf>, steam: &'a dyn Fn(Title) -> Option<PathBuf>) -> Sources<'a> {
        Sources { app_id, env_dir, saved: &|_| None, steam }
    }

    #[test]
    fn the_game_option_names_the_game_by_its_folder() {
        let (_root, e, r) = installs("detect-game");
        let none = |_: Title| None;
        let (d, rest) = detect(strings(&["--game", r.to_str().unwrap(), "--window"]), &sources(None, None, &none)).unwrap();
        assert_eq!(d, Detected::One(Title::Ryza, r.clone()));
        assert_eq!(rest, ["--window"]);
        let (d, _) = detect(strings(&["--game", e.to_str().unwrap()]), &sources(None, None, &none)).unwrap();
        assert_eq!(d, Detected::One(Title::Escha, e));
        assert!(detect(strings(&["--game", "/no/such"]), &sources(None, None, &none)).is_err());
    }

    #[test]
    fn steams_command_gives_the_game_and_drops_its_arguments() {
        let (_root, e, r) = installs("detect-exe");
        let none = |_: Title| None;
        // Windows: the game's program.
        let exe = format!("{}/Atelier_Ryza_DX.exe", r.display());
        let (d, rest) = detect(strings(&[&exe]), &sources(None, None, &none)).unwrap();
        assert_eq!((d, rest.len()), (Detected::One(Title::Ryza, r), 0));
        // Linux with Proton: its own program and verb come first, the game's arguments last.
        let exe = format!("{}/Atelier_Escha_and_Logy_EN.exe", e.display());
        let (d, rest) = detect(strings(&["/steam/proton/proton", "waitforexitandrun", &exe, "-foo"]), &sources(None, None, &none)).unwrap();
        assert_eq!((d, rest.len()), (Detected::One(Title::Escha, e), 0));
    }

    #[test]
    fn unknown_arguments_without_a_flag_are_ignored() {
        let (_root, e, _r) = installs("detect-ignore");
        let e2 = e.clone();
        let steam = move |_: Title| Some(e2.clone());
        let (d, rest) = detect(strings(&["/some/proton", "waitforexitandrun"]), &sources(Some(1152310), None, &steam)).unwrap();
        assert_eq!((d, rest.len()), (Detected::One(Title::Escha, e), 0));
        let (_, rest) = detect(strings(&["--window", "0"]), &sources(Some(1152310), None, &steam)).unwrap();
        assert_eq!(rest, ["--window", "0"]);
    }

    #[test]
    fn a_path_inside_an_install_names_the_game_and_the_arguments_stay() {
        let (_root, e, _r) = installs("detect-path");
        let none = |_: Title| None;
        let field = format!("{}/Data/x64/Field/TOWN_04", e.display());
        let args = strings(&["--field", &field, "out.png", "--size", "1920x1080"]);
        let (d, rest) = detect(args.clone(), &sources(None, None, &none)).unwrap();
        assert_eq!(d, Detected::One(Title::Escha, e));
        assert_eq!(rest, args);
    }

    #[test]
    fn steams_app_id_picks_the_game_even_with_two_installed() {
        let (_root, e, r) = installs("detect-appid");
        let (e2, r2) = (e.clone(), r.clone());
        let steam = move |t: Title| Some(if t == Title::Escha { e2.clone() } else { r2.clone() });
        let (d, _) = detect(Vec::new(), &sources(Some(3365030), None, &steam)).unwrap();
        assert_eq!(d, Detected::One(Title::Ryza, r));
        let (d, _) = detect(Vec::new(), &sources(Some(1152310), None, &steam)).unwrap();
        assert_eq!(d, Detected::One(Title::Escha, e));
        // An unknown app id decides nothing: both games found, so the user picks.
        let (d, _) = detect(Vec::new(), &sources(Some(7), None, &steam)).unwrap();
        assert!(matches!(d, Detected::Several(v) if v.len() == 2));
    }

    #[test]
    fn the_environment_folder_comes_before_the_search() {
        let (_root, e, r) = installs("detect-env");
        let r2 = r.clone();
        let steam = move |_: Title| Some(r2.clone());
        let (d, _) = detect(Vec::new(), &sources(None, Some(e.clone()), &steam)).unwrap();
        assert_eq!(d, Detected::One(Title::Escha, e));
        assert!(detect(Vec::new(), &sources(None, Some("/no/such".into()), &steam)).is_err());
    }

    #[test]
    fn one_game_found_starts_several_ask_none_locates() {
        let (_root, e, r) = installs("detect-search");
        let e2 = e.clone();
        let only_escha = move |t: Title| (t == Title::Escha).then(|| e2.clone());
        assert_eq!(detect(Vec::new(), &sources(None, None, &only_escha)).unwrap().0, Detected::One(Title::Escha, e.clone()));
        let (e3, r3) = (e.clone(), r.clone());
        let both = move |t: Title| Some(if t == Title::Escha { e3.clone() } else { r3.clone() });
        assert_eq!(detect(Vec::new(), &sources(None, None, &both)).unwrap().0, Detected::Several(vec![(Title::Escha, e), (Title::Ryza, r)]));
        let nothing = |_: Title| None;
        assert_eq!(detect(Vec::new(), &sources(None, None, &nothing)).unwrap().0, Detected::None);
    }

    #[test]
    fn a_saved_folder_stands_before_steams() {
        let (_root, e, r) = installs("detect-saved");
        let (e2, r2) = (e.clone(), r.clone());
        let saved = move |t: Title| (t == Title::Escha).then(|| e2.clone());
        let steam = move |t: Title| (t == Title::Ryza).then(|| r2.clone());
        let src = Sources { app_id: None, env_dir: None, saved: &saved, steam: &steam };
        assert_eq!(detect(Vec::new(), &src).unwrap().0, Detected::Several(vec![(Title::Escha, e), (Title::Ryza, r)]));
    }
}
