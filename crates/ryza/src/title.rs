//! Atelier Ryza DX's title screen, as it stands with no input: the logo, the pulsing "press any
//! button" prompt, the copyright line and the version string with its outline (`docs/formats.md`,
//! "Ryza's title layout"; the 3D backdrop and the menu behind the prompt are not here).
//!
//! The picture is laid out from the game's own data: `uil_a21_title.xml` places the nodes in a
//! 1280 x 720 layout, `uis_gen_a21_title.xml` says where each picture sits in `a21_title.g1t`
//! (a 2160-line atlas, so its pixels are a third of a layout pixel), and `uia_common.xml` holds
//! the animations (`anm_common_juvenile_blink`, `anm_common_slide_in_bottom`). Positions scale
//! by `screen height / 720` and the layout is centred across the screen's width.

use std::collections::HashMap;

use barrel_formats::table::{self, Element};
use barrel_render::text::{Glyph, Quad, FONT, UI};

use crate::font::ATLAS_SIZE;

/// The field the title draws behind its layer. The capture's G-buffer draws use textures found
/// only in this field's pack (`barrel-parity textures` over the archives' containers: 28 of the
/// scene's textures sit in this one pack and no other), and its draws reappear here with the same
/// textures and index counts (`diff3d`). Not `event_titleback`, the field the name suggests: its
/// terrain and textures do not match the capture.
pub const FIELD: &str = "s19_l_highway_03";

/// The title's camera, read off the capture's `mW2P` (the same at every captured frame, 300 to
/// 1900): the eye and forward vector (the depth row of the matrix) in the game's world (x not mirrored), a 45 degree vertical
/// field of view, near 50 and far 300000 (the projection's depth terms; the far is fitted to
/// 300073 +- the float noise of a 5-digit division, so 300000).
#[derive(Clone, Copy, Debug)]
pub struct Camera {
    pub eye: [f32; 3],
    pub forward: [f32; 3],
    /// Vertical field of view in degrees.
    pub fov: f32,
    pub near: f32,
    pub far: f32,
}

pub const CAMERA: Camera = Camera { eye: [29789.2, 6484.96, 41557.74], forward: [0.404478, 0.105262, -0.908470], fov: 45.0, near: 50.0, far: 300_000.0 };

impl Camera {
    /// The camera as the field renderer takes it: the look point in render space (x mirrored),
    /// then the yaw and pitch in degrees and the distance from that point back to the eye
    /// (`near` is a hundredth of it, as the renderer's own rule has it).
    pub fn look(&self) -> (glam::Vec3, f32, f32, f32) {
        let dist = self.near * 100.0;
        let eye = glam::Vec3::new(-self.eye[0], self.eye[1], self.eye[2]);
        let fwd = glam::Vec3::new(-self.forward[0], self.forward[1], self.forward[2]).normalize();
        let back = -fwd;
        (eye + fwd * dist, back.x.atan2(back.z).to_degrees(), back.y.asin().to_degrees(), dist)
    }
}

/// The layout's height in pixels; the atlas's pictures are three times this size.
const LAYOUT_HEIGHT: f32 = 720.0;
const ATLAS_PER_LAYOUT: f32 = 3.0;

/// How far above the layout's `pos.y` the version string's line top sits, in layout pixels.
/// Fitted to the capture (the first glyph row at 1034.5 on 1080 lines, 12 atlas pixels below the
/// line top: 686 layout lines for a node at 690); the game's own rule is not read.
const TEXT_TOP: f32 = -4.0;
/// The outline's alpha: the outline draws' `materialColor` in the capture is black at 0.5.
const OUTLINE_ALPHA: f32 = 0.5;
/// The patch level the capture was taken at (`Ver. 1.02`): the executable's patch table
/// (`Patch 1.01` ... `Patch 1.15`) lists the levels' strings, the level in force is not read.
pub const PATCH_VERSION: &str = "1.02";

