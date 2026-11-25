//! The popups of the arrival after the opening: the story marquee (`fm_marquee` of
//! `uil_a21_fm01.xml`: a banner that slides in from above with its text, `"<title>" added to
//! Story.`, over a stencil mask the text is drawn inside) and the autosave mark (`loading_1` of
//! `uil_a21_autosave.xml`: the saving spinner at the top left). Both are the game's reaction to
//! the opening's end: the story moves on and the game saves. Barrel never writes the original's
//! saves, so only the marks are drawn (docs/engine.md, "Deliberate differences").

use barrel_formats::table::{self, Element};
use barrel_render::text::Clip;

use super::{label, picture, Hud, Label, Pen, Sources, BATTLE, FIELD, SAVELOAD};
use crate::boot::{nine_slice, pair, style, Track, FPS};
use crate::spinner::Spinner;

/// The `STR_TELOP` line that announces a story entry (`"<STR1>" added to Story.`) and the entry's
/// title (the story summary's first, `str_summary_title`).
pub(super) const ADDED_LINE: u32 = 196_612;
// STAND-IN: that the entry shown is the first summary title is read off `opening-key4` (the text of 14955 decoded against the font table); the game takes it from the story's own state at the opening's end.
pub(super) const STORY_TITLE: u32 = 6_160_385;

// STAND-IN: the clocks below are read off `opening-key4`'s every frame 14862 to 15219 (the field's first frame 14862): the marquee's slide-in clock is 2 frames on the field's, it holds until its 329th tick and slides out the same animation backwards; the spinner's fade-in clock is 1 frame on, it holds to its 166th tick and fades out by `anm_common_fade_out`. The game's own timers (a save's length) are not read.
/// The marquee's first tick falls this many frames before the field's first frame.
const MARQUEE_LEAD: f32 = 2.0;
/// The tick the marquee's slide runs backwards from (its value there is the full animation's).
const MARQUEE_OUT_AT: f32 = 329.0;
/// The spinner's first tick falls this many frames before the field's first frame.
const SPINNER_LEAD: f32 = 1.0;
/// The tick the spinner starts to fade out at.
const SPINNER_OUT_AT: f32 = 166.0;

// STAND-IN: the text sits 2 layout pixels left of where its centred node puts it (every glyph of `opening-key4` 14955 is 3 pixels left of the layout's rule, the same for the whole line); the game's own text-width measure is not read.
/// How far left of its node's centre the marquee's text is drawn, in layout pixels.
const TEXT_SHIFT: f32 = 2.0;

/// The marquee's text scrolls in from the right at 20 layout pixels a frame (30 pixels of the
/// capture's 1080 lines), rests, and scrolls out to the left; read off `opening-key4` 14898 to
/// 15189: the first glyph is at its resting place 835 layout pixels further right on the
/// field's frame 35 (14897) and the scroll ends on frame 77; it leaves from frame 290 for 37
/// frames and holds, and the text is gone from frame 328 (the banner's own slide out starts).
const TEXT_SPEED: f32 = 20.0;
const TEXT_IN_OFFSET: f32 = 835.0;
const TEXT_IN_AGE: f32 = 35.0;
const TEXT_OUT_AGE: f32 = 289.0;
const TEXT_OUT_FRAMES: f32 = 37.0;
const TEXT_END_AGE: f32 = 328.0;

/// How far right of its resting place the marquee's text is `age` frames after the field's
/// first, in layout pixels; `None` once it is gone.
fn text_shift(age: f32) -> Option<f32> {
    if age >= TEXT_END_AGE {
        return None;
    }
    Some(if age <= TEXT_OUT_AGE { (TEXT_IN_OFFSET - TEXT_SPEED * (age - TEXT_IN_AGE)).max(0.0) } else { -TEXT_SPEED * (age - TEXT_OUT_AGE).min(TEXT_OUT_FRAMES) })
}

/// The layouts' pieces the arrival draws.
pub(super) struct Arrival {
    /// The marquee's plate (its picture and place when it has slid in) and its text node.
    back: ([f32; 4], [f32; 2]),
    /// The mask's pieces: each picture's place in the atlas and its offset and size in the node.
    mask: Vec<([f32; 4], [f32; 4])>,
    mask_at: [f32; 2],
    text: Label,
    /// `anm_common_slide_in_top`'s opacity and place.
    slide_color: Track,
    slide_pos: Track,
    fade_in: Track,
    fade_out: Track,
    spinner: Spinner,
}

