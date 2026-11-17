//! What a field's ground hides: an object whose sphere is wholly below the ground's skyline from the eye is
//! not drawn. Docs: docs/renderer.md, "Ryza DX: objects behind the ground".

use barrel_formats::khm::Heightmap;
use glam::Vec3;
use std::cell::RefCell;

/// Directions around the eye the skyline is kept for (a quarter of a degree apart).
const DIRECTIONS: usize = 1440;

/// The ground's skyline around an eye, in the field's own space (the game's, x not mirrored):
/// for a direction and a distance, the steepest slope from the eye up to any ground nearer
/// than that. Lines are made as they are asked for and kept while the eye stays.
pub struct Horizon {
    ground: Heightmap,
    eye: Vec3,
    /// A direction's running steepest slope, a step of half a cell each; empty until asked.
    lines: RefCell<Vec<Vec<f32>>>,
}

impl Horizon {
    pub fn new(ground: Heightmap) -> Horizon {
        Horizon { ground, eye: Vec3::splat(f32::NAN), lines: RefCell::new(vec![Vec::new(); DIRECTIONS]) }
    }

    /// Moves the eye; the lines are kept if it has not moved.
    pub fn look_from(&mut self, eye: Vec3) {
        if eye != self.eye {
            self.eye = eye;
            self.lines.get_mut().iter_mut().for_each(Vec::clear);
        }
    }

    fn step(&self) -> f32 {
        self.ground.cell_size * 0.5
    }

    /// The steepest slope up to the ground within `steps` steps along direction `i`.
    fn slope(&self, i: usize, steps: usize) -> f32 {
        let mut lines = self.lines.borrow_mut();
        let line = &mut lines[i];
        if line.is_empty() {
            let angle = i as f32 * std::f32::consts::TAU / DIRECTIONS as f32;
            let (dx, dz) = (angle.sin(), angle.cos());
            let reach = self.ground.cell_size * (self.ground.cells_x.max(self.ground.cells_z) as f32) * 1.5;
            let mut steepest = f32::NEG_INFINITY;
            let mut d = self.step();
            while d <= reach {
                if let Some(y) = self.ground.at(self.eye.x + dx * d, self.eye.z + dz * d) {
                    steepest = steepest.max((y - self.eye.y) / d);
                }
                line.push(steepest);
                d += self.step();
            }
        }
        line.get(steps.min(line.len()) - 1).copied().unwrap_or(f32::NEG_INFINITY)
    }

    /// Whether the sphere at `centre` is wholly behind the ground: in every direction it
    /// spans, some ground nearer than its nearest point stands steeper from the eye than its top.
    pub fn hides(&self, centre: Vec3, radius: f32) -> bool {
        let eye = self.eye;
        let far = (centre.x - eye.x).hypot(centre.z - eye.z);
        let near = far - radius;
        let steps = (near / self.step()) as usize;
        if !eye.is_finite() || !radius.is_finite() || near <= 0.0 || steps == 0 {
            return false;
        }
        let top = centre.y + radius - eye.y;
        let slope = if top > 0.0 { top / near } else { top / (far + radius) };
        let (toward, half) = ((centre.x - eye.x).atan2(centre.z - eye.z), (radius / far).asin());
        let index = |a: f32| a / std::f32::consts::TAU * DIRECTIONS as f32;
        let (a, b) = (index(toward - half).floor() as i64, index(toward + half).ceil() as i64);
        (a..=b).all(|i| self.slope(i.rem_euclid(DIRECTIONS as i64) as usize, steps) > slope)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A flat field 100 high with a ridge 500 high across z = 5000.
    fn ridge() -> Heightmap {
        let n = 21;
        let heights = (0..n * n).map(|i| if i / n == 10 { 500.0 } else { 100.0 }).collect();
        Heightmap { cells_x: n - 1, cells_z: n - 1, cell_size: 500.0, heights, flags: vec![0; n * n] }
    }

    #[test]
    fn a_ridge_hides_what_is_low_behind_it() {
        let mut h = Horizon::new(ridge());
        h.look_from(Vec3::new(5000.0, 200.0, 1000.0));
        // Behind the ridge and lower than the line over its top: hidden.
        assert!(h.hides(Vec3::new(5000.0, 200.0, 8000.0), 100.0));
        // Tall enough to show over the ridge.
        assert!(!h.hides(Vec3::new(5000.0, 200.0, 8000.0), 800.0));
        // In front of the ridge.
        assert!(!h.hides(Vec3::new(5000.0, 200.0, 3000.0), 100.0));
        // Around the eye.
        assert!(!h.hides(Vec3::new(5000.0, 100.0, 1500.0), 2000.0));
        // From above the ridge nothing is hidden.
        h.look_from(Vec3::new(5000.0, 3000.0, 1000.0));
        assert!(!h.hides(Vec3::new(5000.0, 200.0, 8000.0), 100.0));
    }
}
