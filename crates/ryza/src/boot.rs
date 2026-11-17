//! Atelier Ryza DX's boot screens, the ones before the title (`docs/renderer.md`, "Ryza's title
//! screen"): the "Creating system data..." notice, the two logos, and the autosave notice that
//! stands in for the loading screen. All of it is 2D through the text layer's quads, so the draw
//! trace records it as it does the title.
//!
//! The pictures and their places come from the game's own files: the notice's nodes from
//! `uil_a21_saveload_progress.xml` (styles `uis_gen_a21_saveload.xml`, animations
//! `uia_a21_saveload.xml`), the autosave window from `uil_a21_notification_display.xml` (window
//! style `uis_gen_a21_memorymap.xml`, spinner style `uis_gen_a21v2_saveload.xml`, the sentence in
//! `str_ui.xml`), the slide and fade curves from `uia_common.xml`. The logos are single full-screen
//! pictures. Positions are in the 1280 x 720 layout scaled by `screen height / 720`, centred
//! across the width, as the title's.

use std::collections::HashMap;

use barrel_formats::table::{self, Element};
use barrel_render::text::{Glyph, Quad, Spin};

use crate::font::ATLAS_SIZE;

/// The textures a boot layer holds, in the order of `textures`'s quads: the title's own two
/// (font, title atlas: `text::FONT`, `text::UI`) come first, then these.
pub const SAVELOAD_LETTERS: usize = 2;
pub const SAVELOAD_PICTURES: usize = 3;
pub const LOGO_KT: usize = 4;
pub const LOGO_GUST: usize = 5;
pub const WINDOW: usize = 6;
pub const SPINNER: usize = 7;
/// How many textures a boot layer has.
pub const TEXTURES: usize = 8;

const LAYOUT_HEIGHT: f32 = 720.0;
/// A style's pixels per layout pixel (the 2160-line atlases).
const ATLAS_PER_LAYOUT: f32 = 3.0;

// The schedule, in seconds from the notice's start, read from the capture (`title-cycle`,
// every 5th frame from 35 to 1635, made on ryza-recomp's exact virtual clock: 60 frames a
// second, the same frame for frame in every run). Each time is the one frame offset at which
// the capture's quads match at the frames named (a scan of plus and minus 12 frames; the capture made with the
// timeouts on the virtual clock starts the notice a frame later, at 12, and the title where it
// was).
/// The capture frame the notice starts at (its pictures and pen match at frames 35 and 105).
const CAPTURE_START: f32 = 12.0;
const FPS: f32 = 60.0;
/// The notice fades out from capture frame 118 over `anm_common_fade_out`'s 15 frames (frame
/// 125) and is gone at 133.
const NOTICE_FADE_OUT_AT: f32 = (118.0 - CAPTURE_START) / FPS;
const NOTICE_END: f32 = (133.0 - CAPTURE_START) / FPS;
/// The Koei Tecmo logo's fade-in starts at capture frame 172 (frame 180) and the Gust logo's
/// at 352, as the first has faded out: a logo fades for 30 frames each way (the capture's
/// `fAlphaRate`: 0.5 s) and holds for 120 between (the Gust logo's fade-out at frame 515).
const KT_START: f32 = (172.0 - CAPTURE_START) / FPS;
const GUST_START: f32 = (352.0 - CAPTURE_START) / FPS;
const LOGO_FADE: f32 = 30.0 / FPS;
const LOGO_HOLD: f32 = 120.0 / FPS;
/// The autosave window slides in at capture frame 532, as the Gust logo is gone (frames 535 and
/// 635), and its getOut starts at 802 (frame 825): the spinner fades out then
/// (`anm_common_fade_in` played backwards at speed 0.3, the window's `getOut`) and the window's
/// own slide back follows 0.2 s later (`delay` of that bind).
const WINDOW_START: f32 = (532.0 - CAPTURE_START) / FPS;
const SPINNER_OUT: f32 = (802.0 - CAPTURE_START) / FPS;
const SPINNER_OUT_SPEED: f32 = 0.3;
const WINDOW_OUT: f32 = SPINNER_OUT + 0.2;
/// The title's first frame (capture frame 875, where `--title-at 0` sits): the screen is black
/// from the window's end until then (the title's own fade from black is not drawn).
pub const TITLE_START: f32 = (875.0 - CAPTURE_START) / FPS;

/// The seconds of the boot sequence (the title starts at [`TITLE_START`]).
pub const LENGTH: f32 = TITLE_START;

