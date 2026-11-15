//! The Battle Equip. page of the main menu's character picker (the `menu-equip-battle` capture),
//! drawn from the game's equipment screen, layout 404: over the desk (383) the slot panel (403:
//! two stretched panels, four bars, the four slots of layout 390 with the worn item's name and
//! its kind's icon, the chosen slot's bar pulsing white to tan), the member's notebook (399:
//! the Status page's right page with the bookmark), the item list's frame and scroll bar (568,
//! empty while the cursor is on the slots), the chosen item's card (752, the Basket page's at
//! another place), the footer (372) and header (371) lines, the command's tab (531) and the
//! cursor's glow.
//!
//! The card is the Basket card's layout 16 left and 10 down from where the Basket page puts it,
//! without its panel (`basketpage::card_at`); the node places the layout 404 gives its nested
//! layouts are the Basket's, the game moves them. The slot icons' cut is the slot's kind (weapon
//! 1, armour 2, accessories 3). The pulse is fitted to the capture's nine frames (60 apart in
//! phase, see [`pulse`]).
//!
//! Not yet the game's: the cursor stays on the first slot (no moving, no list, no comparison
//! arrows of layout 406), the item's effect lines (the card shows its traits and properties).

use barrel_escha::data::layout::{self, Placed, Text};
use barrel_formats::pe::Pe;

use crate::basket::Item;
use crate::formation::Member;
use crate::hud::Element;
use crate::kit::{horizontal_bar, raised, tiled, vertical_bar};

const SCREEN: u32 = 404;
const SLOT_PANEL: u32 = 403;
const SLOT: u32 = 390;
const PANELS: u32 = 401;
const NOTEBOOK: u32 = 399;
const GRID: u32 = 568;
const SCROLL: u32 = 563;
const FRAME: u32 = 565;
/// Node kinds: a stretched panel, a three-cut bar.
const STRETCHED: i32 = 15;
const BAR: i32 = 19;
/// The slot bar's part, and the icon the slot's node (part 0xb cut 4) shows by the slot's kind.
const SLOT_BAR: i32 = 0x3d;
const SLOT_ICON: i32 = 0xb;
const ICON_CUTS: [i32; 4] = [1, 2, 3, 3];
const ICON_STEP: i32 = 56;
/// The card's "Item Info" pages (the capture's pager reads 1/2).
const ITEM_INFO_PAGES: i32 = 2;
/// The item names' ink: the notebook's dark brown (stored a, r, g, b).
const NAME_INK: [u8; 4] = [0xff, 58, 26, 8];
/// Where the card's layout goes (the Basket page puts it at (600, 162); fitted to the capture).
const CARD_AT: [i32; 2] = [660, 190];
/// The SysInfo.xml row ("Select equipment to change.") and the Menu_Line_Help.xml row (Select,
/// Details, Character, Remove, Return, Confirm) of the header and footer.
pub const HEADER_ROW: usize = 43;
pub const FOOTER_ROW: usize = 17;
/// The cursor's glow from the chosen slot's icon (the capture: (56, 140) with the icon at (146, 206)).
const CURSOR_FROM_ICON: [i32; 2] = [-90, -66];

/// The pulse's far colour (the capture's tan at 2820), its period in ticks of the clock the glows
/// keep (`game frame + 204`), the tick it is white at, and the curve's exponent. Fitted to the
/// nine captured frames (the first slot's bar at 2700 to 2860, every 20).
const TAN: [f32; 3] = [0.877, 0.703, 0.364];
const PULSE_PERIOD: f32 = 62.125;
const PULSE_WHITE_AT: f32 = 6.5;
const PULSE_SHAPE: f32 = 0.75;

/// The chosen slot bar's colour at clock tick `tick`: white to tan.
fn pulse(tick: u32) -> [u8; 3] {
    pulse_with(tick, PULSE_PERIOD, PULSE_WHITE_AT)
}

/// The same pulse with another period and white tick: a page's pulse starts when it opens, so
/// each page's phase is its own (the Adventure Equip. page's frame is fitted apart).
pub fn pulse_with(tick: u32, period: f32, white_at: f32) -> [u8; 3] {
    let phase = (tick as f32 - white_at) / period * std::f32::consts::TAU;
    let u = ((1.0 - phase.cos()) / 2.0).powf(PULSE_SHAPE);
    TAN.map(|t| ((1.0 - (1.0 - t) * u) * 255.0).round() as u8)
}

/// The layouts the page is made of, placed once.
pub struct Layouts {
    desk: Vec<Placed>,
    screen: Vec<Placed>,
    card: crate::basketpage::Layouts,
    cursor: Option<layout::Flip>,
    pub tab: Vec<Placed>,
    pub header: Vec<Placed>,
    pub footer: Vec<Placed>,
}

impl Layouts {
    pub fn load(pe: &Pe) -> Layouts {
        let get = |id| layout::placed(pe, id);
        let pick = crate::picker::Layouts::load(pe);
        Layouts { desk: get(crate::picker::DESK), screen: get(SCREEN), card: crate::basketpage::Layouts::load(pe), cursor: crate::kit::cursor_glow(pe), tab: pick.tab, header: pick.header, footer: pick.footer }
    }

