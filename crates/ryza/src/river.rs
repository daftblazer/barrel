//! A field's water (`<FIELD>_river.bin` in its main container; docs/formats.md, "The water
//! file"): the rivers and the sea as bodies, each a ribbon along a line of points (a place, a
//! level, a width) with the look of its surface.
//!
//! Layout: an `i32` (-2), then bodies: a `u32` (0), 48 `f32` of the surface's look, a `u32`
//! count and `count` points of 4 `f32` (x, level, z, width). A body with no points ends the
//! list (the shore bubbles' tables follow it, not read).

/// The word that opens the file.
const MAGIC: i32 = -2;
/// The look's words in a body.
const LOOK_WORDS: usize = 48;
/// Where the look's words sit in a body, in words from its first (after the `u32` that opens it).
const REFRACTIVE_INDEX: usize = 12;
const TRANSPARENCY: usize = 13;
const ALBEDO: usize = 40;
const REFLECTANCE: usize = 44;
const STRENGTH: usize = 47;

/// A point of a ribbon, in the field's space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Point {
    pub x: f32,
    /// The surface's height here.
    pub level: f32,
    pub z: f32,
    /// How wide the ribbon is here.
    pub width: f32,
}

/// A body of water: a river or the sea.
#[derive(Clone, Debug, PartialEq)]
pub struct Body {
    pub points: Vec<Point>,
    pub albedo: [f32; 3],
    /// The reflectance at normal incidence (`vReflectance`).
    pub reflectance: [f32; 3],
    /// The `w` the game's albedo and reflectance share.
    pub strength: f32,
    pub refractive_index: f32,
    /// How far through the water the light goes before it is gone (the game's `fRrTransparency`).
    pub transparency: f32,
}

/// The bodies of a field's water file.
pub fn parse(bytes: &[u8]) -> Result<Vec<Body>, String> {
    let word = |o: usize| bytes.get(o..o + 4).map(|b| <[u8; 4]>::try_from(b).unwrap()).ok_or_else(|| "river.bin: truncated".to_string());
    let float = |o: usize| word(o).map(f32::from_le_bytes);
    if i32::from_le_bytes(word(0)?) != MAGIC {
        return Err("river.bin: not a water file".into());
    }
    let mut bodies = Vec::new();
    let mut at = 4;
    while at + 8 + 4 * LOOK_WORDS <= bytes.len() {
        let look = at + 4;
        let count = u32::from_le_bytes(word(look + 4 * LOOK_WORDS)?) as usize;
        if count == 0 || count > 1024 {
            break;
        }
        let points_at = look + 4 * LOOK_WORDS + 4;
        let vec3 = |w: usize| -> Result<[f32; 3], String> { Ok([float(look + 4 * w)?, float(look + 4 * w + 4)?, float(look + 4 * w + 8)?]) };
        let mut points = Vec::with_capacity(count);
        for i in 0..count {
            let p = points_at + 16 * i;
            points.push(Point { x: float(p)?, level: float(p + 4)?, z: float(p + 8)?, width: float(p + 12)? });
        }
        bodies.push(Body {
            points,
            albedo: vec3(ALBEDO)?,
            reflectance: vec3(REFLECTANCE)?,
            strength: float(look + 4 * STRENGTH)?,
            refractive_index: float(look + 4 * REFRACTIVE_INDEX)?,
            transparency: float(look + 4 * TRANSPARENCY)?,
        });
        at = points_at + 16 * count;
    }
    if bodies.is_empty() {
        return Err("river.bin: no bodies".into());
    }
    Ok(bodies)
}

/// One cubic piece of a ribbon's centre line: `a t^3 + b t^2 + c t + d` for t in 0 to 1, the
/// widths at its ends.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Piece {
    pub a: [f32; 3],
    pub b: [f32; 3],
    pub c: [f32; 3],
    pub d: [f32; 3],
    pub widths: [f32; 2],
}

