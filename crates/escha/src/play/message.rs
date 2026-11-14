//! Escha & Logy DX's text on screen: its markup (`<CR>` breaks, `<IMnn>` button icons,
//! `<CLxx>` colours), its English line breaking and the message window under a line of an
//! event, laid out into the text layer's quads (`barrel_render::text`).
//!
//! The message window is drawn from the game's own pieces (`WinXls_EN/a15_common.g1t`): its
//! parchment panel, name tag and cord; how they are put together (sizes, places, colours) is
//! measured from an oracle frame, as the game's layout code is not read.

use barrel_render::text::{Font, Quad, FONT, UI};

/// The font's line height in atlas pixels (the Latin glyphs reach 53 below a line's top; the
/// game's own spacing is not read).
pub const LINE: f32 = 53.0;

/// The text colours the markup names (`<CLGR>` ...), from the executable's table at
/// `0x1407b49b0` (English; 16-byte records `{u64 name, u8 r, g, b, 0}`, in this order).
pub const COLOURS: [(&str, [u8; 3]); 11] = [
    ("NR", [0xff, 0xff, 0xff]),
    ("BK", [0x00, 0x00, 0x00]),
    ("RE", [0xc8, 0x00, 0x00]),
    ("GR", [0x00, 0xaa, 0x00]),
    ("BL", [0x50, 0x50, 0xf0]),
    ("R1", [0xff, 0x50, 0x50]),
    ("G1", [0x0a, 0xf0, 0x00]),
    ("P1", [0x88, 0x22, 0x44]),
    ("YL", [0xff, 0xff, 0x00]),
    ("EG", [0x1e, 0xff, 0x1e]),
    ("BR", [0x3a, 0x1a, 0x08]),
];

/// The first button icon's character: `<IMnn>` is the font's glyph for U+0410 + nn (codes
/// `0xd090` on, which would be Cyrillic А to а; the game uses them for its 33 icons).
pub const ICONS: u32 = 0x410;
/// The icon drawn for `<IMnn>` with the confirm button on A (the Western layout the English
/// release shows, and Barrel's): the font's glyphs 0 and 1 are B, 2 and 3 A, and the text
/// names them by role (00/01 confirm, 02/03 cancel, the Japanese layout), so the two pairs
/// swap: the maps' "<IM02> Return" shows B, as the recording does. So do the next two pairs
/// (4 and 5 Y, 6 and 7 X; the button sprite part 0x41's cuts in the same order): the world
/// map's "<IM04> Area Info" and the area map scroll's button (cut 4) show X in the
/// colseit-arrival recording.
pub fn western(n: u32) -> u32 {
    match n {
        0..=1 | 4..=5 => n + 2,
        2..=3 | 6..=7 => n - 2,
        _ => n,
    }
}

/// Colour changes ride in the laid-out text as these private-use characters (U+E000 + the
/// colour's place in [`COLOURS`]; U+E0FF goes back to the line's own colour). They have no
/// glyph, so they take no room.
const COLOUR_MARK: u32 = 0xe000;
const COLOUR_RESET: u32 = 0xe0ff;

fn colour(rgb: [u8; 3]) -> [f32; 4] {
    [f32::from(rgb[0]) / 255.0, f32::from(rgb[1]) / 255.0, f32::from(rgb[2]) / 255.0, 1.0]
}

