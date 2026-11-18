//! The pixel board: the game's render targets for a captured frame (ryza-recomp's pixel mirror)
//! against Barrel's own at the matching stages (`BARREL_DUMP_STAGES`, docs/parity.md, "The pixel
//! board"), as errors over the frame, a grid of regions and masks, and side-by-side pictures.
//!
//!   barrel-parity pixels <game dump dir> <barrel dump dir> [<picture dir>]
//!
//! The game's dump dir is one frame's folder (`f2550`); its files are named
//! `<first draw>-<last draw>_<target>`. Floats are read from `.f32` (width, height, channels and
//! format in the `.txt` beside it), 8-bit targets from `.png` (as 0 to 1), Barrel's the same.

use std::path::Path;


type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

/// One target: RGBA floats, rows top to bottom.
struct Image {
    w: usize,
    h: usize,
    px: Vec<f32>,
}

impl Image {
    fn at(&self, x: usize, y: usize) -> &[f32] {
        &self.px[4 * (y * self.w + x)..][..4]
    }

    /// The part of this image `w` by `h` wide at `x`, `y`.
    fn crop(&self, x: usize, y: usize, w: usize, h: usize) -> Image {
        let mut px = Vec::with_capacity(4 * w * h);
        for row in y..y + h {
            px.extend_from_slice(&self.px[4 * (row * self.w + x)..][..4 * w]);
        }
        Image { w, h, px }
    }
}

fn load_f32(base: &Path) -> Result<Image> {
    // (Not `with_extension`: a target's own name has a dot in it.)
    let named = |ext: &str| base.with_file_name(format!("{}.{ext}", base.file_name().map_or_else(String::new, |n| n.to_string_lossy().into_owned())));
    let text = std::fs::read_to_string(named("txt"))?;
    let mut it = text.split_whitespace().map(|v| v.parse::<usize>());
    let (w, h) = (it.next().ok_or("txt")??, it.next().ok_or("txt")??);
    let bytes = std::fs::read(named("f32"))?;
    if bytes.len() != w * h * 16 {
        return Err(format!("{}: {} bytes for {w}x{h}", base.display(), bytes.len()).into());
    }
    let px = bytes.chunks_exact(4).map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect();
    Ok(Image { w, h, px })
}

/// An 8-bit RGB or RGBA PNG (not interlaced), its values 0 to 1.
fn load_png(path: &Path) -> Result<Image> {
    let png = barrel_formats::png::decode(&std::fs::read(path)?).map_err(|e| format!("{}: {e}", path.display()))?;
    let (w, h, channels) = (png.width, png.height, png.channels);
    let mut px = Vec::with_capacity(4 * w * h);
    for p in png.pixels.chunks_exact(channels) {
        px.extend(p[..3].iter().map(|&v| f32::from(v) / 255.0));
        px.push(if channels == 4 { f32::from(p[3]) / 255.0 } else { 1.0 });
    }
    Ok(Image { w, h, px })
}

/// A target by its file's stem: a `.f32` if there is one, else a `.png`.
fn load(dir: &Path, stem: &str) -> Result<Image> {
    if dir.join(format!("{stem}.f32")).exists() {
        load_f32(&dir.join(stem))
    } else {
        load_png(&dir.join(format!("{stem}.png")))
    }
}

/// One pair of targets: the game's file (this capture's frame 2550) and Barrel's name, and the
/// scale of its error (255 for the 8-bit picture).
struct Stage {
    name: &'static str,
    game: &'static str,
    barrel: &'static str,
    scale: f64,
    /// The game's alpha is the value (the sun's share), taken as grey; its colour is not.
    alpha: bool,
    /// The mean of the regions below is printed, the game's and Barrel's.
    regions: bool,
}

/// Regions of the 1920 by 1080 pictures (x0, y0, x1, y1) where the picture is judged by eye:
/// the trees' canopy, the trunk, the grass, the sand, the far hills and field.
const REGIONS: &[(&str, [usize; 4])] = &[
    ("canopy left", [100, 40, 600, 260]),
    ("mid tree", [960, 150, 1250, 330]),
    ("trunk", [560, 150, 680, 420]),
    ("grass", [100, 800, 500, 900]),
    ("sand", [1100, 950, 1500, 1050]),
    ("hills right", [1500, 330, 1800, 350]),
    ("far field", [700, 380, 820, 410]),
    ("canopy dark", [150, 300, 500, 420]),
    ("stone wall", [860, 400, 1000, 450]),
    ("shaded path", [600, 620, 800, 680]),
];

