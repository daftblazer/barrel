//! The Requests screen (Start on the area map and in a field): the requests the player has taken,
//! as a list with the chosen one's details beside it, on the game's own layouts (`a15_common.g1t`,
//! layout 678 over the desk 383, the header 371, the footer 372 and the tab 531).
//!
//! Layout 678 holds three pieces: the rank panel (527: the rank's icon, part 0x31, and the
//! rank points still needed for the next rank, a number in part 0x4e), the detail page (676 with
//! its frame 677) and the list (674; one row is layout 673, the node set of a row the game
//! repeats down the list every 58 pixels, with the chosen row's plate, part 0xab, and, for a
//! request done, the "Report!" glow, 704).
//!
//! Layout nodes of a stretched kind carry their size in the node: kind 15 (a piece cut in nine,
//! tiled to fill) has its width in word 5 and its height in word 6, kind 19 (a bar of three
//! cuts) its width in word 5, kind 4 (a picture) its scale in percent in words 5 and 6; a
//! number (kind 28) has its largest value in word 5 and its colour in word 7 (the layouts' ink).
//!
//! The data: the session's taken requests ([`barrel_escha::session::Taken`], from the save's
//! `quest/player`: kind, target, wanted, done, points and two prizes already rolled) with the
//! monsters' and items' names, and the rank and its points (`RankData.xml`). A taken request's kind
//! reads 0 Synthesis, 1 Gathering, 2 Subjugation (the order of the screen's strings; only 2 is seen
//! in a recording: its prizes are `QuestBtl`'s).
//!
//! Read from the capture of the game (`area-requests`), not from the layouts: the cursor's place
//! on the chosen row ([`CURSOR_FROM_BAR`]), the second name line's step ([`NAME_STEP`]) and the
//! words of the unseen kinds. Not yet the game's: the item kinds' pages (a synthesis or gathering
//! request is shown with its item's picture, name, count and prizes, but not its effects,
//! "Quality" or "Held" lines; their row icons are guessed); the cursor (a glow cycling its eight cuts
//! in a second) and the chosen row's plate (its colour pulses) move in the game: here the layout's
//! first cut and white, a phase difference the parity diff shows as the one cursor mismatch;
//! no scrolling (the list holds ten at most).

use barrel_escha::data::layout::{self, Placed, Text};
use barrel_escha::session::Taken;
use barrel_formats::pe::Pe;

use crate::hud::{Element, TEXT_RISE};
use crate::title::Button;

pub const LAYOUT: u32 = 678;
const BACKGROUND: u32 = 383;
const HEADER: u32 = 371;
const FOOTER: u32 = 372;
const TAB: u32 = 531;
const RANK_PANEL: u32 = 527;
const DETAIL: u32 = 676;
const LIST: u32 = 674;
const ROW: u32 = 673;
/// The row's glow, nested in it.
const GLOW: u32 = 704;
/// The detail page's panel of an item's effects.
const EFFECTS: u32 = 675;
/// The tab's two sprites (parts 0x2e the title, 0x2c the icon) and their cut for this screen
/// (the menu's Request command, the eighth of `menu::COMMANDS`).
const TAB_PARTS: [i32; 2] = [0x2e, 0x2c];
const TAB_CUT: i32 = 7;
const DIGITS: i32 = 0x4e;
const ROW_DIGITS: i32 = 0x4f;
const RANK_ICON: i32 = 0x31;
const ROW_PLATE: i32 = 0xab;
const ROW_ICON: i32 = 0x99;
const CURSOR: i32 = 0x9f;
const TEXT: i32 = 0x164;
const NUMBER_KIND: i32 = 28;
const STRETCHED: i32 = 15;
const BAR: i32 = 19;
const PICTURE: i32 = 4;
/// A digit sprite's width, and the rows' pitch down the list.
const DIGIT_ADVANCE: i32 = 20;
const ROW_PITCH: i32 = 58;
/// The list's rows the layout has.
pub const ROWS: usize = 10;
/// The cursor (layout 691's part 0x9f, a glow of 96 pixels placed at its node less 90) from the
/// chosen row's bar, as the game's capture has it.
const CURSOR_FROM_BAR: [i32; 2] = [-90, -56];
/// The row's count mark, drawn as a text of the row (a counter's character of the game's font).
const COUNT_MARK: &str = "#";
/// The detail page's paper (part 0x98 cut 0), its item picture (0xd6) and monster picture (0x144).
const PAPER: i32 = 0x98;
const ITEM_PICTURE: i32 = 0xd6;
const MONSTER_PICTURE: i32 = 0x144;
/// A second line of a name (the monster's under "Subjugation") lies this far below the first.
const NAME_STEP: i32 = 36;
const PICTURES: &str = "a15_itempic.g1t";

