//! A playable character's field motions: `\data\x64\character\<set>\<set>_<NNNNN>.elixir.gz`
//! (one `.g1a` each), the set from `saves\chara\charamotionset.xml` (Ryza: `PC00A`). The
//! numbers come in threes, start, loop and stop; the motions stay in place (the root bone
//! does not travel), so how fast she moves is the game's, not the motion's. Checked by posing
//! Ryza's (docs/formats.md, "Ryza's motions").

/// The idle stand (2.52 s, looping).
pub const STAND: u32 = 0;
/// Walking: start (0.63 s), the loop (1.35 s), stop (0.67 s).
pub const WALK_START: u32 = 10;
pub const WALK: u32 = 11;
pub const WALK_STOP: u32 = 12;
/// Running: start (0.25 s), the loop (0.75 s), stop (0.65 s).
pub const RUN_START: u32 = 20;
pub const RUN: u32 = 21;
pub const RUN_STOP: u32 = 22;

/// A motion's container in the archives.
pub fn path(set: &str, motion: u32) -> String {
    let set = set.to_ascii_lowercase();
    format!("\\data\\x64\\character\\{set}\\{set}_{motion:05}.elixir.gz")
}

#[cfg(test)]
mod tests {
    #[test]
    fn motion_paths() {
        assert_eq!(super::path("PC00A", super::RUN), "\\data\\x64\\character\\pc00a\\pc00a_00021.elixir.gz");
    }
}
