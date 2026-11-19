//! The pixel board: the game's render targets for a captured frame (ryza-recomp's pixel mirror)
//! against Barrel's own at the matching stages (`BARREL_DUMP_STAGES`, docs/parity.md, "The pixel
//! board"), as errors over the frame, a grid of regions and masks, and side-by-side pictures.
//!
//!   barrel-parity pixels <view file> <game dump dir> <barrel dump dir> [<picture dir>]
//!
//! The game's dump dir is one frame's folder (`f2550`); its files are named
//! `<first draw>-<last draw>_<target>`. Floats are read from `.f32` (width, height, channels and
//! format in the `.txt` beside it), 8-bit targets from `.png` (as 0 to 1), Barrel's the same.
//! The view file (`docs/ryza/views/`) names the game's file for each stage and holds the
//! view's matrix and regions.

use std::collections::HashMap;
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

/// One pair of targets: the stage's name (the view names the game's file for it) and Barrel's
/// name, and the scale of its error (255 for the 8-bit picture).
struct Stage {
    name: &'static str,
    barrel: &'static str,
    scale: f64,
    /// The game's alpha is the value (the sun's share), taken as grey; its colour is not.
    alpha: bool,
    /// The mean of the regions below is printed, the game's and Barrel's.
    regions: bool,
    /// Barrel's alpha is the value too (taken as grey), as the game's with `alpha`.
    own_alpha: bool,
}

/// One view of a captured frame (a text file, `docs/ryza/views/`), a line each: `stage <name>
/// <game file stem>`, `masks <stem>` (the game's depth and stencil before the sky), `normal
/// <stem>` (its normal target), `strip <after stem> <before stem>` (the cloud strip's two
/// targets), `view <nine numbers>` (the capture's `mWV` rotation, row by row) and `region <name>
/// x0 y0 x1 y1` (in the 1920 by 1080 picture's pixels, where the picture is judged by eye; `_`
/// in the name is a space). A line starting with `#`, or a `#` after a space, is a comment (a
/// target's own name has a `#` in it). A stage the view does not name is not compared.
struct View {
    stages: HashMap<String, String>,
    masks: Option<String>,
    normal: Option<String>,
    strip: Option<(String, String)>,
    rows: [[f32; 3]; 3],
    regions: Vec<(String, [usize; 4])>,
}

impl View {
    fn parse(text: &str) -> Result<View> {
        let mut v = View { stages: HashMap::new(), masks: None, normal: None, strip: None, rows: [[0.0; 3]; 3], regions: Vec::new() };
        for raw in text.lines() {
            let line = raw.find(" #").map_or(raw, |i| &raw[..i]);
            let w: Vec<&str> = line.split_whitespace().collect();
            match w.as_slice() {
                [] => {}
                [first, ..] if first.starts_with('#') => {}
                ["stage", name, stem] => {
                    v.stages.insert((*name).into(), (*stem).into());
                }
                ["masks", stem] => v.masks = Some((*stem).into()),
                ["normal", stem] => v.normal = Some((*stem).into()),
                ["strip", after, before] => v.strip = Some(((*after).into(), (*before).into())),
                ["view", n @ ..] if n.len() == 9 => {
                    for (i, t) in n.iter().enumerate() {
                        v.rows[i / 3][i % 3] = t.parse()?;
                    }
                }
                ["region", name, r @ ..] if r.len() == 4 => {
                    let r: Vec<usize> = r.iter().map(|t| t.parse()).collect::<std::result::Result<_, _>>()?;
                    v.regions.push(((*name).replace('_', " "), [r[0], r[1], r[2], r[3]]));
                }
                _ => return Err(format!("view file: cannot read `{raw}`").into()),
            }
        }
        Ok(v)
    }
}

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

/// Barrel's own characters: the connected pieces of its coverage stage (`chara`, 1 where a
/// character-path draw is) that touch the game's characters (stencil 8): her and the townsfolk.
fn own_characters(masks: &Image, chara: &Image) -> Vec<bool> {
    let (w, h) = (chara.w, chara.h);
    let covered = |i: usize| chara.px[4 * i] > 0.5;
    let game = |i: usize| masks.px[4 * i] < 1.0 && masks.px[4 * i + 1] as u32 == 8;
    let mut keep = vec![false; w * h];
    let mut stack = Vec::new();
    for start in (0..w * h).filter(|&i| game(i) && covered(i)) {
        if keep[start] {
            continue;
        }
        keep[start] = true;
        stack.push(start);
        while let Some(i) = stack.pop() {
            let (x, y) = (i % w, i / w);
            for (nx, ny) in [(x.wrapping_sub(1), y), (x + 1, y), (x, y.wrapping_sub(1)), (x, y + 1)] {
                if nx < w && ny < h && covered(ny * w + nx) && !keep[ny * w + nx] {
                    keep[ny * w + nx] = true;
                    stack.push(ny * w + nx);
                }
            }
        }
    }
    keep
}

