//! The title screen's menu: what follows a button at the prompt (`docs/ryza/open.md`, "The
//! title's menu"). The prompt fades out, the menu's items slide up one after another, then the
//! cursor spins in beside the item it starts on and that item pulses.
//!
//! The pictures, places and animations are the layout's (`uil_a21_title.xml`, the node
//! `title_menu`); which items show, where they sit and when each thing starts are the game's own
//! rules, read from its code (the addresses are in the docs).

use barrel_formats::table::{self, Element};
use barrel_render::text::{Quad, Spin, UI};

use crate::boot::{loop_time, FRAME};
use crate::title::{curve, find, numbers, pair, Curve, ATLAS_PER_LAYOUT, LAYOUT_HEIGHT};

/// The menu's items in the game's order (its table of `menu/<name>` nodes).
const NAMES: [&str; 6] = ["new_game", "load_game", "setting", "extra", "episode", "end_game"];
const LOAD_GAME: usize = 1;
const EXTRA: usize = 3;
/// How many items the list is centred for (the game's 6.0).
const SLOTS: f32 = 6.0;
/// The seconds the prompt's fade runs before the menu comes up (the game waits until its timer
/// passes 0.25).
const MENU_WAIT: f32 = 0.25;
/// The seconds between one item's slide-in and the next's (the game's 0.08, on the animation's
/// own clock).
const ITEM_STAGGER: f32 = 0.08;
// STAND-IN: The cursor's travel time is measured (six frames whatever the distance); the real thing is the list control's own value.
const TRAVEL: f32 = 6.0;
// STAND-IN: A held direction's repeat (37 frames, then every 15, stopping at the list's ends) is measured; the real thing is the list control's own repeat rule.
const REPEAT_FIRST: i32 = 37;
const REPEAT_NEXT: i32 = 15;
// STAND-IN: The eye's distance from the layout (a 30 degree view over its 720 lines) is fitted to the cursor's spin; the real thing is the 2D layer's own projection.
/// A node turned out of the screen's plane is seen from this far, straight on to the layout's
/// centre: the cursor's corners through its spin fit 1340 within 0.01 pixels.
const EYE: f32 = 1343.5;
// STAND-IN: The prompt's fade starts two steps in because the capture has it there; the real thing is how the game starts a reversed animation.
const FADE_AHEAD: f32 = 2.0;

/// One of the menu's items: its two pictures (plain, selected) and its place.
#[derive(Clone, Debug, PartialEq)]
struct Item {
    /// Left, top, width, height in the atlas (2160-line pixels).
    pictures: [[f32; 4]; 2],
    /// Its left edge in the layout and its own top within the menu (the first two items' tops
    /// give the list's start and spacing).
    pos: [f32; 2],
    /// Its size in layout pixels.
    size: [f32; 2],
}

/// One picture of the cursor: where it is in the cursor's atlas, its size in the layout and the
/// point it turns about (from its own top-left).
#[derive(Clone, Debug, PartialEq)]
struct Icon {
    uvwh: [f32; 4],
    size: [f32; 2],
    pivot: [f32; 2],
    /// Which way `anm_rotate` turns it: 1, -1 (bound reversed) or 0 (not bound).
    turns: f32,
}

/// A direction held on the pad or keys, in title frames: from the frame it went down to the
/// frame it came up (`None` while still down).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hold {
    pub from: i32,
    pub until: Option<i32>,
    /// 1 down the list, -1 up.
    pub way: i32,
}

/// What the player has done on the title, in title frames (60 a second from its start).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Input {
    /// A save exists: Load Game shows, and the cursor starts on it.
    pub saves: bool,
    /// The extras are unlocked.
    pub extra: bool,
    /// The frame of the button at the prompt.
    pub press: Option<i32>,
    pub holds: Vec<Hold>,
    /// The frame of the button on the menu's item under the cursor.
    pub choose: Option<i32>,
    /// On the load screen: the directions and the frame of the button on a save.
    pub list_holds: Vec<Hold>,
    pub confirm: Option<i32>,
}

/// The frames the chosen item takes to fade out once chosen, and the share of its pulse's opacity
/// it starts from: the opacity is that share times the pulse where it stood at the button times
/// 1 - frames / 13 (196, 121 and 45 of 255 on the capture's frames 0, 5 and 10 after the button
/// on Load Game, the pulse at .879; 188, 115 and 43 on Settings, at .836).
// STAND-IN: The fade's length (13 frames), its straight line and its start (.878 of the pulse) are fitted to those six values (each within 1.5 / 255); the real thing is the item's own leave animation.
pub const CHOSEN_FADE: i32 = 13;
const CHOSEN_START: f32 = 0.878;