/// One animation attribute: keys of (seconds, value), easing between them. Unlike the title's
/// reader this keeps a key that repeats the one before's frame: that is a jump (the pen's
/// move), the value then the later key's.
#[derive(Clone, Debug)]
struct Track {
    keys: Vec<(f32, [f32; 4])>,
    easing: Easing,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Easing {
    Linear,
    InCubic,
    OutCubic,
}

impl Track {
    fn read(anims: &Element, name: &str, attr: &str) -> Result<Track, String> {
        let anim = anims.children.iter().find(|e| e.name == "anim" && e.attr("name") == Some(name)).ok_or(format!("no animation {name}"))?;
        let c = anim.children.iter().find(|e| e.name == "curve" && e.attr("attr") == Some(attr)).ok_or(format!("{name} has no {attr}"))?;
        let easing = match c.attr("interpolate") {
            Some("easing_incubic") => Easing::InCubic,
            Some("easing_outcubic") => Easing::OutCubic,
            None => Easing::Linear,
            Some(other) => return Err(format!("{name}: easing {other} (not read)")),
        };
        let keys = c
            .children
            .iter()
            .filter(|e| e.name == "key")
            .map(|k| {
                let t = k.attr("frame60").and_then(|f| f.trim().parse::<f32>().ok()).map_or(0.0, |f| f / FPS);
                let mut value = [0.0; 4];
                for (d, s) in value.iter_mut().zip(numbers(k, "value")) {
                    *d = s;
                }
                (t, value)
            })
            .collect::<Vec<_>>();
        if keys.is_empty() {
            return Err(format!("{name}'s {attr} has no keys"));
        }
        Ok(Track { keys, easing })
    }

    fn duration(&self) -> f32 {
        self.keys.last().unwrap().0
    }

    /// The value `t` seconds in (held at the ends).
    fn at(&self, t: f32) -> [f32; 4] {
        let (first, last) = (self.keys[0], *self.keys.last().unwrap());
        if t <= first.0 {
            return first.1;
        }
        if t >= last.0 {
            return last.1;
        }
        let i = self.keys.iter().rposition(|k| k.0 <= t).unwrap();
        let ((t0, v0), (t1, v1)) = (self.keys[i], self.keys[i + 1]);
        let u = (t - t0) / (t1 - t0);
        let e = match self.easing {
            Easing::Linear => u,
            Easing::InCubic => u * u * u,
            Easing::OutCubic => 1.0 - (1.0 - u).powi(3),
        };
        std::array::from_fn(|c| v0[c] + (v1[c] - v0[c]) * e)
    }
}

fn numbers(e: &Element, attr: &str) -> Vec<f32> {
    e.attr(attr).unwrap_or("").split(',').filter_map(|v| v.trim().parse().ok()).collect()
}

fn pair(e: &Element, attr: &str, default: [f32; 2]) -> [f32; 2] {
    let v = numbers(e, attr);
    [v.first().copied().unwrap_or(default[0]), v.get(1).copied().unwrap_or(default[1])]
}

/// The node called `name` with the positions above it added up.
fn find<'a>(e: &'a Element, name: &str, at: [f32; 2]) -> Option<(&'a Element, [f32; 2])> {
    let here = pair(e, "pos", [0.0; 2]);
    let at = [at[0] + here[0], at[1] + here[1]];
    if e.attr("name") == Some(name) {
        return Some((e, at));
    }
    e.children.iter().find_map(|c| find(c, name, at))
}

/// A style: its pictures (left, top, width, height in the 2160-line atlas) and, for a nine-slice
/// (`prim_type="wrap"`), its edge margins (left, top, right, bottom).
#[derive(Clone, Debug)]
struct Style {
    pictures: Vec<[f32; 4]>,
    margins: Option<[f32; 4]>,
}

fn style(styles: &Element, name: &str) -> Result<Style, String> {
    let image = |e: &Element| -> Option<[f32; 4]> { numbers(e, "uvwh").try_into().ok() };
    styles
        .children
        .iter()
        .find_map(|e| match e.name.as_str() {
            "image" if e.attr("name") == Some(name) => Some(Style { pictures: image(e).into_iter().collect(), margins: numbers(e, "edge_margin").try_into().ok().filter(|_| e.attr("prim_type") == Some("wrap")) }),
            "image_list" if e.attr("name") == Some(name) => Some(Style { pictures: e.children.iter().filter_map(image).collect(), margins: None }),
            _ => None,
        })
        .ok_or(format!("no style {name}"))
}

/// A letter of the notice's text line.
#[derive(Clone, Debug)]
struct Letter {
    /// The top-left in the layout and the picture's size in layout pixels.
    pos: [f32; 2],
    size: [f32; 2],
    uvwh: [f32; 4],
    /// Seconds into the hopping loop the letter's hop starts at (`bind_node`'s `delay`).
    delay: f32,
    /// The hop's own curves (the dots use the plain jump, `anm_hopping_ten`).
    spins: bool,
}