/// How the screen words a request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Synthesis,
    Gathering,
    Subjugation,
}

impl Kind {
    fn of(taken: i32) -> Kind {
        match taken {
            0 => Kind::Synthesis,
            1 => Kind::Gathering,
            _ => Kind::Subjugation,
        }
    }

    /// The row title's ending (`%s Subjugation`) and the page's first line (`Subjugation<CR>%s`).
    fn word(self) -> &'static str {
        match self {
            Kind::Synthesis => "Synthesis",
            Kind::Gathering => "Gathering",
            Kind::Subjugation => "Subjugation",
        }
    }

    /// The icon on a row (part 0x99): the first cut for monsters, the next two guessed.
    fn icon(self) -> i32 {
        match self {
            Kind::Subjugation => 0,
            Kind::Synthesis => 1,
            Kind::Gathering => 2,
        }
    }
}

/// What a request is about, for its picture.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Subject {
    /// A monster by number (`a15_dict_mons<NNN>.g1t`).
    Monster(i32),
    /// An item by its picture number (`a15_itempic.g1t`).
    Item(u32),
}

/// One taken request, ready to show.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub kind: Kind,
    pub name: String,
    pub subject: Subject,
    pub wanted: i32,
    pub done: i32,
    pub points: i32,
    /// The prizes' names and counts.
    pub prizes: Vec<(String, i32)>,
}

/// The tables the entries' words come from.
pub struct Names<'a> {
    pub monsters: &'a [String],
    /// `itemData.xml`'s rows.
    pub items: &'a [barrel_formats::table::Element],
}

impl Names<'_> {
    fn item(&self, row: i32) -> Option<(String, u32)> {
        let row = self.items.get(usize::try_from(row).ok()?)?;
        Some((row.attr("ItemName").unwrap_or("?").to_string(), row.attr("ImageNo").and_then(|v| v.parse().ok()).unwrap_or(0)))
    }
}

/// The taken requests as entries (the list's rows, in slot order).
pub fn entries(taken: &[Option<Taken>], names: &Names) -> Vec<Entry> {
    taken
        .iter()
        .flatten()
        .filter_map(|t| {
            let kind = Kind::of(t.kind);
            let (name, subject) = match kind {
                Kind::Subjugation => (names.monsters.get(usize::try_from(t.target).ok()?)?.clone(), Subject::Monster(t.target)),
                _ => {
                    let (name, picture) = names.item(t.target)?;
                    (name, Subject::Item(picture))
                }
            };
            let prizes = t.prizes.iter().filter(|p| p.0 >= 0).filter_map(|&(item, count)| Some((names.item(item)?.0, count))).collect();
            Some(Entry { kind, name, subject, wanted: t.wanted, done: t.progress, points: t.points, prizes })
        })
        .collect()
}

/// The rank panel: the rank's icon (the `RankData` row) and the points still needed for the
/// next rank (0 at the top).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Panel {
    pub icon: i32,
    pub to_next: i32,
}

impl Panel {
    /// From the save's rank (a rank number, from 1) and points, and `RankData`'s rows.
    pub fn of(ranks: &[barrel_escha::rank::Rank], rank: i32, points: i32) -> Panel {
        let to_next = usize::try_from(rank).ok().and_then(|next| ranks.get(next)).map_or(0, |r| (r.points - points).max(0));
        Panel { icon: (rank - 1).max(0), to_next }
    }
}

/// The screen's state: the chosen row.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Requests {
    pub chosen: usize,
}

