//! The synthesis's ring graph (`uil_a21_mix_field.xml`): the rings of a recipe's field on their hexagonal grid, the lines between them, the cursor's
//! glow and the plates that say what each ring gives. The game's code builds the graph from the layout's `clone_targets`; the rings are walked here
//! from the layout's own parts, in the order the capture draws them (the masks, the lines, the focus, the rings, the labels).

use super::*;

/// The graph's centre in layout pixels (`field_root` (660, 360) and `center` (-20, 0)) and the grid's steps for a unit of a ring's `x` and `y`
/// (the half of the width and the third of the height between two neighbours: `docs/ryza/open.md`, Synthesis part 3).
const CENTRE: [f32; 2] = [640.0, 360.0];
/// The rings stand this much further than the grid says (layout pixels: the capture's rings are 2.83 pixels of 1080 lines right and down of it).
const RING_SHIFT: [f32; 2] = [1.9, 1.9];
const GRID: [f32; 2] = [142.15, 82.07];
/// The rings are drawn at this zoom (`synth-atelier` 2900: a ring's picture is 189 pixels of 1080 lines where the layout's is 168 layout pixels).
const ZOOM: f32 = 0.75;
/// The plate under a closed ring's lock stands this far below the ring's centre in unzoomed layout pixels (the layout's `dummy` says 31; the capture's plates stand 8.7 higher: STAND-IN).
const PLATE_DY: f32 = 20.4;
/// A ring's lines: from its centre to the line's start, the line's length and its width (layout pixels at the ring's zoom, measured on the
/// capture's quads at 2900: 64.8 pixels, 126 and 45 of 1080 lines).
const EDGE_FROM: f32 = 57.6;
const EDGE_LENGTH: f32 = 112.0;
const EDGE_WIDTH: f32 = 40.0;
/// The light behind a ring turns, grows and fades in 480 frames (`anm_ring_bg_loop`); two sets of rings run 142.4 frames apart (STAND-IN: which
/// rings are in which set is the capture's: the slots 0, 1, 2 and 4 are in the first).
const GLOW_FRAMES: f32 = 480.0;
const GLOW_SET_LAG: f32 = 142.4;
/// The second light's start after the first's (`synth-atelier` 2910: 0.07 of its opacity at 253.6 frames of the first).
const GLOW_SECOND_LAG: f32 = 243.0;
const GLOW_FIRST_SET: [usize; 4] = [0, 1, 2, 4];
/// The glow's size in layout pixels before the ring's zoom (520 style pixels).
const GLOW_SIZE: f32 = 520.0 / 3.0;
/// The cursor's glow turns once in about 375 frames (the capture's 12.1 degrees at 2850 and 60.0 at 2900): it was at no turn at `FOCUS_ZERO_AT` on the capture's clock.
const FOCUS_DEGREES_PER_FRAME: f32 = 0.958;
const FOCUS_ZERO_AT: f32 = 2837.4;
/// Where a ring's label plate stands from the ring's centre (measured on the capture's plates: STAND-IN).
const LABEL_AT: [f32; 2] = [-98.4, -63.4];

/// The layout's node a ring is walked from.
const B: &str = "clone_tree_behind_node";

/// One ring on the graph.
#[derive(Clone, Debug)]
pub struct RingShow {
    pub at: [i32; 2],
    /// Fire, ice, bolt or air.
    pub elem: usize,
    /// What the ring gives (0 to 3 an effect, 4 quality, 5 and up traits): its label.
    pub kind: u32,
    /// Open: lit and without its lock.
    pub open: bool,
    /// The category of the row whose materials the ring takes (by place in the category table).
    pub restrict: Option<usize>,
    /// The ring it hangs from and the direction of the line (0 up, then clockwise by sixths).
    pub link: Option<(usize, u8)>,
    /// The element value the lock asks of the ring above it: the lock's plate says "X n".
    pub need: Option<(usize, i32)>,
}

/// What the graph shows.
pub struct FieldLook {
    pub rings: Vec<RingShow>,
    /// The ring under the cursor (its focus glow turns).
    pub cursor: usize,
    /// The graph's clock: the frame of the capture's run (the lights and the glow run on it).
    pub clock: f32,
    /// The graph's shift in pixels of the layout (the capture pans it as the cursor moves).
    pub shift: [f32; 2],
}