/// An orb of the autosave window's spinner.
#[derive(Clone, Debug)]
struct Orb {
    /// The centre in the layout, and the turn in degrees.
    center: [f32; 2],
    rot: f32,
    /// Which of the eight glow curves lights it.
    glow: usize,
}

pub struct Boot {
    glyphs: HashMap<char, Glyph>,
    /// The atlases in pixels as the styles count them: letters, pictures, window, spinner, font.
    atlas: [(f32, f32); 5],
    // The notice.
    icon_at: [f32; 2],
    icon: Style,
    icon_flick: f32,
    letters: Vec<Letter>,
    pen_style: [f32; 4],
    pen_speed: f32,
    hop_speed: f32,
    hop: [Track; 3],
    hop_ten: Track,
    pen_pos: Track,
    pen_rot: Track,
    // The autosave window.
    window_at: [f32; 2],
    window_size: [f32; 2],
    window_style: Style,
    text_at: [f32; 2],
    text_size: f32,
    text_color: [f32; 4],
    text_wrap: f32,
    text_gap: f32,
    sentence: String,
    orbs: Vec<Orb>,
    orb_size: f32,
    orb_pictures: Vec<[f32; 4]>,
    group_at: [f32; 2],
    group_pivot: [f32; 2],
    orb_scale: f32,
    orb_speed: f32,
    glows: Vec<Track>,
    slide_color: Track,
    slide_pos: Track,
    fade_out: Track,
}

/// The files `Boot::read` takes, as their bytes.
pub struct Sources<'a> {
    /// `saves/ui_<lang>/a21_saveload/uil_a21_saveload_progress.xml`
    pub progress: &'a [u8],
    /// `saves/ui_<lang>/gen_styles/uis_gen_a21_saveload.xml`
    pub saveload_styles: &'a [u8],
    /// `saves/ui_<lang>/a21_saveload/uia_a21_saveload.xml`
    pub saveload_anims: &'a [u8],
    /// `saves/ui_<lang>/uia_common.xml`
    pub common_anims: &'a [u8],
    /// `saves/ui_cmn/a21_saveload/uil_a21_notification_display.xml`
    pub notification: &'a [u8],
    /// `saves/ui_<lang>/gen_styles/uis_gen_a21_memorymap.xml`
    pub window_styles: &'a [u8],
    /// `saves/ui_cmn/gen_styles/uis_gen_a21v2_saveload.xml`
    pub spinner_styles: &'a [u8],
    /// `saves/text_<lang>/str_ui.xml`
    pub strings: &'a [u8],
    pub glyphs: HashMap<char, Glyph>,
    /// Sizes of the 1080-line textures: the notice's letters and pictures, the window's, the
    /// spinner's, and the font's.
    pub sizes: [(u32, u32); 5],
}

/// `STR_UI_nnnn` is the string numbered `STR_UI_BASE + nnnn` in `str_ui.xml`.
const STR_UI_BASE: u32 = 6_619_137;
/// The autosave sentence's `STR_UI` number (the window's text node names it).
const SENTENCE: u32 = 896;
/// The text line's top against its node, in layout pixels. Fitted to the capture (the first
/// glyph row 6 pixels above where the node's `pos.y` puts it at 1080 lines): the title's version
/// string needed the same 4 pixels (`title::TEXT_TOP`), the game's own rule is not read.
const TEXT_TOP: f32 = -4.0;
/// The distance between the window text's lines in layout pixels. Fitted to the capture (lines
/// 33 pixels apart at 1080 lines); the file's `line_space` of 9 is added to a line height the
/// layout does not give.
const LINE_STEP: f32 = 22.0;
/// How wide a line may run before the text wraps, in `font_size` units: the node's
/// `line_char_length`. Fitted: the capture's first line is 640 pixels wide at 1080 lines.
const WRAP_FACTOR: f32 = 1.0;