/// What the characters mask holds of a picture: the colour of the game's characters (its
/// stencil's 8) and of Barrel's own (`own_characters`), each over its own pixels,
/// and the two sorted luminance and saturation distributions compared (the sorted lists
/// resampled to the shorter one's length), so the pose does not count. The error of the
/// distributions is the mean absolute difference over the quantiles, in the stage's units (and
/// over the game's mean luminance); the saturation's is `(max - min) / max`.
fn chara_stats(name: &str, game: &Image, barrel: &Image, masks: &Image, own: &[bool], scale: f64) {
    type Px = ([f32; 3], f32, f32);
    let (mut g, mut b): (Vec<Px>, Vec<Px>) = (Vec::new(), Vec::new());
    let take = |p: &[f32]| -> Px {
        let lum = 0.2126 * p[0] + 0.7152 * p[1] + 0.0722 * p[2];
        let hi = p[0].max(p[1]).max(p[2]);
        let sat = if hi > 1.0e-6 { (hi - p[0].min(p[1]).min(p[2])) / hi } else { 0.0 };
        ([p[0], p[1], p[2]], lum, sat)
    };
    for y in 0..game.h {
        for x in 0..game.w {
            let m = masks.at(x, y);
            if m[0] < 1.0 && m[1] as u32 == 8 {
                g.push(take(game.at(x, y)));
            }
            if own[y * game.w + x] {
                b.push(take(barrel.at(x, y)));
            }
        }
    }
    if g.is_empty() || b.is_empty() {
        println!("  chara {name}: game {} pixels, barrel {}", g.len(), b.len());
        return;
    }
    let mean = |v: &[Px]| -> [f64; 3] { std::array::from_fn(|c| v.iter().map(|p| f64::from(p.0[c])).sum::<f64>() / v.len() as f64) };
    let (gm, bm) = (mean(&g), mean(&b));
    let sorted = |v: &[Px], k: usize| -> Vec<f32> {
        let mut s: Vec<f32> = v.iter().map(|p| if k == 0 { p.1 } else { p.2 }).collect();
        s.sort_by(f32::total_cmp);
        s
    };
    let distance = |a: &[f32], b: &[f32]| -> f64 {
        let n = a.len().min(b.len());
        (0..n).map(|i| f64::from((a[i * a.len() / n] - b[i * b.len() / n]).abs())).sum::<f64>() / n as f64
    };
    let (gl, bl) = (sorted(&g, 0), sorted(&b, 0));
    let (gs, bs) = (sorted(&g, 1), sorted(&b, 1));
    let glum = gl.iter().map(|&v| f64::from(v)).sum::<f64>() / gl.len() as f64;
    let blum = bl.iter().map(|&v| f64::from(v)).sum::<f64>() / bl.len() as f64;
    println!(
        "  chara {name}: game {:.4} {:.4} {:.4} over {}, barrel {:.4} {:.4} {:.4} over {}; mean luminance {:+.1}%; sorted luminance mean abs {:.4} (rel {:.3}), sorted saturation mean abs {:.4}",
        gm[0] * scale, gm[1] * scale, gm[2] * scale, g.len(), bm[0] * scale, bm[1] * scale, bm[2] * scale, b.len(),
        100.0 * (blum - glum) / glum.max(1.0e-9), distance(&gl, &bl) * scale, distance(&gl, &bl) / glum.max(1.0e-9), distance(&gs, &bs)
    );
}