/// A ring's colours: the dark of its disc and the colour of its rim (the capture's bytes).
fn tint(elem: usize) -> ([f32; 3], [f32; 3]) {
    let (dark, rim) = match elem {
        0 => ([82.0, 0.0, 0.0], [191.0, 0.0, 0.0]),
        1 => ([0.0, 42.0, 82.0], [0.0, 144.0, 191.0]),
        2 => ([112.0, 108.0, 0.0], [236.0, 245.0, 0.0]),
        _ => ([71.0, 126.0, 46.0], [0.0, 245.0, 92.0]),
    };
    (dark.map(|v: f32| v / 255.0), rim.map(|v: f32| v / 255.0))
}

/// `outsine` of `p` in 0 to 1.
fn out_sine(p: f32) -> f32 {
    (p.clamp(0.0, 1.0) * std::f32::consts::FRAC_PI_2).sin()
}

impl Page {
    /// The ring graph's quads (the order the capture has them), added to `w`.
    pub(super) fn ring_graph(&self, w: &mut Walker, root: &Xf, look: &FieldLook) {
        let docs = &self.mix_docs;
        let Some(main) = find(&docs.field, "clone_tree_behind_node") else { return };
        let Some(front) = find(&docs.field, "clone_tree_front_node") else { return };
        let at = |r: &RingShow| [CENTRE[0] + RING_SHIFT[0] + look.shift[0] + r.at[0] as f32 * GRID[0], CENTRE[1] + RING_SHIFT[1] + look.shift[1] + r.at[1] as f32 * GRID[1]];
        let ring_xf = |r: &RingShow| root.node(at(r), 0.0, [ZOOM; 2]);
        let variant = |r: &RingShow| if r.kind == 4 { "frame_param" } else { "frame_eff" };
        let hide = || Over { hide: true, ..Default::default() };
        let show = || Over { show: true, ..Default::default() };
        // The masks: each ring's disc and rim.
        for r in &look.rings {
            let (dark, rim) = tint(r.elem);
            let v = variant(r);
            let over = |p: &[&str]| -> Over {
                match p {
                    [B, "main", "frames", n] if *n == v => show(),
                    [B, "main", "frames", n, "frame_bg"] if *n == v => Over { image_no: Some(1), paint: Some([dark[0], dark[1], dark[2], 222.0 / 255.0]), ..Default::default() },
                    [B, "main", "frames", n, "frame_ring"] if *n == v => Over { image_no: Some(2), paint: Some([rim[0], rim[1], rim[2], 1.0]), ..Default::default() },
                    [B] | [B, "main"] | [B, "main", "frames"] => Over::default(),
                    _ => hide(),
                }
            };
            w.walk(main, ring_xf(r), &mut Vec::new(), &over, &HashMap::new());
        }
        // The lines: one for each ring that hangs from another, along the direction it leaves by.
        for r in &look.rings {
            let Some((from, dir)) = r.link else { continue };
            let Some(parent) = look.rings.get(from) else { continue };
            let angle = (-90.0 + 60.0 * f32::from(dir)).to_radians();
            let origin = ring_xf(parent);
            let line = Element { name: "image".into(), attributes: vec![("style".into(), "gen_a21_mix01_line_p2".into()), ("size_wh".into(), format!("{EDGE_LENGTH}, {EDGE_WIDTH}, 0"))], children: Vec::new() };
            let xf = origin.node([0.0, 0.0], angle.to_degrees(), [1.0; 2]).node([EDGE_FROM, -EDGE_WIDTH / 2.0], 0.0, [1.0; 2]);
            for add in [false, true] {
                let before = w.out.len();
                w.image(&line, xf, &Over { alpha: Some(0.949), natural: false, ..Default::default() });
                if add {
                    w.additive.extend(before..w.out.len());
                }
            }
        }
        // The cursor's glow.
        if let Some(r) = look.rings.get(look.cursor) {
            let angle = FOCUS_DEGREES_PER_FRAME * (look.clock - FOCUS_ZERO_AT);
            let side = 262.0 * ZOOM;
            let glow = Element { name: "image".into(), attributes: vec![("style".into(), "gen_a21_mmap02_point_eff_0".into()), ("size_wh".into(), format!("{side}, {side}, 0"))], children: Vec::new() };
            w.image(&glow, root.node(at(r), angle, [1.0; 2]).node([-side / 2.0, -side / 2.0], 0.0, [1.0; 2]), &Over::default());
        }
        // The rings.
        w.bold_scale = ZOOM;
        for (k, r) in look.rings.iter().enumerate() {
            let (dark, rim) = tint(r.elem);
            let dim = if r.open { 1.0 } else { 0.5 };
            let v = variant(r);
            // The light behind it, and a second light (the first's picture turned over) that starts 243 frames after the first on its second cycle.
            let run = look.clock - 2656.4 + if GLOW_FIRST_SET.contains(&k) { 0.0 } else { -GLOW_SET_LAG };
            let center = ring_xf(r).apply([1.0, 0.0]);
            let lights = [(run.rem_euclid(GLOW_FRAMES), false, 1.0), ((run - GLOW_SECOND_LAG).rem_euclid(GLOW_FRAMES), true, 0.9)];
            for (n, (t, turned, shrink)) in lights.into_iter().enumerate() {
                if n == 1 && (run <= GLOW_SECOND_LAG || out_sine(t / (GLOW_FRAMES / 2.0)) * 255.0 < 1.5) {
                    continue;
                }
                let half = GLOW_FRAMES / 2.0;
                let alpha = if t < half { out_sine(t / half) } else { out_sine((GLOW_FRAMES - t) / half) };
                let p = out_sine(t / GLOW_FRAMES);
                let scale = [shrink * (0.95 + 0.15 * p), shrink * (0.95 + 0.2 * p)];
                let size = [GLOW_SIZE * ZOOM * scale[0] * w.screen_k, GLOW_SIZE * ZOOM * scale[1] * w.screen_k];
                let angle = (30.0 * t / GLOW_FRAMES).to_radians();
                let glow = Xf { a: angle.cos(), b: angle.sin(), c: -angle.sin(), d: angle.cos(), x: center[0], y: center[1] };
                let corners = [[-size[0] / 2.0, -size[1] / 2.0], [size[0] / 2.0, -size[1] / 2.0], [size[0] / 2.0, size[1] / 2.0], [-size[0] / 2.0, size[1] / 2.0]].map(|c| glow.apply(c));
                let (x0, x1) = corners.iter().fold((f32::MAX, f32::MIN), |(a, b), p| (a.min(p[0]), b.max(p[0])));
                let (y0, y1) = corners.iter().fold((f32::MAX, f32::MIN), |(a, b), p| (a.min(p[1]), b.max(p[1])));
                if let Some((texture, pic, (aw, ah))) = self.picture("gen_a21_mix01_ring_eff", r.elem).map(|(t, p, a)| (t, p, (a[0], a[1]))) {
                    let (v0, v1) = (pic.uvwh[1] / ah, (pic.uvwh[1] + pic.uvwh[3]) / ah);
                    let uv = [pic.uvwh[0] / aw, if turned { v1 } else { v0 }, (pic.uvwh[0] + pic.uvwh[2]) / aw, if turned { v0 } else { v1 }];
                    let before = w.out.len();
                    w.out.push(Quad { turn: false, spin: Some(Spin::Corners(corners)), rect: [x0, y0, x1, y1], uv, color: [dim, dim, dim, alpha], texture });
                    w.additive.push(before);
                }
            }
            let over = |p: &[&str]| -> Over {
                match p {
                    [B, "main", "frames", n] if *n == v => show(),
                    [B, "main", "frames", n, "frame_bg"] if *n == v => Over { image_no: Some(1), paint: Some([dark[0] * dim, dark[1] * dim, dark[2] * dim, 222.0 / 255.0]), ..Default::default() },
                    [B, "main", "frames", n, "frame_ring"] if *n == v => Over { image_no: Some(2), paint: Some([rim[0] * dim, rim[1] * dim, rim[2] * dim, 1.0]), ..Default::default() },
                    [B, "main", "frames", n, "frame_circle"] if *n == v => Over { paint: Some([dim, dim, dim, 1.0]), ..Default::default() },
                    [B, "main", "frames", n, "frame_bg_eff1"] | [B, "main", "frames", n, "frame_bg_eff2"] if *n == v => hide(),
                    [B] | [B, "main"] | [B, "main", "frames"] => Over::default(),
                    [B, "main", "frames", ..] => hide(),
                    [B, "main", "dummy"] => Over::default(),
                    [B, "main", "dummy", "mat_restrict"] if r.restrict.is_some() => Over { show: true, at: Some([0.0, if r.need.is_some() { 0.0 } else { 10.0 }]), ..Default::default() },
                    [B, "main", "dummy", "mat_restrict", "item_image"] => hide(),
                    [B, "main", "dummy", "mat_restrict", "cat_image"] => Over { image_no: r.restrict, paint: Some([254.0 / 255.0 * dim, 251.0 / 255.0 * dim, 231.0 / 255.0 * dim, 1.0]), ..Default::default() },
                    [B, "main", "dummy", ..] | [B, "main", "mat_history_0", ..] | [B, "main", "mat_history_1", ..] | [B, "main", "mat_history_2", ..] => hide(),
                    [B, "element"] if r.kind < 5 => show(),
                    [B, "element", "list", n] if matches!(*n, "node_0" | "node_1") => Over::default(),
                    [B, "element", "list", _] => hide(),
                    [B, "element", "list", _, "elem"] => hide(),
                    [B, "lock"] if !r.open => show(),
                    [B, "lock", "__unzoom"] => Over { scale: Some([1.0 / ZOOM; 2]), ..Default::default() },
                    [B, "lock", "__unzoom", "dummy"] => Over { at: Some([0.0, PLATE_DY]), ..Default::default() },
                    [B, "lock", "__unzoom", "dummy", "cond", "elem_icon"] if r.need.is_some() => Over { image_no: r.need.map(|n| n.0), ..Default::default() },
                    [B, "lock", "__unzoom", "dummy", "cond", "threshold"] if r.need.is_some() => Over { text: r.need.map(|n| n.1.to_string()), ..Default::default() },
                    [B, "lock", "__unzoom", "dummy", "cond"] if r.need.is_some() => Over::default(),
                    [B, "lock", "__unzoom", "dummy", "cond", ..] if r.need.is_none() => hide(),
                    [B, "lock", "__unzoom", "dummy", ..] if r.need.is_some() => Over::default(),
                    [B, "lock", "__unzoom", "dummy", ..] => hide(),
                    [B, "lock", ..] => Over::default(),
                    [B, "lv_up_bg", ..] | [B, "lv_up_txt", ..] => hide(),
                    _ => Over::default(),
                }
            };
            w.walk(main, ring_xf(r), &mut Vec::new(), &over, &HashMap::new());
        }
        w.bold_scale = 1.0;
        // The labels.
        for r in &look.rings {
            let label = self.strings.get(&(LABEL_BASE + r.kind.min(5))).cloned();
            let over = |p: &[&str]| -> Over {
                match p {
                    [.., "ring_type"] => Over { text: label.clone(), ..Default::default() },
                    _ => Over::default(),
                }
            };
            w.walk(front, root.node([at(r)[0] + LABEL_AT[0] + 77.0 + 25.0, at(r)[1] + LABEL_AT[1] + 23.0 + 50.0], 0.0, [1.0; 2]), &mut Vec::new(), &over, &HashMap::new());
        }
    }
}