impl Arrival {
    pub(super) fn read(src: &Sources, places: &Element, field: &Element, atlases: &[(f32, f32)]) -> Result<Arrival, String> {
        let battle = table::parse(src.battle_styles).map_err(|e| format!("uis_gen_a21_battle: {e}"))?;
        let anims = table::parse(src.anims).map_err(|e| format!("uia_common: {e}"))?;
        let saveload_anims = table::parse(src.saveload_anims).map_err(|e| format!("uia_a21_saveload: {e}"))?;
        let saveload_styles = table::parse(src.spinner_styles).map_err(|e| format!("uis_gen_a21v2_saveload: {e}"))?;
        let autosave = table::parse(src.autosave).map_err(|e| format!("uil_a21_autosave: {e}"))?;
        let find = |name: &str| crate::title::find(places, name, [0.0; 2]).ok_or(format!("uil_a21_fm01: no {name}"));
        let (back, back_at) = find("back")?;
        let (mask, mask_at) = find("_mask")?;
        let (text, text_at) = find("log")?;
        let mask_style = style(&battle, mask.attr("style").ok_or("_mask has no style")?)?;
        let mut l = label(text, text_at);
        let rgba = crate::title::numbers(text, "font_color");
        l.color = (rgba.len() == 4).then(|| [rgba[0] / 255.0, rgba[1] / 255.0, rgba[2] / 255.0, rgba[3] / 255.0]);
        let fm = |e: &Element| e.attr("style").map(str::to_string).ok_or_else(|| "a picture without a style".to_string());
        Ok(Arrival {
            back: (picture(field, &fm(back)?)?, back_at),
            mask: nine_slice(mask_style.pictures[0], mask_style.margins.ok_or("the mask is not stretched")?, pair(mask, "size_wh", [0.0; 2])),
            mask_at,
            text: l,
            slide_color: Track::read(&anims, "anm_common_slide_in_top", "color")?,
            slide_pos: Track::read(&anims, "anm_common_slide_in_top", "pos")?,
            fade_in: Track::read(&anims, "anm_common_fade_in", "color")?,
            fade_out: Track::read(&anims, "anm_common_fade_out", "color")?,
            spinner: Spinner::read(&autosave, &saveload_styles, &saveload_anims, atlases[SAVELOAD - FIELD], SAVELOAD)?,
        })
    }

    /// How far into its slide the marquee is `age` frames after the field's first, and so its
    /// opacity (a whole 1/255 as the game's colour byte) and how far it is above its place in
    /// layout pixels; `None` before it starts and after it is gone.
    fn marquee(&self, age: f32) -> Option<(f32, f32)> {
        let tick = age + MARQUEE_LEAD;
        let along = if tick <= MARQUEE_OUT_AT { tick } else { self.slide_color.duration() * FPS - (tick - MARQUEE_OUT_AT) };
        if tick < 0.0 || along <= 0.0 {
            return None;
        }
        let alpha = self.slide_color.at(along / FPS)[3].round() / 255.0;
        (alpha > 0.0).then(|| (alpha, self.slide_pos.at(along / FPS)[1]))
    }

    /// The spinner's opacity as a byte, `age` frames after the field's first.
    fn spinner_alpha(&self, age: f32) -> u32 {
        let tick = age + SPINNER_LEAD;
        let value = if tick < SPINNER_OUT_AT { self.fade_in.at(tick / FPS)[3] } else { self.fade_out.at((tick - SPINNER_OUT_AT) / FPS)[3] };
        if tick < 0.0 { 0 } else { value.round() as u32 }
    }
}

impl Hud {
    /// The marquee's quads (the plate, the mask's pieces, the text), then the clips that keep
    /// the mask's pieces out of the picture.
    pub(super) fn marquee(&self, pen: &mut Pen, age: f32, clips: &mut Vec<Clip>, title: &str) {
        let Some(a) = &self.arrival else { return };
        let Some((alpha, slide)) = a.marquee(age) else { return };
        let color = [1.0, 1.0, 1.0, alpha];
        pen.picture(FIELD, a.back.0, [a.back.1[0], a.back.1[1] + slide], 1.0, color);
        for (uvwh, [x, y, w, h]) in &a.mask {
            let [l, t] = pen.at([a.mask_at[0] + x, a.mask_at[1] + slide + y]);
            let (w, h) = (w * pen.k, h * pen.k);
            clips.push(Clip { mask: pen.out.len(), quads: 0..0 });
            let uv = pen.uv(BATTLE, *uvwh);
            pen.quad(BATTLE, uv, [l, t, l + w, t + h], color);
        }
        let Some(shift) = text_shift(age) else { return };
        let text = self.word(ADDED_LINE).replace("<STR1>", title);
        let ink = a.text.color.unwrap_or([0.0, 0.0, 0.0, 1.0]);
        let mut at = a.text;
        at.at = [at.at[0] - TEXT_SHIFT + shift, at.at[1] + slide];
        let first = pen.out.len();
        pen.text(&at, [0.0; 2], &text, [ink[0], ink[1], ink[2], alpha]);
        // The game draws no glyph that falls wholly off the screen (the text comes in from the
        // right and goes out at the left).
        let (from, to) = (pen.left, pen.left + 1280.0 * pen.k);
        let mut at = first;
        pen.out.retain(|q| {
            at += 1;
            at <= first || (q.rect[0] < to && q.rect[2] > from)
        });
    }

    /// The autosave mark's quads.
    pub(super) fn autosave_mark(&self, pen: &mut Pen, age: f32) {
        let Some(a) = &self.arrival else { return };
        let base = a.spinner_alpha(age);
        if base == 0 {
            return;
        }
        pen.out.extend(a.spinner.quads((age + SPINNER_LEAD) / FPS, pen.k, pen.left, base, 0.0));
    }
}