/// The menu's layout and animations.
#[derive(Clone, Debug, PartialEq)]
pub struct Menu {
    /// The layout's top of the node the items and the cursor's line hang from.
    top: f32,
    items: Vec<Item>,
    /// The speed the items' `up2` plays at.
    up_speed: f32,
    /// The cursor node's left in the layout, and the pictures' offset from it.
    selector: f32,
    offset: [f32; 2],
    icons: Vec<Icon>,
    slide_color: Curve,
    slide_pos: Curve,
    in_color: Curve,
    in_pos: Curve,
    /// The cursor's turn about the upright axis as it comes in, and the speed it plays at.
    in_turn: Curve,
    in_turn_speed: f32,
    rotate: Curve,
    blink: Curve,
    /// The prompt's fade (played backwards) and the speed it plays at.
    fade: Curve,
    fade_speed: f32,
    /// The seconds into the title from which a button counts: the prompt's slide-in is over.
    ready: f32,
    /// The title atlas and the cursor's atlas in 2160-line pixels, and the cursor's texture in
    /// the layer.
    atlas: [(f32, f32); 2],
    cursor_texture: usize,
}

/// The cursor's side of what `Menu::read` takes: its styles (`uis_gen_a21_balloon_frame.xml`),
/// its animations (`uia_a21_balloon_frame.xml`), its atlas's size in the styles' pixels and its
/// texture's place in the layer.
pub struct Cursor<'a> {
    pub styles: &'a [u8],
    pub anims: &'a [u8],
    pub atlas: (u32, u32),
    pub texture: usize,
}

fn speed(node: &Element, ctrl: &str, anim: Option<&str>) -> Option<f32> {
    let c = node.children.iter().find(|c| c.name == "anim_ctrl" && c.attr("name") == Some(ctrl))?;
    let of = |e: &Element| e.attr("speed").and_then(|s| s.trim().parse().ok());
    match anim {
        None => Some(of(c).unwrap_or(1.0)),
        Some(a) => Some(c.children.iter().find(|b| b.attr("anim") == Some(a)).and_then(of).unwrap_or(1.0)),
    }
}

fn pictures(styles: &Element, name: &str) -> Option<Vec<[f32; 4]>> {
    let image = |e: &Element| -> Option<[f32; 4]> { numbers(e, "uvwh").try_into().ok() };
    styles.children.iter().find_map(|e| match e.name.as_str() {
        "image" if e.attr("name") == Some(name) => Some(vec![image(e)?]),
        "image_list" if e.attr("name") == Some(name) => e.children.iter().map(image).collect(),
        _ => None,
    })
}

/// The frames a wait of `seconds` takes: the game adds a frame's time to a 32-bit float each
/// frame and goes on once the sum is past the wait.
fn wait_frames(seconds: f32) -> i32 {
    let (mut t, mut n) = (0.0f32, 0);
    while t <= seconds {
        t += FRAME;
        n += 1;
    }
    n
}

/// The time shown `ticks` frames into a loop of `duration` seconds: as `boot::loop_time`, but
/// on the frame the sum passes the end what is past it is shown (the next frame starts from 0:
/// the cursor's 240-frame turn shows its first step twice running, once each side of the wrap).
fn loop_shown(ticks: i32, duration: f32) -> f32 {
    let (mut t, mut shown) = (0.0f32, 0.0f32);
    for _ in 0..ticks.max(0) {
        t += FRAME;
        shown = t;
        if t >= duration {
            shown = t - duration;
            t = 0.0;
        }
    }
    shown
}