/// `STR_ring label n` is the string `LABEL_BASE + n`: "Effect 1" to "Effect 3", then "Quality" and "Traits" from kind 4 (`str_menu_define`).
const LABEL_BASE: u32 = 10_092_545;

impl Page {
    /// The bars of the field's screens (a gradation top and bottom over a white: the capture draws the set twice, under the ring graph and over it).
    pub(super) fn bars(&self, w: &mut Walker, root: &Xf) {
        let gradation = self.gradation_texture();
        // (the top one is drawn turned: its picture's u runs backwards)
        w.piece(root.node([0.0, 0.0], 0.0, [1.0; 2]), [0.0; 2], [1280.0, 64.0], [1.0, 0.0, 0.0, 1.0], [1.0; 4], gradation);
        w.piece(root.node([0.0, 64.0], 0.0, [1.0; 2]), [0.0; 2], [1280.0, 592.0], [0.0, 0.0, 0.125, 0.125], [1.0; 4], WHITE);
        w.piece(root.node([0.0, 656.0], 0.0, [1.0; 2]), [0.0; 2], [1280.0, 64.0], [0.0, 0.0, 1.0, 1.0], [1.0; 4], gradation);
    }

    /// The Add Materials screen's quads on a `screen` of pixels: the ring graph, the bars over it, then the description of the ring under the cursor,
    /// the panel on the right, the quantity box and the title plate.
    pub fn add_quads(&self, look: &AddLook, screen: (u32, u32)) -> (Vec<Quad>, Vec<usize>, Vec<Clip>) {
        let k = screen.1 as f32 / LAYOUT_HEIGHT;
        let mut w = Walker { page: self, screen_k: k, out: Vec::new(), additive: Vec::new(), parent: [0.0; 2], card: true, ordinal: 0, mask: None, clips: Vec::new(), mix: true, bold_scale: 1.0, run_base: 0 };
        let root = Xf { a: k, b: 0.0, c: 0.0, d: k, x: (screen.0 as f32 - 1280.0 * k) / 2.0, y: 0.0 };
        self.ring_graph(&mut w, &root, &look.field);
        w.cull_outside(screen);
        self.bars(&mut w, &root);
        // The description of the ring under the cursor.
        if let (Some(ring), Some(desc)) = (look.field.rings.get(look.field.cursor), find(&self.mix_docs.recipe, "ring_desc")) {
            let text = self.strings.get(&(DESC_BASE + ring.kind.min(5))).cloned();
            let over = |p: &[&str]| -> Over {
                match p {
                    ["ring_desc"] => Over { show: true, ..Default::default() },
                    ["ring_desc", "body", "desc"] => Over { text: text.clone(), ..Default::default() },
                    _ => Over::default(),
                }
            };
            w.slid(Slide::in_from(look.desc_age, [0.0, 40.0]), |w| w.walk(desc, root.node(pair(desc, "pos", [0.0; 2]), 0.0, [1.0; 2]), &mut Vec::new(), &over, &HashMap::new()));
        }
        // Select Recipe's list, going out to the left and fading as Add Materials comes in.
        if let Some((list, out)) = &look.list_out {
            w.slid(Slide::in_from(SLIDE_FRAMES - out, [-40.0, 0.0]), |w| self.recipe_list(w, &root, list));
        }
        // (the layout's `mix_param` comes between them, and the ring's own `ring_info` over the panel's frame and the quantity box)
        self.product_panel(&mut w, &root, &look.row, Some(&look.panel));
        if let (Some(popup), Some(ring)) = (&look.popup, look.field.rings.get(look.field.cursor)) {
            self.ring_popup(&mut w, &root, [CENTRE[0] + look.field.shift[0] + ring.at[0] as f32 * GRID[0], CENTRE[1] + look.field.shift[1] + ring.at[1] as f32 * GRID[1] + POPUP_BELOW], ring, popup);
        }
        self.quantity_box(&mut w, &root, &look.row.info);
        self.caption_plate(&mut w, &root, ADD_TITLE, Slide::NONE);
        (w.out, w.additive, w.clips)
    }
}

