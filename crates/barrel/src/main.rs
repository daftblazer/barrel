//! `barrel`: the one program that runs every game Barrel supports. It works out which game to
//! start (`barrel_files::detect`: `--game DIR`, a path or Steam's `%command%` among the arguments,
//! Steam's app id, `BARREL_GAME_DIR`, the saved and Steam folders), asks when several games are
//! installed (the picker) or where the game is when none is found (the "Locate game files"
//! screen), and hands over to the game's own `run` with the arguments left. The games' own
//! programs (`barrel-escha`, `barrel-ryza`) take the same developer flags without the choice.
//!
//! `BARREL_LOCATE_SHOT=out.png` writes the two screens as pictures (`out.png`, `out-pick.png`)
//! and exits, for checking how they look.

use std::path::Path;

use barrel_files::detect::{self, Detected, Sources};
use barrel_files::find::Title;
use barrel_render::{locate, Result};

fn main() -> Result<()> {
    if let Some(out) = std::env::var_os("BARREL_LOCATE_SHOT") {
        locate::shot(Path::new(&out), barrel_files::browse::home())?;
        return Ok(());
    }
    let (detected, args) = match detect::detect(std::env::args().skip(1).collect(), &Sources::system()) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("barrel: {e}");
            std::process::exit(2);
        }
    };
    let (title, root, pump) = match detected {
        Detected::One(title, root) => (title, root, None),
        Detected::Several(games) => {
            let Some((i, pump)) = locate::pick(&games, None, locate::config_display())? else { return Ok(()) };
            let (title, root) = games[i].clone();
            (title, root, Some(pump))
        }
        Detected::None => {
            let Some((dir, pump)) = locate::run(&Title::ALL, None, locate::config_display())? else { return Ok(()) };
            let title = Title::of_install(&dir).ok_or("the chosen folder holds no game")?;
            locate::remember(title, &dir);
            (title, dir, Some(pump))
        }
    };
    let chosen = Some(locate::start(title, root, pump));
    match title {
        Title::Escha => barrel_escha::play::run(args, chosen),
        Title::Ryza => barrel_ryza::play::run(args, chosen),
    }
}
