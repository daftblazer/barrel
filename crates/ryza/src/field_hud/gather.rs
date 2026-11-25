//! The gathering's popups of the field HUD (`uil_a21_fm01.xml`): the prompt that slides up when
//! a gathering point is in reach (`fm_access_icon`: the A button and "Check"), and after A the
//! item's two notices that slide in from the left (`discovery_log`, "New Discovery" over the
//! item's picture and name, for an item the library did not know; `general_log`, the item's
//! small picture and name). Their animations are `uia_a21_fm01.xml`'s and `uia_common.xml`'s.
//! Docs: docs/ryza/open.md, "Gathering (P6)".

use barrel_formats::table::Element;

use super::{item_texture, label, node, picture, Hud, Label, Pen, Sources, FIELD};
use crate::title::{curve, numbers, pair, Curve};

/// The layout's nodes sit two layout pixels higher on the screen than their `pos` add up to
/// (the discovery log's plate: `pos` 121, drawn from 119).
// STAND-IN: read off `gather-rasen`; the source of the shift (a root node's place) is not found.
const ROOT_SHIFT: f32 = -2.0;
/// The string of the prompt's word, the first of the list of gimmick actions ("Check").
// STAND-IN: the capture shows "Check" on a hand-gathering point; which string the game takes for
// the point's type is not read.
pub(super) const PROMPT_WORD: u32 = 4_718_593;
/// The staff point's prompt: "Swing" (the next of the list of gimmick actions) with the X button's glyph (`\u{4dc}`, the one
/// the Swing option's help line carries: `STR_UI` 4194341).
pub(super) const SWING_WORD: u32 = 4_718_596;
const X_BUTTON: &str = "\u{4dc}";
/// The A button's glyph in `mainfont00`, which `<KEY_ASSIGN_FIELD_ACCESS>` (`STR_UI_0281`) becomes.
// STAND-IN: the glyph is the capture's (the game picks it for the pad's assignment).
const A_BUTTON: &str = "\u{4da}";

// STAND-IN: the popups' frames, counted from the frame A is pressed, are read off `gather-rasen`
// (A at 2040): the prompt starts sliding out on the press, the discovery log's fade-in starts 87
// frames after it and the item notice's a frame later (the sounds of 2125 and 2128), the
// notice's fade-out 141 frames after its start and the log's 211 after its. The game's own
// timers (the gathering motion's length, the notices' holds) are not read.
const NOTICE_HOLD: f32 = 141.0;
const DISCOVERY_HOLD: f32 = 211.0;
// STAND-INs read off `gather-rasen2` (A at 2500: a comb, two items): the second item's notice starts 22 frames after the
// first's (2588, 2610); the first notice moves up by the second's slot (66 screen pixels, 44 of the layout's) over the 21
// frames of the fade-in from the second's start (47.6 % of the way on 2620, 95.2 % on 2630); all the notices fade out
// together, the hold counted from the last one's start (the fade-out began on 2751 = 2610 + 141). A third item's gap is taken
// as the same.
const ITEM_GAP: f32 = 22.0;
/// How many frames further along its slide the notice's text is than its plate (STAND-IN: 2 layout pixels of the 9.5 a frame, from the one frame, `gather-rasen2` 3200, where it shows).
const TEXT_AHEAD: f32 = 0.21;
const NOTICE_STEP: f32 = 44.0;
const NOTICE_UP_FRAMES: f32 = 21.0;
/// The frames the prompt takes to slide in (`anm_common_slide_in_bottom`'s 30).
const SLIDE_FRAMES: f32 = 30.0;

/// The frames after the press the first notice starts, by hand and with the staff (the discovery log's a frame before).
// STAND-IN: read off the captures (`gather-rasen`: A at 2040, the notice on 2128; `gather-rasen2`: X at 3160, the notice on 3195
// by its sound's frame and the log's slide there, whose count came to 3194). The game's own timers are not read.
pub const HAND_NOTICE_AT: f32 = 88.0;
pub const STAFF_NOTICE_AT: f32 = 35.0;
pub const STAFF_LEAVE_AT: f32 = 24.0;