/// The message window pieces' rectangles in `a15_common.g1t` texture 4 (1024 × 1024), and
/// where the game draws them on a 1280 × 720 screen (measured from an oracle frame of
/// `EVENT_MM01_OP1_111`, `docs/renderer.md`, "Dialogue").
pub mod window {
    pub const SIZE: f32 = 1024.0;
    /// The framed parchment panel.
    pub const PANEL: [f32; 4] = [0.0, 532.0, 757.0, 738.0];
    /// The brown name tag with its grommet.
    pub const TAG: [f32; 4] = [0.0, 298.0, 408.0, 372.0];
    /// The cord hanging from the tag's grommet.
    pub const CORD: [f32; 4] = [585.0, 2.0, 635.0, 260.0];
    /// On the 1280 × 720 screen: the panel, the tag (drawn level; the game tilts it about 5°),
    /// the cord, the text's left edge and first line's top, the text's widest line, and the
    /// name's centre.
    pub const AT_PANEL: [f32; 4] = [360.0, 562.0, 960.0, 710.0];
    pub const AT_TAG: [f32; 4] = [352.0, 535.0, 627.0, 585.0];
    pub const AT_CORD: [f32; 4] = [340.0, 548.0, 374.0, 722.0];
    pub const AT_TEXT: [f32; 2] = [409.0, 596.0];
    pub const TEXT_WIDTH: f32 = 540.0;
    pub const AT_NAME: [f32; 2] = [505.0, 560.0];
    /// The game's first line, "You haven't forgotten anything, have you, Escha?", spans 482
    /// pixels there: the text is sized to match.
    pub const SAMPLE: &str = "You haven't forgotten anything, have you, Escha?";
    pub const SAMPLE_WIDTH: f32 = 482.0;
}

/// The most characters on one line of the message window, as the game breaks its English
/// text (measured in the openings: docs/renderer.md, "Message window").
pub const LINE_CHARS: usize = 48;

/// Breaks `line` into lines of at most [`LINE_CHARS`] characters and `max` atlas pixels: at
/// spaces, or anywhere in a word longer than a line (Japanese has no spaces).
pub fn wrap(font: &Font, line: &str, max: f32) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for word in line.split(' ') {
        let candidate = if cur.is_empty() { word.to_string() } else { format!("{cur} {word}") };
        // The game breaks English lines by count, at most LINE_CHARS characters (it keeps
        // "…, have you, Escha?", 48 and 480 pixels, yet moves "trip," after "…for a long",
        // 54 and 464 pixels); the width only guards wide glyphs.
        if candidate.chars().count() <= LINE_CHARS && font.width(&candidate) <= max {
            cur = candidate;
            continue;
        }
        if !cur.is_empty() {
            out.push(std::mem::take(&mut cur));
        }
        for c in word.chars() {
            if !cur.is_empty() && font.width(&cur) + font.advance(c) > max {
                out.push(std::mem::take(&mut cur));
            }
            cur.push(c);
        }
    }
    if !cur.is_empty() || out.is_empty() {
        out.push(cur);
    }
    out
}

/// Glyph quads for `s` with its line's top-left at (x, y), `scale` screen pixels per atlas
/// pixel.
pub fn run(font: &Font, s: &str, x: f32, y: f32, scale: f32, base: [f32; 4], out: &mut Vec<Quad>) {
    let mut pen = x;
    let mut color = base;
    for c in s.chars() {
        match u32::from(c) {
            COLOUR_RESET => color = base,
            // `<CLNR>` (normal) is the line's own colour too: white is for dark windows.
            n if n == COLOUR_MARK => color = base,
            n if (COLOUR_MARK..COLOUR_MARK + COLOURS.len() as u32).contains(&n) => color = colour(COLOURS[(n - COLOUR_MARK) as usize].1),
            _ => {}
        }
        let Some(g) = font.glyphs.get(&c) else { continue };
        // Icons keep their own colours.
        let tint = if (ICONS..ICONS + 33).contains(&u32::from(c)) { [1.0; 4] } else { color };
        if g.width > 3 {
            let (gx, gy, gw, gh) = (f32::from(g.x), f32::from(g.y), f32::from(g.width), f32::from(g.height));
            let left = pen + g.x_offset as f32 * scale;
            let top = y + g.y_offset as f32 * scale;
            out.push(Quad { turn: false,
                rect: [left, top, left + gw * scale, top + gh * scale],
                uv: [gx / font.size, gy / font.size, (gx + gw) / font.size, (gy + gh) / font.size],
                color: tint,
                texture: FONT,
            });
        }
        pen += g.advance as f32 * scale;
    }
}