/// A picture of the title's atlas placed in the layout.
#[derive(Clone, Debug, PartialEq)]
struct Image {
    /// Where the picture is in the atlas: left, top, width, height (2160-line pixels).
    uvwh: [f32; 4],
    /// Its top-left corner in the layout (the node positions added up) and its scale.
    pos: [f32; 2],
    scale: [f32; 2],
    /// A size in layout pixels that replaces the picture's own (`size_wh`).
    size: Option<[f32; 2]>,
}

/// The version string's text node.
#[derive(Clone, Debug, PartialEq)]
struct Text {
    /// The right edge and the node's top in the layout.
    pos: [f32; 2],
    font_size: f32,
    /// The outline's reach in layout pixels (`bold_size`).
    bold: f32,
}

/// One animation curve: keys of (seconds, value) and how the values are eased between them.
#[derive(Clone, Debug, PartialEq)]
pub struct Curve {
    keys: Vec<(f32, [f32; 4])>,
    easing: Easing,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Easing {
    Linear,
    InCubic,
    OutCubic,
}

impl Curve {
    /// The value at `t` seconds (held at the ends).
    pub fn at(&self, t: f32) -> [f32; 4] {
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

    /// The seconds the curve runs.
    pub fn duration(&self) -> f32 {
        self.keys.last().unwrap().0
    }
}

/// The animations of `uia_common.xml` the title uses: `name`'s curve for attribute `attr`.
/// Keys are `frame60` (frames at 60 a second, from the start) or none (the start); a key
/// that does not come after the one before it is dropped (the blink's third key repeats the
/// second's frame).
pub fn curve(anims: &[u8], name: &str, attr: &str) -> Result<Curve, String> {
    let root = table::parse(anims).map_err(|e| format!("uia_common: {e}"))?;
    let anim = root.children.iter().find(|e| e.name == "anim" && e.attr("name") == Some(name)).ok_or(format!("uia_common: no {name}"))?;
    let c = anim.children.iter().find(|e| e.name == "curve" && e.attr("attr") == Some(attr)).ok_or(format!("uia_common: {name} has no {attr}"))?;
    let easing = match c.attr("interpolate") {
        Some("easing_incubic") => Easing::InCubic,
        Some("easing_outcubic") => Easing::OutCubic,
        None => Easing::Linear,
        Some(other) => return Err(format!("uia_common: {name}: easing {other} (not read)")),
    };
    let mut keys: Vec<(f32, [f32; 4])> = Vec::new();
    for k in c.children.iter().filter(|e| e.name == "key") {
        let t = k.attr("frame60").and_then(|f| f.trim().parse::<f32>().ok()).map_or(0.0, |f| f / 60.0);
        let v: Vec<f32> = k.attr("value").unwrap_or("").split(',').filter_map(|v| v.trim().parse().ok()).collect();
        let mut value = [0.0f32; 4];
        for (d, s) in value.iter_mut().zip(&v) {
            *d = *s;
        }
        if keys.last().is_none_or(|l| t > l.0) {
            keys.push((t, value));
        }
    }
    if keys.len() < 2 {
        return Err(format!("uia_common: {name}'s {attr} has fewer than two keys"));
    }
    Ok(Curve { keys, easing })
}

fn numbers(e: &Element, attr: &str) -> Vec<f32> {
    e.attr(attr).unwrap_or("").split(',').filter_map(|v| v.trim().parse().ok()).collect()
}

fn pair(e: &Element, attr: &str, default: [f32; 2]) -> [f32; 2] {
    let v = numbers(e, attr);
    [v.first().copied().unwrap_or(default[0]), v.get(1).copied().unwrap_or(default[1])]
}

/// The layout's nodes by name, each with the positions of the nodes above it added up.
fn find<'a>(e: &'a Element, name: &str, at: [f32; 2]) -> Option<(&'a Element, [f32; 2])> {
    let here = pair(e, "pos", [0.0; 2]);
    let at = [at[0] + here[0], at[1] + here[1]];
    if e.attr("name") == Some(name) {
        return Some((e, at));
    }
    e.children.iter().find_map(|c| find(c, name, at))
}

/// A style's first picture (`gen_a21_title01_anybutton` is a list of two: the first is shown).
fn style(styles: &Element, name: &str) -> Option<[f32; 4]> {
    let image = |e: &Element| -> Option<[f32; 4]> { numbers(e, "uvwh").try_into().ok() };
    styles.children.iter().find_map(|e| match e.name.as_str() {
        "image" if e.attr("name") == Some(name) => image(e),
        "image_list" if e.attr("name") == Some(name) => e.children.first().and_then(image),
        _ => None,
    })
}

/// The title screen's layout and animations.
pub struct Title {
    logo: Image,
    prompt: Image,
    copyright: Image,
    version: Text,
    /// The prompt appears this many seconds into the title (`anim_ctrl up`'s `delay`).
    prompt_delay: f32,
    blink: Curve,
    slide_color: Curve,
    slide_pos: Curve,
    glyphs: HashMap<char, Glyph>,
    /// The title atlas and the font atlas, in pixels.
    atlas: [(f32, f32); 2],
}

impl Title {
    /// Reads the layout (`uil_a21_title.xml`), its styles (`uis_gen_a21_title.xml`) and the common
    /// animations (`uia_common.xml`); `title_atlas` and `font_atlas` are the sizes of
    /// `a21_title.g1t` and the font's atlas, `glyphs` the font's table.
    pub fn read(layout: &[u8], styles: &[u8], anims: &[u8], glyphs: HashMap<char, Glyph>, title_atlas: (u32, u32), font_atlas: (u32, u32)) -> Result<Title, String> {
        let layout = table::parse(layout).map_err(|e| format!("uil_a21_title: {e}"))?;
        let styles = table::parse(styles).map_err(|e| format!("uis_gen_a21_title: {e}"))?;
        let image = |name: &str| -> Result<Image, String> {
            let (e, at) = find(&layout, name, [0.0; 2]).ok_or(format!("uil_a21_title: no {name}"))?;
            let s = e.attr("style").ok_or(format!("uil_a21_title: {name} has no style"))?;
            let uvwh = style(&styles, s).ok_or(format!("uis_gen_a21_title: no style {s}"))?;
            // `scale` and `size_wh` belong to the image, `pos` is already in `at`.
            let scale = pair(e, "scale", [1.0, 1.0]);
            let size = e.attr("size_wh").is_some().then(|| pair(e, "size_wh", [0.0; 2]));
            Ok(Image { uvwh, pos: at, scale, size })
        };
        let (patch, at) = find(&layout, "patch_ver", [0.0; 2]).ok_or("uil_a21_title: no patch_ver")?;
        let version = Text {
            pos: at,
            font_size: patch.attr("font_size").and_then(|v| v.trim().parse().ok()).ok_or("uil_a21_title: patch_ver's font_size")?,
            bold: patch.attr("bold_size").and_then(|v| v.trim().parse().ok()).unwrap_or(0.0),
        };
        let (up, _) = find(&layout, "press_start_top", [0.0; 2]).ok_or("uil_a21_title: no press_start_top")?;
        let prompt_delay = up
            .children
            .iter()
            .find(|c| c.name == "anim_ctrl" && c.attr("name") == Some("up"))
            .and_then(|c| c.attr("delay"))
            .and_then(|d| d.trim().parse().ok())
            .ok_or("uil_a21_title: press_start_top has no `up` delay")?;
        Ok(Title {
            logo: image("logo")?,
            prompt: image("press_start_image")?,
            copyright: image("copyright")?,
            version,
            prompt_delay,
            blink: curve(anims, "anm_common_juvenile_blink", "color")?,
            slide_color: curve(anims, "anm_common_slide_in_bottom", "color")?,
            slide_pos: curve(anims, "anm_common_slide_in_bottom", "pos")?,
            glyphs,
            atlas: [(title_atlas.0 as f32, title_atlas.1 as f32), (font_atlas.0 as f32, font_atlas.1 as f32)],
        })
    }

