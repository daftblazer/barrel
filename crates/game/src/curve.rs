//! Keyed curves (`Saves/curve/curve_data.xml`, 72 of them, `curve_obj_000` to `curve_obj_071`):
//! the shapes timed turns (`s26`) and other interpolations follow, as a value (mostly 0 to 1)
//! over 60 frames that the user stretches to its own length.
//!
//! ```text
//! <CurveData id="curve_obj_001">
//!   <CURVE_ENVELOPE_LINKER_DATA initData="0" num="3" bLoop="0" offset="0">
//!     <CURVE_ENVELOPE_DATA point="0" frame="0" liner="0"/>       (value, frame, mode)
//!     <CURVE_ENVELOPE_DATA point="0.757" frame="30" liner="0"/>
//!     <CURVE_ENVELOPE_DATA point="1" frame="60" liner="0"/>
//!     ... 16 slots in all; those past `num` have frame -1
//! ```
//!
//! The evaluation is `0x1404e1230` in the English executable: find the segment the time is
//! in, then by the segment's first key's mode: 1 straight, 2 hold the key's value, 0 smooth
//! (`0x1404e09e0`): a cubic spline (`0x1404e0440` solves it, `0x1404e0850` evaluates it)
//! through this key and the next, the key before when it is smooth too, and the key after
//! next when the next is smooth; with only the two it is straight. The spline is natural (σ
//! 0 at both ends), with one quirk for four points (see [`spline`]).
//! Past the last key the value is the last key's.