/// The largest connected piece of a mask: the player, without the townsfolk's few pixels.
fn largest_piece(mask: &[bool], w: usize) -> Vec<bool> {
    let h = mask.len() / w;
    let mut label = vec![0u32; mask.len()];
    let (mut best, mut best_size, mut next) = (0u32, 0usize, 0u32);
    let mut stack = Vec::new();
    for start in (0..mask.len()).filter(|&i| mask[i]) {
        if label[start] != 0 {
            continue;
        }
        next += 1;
        label[start] = next;
        stack.push(start);
        let mut size = 0;
        while let Some(i) = stack.pop() {
            size += 1;
            let (x, y) = (i % w, i / w);
            for (nx, ny) in [(x.wrapping_sub(1), y), (x + 1, y), (x, y.wrapping_sub(1)), (x, y + 1)] {
                if nx < w && ny < h && mask[ny * w + nx] && label[ny * w + nx] == 0 {
                    label[ny * w + nx] = next;
                    stack.push(ny * w + nx);
                }
            }
        }
        if size > best_size {
            (best, best_size) = (next, size);
        }
    }
    label.iter().map(|&l| l == best).collect()
}

/// Her body's bands, each in its own picture's coordinates (a fraction of the height of the
/// largest piece of the character mask, from its top): the boots (the lowest 15%) and the thighs
/// (62% to 80%), the mean colour of the game's and Barrel's.
fn chara_bands(name: &str, game: &Image, barrel: &Image, masks: &Image, own: &[bool], scale: f64) {
    let w = game.w;
    let game_mask: Vec<bool> = (0..w * game.h).map(|i| masks.px[4 * i] < 1.0 && masks.px[4 * i + 1] as u32 == 8).collect();
    let (gm, bm) = (largest_piece(&game_mask, w), largest_piece(own, w));
    let rows = |m: &[bool]| {
        let ys: Vec<usize> = (0..m.len()).filter(|&i| m[i]).map(|i| i / w).collect();
        (ys.iter().copied().min().unwrap_or(0), ys.iter().copied().max().unwrap_or(0))
    };
    let mean = |im: &Image, m: &[bool], (y0, y1): (usize, usize), (a, b): (f64, f64)| -> [f64; 3] {
        let h = (y1 - y0 + 1) as f64;
        let (lo, hi) = (y0 + (a * h) as usize, y0 + (b * h) as usize);
        let (mut sum, mut n) = ([0f64; 3], 0f64);
        for y in lo..=hi.min(y1) {
            for x in 0..w {
                if m[y * w + x] {
                    let p = im.at(x, y);
                    for c in 0..3 {
                        sum[c] += f64::from(p[c]);
                    }
                    n += 1.0;
                }
            }
        }
        sum.map(|v| v * scale / n.max(1.0))
    };
    let (gr, br) = (rows(&gm), rows(&bm));
    for (band, range) in [("boots", (0.85, 1.0)), ("thighs", (0.62, 0.80))] {
        let (g, b) = (mean(game, &gm, gr, range), mean(barrel, &bm, br, range));
        println!("  chara {name} {band}: game {:.4} {:.4} {:.4}, barrel {:.4} {:.4} {:.4}", g[0], g[1], g[2], b[0], b[1], b[2]);
    }
}

/// In the frame's order. Where Barrel has no pass of its own, the game's later stage is the
/// match: its scene fog is in each fragment's light, so its picture before the sky is compared
/// with the game's after the fog (`lit`: the game's lit scene without it, kept for the
/// ground's own error), and its finished sky with the game's after both fogs.
const STAGES: &[Stage] = &[
    Stage { name: "cloud_map", barrel: "cloud_map", scale: 1.0, alpha: false, regions: false, own_alpha: false },
    Stage { name: "plane", barrel: "plane", scale: 1.0, alpha: false, regions: false, own_alpha: false },
    Stage { name: "dome1", barrel: "dome1", scale: 1.0, alpha: false, regions: false, own_alpha: false },
    Stage { name: "dome", barrel: "dome", scale: 1.0, alpha: false, regions: false, own_alpha: false },
    Stage { name: "ao", barrel: "ao", scale: 1.0, alpha: false, regions: true, own_alpha: false },
    Stage { name: "shadow", barrel: "shadow", scale: 1.0, alpha: true, regions: true, own_alpha: false },
    Stage { name: "lit", barrel: "lit", scale: 1.0, alpha: false, regions: true, own_alpha: false },
    Stage { name: "lit_fogged", barrel: "sky", scale: 1.0, alpha: false, regions: false, own_alpha: false },
    Stage { name: "sky", barrel: "sky", scale: 1.0, alpha: false, regions: false, own_alpha: false },
    // The reflections' march (half size): its colour, then its alpha, the weight.
    Stage { name: "reflections", barrel: "reflections", scale: 1.0, alpha: false, regions: false, own_alpha: false },
    Stage { name: "reflections_alpha", barrel: "reflections", scale: 1.0, alpha: true, regions: false, own_alpha: true },
    Stage { name: "scene", barrel: "scene", scale: 1.0, alpha: false, regions: true, own_alpha: false },
    Stage { name: "bright", barrel: "bright", scale: 1.0, alpha: false, regions: false, own_alpha: false },
    Stage { name: "bloom", barrel: "bloom", scale: 1.0, alpha: false, regions: false, own_alpha: false },
    Stage { name: "tone", barrel: "tone", scale: 255.0, alpha: false, regions: true, own_alpha: false },
];

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

