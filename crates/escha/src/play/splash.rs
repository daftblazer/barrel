//! The boot's logo splashes before the title: Koei Tecmo's, then Gust's, each a full-screen still
//! faded in from black, held and faded out again (`docs/formats.md`, the still table).
//!
//! Order and times as the escha-boot recording (2026-11-14, 60 Hz): Koei Tecmo fading in from
//! black at 6.08 s, full at 6.62, fading out from 8.62, black at 9.10 as Gust starts fading in
//! (no gap), Gust full 9.63 to 11.62, black at 12.10. Before them the game shows its Loading
//! plate while it reads its data (Barrel needs none); after them come the pre-title movie
//! (`Movie_EN/avantitle.wmv`, not played yet: `docs/escha/open.md`) and the title.

use std::path::Path;
use std::time::Instant;

use ash::vk;

use crate::screen::Target;
use crate::title::Button;
use crate::{text, Gpu, Result};

/// The stills shown, in order: 5 `logo_kt`, 4 `logo_gust`.
const LOGOS: [u32; 2] = [5, 4];
/// Seconds of each logo's fade in, hold and fade out.
const FADE_IN: f32 = 0.5;
const HOLD: f32 = 2.0;
const FADE_OUT: f32 = 0.5;
const EACH: f32 = FADE_IN + HOLD + FADE_OUT;
/// Black between the last logo and the pre-title movie (the recording: 12.10 to 12.25 s).
const MOVIE_GAP: f32 = 0.15;
/// The pre-title movie, played in the slot after the logos.
pub const INTRO_MOVIE: &str = "avantitle.wmv";

/// Which logo shows and how bright (0 black to 255 full), at `t` seconds; `None` once all have
/// shown and the black before the movie has passed.
pub fn at(t: f32) -> Option<(usize, u8)> {
    let logo = (t.max(0.0) / EACH) as usize;
    if logo >= LOGOS.len() {
        return (t < EACH * LOGOS.len() as f32 + MOVIE_GAP).then_some((LOGOS.len() - 1, 0));
    }
    let s = t - logo as f32 * EACH;
    let level = if s < FADE_IN { s / FADE_IN } else if s < FADE_IN + HOLD { 1.0 } else { (EACH - s) / FADE_OUT };
    Some((logo, (level.clamp(0.0, 1.0) * 255.0).round() as u8))
}

/// Shows the splashes: in a window until they end (a button skips the rest; whether the game
/// allows that is open: the recording pressed nothing), offscreen the frame at `shown_at` seconds, written to `out`.
#[allow(clippy::too_many_arguments)]
pub fn run(
    gpu: &Gpu,
    pool: vk::CommandPool,
    target: &Target,
    window: Option<(&crate::present::Presenter, &mut crate::window::Pump)>,
    offscreen: Option<vk::Image>,
    root: &Path,
    shown_at: f32,
    out: Option<&Path>,
) -> Result<()> {
    let exe = std::fs::read(root.join("Atelier_Escha_and_Logy_EN.exe"))?;
    let pe = barrel_formats::pe::Pe::parse(&exe)?;
    let x64 = root.join("Data/x64");
    let mut files = Vec::new();
    for n in LOGOS {
        let still = barrel_escha::data::layout::still(&pe, n).ok_or("splash: no logo still")?;
        let bytes = std::fs::read(x64.join(format!("{}_EN", still.folder)).join(&still.file)).or_else(|_| std::fs::read(x64.join(&still.folder).join(&still.file)))?;
        files.push((still, bytes));
    }
    let packs = files.iter().map(|(still, bytes)| Ok((still, barrel_formats::g1t::G1t::parse(bytes)?))).collect::<Result<Vec<_>>>()?;
    let mut images = Vec::new();
    for (_, g) in &packs {
        images.push(g.textures.first().ok_or("splash: an empty logo")?);
    }
    let (w, h) = (target.width as f32, target.height as f32);
    let quads = |s: &Option<(usize, u8)>| {
        let Some((logo, level)) = *s else { return Vec::new() };
        let (still, t) = (packs[logo].0, images[logo]);
        // Over the 1920 × 1080 screen, scaled to the height and centred, as the title's.
        let k = h / 1080.0;
        let dx = (w - 1920.0 * k) / 2.0;
        let v = f32::from(level) / 255.0;
        vec![text::Quad {
            turn: false,
            rect: [dx, 0.0, dx + still.size[0] * k, still.size[1] * k],
            uv: [0.0, 0.0, still.size[0] / t.width as f32, still.size[1] / t.height as f32],
            color: [v, v, v, 1.0],
            texture: logo,
        }]
    };
    let mut state = at(shown_at);
    let start = Instant::now();
    crate::screen::run_with(gpu, pool, target, window, offscreen, &images, &mut state, quads, |_, _: Button| Some(()), &[], out, |s| {
        *s = at(start.elapsed().as_secs_f32());
        s.is_none().then_some(())
    })?;
    intro_movie(root);
    Ok(())
}

/// The movie slot between the logos and the title: the game plays `Movie_EN/avantitle.wmv`
/// (96 s, skippable as the game allows) there. Stand-in until the VC-1 and WMA Pro decoders
/// land (stage 18): it is named in the log and skipped.
fn intro_movie(root: &Path) {
    let x64 = root.join("Data/x64");
    let file = [x64.join("Movie_EN").join(INTRO_MOVIE), x64.join("Movie").join(INTRO_MOVIE)].into_iter().find(|p| p.exists());
    match file {
        Some(p) => eprintln!("splash: intro movie {} not played yet (no VC-1 decoder)", p.display()),
        None => eprintln!("splash: no intro movie {INTRO_MOVIE}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fades_and_ends() {
        assert_eq!(at(0.0), Some((0, 0)));
        assert_eq!(at(FADE_IN), Some((0, 255)));
        assert_eq!(at(EACH - FADE_OUT / 2.0), Some((0, 128)));
        assert_eq!(at(EACH + 1.0).map(|(l, _)| l), Some(1));
        assert_eq!(at(2.0 * EACH + 0.01), Some((1, 0)));
        assert_eq!(at(2.0 * EACH + MOVIE_GAP + 0.01), None);
    }
}
