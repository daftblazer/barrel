//! The main menu's first page, the party overview (layout 385 on the desk 383): the command
//! list down the left (the buttons of layout 373), the open book (386) with the lead's portrait,
//! name, LV, HP, MP, Next and Alchemy Lv (layout 368), the adventurer rank panel and the money
//! (367, 528), and the other party members' plates down the right (360), under the header (371)
//! and footer (372) lines.
//!
//! The game draws it in this order: the desk, the command buttons, the book (its two stretched
//! halves, then the left page), the rank icon and points to the next rank, the money, the lead's
//! portrait and name plate and values, then each plate, the footer and the header. A number is
//! a row of digit sprites (a node of kind 28) ending at its node, drawn in the node's ink; the
//! sprite's width is the digit's advance.
//!
//! Read from the layouts: the pieces, their places, the order and the numbers' nodes. From the
//! save: the values (levels, experience, HP and MP now and most with equipment, the friendship
//! float truncated, the alchemy level and experience, the rank, the money). From
//! `SysInfo.xml` and `Menu_Line_Help.xml`: the header line of the chosen command and the
//! footer's row 0.
//!
//! Seen in the capture (the area map): the list without the Container command (it starts at
//! the Basket: the game leaves Container out away from a town), the "NEW" badge on Journal while
//! the library has unseen entries, the Basket's help line (row 3). Guesses: the help line of
//! each other command (the rows follow the commands' order in `SysInfo.xml`), the Container in
//! towns, Logy's portrait (the lead's page shows Escha's). Left out: the reserve row (no member
//! waits in the capture), the small coin icon of layout 367 (not drawn), the name plate's tilt
//! and the pulsing glow and badge (they move with time).

use barrel_escha::data::layout::{self, Placed, Text};
use barrel_escha::session::Member;
use barrel_formats::pe::Pe;

use crate::hud::{Element, TEXT_RISE};
use crate::menu::{Command, Menu, COMMANDS};

pub const LAYOUT: u32 = 385;
const HEADER: u32 = 371;
const FOOTER: u32 = 372;
const BOOK: u32 = 386;
/// The layout holding the book, the rank panel and the money, nested in the page.
const BOOK_HOLDER: u32 = 367;
const PAGE: u32 = 368;
const RANK: u32 = 528;
const PLATE: u32 = 360;
const BUTTON: u32 = 373;
const NAME_PLATE: u32 = 369;
const MONEY: i32 = 0x4d;
const RANK_ICON: i32 = 0x31;
const TEXT: i32 = 0x164;
/// The member's face on a plate (part 0x4 of `a15_menu.g1t`: cut = the character's number).
const FACE: i32 = 4;
const NUMBER_KIND: i32 = 28;
const BAR_KIND: i32 = 19;
/// The digit sheet's slash (cut 12 of the number parts).
const SLASH: i32 = 12;
/// The row of `SysInfo.xml` of the first command's help line, and the footer's row of
/// `Menu_Line_Help.xml`.
const HELP_ROW: [usize; 10] = [4, 3, 7, 6, 8, 9, 12, 11, 13, 30];
const FOOTER_ROW: usize = 0;
/// A name's filler in the layouts.
const NAME_FILLER: &str = "アイウエオカ";
/// The desk's cut (part 0 of `a15_menu.g1t`) behind this screen, from the capture.
const DESK_CUT: i32 = 1;

/// One member's plate or the lead's page.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Face {
    pub name: String,
    pub character: i32,
    pub level: i32,
    pub hp: (i32, i32),
    pub mp: (i32, i32),
    pub next: i32,
    pub friend: i32,
}

/// What the page shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Overview {
    pub lead: Face,
    /// The alchemy level and the experience still needed for the next.
    pub alchemy: (i32, i32),
    pub money: i32,
    /// The rank's icon cut and the points still needed for the next rank.
    pub rank: (i32, i32),
    /// The members besides the lead who fight, in the squad's order.
    pub plates: Vec<Face>,
    /// The library has entries not looked at yet (the Journal button's badge).
    pub new_journal: bool,
}

