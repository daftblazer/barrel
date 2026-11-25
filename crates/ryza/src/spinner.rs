//! The saving spinner (`loading_1` of `uil_a21_notification_display.xml` and of
//! `uil_a21_autosave.xml`): eight orbs about a group's middle, each turned 45 degrees more, with
//! a glow over each that loops on its own curve. The boot's autosave window and the field's
//! autosave mark draw it.

use barrel_formats::table::Element;
use barrel_render::text::{Quad, Spin};

use crate::boot::{find, loop_time, numbers, pair, style, Track, ATLAS_PER_LAYOUT, FPS};

/// An orb of the spinner.
#[derive(Clone, Debug)]
struct Orb {
    /// The centre in the layout, and the turn in degrees.
    center: [f32; 2],
    rot: f32,
    /// Which of the eight glow curves lights it.
    glow: usize,
}

/// The spinner's orbs, their pictures and glow curves.
pub(crate) struct Spinner {
    orbs: Vec<Orb>,
    size: f32,
    pictures: Vec<[f32; 4]>,
    group_at: [f32; 2],
    group_pivot: [f32; 2],
    scale: f32,
    speed: f32,
    glows: Vec<Track>,
    /// The orb picture's atlas in style pixels and its texture in the layer.
    atlas: (f32, f32),
    texture: usize,
}

impl Spinner {
    /// The spinner of `layout`'s `loading_1` node, its orb style from `spinner_styles`
    /// (`uis_gen_a21v2_saveload.xml`) and its glow curves from `anims` (`uia_a21_saveload.xml`).
    pub(crate) fn read(layout: &Element, spinner_styles: &Element, anims: &Element, atlas: (f32, f32), texture: usize) -> Result<Spinner, String> {
        let (group, group_at) = find(layout, "loading_1", [0.0; 2]).ok_or("no loading_1")?;
        let glow_of = |image: &Element| -> Option<usize> {
            let anim = image.children.iter().find(|c| c.name == "anim_ctrl")?.children.first()?.attr("anim")?.to_string();
            anim.rsplit('_').next()?.parse::<usize>().ok().map(|n| n - 1)
        };
        let orb_style = style(spinner_styles, "gen_a21_cmn_al_001_orb")?;
        let size = orb_style.pictures[0][2] / ATLAS_PER_LAYOUT;
        let mut orbs = Vec::new();
        for o in group.children.iter().filter(|c| c.attr("name").is_some_and(|n| n.starts_with("circle_base"))) {
            let [x, y] = pair(o, "pos", [0.0; 2]);
            let rot = numbers(o, "rot").get(2).copied().unwrap_or(0.0);
            orbs.push(Orb { center: [x + size / 2.0, y + size / 2.0], rot, glow: glow_of(o).ok_or("an orb has no glow")? });
        }
        let glows = (1..=8).map(|n| Track::read(anims, &format!("anm_loading_icon_eff_{n}"), "color")).collect::<Result<Vec<_>, _>>()?;
        let ctrl = group.children.iter().find_map(|o| o.children.iter().find(|c| c.name == "anim_ctrl"));
        Ok(Spinner {
            orbs,
            size,
            pictures: orb_style.pictures,
            group_at,
            group_pivot: pair(group, "pivot_pos", [0.0; 2]),
            scale: pair(group, "scale", [1.0; 2])[0],
            speed: ctrl.and_then(|c| c.attr("speed")).and_then(|s| s.trim().parse().ok()).unwrap_or(1.0),
            glows,
            atlas,
            texture,
        })
    }

    /// The orbs `since` seconds after the spinner started, each with its glow over it (a glow
    /// that is clear draws nothing). `base` is the orbs' opacity as the byte the game holds it in
    /// (the layout's opacities multiplied down, each product truncated); `slide` moves the group
    /// down in layout pixels.
    pub(crate) fn quads(&self, since: f32, k: f32, left: f32, base: u32, slide: f32) -> Vec<Quad> {
        let mut out = Vec::new();
        let ticks = (since * FPS).round();
        let (pw, ph) = (self.size * self.scale * k, self.size * self.scale * k);
        for o in &self.orbs {
            // The group scales about its pivot: a point p lands at origin + pivot + (p - pivot) * scale.
            let at = |i: usize| self.group_at[i] + self.group_pivot[i] + (o.center[i] - self.group_pivot[i]) * self.scale;
            let (cx, cy) = (left + at(0) * k, (at(1) + slide) * k);
            let rect = [cx - pw / 2.0, cy - ph / 2.0, cx + pw / 2.0, cy + ph / 2.0];
            let spin = (o.rot != 0.0).then(|| Spin::About { pivot: [cx, cy], angle: o.rot.to_radians() });
            out.push(self.quad(self.pictures[0], rect, base as f32 / 255.0, spin));
            let glow = &self.glows[o.glow];
            let g = glow.at(self.glow_time(ticks, glow.duration()))[3].floor() as u32 * base / 255;
            if g > 0 {
                out.push(self.quad(self.pictures[1], rect, g as f32 / 255.0, spin));
            }
        }
        out
    }

    /// How far into its glow's loop the spinner is after `ticks` frames. On the frame the loop restarts the game shows
    /// the first step in (0.8 frames at speed 0.8) and holds it a frame: the field's capture (`opening-key4`,
    /// 14912, 14963, 15014) has the glow at 41 of 255 on two frames running where the loop's plain restart would show 0.
    fn glow_time(&self, ticks: f32, duration: f32) -> f32 {
        let at = loop_time(ticks, duration, self.speed);
        if ticks > 0.0 && at == 0.0 {
            crate::boot::FRAME * self.speed
        } else {
            at
        }
    }

    fn quad(&self, uvwh: [f32; 4], rect: [f32; 4], alpha: f32, spin: Option<Spin>) -> Quad {
        let [x, y, w, h] = uvwh;
        let (aw, ah) = self.atlas;
        Quad { turn: false, spin, rect, uv: [x / aw, y / ah, (x + w) / aw, (y + h) / ah], color: [1.0, 1.0, 1.0, alpha], texture: self.texture }
    }
}