/// The mean of a region's three channels, per channel, over the pixels the game drew before the
/// sky (depth under the far plane) when it has `masks`.
fn region_mean(im: &Image, masks: Option<&Image>, [x0, y0, x1, y1]: [usize; 4]) -> [f64; 3] {
    let mut sum = [0f64; 3];
    let mut n = 0f64;
    for y in y0..y1.min(im.h) {
        for x in x0..x1.min(im.w) {
            if masks.is_some_and(|m| m.at(x, y)[0] >= 1.0) {
                continue;
            }
            let p = im.at(x, y);
            for c in 0..3 {
                sum[c] += f64::from(p[c]);
            }
            n += 1.0;
        }
    }
    sum.map(|v| if n > 0.0 { v / n } else { 0.0 })
}

/// In the frame's order. Where Barrel has no pass of its own, the game's later stage is the
/// match: its scene fog is in each fragment's light, so its picture before the sky is compared
/// with the game's after the fog (`lit`: the game's lit scene without it, kept for the
/// ground's own error), and its finished sky with the game's after both fogs.
const STAGES: &[Stage] = &[
    Stage { name: "cloud_map", game: "1911-1911_rt1024x1024.27#4", barrel: "cloud_map", scale: 1.0, alpha: false, regions: false },
    Stage { name: "plane", game: "1912-1912_rt1920x1080.26#4", barrel: "plane", scale: 1.0, alpha: false, regions: false },
    Stage { name: "dome1", game: "1913-1913_rt1920x1080.26#4", barrel: "dome1", scale: 1.0, alpha: false, regions: false },
    Stage { name: "dome", game: "1914-1914_rt1920x1080.26#4", barrel: "dome", scale: 1.0, alpha: false, regions: false },
    Stage { name: "ao", game: "914-914_rt960x540.27#2", barrel: "ao", scale: 1.0, alpha: false, regions: true },
    Stage { name: "shadow", game: "1892-1894_rt1920x1080.9#15", barrel: "shadow", scale: 1.0, alpha: true, regions: true },
    Stage { name: "lit", game: "1895-1898_rt1920x1080.26#4", barrel: "lit", scale: 1.0, alpha: false, regions: true },
    Stage { name: "lit_fogged", game: "1919-1919_rt1920x1080.26#4", barrel: "sky", scale: 1.0, alpha: false, regions: false },
    Stage { name: "sky", game: "1912-1916_rt1920x1080.26#4", barrel: "sky", scale: 1.0, alpha: false, regions: false },
    Stage { name: "scene", game: "2037-2037_rt1920x1080.26#4", barrel: "scene", scale: 1.0, alpha: false, regions: true },
    Stage { name: "bright", game: "2052-2052_rt482x272.26#1", barrel: "bright", scale: 1.0, alpha: false, regions: false },
    Stage { name: "bloom", game: "2057-2057_rt240x135.26#1", barrel: "bloom", scale: 1.0, alpha: false, regions: false },
    Stage { name: "tone", game: "2078-2078_rt1920x1080.90#7", barrel: "tone", scale: 255.0, alpha: false, regions: false },
];

/// The game's depth and stencil before the sky, for the masks.
const MASKS: &str = "1895-1898_rt1920x1080.19#15.depth";

#[derive(Default, Clone, Copy)]
struct Sums {
    abs: f64,
    game: f64,
    n: f64,
}

impl Sums {
    fn add(&mut self, game: &[f32], barrel: &[f32]) {
        for c in 0..3 {
            self.abs += f64::from((game[c] - barrel[c]).abs());
            self.game += f64::from(game[c].abs());
            self.n += 1.0;
        }
    }

    fn mean_abs(&self, scale: f64) -> f64 {
        if self.n > 0.0 { self.abs / self.n * scale } else { 0.0 }
    }

    /// The error over the game's own light: `sum |g - b| / sum |g|`.
    fn rel(&self) -> f64 {
        if self.game > 0.0 { self.abs / self.game } else { 0.0 }
    }
}