/// What the gathering shows at a frame.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Gathering {
    /// Frames since the prompt began to slide in (a point in reach, faced), if one is.
    pub near: Option<f32>,
    /// Frames since A was pressed at the point.
    pub pressed: Option<f32>,
    /// What came of it: the items in the order gathered.
    pub found: Vec<Found>,
    /// The frames after the press the first notice starts (`HAND_NOTICE_AT`, `STAFF_NOTICE_AT`), and the prompt begins to
    /// slide out (0 by hand; the staff's swing takes 24 frames: the gathering's draws, `STAFF_LEAVE_AT`).
    pub notice_at: f32,
    pub leave_at: f32,
    /// Whether the point in reach is the staff's (the prompt reads "Swing" with the X button).
    pub staff: bool,
}

/// An item gathered.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Found {
    /// Its name (a string number).
    pub name: u32,
    /// Whether the library did not know it.
    pub new: bool,
}

/// The layout's pieces the popups are drawn from.
pub(super) struct Popups {
    prompt: ([f32; 4], [f32; 2]),
    prompt_button: Label,
    prompt_text: Label,
    back: ([f32; 4], [f32; 2]),
    index: Label,
    /// The index text's outline (`bold_color`, `bold_size`).
    index_bold: ([f32; 4], f32),
    large: [f32; 2],
    large_size: f32,
    name: Label,
    plate: ([f32; 4], [f32; 2]),
    small: [f32; 2],
    small_size: f32,
    line: Label,
    slide_color: Curve,
    slide_pos: Curve,
    discovery_in_color: Curve,
    discovery_in_pos: Curve,
    discovery_out: Curve,
    notice_in_color: Curve,
    notice_in_pos: Curve,
    notice_out: Curve,
}

/// A text node's `font_color` as 0 to 1 channels, or `fallback`.
fn ink(e: &Element, fallback: [f32; 4]) -> [f32; 4] {
    let c = numbers(e, "font_color");
    if c.len() == 4 {
        [c[0] / 255.0, c[1] / 255.0, c[2] / 255.0, c[3] / 255.0]
    } else {
        fallback
    }
}

impl Popups {
    pub(super) fn read(src: &Sources, places: &Element, field: &Element) -> Result<Popups, String> {
        let style = |e: &Element| e.attr("style").map(str::to_string).ok_or_else(|| "a picture without a style".to_string());
        let text = |e: &Element, at: [f32; 2]| {
            let mut l = label(e, at);
            l.color = Some(ink(e, [0.0, 0.0, 0.0, 1.0]));
            l.spacing = e.attr("word_space").and_then(|v| v.trim().parse().ok()).unwrap_or(0.0);
            l
        };
        let (access, _) = node(places, "fm_access_icon")?;
        let (bg, bg_at) = node(access, "bg")?;
        let (button, button_at) = node(access, "button")?;
        let (words, words_at) = node(access, "text")?;
        let (log, _) = node(places, "discovery_log")?;
        let (back, back_at) = node(log, "back")?;
        let (index, index_at) = node(log, "index")?;
        let (large, large_at) = node(log, "item_image")?;
        let (item_name, name_at) = node(log, "item_name")?;
        let (general, _) = node(places, "general_log")?;
        let (plate, plate_at) = node(general, "bg")?;
        let (small, small_at) = node(general, "item_image")?;
        let (item, item_at) = node(general, "item")?;
        let bold = numbers(index, "bold_color");
        let mut prompt_button = label(button, button_at);
        prompt_button.color = Some([1.0; 4]);
        // A text node without a `font_size` is set at 24 (the capture's 50-pixel glyph at 1080 lines).
        if button.attr("font_size").is_none() {
            prompt_button.size = 24.0;
        }
        let (anims, fm) = (src.anims, src.field_anims);
        Ok(Popups {
            prompt: (picture(field, &style(bg)?)?, bg_at),
            prompt_button,
            prompt_text: text(words, words_at),
            back: (picture(field, &style(back)?)?, back_at),
            index: text(index, index_at),
            index_bold: (if bold.len() == 4 { [bold[0] / 255.0, bold[1] / 255.0, bold[2] / 255.0, bold[3] / 255.0] } else { [0.0, 0.0, 0.0, 1.0] }, index.attr("bold_size").and_then(|v| v.trim().parse().ok()).unwrap_or(0.0)),
            large: large_at,
            large_size: pair(large, "size_wh", [80.0; 2])[0],
            name: text(item_name, name_at),
            plate: (picture(field, &style(plate)?)?, plate_at),
            small: small_at,
            small_size: pair(small, "size_wh", [46.0; 2])[0] * pair(small, "scale", [1.0; 2])[0],
            line: text(item, item_at),
            slide_color: curve(anims, "anm_common_slide_in_bottom", "color")?,
            slide_pos: curve(anims, "anm_common_slide_in_bottom", "pos")?,
            discovery_in_color: curve(fm, "fade_in_discovery_log", "color")?,
            discovery_in_pos: curve(fm, "fade_in_discovery_log", "pos")?,
            discovery_out: curve(fm, "fade_out_discovery_log", "color")?,
            notice_in_color: curve(fm, "fade_in_collect_item", "color")?,
            notice_in_pos: curve(fm, "fade_in_collect_item", "pos")?,
            notice_out: curve(fm, "fade_out_collect_item", "color")?,
        })
    }
}