/// What the Add Materials screen shows.
pub struct AddLook {
    pub field: FieldLook,
    /// The recipe's row and the panel's numbers.
    pub row: RecipeRow,
    pub panel: AddPanel,
    /// The tiers of the ring under the cursor (`ring_info`), up while a ring is looked at.
    pub popup: Option<Popup>,
    /// The recipe list still going out, and the frames since it began to.
    pub list_out: Option<(RecipeLook, f32)>,
    /// The frames since the ring's description began to rise into place (`anm_common_slide_in_bottom`; it goes out as that played backwards from A on 2920).
    pub desc_age: f32,
}

/// What a ring's popup says: its heading and a row for each tier.
pub struct Popup {
    pub title: String,
    /// The element value each tier needs (the number of lights on its row) and what it gives.
    pub tiers: Vec<(i32, String)>,
}

/// The popup stands this far below the ring's centre (layout pixels: `ring_info` at (640, 454) under the ring at (640.75, 360) in `synth-atelier` 2900).
const POPUP_BELOW: f32 = 94.0;

/// The ring descriptions' strings: "Effect 1 will appear or grow based on Material Loop level." and the rest by the ring's kind.
const DESC_BASE: u32 = 10_092_605;

/// The title plate's string ("Add Materials", `str_menu_define`).
const ADD_TITLE: u32 = 9_961_591;