impl Boot {
    pub fn read(src: Sources) -> Result<Boot, String> {
        let progress = table::parse(src.progress).map_err(|e| format!("uil_a21_saveload_progress: {e}"))?;
        let styles = table::parse(src.saveload_styles).map_err(|e| format!("uis_gen_a21_saveload: {e}"))?;
        let anims = table::parse(src.saveload_anims).map_err(|e| format!("uia_a21_saveload: {e}"))?;
        let common = table::parse(src.common_anims).map_err(|e| format!("uia_common: {e}"))?;
        let note = table::parse(src.notification).map_err(|e| format!("uil_a21_notification_display: {e}"))?;
        let window_styles = table::parse(src.window_styles).map_err(|e| format!("uis_gen_a21_memorymap: {e}"))?;
        let spinner_styles = table::parse(src.spinner_styles).map_err(|e| format!("uis_gen_a21v2_saveload: {e}"))?;
        let strings = table::parse(src.strings).map_err(|e| format!("str_ui: {e}"))?;

        // The notice: the saving icon with its "creating system data" line and the pen.
        let (icon, icon_at) = find(&progress, "icon_saving", [0.0; 2]).ok_or("no icon_saving")?;
        let icon_style = style(&styles, icon.attr("style").ok_or("icon_saving has no style")?)?;
        let (line, line_at) = find(icon, "text_anime_creating", [0.0; 2]).ok_or("no text_anime_creating")?;
        let ctrl = line.children.iter().find(|c| c.name == "anim_ctrl").ok_or("text_anime_creating has no animation")?;
        let hop_speed = ctrl.attr("speed").and_then(|s| s.trim().parse().ok()).unwrap_or(1.0);
        let mut letters = Vec::new();
        for l in line.children.iter().filter(|c| c.name == "image") {
            let name = l.attr("name").unwrap_or("");
            let bind = ctrl.children.iter().find(|b| b.attr("node") == Some(name)).ok_or(format!("{name}: not bound to the hop"))?;
            let uvwh = style(&styles, l.attr("style").unwrap_or(""))?.pictures[0];
            let [x, y] = pair(l, "pos", [0.0; 2]);
            letters.push(Letter {
                pos: [icon_at[0] + line_at[0] + x, icon_at[1] + line_at[1] + y],
                size: pair(l, "size_wh", [uvwh[2] / ATLAS_PER_LAYOUT, uvwh[3] / ATLAS_PER_LAYOUT]),
                uvwh,
                delay: bind.attr("delay").and_then(|d| d.trim().parse().ok()).unwrap_or(0.0),
                spins: bind.attr("anim") == Some("anm_hopping"),
            });
        }
        let (pen, _) = find(icon, "prompt_icon", [0.0; 2]).ok_or("no prompt_icon")?;
        let pen_ctrl = pen.children.iter().find(|c| c.name == "anim_ctrl").ok_or("prompt_icon has no animation")?;
        let anim_of = |name| Track::read(&anims, name, "pos");

        // The autosave window.
        let (window, window_origin) = find(&note, "window", [0.0; 2]).ok_or("no window")?;
        let window_style = style(&window_styles, window.attr("style").ok_or("window has no style")?)?;
        let (text, _) = find(window, "text", [0.0; 2]).ok_or("no window text")?;
        let [tx, ty] = pair(text, "pos", [0.0; 2]);
        let tc = numbers(text, "color");
        let (group, group_at) = find(&note, "loading_1", [0.0; 2]).ok_or("no loading_1")?;
        let orb_ctrl_at = |image: &Element| -> Option<usize> {
            let anim = image.children.iter().find(|c| c.name == "anim_ctrl")?.children.first()?.attr("anim")?.to_string();
            anim.rsplit('_').next()?.parse::<usize>().ok().map(|n| n - 1)
        };
        let orb_style = style(&spinner_styles, "gen_a21_cmn_al_001_orb")?;
        let mut orbs = Vec::new();
        for o in group.children.iter().filter(|c| c.attr("name").is_some_and(|n| n.starts_with("circle_base"))) {
            let [x, y] = pair(o, "pos", [0.0; 2]);
            let rot = numbers(o, "rot").get(2).copied().unwrap_or(0.0);
            orbs.push(Orb { center: [x, y], rot, glow: orb_ctrl_at(o).ok_or("an orb has no glow")? });
        }
        let orb_size = orb_style.pictures[0][2] / ATLAS_PER_LAYOUT;
        for o in &mut orbs {
            o.center = [o.center[0] + orb_size / 2.0, o.center[1] + orb_size / 2.0];
        }
        let sentence_no = (STR_UI_BASE + SENTENCE).to_string();
        let sentence = strings
            .children
            .iter()
            .find(|s| s.attr("String_No") == Some(sentence_no.as_str()))
            .and_then(|s| s.attr("Text"))
            .ok_or("the autosave sentence is not in str_ui.xml")?
            .to_string();
        let glows = (1..=8).map(|n| Track::read(&anims, &format!("anm_loading_icon_eff_{n}"), "color")).collect::<Result<Vec<_>, _>>()?;
        let pen_speed = pen_ctrl.attr("speed").and_then(|s| s.trim().parse().ok()).unwrap_or(1.0);
        let half = |(w, h): (u32, u32)| (w as f32 * 2.0, h as f32 * 2.0);
        let s = src.sizes;
        Ok(Boot {
            glyphs: src.glyphs,
            atlas: [half(s[0]), half(s[1]), half(s[2]), half(s[3]), (s[4].0 as f32, s[4].1 as f32)],
            icon_at,
            icon: icon_style,
            icon_flick: icon.attr("flick_time").and_then(|f| f.trim().parse().ok()).ok_or("icon_saving has no flick_time")?,
            letters,
            pen_style: style(&styles, pen.attr("style").ok_or("prompt_icon has no style")?)?.pictures[0],
            pen_speed,
            hop_speed,
            hop: [Track::read(&anims, "anm_hopping", "pos")?, Track::read(&anims, "anm_hopping", "rot")?, Track::read(&anims, "anm_hopping", "scale")?],
            hop_ten: anim_of("anm_hopping_ten")?,
            pen_pos: anim_of("anm_penmove")?,
            pen_rot: Track::read(&anims, "anm_penmove", "rot")?,
            window_at: window_origin,
            window_size: pair(window, "size_wh", [0.0; 2]),
            window_style,
            text_at: [tx, ty],
            text_size: text.attr("font_size").and_then(|v| v.trim().parse().ok()).ok_or("window text has no font_size")?,
            text_color: [tc.first().copied().unwrap_or(255.0) / 255.0, tc.get(1).copied().unwrap_or(255.0) / 255.0, tc.get(2).copied().unwrap_or(255.0) / 255.0, tc.get(3).copied().unwrap_or(255.0) / 255.0],
            text_wrap: text.attr("line_char_length").and_then(|v| v.trim().parse().ok()).ok_or("window text has no line_char_length")?,
            text_gap: text.attr("line_space").and_then(|v| v.trim().parse().ok()).unwrap_or(0.0),
            sentence,
            orbs,
            orb_size,
            orb_pictures: orb_style.pictures.clone(),
            group_at,
            group_pivot: pair(group, "pivot_pos", [0.0; 2]),
            orb_scale: pair(group, "scale", [1.0; 2])[0],
            orb_speed: {
                let c = group.children.iter().find_map(|o| o.children.iter().find(|c| c.name == "anim_ctrl"));
                c.and_then(|c| c.attr("speed")).and_then(|s| s.trim().parse().ok()).unwrap_or(1.0)
            },
            glows,
            slide_color: Track::read(&common, "anm_common_slide_in_bottom", "color")?,
            slide_pos: Track::read(&common, "anm_common_slide_in_bottom", "pos")?,
            fade_out: Track::read(&common, "anm_common_fade_out", "color")?,
        })
    }