    /// The prompt's opacity `t` seconds into the title, 0 before it appears: the slide-in's own
    /// fade (`anm_common_slide_in_bottom` sets the colour, which replaces the plain fade-in
    /// bound beside it) times the blink, a dip to half over `blink`'s run and back (the capture's
    /// pulse is symmetric about its dip, so the run is played forward and then back).
    pub fn prompt_alpha(&self, t: f32) -> f32 {
        let since = t - self.prompt_delay;
        if since < 0.0 {
            return 0.0;
        }
        // The dip and the way back are one loop, which takes a frame more than its length
        // (`boot::loop_time`: 91 frames for the 1.5 s).
        let d = self.blink.duration();
        let phase = crate::boot::loop_time((t * 60.0).round(), 2.0 * d, 1.0);
        let pulse = self.blink.at(if phase < d { phase } else { 2.0 * d - phase })[3] / 255.0;
        self.slide_color.at(since)[3] / 255.0 * pulse
    }

    /// The quads `t` seconds into the title on a `screen` of pixels, in draw order.
    pub fn quads(&self, t: f32, screen: (u32, u32)) -> Vec<Quad> {
        let k = screen.1 as f32 / LAYOUT_HEIGHT;
        let left = (screen.0 as f32 - 1280.0 * k) * 0.5;
        let place = |i: &Image, shift: [f32; 2], alpha: f32| -> Quad {
            let [x, y, w, h] = i.uvwh;
            let (w_px, h_px) = match i.size {
                Some([w, h]) => (w * k, h * k),
                None => (w / ATLAS_PER_LAYOUT * i.scale[0] * k, h / ATLAS_PER_LAYOUT * i.scale[1] * k),
            };
            let (l, top) = (left + (i.pos[0] + shift[0]) * k, (i.pos[1] + shift[1]) * k);
            let (aw, ah) = self.atlas[0];
            Quad { turn: false, spin: None, rect: [l, top, l + w_px, top + h_px], uv: [x / aw, y / ah, (x + w) / aw, (y + h) / ah], color: [1.0, 1.0, 1.0, alpha], texture: UI }
        };
        let mut out = vec![place(&self.logo, [0.0; 2], 1.0)];
        let alpha = self.prompt_alpha(t);
        if alpha > 0.0 {
            let slide = self.slide_pos.at(t - self.prompt_delay)[1];
            out.push(place(&self.prompt, [0.0, slide], alpha));
        }
        out.extend(self.version_quads(k, left));
        out.push(place(&self.copyright, [0.0; 2], 1.0));
        out
    }