impl Requests {
    /// Moves the cursor over `count` rows; Back returns true (the screen closes).
    pub fn press(&mut self, b: Button, count: usize) -> bool {
        match b {
            Button::Up => self.chosen = self.chosen.saturating_sub(1),
            Button::Down => self.chosen = (self.chosen + 1).min(count.saturating_sub(1)),
            Button::Back => return true,
            _ => {}
        }
        false
    }
}

/// The layouts' nodes the screen is made of.
pub struct Layouts {
    desk: Vec<Placed>,
    header: Vec<Placed>,
    footer: Vec<Placed>,
    tab: Vec<Placed>,
    screen: Vec<Placed>,
}

impl Layouts {
    pub fn load(pe: &Pe) -> Layouts {
        let get = |id| layout::placed(pe, id);
        Layouts { desk: get(BACKGROUND), header: get(HEADER), footer: get(FOOTER), tab: get(TAB), screen: get(LAYOUT) }
    }
}

/// The lines under and over the lists.
pub struct Words {
    /// What the header says (`SysInfo.xml`'s "Please select an entry to view.").
    pub header: String,
    /// The footer's button help (`Menu_Line_Help.xml`'s "Select, Return" line).
    pub footer: String,
}

fn raised(t: Text, text: impl Into<String>) -> Element {
    Element::Label(Text { at: [t.at[0], t.at[1] - TEXT_RISE], text: text.into(), ..t })
}

/// The ink of a number node (its word 7, stored r, g, b, a).
fn ink(pe: &Pe, p: &Placed) -> [u8; 3] {
    let [r, g, b, _] = layout::word(pe, p, 7).unwrap_or(0).to_le_bytes();
    [r, g, b]
}

/// A number (`value` held to the node's largest) right-aligned on its node: the last digit just
/// left of it, in the node's ink.
fn number(pe: &Pe, p: &Placed, value: i32) -> Vec<Element> {
    let most = layout::word(pe, p, 5).filter(|&m| m > 0).unwrap_or(i32::MAX);
    let digits = value.clamp(0, most).to_string();
    let (part, rgb) = (p.node.part, ink(pe, p));
    let last = digits.len() as i32 - 1;
    digits
        .bytes()
        .enumerate()
        .map(|(i, d)| Element::Tinted { part, cut: i32::from(d - b'0'), at: [p.at[0] - DIGIT_ADVANCE * (last - i as i32 + 1), p.at[1]], rgb })
        .collect()
}

/// A piece cut in nine (`part`'s cuts 0 to 8 in rows) tiled over `size` from `at`, in the game's
/// order: the top row, the left column, the right column, the bottom row, then the middle row by
/// row. The edge and middle tiles repeat at their size, the last one cut short.
fn tiled(pe: &Pe, part: i32, at: [i32; 2], size: [i32; 2]) -> Vec<Element> {
    let cut = |c| layout::sprite(pe, part, c, [0, 0]);
    let Some(cuts) = (0..9).map(cut).collect::<Option<Vec<_>>>() else { return Vec::new() };
    let wide = |c: usize| i32::from(cuts[c].src[2] - cuts[c].src[0]);
    let high = |c: usize| i32::from(cuts[c].src[3] - cuts[c].src[1]);
    // The pieces along one axis: (cut column or row, start, length) from `edge` to the far edge.
    let along = |first: i32, mid: i32, last: i32, total: i32| {
        let mut out = vec![(0, 0, first)];
        let mut x = first;
        while x < total - last {
            out.push((1, x, mid.min(total - last - x)));
            x += mid;
        }
        out.push((2, total - last, last));
        out
    };
    let xs = along(wide(0), wide(1), wide(2), size[0]);
    let ys = along(high(0), high(3), high(6), size[1]);
    let slice = |c: usize, x: (usize, i32, i32), y: (usize, i32, i32)| {
        let p = &cuts[c];
        let [l, t, ..] = p.src.map(f32::from);
        Element::Slice { pack: p.texture.clone(), index: p.index, src: [l, t, l + x.2 as f32, t + y.2 as f32], rect: [at[0] + x.1, at[1] + y.1, at[0] + x.1 + x.2, at[1] + y.1 + y.2] }
    };
    let (top, bottom) = (ys[0], *ys.last().unwrap());
    let (left, right) = (xs[0], *xs.last().unwrap());
    let mut out: Vec<Element> = xs.iter().map(|&x| slice(x.0, x, top)).collect();
    let middle = &ys[1..ys.len() - 1];
    out.extend(middle.iter().map(|&y| slice(3, left, y)));
    out.extend(middle.iter().map(|&y| slice(5, right, y)));
    out.extend(xs.iter().map(|&x| slice(6 + x.0, x, bottom)));
    for &y in middle {
        out.extend(xs[1..xs.len() - 1].iter().map(|&x| slice(4, x, y)));
    }
    out
}