impl Body {
    /// The ribbon's pieces, one between each two points: a Catmull-Rom curve through them
    /// (the end points repeated), position (x, level, z).
    // STAND-IN: the game's curve through the points (its vertex program takes a cubic per piece from the draw; how the pieces are made from the file is not read): Catmull-Rom.
    pub fn pieces(&self) -> Vec<Piece> {
        let at = |i: isize| {
            let p = self.points[i.clamp(0, self.points.len() as isize - 1) as usize];
            [p.x, p.level, p.z]
        };
        (0..self.points.len().saturating_sub(1))
            .map(|i| {
                let i = i as isize;
                let (p0, p1, p2, p3) = (at(i - 1), at(i), at(i + 1), at(i + 2));
                let k = |f: &dyn Fn(f32, f32, f32, f32) -> f32| -> [f32; 3] { std::array::from_fn(|j| f(p0[j], p1[j], p2[j], p3[j])) };
                Piece {
                    a: k(&|a, b, c, d| 0.5 * (-a + 3.0 * b - 3.0 * c + d)),
                    b: k(&|a, b, c, d| 0.5 * (2.0 * a - 5.0 * b + 4.0 * c - d)),
                    c: k(&|a, _, c, _| 0.5 * (c - a)),
                    d: k(&|_, b, _, _| b),
                    widths: [self.points[i as usize].width, self.points[i as usize + 1].width],
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A file of Rasenboden's shape: its river's first two points and look, and its sea's.
    fn file() -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&MAGIC.to_le_bytes());
        for (look, points) in <[((f32, f32, [f32; 3], [f32; 3], f32), Vec<[f32; 4]>); 2]>::from([
            ((1.368932, 1000.0, [0.06, 0.18226415, 0.6], [0.03; 3], 0.6990291), vec![[52462.96, 5037.21, 53336.78, 7557.97], [39596.48, 4705.0, 41930.36, 5062.61]]),
            ((1.33299, 315.534, [0.0, 0.1, 0.6], [0.03; 3], 0.5), vec![[236904.45, 4087.58, -121881.62, 304868.31]]),
        ]) {
            let mut words = [0f32; LOOK_WORDS];
            words[REFRACTIVE_INDEX] = look.0;
            words[TRANSPARENCY] = look.1;
            words[ALBEDO..ALBEDO + 3].copy_from_slice(&look.2);
            words[REFLECTANCE..REFLECTANCE + 3].copy_from_slice(&look.3);
            words[STRENGTH] = look.4;
            out.extend_from_slice(&0u32.to_le_bytes());
            words.iter().for_each(|w| out.extend_from_slice(&w.to_le_bytes()));
            out.extend_from_slice(&(points.len() as u32).to_le_bytes());
            points.iter().flatten().for_each(|w| out.extend_from_slice(&w.to_le_bytes()));
        }
        // The shore bubbles' tables follow.
        out.extend_from_slice(&[0; 704]);
        out
    }

    #[test]
    fn rasenbodens_two_bodies() {
        let bodies = parse(&file()).unwrap();
        assert_eq!(bodies.len(), 2);
        assert_eq!(bodies[0].points.len(), 2);
        assert_eq!(bodies[0].points[1], Point { x: 39596.48, level: 4705.0, z: 41930.36, width: 5062.61 });
        assert_eq!(bodies[0].albedo, [0.06, 0.18226415, 0.6]);
        assert_eq!(bodies[0].strength, 0.6990291);
        assert_eq!(bodies[1].points[0].width, 304868.31);
        assert_eq!(bodies[1].refractive_index, 1.33299);
        assert_eq!(bodies[1].transparency, 315.534);
    }

    #[test]
    fn a_piece_runs_through_its_points() {
        let body = &parse(&file()).unwrap()[0];
        let p = body.pieces()[0];
        let at = |t: f32| -> [f32; 3] { std::array::from_fn(|j| ((p.a[j] * t + p.b[j]) * t + p.c[j]) * t + p.d[j]) };
        assert_eq!(at(0.0), [52462.96, 5037.21, 53336.78]);
        let end = at(1.0);
        assert!((end[0] - 39596.48).abs() < 0.1 && (end[2] - 41930.36).abs() < 0.1);
    }

    #[test]
    fn other_files_are_refused() {
        assert!(parse(&[0; 16]).is_err());
    }
}