    /// Every cut the page's flipbooks turn through.
    pub fn flips(&self) -> Vec<Element> {
        self.cursor.into_iter().flat_map(|f| crate::kit::cuts_of(crate::kit::CURSOR, f)).collect()
    }
}

/// What the page shows of the chosen member: the notebook's values and what each slot holds
/// (the item's name and its card).
pub struct Worn {
    pub member: Member,
    pub slots: [Option<Item>; 4],
}

/// The slot panel (403): its stretched panels, bars and the four slots.
fn slots(pe: &Pe, l: &Layouts, w: &Worn, chosen: usize, tick: u32, cursor: &mut Option<[i32; 2]>) -> Vec<Element> {
    let mut out = Vec::new();
    let (mut slot, mut first_icon) = (0, 0);
    for p in l.screen.iter().filter(|p| p.path.get(1) == Some(&SLOT_PANEL)) {
        let n = p.node;
        let word = |i| layout::word(pe, p, i).unwrap_or(0);
        if p.path.get(2) == Some(&PANELS) && n.kind == STRETCHED {
            out.extend(tiled(pe, n.part, p.at, [word(5), word(6)]));
        } else if n.kind == BAR {
            let rgb = crate::kit::ink(pe, p);
            out.extend(horizontal_bar(pe, n.part, p.at, word(5)).into_iter().map(|e| match e {
                Element::Slice { pack, index, src, rect } => Element::InkedSlice { pack, index, src, rect, rgb },
                e => e,
            }));
        } else if p.path.get(2) == Some(&SLOT) {
            // A slot's nodes: its bar (only the chosen one's is drawn), name text, kind icon.
            let item = w.slots[slot].as_ref();
            match n.part {
                SLOT_BAR if slot == chosen => out.push(Element::Tinted { part: SLOT_BAR, cut: n.cut, at: p.at, rgb: pulse(tick) }),
                0x164 => out.extend(item.and_then(|i| layout::text(pe, p).map(|t| raised(Text { colour: NAME_INK, ..t }, &i.name)))),
                SLOT_ICON => {
                    // The icons are a fixed step apart (the layout's third and fourth are 2 low).
                    if slot == 0 {
                        first_icon = p.at[1];
                    }
                    let at = [p.at[0], first_icon + ICON_STEP * slot as i32];
                    out.push(Element::Sprite { part: SLOT_ICON, cut: ICON_CUTS[slot], at });
                    if slot == chosen {
                        *cursor = Some([at[0] + CURSOR_FROM_ICON[0], at[1] + CURSOR_FROM_ICON[1]]);
                    }
                    slot += 1;
                }
                _ => {}
            }
        }
    }
    out
}

/// The item list: its scroll bar and frame (no items shown, the cursor is on the slots).
fn list(pe: &Pe, l: &Layouts) -> Vec<Element> {
    let mut out = Vec::new();
    for p in l.screen.iter().filter(|p| p.path.get(1) == Some(&GRID)) {
        let n = p.node;
        match p.path.get(3..).unwrap_or(&[]) {
            [SCROLL] if n.kind == 20 => out.extend(vertical_bar(pe, n.part, p.at, layout::word(pe, p, 6).unwrap_or(0))),
            [SCROLL] => out.push(Element::stored(p)),
            [] if p.path.get(2) == Some(&FRAME) && n.part == 0x45 => out.push(Element::stored(p)),
            _ => {}
        }
    }
    out
}

/// All the page's elements for the chosen member `w`, slot `chosen` lit, `lead` the lead's
/// character number, `tick` the clock's 60 Hz ticks.
pub fn elements(pe: &Pe, l: &Layouts, words: &crate::picker::Words, w: &Worn, chosen: usize, lead: usize, tick: u32) -> Vec<Element> {
    let mut out: Vec<Element> = l.desk.iter().map(|p| Element::Sprite { part: p.node.part, cut: crate::picker::DESK_CUT, at: p.at }).collect();
    let mut cursor = None;
    for p in l.screen.iter().filter(|p| p.path.get(1) == Some(&NOTEBOOK) && p.path.len() == 2) {
        out.push(Element::stored(p));
    }
    let page: Vec<&Placed> = l.screen.iter().filter(|p| p.path.get(2) == Some(&crate::statuspage::RIGHT_PAGE)).collect();
    out.extend(crate::statuspage::right_page(pe, &page, &w.member, w.member.face.character as usize == lead));
    out.extend(slots(pe, l, w, chosen, tick, &mut cursor));
    out.extend(crate::picker::chrome(pe, (&l.footer, &words.footer), (&[], ""), &[], 0));
    out.extend(list(pe, l));
    if let Some(item) = w.slots.get(chosen).and_then(Option::as_ref) {
        out.extend(crate::basketpage::card_at(pe, &l.card, item, CARD_AT, ITEM_INFO_PAGES));
    }
    out.extend(crate::picker::chrome(pe, (&[], ""), (&l.header, &words.header), &l.tab, crate::picker::Kind::BattleEquip.tab_cut()));
    out.extend(cursor.map(|at| Element::Sprite { part: crate::kit::CURSOR, cut: l.cursor.map_or(0, |f| f.cut(tick)), at }));
    out
}