    /// The quads `t` seconds into the boot sequence on a `screen` of pixels, in draw order.
    pub fn quads(&self, t: f32, screen: (u32, u32)) -> Vec<Quad> {
        let k = screen.1 as f32 / LAYOUT_HEIGHT;
        let left = (screen.0 as f32 - 1280.0 * k) * 0.5;
        if t < NOTICE_END {
            self.notice(t, k, left)
        } else if t >= KT_START && t < KT_START + 2.0 * LOGO_FADE + LOGO_HOLD {
            vec![logo(LOGO_KT, logo_alpha(t - KT_START), screen)]
        } else if t >= GUST_START && t < GUST_START + 2.0 * LOGO_FADE + LOGO_HOLD {
            vec![logo(LOGO_GUST, logo_alpha(t - GUST_START), screen)]
        } else if t >= WINDOW_START && t < TITLE_START {
            self.autosave(t, k, left)
        } else {
            Vec::new()
        }
    }

    fn quad(&self, atlas: usize, texture: usize, uvwh: [f32; 4], rect: [f32; 4], color: [f32; 4], spin: Option<Spin>) -> Quad {
        let (aw, ah) = self.atlas[atlas];
        let [x, y, w, h] = uvwh;
        Quad { turn: false, spin, rect, uv: [x / aw, y / ah, (x + w) / aw, (y + h) / ah], color, texture }
    }