impl Menu {
    /// Reads the menu out of the title's layout, styles and the common animations, with the
    /// cursor's own files; `title_atlas` is the title atlas's size in the styles' pixels.
    pub fn read(layout: &[u8], styles: &[u8], anims: &[u8], cursor: Cursor, title_atlas: (u32, u32)) -> Result<Menu, String> {
        let layout = table::parse(layout).map_err(|e| format!("uil_a21_title: {e}"))?;
        let styles = table::parse(styles).map_err(|e| format!("uis_gen_a21_title: {e}"))?;
        let cursor_styles = table::parse(cursor.styles).map_err(|e| format!("uis_gen_a21_balloon_frame: {e}"))?;
        let node = |name: &str| find(&layout, name, [0.0; 2]).ok_or(format!("uil_a21_title: no {name}"));
        let (menu, menu_at) = node("menu")?;
        let mut items = Vec::new();
        for name in NAMES {
            let (e, at) = find(menu, name, [0.0; 2]).ok_or(format!("uil_a21_title: no menu/{name}"))?;
            let s = e.attr("style").ok_or(format!("uil_a21_title: {name} has no style"))?;
            let p = pictures(&styles, s).filter(|p| p.len() >= 2).ok_or(format!("uis_gen_a21_title: {s} is not a list of two"))?;
            // `offset_uvwh` moves the picture in the atlas, in layout pixels.
            let mut shift = numbers(e, "offset_uvwh");
            shift.resize(4, 0.0);
            let moved = |q: [f32; 4]| std::array::from_fn(|i| q[i] + shift[i] * ATLAS_PER_LAYOUT);
            // The picture's own size: the menu's setup picks each item's picture, which sets
            // its size anew (Load Game's `size_wh` in the layout is not what is drawn).
            let size = [p[0][2] / ATLAS_PER_LAYOUT, p[0][3] / ATLAS_PER_LAYOUT];
            // `find` from the menu node adds the menu's own place in: only the item's is wanted
            let own = pair(menu, "pos", [0.0; 2]);
            items.push(Item { pictures: [moved(p[0]), moved(p[1])], pos: [menu_at[0] + at[0] - own[0], at[1] - own[1]], size });
        }
        let (selector, selector_at) = node("selector")?;
        let (prompt, _) = node("prompt")?;
        let mut icons = Vec::new();
        let turn = prompt.children.iter().find(|c| c.name == "anim_ctrl" && c.attr("name") == Some("anim_rotate"));
        for e in prompt.children.iter().filter(|c| c.name == "image") {
            let s = e.attr("style").ok_or("uil_a21_title: a cursor picture has no style")?;
            let uvwh = pictures(&cursor_styles, s).and_then(|p| p.first().copied()).ok_or(format!("uis_gen_a21_balloon_frame: no style {s}"))?;
            let bound = turn.and_then(|t| t.children.iter().find(|b| b.attr("node") == e.attr("name")));
            let turns = match bound {
                Some(b) if b.attr("reverse") == Some("true") => -1.0,
                Some(_) => 1.0,
                None => 0.0,
            };
            let size = [uvwh[2] / ATLAS_PER_LAYOUT, uvwh[3] / ATLAS_PER_LAYOUT];
            icons.push(Icon { uvwh, size, pivot: pair(e, "pivot_pos", [0.0; 2]), turns });
        }
        let (press, _) = node("press_start")?;
        let (up, _) = node("press_start_top")?;
        let prompt_delay: f32 = up.children.iter().find(|c| c.name == "anim_ctrl" && c.attr("name") == Some("up")).and_then(|c| c.attr("delay")).and_then(|d| d.trim().parse().ok()).unwrap_or(0.0);
        let slide_pos = curve(anims, "anm_common_slide_in_bottom", "pos")?;
        Ok(Menu {
            top: menu_at[1],
            up_speed: speed(find(menu, NAMES[0], [0.0; 2]).unwrap().0, "up2", None).ok_or("uil_a21_title: new_game has no up2")?,
            items,
            selector: selector_at[0],
            offset: pair(prompt, "pos", [0.0; 2]),
            icons,
            slide_color: curve(anims, "anm_common_slide_in_bottom", "color")?,
            in_color: curve(anims, "anm_common_slide_in_left", "color")?,
            in_pos: curve(anims, "anm_common_slide_in_left", "pos")?,
            in_turn: curve(anims, "anm_common_rotateY360", "rot")?,
            in_turn_speed: speed(selector, "in", Some("anm_common_rotateY360")).ok_or("uil_a21_title: selector has no in")?,
            rotate: curve(cursor.anims, "anm_rotate", "rot")?,
            blink: curve(anims, "anm_common_juvenile_blink", "color")?,
            fade: curve(anims, "anm_common_fade_in", "color")?,
            fade_speed: speed(press, "in", None).ok_or("uil_a21_title: press_start has no in")?,
            // the slide-in runs a frame behind its delay (`title::SLIDE_LATE`)
            ready: prompt_delay + FRAME + slide_pos.duration(),
            slide_pos,
            atlas: [(title_atlas.0 as f32, title_atlas.1 as f32), (cursor.atlas.0 as f32, cursor.atlas.1 as f32)],
            cursor_texture: cursor.texture,
        })
    }

    /// The items that show, as indices into the six (the game's `0x1400dc440`): New Game,
    /// Load Game with a save, Settings, Extra once unlocked, Episodes, End.
    fn shown(&self, input: &Input) -> Vec<usize> {
        // STAND-IN: Episodes always shows, as in the captures; the real thing is the game's check of the five episodes' ownership (`0x1405a2350`).
        (0..NAMES.len()).filter(|&i| (i != LOAD_GAME || input.saves) && (i != EXTRA || input.extra)).collect()
    }

    /// The top of the `k`th of `n` shown items within the menu: the list is centred in the room
    /// of six, a slot being the distance between the first two items in the layout.
    fn item_top(&self, k: usize, n: usize) -> f32 {
        let (base, slot) = (self.items[0].pos[1], self.items[1].pos[1] - self.items[0].pos[1]);
        (slot * SLOTS + base) * 0.5 - n as f32 * slot * 0.5 + k as f32 * slot
    }

