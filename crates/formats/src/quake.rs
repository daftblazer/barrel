//! Camera quakes (`Saves/cameraQuake/camera_quake_data.xml`, a PSSG-style XML): the shakes an
//! event's `t14` (`ECT_CAMERA_QUAKE_SET {quake, seconds × 100}`) starts and `t16` stops
//! (`docs/events.md`, "Camera quakes").
//!
//! ```text
//! <CameraQuakeData id="cq_obj_001">          61 of them, cq_obj_000 to cq_obj_060
//!   <CQ_ENVELOPE_LINKER_DATA initData num bLoop offset>     six tracks
//!     <CQ_ENVELOPE_DATA point frame liner/>                 16 keys, the first `num` used
//! ```
//!
//! Frames are 1/60 s. The six tracks come in opposite-signed pairs (cq_obj_001: 0 → -0.3 and
//! 0 → +0.3 over 120 frames), read as each axis's lower and upper bound; what each pair moves
//! is not read from the executable yet.

use crate::table::Element;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Key {
    pub value: f32,
    pub frame: i32,
    /// `liner`: linear to the next key (1 in every key seen).
    pub linear: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Track {
    pub initial: f32,
    pub looping: bool,
    pub offset: i32,
    pub keys: Vec<Key>,
}

impl Track {
    /// The value `frame` frames in: linear between keys, held after the last (wrapping when
    /// the track loops).
    pub fn at(&self, frame: f32) -> f32 {
        let Some(last) = self.keys.last() else { return self.initial };
        let span = last.frame.max(1) as f32;
        let f = if self.looping { frame.rem_euclid(span) } else { frame };
        for w in self.keys.windows(2) {
            let (a, b) = (w[0], w[1]);
            if f <= b.frame as f32 {
                let t = if b.frame > a.frame { ((f - a.frame as f32) / (b.frame - a.frame) as f32).clamp(0.0, 1.0) } else { 1.0 };
                return a.value + (b.value - a.value) * t;
            }
        }
        last.value
    }

    /// The frame the track ends at (its last key).
    pub fn length(&self) -> i32 {
        self.keys.last().map_or(0, |k| k.frame)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Quake {
    /// `cq_obj_001` ...; `t14`'s first operand is the number.
    pub id: String,
    pub tracks: Vec<Track>,
}

impl Quake {
    pub fn number(&self) -> Option<u32> {
        self.id.strip_prefix("cq_obj_")?.parse().ok()
    }
}

/// Every quake in the table.
pub fn read(root: &Element) -> Vec<Quake> {
    let num = |e: &Element, k: &str| e.attr(k).and_then(|v| v.trim().parse::<f32>().ok()).unwrap_or(0.0);
    let mut out = Vec::new();
    let mut stack = vec![root];
    while let Some(e) = stack.pop() {
        if e.name == "CameraQuakeData" {
            let tracks = e
                .children
                .iter()
                .filter(|l| l.name == "CQ_ENVELOPE_LINKER_DATA")
                .map(|l| {
                    let used = num(l, "num") as usize;
                    Track {
                        initial: num(l, "initData"),
                        looping: num(l, "bLoop") != 0.0,
                        offset: num(l, "offset") as i32,
                        keys: l
                            .children
                            .iter()
                            .filter(|k| k.name == "CQ_ENVELOPE_DATA")
                            .take(used)
                            .map(|k| Key { value: num(k, "point"), frame: num(k, "frame") as i32, linear: num(k, "liner") != 0.0 })
                            .collect(),
                    }
                })
                .collect();
            out.push(Quake { id: e.attr("id").unwrap_or("").to_string(), tracks });
        } else {
            stack.extend(e.children.iter().rev());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_and_plays_a_track() {
        let xml = r#"<PSSGFILE><LIBRARY><CameraQuakeData id="cq_obj_001" >
<CQ_ENVELOPE_LINKER_DATA initData="0" num="3" bLoop="0" offset="0" >
<CQ_ENVELOPE_DATA point="0" frame="0" liner="1" ></CQ_ENVELOPE_DATA>
<CQ_ENVELOPE_DATA point="-0.15" frame="80" liner="1" ></CQ_ENVELOPE_DATA>
<CQ_ENVELOPE_DATA point="-0.3" frame="120" liner="1" ></CQ_ENVELOPE_DATA>
<CQ_ENVELOPE_DATA point="0" frame="-1" liner="1" ></CQ_ENVELOPE_DATA>
</CQ_ENVELOPE_LINKER_DATA></CameraQuakeData></LIBRARY></PSSGFILE>"#;
        let q = read(&crate::table::parse_str(xml).unwrap());
        assert_eq!((q.len(), q[0].number(), q[0].tracks[0].keys.len()), (1, Some(1), 3));
        let t = &q[0].tracks[0];
        for (frame, want) in [(40.0, -0.075), (100.0, -0.225), (500.0, -0.3)] {
            assert!((t.at(frame) - want).abs() < 1e-6, "{frame}: {}", t.at(frame));
        }
        assert_eq!(t.length(), 120);
    }
}