/// A bar of three cuts (`part`'s cuts 0 to 2: left end, middle, right end) over `width` from `at`:
/// the middle repeated, its last repeat cut short.
fn bar(pe: &Pe, part: i32, at: [i32; 2], width: i32) -> Vec<Element> {
    let Some(cuts) = (0..3).map(|c| layout::sprite(pe, part, c, [0, 0])).collect::<Option<Vec<_>>>() else { return Vec::new() };
    let wide = |c: usize| i32::from(cuts[c].src[2] - cuts[c].src[0]);
    let slice = |c: usize, x: i32, w: i32| {
        let p = &cuts[c];
        let [l, t, _, b] = p.src.map(f32::from);
        Element::Slice { pack: p.texture.clone(), index: p.index, src: [l, t, l + w as f32, b], rect: [at[0] + x, at[1], at[0] + x + w, at[1] + (b - t) as i32] }
    };
    let mut out = vec![slice(0, 0, wide(0))];
    let mut x = wide(0);
    while x < width - wide(2) {
        let w = wide(1).min(width - wide(2) - x);
        out.push(slice(1, x, w));
        x += wide(1);
    }
    out.push(slice(2, width - wide(2), wide(2)));
    out
}

/// A layout node's elements as stored (stretched kinds tiled to their size).
fn stored(pe: &Pe, p: &Placed) -> Vec<Element> {
    let word = |i| layout::word(pe, p, i).unwrap_or(0);
    match p.node.kind {
        STRETCHED => tiled(pe, p.node.part, p.at, [word(5), word(6)]),
        BAR => bar(pe, p.node.part, p.at, word(5)),
        _ => vec![Element::stored(p)],
    }
}

/// A layout's sprites, and its texts through `text`.
fn plain(pe: &Pe, nodes: &[Placed], text: impl Fn(&Placed) -> Option<Element>) -> Vec<Element> {
    nodes.iter().flat_map(|p| if p.node.part == TEXT { text(p).into_iter().collect() } else { stored(pe, p) }).collect()
}

/// All the elements of the screen with row `view.chosen` chosen.
pub fn elements(pe: &Pe, l: &Layouts, words: &Words, panel: Panel, list: &[Entry], view: Requests) -> Vec<Element> {
    let label = |p: &Placed, text: &str| layout::text(pe, p).map(|t| raised(t, text));
    let mut out: Vec<Element> = l.desk.iter().map(|p| Element::Sprite { part: p.node.part, cut: DESK_CUT, at: p.at }).collect();
    out.extend(plain(pe, &l.footer, |p| label(p, &words.footer)));
    out.extend(plain(pe, &l.header, |p| label(p, &words.header)));
    for p in &l.tab {
        out.push(match Element::stored(p) {
            Element::Sprite { part, at, .. } if TAB_PARTS.contains(&part) => Element::Sprite { part, cut: TAB_CUT, at },
            e => e,
        });
    }
    let nodes_of = |id: u32| l.screen.iter().filter(move |p| p.path.get(1) == Some(&id));
    out.extend(rank_panel(pe, nodes_of(RANK_PANEL), panel));
    match list.get(view.chosen) {
        Some(entry) => out.extend(detail(pe, nodes_of(DETAIL), entry)),
        None => out.extend(nodes_of(DETAIL).filter(|p| p.node.kind == STRETCHED || p.node.part == PAPER).flat_map(|p| stored(pe, p))),
    }
    // The list: its frame, then a bar for each request, then each row from the first row's
    // nodes (layout 673, its glow 704 nested), the cursor after the chosen one.
    let bar_at = nodes_of(LIST).find(|p| p.node.kind == BAR).map_or([0, 0], |p| p.at);
    let mut bars = 0;
    for p in nodes_of(LIST).filter(|p| p.path.get(2) != Some(&ROW)) {
        if p.node.kind == BAR {
            bars += 1;
            if bars > list.len() {
                continue;
            }
        }
        out.extend(stored(pe, p));
    }
    let first: Vec<&Placed> = nodes_of(LIST).filter(|p| p.path.get(2) == Some(&ROW)).collect();
    for (k, e) in list.iter().enumerate().take(ROWS) {
        let chosen = k == view.chosen;
        let down = ROW_PITCH * k as i32;
        for p in &first {
            let at = [p.at[0], p.at[1] + down];
            let node = Placed { at, ..(*p).clone() };
            let glow = p.path.contains(&GLOW);
            match p.node.part {
                // The plate marks the chosen row, the glow ("Report!") a request done.
                _ if p.node.part == ROW_PLATE && !chosen => {}
                _ if glow && e.done < e.wanted => {}
                ROW_ICON => out.push(Element::Sprite { part: ROW_ICON, cut: e.kind.icon(), at }),
                TEXT => {
                    let Some(t) = layout::text(pe, p) else { continue };
                    // The row's title, then the count's mark.
                    let text = if t.text.is_empty() { COUNT_MARK.to_string() } else { format!("{} {}", e.name, e.kind.word()) };
                    out.push(raised(Text { at, ..t }, text));
                }
                ROW_DIGITS => out.extend(number(pe, &node, e.wanted)),
                _ => out.push(Element::stored(&node)),
            }
        }
        if chosen {
            out.push(Element::Sprite { part: CURSOR, cut: 0, at: [bar_at[0] + CURSOR_FROM_BAR[0], bar_at[1] + down + CURSOR_FROM_BAR[1]] });
        }
    }
    out
}