/// The game's view-space normal from its octahedral pair `(a, b)` (stored 0 to 1): with
/// `p = (z, x, y) / |.|_1`, the front (`p.x >= 0`) holds `(p.y, p.z)` and the back
/// `((1 - |p.z|) sign(p.y), (1 - |p.y|) sign(p.z))`. Returns the unit vector `(x, y, z)`.
fn octahedral_view(a: f32, b: f32) -> [f32; 3] {
    let (u, v) = (a * 2.0 - 1.0, b * 2.0 - 1.0);
    let (px, py, pz) = if u.abs() + v.abs() <= 1.0 {
        (1.0 - u.abs() - v.abs(), u, v)
    } else {
        (1.0 - u.abs() - v.abs(), u.signum() * (1.0 - v.abs()), v.signum() * (1.0 - u.abs()))
    };
    let len = (px * px + py * py + pz * pz).sqrt().max(1.0e-9);
    [py / len, pz / len, px / len]
}

/// The game's normal in Barrel's render space (the world with x mirrored): the view vector
/// turned back by `mWV`'s transpose.
fn game_normal_render(a: f32, b: f32, rows: &[[f32; 3]; 3]) -> [f32; 3] {
    let v = octahedral_view(a, b);
    let w: [f32; 3] = std::array::from_fn(|c| (0..3).map(|r| v[r] * rows[r][c]).sum());
    [-w[0], w[1], w[2]]
}

/// The mapped normals over the stage's pixels (stencil 2) as an angle between Barrel's (the
/// occlusion pass's `depth` target, half size, render space) and the game's (the G-buffer's
/// target at the half-size pixel's top-left full-size pixel): mean and median in degrees, and
/// the same over the regions. A picture of the angle (0 to 45 degrees, black to white) is
/// written when there is a picture folder.
fn normals(view: &View, game: &Path, barrel: &Path, masks: &Image, pictures: Option<&Path>) {
    let (Some(stem), Ok(b)) = (&view.normal, load_f32(&barrel.join("depth"))) else {
        println!("normal: missing");
        return;
    };
    let Ok(g) = load(game, stem) else {
        println!("normal: missing");
        return;
    };
    if (g.w, g.h) != (2 * b.w, 2 * b.h) || (masks.w, masks.h) != (g.w, g.h) {
        println!("normal: sizes differ");
        return;
    }
    let angle = |x: usize, y: usize| -> Option<f32> {
        let (gx, gy) = (2 * x, 2 * y);
        let (m, bp) = (masks.at(gx, gy), b.at(x, y));
        if m[0] >= 1.0 || m[1] as u32 != 2 || bp[3] >= 1.0e8 {
            return None;
        }
        let gp = g.at(gx, gy);
        let gn = game_normal_render(gp[0], gp[1], &view.rows);
        let len = (bp[0] * bp[0] + bp[1] * bp[1] + bp[2] * bp[2]).sqrt().max(1.0e-9);
        let cos = (0..3).map(|c| gn[c] * bp[c] / len).sum::<f32>();
        Some(cos.clamp(-1.0, 1.0).acos().to_degrees())
    };
    let stats = |mut v: Vec<f32>| -> String {
        if v.is_empty() {
            return "no pixels".into();
        }
        v.sort_by(f32::total_cmp);
        format!("median {:.1}, mean {:.1} degrees over {} pixels", v[v.len() / 2], v.iter().sum::<f32>() / v.len() as f32, v.len())
    };
    let mut all = Vec::new();
    let mut by_region: Vec<Vec<f32>> = vec![Vec::new(); view.regions.len()];
    let mut out = vec![255u8; 4 * b.w * b.h];
    for y in 0..b.h {
        for x in 0..b.w {
            let Some(a) = angle(x, y) else { continue };
            all.push(a);
            for ((_, [x0, y0, x1, y1]), v) in view.regions.iter().zip(&mut by_region) {
                if (x0 / 2..x1 / 2).contains(&x) && (y0 / 2..y1 / 2).contains(&y) {
                    v.push(a);
                }
            }
            let g = (a / 45.0).clamp(0.0, 1.0) * 255.0;
            out[4 * (y * b.w + x)..][..3].fill(g as u8);
        }
    }
    println!("normal: {}", stats(all));
    for ((name, _), v) in view.regions.iter().zip(by_region) {
        println!("  region {name}: {}", stats(v));
    }
    if let Some(p) = pictures {
        let _ = std::fs::write(p.join("normal.png"), barrel_formats::png::encode_rgba(b.w as u32, b.h as u32, &out));
    }
}