const GRID: (usize, usize) = (8, 6);

/// A value to show: HDR light by `x / (1 + x)` and a gamma, the 8-bit pictures as they are.
fn shown(v: f32, hdr: bool) -> f32 {
    if hdr { (v / (1.0 + v)).clamp(0.0, 1.0).powf(1.0 / 2.2) } else { v.clamp(0.0, 1.0) }
}

/// A picture of the pair at most 960 wide: the game's, Barrel's and the difference (red where
/// Barrel is brighter, blue where it is darker, four times as strong).
fn side_by_side(game: &Image, barrel: &Image, hdr: bool) -> (u32, u32, Vec<u8>) {
    let tile_w = (960 / 3).min(game.w);
    let tile_h = (game.h * tile_w / game.w).max(1);
    let mut out = vec![255u8; 3 * tile_w * 4 * tile_h];
    let stride = 3 * tile_w * 4;
    for ty in 0..tile_h {
        for tx in 0..tile_w {
            // The mean of the source texels this one covers.
            let (x0, x1) = (tx * game.w / tile_w, ((tx + 1) * game.w / tile_w).max(tx * game.w / tile_w + 1));
            let (y0, y1) = (ty * game.h / tile_h, ((ty + 1) * game.h / tile_h).max(ty * game.h / tile_h + 1));
            let (mut g, mut b, mut n) = ([0f32; 3], [0f32; 3], 0f32);
            for y in y0..y1.min(game.h) {
                for x in x0..x1.min(game.w) {
                    for c in 0..3 {
                        g[c] += shown(game.at(x, y)[c], hdr);
                        b[c] += shown(barrel.at(x, y)[c], hdr);
                    }
                    n += 1.0;
                }
            }
            let put = |col: usize, rgb: [f32; 3], out: &mut Vec<u8>| {
                for c in 0..3 {
                    out[4 * (ty * stride / 4 + col * tile_w + tx) + c] = (rgb[c] * 255.0 + 0.5).clamp(0.0, 255.0) as u8;
                }
            };
            let (g, b) = (g.map(|v| v / n), b.map(|v| v / n));
            let d = [0, 1, 2].map(|c| b[c] - g[c]);
            let lum = (d[0] + d[1] + d[2]) / 3.0;
            put(0, g, &mut out);
            put(1, b, &mut out);
            put(2, [(lum * 4.0).max(0.0), 0.0, (-lum * 4.0).max(0.0)], &mut out);
        }
    }
    ((3 * tile_w) as u32, tile_h as u32, out)
}

/// Where Barrel's scene draws nothing against where the game's depth is at the far plane
/// (sky), on Barrel's half-size normal and depth target (`depth`: view depth in the fourth
/// channel, `ao::CLEAR_DEPTH` where nothing is drawn). A half-size pixel is the game's when at
/// least two of its four full-size pixels are. Prints the shares of the picture and, for the
/// cells of the 6 by 4 grid of 320 by 270 pixels that hold the crowns, the share of each cell.
fn coverage(masks: &Image, depth: &Image) {
    if (masks.w, masks.h) != (2 * depth.w, 2 * depth.h) {
        println!("coverage: sizes differ, {}x{} against {}x{}", masks.w, masks.h, depth.w, depth.h);
        return;
    }
    // Where nothing was drawn, the target holds its clear depth.
    let far = |px: &[f32]| px[3] >= 1.0e8;
    let game_sky = |x: usize, y: usize| (0..4).filter(|k| masks.at(2 * x + k % 2, 2 * y + k / 2)[0] >= 1.0).count() >= 2;
    let (mut g, mut b, mut b_only, mut g_only) = (0usize, 0usize, 0usize, 0usize);
    const CELL: (usize, usize) = (160, 135);
    let mut cells = [[0usize; 3]; 24];
    for y in 0..depth.h {
        for x in 0..depth.w {
            let (gs, bs) = (game_sky(x, y), far(depth.at(x, y)));
            g += usize::from(gs);
            b += usize::from(bs);
            b_only += usize::from(bs && !gs);
            g_only += usize::from(gs && !bs);
            let c = &mut cells[y / CELL.1 * 6 + x / CELL.0];
            c[0] += usize::from(bs && !gs);
            c[1] += usize::from(gs && !bs);
            c[2] += 1;
        }
    }
    let n = (depth.w * depth.h) as f64;
    let pct = |v: usize, of: f64| 100.0 * v as f64 / of;
    println!("coverage: game sky {:.2}%, barrel sky {:.2}%, barrel-only {:.2}%, game-only {:.2}%", pct(g, n), pct(b, n), pct(b_only, n), pct(g_only, n));
    for (row, col) in [(0, 2), (0, 3)] {
        let c = cells[row * 6 + col];
        println!("  cell ({col},{row}): barrel-only {:.1}%, game-only {:.1}%", pct(c[0], c[2] as f64), pct(c[1], c[2] as f64));
    }
}