/// The rank panel's nodes (layout 527): the panel, the rank's own icon (on the first icon's
/// place; the others are the ranks' places the game leaves empty), the points to the next rank
/// (the first number; the second, with its slash, is not drawn) and its "pt".
fn rank_panel<'a>(pe: &Pe, nodes: impl Iterator<Item = &'a Placed>, panel: Panel) -> Vec<Element> {
    let (mut icon, mut numbers) = (true, 0);
    let mut out = Vec::new();
    for p in nodes {
        match p.node.part {
            RANK_ICON => {
                if std::mem::take(&mut icon) {
                    out.push(Element::Sprite { part: RANK_ICON, cut: panel.icon, at: p.at });
                }
            }
            DIGITS if p.node.kind == NUMBER_KIND => {
                if numbers == 0 {
                    out.extend(number(pe, p, panel.to_next));
                }
                numbers += 1;
            }
            DIGITS => {}
            TEXT => out.extend(layout::text(pe, p).map(|t| raised(t.clone(), t.text))),
            _ => out.push(Element::stored(p)),
        }
    }
    out
}

/// The detail page for `entry` (layout 676's nodes in order). Its texts come in this order: the
/// kind and name, the count's mark, "Effects:" and its list, "Reward:", "Results", "pts", the
/// two prizes' names (each followed by an empty text), "Quality:" (read "Slain" for a monster);
/// its numbers: the count, the points, the two prizes' counts, then the done and wanted counts.
fn detail<'a>(pe: &Pe, nodes: impl Iterator<Item = &'a Placed>, e: &Entry) -> Vec<Element> {
    let (mut text, mut digits) = (0, 0);
    let monster = matches!(e.subject, Subject::Monster(_));
    let mut out = Vec::new();
    for p in nodes {
        let n = p.node;
        match (n.part, n.kind) {
            // The effects' panel (layout 675 in 676) only an item's page has.
            (PAPER, _) if monster && p.path.contains(&EFFECTS) => {}
            (PAPER, _) | (_, STRETCHED) => out.extend(stored(pe, p)),
            (TEXT, _) => {
                let Some(t) = layout::text(pe, p) else { continue };
                text += 1;
                match text {
                    1 => {
                        out.push(raised(t.clone(), e.kind.word()));
                        out.push(Element::Label(Text { at: [t.at[0], t.at[1] - TEXT_RISE + NAME_STEP], text: e.name.clone(), ..t }));
                    }
                    2 if !monster => out.push(raised(t, COUNT_MARK)),
                    5..=7 => out.push(raised(t.clone(), t.text)),
                    8 | 10 => {
                        if let Some((name, _)) = e.prizes.get(usize::from(text == 10)) {
                            out.push(raised(t, name.as_str()));
                        }
                    }
                    12 if monster => out.push(raised(t, "Slain")),
                    _ => {}
                }
            }
            (DIGITS, NUMBER_KIND) => {
                digits += 1;
                let value = match digits {
                    1 if !monster => Some(e.wanted),
                    2 => Some(e.points),
                    3 | 4 => e.prizes.get(digits - 3).map(|p| p.1),
                    5 if monster => Some(e.done),
                    6 if monster => Some(e.wanted),
                    _ => None,
                };
                out.extend(value.into_iter().flat_map(|v| number(pe, p, v)));
            }
            // The slash between the done and wanted counts is not drawn.
            (DIGITS, _) => {}
            (ITEM_PICTURE, _) => {
                if let Subject::Item(index) = e.subject
                    && let Some(piece) = layout::sprite(pe, n.part, n.cut.max(0), p.at)
                {
                    out.push(Element::Picture { pack: PICTURES.into(), index, size: piece_size(&piece), at: p.at, dim: false });
                }
            }
            (MONSTER_PICTURE, PICTURE) => {
                if let Subject::Monster(number) = e.subject
                    && let Some(piece) = layout::sprite(pe, n.part, n.cut.max(0), p.at)
                {
                    let percent = layout::word(pe, p, 5).filter(|&v| v > 0).unwrap_or(100) as f32 / 100.0;
                    // The picture fills the corner of its (larger) texture the node's cut names.
                    let [l, t, r, b] = piece.src.map(f32::from);
                    let rect = [p.at[0], p.at[1], p.at[0] + ((r - l) * percent) as i32, p.at[1] + ((b - t) * percent) as i32];
                    out.push(Element::Slice { pack: format!("a15_dict_mons{number:03}.g1t"), index: 0, src: [l, t, r, b], rect });
                }
            }
            _ => {}
        }
    }
    out
}