/// The game's colour bytes as 0 to 1 channels.
fn bytes(c: [f32; 4]) -> [f32; 4] {
    c.map(|v| v / 255.0)
}

/// `ink` under a notice's fade `tint`.
fn under(ink: [f32; 4], tint: [f32; 4]) -> [f32; 4] {
    std::array::from_fn(|i| ink[i] * tint[i])
}

impl Hud {
    /// The discovery log, the prompt and the item notices for `g`, over the rest of the HUD (`gather-rasen2` 3200: the game's quads of
    /// one texture run the log's, the prompt's, the notices').
    pub(super) fn gathering(&self, pen: &mut Pen, g: &Gathering) {
        let Some(p) = &self.popups else { return };
        let since = g.pressed.filter(|_| !g.found.is_empty());
        // The discovery log (the first item the library did not know).
        let first = pen.out.len();
        if let (Some(since), Some((k, f))) = (since, g.found.iter().enumerate().find(|(_, f)| f.new)) {
            let age = since - (g.notice_at - 1.0) - k as f32 * ITEM_GAP;
            if let Some((tint, shift)) = log_state(&p.discovery_in_color, &p.discovery_in_pos, &p.discovery_out, age, DISCOVERY_HOLD) {
                let at = |a: [f32; 2]| [a[0] + shift, a[1] + ROOT_SHIFT];
                pen.picture(FIELD, p.back.0, at(p.back.1), 1.0, tint);
                pen.dropped(&p.index, [shift, ROOT_SHIFT], self.string(280), under(p.index.color.unwrap_or([1.0; 4]), tint), p.index_bold, tint);
                pen.sized(item_texture(k, true), [0.0, 0.0, 512.0, 512.0], at(p.large), p.large_size, tint);
                pen.text(&p.name, [shift, ROOT_SHIFT], self.word(f.name), under(p.name.color.unwrap_or([0.0, 0.0, 0.0, 1.0]), tint));
            }
        }
        keep_in_view(pen, first);
        // The prompt: slides up from below while a point is in reach, back down from A on.
        if let Some(near) = g.near {
            let along = g.pressed.map_or(near, |since| SLIDE_FRAMES - (since - g.leave_at).max(0.0));
            if along > 0.0 {
                let t = along / 60.0;
                let (c, shift) = (p.slide_color.at(t), p.slide_pos.at(t)[1]);
                // The game keeps the colour as bytes: a slide-out frame at 216.3 draws 216 (`gather-rasen2` 3200).
                let color = [1.0, 1.0, 1.0, c[3].floor() / 255.0];
                pen.picture(FIELD, p.prompt.0, [p.prompt.1[0], p.prompt.1[1] + ROOT_SHIFT + shift], 1.0, color);
                let (button, word) = if g.staff { (X_BUTTON, SWING_WORD) } else { (A_BUTTON, PROMPT_WORD) };
                pen.text(&p.prompt_button, [0.0, ROOT_SHIFT + shift], button, color);
                let ink = p.prompt_text.color.unwrap_or([0.0, 0.0, 0.0, 1.0]);
                pen.text(&p.prompt_text, [0.0, ROOT_SHIFT + shift], self.word(word), [ink[0], ink[1], ink[2], color[3]]);
            }
        }
        let Some(since) = since else { return };
        // The item notices, each pushed up a slot by the next one's coming; they leave together.
        let first = pen.out.len();
        let start = |k: usize| g.notice_at + k as f32 * ITEM_GAP;
        let last = start(g.found.len() - 1);
        for (k, f) in g.found.iter().enumerate() {
            let hold = last - start(k) + NOTICE_HOLD;
            let Some((tint, shift)) = log_state(&p.notice_in_color, &p.notice_in_pos, &p.notice_out, since - start(k), hold) else { continue };
            let up: f32 = (k + 1..g.found.len()).map(|j| ((since - start(j)) / NOTICE_UP_FRAMES).clamp(0.0, 1.0) * NOTICE_STEP).sum();
            let at = |a: [f32; 2]| [a[0] + shift, a[1] + ROOT_SHIFT - up];
            pen.picture(FIELD, p.plate.0, at(p.plate.1), 1.0, tint);
            pen.sized(item_texture(k, false), [0.0, 0.0, 256.0, 256.0], at(p.small), p.small_size, tint);
            // The notice's text is a fifth of a frame ahead of its plate on the slide (`gather-rasen2` 3200: its glyphs sit 3 pixels right of Barrel's, one of them over the screen's edge).
            let text_shift = if since - start(k) < hold { p.notice_in_pos.at((since - start(k) + TEXT_AHEAD) / 60.0)[0] } else { shift };
            pen.text(&p.line, [text_shift, ROOT_SHIFT - up], self.word(f.name), under(p.line.color.unwrap_or([0.0, 0.0, 0.0, 1.0]), tint));
        }
        keep_in_view(pen, first);
    }
}