impl Walker<'_> {
    /// Drops the quads that lie wholly outside the screen (the game does not draw them: the capture has none).
    pub(super) fn cull_outside(&mut self, screen: (u32, u32)) {
        let (w, h) = (screen.0 as f32, screen.1 as f32);
        let keep: Vec<bool> = self.out.iter().map(|q| q.rect[2] > 0.0 && q.rect[0] < w && q.rect[3] > 0.0 && q.rect[1] < h).collect();
        let mut at = 0;
        let moved: Vec<Option<usize>> = keep.iter().map(|&k| k.then(|| { at += 1; at - 1 })).collect();
        let mut i = 0;
        self.out.retain(|_| { i += 1; keep[i - 1] });
        self.additive = self.additive.iter().filter_map(|&a| moved.get(a).copied().flatten()).collect();
    }
}

impl Page {
    /// The popup of a ring (`ring_info`'s `normal`): the heading, and a row for each tier with a light for each point of the element it needs.
    /// (The layout holds the first row; the game's code copies it for the others.)
    fn ring_popup(&self, w: &mut Walker, root: &Xf, at: [f32; 2], ring: &RingShow, popup: &Popup) {
        let Some(info) = find(&self.mix_docs.recipe, "ring_info") else { return };
        let hide = || Over { hide: true, ..Default::default() };
        let n = |s: &str, p: &str| s.strip_prefix(p).and_then(|k| k.parse::<usize>().ok()).unwrap_or(0);
        // A row's parts by its path below `node_0`.
        let row_over = |k: usize, rest: &[&str]| -> Over {
            let Some((lights, desc)) = popup.tiers.get(k) else { return hide() };
            match rest {
                ["exp", icon, tail @ ..] => {
                    if n(icon, "icon_") >= *lights as usize {
                        return hide();
                    }
                    match tail {
                        ["dummy", "off"] => Over { image_no: Some(ring.elem), ..Default::default() },
                        ["dummy", "dummy_1", ..] => hide(),
                        _ => Over::default(),
                    }
                }
                ["dummy", _, "desc"] => Over { text: Some(desc.clone()), ..Default::default() },
                _ => Over::default(),
            }
        };
        let over = |p: &[&str]| -> Over {
            match p {
                ["ring_info"] | ["ring_info", "normal"] => Over { show: true, ..Default::default() },
                ["ring_info", "recipe", ..] => hide(),
                // (the frame is 201 layout pixels high for six rows, 22.5 less for each row fewer)
                ["ring_info", "normal", "frame"] => Over { size_h: Some(201.0 - 22.5 * (6 - popup.tiers.len().min(6)) as f32), ..Default::default() },
                ["ring_info", "normal", "type"] => Over { text: Some(popup.title.clone()), ..Default::default() },
                ["ring_info", "normal", "list", "node_0", rest @ ..] => row_over(0, rest),
                ["ring_info", "normal", "list", node, ..] if n(node, "node_") > 0 => hide(),
                _ => Over::default(),
            }
        };
        w.walk(info, root.node(at, 0.0, [1.0; 2]), &mut Vec::new(), &over, &HashMap::new());
        if let Some(row) = find(info, "node_0") {
            for k in 1..popup.tiers.len().min(6) {
                let over = |p: &[&str]| -> Over {
                    match p {
                        ["node_0", rest @ ..] => row_over(k, rest),
                        _ => Over::default(),
                    }
                };
                w.walk(row, root.node([at[0] - 162.0, at[1] + 38.0 + ROW_STEP_POPUP * k as f32], 0.0, [1.0; 2]), &mut Vec::new(), &over, &HashMap::new());
            }
        }
    }
}

/// The popup's rows stand this far apart (layout pixels: `ring_info`'s `node_1` at 21.78).
const ROW_STEP_POPUP: f32 = 21.78;