/// The cloud strip's own contribution (the stage after it less the stage before, the game's
/// the view's `strip` pair, Barrel's `sky` less `dome`) over the sky pixels, by bands of rows: the
/// mean absolute difference of the two contributions, and each one's mean light (over the
/// pixels that are sky in both).
fn strip(view: &View, game: &Path, barrel: &Path, masks: &Image) {
    let Some((after, before)) = &view.strip else { return };
    // Only where both pictures have sky: the trees' edges differ in coverage, which is not the
    // strip's.
    let Ok(depth) = load(barrel, "depth") else {
        println!("strip: no depth");
        return;
    };
    let (Ok(g1), Ok(g0), Ok(b1), Ok(b0)) = (load(game, after), load(game, before), load(barrel, "sky"), load(barrel, "dome")) else {
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

pub fn run(view: &Path, game: &Path, barrel: &Path, pictures: Option<&Path>) -> Result<()> {
    let view = View::parse(&std::fs::read_to_string(view)?)?;
    if let Some(p) = pictures {
        std::fs::create_dir_all(p)?;
    }
    let masks = view.masks.as_ref().and_then(|m| load_f32(&game.join(m)).ok());
    let chara = load_f32(&barrel.join("chara")).ok();
    let mut tone = None;
    for stage in STAGES {
        let Some(stem) = view.stages.get(stage.name) else { continue };
        let (g, b) = match (load(game, stem), load(barrel, if stage.barrel == "tone" { "tone" } else { stage.barrel })) {
            (Ok(g), Ok(b)) => (g, b),
            (g, b) => {
                println!("{}: missing ({})", stage.name, g.err().or(b.err()).map_or(String::new(), |e| e.to_string()));
                continue;
            }
        };
        let b = if stage.own_alpha { Image { px: b.px.chunks_exact(4).flat_map(|p| [p[3], p[3], p[3], 1.0]).collect(), ..b } } else { b };
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
        if let (Some(m), Some(c)) = (full, chara.as_ref().filter(|c| (c.w, c.h) == (g.w, g.h))) {
            if matches!(stage.name, "lit" | "scene" | "tone") {
                let own = own_characters(m, c);
                chara_stats(stage.name, &g, &b, m, &own, stage.scale);
                chara_bands(stage.name, &g, &b, m, &own, stage.scale);
            }
        }
        if stage.regions && ((g.w, g.h) == (1920, 1080) || (g.w, g.h) == (960, 540)) {
            let k = 1920 / g.w;
            for (name, r) in &view.regions {
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
        strip(&view, game, barrel, m);
    }
    if let (Some(m), Ok(d)) = (&masks, load_f32(&barrel.join("depth"))) {
        coverage(m, &d);
    }
    if let Some(m) = &masks {
        normals(&view, game, barrel, m, pictures);
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

    #[test]
    fn a_view_file_reads_back() {
        let text = "# a comment\nstage tone 2078-2078_rt1920x1080.90#7  # after a space\nmasks 1-2_rt.19#15.depth\nview 1 0 0 0 1 0 0 0 1\nregion mid_tree 1 2 3 4\n";
        let v = View::parse(text).unwrap();
        assert_eq!(v.stages["tone"], "2078-2078_rt1920x1080.90#7");
        assert_eq!(v.masks.as_deref(), Some("1-2_rt.19#15.depth"));
        assert_eq!(v.rows[1][1], 1.0);
        assert_eq!(v.regions, vec![("mid tree".to_string(), [1, 2, 3, 4])]);
        assert!(View::parse("stage only").is_err());
    }
}
