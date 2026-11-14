//! Barrel's save slots: a folder of `slotNN.save` files in Barrel's own text format
//! (`session::Session::save`), numbered from 1 as the game numbers its "Game Data" slots. The
//! folder is Barrel's own (by default `$XDG_DATA_HOME/barrel/saves`), never the game's.

use std::path::{Path, PathBuf};

/// What a slot's list entry shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Slot {
    pub number: u32,
    pub path: PathBuf,
    /// Logy leads.
    pub logy: bool,
    /// The story's day number (`calendar::Date::of`).
    pub today: i32,
    /// When it was written, in seconds since 1970 (UTC).
    pub written: Option<u64>,
}

/// The default saves folder.
pub fn default_dir() -> Option<PathBuf> {
    let data = std::env::var_os("XDG_DATA_HOME").map(PathBuf::from).or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))?;
    Some(data.join("barrel/saves"))
}

/// Slot `n`'s file in `dir`.
pub fn slot_path(dir: &Path, n: u32) -> PathBuf {
    dir.join(format!("slot{n:02}.save"))
}

/// The lead and day a save's text holds, without loading it.
pub fn peek(text: &str) -> Option<(bool, i32)> {
    let mut lines = text.lines();
    (lines.next()? == "barrel-save 1").then_some(())?;
    let (mut logy, mut today) = (false, None);
    for l in lines {
        let mut w = l.split_whitespace();
        match w.next() {
            Some("lead") => logy = w.next() == Some("logy"),
            Some("today") => today = w.next()?.parse().ok(),
            _ => {}
        }
    }
    Some((logy, today?))
}

/// The saves in `dir`, by slot number (unreadable files and other names left out).
pub fn list(dir: &Path) -> Vec<Slot> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else { return out };
    for e in entries.flatten() {
        let name = e.file_name();
        let Some(n) = name.to_str().and_then(|n| n.strip_prefix("slot")?.strip_suffix(".save")?.parse::<u32>().ok()) else { continue };
        let Ok(text) = std::fs::read_to_string(e.path()) else { continue };
        let Some((logy, today)) = peek(&text) else { continue };
        let written = e.metadata().ok().and_then(|m| m.modified().ok()).and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs());
        out.push(Slot { number: n, path: e.path(), logy, today, written });
    }
    out.sort_by_key(|s| s.number);
    out
}

/// A time in seconds since 1970 as the game's list writes it, "YYYY/MM/DD HH:MM:SS" (UTC).
pub fn stamp(secs: u64) -> String {
    let (days, rest) = (secs / 86400, secs % 86400);
    // Civil date from days since 1970 (Howard Hinnant's algorithm).
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}/{m:02}/{d:02} {:02}:{:02}:{:02}", rest / 3600, rest / 60 % 60, rest % 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn peeks_and_stamps() {
        assert_eq!(peek("barrel-save 1\nlead logy\nfield 3 1 2\ntoday 95\n"), Some((true, 95)));
        assert_eq!(peek("something else"), None);
        assert_eq!(stamp(0), "1970/01/01 00:00:00");
        assert_eq!(stamp(1_763_000_000), "2025/11/13 02:13:20");
    }
}