/// The cloud strip's own contribution (the stage after it less the stage before, the game's
/// `1915` less `1914`, Barrel's `sky` less `dome`) over the sky pixels, by bands of rows: the
/// mean absolute difference of the two contributions, and each one's mean light (over the
/// pixels that are sky in both).
fn strip(game: &Path, barrel: &Path, masks: &Image) {
    // Only where both pictures have sky: the trees' edges differ in coverage, which is not the
    // strip's.
    let Ok(depth) = load(barrel, "depth") else {
        println!("strip: no depth");
        return;
    };
    let (Ok(g1), Ok(g0), Ok(b1), Ok(b0)) = (load(game, "1915-1915_rt1920x1080.26#4"), load(game, "1914-1914_rt1920x1080.26#4"), load(barrel, "sky"), load(barrel, "dome")) else {
        println!("strip: missing");
        return;
    };
    if [&g0, &b1, &b0, masks].iter().any(|i| (i.w, i.h) != (g1.w, g1.h)) {
        println!("strip: sizes differ");
        return;
    }
    const BAND: usize = 60;
    let mut bands = vec![([0f64; 3], [0f64; 3], 0f64, 0f64); g1.h.div_ceil(BAND)];
    for y in 0..g1.h {
        for x in 0..g1.w {
            if masks.at(x, y)[0] < 1.0 || depth.at(x / 2, y / 2)[3] < 1.0e8 {
                continue;
            }
            let band = &mut bands[y / BAND];
            for c in 0..3 {
                let (g, b) = (g1.at(x, y)[c] - g0.at(x, y)[c], b1.at(x, y)[c] - b0.at(x, y)[c]);
                band.0[c] += f64::from(g);
                band.1[c] += f64::from(b);
                band.3 += f64::from((g - b).abs());
            }
            band.2 += 1.0;
        }
    }
    let n: f64 = bands.iter().map(|b| b.2).sum();
    let abs: f64 = bands.iter().map(|b| b.3).sum();
    println!("strip: mean abs {:.4} over {} sky pixels", abs / (3.0 * n.max(1.0)), n as u64);
    for (i, b) in bands.iter().enumerate().filter(|(_, b)| b.2 > 0.0) {
        let m = |v: [f64; 3]| v.map(|v| v / b.2);
        let (g, d) = (m(b.0), m(b.1));
        println!("  rows {}-{}: mean abs {:.4}, game {:.3} {:.3} {:.3}, barrel {:.3} {:.3} {:.3}", i * BAND, (i + 1) * BAND, b.3 / (3.0 * b.2), g[0], g[1], g[2], d[0], d[1], d[2]);
    }
}