    /// "Creating system data...": the saving icon (its picture list steps every `flick_time`), the
    /// hopping letters and the writing pen, fading out at the end.
    fn notice(&self, t: f32, k: f32, left: f32) -> Vec<Quad> {
        let alpha = if t < NOTICE_FADE_OUT_AT { 1.0 } else { self.fade_out.at(t - NOTICE_FADE_OUT_AT)[3] / 255.0 };
        let color = [1.0, 1.0, 1.0, alpha];
        let px = |p: [f32; 2]| [left + p[0] * k, p[1] * k];
        let mut out = Vec::new();
        let n = self.icon.pictures.len();
        let pic = self.icon.pictures[(t / self.icon_flick).floor() as usize % n];
        let [x, y] = px(self.icon_at);
        let (w, h) = (pic[2] / ATLAS_PER_LAYOUT, pic[3] / ATLAS_PER_LAYOUT);
        out.push(self.quad(1, SAVELOAD_PICTURES, pic, [x, y, x + w * k, y + h * k], color, None));
        for l in &self.letters {
            // The hop's clock is the control's speed times the time, the letter's delay later.
            let tau = t * self.hop_speed - l.delay;
            let (dy, rot, scale) = if l.spins {
                (self.hop[0].at(tau)[1], self.hop[1].at(tau)[2], self.hop[2].at(tau)[0])
            } else {
                (self.hop_ten.at(tau)[1], 0.0, 1.0)
            };
            let c = [l.pos[0] + l.size[0] / 2.0, l.pos[1] + dy + l.size[1] / 2.0];
            let [cx, cy] = px(c);
            let (hw, hh) = (l.size[0] * scale * k / 2.0, l.size[1] * scale * k / 2.0);
            let rect = [cx - hw, cy - hh, cx + hw, cy + hh];
            let spin = (rot != 0.0).then(|| Spin { pivot: [cx, cy], angle: rot.to_radians() });
            out.push(self.quad(0, SAVELOAD_LETTERS, l.uvwh, rect, color, spin));
        }
        // The pen hangs from the icon's bottom right corner, turned about its top left.
        // A looping animation restarts on the first 60 Hz tick after its period (the game steps in
        // whole frames and drops the remainder), so the second pass runs a part of a frame behind.
        let ticks = (t * FPS).round();
        let tau = loop_time(ticks, self.pen_pos.duration().max(self.pen_rot.duration()), self.pen_speed);
        let [dx, dy, _, _] = self.pen_pos.at(tau);
        let rot = self.pen_rot.at(tau)[2];
        let [x, y] = px([self.icon_at[0] + w + dx, self.icon_at[1] + h + dy]);
        let [pw, ph] = [self.pen_style[2] / ATLAS_PER_LAYOUT * k, self.pen_style[3] / ATLAS_PER_LAYOUT * k];
        out.push(self.quad(1, SAVELOAD_PICTURES, self.pen_style, [x, y, x + pw, y + ph], color, Some(Spin { pivot: [x, y], angle: rot.to_radians() })));
        out
    }

    /// The autosave window: a nine-slice panel, its sentence and the spinner, sliding in from
    /// below and out again.
    fn autosave(&self, t: f32, k: f32, left: f32) -> Vec<Quad> {
        let since = t - WINDOW_START;
        let (alpha, slide) = if t < WINDOW_OUT {
            (self.slide_color.at(since)[3] / 255.0, self.slide_pos.at(since)[1])
        } else {
            // getOut plays the slide backwards.
            let back = self.slide_color.duration() - (t - WINDOW_OUT);
            (self.slide_color.at(back)[3] / 255.0, self.slide_pos.at(back)[1])
        };
        // Colours are bytes in the game: an opacity is truncated to 1/255.
        let alpha = byte(alpha) as f32 / 255.0;
        if alpha <= 0.0 {
            return Vec::new();
        }
        let (ox, oy) = (self.window_at[0], self.window_at[1] + slide);
        let mut out = Vec::new();
        // The nine slices: each edge at its margin, the middle repeated and cropped to fit.
        let [ml, mt, mr, mb] = self.window_style.margins.unwrap_or([0.0; 4]);
        let [u, v, uw, uh] = self.window_style.pictures[0];
        let [w, h] = self.window_size;
        let cols = [(0.0, ml, u, ml), (ml, uw - ml - mr, u + ml, uw - ml - mr), (uw - mr, mr, u + uw - mr, mr)];
        let rows = [(0.0, mt, v, mt), (mt, uh - mt - mb, v + mt, uh - mt - mb), (uh - mb, mb, v + uh - mb, mb)];
        let spans = |extent: f32, edges: [(f32, f32, f32, f32); 3]| -> Vec<(f32, f32, f32, f32)> {
            // (layout offset, layout length, uv start, uv length)
            let (a, b, c) = (edges[0], edges[1], edges[2]);
            let mut v = vec![(0.0, a.1 / ATLAS_PER_LAYOUT, a.2, a.3)];
            let room = extent - (a.1 + c.1) / ATLAS_PER_LAYOUT;
            let tile = b.1 / ATLAS_PER_LAYOUT;
            let mut at = a.1 / ATLAS_PER_LAYOUT;
            let mut left = room;
            while left > 0.001 {
                let len = tile.min(left);
                v.push((at, len, b.2, b.3 * len / tile));
                at += len;
                left -= len;
            }
            v.push((at, c.1 / ATLAS_PER_LAYOUT, c.2, c.3));
            v
        };
        let (xs, ys) = (spans(w, cols), spans(h, rows));
        let color = [1.0, 1.0, 1.0, alpha];
        for r in &ys {
            for c in &xs {
                let (x, y) = (left + (ox + c.0) * k, (oy + r.0) * k);
                out.push(self.quad(2, WINDOW, [c.2, r.2, c.3, r.3], [x, y, x + c.1 * k, y + r.1 * k], color, None));
            }
        }
        out.extend(self.sentence_quads(ox, oy, k, left, alpha));
        let spinner = if t < SPINNER_OUT { 1.0 } else { self.fade_out.at((t - SPINNER_OUT) * SPINNER_OUT_SPEED)[3] / 255.0 };
        out.extend(self.orb_quads(since, k, left, byte(alpha), byte(spinner), slide));
        out
    }