    /// The version string, right-aligned at its node: the outline (the text black at half
    /// opacity set at each of the 3 x 3 offsets of the outline's reach, columns first), then the
    /// text white.
    fn version_quads(&self, k: f32, left: f32) -> Vec<Quad> {
        let text = format!("Ver. {PATCH_VERSION}");
        let s = self.version.font_size * k / ATLAS_SIZE;
        let width: f32 = text.chars().filter_map(|c| self.glyphs.get(&c)).map(|g| g.advance as f32 * s).sum();
        let (pen0, top) = (left + self.version.pos[0] * k - width, (self.version.pos[1] + TEXT_TOP) * k);
        let (aw, ah) = self.atlas[1];
        let run = |dx: f32, dy: f32, color: [f32; 4]| -> Vec<Quad> {
            let mut pen = pen0 + dx;
            let mut quads = Vec::new();
            for g in text.chars().filter_map(|c| self.glyphs.get(&c)) {
                let l = pen + g.x_offset as f32 * s;
                let t = top + dy + g.y_offset as f32 * s;
                let (x, y, w, h) = (g.x as f32, g.y as f32, g.width as f32, g.height as f32);
                quads.push(Quad { turn: false, spin: None, rect: [l, t, l + w * s, t + h * s], uv: [x / aw, y / ah, (x + w) / aw, (y + h) / ah], color, texture: FONT });
                pen += g.advance as f32 * s;
            }
            quads
        };
        let reach = self.version.bold * k;
        let mut out = Vec::new();
        for dx in [-reach, 0.0, reach] {
            for dy in [-reach, 0.0, reach] {
                out.extend(run(dx, dy, [0.0, 0.0, 0.0, OUTLINE_ALPHA]));
            }
        }
        out.extend(run(0.0, 0.0, [1.0; 4]));
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ANIMS: &[u8] = br#"<root>
        <anim name="anm_common_juvenile_blink"><curve attr="color" interpolate="easing_incubic">
            <key value="255, 255, 255, 255"/><key frame60="45" value="255, 255, 255, 127"/><key frame60="45" value="255, 255, 255, 255"/></curve></anim>
        <anim name="anm_common_slide_in_bottom">
            <curve attr="color" interpolate="easing_outcubic"><key value="255, 255, 255, 0"/><key frame60="30" value="255, 255, 255, 255"/></curve>
            <curve attr="pos" interpolate="easing_outcubic"><key value="0, 30, 0"/><key frame60="30" value="0, 0, 0"/></curve></anim></root>"#;
    const LAYOUT: &[u8] = br#"<root>
        <image pos="315, 55, 0" style="logo_style" name="logo" scale="0.95, 0.95, 1.00"/>
        <node name="press_start_top" pos="0, 0, 0"><node name="press_start" pos="-2, -30, 0">
            <image name="press_start_image" pos="456.33, 459, 0" style="prompt_style"/></node>
            <anim_ctrl name="up" delay="2.00"/></node>
        <node name="patch" pos="1241, 690, 0"><text name="patch_ver" font_size="11.00" bold_size="1.00"/></node>
        <image pos="491, 664, 0" style="copy_style" name="copyright" size_wh="300, 18, 0"/></root>"#;
    const STYLES: &[u8] = br#"<root>
        <image name="logo_style" uvwh="4,21,2040,1033"/>
        <image_list name="prompt_style"><image uvwh="11,1779,1099,112"/><image uvwh="11,1914,1099,112"/></image_list>
        <image name="copy_style" uvwh="151,2487,1044,63"/></root>"#;

    fn glyphs() -> HashMap<char, Glyph> {
        let g = |x, y, width, height, x_offset, y_offset, advance| Glyph { x, y, width, height, x_offset, y_offset, advance };
        [('V', g(2129, 523, 29, 34, -1, 12, 28)), ('e', g(480, 600, 22, 25, 0, 20, 24))].into_iter().collect()
    }

    fn title() -> Title {
        Title::read(LAYOUT, STYLES, ANIMS, glyphs(), (2048, 2560), (4096, 2048)).unwrap()
    }

    fn near(a: f32, b: f32, tol: f32) -> bool {
        (a - b).abs() <= tol
    }

    #[test]
    fn the_logo_prompt_and_copyright_sit_where_the_capture_has_them() {
        let t = title();
        let q = t.quads(10.0, (1920, 1080));
        // Logo (472.5, 82.5)-(1441.5, 573.18), uv (0.002, 0.0082)-(0.998, 0.4117).
        assert!(q[0].rect.iter().zip([472.5, 82.5, 1441.5, 573.18]).all(|(a, b)| near(*a, b, 0.01)), "{:?}", q[0].rect);
        assert!(near(q[0].uv[0], 4.0 / 2048.0, 1e-6) && near(q[0].uv[3], 1054.0 / 2560.0, 1e-6));
        // Prompt (681.49, 643.5)-(1230.99, 699.5), the first picture of its list.
        assert!(q[1].rect.iter().zip([681.495, 643.5, 1230.995, 699.5]).all(|(a, b)| near(*a, b, 0.01)), "{:?}", q[1].rect);
        assert!(near(q[1].uv[1], 1779.0 / 2560.0, 1e-6));
        // Copyright (736.5, 996)-(1186.5, 1023): `size_wh` replaces the picture's size.
        let c = q.last().unwrap();
        assert_eq!((c.rect, c.texture), ([736.5, 996.0, 1186.5, 1023.0], UI));
    }

    #[test]
    fn the_version_string_is_outlined_nine_times_and_then_drawn() {
        let q = title().quads(10.0, (1920, 1080));
        let text: Vec<&Quad> = q.iter().filter(|q| q.texture == FONT).collect();
        // "Ver. 1.02" has only V and e here: the glyphs without a table entry are skipped.
        assert_eq!(text.len(), 10 * 2);
        assert_eq!(text[0].color, [0.0, 0.0, 0.0, 0.5]);
        assert_eq!(text[18].color, [1.0; 4]);
        // The first outline draw sits one outline reach (1.5) up and left of the text's.
        assert!(near(text[18].rect[0] - text[0].rect[0], 1.5, 1e-3) && near(text[18].rect[1] - text[0].rect[1], 1.5, 1e-3));
        // 'V' is 29 atlas pixels at 11 * 1.5 / 36 = 0.4583: 13.29 wide.
        assert!(near(text[0].rect[2] - text[0].rect[0], 13.2917, 1e-3));
    }

    #[test]
    fn the_prompt_slides_in_after_its_delay_and_pulses() {
        let t = title();
        assert_eq!(t.prompt_alpha(1.9), 0.0);
        // Half a second into the slide it is fully faded in (the blink aside).
        assert!(near(t.prompt_alpha(2.5), t.blink.at(0.5 % 0.75)[3] / 255.0, 0.05));
        // The blink: full at its start, half 0.75 s on and back after another 0.75; the 1.5 s
        // loop takes 91 frames, so by 3 s (180 frames) it is one frame behind.
        let at = |frames: f32| t.prompt_alpha(frames / 60.0);
        assert!(near(at(182.0), 1.0, 1e-3) && near(at(182.0 + 45.0), 127.0 / 255.0, 1e-3) && near(at(273.0), 1.0, 1e-3));
        // Early in the slide the prompt is still low on the screen.
        let q = t.quads(2.1, (1920, 1080));
        assert!(q[1].rect[1] > 643.5 + 20.0);
    }

    #[test]
    fn easing_curves_read_from_the_animation_file() {
        let c = curve(ANIMS, "anm_common_slide_in_bottom", "pos").unwrap();
        assert_eq!(c.at(0.0), [0.0, 30.0, 0.0, 0.0]);
        assert_eq!(c.at(1.0), [0.0, 0.0, 0.0, 0.0]);
        assert!(curve(ANIMS, "anm_common_slide_in_bottom", "rot").is_err());
        // The blink's repeated third key is dropped: it runs 0.75 s.
        assert_eq!(curve(ANIMS, "anm_common_juvenile_blink", "color").unwrap().duration(), 0.75);
    }

    #[test]
    fn camera_look_puts_the_eye_back_at_the_mirrored_game_eye() {
        let (center, yaw, pitch, dist) = CAMERA.look();
        let (y, p) = (yaw.to_radians(), pitch.to_radians());
        // The renderer's own rule (`look_for`): eye = center + (sin y cos p, sin p, cos y cos p) * dist.
        let eye = center + glam::Vec3::new(y.sin() * p.cos(), p.sin(), y.cos() * p.cos()) * dist;
        let want = glam::Vec3::new(-CAMERA.eye[0], CAMERA.eye[1], CAMERA.eye[2]);
        assert!(eye.distance(want) < 0.05, "{eye:?} vs {want:?}");
        // The look direction is the mirrored forward vector (a unit one).
        let f = (center - eye).normalize();
        assert!((f.x + CAMERA.forward[0]).abs() < 1e-4 && (f.y - CAMERA.forward[1]).abs() < 1e-4 && (f.z - CAMERA.forward[2]).abs() < 1e-4);
        assert_eq!(dist * 0.01, CAMERA.near);
    }
}