use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Key {
    pub value: f32,
    pub frame: f32,
    /// 0 smooth, 1 straight, 2 hold (`liner`).
    pub mode: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Curve {
    pub keys: Vec<Key>,
}

/// The frames a curve spans: users scale time so that their length maps to this
/// (`0x14071c348`).
pub const FRAMES: f32 = 60.0;

impl Curve {
    /// Every curve in `curve_data.xml`, by the number in its id.
    pub fn table(root: &barrel_formats::table::Element) -> BTreeMap<u32, Curve> {
        let mut out = BTreeMap::new();
        for c in &root.children {
            let Some(n) = c.attr("id").and_then(|id| id.strip_prefix("curve_obj_")).and_then(|n| n.parse().ok()) else { continue };
            let Some(linker) = c.children.first() else { continue };
            let num: usize = linker.attr("num").and_then(|v| v.parse().ok()).unwrap_or(0);
            let keys: Vec<Key> = linker
                .children
                .iter()
                .take(num)
                .filter_map(|k| {
                    Some(Key {
                        value: k.attr("point")?.trim().parse().ok()?,
                        frame: k.attr("frame")?.trim().parse().ok()?,
                        mode: k.attr("liner")?.trim().parse().ok()?,
                    })
                })
                .collect();
            if !keys.is_empty() {
                out.insert(n, Curve { keys });
            }
        }
        out
    }

    /// The value at `frame` (0 to [`FRAMES`]).
    pub fn at(&self, frame: f32) -> f32 {
        let k = &self.keys;
        let Some(last) = k.last() else { return 0.0 };
        if frame >= last.frame || k.len() == 1 {
            return last.value;
        }
        if frame <= k[0].frame {
            return k[0].value;
        }
        let i = (0..k.len() - 1).rfind(|&i| k[i].frame <= frame).unwrap_or(0);
        let (a, b) = (k[i], k[i + 1]);
        if b.frame == a.frame {
            return a.value;
        }
        let straight = a.value + (b.value - a.value) * (frame - a.frame) / (b.frame - a.frame);
        if k.len() == 2 {
            return straight;
        }
        match a.mode {
            1 => straight,
            2 => a.value,
            _ => {
                let first = if i > 0 && k[i - 1].mode == 0 { i - 1 } else { i };
                let end = if i + 2 < k.len() && b.mode == 0 { i + 2 } else { i + 1 };
                if end - first < 2 {
                    return straight;
                }
                spline(&k[first..=end], frame)
            }
        }
    }
}

/// The game's cubic spline through `points` (three or four), at `x`: `0x1404e0440`'s solve and
/// `0x1404e0850`'s evaluation, step for step in f32. σ is the second derivative over 6, 0 at
/// both ends (a natural spline). With four points the solver sets the third row's right-hand
/// side to 0 instead of the slope difference (`subss xmm1, xmm1` at `0x1404e0581`), so the
/// curve is not the textbook spline there; that is kept.
fn spline(points: &[Key], x: f32) -> f32 {
    let n = points.len();
    let xs: Vec<f32> = points.iter().map(|p| p.frame).collect();
    let ys: Vec<f32> = points.iter().map(|p| p.value).collect();
    let h: Vec<f32> = (0..n - 1).map(|i| xs[i + 1] - xs[i]).collect();
    // s[i + 1] is the slope of segment i; from index 1 on the solver reuses it for the diagonal.
    let mut s = vec![0.0f32; n];
    for i in 0..n - 1 {
        s[i + 1] = (ys[i + 1] - ys[i]) / h[i];
    }
    let mut sigma = vec![0.0f32; n];
    sigma[1] = s[2] - s[1] - sigma[0] * h[0];
    s[1] = 2.0 * (xs[2] - xs[0]);
    for i in 1..n - 2 {
        let t = h[i] / s[i];
        sigma[i + 1] = 0.0 - t * sigma[i];
        s[i + 1] = 2.0 * (xs[i + 2] - xs[i]) - t * h[i];
    }
    sigma[n - 2] -= h[n - 2] * sigma[n - 1];
    for i in (1..=n - 2).rev() {
        sigma[i] = (sigma[i] - h[i] * sigma[i + 1]) / s[i];
    }
    // 0x1404e0850: the segment by binary search, then the cubic from its first point.
    let (mut lo, mut hi) = (0usize, n - 1);
    while lo < hi {
        let mid = (lo + hi) / 2;
        if x > xs[mid] {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }
    let i = if lo == 0 { 0 } else { lo - 1 };
    let (x0, y0, s0, s1) = (xs[i], ys[i], sigma[i], sigma[i + 1]);
    let w = xs[i + 1] - x0;
    let dx = x - x0;
    let b = (ys[i + 1] - y0) / w - (s0 + s0 + s1) * w;
    let c = (s1 - s0) * dx / w + s0 * 3.0;
    (b + c * dx) * dx + y0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn curve(keys: &[(f32, f32, u32)]) -> Curve {
        Curve { keys: keys.iter().map(|&(value, frame, mode)| Key { value, frame, mode }).collect() }
    }

    #[test]
    fn two_keys_are_straight_and_three_bend() {
        // curve_obj_000 and _002: two keys, straight whatever the mode.
        assert!((curve(&[(0.0, 0.0, 0), (1.0, 60.0, 0)]).at(15.0) - 0.25).abs() < 1e-6);
        // curve_obj_001: through 0.757 at the middle, an ease-out.
        let c = curve(&[(0.0, 0.0, 0), (0.757, 30.0, 0), (1.0, 60.0, 0)]);
        assert!((c.at(30.0) - 0.757).abs() < 1e-6);
        let q = c.at(15.0);
        assert!(q > 0.757 / 2.0 && q < 0.757, "{q}");
        assert_eq!(c.at(90.0), 1.0);
        // A held key keeps its value through its segment.
        assert_eq!(curve(&[(0.0, 0.0, 2), (0.5, 30.0, 0), (1.0, 60.0, 0)]).at(20.0), 0.0);
    }

    #[test]
    fn the_spline_is_natural() {
        // Through (0,0), (1,1), (2,0): natural spline value at 0.5 is 0.6875.
        let k = |value: f32, frame: f32| Key { value, frame, mode: 0 };
        assert!((spline(&[k(0.0, 0.0), k(1.0, 1.0), k(0.0, 2.0)], 0.5) - 0.6875).abs() < 1e-5);
        // Four points still pass through every point, the quirk only changes the bend.
        let four = [k(0.0, 0.0), k(1.0, 1.0), k(0.0, 2.0), k(1.0, 3.0)];
        for p in &four[..3] {
            assert!((spline(&four, p.frame) - p.value).abs() < 1e-5);
        }
        // Straight data stays straight.
        let line = [k(0.0, 0.0), k(1.0, 1.0), k(2.0, 2.0), k(3.0, 3.0)];
        assert!((spline(&line, 1.5) - 1.5).abs() < 1e-5);
    }
}