    /// The item the button on frame `choose` took (`new_game`, `load_game`, ...), the cursor
    /// being where `input` has moved it.
    pub fn chosen(&self, input: &Input, choose: i32) -> Option<&'static str> {
        let press = self.pressed(input)?;
        let shown = self.shown(input);
        let n = shown.len();
        let first = shown.iter().position(|&i| i == LOAD_GAME).unwrap_or(0);
        let since = choose - press - wait_frames(MENU_WAIT) + 2 - self.settled(n);
        if since < 1 {
            return None;
        }
        let on = self.moves(input, n, first, choose - since + 1, choose).last().map_or(first, |m| m.1);
        Some(NAMES[shown[on]])
    }

    /// The frame the button at the prompt counts from, if it does: the prompt is all the way in.
    pub fn pressed(&self, input: &Input) -> Option<i32> {
        input.press.filter(|&p| p as f32 >= (self.ready * 60.0).ceil())
    }

    /// The prompt's opacity after the button, as a share of what it was the frame before
    /// (its pulse stops there), or `None` once the menu has taken its place.
    pub fn prompt_fade(&self, frame: i32, press: i32) -> Option<f32> {
        let k = frame - press;
        (k < wait_frames(MENU_WAIT) - 1).then(|| self.fade.at(self.fade.duration() - (k as f32 + FADE_AHEAD) * self.fade_speed * FRAME)[3] / 255.0)
    }

    /// The frame (counted from the menu's first, which is 1) on which the last of `n` items'
    /// slide-in ends: the cursor and the pulse start the frame after.
    fn settled(&self, n: usize) -> i32 {
        let last = n.saturating_sub(1) as f32 * ITEM_STAGGER;
        (1..).find(|&e| e as f32 * FRAME * self.up_speed - last >= self.slide_pos.duration()).unwrap()
    }

    /// The moves of the cursor on or before `frame`: (frame, item it lands on), among `n`
    /// shown items from `first`, none before `from` (the cursor's first frame). A press wraps
    /// round the list's ends; a held direction repeats and stops at them.
    fn moves(&self, input: &Input, n: usize, first: usize, from: i32, frame: i32) -> Vec<(i32, usize)> {
        let mut steps: Vec<(i32, i32, bool)> = Vec::new();
        for h in &input.holds {
            let end = h.until.unwrap_or(i32::MAX).min(frame + 1);
            let mut at = h.from;
            let mut repeat = false;
            while at < end {
                steps.push((at, h.way, repeat));
                at += if repeat { REPEAT_NEXT } else { REPEAT_FIRST };
                repeat = true;
            }
        }
        steps.sort_by_key(|s| s.0);
        let (mut on, mut out) = (first as i32, Vec::new());
        for (at, way, repeat) in steps.into_iter().filter(|s| s.0 >= from && s.0 <= frame) {
            let to = on + way;
            let to = if repeat { to.clamp(0, n as i32 - 1) } else { to.rem_euclid(n as i32) };
            if to != on {
                on = to;
                out.push((at, on as usize));
            }
        }
        out
    }

    /// The menu's quads on title frame `frame`, the button having counted on frame `press`, on
    /// a `screen` of pixels: the cursor's pictures, then the items. Nothing before the menu's
    /// first frame.
    pub fn quads(&self, frame: i32, press: i32, input: &Input, screen: (u32, u32)) -> Vec<Quad> {
        // frames into the menu, its first being 1 (the animations have had one step by then)
        // (the wait's timer takes its first step on the button's own frame)
        let e = frame - press - wait_frames(MENU_WAIT) + 2;
        if e < 1 {
            return Vec::new();
        }
        let k = screen.1 as f32 / LAYOUT_HEIGHT;
        let left = (screen.0 as f32 - 1280.0 * k) * 0.5;
        let to_screen = |[x, y]: [f32; 2]| [left + x * k, y * k];
        let shown = self.shown(input);
        let n = shown.len();
        let first = shown.iter().position(|&i| i == LOAD_GAME).unwrap_or(0);
        let since = e - self.settled(n);
        // Once an item is chosen the cursor and the pulse stand where they were.
        let live = input.choose.map_or(frame, |c| frame.min(c));
        let moves = if since >= 1 { self.moves(input, n, first, frame - since + 1, live) } else { Vec::new() };
        let on = moves.last().map_or(first, |m| m.1);
        let mut out = Vec::new();
        if since >= 1 {
            // The cursor's line: the middle of the item it is on, travelling from where it was.
            let middle = |i: usize| self.item_top(i, n) + self.items[shown[i]].size[1] * 0.5;
            let (mut line, mut goal, mut start) = (middle(first), middle(first), i32::MIN / 2);
            let at = |line: f32, goal: f32, start: i32, f: i32| line + (goal - line) * ((f - start) as f32 / TRAVEL).clamp(0.0, 1.0);
            for &(f, to) in &moves {
                (line, goal, start) = (at(line, goal, start, f), middle(to), f);
            }
            let y = self.top + at(line, goal, start, frame);
            let t = since as f32 * FRAME;
            let alpha = self.in_color.at(t)[3] / 255.0;
            let x = self.selector + self.in_pos.at(t)[0];
            // The turn about the upright line through the cursor's node as it comes in, and the
            // pictures' own turning.
            let swing = self.in_turn.at(t * self.in_turn_speed)[1].rem_euclid(360.0).to_radians();
            // STAND-IN: From the button on an item the cursor's turn runs one frame ahead (the capture's icons sit 1.5 degrees on from the loop's from that frame on); the real thing is the cursor's own leave.
            let ahead = i32::from(input.choose.is_some_and(|c| frame >= c));
            let spin = self.rotate.at(loop_shown(since + ahead, self.rotate.duration()))[2];
            let (aw, ah) = self.atlas[1];
            for icon in &self.icons {
                let turned = (spin * icon.turns).rem_euclid(360.0).to_radians();
                let (s, c) = turned.sin_cos();
                let [w, h] = icon.size;
                let corners = [[0.0, 0.0], [w, 0.0], [w, h], [0.0, h]].map(|[cx, cy]: [f32; 2]| {
                    // about the picture's pivot, clockwise on the screen
                    let (dx, dy) = (cx - icon.pivot[0], cy - icon.pivot[1]);
                    let (px, py) = (self.offset[0] + icon.pivot[0] + dx * c - dy * s, self.offset[1] + icon.pivot[1] + dx * s + dy * c);
                    // about the node's upright line, then seen from `EYE` at the layout's centre
                    let (ax, depth) = (px * swing.cos(), -px * swing.sin());
                    let near = EYE / (EYE + depth);
                    to_screen([640.0 + (x + ax - 640.0) * near, 360.0 + (y + py - 360.0) * near])
                });
                let [ix, iy, iw, ih] = icon.uvwh;
                let upright = swing == 0.0 && turned == 0.0;
                let (l, t, r, b) = corners.iter().fold((f32::MAX, f32::MAX, f32::MIN, f32::MIN), |a, p| (a.0.min(p[0]), a.1.min(p[1]), a.2.max(p[0]), a.3.max(p[1])));
                out.push(Quad {
                    turn: false,
                    spin: (!upright).then_some(Spin::Corners(corners)),
                    rect: [l, t, r, b],
                    uv: [ix / aw, iy / ah, (ix + iw) / aw, (iy + ih) / ah],
                    color: [1.0, 1.0, 1.0, alpha],
                    texture: self.cursor_texture,
                });
            }
        }
        let (aw, ah) = self.atlas[0];
        for (place, &i) in shown.iter().enumerate() {
            let item = &self.items[i];
            // Each item's slide-in starts a little after the one above it.
            let t = e as f32 * FRAME * self.up_speed - place as f32 * ITEM_STAGGER;
            let selected = since >= 1 && place == on;
            let alpha = match selected {
                // The selected item's pulse: down to half and back, from when it was selected.
                true => {
                    let ticks = moves.last().map_or(since + 1 - (frame - live), |m| live - m.0 + 1);
                    let d = self.blink.duration();
                    let phase = loop_time(ticks as f32, 2.0 * d, 1.0);
                    self.blink.at(if phase < d { phase } else { 2.0 * d - phase })[3] / 255.0
                }
                false => self.slide_color.at(t)[3] / 255.0,
            };
            let alpha = match input.choose.filter(|&c| selected && frame >= c) {
                Some(c) => alpha * CHOSEN_START * (1.0 - (frame - c) as f32 / CHOSEN_FADE as f32).max(0.0),
                None => alpha,
            };
            // the game's opacity is a byte: under one step of it nothing is drawn
            if t <= 0.0 || alpha * 255.0 < 1.0 {
                continue;
            }
            let [x, y, w, h] = item.pictures[selected as usize];
            let top = self.top + self.item_top(place, n) + self.slide_pos.at(t)[1];
            let [l, t0] = to_screen([item.pos[0], top]);
            out.push(Quad { turn: false, spin: None, rect: [l, t0, l + item.size[0] * k, t0 + item.size[1] * k], uv: [x / aw, y / ah, (x + w) / aw, (y + h) / ah], color: [1.0, 1.0, 1.0, alpha], texture: UI });
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LAYOUT: &[u8] = br#"<root>
        <node name="press_start_top"><anim_ctrl name="up" delay="2.00"/><node name="press_start" pos="-2, -30, 0">
            <anim_ctrl name="in" speed="0.50"/></node></node>
        <node name="title_menu" pos="514, 436, 0">
            <node name="line_cusor" pos="-28, -53, 0"><node name="selector" pos="60, 152, 0">
                <node name="prompt" pos="-57, -17, 0">
                    <image name="prompt_icon_1" style="prop02" pivot_pos="16.67, 16.67, 0"/>
                    <image name="prompt_icon_0" style="prop" pivot_pos="16.65, 16.65, 0"/>
                    <image name="prompt_core" style="core"/>
                    <anim_ctrl name="anim_rotate"><bind_node node="prompt_icon_1" anim="anm_rotate"/>
                        <bind_node node="prompt_icon_0" anim="anm_rotate" reverse="true"/></anim_ctrl></node>
                <anim_ctrl name="in"><bind_node node="." anim="anm_common_slide_in_left"/>
                    <bind_node node="." anim="anm_common_rotateY360" pos="30, 30, 0" speed="3.00"/></anim_ctrl></node></node>
            <node name="menu" pos="-28, -53, 0">
                <node name="new_game_top"><image name="new_game" pos="38, 11, 0" style="newgame"><anim_ctrl name="up2" speed="1.50"/></image></node>
                <node name="load_game_top"><image name="load_game" pos="38, 55, 0" style="loadgame" size_wh="232, 37"/></node>
                <node name="setting_top"><image name="setting" pos="38, 100, 0" style="setting" offset_uvwh="0, -3, 0, 0"/></node>
                <node name="extra_top"><image name="extra" pos="38, 147, 0" style="extra"/></node>
                <node name="episode_top"><image name="episode" pos="38, 192, 0" style="episode"/></node>
                <node name="endgame_top"><image name="end_game" pos="38, 241, 0" style="st"/></node>
            </node></node></root>"#;
    const STYLES: &[u8] = br#"<root>
        <image_list name="newgame"><image uvwh="147,1083,698,112"/><image uvwh="961,1083,698,112"/></image_list>
        <image_list name="loadgame"><image uvwh="147,1251,698,112"/><image uvwh="961,1251,698,112"/></image_list>
        <image_list name="setting"><image uvwh="147,1423,698,112"/><image uvwh="961,1423,698,112"/></image_list>
        <image_list name="extra"><image uvwh="147,1600,698,112"/><image uvwh="961,1600,698,112"/></image_list>
        <image_list name="episode"><image uvwh="147,2214,698,124"/><image uvwh="961,2214,698,124"/></image_list>
        <image_list name="st"><image uvwh="147,2088,698,112"/><image uvwh="961,2088,698,112"/></image_list></root>"#;
    const CURSOR_STYLES: &[u8] = br#"<root><image name="core" uvwh="979,664,100,100"/><image name="prop" uvwh="1151,664,100,100"/>
        <image name="prop02" uvwh="1323,664,100,100"/></root>"#;
    const ANIMS: &[u8] = br#"<root>
        <anim name="anm_common_rotateY360"><curve attr="rot"><key value="0, 0, 0"/><key frame60="60" value="0, 360, 0"/></curve></anim>
        <anim name="anm_common_fade_in"><curve attr="color"><key value="255, 255, 255, 0"/><key frame60="15" value="255, 255, 255, 255"/></curve></anim>
        <anim name="anm_common_juvenile_blink"><curve attr="color" interpolate="easing_incubic">
            <key value="255, 255, 255, 255"/><key frame60="45" value="255, 255, 255, 127"/><key frame60="45" value="255, 255, 255, 255"/></curve></anim>
        <anim name="anm_common_slide_in_left">
            <curve attr="color" interpolate="easing_outcubic"><key value="255, 255, 255, 0"/><key frame60="30" value="255, 255, 255, 255"/></curve>
            <curve attr="pos" interpolate="easing_outcubic"><key value="-40, 0, 0"/><key frame60="30" value="0, 0, 0"/></curve></anim>
        <anim name="anm_common_slide_in_bottom">
            <curve attr="color" interpolate="easing_outcubic"><key value="255, 255, 255, 0"/><key frame60="30" value="255, 255, 255, 255"/></curve>
            <curve attr="pos" interpolate="easing_outcubic"><key value="0, 30, 0"/><key frame60="30" value="0, 0, 0"/></curve></anim></root>"#;
    const CURSOR_ANIMS: &[u8] = br#"<root><anim name="anm_rotate"><curve attr="rot"><key value="0, 0, 0"/><key frame60="240" value="0, 0, 360"/></curve></anim></root>"#;

    fn menu() -> Menu {
        Menu::read(LAYOUT, STYLES, ANIMS, Cursor { styles: CURSOR_STYLES, anims: CURSOR_ANIMS, atlas: (2600, 3200), texture: 3 }, (2048, 2560)).unwrap()
    }

    fn near(a: f32, b: f32, tol: f32) -> bool {
        (a - b).abs() <= tol
    }

    /// The capture's button is on frame 1235 and its title starts at 875: title frame 360.
    const PRESS: i32 = 360;
    fn at(capture: i32) -> i32 {
        capture - 875
    }
    fn input(holds: &[(i32, i32, i32)]) -> Input {
        Input { press: Some(PRESS), holds: holds.iter().map(|&(f, way, held)| Hold { from: at(f), until: Some(at(f) + held), way }).collect(), ..Input::default() }
    }

    #[test]
    fn a_button_counts_once_the_prompt_is_in() {
        let m = menu();
        // the slide-in ends 2 s + a frame + 0.5 s into the title: frame 151
        assert_eq!(m.pressed(&Input { press: Some(150), ..Input::default() }), None);
        assert_eq!(m.pressed(&Input { press: Some(151), ..Input::default() }), Some(151));
    }

    #[test]
    fn the_prompt_fades_for_fifteen_frames_from_two_steps_in() {
        let m = menu();
        // the capture: 238 of 255 on the button's frame, 119 on the last (1249), then the menu
        assert!(near(m.prompt_fade(PRESS, PRESS).unwrap(), 238.0 / 255.0, 1e-4));
        assert!(near(m.prompt_fade(at(1249), PRESS).unwrap(), 119.0 / 255.0, 1e-4));
        assert_eq!(m.prompt_fade(at(1250), PRESS), None);
        assert!(m.quads(at(1249), PRESS, &input(&[]), (1920, 1080)).is_empty());
    }

    #[test]
    fn four_items_slide_up_one_after_another_where_the_capture_has_them() {
        let m = menu();
        let i = input(&[]);
        // 1250: the first alone, 38.58 pixels low at 36 of 255
        let q = m.quads(at(1250), PRESS, &i, (1920, 1080));
        assert_eq!(q.len(), 1);
        assert!(near(q[0].rect[0], 786.0, 0.01) && near(q[0].rect[1], 687.33, 0.01) && near(q[0].color[3] * 255.0, 36.4, 0.5), "{:?}", q[0]);
        // 1256: the third comes in at 821.82
        let q = m.quads(at(1256), PRESS, &i, (1920, 1080));
        assert_eq!(q.len(), 3);
        assert!(near(q[2].rect[1], 821.82, 0.01), "{:?}", q[2]);
        // 1279: all four at rest, 66 pixels apart, none selected yet; Settings' picture sits
        // nine atlas pixels up and Episodes is taller
        let q = m.quads(at(1279), PRESS, &i, (1920, 1080));
        assert_eq!(q.iter().map(|q| q.rect[1]).collect::<Vec<_>>(), [648.75, 714.75, 780.75, 846.75]);
        assert!(near(q[1].uv[1], 1414.0 / 2560.0, 1e-6) && near(q[2].rect[3] - q[2].rect[1], 62.0, 0.01));
        assert!(q.iter().all(|q| q.texture == UI && near(q.uv[0], 147.0 / 2048.0, 1e-6)));
    }

    #[test]
    fn with_a_save_five_items_show_and_the_cursor_starts_on_load_game() {
        let m = menu();
        let i = Input { saves: true, ..input(&[]) };
        // the capture with a save: tops from 615.75, the cursor and the selected picture at 1283
        let q = m.quads(at(1282), PRESS, &i, (1920, 1080));
        assert_eq!(q.iter().map(|q| q.rect[1]).collect::<Vec<_>>(), [615.75, 681.75, 747.75, 813.75, 879.75]);
        let q = m.quads(at(1283), PRESS, &i, (1920, 1080));
        assert_eq!(q.len(), 8);
        assert!(near(q[4].uv[0], 961.0 / 2048.0, 1e-6) && near(q[4].rect[1], 681.75, 0.01));
        // Load Game is drawn at its picture's size, not the layout's `size_wh`
        assert!(near(q[4].rect[2] - q[4].rect[0], 349.0, 0.01));
    }

    #[test]
    fn the_cursor_spins_in_seen_in_perspective() {
        let m = menu();
        let i = input(&[]);
        let core = |f: i32| m.quads(at(f), PRESS, &i, (1920, 1080))[2];
        // 1280, a tenth of the turn in: the capture's corners
        let c = core(1280).corners();
        let want = [[687.06, 649.81], [732.28, 650.65], [732.28, 700.38], [687.06, 699.17]];
        assert!(c.iter().zip(want).all(|(a, b)| near(a[0], b[0], 0.1) && near(a[1], b[1], 0.1)), "{c:?}");
        assert!(near(core(1280).color[3] * 255.0, 25.0, 0.5));
        // 1289, half way: flat again, mirrored about the node
        let c = core(1289).corners();
        assert!(near(c[0][0], 886.72, 0.05) && near(c[1][0], 836.72, 0.05) && near(c[0][1], 651.25, 0.05), "{c:?}");
        // 1308 on: at rest, upright
        let q = core(1310);
        assert!(q.spin.is_none() && near(q.rect[0], 733.5, 0.01) && near(q.rect[1], 651.25, 0.01) && near(q.rect[2], 783.5, 0.01), "{q:?}");
        // the two props turn against each other, 1.5 degrees a frame: 30 degrees at 1299
        let q = m.quads(at(1299), PRESS, &i, (1920, 1080));
        let (a, b) = (q[0].corners(), q[1].corners());
        assert!(near(a[0][0], 747.1, 0.1) && near(a[0][1], 642.1, 0.1) && near(a[1][0], 790.4, 0.1) && near(a[1][1], 667.1, 0.1), "{a:?}");
        assert!(near(b[0][0], 722.1, 0.1) && near(b[0][1], 667.1, 0.1) && near(b[1][0], 765.4, 0.1) && near(b[1][1], 642.1, 0.1), "{b:?}");
    }

    #[test]
    fn the_props_turn_loops_with_its_first_step_shown_twice() {
        // the capture: 0 degrees at 1519 (240 frames in), 1.5 at 1520 and 1521, 3 at 1522
        let d = 4.0;
        let deg = |ticks: i32| loop_shown(ticks, d) / d * 360.0;
        assert!(near(deg(240).rem_euclid(360.0).min(360.0 - deg(240).rem_euclid(360.0)), 0.0, 0.01));
        assert!(near(deg(241), 1.5, 0.01) && near(deg(242), 1.5, 0.01) && near(deg(243), 3.0, 0.01));
    }

    #[test]
    fn the_selected_item_pulses_from_when_it_is_selected() {
        let m = menu();
        let alpha = |f: i32, i: &Input| {
            let q = m.quads(at(f), PRESS, i, (1920, 1080));
            let s: Vec<_> = q.iter().filter(|q| q.texture == UI && q.uv[0] > 0.4).collect();
            assert_eq!(s.len(), 1);
            (s[0].rect[1], (s[0].color[3] * 255.0).round())
        };
        let none = input(&[]);
        // the capture: 255 at 1280, 254 at 1286, 240 at 1300, 127 at 1323, 135 either side,
        // and the second dip 91 frames after the first (217 at 1399 as at 1308)
        assert_eq!(alpha(1280, &none), (648.75, 255.0));
        assert_eq!((alpha(1286, &none).1, alpha(1300, &none).1, alpha(1322, &none).1, alpha(1323, &none).1, alpha(1324, &none).1), (254.0, 240.0, 135.0, 127.0, 135.0));
        assert_eq!(alpha(1399, &none).1, alpha(1308, &none).1);
        // down at 1400: the second item at once, its pulse from there (254 at 1407, 252 at 1412)
        let down = input(&[(1400, 1, 6)]);
        assert_eq!(alpha(1399, &down).0, 648.75);
        assert_eq!(alpha(1400, &down), (714.75, 255.0));
        assert_eq!((alpha(1407, &down).1, alpha(1412, &down).1), (254.0, 252.0));
    }

    #[test]
    fn the_cursor_travels_six_frames_wraps_on_a_press_and_stops_on_a_repeat() {
        let m = menu();
        let core = |f: i32, i: &Input| m.quads(at(f), PRESS, i, (1920, 1080))[2].rect[1];
        let i = input(&[(1400, 1, 6), (1460, 1, 6), (1520, -1, 6)]);
        // the capture: 651.25, then 11 a frame to 717.25; the third item is taller (786.25)
        assert_eq!((core(1400, &i), core(1401, &i), core(1403, &i), core(1406, &i), core(1407, &i)), (651.25, 662.25, 684.25, 717.25, 717.25));
        assert_eq!((core(1461, &i), core(1466, &i), core(1521, &i), core(1526, &i)), (728.75, 786.25, 774.75, 717.25));
        // the other capture: down four times comes round to the first, up from there goes to
        // the last, and down held goes round once, then repeats after 37 frames and every 15
        // to the last item and no further
        let w = input(&[(1300, 1, 6), (1320, 1, 6), (1340, 1, 6), (1360, 1, 6), (1380, -1, 6), (1400, 1, 140)]);
        assert_eq!((core(1346, &w), core(1361, &w), core(1366, &w), core(1386, &w)), (849.25, 816.25, 651.25, 849.25));
        assert_eq!((core(1406, &w), core(1436, &w), core(1443, &w), core(1458, &w), core(1473, &w), core(1530, &w)), (651.25, 651.25, 717.25, 786.25, 849.25, 849.25));
    }
}