/// The widest line of layout text `text`, in atlas pixels.
pub fn width_of(font: &Font, text: &str) -> f32 {
    plain_lines(text).iter().map(|l| font.width(l)).fold(0.0, f32::max)
}

/// One line of layout text (`<IMnn>` icons and `<CLxx>` colours read) with its top-left at
/// (x, y), `scale` screen pixels per atlas pixel.
pub fn line(font: &Font, text: &str, x: f32, y: f32, scale: f32, colour: [f32; 4], out: &mut Vec<Quad>) {
    let mut y = y;
    for l in plain_lines(text) {
        run(font, &l, x, y, scale, colour, out);
        y += LINE * scale;
    }
}

/// The message window for a line on a `width` × `height` screen, as the game lays it out
/// (`window`): the parchment panel, the name tag and its cord, the name, and the text,
/// wrapped to the panel. The 1280 × 720 layout is scaled to the height and centred.
pub fn dialogue(font: &Font, speaker: Option<&str>, text: &str, width: f32, height: f32) -> Vec<Quad> {
    use window::*;
    let k = height / 720.0;
    let dx = (width - 1280.0 * k) / 2.0;
    let place = |r: [f32; 4]| [dx + r[0] * k, r[1] * k, dx + r[2] * k, r[3] * k];
    let uv = |r: [f32; 4]| r.map(|v| v / SIZE);
    let white = [1.0; 4];
    // Text at the size the game's sample line has (in screen pixels per atlas pixel).
    let text_scale = SAMPLE_WIDTH / font.width(SAMPLE).max(1.0) * k;
    let lines: Vec<String> = plain_lines(text).iter().flat_map(|l| wrap(font, l, TEXT_WIDTH * k / text_scale)).collect();
    let mut out = vec![Quad { turn: false, rect: place(AT_PANEL), uv: uv(PANEL), color: white, texture: UI }];
    if let Some(name) = speaker.filter(|n| !n.is_empty()) {
        out.push(Quad { turn: false, rect: place(AT_CORD), uv: uv(CORD), color: white, texture: UI });
        out.push(Quad { turn: false, rect: place(AT_TAG), uv: uv(TAG), color: white, texture: UI });
        let name_scale = text_scale * 0.95;
        let (cx, cy) = (dx + AT_NAME[0] * k, AT_NAME[1] * k);
        let w = font.width(name) * name_scale;
        run(font, name, cx - w / 2.0, cy - LINE * name_scale * 0.5, name_scale, [0.96, 0.93, 0.86, 1.0], &mut out);
    }
    let mut y = AT_TEXT[1] * k;
    for l in &lines {
        // The game's brown (`BR`) on the parchment.
        run(font, l, dx + AT_TEXT[0] * k, y, text_scale, colour(COLOURS[10].1), &mut out);
        y += LINE * text_scale;
    }
    out
}