fn piece_size(p: &layout::Piece) -> [f32; 2] {
    [f32::from(p.src[2] - p.src[0]), f32::from(p.src[3] - p.src[1])]
}

/// The `SysInfo.xml` and `Menu_Line_Help.xml` row of this screen's header and footer lines.
const HELP_ROW: usize = 50;
/// The footer's row of `Menu_Line_Help.xml` (the first reading "Select, Return" alone; the rows are
/// not paired with `SysInfo.xml`'s, which row the game takes is not read).
const FOOTER_ROW: usize = 46;
/// The desk's cut (part 0 of `a15_menu.g1t`) behind this screen, from the capture.
const DESK_CUT: i32 = 1;

/// Shows the screen over the player's taken requests and `rank` (the save's rank number and
/// points) until Back closes it (or, offscreen, draws once after `keys` and writes `out`).
#[allow(clippy::too_many_arguments)]
pub fn run(
    gpu: &crate::Gpu,
    pool: ash::vk::CommandPool,
    target: &crate::screen::Target,
    window: Option<(&crate::present::Presenter, &mut crate::window::Pump)>,
    offscreen: Option<ash::vk::Image>,
    root: &std::path::Path,
    taken: &[Option<Taken>],
    rank: (i32, i32),
    keys: &[Button],
    out: Option<&std::path::Path>,
) -> crate::Result<Option<()>> {
    use crate::{hud, screen, text};
    let read = |rel: &str| -> crate::Result<barrel_formats::table::Element> { Ok(barrel_formats::table::parse(&std::fs::read(root.join(rel))?)?) };
    let exe = std::fs::read(root.join("Atelier_Escha_and_Logy_EN.exe"))?;
    let pe = Pe::parse(&exe)?;
    let items = read("Saves_EN/item/itemData.xml")?;
    let monsters: Vec<String> = barrel_escha::data::battle_data::monsters(&pe, barrel_escha::data::battle_data::MONSTERS_EN).ok_or("requests: no monster table")?.into_iter().map(|m| m.name).collect();
    let list = entries(taken, &Names { monsters: &monsters, items: &items.children });
    let ranks = barrel_escha::rank::ranks(&read("Saves/rank/RankData.xml")?);
    let panel = Panel::of(&ranks, rank.0, rank.1);
    let line = |file: &str, attribute: &str, row: usize| -> crate::Result<String> {
        Ok(barrel_escha::data::sys_text::lines(&read(&format!("Saves_EN/systemMessage/{file}"))?, attribute).get(row).cloned().unwrap_or_default())
    };
    let words = Words { header: line("SysInfo.xml", "Text", HELP_ROW)?, footer: line("Menu_Line_Help.xml", "LineHelp", FOOTER_ROW)? };
    let layouts = Layouts::load(&pe);
    let glyphs = barrel_escha::data::font::glyphs(&pe, barrel_escha::data::font::FONT_EN).ok_or("requests: no glyph table")?;
    let atlas_bytes = std::fs::read(root.join("Data/x64/Font_EN/MainFont_x64_0.g1t"))?;
    let atlas = barrel_formats::g1t::G1t::parse(&atlas_bytes)?;
    let font = text::Font { glyphs, size: atlas.textures.first().ok_or("requests: an empty font atlas")?.width as f32 };
    // Every picture any row's page shows.
    let every: Vec<Element> = (0..list.len().max(1)).flat_map(|chosen| elements(&pe, &layouts, &words, panel, &list, Requests { chosen })).collect();
    let needed = screen::sheet_keys(&pe, every);
    let packs = screen::read_packs(root, &needed)?;
    let parsed = packs.iter().map(|(n, b)| Ok((n.clone(), barrel_formats::g1t::G1t::parse(b)?))).collect::<crate::Result<Vec<_>>>()?;
    let mut images = vec![&atlas.textures[0]];
    let mut sheets = hud::Sheets { keys: Vec::new(), sizes: Vec::new(), first: 1 };
    for (name, index) in needed {
        let Some(t) = parsed.iter().find(|(n, _)| *n == name).and_then(|(_, g)| g.textures.get(index as usize)) else { continue };
        sheets.sizes.push([t.width as f32, t.height as f32]);
        sheets.keys.push((name, index));
        images.push(t);
    }
    let (w, h) = (target.width as f32, target.height as f32);
    let quads = |r: &Requests| hud::quads(&pe, Some(&font), &sheets, &elements(&pe, &layouts, &words, panel, &list, *r), w, h);
    let count = list.len();
    let mut state = Requests::default();
    let press = |r: &mut Requests, b: Button| r.press(b, count).then_some(());
    screen::run(gpu, pool, target, window, offscreen, &images, &mut state, quads, press, keys, out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn moves_and_closes() {
        let mut r = Requests::default();
        r.press(Button::Down, 3);
        r.press(Button::Down, 3);
        r.press(Button::Down, 3);
        assert_eq!(r.chosen, 2);
        r.press(Button::Up, 3);
        assert_eq!(r.chosen, 1);
        assert!(r.press(Button::Back, 3));
    }

    #[test]
    fn taken_requests_become_rows() {
        let monsters: Vec<String> = ["Puni", "Fake Ruin"].map(String::from).into();
        let taken = [Some(Taken { kind: 2, target: 1, wanted: 2, progress: 2, posted: 220, until: 329, points: 6, prizes: [(0, 2), (-1, 0)] }), None];
        let names = Names { monsters: &monsters, items: &[] };
        // No item rows here: the prize is left out; the request's own words stay.
        let rows = entries(&taken, &names);
        assert_eq!(rows, vec![Entry { kind: Kind::Subjugation, name: "Fake Ruin".into(), subject: Subject::Monster(1), wanted: 2, done: 2, points: 6, prizes: vec![] }]);
    }

    #[test]
    fn rank_points_to_the_next_row() {
        let row = |points| barrel_escha::rank::Rank { points, coal: 0, percent: 0, rates: [0; 7], number: 0 };
        let ranks = [row(0), row(750), row(1500), row(2250)];
        // Rank 3 (row 2) with 1565 points: 685 to row 3's 2250; icon cut 2.
        assert_eq!(Panel::of(&ranks, 3, 1565), Panel { icon: 2, to_next: 685 });
        assert_eq!(Panel::of(&ranks, 4, 2300).to_next, 0);
    }
}