    fn sentence_quads(&self, ox: f32, oy: f32, k: f32, left: f32, alpha: f32) -> Vec<Quad> {
        let s = self.text_size * k / ATLAS_SIZE;
        let limit = self.text_wrap * self.text_size * WRAP_FACTOR * k;
        let mut lines: Vec<String> = Vec::new();
        let mut cur = String::new();
        let width = |t: &str| -> f32 { t.chars().filter_map(|c| self.glyphs.get(&c)).map(|g| g.advance as f32 * s).sum() };
        for word in self.sentence.split(' ') {
            let candidate = if cur.is_empty() { word.to_string() } else { format!("{cur} {word}") };
            if !cur.is_empty() && width(&candidate) > limit {
                // The space before the break stays on the line (the game draws it there).
                cur.push(' ');
                lines.push(std::mem::take(&mut cur));
                cur = word.to_string();
            } else {
                cur = candidate;
            }
        }
        lines.push(cur);
        let mut out = Vec::new();
        let color = [self.text_color[0], self.text_color[1], self.text_color[2], alpha];
        for (i, line) in lines.iter().enumerate() {
            let mut pen = left + (ox + self.text_at[0]) * k;
            let top = (oy + self.text_at[1] + TEXT_TOP + i as f32 * LINE_STEP) * k;
            for c in line.chars() {
                let Some(g) = self.glyphs.get(&c) else { continue };
                let l = pen + g.x_offset as f32 * s;
                let t = top + g.y_offset as f32 * s;
                let (aw, ah) = self.atlas[4];
                let (x, y, w, h) = (g.x as f32, g.y as f32, g.width as f32, g.height as f32);
                out.push(Quad { turn: false, spin: None, rect: [l, t, l + w * s, t + h * s], uv: [x / aw, y / ah, (x + w) / aw, (y + h) / ah], color, texture: barrel_render::text::FONT });
                pen += g.advance as f32 * s;
            }
        }
        let _ = self.text_gap;
        out
    }

    /// The spinner: eight orbs about the group's pivot, each with its glow over it (a glow
    /// that is clear draws nothing).
    fn orb_quads(&self, since: f32, k: f32, left: f32, window: u32, spinner: u32, slide: f32) -> Vec<Quad> {
        let mut out = Vec::new();
        let ticks = (since * FPS).round();
        let (pw, ph) = (self.orb_size * self.orb_scale * k, self.orb_size * self.orb_scale * k);
        // Opacities are bytes multiplied down the node tree, each product truncated.
        let base = window * spinner / 255;
        for o in &self.orbs {
            // The group scales about its pivot: a point p lands at origin + pivot + (p - pivot) * scale.
            let at = |i: usize| self.group_at[i] + self.group_pivot[i] + (o.center[i] - self.group_pivot[i]) * self.orb_scale;
            let (cx, cy) = (left + at(0) * k, (at(1) + slide) * k);
            let rect = [cx - pw / 2.0, cy - ph / 2.0, cx + pw / 2.0, cy + ph / 2.0];
            let spin = (o.rot != 0.0).then(|| Spin { pivot: [cx, cy], angle: o.rot.to_radians() });
            out.push(self.quad(3, SPINNER, self.orb_pictures[0], rect, [1.0, 1.0, 1.0, base as f32 / 255.0], spin));
            let glow = &self.glows[o.glow];
            let g = glow.at(loop_time(ticks, glow.duration(), self.orb_speed))[3].floor() as u32 * base / 255;
            if g > 0 {
                out.push(self.quad(3, SPINNER, self.orb_pictures[1], rect, [1.0, 1.0, 1.0, g as f32 / 255.0], spin));
            }
        }
        out
    }
}