/// A message's text as lines ready to lay out: `<CR>` breaks a line, `<IMnn>` becomes the
/// icon's character ([`ICONS`]), `<CLxx>` a colour mark, and other markup is left out.
pub fn plain_lines(text: &str) -> Vec<String> {
    text.split("<CR>")
        .map(|l| {
            let mut out = String::new();
            let mut rest = l;
            while let Some(i) = rest.find('<') {
                out.push_str(&rest[..i]);
                let Some(j) = rest[i..].find('>') else {
                    rest = &rest[i..];
                    break;
                };
                let tag = &rest[i + 1..i + j];
                if let Some(n) = tag.strip_prefix("IM").and_then(|n| n.parse::<u32>().ok()) {
                    out.extend(char::from_u32(ICONS + western(n)));
                } else if let Some(name) = tag.strip_prefix("CL") {
                    let mark = COLOURS.iter().position(|(c, _)| *c == name).map_or(COLOUR_RESET, |k| COLOUR_MARK + k as u32);
                    out.extend(char::from_u32(mark));
                } else if tag.chars().any(|c| !c.is_ascii_alphanumeric()) {
                    // Not markup ("x < y > z").
                    out.push_str(&rest[i..i + j + 1]);
                }
                rest = &rest[i + j + 1..];
            }
            out.push_str(rest);
            out
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use barrel_render::text::Glyph;

    fn font() -> Font {
        let g = |advance| Glyph { x: 0, y: 0, width: 10, height: 20, x_offset: 0, y_offset: 10, advance };
        let glyphs = "abcdefghijklmnopqrstuvwxyz".chars().map(|c| (c, g(10))).chain([(' ', g(10)), ('あ', g(40))]).collect();
        Font { glyphs, size: 2048.0 }
    }

    #[test]
    fn markup_and_breaks() {
        assert_eq!(plain_lines("Hello<CR><CLGR>there<CLNR>!"), ["Hello", "\u{e003}there\u{e000}!"]);
        assert_eq!(plain_lines("Press <IM15> now"), ["Press \u{41f} now"]);
        assert_eq!(plain_lines("a<XX>b<CLZZ>c"), ["ab\u{e0ff}c"]);
        assert_eq!(plain_lines("x < y"), ["x < y"]);
        assert_eq!(plain_lines("x < y > z"), ["x < y > z"]);
    }

    #[test]
    fn colours_and_icons() {
        let mut f = font();
        f.glyphs.insert('\u{410}', Glyph { x: 0, y: 0, width: 10, height: 20, x_offset: 0, y_offset: 10, advance: 10 });
        let mut out = Vec::new();
        // <IM02> (cancel) draws glyph 0 (B) with the confirm button on A (`western`).
        let line = &plain_lines("a<CLGR>b<CLNR>c<IM02>")[0];
        run(&f, line, 0.0, 0.0, 1.0, [0.5; 4], &mut out);
        let colours: Vec<[f32; 4]> = out.iter().map(|q| q.color).collect();
        assert_eq!(colours, [[0.5; 4], colour([0, 0xaa, 0]), [0.5; 4], [1.0; 4]]);
        // Marks take no room: the four glyphs sit 10 apart.
        assert_eq!(out.iter().map(|q| q.rect[0]).collect::<Vec<_>>(), [0.0, 10.0, 20.0, 30.0]);
    }

    #[test]
    fn wrapping() {
        let f = font();
        // Each letter and space is 10 wide: "abc de" is 60.
        assert_eq!(wrap(&f, "abc de fgh", 60.0), ["abc de", "fgh"]);
        assert_eq!(wrap(&f, "abcdefgh", 30.0), ["abc", "def", "gh"]);
        assert_eq!(wrap(&f, "あああ", 80.0), ["ああ", "あ"]);
        assert_eq!(wrap(&f, "", 80.0), [""]);
        let long = "...Finally, I'm here. I knew I was in for a long trip, it was farther than I expected.";
        assert_eq!(wrap(&f, long, 1e9)[0], "...Finally, I'm here. I knew I was in for a long");
    }

    #[test]
    fn dialogue_box() {
        let f = font();
        let q = dialogue(&f, Some("ab"), "abc<CR>d", 1920.0, 1080.0);
        // The panel, the cord and the tag, two name glyphs, four text glyphs.
        assert_eq!(q.len(), 3 + 2 + 4);
        assert!(q[..3].iter().all(|q| q.texture == UI));
        assert!(q[3..].iter().all(|q| q.texture == FONT));
        // At 1080 lines the 720 layout is 1.5 times as large: the panel from x 540 (360 × 1.5).
        assert_eq!(q[0].rect[0], 540.0);
        // The text's lines are a (scaled) line apart, inside the panel.
        let scale = window::SAMPLE_WIDTH / f.width(window::SAMPLE) * 1.5;
        assert!((q[8].rect[1] - q[5].rect[1] - LINE * scale).abs() < 1e-3);
        assert!(q[5].rect[1] > q[0].rect[1] && q[8].rect[3] < q[0].rect[3]);
    }
}