/// The game draws nothing that falls wholly off the screen (the notices come in from the left): the quads from `first` on.
fn keep_in_view(pen: &mut Pen, first: usize) {
    let (from, to) = (pen.left, pen.left + 1280.0 * pen.k);
    let mut at = 0;
    pen.out.retain(|q| {
        at += 1;
        at <= first || (q.rect[0] < to && q.rect[2] > from)
    });
}

/// A notice's colour and how far right of its place it is `age` frames after it began (`None`
/// before it, and after it is gone): faded in by `fade_in`, held until `hold` frames after its
/// start, then faded out by `fade_out`.
fn log_state(fade_in: &Curve, slide: &Curve, fade_out: &Curve, age: f32, hold: f32) -> Option<([f32; 4], f32)> {
    if age < 0.0 {
        return None;
    }
    let (tint, shift) = if age < hold { (bytes(fade_in.at(age / 60.0)), slide.at(age / 60.0)[0]) } else { (bytes(fade_out.at((age - hold) / 60.0)), 0.0) };
    (tint[3] > 0.0).then_some((tint, shift))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn linear(xml: &str, name: &str, attr: &str) -> Curve {
        curve(format!("<root>{xml}</root>").as_bytes(), name, attr).unwrap()
    }

    #[test]
    fn a_notice_fades_in_holds_and_fades_out() {
        let fade_in = linear(r#"<anim name="a"><curve attr="color"><key value="128, 128, 128, 0"/><key frame60="30" value="255, 255, 255, 255"/></curve></anim>"#, "a", "color");
        let slide = linear(r#"<anim name="a"><curve attr="pos"><key value="-200, 0, 0"/><key frame60="30" value="0, 0, 0"/></curve></anim>"#, "a", "pos");
        let out = linear(r#"<anim name="a"><curve attr="color"><key value="255, 255, 255, 255"/><key frame60="30" value="128, 128, 128, 0"/></curve></anim>"#, "a", "color");
        // `gather-rasen` 2135, 8 frames in: alpha 68 of 255, grey 162, 146.7 layout pixels left of its place.
        let (tint, shift) = log_state(&fade_in, &slide, &out, 8.0, 211.0).unwrap();
        assert!((tint[3] - 68.0 / 255.0).abs() < 1e-3 && (tint[0] - 162.0 / 255.0).abs() < 1e-3 && (shift + 146.667).abs() < 0.01, "{tint:?} {shift}");
        assert_eq!(log_state(&fade_in, &slide, &out, -1.0, 211.0), None);
        assert_eq!(log_state(&fade_in, &slide, &out, 100.0, 211.0).map(|(t, s)| (t, s)), Some(([1.0; 4], 0.0)));
        // 2365 is 27 frames into the fade-out: alpha 25 of 255.
        let (tint, _) = log_state(&fade_in, &slide, &out, 211.0 + 27.0, 211.0).unwrap();
        assert!((tint[3] - 25.5 / 255.0).abs() < 1e-3);
        assert_eq!(log_state(&fade_in, &slide, &out, 211.0 + 30.0, 211.0), None);
    }
}