pub fn run(game: &Path, barrel: &Path, pictures: Option<&Path>) -> Result<()> {
    if let Some(p) = pictures {
        std::fs::create_dir_all(p)?;
    }
    let masks = load_f32(&game.join(MASKS)).ok();
    let mut tone = None;
    for stage in STAGES {
        let (g, b) = match (load(game, stage.game), load(barrel, if stage.barrel == "tone" { "tone" } else { stage.barrel })) {
            (Ok(g), Ok(b)) => (g, b),
            (g, b) => {
                println!("{}: missing ({})", stage.name, g.err().or(b.err()).map_or(String::new(), |e| e.to_string()));
                continue;
            }
        };
        let g = if stage.alpha { Image { px: g.px.chunks_exact(4).flat_map(|p| [p[3], p[3], p[3], 1.0]).collect(), ..g } } else { g };
        // The game's bloom targets carry a border round the quarter-size picture's own.
        let g = if g.w > b.w || g.h > b.h { g.crop((g.w - b.w) / 2, (g.h - b.h) / 2, b.w, b.h) } else { g };
        if (g.w, g.h) != (b.w, b.h) {
            println!("{}: sizes differ, {}x{} against {}x{}", stage.name, g.w, g.h, b.w, b.h);
            continue;
        }
        let mut all = Sums::default();
        let mut grid = vec![Sums::default(); GRID.0 * GRID.1];
        // Sky (nothing drawn before it: depth at the far plane), the characters (the stencil's
        // 8) and the stage (the rest).
        let mut by_mask = [Sums::default(); 3];
        let full = masks.as_ref().filter(|m| (m.w, m.h) == (g.w, g.h));
        for y in 0..g.h {
            for x in 0..g.w {
                let (gp, bp) = (g.at(x, y), b.at(x, y));
                all.add(gp, bp);
                grid[(y * GRID.1 / g.h) * GRID.0 + x * GRID.0 / g.w].add(gp, bp);
                if let Some(m) = full {
                    let px = m.at(x, y);
                    let k = if px[0] >= 1.0 { 0 } else if px[1] as u32 == 8 { 1 } else { 2 };
                    by_mask[k].add(gp, bp);
                }
            }
        }
        let (worst, w) = grid.iter().enumerate().map(|(i, s)| (i, s.mean_abs(stage.scale))).fold((0, 0.0), |a, v| if v.1 > a.1 { v } else { a });
        println!("{}: mean abs {:.4}, rel {:.4}, worst region ({},{}) {:.4}", stage.name, all.mean_abs(stage.scale), all.rel(), worst % GRID.0, worst / GRID.0, w);
        if full.is_some() {
            for (name, s) in ["sky", "characters", "stage"].iter().zip(by_mask) {
                println!("  {name}: mean abs {:.4}, rel {:.4}", s.mean_abs(stage.scale), s.rel());
            }
        }
        // The regions are in the full-size picture's pixels; a half-size stage (the occlusion) takes
        // them halved (and has no masks).
        if stage.regions && ((g.w, g.h) == (1920, 1080) || (g.w, g.h) == (960, 540)) {
            let k = 1920 / g.w;
            for (name, r) in REGIONS {
                let r = r.map(|v| v / k);
                let (gm, bm) = (region_mean(&g, full, r), region_mean(&b, full, r));
                println!("  region {name}: game {:.3} {:.3} {:.3}, barrel {:.3} {:.3} {:.3}", gm[0], gm[1], gm[2], bm[0], bm[1], bm[2]);
            }
        }
        if stage.name == "tone" {
            tone = Some(all.mean_abs(stage.scale));
        }
        if let Some(p) = pictures {
            let (w, h, rgba) = side_by_side(&g, &b, stage.scale == 1.0);
            std::fs::write(p.join(format!("{}.png", stage.name)), barrel_formats::png::encode_rgba(w, h, &rgba))?;
        }
    }
    if let Some(m) = &masks {
        strip(game, barrel, m);
    }
    if let (Some(m), Ok(d)) = (&masks, load_f32(&barrel.join("depth"))) {
        coverage(m, &d);
    }
    match tone {
        Some(t) => println!("pixels: tone mean abs {t:.2}"),
        None => return Err("no tone pair".into()),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_png_reads_back_as_written() {
        let rgba: Vec<u8> = (0..4 * 6).map(|i| (i * 11) as u8).collect();
        let dir = std::env::temp_dir().join(format!("barrel-pixels-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("t.png");
        std::fs::write(&path, barrel_formats::png::encode_rgba(3, 2, &rgba)).unwrap();
        let im = load_png(&path).unwrap();
        std::fs::remove_dir_all(&dir).ok();
        assert_eq!((im.w, im.h), (3, 2));
        let px = |i: usize| f32::from(rgba[i]) / 255.0;
        assert_eq!(im.at(2, 1), [px(20), px(21), px(22), px(23)]);
    }

    #[test]
    fn errors_are_over_the_games_light() {
        let mut s = Sums::default();
        s.add(&[1.0, 2.0, 3.0, 1.0], &[2.0, 2.0, 1.0, 1.0]);
        assert_eq!(s.mean_abs(1.0), 1.0);
        assert!((s.rel() - 0.5).abs() < 1e-9);
    }
}
