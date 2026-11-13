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

use barrel_formats::field_look::{VShaker, Wind};

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
    fn a_shaker_turns_once_a_shake() {
        let v = VShaker { keyword: "xV1_".into(), weight: 500.0, root_height: 0.0, height: 0.5, shake_time: 4.0, shift: [1.0, 1.0] };
        let mut s = Shaker::default();
        let sway = s.step(&v, [50.0, 0.0, 0.0], 1.0);
        assert!((sway.time - std::f32::consts::FRAC_PI_2).abs() < 1e-5);
        assert_eq!((sway.wind[0], sway.inv_height, sway.shift), (0.1, 2.0, [1.0, 0.0, 1.0]));
    }
}