/// An opacity as the byte the game holds it in (truncated).
fn byte(alpha: f32) -> u32 {
    (alpha.clamp(0.0, 1.0) * 255.0 + 1e-3).floor() as u32
}

/// The game's frame time at 60 frames a second, as the 32-bit float it holds it in
/// (16,666,667 ns over 1e9).
const FRAME: f32 = 0.016_666_668;

/// The time inside a looping animation of `duration` seconds at `speed` after `ticks` frames,
/// as the game counts it: each frame it adds `speed x frame time` to a 32-bit float and starts
/// again from 0 once the sum reaches the duration, dropping the remainder. A loop so takes the
/// whole number of frames above its length, and one whose length is an exact number of frames
/// takes one more (50 frames' worth takes 51: the sum falls a hair short), or not, by the
/// float sums: at a frame time 4 ns longer the same loop takes 50 (docs/ryza/open.md, "Loops
/// that take 50 or 51 frames").
pub(crate) fn loop_time(ticks: f32, duration: f32, speed: f32) -> f32 {
    let step = FRAME * speed;
    let mut t = 0.0f32;
    for _ in 0..ticks.max(0.0) as u32 {
        t += step;
        if t >= duration {
            t = 0.0;
        }
    }
    t
}

/// A logo's opacity `t` seconds after its start: in, hold, out.
fn logo_alpha(t: f32) -> f32 {
    ((t / LOGO_FADE).min(1.0)).min(1.0 - (t - LOGO_FADE - LOGO_HOLD) / LOGO_FADE).clamp(0.0, 1.0)
}

/// A logo covers the whole screen.
fn logo(texture: usize, alpha: f32, screen: (u32, u32)) -> Quad {
    Quad { turn: false, spin: None, rect: [0.0, 0.0, screen.0 as f32, screen.1 as f32], uv: [0.0, 0.0, 1.0, 1.0], color: [1.0, 1.0, 1.0, alpha], texture }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn near(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-4
    }

    #[test]
    fn a_logo_fades_in_holds_and_fades_out() {
        assert!(near(logo_alpha(0.0), 0.0));
        assert!(near(logo_alpha(LOGO_FADE / 2.0), 0.5));
        assert!(near(logo_alpha(LOGO_FADE + LOGO_HOLD / 2.0), 1.0));
        assert!(near(logo_alpha(2.0 * LOGO_FADE + LOGO_HOLD), 0.0));
    }

    #[test]
    fn a_loop_takes_the_whole_frame_above_its_length() {
        // 40 frames at speed 0.8 are 50 ticks of the clock; the sum falls short and the 51st restarts.
        assert!(near(loop_time(50.0, 40.0 / 60.0, 0.8), 50.0 * 0.8 / 60.0));
        assert!(near(loop_time(51.0, 40.0 / 60.0, 0.8), 0.0));
        // 120 frames at speed 1.33 are 90.2 ticks: the restart is at tick 91.
        assert!(near(loop_time(90.0, 2.0, 1.33), 90.0 * 1.33 / 60.0));
        assert!(near(loop_time(91.0, 2.0, 1.33), 0.0));
    }

    #[test]
    fn opacities_are_truncated_to_bytes() {
        assert_eq!(byte(0.4213), 107);
        assert_eq!(byte(1.0), 255);
        assert_eq!(byte(1.5), 255);
    }

    #[test]
    fn a_track_keeps_a_jump_and_eases() {
        let anims = table::parse_str(
            r#"<root><anim name="a"><curve attr="pos"><key value="0,0,0"/><key frame60="60" value="10,0,0"/><key frame60="60" value="0,5,0"/><key frame60="120" value="0,15,0"/></curve></anim>
            <anim name="b"><curve attr="color" interpolate="easing_outcubic"><key value="0,0,0,0"/><key frame60="30" value="0,0,0,255"/></curve></anim></root>"#,
        )
        .unwrap();
        let a = Track::read(&anims, "a", "pos").unwrap();
        assert!(near(a.at(0.5)[0], 5.0));
        assert!(near(a.at(1.0)[1], 5.0), "the later key of a repeated frame holds from there");
        assert!(near(a.at(1.5)[1], 10.0));
        let b = Track::read(&anims, "b", "color").unwrap();
        assert!(near(b.at(0.25)[3], 255.0 * (1.0 - 0.5f32.powi(3))));
    }
}
