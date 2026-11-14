//! The fields' wind and the grass it sways (`docs/shading.md`, "Wind"), as the game drives
//! `MapSTD`'s sway constants.
//!
//! Each frame (`0x140286540`) the field's wind clock `T` (seconds, kept within 0 to 3600) gives
//! each `<wind>` a strength between none and full, `(sin((T + shift) · 360 / freq °) + 1) / 2`
//! (`sinf`), and the winds are summed, `power · dir` each. Each `<v_shaker>` (`0x140285d80`)
//! keeps its own sway clock, advanced by `2π / shake_time` a second (kept within 0 to 3600), and
//! sets its meshes' constants: the flag on, `g_fWindTime` its clock, `g_f3Wind` the summed wind
//! over its `weight`, `g_fInvHeight` one over its height, `g_fRootHeight` its root height,
//! `g_f3Shift` (`shift_x`, 0, `shift_z`).

use barrel_formats::field_look::{NShaker, VShaker, Wind};

/// Keeps a clock within 0 to 3600, as the game does (it adds or takes 3600 until it is).
fn wrap(t: f32) -> f32 {
    t.rem_euclid(3600.0)
}

/// The winds blowing together at wind time `t` (seconds).
pub fn wind(winds: &[Wind], t: f32) -> [f32; 3] {
    let t = wrap(t);
    winds.iter().fold([0.0; 3], |sum, w| {
        let strength = (((t + w.shift) * 360.0 / w.freq).to_radians().sin() + 1.0) * 0.5;
        [0, 1, 2].map(|i| sum[i] + w.dir[i] * w.power * strength)
    })
}

/// A swaying mesh group's constants.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sway {
    pub wind: [f32; 3],
    pub time: f32,
    pub inv_height: f32,
    pub root_height: f32,
    pub shift: [f32; 3],
}

/// One `<v_shaker>`'s sway clock.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Shaker {
    pub time: f32,
}

impl Shaker {
    /// Advances its clock by `dt` seconds and gives its meshes' constants under `wind`.
    pub fn step(&mut self, v: &VShaker, wind: [f32; 3], dt: f32) -> Sway {
        self.time = wrap(self.time + std::f32::consts::TAU / v.shake_time * dt);
        Sway {
            wind: wind.map(|w| w / v.weight),
            time: self.time,
            inv_height: 1.0 / v.height,
            root_height: v.root_height,
            shift: [v.shift[0], 0.0, v.shift[1]],
        }
    }
}

/// The longest step a node spring takes (`0x1402855f0` caps the frame's time at 1/30 s).
pub const NODE_STEP: f32 = 1.0 / 30.0;

/// One `<n_shaker>` node's spring (`0x1402855f0`): its tip direction `d` (unit, the node's own
/// space, starting on the rest axis) and velocity `v`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NodeSpring {
    pub d: [f32; 3],
    pub v: [f32; 3],
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn unit(a: [f32; 3]) -> [f32; 3] {
    let l = dot(a, a).sqrt();
    if l > 0.0 { a.map(|x| x / l) } else { a }
}

impl NodeSpring {
    pub fn new(n: &NShaker) -> NodeSpring {
        NodeSpring { d: n.axis, v: [0.0; 3] }
    }

    /// A step of `dt` seconds (at most [`NODE_STEP`]) under a wind of strength `strength`
    /// along `toward` (a unit vector in the node's own space), with `push` the player's random
    /// push (a unit vector) when within its `char_range`:
    /// 1. unless the wind is nearly along the tip (`|toward · d| ≥ 0.9`), `v += toward ×
    ///    strength × dt / weight`; 2. `v += push × char_power × dt`; 3. `v += (axis - d) ×
    ///    axis_power × 100 × dt` (the parser keeps it × 100); 4. `d += v × dt`, normalised;
    /// 5. past `max_angle` from the axis, `d` is put back on that cone (in the plane of `d` and
    ///    the axis: this module's, the game's cross-product version not being read through).
    pub fn step(&mut self, n: &NShaker, toward: [f32; 3], strength: f32, push: Option<[f32; 3]>, dt: f32) {
        let dt = dt.min(NODE_STEP);
        if dot(toward, self.d).abs() < 0.9 {
            let k = strength * dt / n.weight;
            self.v = [0, 1, 2].map(|i| self.v[i] + toward[i] * k);
        }
        if let Some(r) = push {
            self.v = [0, 1, 2].map(|i| self.v[i] + r[i] * n.char_power * dt);
        }
        let pull = n.axis_power * 100.0 * dt;
        self.v = [0, 1, 2].map(|i| self.v[i] + (n.axis[i] - self.d[i]) * pull);
        self.d = unit([0, 1, 2].map(|i| self.d[i] + self.v[i] * dt));
        let limit = n.max_angle.to_radians();
        let c = dot(n.axis, self.d);
        if c < limit.cos() {
            let side = unit([0, 1, 2].map(|i| self.d[i] - n.axis[i] * c));
            self.d = [0, 1, 2].map(|i| n.axis[i] * limit.cos() + side[i] * limit.sin());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn winds_breathe_over_their_period() {
        let w = [Wind { dir: [1.0, 0.0, 0.0], power: 50.0, freq: 10.0, shift: 0.0 }];
        // Half strength at the start, full a quarter in, none three quarters in.
        assert!((wind(&w, 0.0)[0] - 25.0).abs() < 1e-3);
        assert!((wind(&w, 2.5)[0] - 50.0).abs() < 1e-3);
        assert!(wind(&w, 7.5)[0].abs() < 1e-3);
    }

    #[test]
    fn a_node_bends_with_the_wind_but_no_further_than_its_angle() {
        let n = NShaker { keyword: "xN1_".into(), weight: 15.0, axis: [0.0, 0.0, 1.0], max_angle: 10.0, axis_power: 0.0, char_range: 0.0, char_power: 0.0 };
        let mut s = NodeSpring::new(&n);
        for _ in 0..300 {
            s.step(&n, [1.0, 0.0, 0.0], 50.0, None, 1.0 / 30.0);
        }
        // With no pull back it leans to its limit, toward the wind.
        assert!((s.d[2] - 10f32.to_radians().cos()).abs() < 1e-4 && s.d[0] > 0.0, "{:?}", s.d);
        // A pull back with no wind brings it home.
        let n2 = NShaker { axis_power: 50.0, ..n.clone() };
        let mut s2 = NodeSpring { d: s.d, v: [0.0; 3] };
        s2.step(&n2, [1.0, 0.0, 0.0], 0.0, None, 1.0 / 30.0);
        assert!(s2.d[0] < s.d[0]);
    }

    #[test]
    fn a_shaker_turns_once_a_shake() {
        let v = VShaker { keyword: "xV1_".into(), weight: 500.0, root_height: 0.0, height: 0.5, shake_time: 4.0, shift: [1.0, 1.0] };
        let mut s = Shaker::default();
        let sway = s.step(&v, [50.0, 0.0, 0.0], 1.0);
        assert!((sway.time - std::f32::consts::FRAC_PI_2).abs() < 1e-5);
        assert_eq!((sway.wind[0], sway.inv_height, sway.shift), (0.1, 2.0, [1.0, 0.0, 1.0]));
    }
}