/// The save's side of the page: the party and what the menu reads of the session.
pub struct Source<'a> {
    pub lead: usize,
    pub party: &'a [Member],
    pub squad: &'a [usize],
    pub friendship: &'a [f32],
    pub alchemy: (i32, i32),
    pub money: i32,
    pub rank: (i32, i32),
    pub library_new: bool,
    /// The Container command is in the menu's list.
    pub container: bool,
}

impl<'a> Source<'a> {
    pub fn of(s: &'a barrel_escha::session::Session) -> Source<'a> {
        Source { lead: usize::from(s.logy), party: &s.party, squad: &s.squad, friendship: &s.friendship, alchemy: s.alchemy, money: s.money, rank: s.rank, library_new: s.library_new, container: s.field().is_some_and(|m| (m as usize) < barrel_escha::data::gimmick::TOWN_MAPS) }
    }
}

impl Overview {
    /// The page from `source`, with the game's tables: the executable `pe`, `items`
    /// (`itemData.xml`'s rows), `ranks` (`RankData.xml`).
    pub fn of(pe: &Pe, source: &Source, items: &[barrel_formats::table::Element], ranks: &[barrel_escha::rank::Rank]) -> Option<Overview> {
        use barrel_escha::data::battle_data as bd;
        let exp = bd::exp_table(pe, bd::EXP_TABLE_EN)?;
        let face = |c: usize| -> Option<Face> {
            let v = crate::status::Values::of(pe, c, source.party.get(c), items)?;
            Some(Face { name: v.name, character: c as i32, level: v.level, hp: v.hp, mp: v.mp, next: v.next, friend: source.friendship.get(c).map_or(0, |&f| f as i32) })
        };
        let squad: Vec<usize> = if source.squad.is_empty() { (0..2).collect() } else { source.squad.to_vec() };
        let (level, alchemy_exp) = source.alchemy;
        let (rank, points) = source.rank;
        let to_next = usize::try_from(rank).ok().and_then(|next| ranks.get(next)).map_or(0, |r| (r.points - points).max(0));
        Some(Overview {
            lead: face(source.lead)?,
            alchemy: (level, barrel_escha::growth::alchemy_to_next(&exp, level, alchemy_exp).unwrap_or(0).max(0)),
            money: source.money,
            rank: ((rank - 1).max(0), to_next),
            plates: squad.into_iter().filter(|&c| c != source.lead).filter_map(face).collect(),
            new_journal: source.library_new,
        })
    }
}

/// The layouts the page draws, placed once.
pub struct Layouts {
    screen: Vec<Placed>,
    header: Vec<Placed>,
    footer: Vec<Placed>,
}

impl Layouts {
    pub fn load(pe: &Pe) -> Layouts {
        Layouts { screen: layout::placed(pe, LAYOUT), header: layout::placed(pe, HEADER), footer: layout::placed(pe, FOOTER) }
    }
}

/// The header (the chosen command's help) and footer lines.
pub struct Words {
    pub help: Vec<String>,
    pub footer: String,
}

impl Words {
    /// From `SysInfo.xml` and `Menu_Line_Help.xml`'s rows.
    pub fn read(sys_info: &barrel_formats::table::Element, line_help: &barrel_formats::table::Element) -> Words {
        let info = barrel_escha::data::sys_text::lines(sys_info, "Text");
        let help = HELP_ROW.iter().map(|&row| info.get(row).cloned().unwrap_or_default()).collect();
        let footer = barrel_escha::data::sys_text::lines(line_help, "LineHelp").get(FOOTER_ROW).cloned().unwrap_or_default();
        Words { help, footer }
    }
}

/// A number's ink: the node's word 7 (stored r, g, b, a).
fn ink(pe: &Pe, p: &Placed) -> [u8; 3] {
    let [r, g, b, _] = layout::word(pe, p, 7).unwrap_or(0).to_le_bytes();
    [r, g, b]
}

/// `value` as digit sprites of the node's part, the last just left of the node's place.
fn number(pe: &Pe, p: &Placed, value: i32) -> Vec<Element> {
    let part = p.node.part;
    let advance = layout::sprite(pe, part, 0, [0, 0]).map_or(20, |s| i32::from(s.src[2] - s.src[0]));
    let digits = value.max(0).to_string();
    let rgb = ink(pe, p);
    let last = digits.len() as i32 - 1;
    digits
        .bytes()
        .enumerate()
        .map(|(i, d)| Element::Tinted { part, cut: i32::from(d - b'0'), at: [p.at[0] - advance * (last - i as i32 + 1), p.at[1]], rgb })
        .collect()
}

/// A text of a node as the game draws it: raised by [`TEXT_RISE`], opaque (the stored alpha
/// of the layouts' ink is 0xe1, the game draws it whole), `text` in place of a stand-in.
fn raised(t: Text, text: Option<&str>) -> Element {
    let colour = if t.colour[0] == 0 { t.colour } else { [0xff, t.colour[1], t.colour[2], t.colour[3]] };
    Element::Label(Text { at: [t.at[0], t.at[1] - TEXT_RISE], text: text.map_or(t.text.clone(), str::to_string), colour, ..t })
}

/// Whether a text is a stand-in the game replaces (a name's katakana filler, or the usual kana).
fn stand_in(t: &Text) -> bool {
    t.filled_in() || t.text == NAME_FILLER
}

/// A bar of three cuts (`part`'s cuts 0 to 2: left end, middle, right end) over `width` from
/// `at`: the middle repeated, its last repeat cut short.
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

/// The nodes of `group` (a nested layout) of the screen, grouped by the place the nesting put
/// them at, each group in node order (the plates: one group each).
fn groups(nodes: &[Placed], group: u32) -> Vec<Vec<&Placed>> {
    let mut out: Vec<(Vec<u32>, [i32; 2], Vec<&Placed>)> = Vec::new();
    for p in nodes.iter().filter(|p| p.path.get(1) == Some(&group)) {
        let origin = [p.at[0] - p.node.at[0], p.at[1] - p.node.at[1]];
        match out.iter_mut().find(|g| g.1 == origin) {
            Some(g) => g.2.push(p),
            None => out.push((p.path.clone(), origin, vec![p])),
        }
    }
    out.into_iter().map(|g| g.2).collect()
}

/// One plate's or the page's elements: the sprites as stored, the texts raised (the first
/// filled-in text the name), the numbers from `numbers` in the nodes' order, a slash drawn in the
/// ink of the number before it.
fn face(pe: &Pe, nodes: &[&Placed], texts: &impl Fn(&Placed) -> Option<Text>, name: &str, portrait: Option<i32>, numbers: &[i32]) -> Vec<Element> {
    let mut out = Vec::new();
    let mut k = 0;
    let mut named = false;
    for p in nodes {
        let n = p.node;
        if n.part == TEXT {
            let Some(t) = texts(p) else { continue };
            if stand_in(&t) && !named {
                named = true;
                out.push(raised(t, Some(name)));
            } else if !stand_in(&t) && !t.text.is_empty() {
                out.push(raised(t, None));
            }
        } else if n.kind == NUMBER_KIND {
            let Some(&value) = numbers.get(k) else { continue };
            k += 1;
            out.extend(number(pe, p, value));
        } else if let (FACE, Some(cut)) = (n.part, portrait) {
            out.push(Element::Sprite { part: FACE, cut, at: p.at });
        } else if n.cut == SLASH && matches!(n.part, 0x4e | 0x50) {
            out.push(Element::Tinted { part: n.part, cut: SLASH, at: p.at, rgb: [58, 26, 8] });
        } else {
            out.push(Element::stored(p));
        }
    }
    out
}

/// All the elements of the page with `menu`'s command chosen.
pub fn elements(pe: &Pe, l: &Layouts, menu: &Menu, v: &Overview, words: &Words, texts: impl Fn(&Placed) -> Option<Text>) -> Vec<Element> {
    let nodes = &l.screen;
    let mut out: Vec<Element> = nodes.iter().filter(|p| p.path.len() == 1 && p.node.part == 0).map(|p| Element::Sprite { part: 0, cut: DESK_CUT, at: p.at }).collect();
    let buttons: Vec<Placed> = nodes.iter().filter(|p| p.path.get(1) == Some(&BUTTON)).cloned().collect();
    out.extend(menu.elements_with(&buttons, &texts, v.new_journal.then_some(Command::Journal)));
    // The page: the book (386 through 367), the rank panel and the money, then 368's own nodes
    // down to the alchemy numbers, then the name plate.
    let page: Vec<&Placed> = nodes.iter().filter(|p| p.path.get(1) == Some(&PAGE)).collect();
    let lead = &v.lead;
    let mut ranked = false;
    for p in page.iter().filter(|p| p.path.get(2) == Some(&BOOK_HOLDER)) {
        match p.path.get(3) {
            Some(&BOOK) if p.node.kind == BAR_KIND => out.extend(bar(pe, p.node.part, p.at, layout::word(pe, p, 5).unwrap_or(0))),
            Some(&BOOK) => out.push(Element::stored(p)),
            Some(&RANK) => match (p.node.part, p.node.kind) {
                // The first icon's place shows the rank's; the other places stay empty.
                (RANK_ICON, _) if p.node.cut == 0 => out.push(Element::Sprite { part: RANK_ICON, cut: v.rank.0, at: p.at }),
                // The first number is the points to the next rank; the pair after it is not shown.
                (_, NUMBER_KIND) if !ranked => {
                    ranked = true;
                    out.extend(number(pe, p, v.rank.1));
                }
                (TEXT, _) => out.extend(texts(p).map(|t| raised(t, None))),
                _ => {}
            },
            _ if p.node.part == MONEY && p.node.kind == NUMBER_KIND => out.extend(number(pe, p, v.money)),
            _ => {}
        }
    }
    let own: Vec<&Placed> = page.iter().filter(|p| !matches!(p.path.get(2), Some(&BOOK_HOLDER) | Some(&NAME_PLATE))).copied().collect();
    out.extend(face(pe, &own, &texts, &lead.name, None, &[lead.level, lead.hp.0, lead.hp.1, lead.mp.0, lead.mp.1, lead.next, v.alchemy.0, v.alchemy.1]));
    for p in page.iter().filter(|p| p.path.get(2) == Some(&NAME_PLATE)) {
        match texts(p) {
            Some(t) => out.push(raised(t, Some(&lead.name))),
            None => out.push(Element::stored(p)),
        }
    }
    for (slot, plate) in groups(nodes, PLATE).into_iter().zip(&v.plates) {
        out.extend(face(pe, &slot, &texts, &plate.name, Some(plate.character), &[plate.level, plate.hp.0, plate.hp.1, plate.mp.0, plate.mp.1, plate.next, plate.friend]));
    }
    let line = |placed: &[Placed], text: &str| -> Vec<Element> {
        placed
            .iter()
            .flat_map(|p| match texts(p) {
                Some(t) => vec![raised(t, Some(text))],
                None => vec![Element::stored(p)],
            })
            .collect()
    };
    out.extend(line(&l.footer, &words.footer));
    let chosen = menu.commands().get(menu.chosen).and_then(|c| COMMANDS.iter().position(|x| x == c));
    out.extend(line(&l.header, chosen.and_then(|k| words.help.get(k)).map_or("", String::as_str)));
    out
}
