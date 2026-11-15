//! The folder list of the "Locate game files" screen: the folders of one directory, stepped with
//! up and down, entered with confirm, left with back. Confirming a folder that holds the game
//! chooses it. No drawing here (the game's art cannot be loaded before the game is found): the
//! screen reads [`Browser::dir`], [`Browser::rows`] and [`Browser::selected`].

use std::path::PathBuf;

use crate::find::Title;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Row {
    /// The folder above.
    Up,
    /// A folder inside; `game` when it holds the game's files.
    Folder { name: String, game: bool },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Input {
    Up,
    Down,
    /// A page up or down the list.
    PageUp,
    PageDown,
    Confirm,
    Back,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Browser {
    /// The games whose folder the list looks for.
    pub titles: Vec<Title>,
    pub dir: PathBuf,
    pub rows: Vec<Row>,
    pub selected: usize,
    /// A line for the screen when something went wrong (a folder that cannot be read).
    pub message: String,
}

/// How far a page step moves.
const PAGE: usize = 8;

impl Browser {
    /// The list of `start`, or of the user's home when that cannot be read.
    pub fn new(title: Title, start: PathBuf) -> Browser {
        Browser::for_games(vec![title], start)
    }

    /// The list of `start`, marking the folders of any of `titles`.
    pub fn for_games(titles: Vec<Title>, start: PathBuf) -> Browser {
        let mut b = Browser { titles, dir: start, rows: Vec::new(), selected: 0, message: String::new() };
        if b.fill().is_err() {
            b.dir = home();
            if b.fill().is_err() {
                b.dir = PathBuf::from(std::path::MAIN_SEPARATOR_STR);
                let _ = b.fill();
            }
        }
        b
    }

    fn fill(&mut self) -> std::io::Result<()> {
        let mut folders: Vec<(String, bool)> = std::fs::read_dir(&self.dir)?
            .flatten()
            .filter(|e| e.path().is_dir())
            .filter_map(|e| e.file_name().into_string().ok())
            .filter(|n| !n.starts_with('.'))
            .map(|n| {
                let game = Title::of_install(&self.dir.join(&n)).is_some_and(|t| self.titles.contains(&t));
                (n, game)
            })
            .collect();
        folders.sort_by_key(|(n, _)| n.to_lowercase());
        self.rows = self.dir.parent().map(|_| Row::Up).into_iter().chain(folders.into_iter().map(|(name, game)| Row::Folder { name, game })).collect();
        self.selected = 0;
        self.message.clear();
        // A game folder here is where the cursor starts.
        if let Some(i) = self.rows.iter().position(|r| matches!(r, Row::Folder { game: true, .. })) {
            self.selected = i;
        }
        Ok(())
    }

    fn go(&mut self, dir: PathBuf, select: Option<&str>) {
        let before = std::mem::replace(&mut self.dir, dir);
        if let Err(e) = self.fill() {
            self.dir = before;
            self.message = format!("cannot open that folder: {e}");
            return;
        }
        if let Some(i) = select.and_then(|s| self.rows.iter().position(|r| matches!(r, Row::Folder { name, .. } if name == s))) {
            self.selected = i;
        }
    }

    /// Takes a button; the game's folder once one is chosen.
    pub fn press(&mut self, input: Input) -> Option<PathBuf> {
        let last = self.rows.len().saturating_sub(1);
        match input {
            Input::Up => self.selected = self.selected.checked_sub(1).unwrap_or(last),
            Input::Down => self.selected = if self.selected >= last { 0 } else { self.selected + 1 },
            Input::PageUp => self.selected = self.selected.saturating_sub(PAGE),
            Input::PageDown => self.selected = (self.selected + PAGE).min(last),
            Input::Back => {
                if let Some(parent) = self.dir.parent().map(|p| p.to_path_buf()) {
                    let from = self.dir.file_name().map(|n| n.to_string_lossy().into_owned());
                    self.go(parent, from.as_deref());
                }
            }
            Input::Confirm => match self.rows.get(self.selected).cloned() {
                Some(Row::Up) => return self.press(Input::Back),
                Some(Row::Folder { name, game }) => {
                    let dir = self.dir.join(name);
                    if game {
                        return Some(dir);
                    }
                    self.go(dir, None);
                }
                None => {}
            },
        }
        None
    }
}

/// The user's home folder (the list's start), else the root.
pub fn home() -> PathBuf {
    std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(PathBuf::from).unwrap_or_else(|| PathBuf::from(std::path::MAIN_SEPARATOR_STR))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fs::tests::Scratch;

    #[test]
    fn steps_into_folders_and_chooses_the_game() {
        let root = Scratch::new("browse");
        root.file("Games/Other/readme", b"x").file("Games/Atelier/Atelier_Escha_and_Logy_EN.exe", b"MZ").file("Games/Atelier/Data/a", b"");
        std::fs::create_dir_all(root.0.join(".hidden")).unwrap();
        let mut b = Browser::new(Title::Escha, root.0.clone());
        assert!(b.rows.contains(&Row::Up));
        assert!(!b.rows.iter().any(|r| matches!(r, Row::Folder { name, .. } if name.starts_with('.'))));
        // Into Games (the first folder), where the cursor rests on the game's folder.
        let games = b.rows.iter().position(|r| matches!(r, Row::Folder { name, .. } if name == "Games")).unwrap();
        b.selected = games;
        assert_eq!(b.press(Input::Confirm), None);
        assert_eq!(b.dir, root.0.join("Games"));
        assert!(matches!(&b.rows[b.selected], Row::Folder { name, game: true } if name == "Atelier"));
        // Down past the end wraps to the top (the ".." row); back out returns to the folder it left.
        assert_eq!(b.press(Input::Back), None);
        assert_eq!(b.dir, root.0);
        assert!(matches!(&b.rows[b.selected], Row::Folder { name, .. } if name == "Games"));
        b.press(Input::Confirm);
        assert_eq!(b.press(Input::Confirm), Some(root.0.join("Games/Atelier")));
    }
}
