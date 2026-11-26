//! The field menu's Equipment page: the character's figure, the four slots with the worn items' pictures and
//! the stat panel with the real numbers, from the game's layouts (`uil_a21_menu_equip.xml`, `uil_a21_menu_chara.xml`,
//! `uil_a21_menu_chara_status.xml`) and their style files (`ui_cmn/gen_styles`, `ui_cmn/etc`).
//!
//! The layouts are read by a small generic walker: a node has a place, a turn and a scale about its top left,
//! an image takes its style's picture (a list's `image_no`) at the style's natural size or its `size_wh`, a text
//! its string, font and colour (with the nine-copy outline of `bold_size`). What the game's code decides (which
//! nodes show, the numbers, the item pictures) is given to the walker as a list of overrides by node path.
//!
//! Drawn: the figure, the four slots, the stat panel with the element resists, the party icons, the title plate and (by
//! `field_menu::View::page_help`) the help bar, with the cursor's blink and the party icons' animations. Not drawn: the item card, the
//! slides and fades of the page's coming and going (docs/ryza/open.md, "Field menu, part 7").

mod material_view;
mod prompt_view;
mod recipe_view;
mod ring_view;
pub use material_view::{MaterialInfo, MaterialItem, MaterialsLook};
pub use prompt_view::Prompt;
pub use recipe_view::{AddPanel, RecipeInfo, RecipeLook, RecipeRow, Tutorial};
pub use ring_view::{AddLook, FieldLook, Popup, RingShow, FOCUS_ZERO_AT};

use std::collections::{BTreeSet, HashMap};

use barrel_formats::table::{self, Element};
use barrel_render::text::{Clip, Glyph, Quad, Spin};

use crate::font::ATLAS_SIZE;
use crate::item_info::{Card, CATEGORY_BASE, EFFECT_BASE, NAME_BASE, POTENTIAL_BASE};
use crate::load_view::TEXT_TOP;
use crate::party_skills::{Skill, ACTION_COMMENT, ACTION_NAME};
use crate::options_view::segments;
use crate::title::{numbers, pair, LAYOUT_HEIGHT};

/// The page layer's textures: the two fonts and the white, then the styles' (`Page::wanted`).
pub const FONT01: usize = 0;
pub const FONT00: usize = 1;
pub const WHITE: usize = 2;
const FIXED: usize = 3;

/// Style pixels per layout pixel (the styles are drawn for a 2160-line screen, the layouts for 720).
const PER_LAYOUT: f32 = 3.0;
/// A texture's size in style pixels over its own.
const STYLE_OVER_TEXTURE: f32 = 2.0;
/// `STR_UI_nnnn` is the string numbered `STR_UI_BASE + nnnn`.
const STR_UI_BASE: u32 = 6_619_137;
const OUTLINE_ALPHA: f32 = 0.5;
/// The title plate's string ("Equipment"; the game's code names it, the layout's is another).
/// The help bar's plate: texture 3 of `a21_common.g1t`.
const HELP_PLATE: usize = 3;
const EQUIPMENT_TITLE: u32 = 6_684_752;
/// The Status page's title plate string (the menu's own word "Status").
const STATUS_TITLE: u32 = 4_194_516;
/// The synthesis's finished screen's title plate ("Complete", `str_menu_define`).
const COMPLETE_TITLE: u32 = 4_194_480;
/// The party icons' places in the layout.
const ICON_PLACES: usize = 9;
// STAND-IN: the shoulder buttons' glyphs (private-use characters of `MainFont00`) are the capture's; the game picks them by the pad's assignment
// (`STR_QUICK_MENU_DEFINE_331` and `_332`).
const BUTTON_SIZE: f32 = 24.0;
const BLINK_FRAMES: f32 = 120.0;
/// STAND-IN: the cursor's blink is read from the frame before the key's (`field-menu-equip`: DOWN on 2300, the opacity of 217/255 on
/// 2410, 111 frames into the curve).
pub const BLINK_LAG: f32 = 1.0;
/// The slot cursor's blink runs from this many frames after A on the card, before any move (`field-menu-equip` 2290: phase 29 of 120).
/// STAND-IN: one sample (the page itself is first drawn at `PAGE_AT`).
pub const BLINK_FROM: f32 = 1.0;
/// The icon highlight's animations: a turn in 360 frames; the glow's 100 frames, its opacity peak (frame 2.82, 100 of 255) and a lag
/// STAND-IN fitted to the capture's opacity of 33/255 on 2410.
const ROTATION_FRAMES: f32 = 360.0;
const EFFECT_FRAMES: f32 = 100.0;
const EFFECT_PEAK: f32 = 2.818_787_6;
const EFFECT_ALPHA: f32 = 100.0 / 255.0;
const EFFECT_LAG: f32 = 0.3;
/// STAND-IN: the glow's loop is 101 frames long (the animation's 100 and one more; `field-menu-status` 2400, 2550 and 2620 and
/// `field-menu-equip` 2410 fit one lag with it, where a loop of 100 needs 0.4, 1.9 and 2.1), and B on the page puts it a frame ahead.
const EFFECT_LOOP: f32 = 101.0;
const EFFECT_AFTER_B: f32 = 1.0;
const CENTER_LIFT: f32 = 0.6875;
const LEFT_BUTTON: char = '\u{4e6}';
const RIGHT_BUTTON: char = '\u{4ea}';
// STAND-IN: the Status tabs' shoulder-button glyphs (LB and RB) are the capture's.
const TAB_LEFT_BUTTON: char = '\u{4e3}';
const TAB_RIGHT_BUTTON: char = '\u{4e7}';
const BROWN: [f32; 4] = [26.0, 0.0, 0.0, 255.0];
/// The slide-in animations take 30 frames (`anm_common_slide_in_left` and the others of `uia_common.xml`); the name plate's
/// starts 6 frames after the figure's (`delay="0.10"` of the chara layout's `get_in_ud`); the party icons fade in over 30
/// (`anm_common_juvenile_fade_in`, 60 frames at twice the speed).
const SLIDE_FRAMES: f32 = 30.0;
const NAME_DELAY: f32 = 6.0;
/// The Equipment page's slots going out: frames (the page's own, `Pick::page_out`) and the sink in layout pixels (fitted to 2510, 2520).
const SLOT_OUT_FRAMES: f32 = 24.0;
const SLOT_SINK: f32 = 20.0;
/// The figures' change of character (`anm_chara_in`, `anm_chara_change_out`: 24 frames).
const CHANGE_FRAMES: f32 = 24.0;
const FACE_FADE_FRAMES: f32 = 30.0;
/// The skills list: the cursor's place in the window's body, a row's height, and the cursor's glow (`anm_cursor_blink_ex_skill`).
const CURSOR_AT: [f32; 2] = [-13.0, 25.0];
const ROW_STEP: f32 = 27.5;
const CURSOR_GLOW_FRAMES: f32 = 192.0;
const CURSOR_GLOW_PEAK: f32 = 100.0;
// STAND-IN: the skills' names are drawn in this colour (3/255 red; the layout's own is 26) in the capture of the Status page: the
// menu's own font colour is not read.
const ROW_COLOR: [f32; 4] = [3.0 / 255.0, 0.0, 0.0, 1.0];
/// A description's line step in layout pixels.
// STAND-IN: fitted to the Status page's description (lines 33 pixels apart at 1080 lines with the layout's size 14 and line_space 14),
// as `boot`'s `LINE_STEP`: the game's rule (the font's line height plus `line_space`) is not read.
const LINE_STEP: f32 = 22.0;

/// What the page reads from the game's files, by install path (`\saves\ui_en\...`).
pub struct Files<'a> {
    pub read: &'a dyn Fn(&str) -> Result<Vec<u8>, String>,
    pub glyphs: [HashMap<char, Glyph>; 2],
}

/// A style's picture: its texture (a slot of the layer), its place in the atlas and the texture's size.
#[derive(Clone, Debug)]
struct Pic {
    path: String,
    index: usize,
    uvwh: [f32; 4],
    /// Style pixels to a layout pixel (3 for a style file of `resolution_hd` 2160, 1.5 for 1080) and to a texture pixel (2 or 1).
    per_layout: f32,
    over_texture: f32,
    /// A `wrap` picture's margins (left, top, right, bottom): a nine-slice whose middle repeats.
    edges: Option<[f32; 4]>,
}

/// An item of the Container page's grid, for the textures the page loads: its picture number and its category's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShelfItem {
    pub image: usize,
    pub category: Option<usize>,
}

/// What the Container page shows.
pub struct ShelfLook<'a> {
    pub party: &'a [crate::field_menu::Member],
    /// The grid's items in order (a picture number each), the cell under the cursor and the tab.
    pub images: &'a [usize],
    pub cursor: usize,
    pub tab: usize,
    /// The free places of the container (the count's "Empty").
    pub free: usize,
    /// The card of the item under the cursor.
    pub item: Option<&'a Card>,
    /// Frames since the page's first, and, going out, since B.
    pub age: f32,
    pub leaving: Option<f32>,
}

/// What the Gathering Tools page shows.
pub struct ToolsLook<'a> {
    pub party: &'a [crate::field_menu::Member],
    /// The tools' picture numbers in order (the row's cells), the cell under the cursor and its card.
    pub images: &'a [usize],
    pub cursor: usize,
    pub item: Option<&'a Card>,
    /// Frames since the page's first, and, going out, since B.
    pub age: f32,
    /// The same on game frames (the title plate does not lose the animation clock's stall).
    pub plate_age: f32,
    pub leaving: Option<f32>,
}

/// `STR_LIBRARY_DEFINE_nnn` is the string numbered `LIBRARY_BASE + nnn` (`str_library_define.xml`: the Guide's buttons are 24, 36, 40 and 46).
const LIBRARY_BASE: u32 = 4_063_233;
const LIBRARY_STRINGS: u32 = 128;

/// What the Guide page shows.
pub struct GuideLook<'a> {
    /// The button under the cursor (Items, Effects/Traits, Monsters, Help).
    pub cursor: usize,
    /// Frames since the page's first frame (its first draws at 1), and, going out, since B.
    pub age: f32,
    pub leaving: Option<f32>,
    /// Frames since the cursor last moved (its highlight's blink begins again), if it has.
    pub moved: Option<f32>,
    /// A sub-page opened from the top, and the frames since the top's blink began (the top's own clock when none is up).
    pub sub: Option<SubLook<'a>>,
}

/// A sub-page of the Guide: Items or Effects/Traits.
pub struct SubLook<'a> {
    pub page: SubPage<'a>,
    /// Frames since A on the top, and, going back, since B on the page.
    pub since: f32,
    pub back: Option<f32>,
    /// Frames since the first A that opened this page: its cursor's blink runs from there, whether the page is up or not.
    pub clock: f32,
    /// The Gather Rate in per cent, and the page's cursor (a card, an effect).
    pub rate: u32,
    pub cursor: usize,
}

/// What a sub-page lists.
pub enum SubPage<'a> {
    Items(&'a [crate::library::Card]),
    Effects(&'a [crate::library::Effect]),
}

/// Frames from A on the Guide's top to the sub-page's first frame: the top fades out first (`field-menu-guide` 2330: A, 2340 the top at a third of
/// its opacity, 2350 the page at a third coming in).
pub const SUB_AT: f32 = GUIDE_FADE;

/// The sub-pages' cursor blink runs 120 frames in 120.75 of the capture's (`field-menu-guide`: the Effects list's lowest points 2675.2, 2917.8 and 3160.4 are two periods
/// of 121.3 apart; the Items' peak 2467.3 is 121.3 after its start): the game's animation clock is a little slow. STAND-IN: the ratio as measured.
const BLINK_RATE: f32 = 0.9938;

/// The Items page's cards lie tilted: the game turns each card's `node_center` (its paper and cursor highlight; the picture, name and level stay upright)
/// about its own origin and squashes it along its own height, as (degrees clockwise, the height's share). From the capture's corners (`field-menu-guide`
/// 2400: both cards' pieces solve to one rotation about the node's origin, (206, 262) layout pixels, to 0.01 pixel); the first two cards of the capture are
/// the only samples. STAND-IN: the game's rule (a seed per card, the item?) is not read; the two repeat down the list.
const CARD_TILTS: [(f32, f32); 2] = [(39.770, 0.96623), (-59.106, 0.77800)];

/// The colour an item's picture takes while the library shows it only as a silhouette (`field-menu-guide` 2440).
const SILHOUETTE: [f32; 4] = [139.0 / 255.0, 113.0 / 255.0, 88.0 / 255.0, 1.0];

/// The Guide's fades take 15 frames (`anm_common_fade_in`); the page's own fade and the top's (a second one) run together coming in, and going
/// out the top goes first and the background after it (`field-menu-pages` 3210: 136 and 72 of 255, 3320: the top at a third, the background whole,
/// 3330: the background at two thirds).
const GUIDE_FADE: f32 = 15.0;
/// The Guide's button labels' size in layout pixels: the layout names none; fitted to the glyphs' height of `field-menu-pages` 3250 (32 pixels of 1080
/// lines for a capital, 1.335 times the size). STAND-IN: the game's default for a text with no size is not read.
const GUIDE_TEXT_SIZE: f32 = 24.0;

/// The Gathering Tools page's title plate leaves this many frames before its other parts (STAND-IN: one sample).
const TOOLS_TITLE_EARLY: f32 = 1.0;

/// The Gathering Tools page's title.
pub const TOOLS_TITLE: u32 = 4_194_514;
/// The page's strings: "Container", "Empty" and "All".
pub const CONTAINER_TITLE: u32 = 6_684_760;
const EMPTY_WORD: u32 = 6_684_711;
const ALL_WORD: u32 = 4_194_319;
const SHELF_STRINGS: [u32; 4] = [CONTAINER_TITLE, EMPTY_WORD, ALL_WORD, TOOLS_TITLE];
/// The tab names (consumables ... exploration): the page shows its tab's.
const TAB_NAMES: u32 = 6_684_751;
/// The grid's columns, the tabs' label pictures (`gen_a21_mix01_choise`'s: all, materials, consumables, equipment, synthesis, important) and
/// the page's slide (`speed` 0.75 of 30 frames).
const SHELF_COLUMNS: usize = 8;
const SHELF_TABS: [usize; 6] = [0, 5, 6, 7, 8, 9];
const SHELF_SLIDE_FRAMES: f32 = 40.0;
/// The items' opacity in the grid (242 of 255 in the capture: not explained; STAND-IN).
const ITEM_ALPHA: f32 = 242.0 / 255.0;
/// The cursor cell's animation loops in 120 frames from the page's first; STAND-IN: a tenth of a frame early, which puts the byte roundings of
/// `field-menu-pages` 2080, 2100, 2110 and 2120 where the game's are.
const CURSOR_LOOP: f32 = 120.0;
const CURSOR_PHASE_LAG: f32 = -0.1;
/// The Effects list's scroll bar's slider with the list not scrolled: from the bar's top, its length (STAND-IN: the capture's, the rule is not read).
const EFFECT_THUMB: [f32; 2] = [18.0, 462.0];
/// The scroll bar's slider with the grid not full: from the bar's top, its length (STAND-IN: the capture's, the rule is not read).
const SHELF_THUMB: [f32; 2] = [18.0, 390.0];
/// The count's label and number: where the capture puts them in the `cc` node (the layout's are (-52, 31) and (-5, 31); the game's code
/// moves them: STAND-IN).
const COUNT_LABEL_AT: [f32; 2] = [-65.44, 31.0];
const COUNT_VALUE_AT: [f32; 2] = [31.44, 31.0];

/// What a page shows: the member whose page it is and the slot under the cursor.
pub struct Look<'a> {
    pub member: &'a crate::field_menu::Member,
    /// The party in the menu's order and the place of `member` in it (the party icons at the top right).
    pub party: &'a [crate::field_menu::Member],
    pub card: usize,
    pub item_images: &'a dyn Fn(u16) -> usize,
    /// The slot (0 weapon, 1 armour, 2 and 3 accessories) the cursor is on.
    pub slot: usize,
    /// Frames since the page's first.
    pub age: f32,
    /// Frames the cursor has been on its slot (its blink's clock).
    pub slot_age: f32,
    /// The Status page and not the Equipment page, with the character's active skills and the row the cursor is on.
    pub status: bool,
    pub skills: &'a [Skill],
    pub row: usize,
    /// Frames since B on the page, while its parts go out (their slide-ins backwards).
    pub leaving: Option<f32>,
    /// The item under the Equipment page's cursor.
    pub item: Option<&'a Card>,
    /// A change of character in progress: the figure of the one left, the frames since the key and the way (+1 RT, -1 LT).
    pub swap: Option<(u8, f32, i32)>,
}

/// A node the game's code changes: shown or hidden outright, another picture or another text.
#[derive(Clone, Debug, Default)]
struct Over {
    show: bool,
    hide: bool,
    image_no: Option<usize>,
    text: Option<String>,
    /// A multiplier of the node's own opacity.
    alpha: Option<f32>,
    /// Another place for the node, in its parent's layout pixels.
    at: Option<[f32; 2]>,
    /// A text's size (layout pixels) and colour (0 to 1) where the layout names none.
    size: Option<f32>,
    color: Option<[f32; 4]>,
    /// A text's colour whatever its layout says (the game's code sets it).
    tint: Option<[f32; 4]>,
    /// The node's turn (degrees clockwise) and scale where an animation sets them.
    turn: Option<f32>,
    scale: Option<[f32; 2]>,
    /// A picture cut at this share of its width (the quality gauge).
    crop: Option<f32>,
    /// A picture's colour where an animation sets it (0 to 1).
    paint: Option<[f32; 4]>,
    /// A scroll bar's slider: its top and height in the bar's layout pixels.
    thumb: Option<[f32; 2]>,
    /// A picture at its style's natural size whatever its node says (the game's code sets it).
    natural: bool,
    /// The distance between a text's lines where the game's code sets it (layout pixels).
    step: Option<f32>,
    /// A text's colour runs: the plain string's character where each begins and its colour (the game's `<#RRGGBB>` markup).
    runs: Vec<(usize, [f32; 4])>,
    /// A picture's height in layout pixels where the game's code sets it.
    size_h: Option<f32>,
    /// The picture is one copy of an outline already (`bold_size` makes nine copies of a picture in the synthesis's screens).
    outlined: bool,
}

/// The layouts of the synthesis's Select Recipe and Add Materials screens.
struct MixDocs {
    recipe: Element,
    field: Element,
    tabs: Element,
    cabinet: Element,
    cell: Element,
    /// The screens' caption layout (`uil_a21_mix_caption.xml`: the quantity box and the help plates).
    caption: Element,
    /// The tutorial box (`a21_balloon_frame/uil_a21_balloon_frame_20x03.xml`).
    balloon: Element,
    /// The panel under the material list (`uil_a21_mix_simple_item_info.xml`: the item's quality, elements, effects and traits).
    simple: Element,
    /// The balloon select's window and cursor (`_menu_templates/uil_mt_balloon_select.xml`) and its line's text (`_line_format/uil_lf_balloon_select.xml`).
    select: Element,
    select_line: Element,
}

pub struct Page {
    styles: HashMap<String, Vec<Pic>>,
    /// The layer's textures after the fixed three: (install path, texture index in its file) and its size.
    wanted: Vec<(String, usize)>,
    sizes: Vec<(u32, u32)>,
    strings: HashMap<u32, String>,
    equip: Element,
    chara: Element,
    status: Element,
    /// The title plate's layout (`uil_a21_caption.xml`) and the party icons' (`uil_a21_menu_faceicon.xml`).
    caption: Element,
    faces: Element,
    /// The Status page's skills panel (`uil_a21_menu_skill_info.xml`).
    skill: Element,
    /// The item card (`uil_a21_item_info.xml`) and the parts it copies (`uil_a21_item_info_clone_targets.xml`).
    item: Element,
    targets: Element,
    /// The Container page's layout (`uil_a21_cabinet.xml`), its cell and its tabs (what its `template_cell` and `template_tab` load).
    cabinet: Element,
    cell: Element,
    tabs: Element,
    /// The Gathering Tools page's layout (`uil_a21_menu_explore_equip.xml`).
    explore: Element,
    /// The Guide's background and top page (`library/uil_library_00_bg.xml`, `uil_library_01_top.xml`).
    library_bg: Element,
    library_top: Element,
    /// The Guide's Item page (`uil_library_04_item_top.xml`) and its Effect/Trait page (`uil_library_08_eff_esp.xml`).
    library_items: Element,
    library_eff: Element,
    /// The synthesis's finished item card (`a21_mix/uil_a21_mix_item_sheet.xml`).
    mix_sheet: Element,
    /// The synthesis's Select Recipe and ring graph layouts (`a21_mix/uil_a21_mix_recipe.xml`, `_field.xml`), its tab row (`a21_cabinet/uil_a21_item_tab_mix.xml`), the
    /// material list's grid and cell (`uil_a21_cabinet_mix.xml`, `_cell.xml`).
    mix_docs: MixDocs,
    /// The text styles the card's texts copy (`font_template`: the item card's and the common ones), by name.
    text_styles: HashMap<String, Vec<(String, String)>>,
    glyphs: [HashMap<char, Glyph>; 2],
}

/// The style files the layouts' styles are in.
const STYLE_FILES: [&str; 17] = [
    "ui_en\\gen_styles\\uis_gen_a21_status.xml",
    "ui_en\\gen_styles\\uis_gen_a21_item_info.xml",
    "ui_en\\gen_styles\\uis_gen_a21_caption.xml",
    "ui_cmn\\etc\\uis_a21_category_image.xml",
    "ui_cmn\\gen_styles\\uis_gen_a21_btl_equip.xml",
    "ui_cmn\\gen_styles\\uis_gen_a21_common.xml",
    "ui_cmn\\gen_styles\\uis_gen_a21_icon.xml",
    "ui_cmn\\gen_styles\\uis_gen_a21_window.xml",
    "ui_cmn\\gen_styles\\uis_gen_a21v2_common.xml",
    "ui_cmn\\gen_styles\\uis_gen_a21_cabinet.xml",
    "ui_cmn\\etc\\uis_a21_item_l.xml",
    "ui_cmn\\etc\\uis_a21_chara_stand_card.xml",
    "ui_cmn\\etc\\uis_a21_bg.xml",
    "ui_en\\gen_styles\\uis_gen_a21_mix.xml",
    "ui_cmn\\etc\\uis_a21_item_s.xml",
    "ui_en\\gen_styles\\uis_gen_a21_library.xml",
    "ui_en\\gen_styles\\uis_gen_a21_balloon_frame.xml",
];

fn texture_path(texture: &str, english: bool) -> String {
    format!("\\data\\x64\\{}\\{}", if english { "res_en" } else { "res_cmn" }, texture.replace('/', "\\"))
}

impl Page {
    /// Reads the layouts and styles for `party` (the pictures and textures its members need).
    pub fn read(files: Files, party: &[crate::field_menu::Member], item_image: &dyn Fn(u16) -> usize, shelf: &[ShelfItem]) -> Result<Page, String> {
        let read = |p: &str| (files.read)(&format!("\\saves\\{p}"));
        let layout = |name: &str| -> Result<Element, String> { table::parse(&read(&format!("ui_en\\a21_main_menu\\{name}"))?).map_err(|e| format!("{name}: {e}")) };
        let (equip, chara, status) = (layout("uil_a21_menu_equip.xml")?, layout("uil_a21_menu_chara.xml")?, layout("uil_a21_menu_chara_status.xml")?);
        let faces = layout("uil_a21_menu_faceicon.xml")?;
        let skill = layout("uil_a21_menu_skill_info.xml")?;
        let caption = table::parse(&read("ui_en\\a21_caption\\uil_a21_caption.xml")?).map_err(|e| format!("uil_a21_caption: {e}"))?;
        let item = table::parse(&read("ui_en\\a21_item_info\\uil_a21_item_info.xml")?).map_err(|e| format!("item_info: {e}"))?;
        let targets = table::parse(&read("ui_en\\a21_item_info\\uil_a21_item_info_clone_targets.xml")?).map_err(|e| format!("item_info targets: {e}"))?;
        let cabinet = table::parse(&read("ui_en\\a21_cabinet\\uil_a21_cabinet.xml")?).map_err(|e| format!("cabinet: {e}"))?;
        let cell = table::parse(&read("ui_en\\a21_cabinet\\uil_a21_cabinet_cell.xml")?).map_err(|e| format!("cabinet cell: {e}"))?;
        let tabs = table::parse(&read("ui_en\\a21_cabinet\\uil_a21_item_tab.xml")?).map_err(|e| format!("cabinet tabs: {e}"))?;
        let explore = layout("uil_a21_menu_explore_equip.xml")?;
        let library = |name: &str| -> Result<Element, String> { table::parse(&read(&format!("ui_en\\library\\{name}"))?).map_err(|e| format!("{name}: {e}")) };
        let (library_bg, library_top) = (library("uil_library_00_bg.xml")?, library("uil_library_01_top.xml")?);
        let (library_items, mut library_eff) = (library("uil_library_04_item_top.xml")?, library("uil_library_08_eff_esp.xml")?);
        stars_copy_the_first(&mut library_eff);
        let mix_sheet = table::parse(&read("ui_en\\a21_mix\\uil_a21_mix_item_sheet.xml")?).map_err(|e| format!("mix item sheet: {e}"))?;
        let mix_read = |n: &str| table::parse(&read(&format!("ui_en\\a21_mix\\{n}"))?).map_err(|e| format!("{n}: {e}"));
        let cab_read = |n: &str| table::parse(&read(&format!("ui_en\\a21_cabinet\\{n}"))?).map_err(|e| format!("{n}: {e}"));
        let mix_docs = MixDocs { recipe: mix_read("uil_a21_mix_recipe.xml")?, field: mix_read("uil_a21_mix_field.xml")?, tabs: cab_read("uil_a21_item_tab_mix.xml")?, cabinet: cab_read("uil_a21_cabinet_mix.xml")?, cell: cab_read("uil_a21_cabinet_mix_cell.xml")?, caption: mix_read("uil_a21_mix_caption.xml")?, balloon: table::parse(&read("ui_en\\a21_balloon_frame\\uil_a21_balloon_frame_20x03.xml")?).map_err(|e| format!("balloon: {e}"))?, simple: mix_read("uil_a21_mix_simple_item_info.xml")?, select: table::parse(&read("ui_en\\_menu_templates\\uil_mt_balloon_select.xml")?).map_err(|e| format!("balloon select: {e}"))?, select_line: table::parse(&read("ui_en\\_line_format\\uil_lf_balloon_select.xml")?).map_err(|e| format!("balloon select line: {e}"))? };
        let mut text_styles = HashMap::new();
        for f in ["ui_cmn\\uil_template.xml", "ui_en\\a21_item_info\\uil_a21_item_info_template.xml"] {
            for t in table::parse(&read(f)?).map_err(|e| format!("{f}: {e}"))?.children {
                if let Some(name) = t.attr("name") {
                    text_styles.insert(name.to_string(), t.attributes.clone());
                }
            }
        }
        let mut styles: HashMap<String, Vec<Pic>> = HashMap::new();
        for f in STYLE_FILES {
            let root = table::parse(&read(f)?).map_err(|e| format!("{f}: {e}"))?;
            // A style file's `resolution_hd` says which screen height its pixels are for: 2160 (the default) or 1080.
            let (per_layout, over_texture) = if root.attr("resolution_hd").map(str::trim) == Some("1080") { (PER_LAYOUT / 2.0, 1.0) } else { (PER_LAYOUT, STYLE_OVER_TEXTURE) };
            let pic = |e: &Element| -> Option<Pic> {
                let uvwh = numbers(e, "uvwh").try_into().ok()?;
                let edges = (e.attr("prim_type") == Some("wrap")).then(|| numbers(e, "edge_margin")).and_then(|m| m.try_into().ok());
                Some(Pic { path: texture_path(e.attr("texture")?, f.starts_with("ui_en")), index: e.attr("texture_index")?.trim().parse().ok()?, uvwh, per_layout, over_texture, edges })
            };
            for e in &root.children {
                let Some(name) = e.attr("name") else { continue };
                let list: Vec<Pic> = if e.name == "image_list" { e.children.iter().filter_map(pic).collect() } else { pic(e).into_iter().collect() };
                styles.insert(name.to_string(), list);
            }
        }
        // The textures: every style a layout names (a list's every picture, except the long lists of items and figures:
        // only the party's).
        let mut used = BTreeSet::new();
        for doc in [&equip, &chara, &status, &caption, &faces, &skill, &item, &targets, &cabinet, &cell, &tabs, &explore, &library_bg, &library_top, &library_items, &library_eff, &mix_sheet, &mix_docs.recipe, &mix_docs.field, &mix_docs.tabs, &mix_docs.cabinet, &mix_docs.cell, &mix_docs.caption, &mix_docs.balloon, &mix_docs.simple, &mix_docs.select, &mix_docs.select_line] {
            collect_styles(doc, &mut used);
        }
        let mut wanted: BTreeSet<(String, usize)> = BTreeSet::new();
        for name in &used {
            if name == "etc_a21_item_l" || name == "etc_a21_chara_stand_card" || name == "etc_a21_category_image" || name == "etc_a21_item_s" {
                continue;
            }
            for p in styles.get(name).into_iter().flatten() {
                wanted.insert((p.path.clone(), p.index));
            }
        }
        for m in party {
            if let Some(p) = styles.get("etc_a21_chara_stand_card").and_then(|l| l.get(usize::from(m.id))) {
                wanted.insert((p.path.clone(), p.index));
            }
            for w in m.worn.iter().flatten() {
                if let Some(p) = styles.get("etc_a21_item_l").and_then(|l| l.get(item_image(w.item))) {
                    wanted.insert((p.path.clone(), p.index));
                }
            }
        }
        // The Container page's items: the grid's small picture, the card's big one and its category's.
        for it in shelf {
            for (style, n) in [("etc_a21_item_s", Some(it.image)), ("etc_a21_item_l", Some(it.image)), ("etc_a21_category_image", it.category)] {
                if let Some(p) = n.and_then(|n| styles.get(style).and_then(|l| l.get(n))) {
                    wanted.insert((p.path.clone(), p.index));
                }
            }
        }
        wanted.insert((texture_path("ui/a21_common.g1t", false), HELP_PLATE));
        wanted.insert((texture_path("ui/sys_alpha_gradation.g1t", false), 0));
        wanted.insert((texture_path("ui/sys_no_image.g1t", false), 0));
        wanted.insert((texture_path("ui/sys_alpha_circle.g1t", false), 0));
        let wanted: Vec<(String, usize)> = wanted.into_iter().collect();
        let sizes = wanted
            .iter()
            .map(|(p, i)| {
                let bytes = (files.read)(p)?;
                let g1t = barrel_formats::g1t::G1t::parse(&bytes).map_err(|e| format!("{p}: {e}"))?;
                let t = g1t.textures.get(*i).ok_or(format!("{p}: no texture {i}"))?;
                Ok((t.width, t.height))
            })
            .collect::<Result<Vec<_>, String>>()?;
        // The gradation the layouts call `alpha_gradation` is the whole of `ui/sys_alpha_gradation.g1t`.
        if let Some(slot) = wanted.iter().position(|(p, _)| p.ends_with("sys_alpha_gradation.g1t")) {
            let (tw, th) = sizes[slot];
            styles.insert("alpha_gradation".to_string(), vec![Pic { path: wanted[slot].0.clone(), index: 0, uvwh: [0.0, 0.0, tw as f32 * PER_LAYOUT, th as f32 * PER_LAYOUT], per_layout: PER_LAYOUT, over_texture: PER_LAYOUT, edges: None }]);
        }
        if let Some(slot) = wanted.iter().position(|(p, _)| p.ends_with("sys_alpha_circle.g1t")) {
            let (tw, th) = sizes[slot];
            styles.insert("alpha_circle".to_string(), vec![Pic { path: wanted[slot].0.clone(), index: 0, uvwh: [0.0, 0.0, tw as f32 * PER_LAYOUT, th as f32 * PER_LAYOUT], per_layout: PER_LAYOUT, over_texture: PER_LAYOUT, edges: None }]);
        }
        let strings_root = table::parse(&read("text_en\\strcombineall.xml")?).map_err(|e| format!("strcombineall: {e}"))?;
        let strings = strings_root
            .children
            .iter()
            .filter_map(|s| Some((s.attr("String_No")?.parse::<u32>().ok()?, s.attr("Text")?.to_string())))
            .filter(|(n, _)| {
                (STR_UI_BASE..STR_UI_BASE + 1000).contains(n)
                    || (262_148..262_188).contains(n)
                    || *n == EQUIPMENT_TITLE
                    || *n == STATUS_TITLE
                    || *n == COMPLETE_TITLE
                    || *n == prompt_view::OK_STRING
                    || (ACTION_NAME..ACTION_NAME + 1000).contains(n)
                    || (ACTION_COMMENT..ACTION_COMMENT + 1000).contains(n)
                    || (NAME_BASE..NAME_BASE + 1000).contains(n)
                    || (EFFECT_BASE..EFFECT_BASE + 2000).contains(n)
                    || (CATEGORY_BASE..CATEGORY_BASE + 100).contains(n)
                    || (POTENTIAL_BASE..POTENTIAL_BASE + 400).contains(n)
                    || SHELF_STRINGS.contains(n)
                    || (TAB_NAMES..TAB_NAMES + 8).contains(n)
                    || (LIBRARY_BASE..LIBRARY_BASE + LIBRARY_STRINGS).contains(n)
                    || *n == crate::library::UNKNOWN_NAME
                    || (3_538_945..3_538_945 + 2000).contains(n)
                    || (9_961_000..9_962_000).contains(n)
                    || (10_092_000..10_093_000).contains(n)
                    || (10_027_000..10_028_000).contains(n)
            })
            .collect();
        Ok(Page { styles, wanted, sizes, strings, equip, chara, status, caption, faces, skill, item, targets, cabinet, cell, tabs, explore, library_bg, library_top, library_items, library_eff, mix_sheet, mix_docs, text_styles, glyphs: files.glyphs })
    }

    /// The layer's texture of the help bar's plate (the menu's GUIDE).
    pub fn help_texture(&self) -> usize {
        self.wanted.iter().position(|(p, i)| p.ends_with("a21_common.g1t") && *i == HELP_PLATE).map_or(WHITE, |i| FIXED + i)
    }

    /// The layer's texture of the screens' bars' gradation (`ui/sys_alpha_gradation.g1t`).
    /// The layer's texture of the stencil masks' placeholder picture (`ui/sys_no_image.g1t`).
    fn no_image_texture(&self) -> usize {
        self.wanted.iter().position(|(p, _)| p.ends_with("sys_no_image.g1t")).map_or(WHITE, |i| FIXED + i)
    }

    fn gradation_texture(&self) -> usize {
        self.wanted.iter().position(|(p, _)| p.ends_with("sys_alpha_gradation.g1t")).map_or(WHITE, |i| FIXED + i)
    }

    /// The layer's textures after the fonts and the white: (install path, texture index).
    pub fn wanted(&self) -> &[(String, usize)] {
        &self.wanted
    }

    /// The page's quads in draw order on a `screen` of pixels, and which of them are drawn added to what is under them.
    pub fn quads(&self, look: &Look, screen: (u32, u32)) -> (Vec<Quad>, Vec<usize>, Vec<Clip>) {
        let k = screen.1 as f32 / LAYOUT_HEIGHT;
        let mut w = Walker { page: self, screen_k: k, out: Vec::new(), additive: Vec::new(), parent: [0.0; 2], card: false, ordinal: 0, mask: None, clips: Vec::new(), mix: false, bold_scale: 1.0, run_base: 0 };
        let root = Xf { a: k, b: 0.0, c: 0.0, d: k, x: (screen.0 as f32 - 1280.0 * k) / 2.0, y: 0.0 };
        // Frames since A on the card: the Status page's parts come in by it (the Equipment page's are whole by the time it is shown).
        let clock = look.age;
        // (leaving: the slide-in played backwards, the part's own delay counted from B)
        let into = |delay: f32| look.leaving.map_or(clock - delay, |out| SLIDE_FRAMES - (out - delay).max(0.0));
        // (the Equipment page's parts are whole when it is shown, and slide only on the way out)
        let moves = look.status || look.leaving.is_some();
        let slide = |delay: f32, from: [f32; 2]| if moves { Slide::in_from(into(delay), from) } else { Slide::NONE };
        let name = self.strings.get(&(262_148 + u32::from(look.member.id))).cloned().unwrap_or_default();

        // The character's figure (`uil_a21_menu_chara.xml`'s `chara_image_1`, whose place the equip layout sets: `chara_image_pos`;
        // the Status page's layout puts it in the middle, with the name plate under it).
        let figure = Over { show: true, image_no: Some(usize::from(look.member.id)), ..Default::default() };
        if look.status {
            let at = find_pos(&self.status, "chara_image_pos").unwrap_or([0.0; 2]);
            if let Some(top) = find(&self.chara, "chara") {
                let xf = root.node(at, 0.0, [1.0; 2]);
                let parts = |plate: bool| {
                    let (figure, name) = (figure.clone(), name.clone());
                    move |path: &[&str]| -> Over {
                        match path {
                            [_, "chara_image_1"] if !plate => figure.clone(),
                            [_, "chara_image_1"] => Over { hide: true, ..Default::default() },
                            [_, "name_node"] if !plate => Over { hide: true, ..Default::default() },
                            [_, "name_node"] | [_, "name_node", "name_fl"] => Over { show: true, ..Default::default() },
                            [_, "name_node", "name_fl", "text_1"] => Over { text: Some(name.clone()), ..Default::default() },
                            _ => Over::default(),
                        }
                    }
                };
                w.slid(slide(0.0, [0.0, 30.0]), |w| w.walk(top, xf, &mut Vec::new(), &parts(false), &HashMap::new()));
                w.slid(slide(NAME_DELAY, [0.0, 30.0]), |w| w.walk(top, xf, &mut Vec::new(), &parts(true), &HashMap::new()));
            }
        } else {
            let at = find_pos(&self.equip, "chara_image_pos").unwrap_or([0.0; 2]);
            if let Some(e) = find(&self.chara, "chara_image_1") {
                let xf = root.node(at, 0.0, [1.0; 2]);
                // The character change (`anm_chara_change_out` and `anm_chara_in`, 24 frames; RT uses the `_rev` pair): the old figure
                // fades out linearly and goes 30 pixels along by `easing_insine`, the new fades in and comes from 30 back by `easing_outsine`.
                if let Some((from, frames, way)) = look.swap.filter(|&(_, frames, _)| frames < CHANGE_FRAMES) {
                    let t = frames / CHANGE_FRAMES;
                    let (old, new) = (Over { show: true, image_no: Some(usize::from(from)), ..Default::default() }, 30.0 * way as f32);
                    w.slid(Slide { alpha: 1.0 - t, dx: new * (1.0 - (t * std::f32::consts::FRAC_PI_2).cos()), dy: 0.0 }, |w| w.image(e, xf, &old));
                    w.slid(Slide { alpha: t, dx: -new * (1.0 - (t * std::f32::consts::FRAC_PI_2).sin()), dy: 0.0 }, |w| w.slid(slide(0.0, [-40.0, 0.0]), |w| w.image(e, xf, &figure)));
                } else {
                    w.slid(slide(0.0, [-40.0, 0.0]), |w| w.image(e, xf, &figure));
                }
            }
        }

        // The Status page's skills: the tab row, the list in its window and the description of the skill under the cursor
        // (`uil_a21_menu_skill_info.xml`), sliding in from the left.
        if look.status {
            if let Some(top) = find(&self.skill, "top") {
                let origin = root.node(pair(top, "pos", [0.0; 2]), 0.0, [1.0; 2]);
                w.slid(slide(0.0, [-40.0, 0.0]), |w| {
                    if let Some(tabs) = find(&self.skill, "skill_select") {
                        let tabs_over = |path: &[&str]| -> Over {
                            match path {
                                [_, "menu", _] => Over { hide: true, ..Default::default() },
                                // The tabs' shoulder-button glyphs (the capture's: the game picks them by the pad's assignment).
                                [.., "l_bt", "select_L"] => Over { text: Some(TAB_LEFT_BUTTON.to_string()), size: Some(BUTTON_SIZE), color: Some([1.0; 4]), ..Default::default() },
                                [.., "r_bt", "select_R"] => Over { text: Some(TAB_RIGHT_BUTTON.to_string()), size: Some(BUTTON_SIZE), color: Some([1.0; 4]), ..Default::default() },
                                _ => Over::default(),
                            }
                        };
                        w.walk(tabs, origin.node(pair(tabs, "pos", [0.0; 2]), 0.0, [1.0; 2]), &mut Vec::new(), &tabs_over, &HashMap::new());
                        if let Some(list) = find(&self.skill, "menu_template") {
                            let rows = |path: &[&str]| self.skill_row(look, clock, path);
                            // (the menu's own place is the window's: the template's `pos` is for the layout viewer)
                            w.walk(list, origin.node(pair(tabs, "pos", [0.0; 2]), 0.0, [1.0; 2]), &mut Vec::new(), &rows, &HashMap::new());
                        }
                    }
                    if let Some(desc) = find(&self.skill, "skill_description") {
                        let comment = look.skills.get(look.row).and_then(|s| self.strings.get(&s.comment)).cloned();
                        let text = |path: &[&str]| -> Over {
                            match path {
                                [_] => Over { show: true, ..Default::default() },
                                [_, "desc"] => Over { text: comment.clone(), ..Default::default() },
                                _ => Over::default(),
                            }
                        };
                        w.walk(desc, origin.node(pair(desc, "pos", [0.0; 2]), 0.0, [1.0; 2]), &mut Vec::new(), &text, &HashMap::new());
                    }
                });
            }
        }

        // The four slots (`equip/battle`, shown by the game's code), the cursor's on `look.slot`.
        let names = ["weapon", "armor", "accessory1", "accessory2"];
        if let Some(top) = find(&self.equip, "equip").filter(|_| !look.status) {
            let slots = |path: &[&str]| -> Over {
                if let [.., "hilight", "__uimenu_cursor_active"] = path {
                    return Over { alpha: Some(blink(look.slot_age)), ..Default::default() };
                }
                let (slot, part) = match path {
                    [.., s, p] => (names.iter().position(|n| n == s), *p),
                    _ => (None, ""),
                };
                let Some(slot) = slot else { return Over { show: path.last() == Some(&"battle"), ..Default::default() } };
                match part {
                    "hilight" => Over { show: slot == look.slot, ..Default::default() },
                    "item" => match look.member.worn[slot] {
                        Some(worn) => Over { image_no: Some((look.item_images)(worn.item)), ..Default::default() },
                        None => Over { hide: true, ..Default::default() },
                    },
                    _ => Over::default(),
                }
            };
            // (going out, the slots fade linearly and sink 20 pixels, in sine: read off 2510 and 2520)
            let gone = look.leaving.map_or(Slide::NONE, Slide::slots_out);
            w.slid(gone, |w| w.walk(top, root.node(pair(top, "pos", [0.0; 2]), 0.0, [1.0; 2]), &mut Vec::new(), &slots, &HashMap::new()));
        }

        // The item card of the slot under the cursor, at the item-info place of the equip layout (`item_info_pos` and its battle child).
        if let Some(card) = look.item.filter(|_| !look.status) {
            let at = [find_pos(&self.equip, "item_info_pos").unwrap_or([0.0; 2]), find_pos(&self.equip, "item_info_pos_battle").unwrap_or([0.0; 2])];
            self.item_card(&mut w, &root, "equip_arms", Some([at[0][0] + at[1][0], at[0][1] + at[1][1]]), card, look.party);
        }

        // The stat panel (`uil_a21_menu_chara_status.xml`'s `top`, shown by the game's code), sliding in from the right.
        if let Some(top) = find(&self.status, "top") {
            let s = look.member.stats;
            let values = [("lv", look.member.level.to_string()), ("hp", s.hp.to_string()), ("atk", s.attack.to_string()), ("def", s.defense.to_string()), ("spd", s.speed.to_string()), ("all", s.equip_quality.to_string()), ("dex", s.dexterity.to_string())];
            let panel = |path: &[&str]| -> Over {
                if path.len() == 1 {
                    return Over { show: true, ..Default::default() };
                }
                // Only the numbers and the labels, the name, the lines and the window.
                match path {
                    [.., "param", "lv", "value"] => Over { text: Some(look.member.level.to_string()), ..Default::default() },
                    [.., "hp", "parent", "current", "value"] => Over { text: Some(s.hp.to_string()), ..Default::default() },
                    [.., "param", "hp", "parent", "max", "now"] => Over { show: true, text: Some(s.hp.to_string()), ..Default::default() },
                    [.., "param", stat, "now"] => Over { text: values.iter().find(|(n, _)| n == stat).map(|(_, v)| v.clone()), ..Default::default() },
                    [.., "change"] | [.., "role"] | [.., "role_info"] | [.., "clone_targets"] | [.., "change_template"] | [.., "updown"] => Over { hide: true, ..Default::default() },
                    // The element resists: a picture each (the "off" one and the arrows are for a resist the game's code sets).
                    [.., "att_d", "resist", "list", _, "icon" | "resist_state_icon"] => Over { hide: true, ..Default::default() },
                    ["top", "status", "text"] => Over { text: Some(name.clone()), ..Default::default() },
                    _ => Over::default(),
                }
            };
            w.slid(slide(0.0, [40.0, 0.0]), |w| w.walk(top, root.node(pair(top, "pos", [0.0; 2]), 0.0, [1.0; 2]), &mut Vec::new(), &panel, &HashMap::new()));
        }

        // The party's icons, with the shoulder buttons' glyphs at their ends (`uil_a21_menu_faceicon.xml`). The icons fill
        // the last places of the nine (STAND-IN: the capture's party of three is at the 7th to 9th); the left button's node
        // goes to the first icon's place.
        if let Some(top) = find(&self.faces, "top") {
            let n = look.party.len().min(ICON_PLACES);
            let first = ICON_PLACES - n;
            let place = |name: &str| name.strip_prefix("face_icon").and_then(|i| i.parse::<usize>().ok()).and_then(|i| i.checked_sub(1 + first));
            let first_x = find_pos(&self.faces, &format!("face_icon{:02}", first + 1)).map_or(0.0, |p| p[0]);
            // The icons' animations run from A on the card (`anm_a21_menu_faceicon_rotation` and `_expandEff`, looping).
            let eff = (clock - EFFECT_LAG + look.leaving.map_or(0.0, |_| EFFECT_AFTER_B)).rem_euclid(EFFECT_LOOP).min(EFFECT_FRAMES);
            let glow = if eff < EFFECT_PEAK { eff / EFFECT_PEAK } else { (EFFECT_FRAMES - eff) / (EFFECT_FRAMES - EFFECT_PEAK) } * EFFECT_ALPHA;
            let glow = (glow * 255.0).round() / 255.0;
            let icons = |path: &[&str]| -> Over {
                match path {
                    ["top", "icon", icon] if place(icon).is_none_or(|i| i >= n) => Over { hide: true, ..Default::default() },
                    ["top", "icon", icon, "hilight"] => Over { show: place(icon) == Some(look.card), turn: Some(clock.rem_euclid(ROTATION_FRAMES) * 360.0 / ROTATION_FRAMES), ..Default::default() },
                    ["top", "icon", _, "hilight", "hilight_eff"] => Over { scale: Some([1.0 + 0.2 * (1.0 - (-10.0 * eff / EFFECT_FRAMES).exp2()); 2]), alpha: Some(glow), ..Default::default() },
                    ["top", "icon", icon, "face"] => Over { image_no: place(icon).and_then(|i| look.party.get(i)).map(|m| usize::from(m.id)), ..Default::default() },
                    ["top", "btn", "L2_node"] => Over { at: Some([first_x, 16.0]), ..Default::default() },
                    ["top", "btn", "L2_node", "select_L"] => Over { text: Some(LEFT_BUTTON.to_string()), size: Some(BUTTON_SIZE), color: Some([1.0; 4]), ..Default::default() },
                    ["top", "btn", "select_R"] => Over { text: Some(RIGHT_BUTTON.to_string()), size: Some(BUTTON_SIZE), color: Some([1.0; 4]), ..Default::default() },
                    [_] => Over { show: true, ..Default::default() },
                    _ => Over::default(),
                }
            };
            // (`anm_common_juvenile_fade_in` at twice its speed: the cube of the time over 30 frames)
            let fade = if moves { Slide { alpha: (into(0.0) / FACE_FADE_FRAMES).clamp(0.0, 1.0).powi(3), dx: 0.0, dy: 0.0 } } else { Slide::NONE };
            w.slid(fade, |w| w.walk(top, root.node([0.0; 2], 0.0, [1.0; 2]), &mut Vec::new(), &icons, &HashMap::new()));
        }

        // The title plate (`uil_a21_caption.xml`), sliding in from above.
        self.caption_plate(&mut w, &root, if look.status { STATUS_TITLE } else { EQUIPMENT_TITLE }, slide(0.0, [0.0, -30.0]));
        (w.out, w.additive, w.clips)
    }

    /// The title plate with string `title`, moved by `slide`.
    fn caption_plate(&self, w: &mut Walker, root: &Xf, title: u32, slide: Slide) {
        let Some(top) = find(&self.caption, "top") else { return };
        let title = self.strings.get(&title).cloned();
        let plate = |path: &[&str]| -> Over {
            match path {
                ["top"] => Over { show: true, ..Default::default() },
                [.., "caption_text"] => Over { text: title.clone(), ..Default::default() },
                _ => Over::default(),
            }
        };
        w.slid(slide, |w| w.walk(top, root.node([0.0; 2], 0.0, [1.0; 2]), &mut Vec::new(), &plate, &HashMap::new()));
    }

    /// The Gathering Tools page: the row of tools in its frame (`uil_a21_menu_explore_equip.xml`'s `top`, sliding in from the left), the cursor
    /// and the card of the tool under it (from the right) and the title plate.
    pub fn tools_quads(&self, look: &ToolsLook, screen: (u32, u32)) -> (Vec<Quad>, Vec<usize>, Vec<Clip>) {
        let k = screen.1 as f32 / LAYOUT_HEIGHT;
        let mut w = Walker { page: self, screen_k: k, out: Vec::new(), additive: Vec::new(), parent: [0.0; 2], card: true, ordinal: 0, mask: None, clips: Vec::new(), mix: false, bold_scale: 1.0, run_base: 0 };
        let root = Xf { a: k, b: 0.0, c: 0.0, d: k, x: (screen.0 as f32 - 1280.0 * k) / 2.0, y: 0.0 };
        let into = |total: f32| look.leaving.map_or(look.age, |out| total - out);
        let (base, add, f) = cursor_colors(look.age);
        let over = |path: &[&str]| -> Over {
            match path {
                ["top"] => Over { show: true, ..Default::default() },
                ["top", "cell_root", "row_0", cell, part, ..] => {
                    let k: usize = cell.strip_prefix("cell_").and_then(|n| n.parse().ok()).unwrap_or(0);
                    let here = k < look.images.len();
                    match (*part, path.len()) {
                        ("item_image", 5) if here => Over { show: true, image_no: Some(look.images[k]), ..Default::default() },
                        ("highlight", 5) if k == look.cursor && here => Over { show: true, paint: Some([base[0], base[1], base[2], 1.0]), ..Default::default() },
                        ("highlight", 6) => Over { paint: Some([add[0], add[1], add[2], (191.0 * f).round() / 255.0]), ..Default::default() },
                        _ => Over { hide: true, ..Default::default() },
                    }
                }
                _ => Over::default(),
            }
        };
        if let Some(top) = find(&self.explore, "top") {
            let at = pair(top, "pos", [0.0; 2]);
            w.slid(Slide::in_from(into(SLIDE_FRAMES), [-40.0, 0.0]), |w| w.walk(top, root.node(at, 0.0, [1.0; 2]), &mut Vec::new(), &over, &HashMap::new()));
        }
        if let Some(card) = look.item {
            w.slid(Slide::in_from(into(SLIDE_FRAMES), [40.0, 0.0]), |w| self.item_card(w, &root, "normal_item", None, card, look.party));
        }
        // (the title plate goes a frame and a tenth ahead of the other parts: `field-menu-pages` 2740, 0.949 of its opacity to their 0.965)
        let title_in = look.leaving.map_or(look.plate_age, |out| SLIDE_FRAMES - out - TOOLS_TITLE_EARLY);
        self.caption_plate(&mut w, &root, TOOLS_TITLE, Slide::in_from(title_in, [0.0, -30.0]));
        (w.out, w.additive, w.clips)
    }

    /// The Guide page: the paper and its picture cards (`uil_library_00_bg.xml` and the top's own picture), the title, the four buttons with the
    /// cursor's highlight and, going out, the fades in the reverse order.
    pub fn guide_quads(&self, look: &GuideLook, screen: (u32, u32)) -> (Vec<Quad>, Vec<usize>, Vec<Clip>) {
        // (the top while a sub-page is up or coming back: out over 15 frames from A, in over 15 from 15 frames after B)
        let top_alpha = look.sub.as_ref().map(|s| match s.back {
            Some(b) => ((b - GUIDE_FADE) / GUIDE_FADE).clamp(0.0, 1.0),
            None => 1.0 - (s.since / GUIDE_FADE).clamp(0.0, 1.0),
        });
        let k = screen.1 as f32 / LAYOUT_HEIGHT;
        let mut w = Walker { page: self, screen_k: k, out: Vec::new(), additive: Vec::new(), parent: [0.0; 2], card: true, ordinal: 0, mask: None, clips: Vec::new(), mix: false, bold_scale: 1.0, run_base: 0 };
        let root = Xf { a: k, b: 0.0, c: 0.0, d: k, x: (screen.0 as f32 - 1280.0 * k) / 2.0, y: 0.0 };
        let fade = |frames: f32| (frames / GUIDE_FADE).clamp(0.0, 1.0);
        let (back, top) = match look.leaving {
            Some(out) => (1.0 - fade(out - GUIDE_FADE), 1.0 - fade(out)),
            None => (fade(look.age - 1.0), top_alpha.unwrap_or(fade(look.age - 1.0).powi(2))),
        };
        // The highlight's glow over the cursor's button blinks (`anm_common_blink`: whole to none and back in 120 frames) at 125/255.
        let t = look.moved.unwrap_or(look.age - 1.0).rem_euclid(120.0);
        let blink = if t < 60.0 { 1.0 - t / 60.0 } else { (t - 60.0) / 60.0 };
        let over = |path: &[&str]| -> Over {
            match path {
                ["top"] => Over { show: true, ..Default::default() },
                ["top", "node_menus", _, "name"] => Over { size: Some(GUIDE_TEXT_SIZE), ..Default::default() },
                ["top", "node_menus", item, "hilight", rest @ ..] => {
                    let mine = look.cursor == ["menu_item", "menu_item_1", "menu_item_2", "menu_item_3"].iter().position(|n| n == item).unwrap_or(usize::MAX);
                    match rest {
                        [] if mine => Over::default(),
                        [_] if mine => Over { paint: Some([1.0, 1.0, 1.0, (125.0 * blink).floor() / 255.0]), ..Default::default() },
                        _ => Over { hide: true, ..Default::default() },
                    }
                }
                _ => Over::default(),
            }
        };
        for (doc, alpha) in [(&self.library_bg, back), (&self.library_top, top)] {
            if let Some(node) = find(doc, "top") {
                w.slid(Slide { alpha, dx: 0.0, dy: 0.0 }, |w| w.walk(node, root.node([0.0; 2], 0.0, [1.0; 2]), &mut Vec::new(), &over, &HashMap::new()));
            }
        }
        if let Some(sub) = &look.sub {
            let alpha = match sub.back {
                Some(b) => 1.0 - fade(b),
                None => fade(sub.since - SUB_AT),
            };
            match &sub.page {
                SubPage::Items(cards) => self.guide_items(&mut w, &root, sub, cards, alpha),
                SubPage::Effects(effects) => self.guide_effects(&mut w, &root, sub, effects, alpha),
            }
        }
        (w.out, w.additive, w.clips)
    }

    /// The Item page (`uil_library_04_item_top.xml`): the title, the four category tabs, the Gather Rate, the cards (ten places, the cursor's lit) and the page number.
    fn guide_items(&self, w: &mut Walker, root: &Xf, sub: &SubLook, cards: &[crate::library::Card], alpha: f32) {
        let age = sub.since - SUB_AT;
        let t = ((sub.clock - SUB_AT - 1.0) * BLINK_RATE).rem_euclid(120.0);
        let blink = if t < 60.0 { 1.0 - t / 60.0 } else { (t - 60.0) / 60.0 };
        let string = |n: u32| self.strings.get(&n).cloned();
        let tab = ["tab_clone", "tab_clone_1", "tab_clone_2", "tab_clone_3"];
        let over = |path: &[&str]| -> Over {
            match path {
                ["top"] => Over { show: true, ..Default::default() },
                ["top", "category", t, "node_on"] if *t == tab[0] => Over { show: true, ..Default::default() },
                ["top", "category", "page_change", "l1", "select_L"] => Over { text: Some(TAB_LEFT_BUTTON.to_string()), size: Some(BUTTON_SIZE), color: Some([1.0; 4]), ..Default::default() },
                ["top", "category", "page_change", "r1", "select_R"] => Over { text: Some(TAB_RIGHT_BUTTON.to_string()), size: Some(BUTTON_SIZE), color: Some([1.0; 4]), ..Default::default() },
                ["top", "node", "collect_per"] => Over { text: Some(sub.rate.to_string()), ..Default::default() },
                ["top", "node_page", "pagebg", "text_page" | "text_maxpage"] => Over { text: Some("1".into()), ..Default::default() },
                ["top", "node_item_top_body", item, rest @ ..] => {
                    let k: usize = item.strip_prefix("node_item").and_then(|n| n.parse().ok()).unwrap_or(usize::MAX);
                    let Some(card) = cards.get(k) else { return Over { hide: true, ..Default::default() } };
                    match rest {
                        ["node_center"] => {
                            let (turn, squash) = CARD_TILTS[k % CARD_TILTS.len()];
                            Over { turn: Some(turn), scale: Some([1.0, squash]), ..Default::default() }
                        }
                        ["node_center", "node_adjust_scaling", "node_hl"] => Over { show: k == sub.cursor, ..Default::default() },
                        ["node_center", "node_adjust_scaling", "node_hl", "img_hl"] => Over { paint: Some([1.0, 1.0, 1.0, (120.0 * blink).round() / 255.0]), ..Default::default() },
                        ["win", "tag", "text_title"] => Over { text: string(if card.revealed { card.name } else { crate::library::UNKNOWN_NAME }), ..Default::default() },
                        ["item", "img_item"] => Over { image_no: Some(card.image), paint: (!card.revealed).then_some(SILHOUETTE), ..Default::default() },
                        ["item", "text_level"] => Over { text: Some(card.level.to_string()), ..Default::default() },
                        _ => Over::default(),
                    }
                }
                _ => Over::default(),
            }
        };
        if let Some(node) = find(&self.library_items, "top") {
            w.slid(Slide { alpha, dx: 0.0, dy: 0.0 }, |w| w.walk(node, root.node(pair(&self.library_items, "pos", [0.0; 2]), 0.0, [1.0; 2]), &mut Vec::new(), &over, &HashMap::new()));
        }
    }

    /// The Effect/Trait page (`uil_library_08_eff_esp.xml`): the title, the Effects and Traits tabs (Effects lit), the Gather Rate, the list of the known
    /// effects in two columns with the cursor's lit, the cursor's effect (name, stars, description) and the empty "Resulting Item" frame.
    fn guide_effects(&self, w: &mut Walker, root: &Xf, sub: &SubLook, effects: &[crate::library::Effect], alpha: f32) {
        let age = sub.since - SUB_AT;
        let t = ((sub.clock - SUB_AT - 1.0) * BLINK_RATE).rem_euclid(120.0);
        let blink = if t < 60.0 { 1.0 - t / 60.0 } else { (t - 60.0) / 60.0 };
        let string = |n: u32| self.strings.get(&n).cloned();
        let here = effects.get(sub.cursor);
        let over = |path: &[&str]| -> Over {
            match path {
                ["node_top"] => Over { show: true, ..Default::default() },
                ["node_top", "effesp_tab", "tab_clone_1", "on"] => Over { hide: true, ..Default::default() },
                ["node_top", "effesp_tab", "page_change", "l1", "select_L"] => Over { text: Some(TAB_LEFT_BUTTON.to_string()), size: Some(BUTTON_SIZE), color: Some([1.0; 4]), ..Default::default() },
                ["node_top", "effesp_tab", "page_change", "r1", "select_R"] => Over { text: Some(TAB_RIGHT_BUTTON.to_string()), size: Some(BUTTON_SIZE), color: Some([1.0; 4]), ..Default::default() },
                ["node_top", "node_collect_per", "collect_per"] => Over { text: Some(sub.rate.to_string()), ..Default::default() },
                ["node_top", "node_page", "pagebg", "text_page" | "text_maxpage"] => Over { text: Some("1".into()), ..Default::default() },
                ["node_top", "node_left", "select01_template", "menu_template", "scroll_bar"] => Over { thumb: Some(EFFECT_THUMB), ..Default::default() },
                ["node_top", "node_left", "select01_template", "menu_template", "selector_item_list", line, rest @ ..] => {
                    let k: usize = line.strip_prefix("clone_line_format").and_then(|n| n.parse::<usize>().ok()).map_or(usize::MAX, |n| n - 1);
                    let Some(effect) = effects.get(k) else { return Over { hide: true, ..Default::default() } };
                    match rest {
                        ["menu_cursor"] => Over { show: k == sub.cursor, ..Default::default() },
                        ["menu_cursor_active"] => Over { show: k == sub.cursor, paint: Some([1.0, 1.0, 1.0, (125.0 * blink).round() / 255.0]), ..Default::default() },
                        ["", "name"] => Over { text: string(effect.name), ..Default::default() },
                        _ => Over::default(),
                    }
                }
                ["node_top", "node_right", "node_effesp", "node_memo", rest @ ..] => match (rest, here) {
                    (["effect_name"], Some(e)) => Over { text: string(e.name), ..Default::default() },
                    (["effect_text"], Some(e)) => Over { text: string(e.text), ..Default::default() },
                    (["effect_text", "star", clone, "on"], Some(e)) => {
                        let n: usize = clone.strip_prefix("clone_star_").and_then(|n| n.parse().ok()).unwrap_or(0);
                        Over { show: n < usize::from(e.stars), hide: n >= usize::from(e.stars), ..Default::default() }
                    }
                    _ => Over::default(),
                },
                ["node_top", "node_right", "node_effesp", "node_eff"] => Over { show: true, ..Default::default() },
                // (no resulting items are listed: the places stand empty)
                ["node_top", "node_right", "node_effesp", "node_eff", "image_drops_bg", "node_items"] => Over { hide: true, ..Default::default() },
                // (the blanks' copies beside the first are not drawn at their places: the game squashes them to nothing, three zero-width pieces a row, which Barrel leaves out)
                ["node_top", "node_right", "node_effesp", "node_eff", "image_drops_bg", "blank", n] if n.starts_with("blank_1") || n.starts_with("blank_2") => Over { hide: true, ..Default::default() },
                _ => Over::default(),
            }
        };
        if let Some(node) = find(&self.library_eff, "node_top") {
            w.slid(Slide { alpha, dx: 0.0, dy: 0.0 }, |w| w.walk(node, root.node([0.0; 2], 0.0, [1.0; 2]), &mut Vec::new(), &over, &HashMap::new()));
        }
    }

    /// The Container page: the tab row, the grid of the items, the count of free places, the card of the item under the cursor and the title
    /// plate (`uil_a21_cabinet.xml`'s `default`, which slides in from the left over 40 frames, and the card from the right).
    pub fn shelf_quads(&self, look: &ShelfLook, screen: (u32, u32)) -> (Vec<Quad>, Vec<usize>, Vec<Clip>) {
        let k = screen.1 as f32 / LAYOUT_HEIGHT;
        let mut w = Walker { page: self, screen_k: k, out: Vec::new(), additive: Vec::new(), parent: [0.0; 2], card: true, ordinal: 0, mask: None, clips: Vec::new(), mix: false, bold_scale: 1.0, run_base: 0 };
        let root = Xf { a: k, b: 0.0, c: 0.0, d: k, x: (screen.0 as f32 - 1280.0 * k) / 2.0, y: 0.0 };
        let into = |total: f32| look.leaving.map_or(look.age, |out| total - out);
        let row = |s: &str, name: &str| s.strip_prefix(name).and_then(|n| n.strip_prefix(':')).and_then(|n| n.parse::<usize>().ok()).unwrap_or(0);
        let string = |n: u32| self.strings.get(&n).cloned();
        let (base, both, f) = cursor_colors(look.age + CURSOR_PHASE_LAG);
        let over = |path: &[&str]| -> Over {
            let [_, rest @ ..] = path else { return Over::default() };
            match rest {
                [] => Over { show: true, ..Default::default() },
                ["none", "count", "cc"] => Over { show: true, ..Default::default() },
                ["none", "count", "cc", ""] => Over { text: string(EMPTY_WORD), at: Some(COUNT_LABEL_AT), ..Default::default() },
                ["none", "count", "cc", ":1"] | ["none", "count", "cc", "capacity"] => Over { hide: true, ..Default::default() },
                ["none", "count", "cc", "have_num"] => Over { text: Some(look.free.to_string()), at: Some(COUNT_VALUE_AT), ..Default::default() },
                ["confirm", ..] => Over { hide: true, ..Default::default() },
                ["scroll_bar"] => Over { thumb: Some(SHELF_THUMB), ..Default::default() },
                ["tab_top", "tab_frame", "l1", "select_L"] => Over { text: Some(TAB_LEFT_BUTTON.to_string()), size: Some(BUTTON_SIZE), color: Some([1.0; 4]), ..Default::default() },
                ["tab_top", "tab_frame", "r1", "select_R"] => Over { text: Some(TAB_RIGHT_BUTTON.to_string()), size: Some(BUTTON_SIZE), color: Some([1.0; 4]), ..Default::default() },
                ["tab_top", "taboff", tab, "text"] | ["tab_top", "tabon", tab, "text"] => {
                    let n: usize = tab.strip_prefix("tab").and_then(|n| n.parse().ok()).unwrap_or(0);
                    Over { image_no: Some(SHELF_TABS.get(n).copied().unwrap_or(5)), ..Default::default() }
                }
                ["tab_top", "tabon", tab, ..] if tab.strip_prefix("tab").and_then(|n| n.parse::<usize>().ok()) != Some(look.tab) => Over { hide: true, ..Default::default() },
                ["tab_name"] => Over { text: string(ALL_WORD), ..Default::default() },
                ["sheet", r, c, part, ..] => {
                    let idx = row(r, "row") * SHELF_COLUMNS + row(c, "cell");
                    let here = idx < look.images.len();
                    match (*part, path.len()) {
                        ("item_image", 5) if here => Over { image_no: Some(look.images[idx]), alpha: Some(ITEM_ALPHA), ..Default::default() },
                        ("highlight", 5) if idx == look.cursor && here && look.leaving.is_none() => Over { show: true, paint: Some([base[0], base[1], base[2], 1.0]), ..Default::default() },
                        ("highlight", 6) => Over { paint: Some([both[0], both[1], both[2], (191.0 * f).round() / 255.0]), ..Default::default() },
                        _ => Over { hide: true, ..Default::default() },
                    }
                }
                _ => Over::default(),
            }
        };
        // (B takes the grid and the card away at once; only the title plate slides out, `field-menu-pages` 2150 to 2170)
        let gone = look.leaving.is_some_and(|out| out >= 1.0);
        if let Some(top) = find(&self.cabinet, "default").filter(|_| !gone) {
            let at = pair(top, "pos", [0.0; 2]);
            w.slid(Slide::over(into(SHELF_SLIDE_FRAMES), SHELF_SLIDE_FRAMES, [-40.0, 0.0]), |w| w.walk(top, root.node(at, 0.0, [1.0; 2]), &mut Vec::new(), &over, &HashMap::new()));
        }
        if let Some(card) = look.item.filter(|_| !gone) {
            // (the card slides in from the right)
            w.slid(Slide::in_from(into(SLIDE_FRAMES), [40.0, 0.0]), |w| self.item_card(w, &root, "normal_arms", None, card, look.party));
        }
        self.caption_plate(&mut w, &root, CONTAINER_TITLE, Slide::in_from(into(SLIDE_FRAMES), [0.0, -30.0]));
        (w.out, w.additive, w.clips)
    }

    /// The item card `kind` (`equip_arms`, `normal_arms`, ...) of `uil_a21_item_info.xml` for `card`, its `top` node at `at` (its own
    /// place where none is given), drawn into `w`.
    fn item_card(&self, w: &mut Walker, root: &Xf, kind: &str, at: Option<[f32; 2]>, card: &Card, party: &[crate::field_menu::Member]) {
        let Some(top) = find(&self.item, kind).and_then(|k| find(k, "top")) else { return };
        let at = at.unwrap_or_else(|| pair(top, "pos", [0.0; 2]));
        let row = |s: &str| s.strip_prefix("node:").and_then(|n| n.parse::<usize>().ok()).unwrap_or(0);
        let text = |s: String| Over { text: Some(s), ..Default::default() };
        let hide = || Over { hide: true, ..Default::default() };
        let string = |n: u32| self.strings.get(&n).cloned().unwrap_or_default();
        let faces = card.equippable.len();
        let look_face = |k: usize| party.get(k).map_or(0, |m| usize::from(m.id));
        let over = |path: &[&str]| -> Over {
            let [_, _, rest @ ..] = path else { return Over::default() };
            match rest {
                ["name", "name", "item_name"] => text(string(card.name)),
                ["common", "icon", "item_icon"] => Over { image_no: Some(card.image), ..Default::default() },
                ["common", "rarity", "rarity_icon"] => Over { image_no: Some(card.rank), ..Default::default() },
                ["common", "lv", _, "lv_value"] => text(card.level.to_string()),
                ["common", "elem", "elem_value"] => text(card.elem_value.to_string()),
                ["common", "elem", "list", n, "on"] => Over { show: card.elements.get(row(n)).copied().unwrap_or(false), ..Default::default() },
                ["common", "weapon_slot"] => Over { show: card.slots.is_some(), ..Default::default() },
                ["common", "weapon_slot", "value"] => text(card.slots.unwrap_or(0).to_string()),
                ["common", "quality", _, gauge] => {
                    let k: usize = gauge.strip_prefix("quality_gauge_").and_then(|n| n.parse().ok()).unwrap_or(0);
                    let share = ((f32::from(card.quality) - 100.0 * k as f32) / 100.0).clamp(0.0, 1.0);
                    if share <= 0.0 { hide() } else { Over { crop: Some(share), ..Default::default() } }
                }
                ["common", "quality", "quality_value"] => text(card.quality.to_string()),
                ["effect", "effect", "list", n, "", "text"] => card.effects.get(row(n)).map_or_else(hide, |&e| Over { text: Some(string(e)), at: Some(EFFECT_TEXT_AT), ..Default::default() }),
                ["potential", "potential", "list", n, "text"] => card.traits.get(row(n)).map_or_else(hide, |&(t, _)| text(string(t))),
                ["potential", "potential", "list", n, "lv"] if row(n) >= card.traits.len() => hide(),
                ["potential", "potential", "list", n, "lv", "text"] => card.traits.get(row(n)).map_or_else(hide, |&(_, l)| text(l.to_string())),
                ["potential", "potential", "list", _, "lock_icon"] => hide(),
                ["status", "status", "list", stat, "value"] => {
                    let i = ["hp", "atk", "def", "spd"].iter().position(|s| s == stat).unwrap_or(0);
                    text(card.status[i].to_string())
                }
                ["role", "role", "list", stat, "value"] => {
                    let i = ["atk", "def", "spd"].iter().position(|s| s == stat).unwrap_or(0);
                    text(card.roles[i].to_string())
                }
                ["equip_chara", "equip_chara", "pos", "list", n, face] => {
                    let k = row(n);
                    let lit = card.equippable.get(k).copied().unwrap_or(false);
                    match (*face, k < faces) {
                        ("on", true) if lit => Over { image_no: Some(look_face(k)), ..Default::default() },
                        ("off", true) if !lit => Over { image_no: Some(look_face(k)), show: true, ..Default::default() },
                        _ => hide(),
                    }
                }
                ["equip_chara", "equip_chara", "pos", "list", n] => {
                    // The icons are centred on the `center` node, one `FACE_STEP` apart (STAND-IN: measured on the capture's cards).
                    let left = FACE_CENTER_X - ((faces.max(1) - 1) as f32 * FACE_STEP + FACE_SIZE) / 2.0 + row(n) as f32 * FACE_STEP;
                    Over { at: Some([left, 0.0]), ..Default::default() }
                }
                ["category", "category", "", ..] => hide(),
                ["category", "category", "list", n, rest @ ..] => match (row(n), card.category, rest) {
                    (0, Some(c), ["", "cat_icon"]) => Over { image_no: Some((c - CATEGORY_BASE) as usize), ..Default::default() },
                    (0, Some(c), ["cat_name"]) => text(string(c)),
                    (0, Some(_), _) => Over::default(),
                    _ => hide(),
                },
                _ => Over::default(),
            }
        };
        let xf = root.node(at, 0.0, [1.0; 2]);
        w.card = true;
        w.walk(top, xf, &mut Vec::new(), &over, &HashMap::new());
        w.card = false;
    }

    /// A text style's attributes (the item card's `copy` names) joined to a text node's own.
    fn styled(&self, e: &Element) -> Element {
        let mut attributes = e.attributes.clone();
        let mut next = e.attr("copy").map(str::to_string);
        for _ in 0..4 {
            let Some(style) = next.take().and_then(|n| self.text_styles.get(&n)) else { break };
            for (k, v) in style {
                if k != "name" && k != "copy" && !attributes.iter().any(|(a, _)| a == k) {
                    attributes.push((k.clone(), v.clone()));
                }
            }
            next = style.iter().find(|(k, _)| k == "copy").map(|(_, v)| v.clone());
        }
        // A text that gives both a `font_color` and its own `color` is tinted by the second (the category's heading at no opacity).
        if let (Some(f), Some(c)) = (numbers_of(&attributes, "font_color"), e.attr("color").map(|_| numbers(e, "color"))) {
            if f.len() >= 4 && c.len() >= 4 {
                let tinted: Vec<String> = (0..4).map(|i| (f[i] * c[i] / 255.0).to_string()).collect();
                attributes.retain(|(k, _)| k != "font_color");
                attributes.push(("font_color".into(), tinted.join(",")));
            }
        }
        Element { name: e.name.clone(), attributes, children: e.children.clone() }
    }

    /// What the game's code changes in the skills list's template: the rows (`clone_line_format01_n` for the n-th skill), the
    /// cursor on `look.row` and the window's scroll bar (hidden).
    fn skill_row(&self, look: &Look, clock: f32, path: &[&str]) -> Over {
        const BODY: [&str; 3] = ["menu_template", "__uimenu_window", "__uimenu_body"];
        let hide = Over { hide: true, ..Default::default() };
        match path {
            [_, "__uimenu_window", "__uimenu_scroll_bar"] => hide,
            [a, b, c, "__uimenu_cursor", "__uimenu_cursor_active"] if [*a, *b, *c] == BODY => {
                Over { show: true, at: Some([CURSOR_AT[0], CURSOR_AT[1] + ROW_STEP * look.row as f32]), ..Default::default() }
            }
            [a, b, c, "__uimenu_cursor", "__uimenu_cursor_active", "add"] if [*a, *b, *c] == BODY => Over { alpha: Some(cursor_glow(clock)), ..Default::default() },
            [a, b, c, "__uimenu_item_list", clone, rest @ ..] if [*a, *b, *c] == BODY => {
                let Some(skill) = clone.strip_prefix("clone_line_format01_").and_then(|n| n.parse::<usize>().ok()).and_then(|n| n.checked_sub(1)).and_then(|n| look.skills.get(n)) else { return hide };
                match rest {
                    ["attribute_icon1"] => Over { image_no: Some(skill.attribute), ..Default::default() },
                    ["attribute_icon2"] => hide,
                    ["buff_icon_base", "buff_icon"] => Over { image_no: Some(skill.icon), ..Default::default() },
                    ["name"] => Over { text: self.strings.get(&skill.name).cloned(), tint: Some(ROW_COLOR), ..Default::default() },
                    ["mp_value_base"] if skill.cost == 0 => hide,
                    ["mp_value_base", "mp_value"] => Over { text: Some(skill.cost.to_string()), ..Default::default() },
                    _ => Over::default(),
                }
            }
            _ => Over::default(),
        }
    }

    /// A style's picture `n`, its slot in the layer and its texture's size in style pixels.
    fn picture(&self, style: &str, n: usize) -> Option<(usize, &Pic, [f32; 2])> {
        let list = self.styles.get(style)?;
        let p = list.get(n).or_else(|| list.first())?;
        let slot = self.wanted.iter().position(|(path, i)| *path == p.path && *i == p.index)?;
        let (tw, th) = self.sizes[slot];
        Some((FIXED + slot, p, [tw as f32 * p.over_texture, th as f32 * p.over_texture]))
    }

    /// The node a `copy` names: its first definition with children, in the equip or status layout (or, walking a synthesis screen, in its own).
    fn template(&self, name: &str, mix: bool) -> Option<&Element> {
        if name.contains('/') {
            // A path (`clone_targets/effect/node`): each name the first node of that name under the one before it.
            // (`template_cell` and `template_tab` are nodes that load a file: its root stands for them)
            let mut parts = name.split('/');
            let first = parts.next()?;
            if mix {
                return match first {
                    "template_cell" => descend(&self.mix_docs.cell, parts),
                    "template_tab" => descend(&self.mix_docs.tabs, parts),
                    _ => [&self.mix_docs.recipe, &self.mix_docs.field, &self.mix_docs.cabinet, &self.mix_docs.tabs, &self.mix_docs.caption, &self.mix_docs.simple].into_iter().find_map(|doc| descend(find(doc, first)?, parts.clone())),
                };
            }
            return match first {
                "template_cell" => descend(&self.cell, parts),
                "template_tab" => descend(&self.tabs, parts),
                _ => [&self.targets, &self.item, &self.cabinet].into_iter().find_map(|doc| descend(find(doc, first)?, parts.clone())),
            };
        }
        let own = [&self.mix_docs.recipe, &self.mix_docs.field, &self.mix_docs.cabinet, &self.mix_docs.tabs, &self.mix_docs.cell, &self.mix_docs.caption];
        let shared = [&self.explore, &self.equip, &self.status, &self.faces, &self.skill, &self.targets, &self.tabs, &self.cell, &self.library_items, &self.library_eff];
        let docs: Vec<&Element> = if mix { own.into_iter().chain(shared).collect() } else { shared.to_vec() };
        docs.into_iter().find_map(|doc| find_with_children(doc, name))
    }

    /// The children a node's `clone_child_tree` gives: the template's, which may be a clone of another (the Container grid's `cell`).
    fn template_children<'a>(&'a self, e: &Element, depth: usize, mix: bool) -> Vec<&'a Element> {
        let Some(template) = e.attr("clone_child_tree").filter(|_| depth < 4).and_then(|t| self.template(t, mix)) else { return Vec::new() };
        let mut children = self.template_children(template, depth + 1, mix);
        children.extend(template.children.iter());
        children
    }

    fn string(&self, id: &str) -> Option<&str> {
        let number = match id.strip_prefix("STR_UI_") {
            Some(n) => STR_UI_BASE + n.parse::<u32>().ok()?,
            None => LIBRARY_BASE + id.strip_prefix("STR_LIBRARY_DEFINE_")?.parse::<u32>().ok()?,
        };
        self.strings.get(&number).map(String::as_str)
    }
}

/// The game's animation clock loses a frame now and then: it falls one more behind the game's frames every `STALL_EVERY` frames counted from frame 0 of the
/// capture's run (stalls at 14, 15, 16 ... times that: 2264, 2425, 2587 ...). The Equipment page's blink is 1 frame ahead of the clock on 2190 to 2260, in step
/// on 2290 to 2420 (a stall between 2260 and 2290) and 1 behind on 2430 to 2490 (the next between 2420 and 2430), and the Gathering Tools page (A on 2580) starts one
/// frame later than the Container's (A on 2000) by the stall on 2587; the period is 161.7 inside the three bounds (161.43 to 162 from the first two, above 161.25
/// from the third) and 1 / 0.9938, the Guide's blink rate, is 161.3 within that rate's error. The cause of the stall is not read.
const STALL_EVERY: f32 = 161.7;

/// What the synthesis's finished item card shows (`uil_a21_mix_item_sheet.xml`): the item, its picture and the number made.
#[derive(Clone, Copy)]
pub struct SheetLook {
    /// The item's number (its name is the string `NAME_BASE + item`).
    pub item: u16,
    /// The item's picture number in `etc_a21_item_l`.
    pub image: usize,
    /// Units made (the card's "To Make:" line).
    pub count: u32,
    /// The animation clock of the card's coming in (`anm_finish`), in frames of 60 a second since it began; `None` for the card at rest.
    pub clock: Option<f32>,
    /// The frames since it began, by the frame count (the white-out's fade runs on that, not on the clock: the capture's clock ran
    /// ahead of the frame count by 12 frames from its 3880 on).
    pub frames: f32,
    /// The frames since the card began to go out (`mix_anm_finish_card_out`: it rises 30 layout pixels by in cubic and fades out in 20).
    pub out: Option<f32>,
}

// The card's coming in (`anm_finish` of `uil_a21_mix_item_sheet.xml`; the curves are `uia_a21_mix.xml`'s, in frames of 60 a second).
/// `mix_anm_finish_card_in`'s rise: the card starts 25 layout pixels low from frame 20 to 140, out cubic, the panel's copy at 2.5 times the speed.
const CARD_RISE: f32 = 25.0;
const PANEL_SPEED: f32 = 2.5;
const CARD_OUT_FRAMES: f32 = 20.0;
const CARD_OUT_RISE: f32 = 30.0;
const CARD_ALPHA: [(f32, f32); 3] = [(0.0, 0.0), (20.0, 0.0), (60.0, 1.0)];
/// `mix_anm_finish_whiteout`: the white over the card's whole picture, whole to frame 6, gone at 20.
const WHITE_FADE: [(f32, f32); 3] = [(0.0, 1.0), (6.0, 1.0), (20.0, 0.0)];
/// The full-screen white's fade (`anm_common_juvenile_fade_in` played backwards, `delay` 0.18 s: 10.8 frames, over 60, in cubic): the
/// opacity left `frames` after the card began.
fn whiteout_all(frames: f32) -> f32 {
    (1.0 - ((frames - 10.8) / 60.0).clamp(0.0, 1.0)).powi(3)
}
const MIX05_SCALE: [(f32, f32); 3] = [(0.0, 2.0), (6.0, 2.0), (60.0, 20.0)];
const MIX05_ALPHA: [(f32, f32); 3] = [(0.0, 1.0), (6.0, 1.0), (60.0, 0.0)];
const MIX06_TURN: [(f32, f32); 2] = [(0.0, 0.0), (120.0, 9.0)];
const MIX06_SCALE: [(f32, f32); 2] = [(0.0, 1.5), (120.0, 2.6)];
const MIX06_ALPHA: [(f32, f32); 3] = [(0.0, 0.0), (40.0, 1.0), (120.0, 0.0)];
const MIX09_SCALE: [(f32, f32); 4] = [(0.0, 0.2), (13.0, 2.0), (20.0, 4.0), (120.0, 10.0)];
const MIX09_ALPHA: [(f32, f32); 3] = [(0.0, 0.0), (25.0, 1.0), (120.0, 0.0)];

/// A node turned and scaled about its pivot (`pivot_pos`, in its own pixels), placed at its `pos` in `root`, as the mix effects' animations do.
fn about(root: &Xf, e: &Element, turn: f32, scale: [f32; 2]) -> Xf {
    let (pivot, at) = (pair(e, "pivot_pos", [0.0; 2]), pair(e, "pos", [0.0; 2]));
    let (sin, cos) = turn.to_radians().sin_cos();
    let moved = [scale[0] * cos * pivot[0] - scale[1] * sin * pivot[1], scale[0] * sin * pivot[0] + scale[1] * cos * pivot[1]];
    root.node([at[0] + pivot[0] - moved[0], at[1] + pivot[1] - moved[1]], turn, scale)
}

/// An out cubic ease of `u` (0 to 1): fast first.
fn out_cubic(u: f32) -> f32 {
    1.0 - (1.0 - u.clamp(0.0, 1.0)).powi(3)
}

/// The card's title is a `MainFont00` text of this size (layout pixels): its node names none.
/// STAND-IN: the engine's default text size is not read; 24 fits the capture's title (its "R" is 32 pixels tall on 1080 lines).
const SHEET_TITLE_SIZE: f32 = 24.0;

impl Page {
    /// The finished item's card, on a `screen` of pixels (the card is whole: its coming in, `mix_anm_finish_card_in`, is not drawn).
    pub fn sheet_quads(&self, look: &SheetLook, screen: (u32, u32)) -> (Vec<Quad>, Vec<usize>, Vec<Clip>) {
        let k = screen.1 as f32 / LAYOUT_HEIGHT;
        let mut w = Walker { page: self, screen_k: k, out: Vec::new(), additive: Vec::new(), parent: [0.0; 2], card: true, ordinal: 0, mask: None, clips: Vec::new(), mix: false, bold_scale: 1.0, run_base: 0 };
        let root = Xf { a: k, b: 0.0, c: 0.0, d: k, x: (screen.0 as f32 - 1280.0 * k) / 2.0, y: 0.0 };
        let t = look.clock;
        // The effect under the card.
        if let (Some(t), Some(e)) = (t, find(&self.mix_sheet, "a21_mix06")) {
            w.image(e, about(&root, e, keyed(&MIX06_TURN, t), [keyed(&MIX06_SCALE, t); 2]), &Over { alpha: Some(keyed(&MIX06_ALPHA, t)), ..Default::default() });
        }
        // The white-out over the whole screen (the layout's `whiteout_all`, 1282 by 722 from (-1, -1)).
        if t.is_some() {
            w.slid(Slide { alpha: whiteout_all(look.frames), dx: 0.0, dy: 0.0 }, |w| w.piece(root.node([-1.0, -1.0], 0.0, [1.0; 2]), [0.0; 2], [1282.0, 722.0], [0.0, 0.0, 0.125, 0.125], [1.0; 4], WHITE));
        }
        if let Some(panel) = find(&self.mix_sheet, "panel") {
            let name = self.strings.get(&(NAME_BASE + u32::from(look.item))).cloned().unwrap_or_default();
            let count = look.count.to_string();
            let over = |path: &[&str]| -> Over {
                match path {
                    ["panel", "item_sheet"] => Over { show: true, ..Default::default() },
                    ["panel", "item_sheet", "image"] => Over { image_no: Some(look.image), ..Default::default() },
                    ["panel", "item_sheet", "name"] => Over { text: Some(name.clone()), size: Some(SHEET_TITLE_SIZE), ..Default::default() },
                    ["panel", "item_sheet", "num"] => Over { text: Some(count.clone()), ..Default::default() },
                    _ => Over::default(),
                }
            };
            // The card rises by the panel's animation (at 2.5 times the speed) and its own, and fades in by both.
            let slide = match t {
                Some(t) => {
                    let rise = |clock: f32| CARD_RISE * (1.0 - out_cubic((clock - 20.0) / 120.0));
                    let fade = |clock: f32| keyed(&CARD_ALPHA, clock);
                    Slide { alpha: fade(t) * fade(t * PANEL_SPEED), dx: 0.0, dy: rise(t) + rise(t * PANEL_SPEED) }
                }
                None => match look.out {
                    Some(out) if out >= CARD_OUT_FRAMES => return (w.out, w.additive, w.clips),
                    Some(out) => {
                        let u = (out / CARD_OUT_FRAMES).clamp(0.0, 1.0);
                        Slide { alpha: 1.0 - u, dx: 0.0, dy: -CARD_OUT_RISE * u * u * u }
                    }
                    None => Slide::NONE,
                },
            };
            w.slid(slide, |w| w.walk(panel, root.node(pair(panel, "pos", [0.0; 2]), 0.0, [1.0; 2]), &mut Vec::new(), &over, &HashMap::new()));
        }
        // The effects over it: the white (1300 by 800 layout pixels), then the glows.
        if let Some(t) = t {
            let a = keyed(&WHITE_FADE, t);
            w.slid(Slide { alpha: a, dx: 0.0, dy: 0.0 }, |w| w.piece(root.node([0.0; 2], 0.0, [1300.0, 800.0]), [0.0; 2], [1.0; 2], [0.0, 0.0, 0.125, 0.125], [1.0; 4], WHITE));
            if let Some(e) = find(&self.mix_sheet, "a21_mix05") {
                w.image(e, about(&root, e, 0.0, [keyed(&MIX05_SCALE, t); 2]), &Over { alpha: Some(keyed(&MIX05_ALPHA, t)), ..Default::default() });
            }
            if let Some(e) = find(&self.mix_sheet, "a21_mix09") {
                w.image(e, about(&root, e, 0.0, [keyed(&MIX09_SCALE, t); 2]), &Over { alpha: Some(keyed(&MIX09_ALPHA, t)), ..Default::default() });
            }
        }
        (w.out, w.additive, w.clips)
    }
}

/// What the synthesis's "Complete" screen shows besides its title (`uil_a21_mix_item_sheet.xml`'s `chara_detail`): the alchemy level
/// after the synthesis, the experience to the next level and the experience gained.
pub struct CompleteLook {
    pub level: u32,
    pub next: u32,
    pub gained: u32,
    /// Frames since the "Level UP!" animation began (`anm_level_up`), 0 before it.
    pub age: f32,
    /// The frames since A sent the screen away to the room: the level panel slides out to the right.
    pub leaving: Option<f32>,
}

/// The Complete screen's level panel leaves over this many frames (`anm_common_slide_in_right` played backwards).
// The slide's own 30 frames (`synth-atelier` 4250 and 4260: the panel's opacity 223/255 and 107/255, 5 and 23.1 layout pixels from its place, are 15 and 25 frames in); it begins 5 frames
// before the Machine's A (`play::LEAVE_BEFORE_A`).
const LEAVE_FRAMES: f32 = 30.0;

/// A keyed curve of `uia_a21_mix.xml`: (frame, value) pairs, linear between them; before the first key the first value, after the
/// last the last (two keys on one frame make a step).
fn keyed(keys: &[(f32, f32)], t: f32) -> f32 {
    let Some(&(_, first)) = keys.first() else { return 0.0 };
    if t < keys[0].0 {
        return first;
    }
    for w in keys.windows(2) {
        let ((t0, v0), (t1, v1)) = (w[0], w[1]);
        if t >= t0 && t < t1 {
            return v0 + (v1 - v0) * (t - t0) / (t1 - t0);
        }
    }
    keys[keys.len() - 1].1
}

/// A keyed curve with `interpolate="spline"`: the Catmull-Rom spline through the keys (uniform in the parameter of each segment;
/// the end keys are their own neighbours), as the plate's rise matches the capture's seven frames to the hundredth of a pixel.
fn spline(keys: &[(f32, f32)], t: f32) -> f32 {
    let last = keys.len() - 1;
    if t <= keys[0].0 {
        return keys[0].1;
    }
    for i in 0..last {
        if t >= keys[i].0 && t <= keys[i + 1].0 {
            let (p0, p1, p2, p3) = (keys[i.saturating_sub(1)].1, keys[i].1, keys[i + 1].1, keys[(i + 2).min(last)].1);
            let u = (t - keys[i].0) / (keys[i + 1].0 - keys[i].0);
            return 0.5 * (2.0 * p1 + (-p0 + p2) * u + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * u * u + (-p0 + 3.0 * p1 - 3.0 * p2 + p3) * u * u * u);
        }
    }
    keys[last].1
}

// The "Level UP!" animations (`mix_anm_level_up_plate`, `_plate_on`, `_effect_07` and `_effect_09` of `uia_a21_mix.xml`, in frames of 60 a second).
const PLATE_RISE: [(f32, f32); 6] = [(0.0, 150.0), (20.0, 150.0), (45.0, -50.0), (65.0, 25.0), (80.0, -13.0), (95.0, 0.0)];
const PLATE_ALPHA: [(f32, f32); 5] = [(0.0, 0.0), (20.0, 0.0), (50.0, 1.0), (105.0, 1.0), (105.0, 0.0)];
const ON_SCALE: [(f32, f32); 3] = [(0.0, 0.9), (50.0, 0.9), (85.0, 1.5)];
const ON_ALPHA: [(f32, f32); 3] = [(0.0, 0.0), (40.0, 1.0), (85.0, 0.0)];
const E07_SCALE: [(f32, f32); 3] = [(0.0, 0.1), (30.0, 0.1), (60.0, 4.0)];
const E07_ALPHA: [(f32, f32); 4] = [(0.0, 0.0), (30.0, 0.0), (40.0, 1.0), (60.0, 0.0)];
const E09_SCALE_X: [(f32, f32); 5] = [(0.0, 6.0), (25.0, 6.0), (55.0, 6.0), (70.0, 26.4), (80.0, 40.0)];
const E09_SCALE_Y: [(f32, f32); 5] = [(0.0, 6.0), (25.0, 6.0), (55.0, 2.0), (70.0, 0.02), (80.0, 0.02)];
const E09_ALPHA: [(f32, f32); 5] = [(0.0, 0.0), (25.0, 0.0), (30.0, 1.0), (55.0, 1.0), (80.0, 0.0)];

impl Page {
    /// The "Complete" screen's parts, on a `screen` of pixels, once the level panel has slid in and the "Level UP!" plate is gone: the
    /// title plate and the alchemy level panel. STAND-IN: the numbers are the capture's; the experience rule is not read.
    pub fn complete_quads(&self, look: &CompleteLook, screen: (u32, u32)) -> (Vec<Quad>, Vec<usize>, Vec<Clip>) {
        let k = screen.1 as f32 / LAYOUT_HEIGHT;
        let mut w = Walker { page: self, screen_k: k, out: Vec::new(), additive: Vec::new(), parent: [0.0; 2], card: true, ordinal: 0, mask: None, clips: Vec::new(), mix: false, bold_scale: 1.0, run_base: 0 };
        let root = Xf { a: k, b: 0.0, c: 0.0, d: k, x: (screen.0 as f32 - 1280.0 * k) / 2.0, y: 0.0 };
        // The effects under the card and over it, then the level panel, then the plate (the layout's order).
        let age = look.age;
        if let Some(e) = find(&self.mix_sheet, "a21_mix07") {
            let a = keyed(&E07_ALPHA, age);
            let s = keyed(&E07_SCALE, age);
            w.image(e, about(&root, e, 0.0, [s, s]), &Over { alpha: Some(a), ..Default::default() });
        }
        if let Some(e) = find(&self.mix_sheet, "a21_mix09") {
            let a = keyed(&E09_ALPHA, age);
            let scale = [keyed(&E09_SCALE_X, age), keyed(&E09_SCALE_Y, age)];
            w.image(e, about(&root, e, 0.0, scale), &Over { alpha: Some(a), ..Default::default() });
        }
        let out = look.leaving.map_or(Slide::NONE, |t| Slide::over(LEAVE_FRAMES - t, LEAVE_FRAMES, [40.0, 0.0]));
        self.level_panel(&mut w, &root, look, out);
        // The "Level UP!" plate: its picture and its words rise together, a glow of the words grows over them.
        if let (Some(plate), Some(text), Some(on)) = (find(&self.mix_sheet, "level_up_plate"), find(&self.mix_sheet, "level_up_text"), find(&self.mix_sheet, "level_up_text_on")) {
            let a = keyed(&PLATE_ALPHA, age);
            let at = pair(plate, "pos", [0.0; 2]);
            let xf = root.node([at[0], at[1] + spline(&PLATE_RISE, age)], 0.0, [1.0; 2]);
            w.image(plate, xf, &Over { alpha: Some(a), ..Default::default() });
            let words = xf.node(pair(text, "pos", [0.0; 2]), 0.0, [1.0; 2]);
            w.image(text, words, &Over { alpha: Some(a), ..Default::default() });
            let s = keyed(&ON_SCALE, age);
            let (pivot, at) = (pair(on, "pivot_pos", [0.0; 2]), pair(on, "pos", [0.0; 2]));
            let glow = words.node([at[0] + pivot[0] - s * pivot[0], at[1] + pivot[1] - s * pivot[1]], 0.0, [s, s]);
            w.image(on, glow, &Over { alpha: Some(a * keyed(&ON_ALPHA, age)), ..Default::default() });
        }
        self.caption_plate(&mut w, &root, COMPLETE_TITLE, Slide::NONE);
        (w.out, w.additive, w.clips)
    }
}

impl Page {
    /// The "Complete" screen's item card, as the Container page's (the made item's `card`: a `normal_item` card at its place), and the
    /// title plate, on a `screen` of pixels.
    /// `leaving`: the frames since A, which sends the card out to the right (the slide in played backwards) and brings the level panel in.
    pub fn result_quads(&self, card: &Card, leaving: Option<(f32, CompleteLook)>, entering: Option<(f32, SheetLook)>, screen: (u32, u32)) -> (Vec<Quad>, Vec<usize>, Vec<Clip>) {
        let k = screen.1 as f32 / LAYOUT_HEIGHT;
        let mut w = Walker { page: self, screen_k: k, out: Vec::new(), additive: Vec::new(), parent: [0.0; 2], card: false, ordinal: 0, mask: None, clips: Vec::new(), mix: false, bold_scale: 1.0, run_base: 0 };
        let root = Xf { a: k, b: 0.0, c: 0.0, d: k, x: (screen.0 as f32 - 1280.0 * k) / 2.0, y: 0.0 };
        let slide = match (&leaving, entering) {
            (Some((out, _)), _) => Slide::in_from(SLIDE_FRAMES - out.max(0.0), [40.0, 0.0]),
            // (the card comes in a frame after the plate and the finished card's going)
            (None, Some((age, _))) => Slide::in_from(age - 1.0, [40.0, 0.0]),
            (None, None) => Slide::NONE,
        };
        w.slid(slide, |w| self.item_card(w, &root, "normal_item", None, card, &[]));
        // The finished card goes out above it, and the title plate comes in.
        if let Some((age, sheet)) = entering {
            let (quads, additive, clips) = self.sheet_quads(&SheetLook { out: Some(age), clock: None, ..sheet }, screen);
            let base = w.out.len();
            w.out.extend(quads);
            w.additive.extend(additive.into_iter().map(|i| i + base));
            debug_assert!(clips.is_empty());
        }
        // The level panel slides in over the 30 frames the card goes out in.
        if let Some((out, look)) = &leaving {
            self.level_panel(&mut w, &root, look, Slide::in_from(*out, [40.0, 0.0]));
        }
        self.caption_plate(&mut w, &root, COMPLETE_TITLE, entering.map_or(Slide::NONE, |(age, _)| Slide::in_from(age, [0.0, -30.0])));
        (w.out, w.additive, w.clips)
    }
}

impl Page {
    /// The alchemy level panel (`chara_detail`: the level, the experience to the next and the experience gained), slid in by `slide`.
    fn level_panel(&self, w: &mut Walker, root: &Xf, look: &CompleteLook, slide: Slide) {
        let Some(panel) = find(&self.mix_sheet, "chara_detail") else { return };
        let numbers = [look.level.to_string(), look.next.to_string(), look.gained.to_string()];
        let over = |path: &[&str]| -> Over {
            match path {
                ["chara_detail"] => Over { show: true, ..Default::default() },
                ["chara_detail", "Alc_lv", "alclv"] => Over { text: Some(numbers[0].clone()), ..Default::default() },
                ["chara_detail", "Next", "next"] => Over { text: Some(numbers[1].clone()), ..Default::default() },
                ["chara_detail", "GetExp", "exp"] => Over { text: Some(numbers[2].clone()), ..Default::default() },
                _ => Over::default(),
            }
        };
        w.slid(slide, |w| w.walk(panel, root.node(pair(panel, "pos", [0.0; 2]), 0.0, [1.0; 2]), &mut Vec::new(), &over, &HashMap::new()));
    }
}

/// The animation clock's reading at game frame `frame`: the frame less the stalls so far.
pub fn animation_clock(frame: f32) -> f32 {
    frame - (frame / STALL_EVERY).floor()
}

/// `anm_common_blink`: the opacity goes from whole to none and back in 120 frames (a byte's steps), looping.
fn blink(frames: f32) -> f32 {
    let t = frames.rem_euclid(BLINK_FRAMES);
    let half = BLINK_FRAMES / 2.0;
    let a = if t < half { 1.0 - t / half } else { (t - half) / half };
    (a * 255.0).round() / 255.0
}

/// The Effects page's stars after the first are empty nodes the game fills with copies of the first (`clone_star`): say so in the layout.
fn stars_copy_the_first(e: &mut Element) {
    if e.attr("name").is_some_and(|n| n.starts_with("clone_star_")) && e.children.is_empty() {
        e.attributes.push(("clone_child_tree".into(), "clone_star".into()));
    }
    e.children.iter_mut().for_each(stars_copy_the_first);
}

fn collect_styles(e: &Element, out: &mut BTreeSet<String>) {
    if let Some(s) = e.attr("style") {
        out.insert(s.to_string());
    }
    e.children.iter().for_each(|c| collect_styles(c, out));
}

fn find<'a>(e: &'a Element, name: &str) -> Option<&'a Element> {
    if e.attr("name") == Some(name) {
        return Some(e);
    }
    e.children.iter().find_map(|c| find(c, name))
}

fn find_pos(e: &Element, name: &str) -> Option<[f32; 2]> {
    find(e, name).map(|n| pair(n, "pos", [0.0; 2]))
}

/// A node's place on the screen: a 2 x 2 matrix (columns `a, b` and `c, d`) and a translation.
#[derive(Clone, Copy, Debug)]
struct Xf {
    a: f32,
    b: f32,
    c: f32,
    d: f32,
    x: f32,
    y: f32,
}

impl Xf {
    fn apply(&self, p: [f32; 2]) -> [f32; 2] {
        [self.a * p[0] + self.c * p[1] + self.x, self.b * p[0] + self.d * p[1] + self.y]
    }

    /// A child at `pos` turned `degrees` clockwise and scaled, about its top left.
    fn node(&self, pos: [f32; 2], degrees: f32, scale: [f32; 2]) -> Xf {
        let (s, c) = degrees.to_radians().sin_cos();
        let (la, lb, lc, ld) = (c * scale[0], s * scale[0], -s * scale[1], c * scale[1]);
        let [x, y] = self.apply(pos);
        Xf { a: self.a * la + self.c * lb, b: self.b * la + self.d * lb, c: self.a * lc + self.c * ld, d: self.b * lc + self.d * ld, x, y }
    }
}

struct Walker<'a> {
    page: &'a Page,
    /// Screen pixels to a layout pixel.
    screen_k: f32,
    out: Vec<Quad>,
    /// The quads of `out` drawn added to what is under them (`alpha_blend="add"`).
    additive: Vec<usize>,
    /// The size of the node whose children are being walked (a `pos_ratio` is a share of it).
    parent: [f32; 2],
    /// Walking the item card: texts take their styles' attributes and siblings of one name are told apart by a count (`node`,
    /// `node:1`, ...) in the paths.
    card: bool,
    ordinal: usize,
    /// The last stencil mask (`stencil_mask="create"`) and the clips of what `masked` nodes drew after it.
    mask: Option<usize>,
    clips: Vec<Clip>,
    /// Walking one of the synthesis screens: clones name the synthesis layouts' templates first.
    mix: bool,
    /// The outlines of texts and pictures stand this share of their size from them (the ring graph's plates keep the ring's zoom, not the unzoomed node's).
    bold_scale: f32,
    /// The plain string's character the line being drawn starts at (the colour runs count from the string's start).
    run_base: usize,
}

/// An animation's hold on a part of the page: its opacity and its offset in layout pixels.
#[derive(Clone, Copy, Debug)]
struct Slide {
    alpha: f32,
    dx: f32,
    dy: f32,
}

impl Slide {
    const NONE: Slide = Slide { alpha: 1.0, dx: 0.0, dy: 0.0 };

    /// `anm_common_slide_in_*`: the opacity and the offset (from `from` to nothing) come in over 30 frames, fast first
    /// (`easing_outcubic`), `frames` after the animation began.
    fn in_from(frames: f32, from: [f32; 2]) -> Slide {
        Slide::over(frames, SLIDE_FRAMES, from)
    }

    /// The Equipment page's slots `frames` after B: the opacity falls linearly over `SLOT_OUT_FRAMES` while they sink
    /// `SLOT_SINK` layout pixels by `easing_insine`.
    fn slots_out(frames: f32) -> Slide {
        let p = (frames / SLOT_OUT_FRAMES).clamp(0.0, 1.0);
        Slide { alpha: 1.0 - p, dx: 0.0, dy: SLOT_SINK * (1.0 - (p * std::f32::consts::FRAC_PI_2).cos()) }
    }

    /// The same over `total` frames (`speed` 0.75 of the Container page: 40).
    fn over(frames: f32, total: f32, from: [f32; 2]) -> Slide {
        let e = 1.0 - (1.0 - (frames / total).clamp(0.0, 1.0)).powi(3);
        Slide { alpha: e, dx: from[0] * (1.0 - e), dy: from[1] * (1.0 - e) }
    }
}

/// The cursor's added glow on the skills list: `anm_cursor_blink_ex_skill`, 0 to 100/255 and back in 192 frames, looping.
fn cursor_glow(frames: f32) -> f32 {
    let t = frames.rem_euclid(CURSOR_GLOW_FRAMES);
    let half = CURSOR_GLOW_FRAMES / 2.0;
    let a = if t < half { t / half } else { (CURSOR_GLOW_FRAMES - t) / half } * CURSOR_GLOW_PEAK;
    a.round() / 255.0
}

impl Walker<'_> {
    /// Runs `draw` and moves what it drew by `slide` and scales its opacity (nothing is drawn at none).
    fn slid(&mut self, slide: Slide, draw: impl FnOnce(&mut Walker)) {
        if slide.alpha <= 0.0 {
            return;
        }
        let start = self.out.len();
        draw(self);
        let (dx, dy) = (slide.dx * self.screen_k, slide.dy * self.screen_k);
        for q in &mut self.out[start..] {
            q.rect = [q.rect[0] + dx, q.rect[1] + dy, q.rect[2] + dx, q.rect[3] + dy];
            match &mut q.spin {
                Some(Spin::Corners(c)) => c.iter_mut().for_each(|p| *p = [p[0] + dx, p[1] + dy]),
                Some(Spin::About { pivot, .. }) => *pivot = [pivot[0] + dx, pivot[1] + dy],
                None => {}
            }
            if slide.alpha < 1.0 {
                // (the animation's opacity is a byte of its own; the product with the quad's is not rounded again: a part
                // at 1/255 going out is under a byte and the capture's trace drops it)
                q.color[3] = (q.color[3] * (slide.alpha * 255.0).round() + 1e-3).floor() / 255.0;
            }
        }
    }

    /// Draws `e` and what is under it. `path` is the names from the walk's start; `over` the game's changes by path;
    /// `commands` the `command` changes a clone's parent gave by node name.
    fn walk(&mut self, e: &Element, xf: Xf, path: &mut Vec<String>, over: &dyn Fn(&[&str]) -> Over, commands: &HashMap<String, Vec<Element>>) {
        // (the item card's texts copy a style: its font, size, colour and place by `pos_ratio_x`)
        let styled;
        let e = if self.card && e.name == "text" && e.attr("copy").is_some() {
            styled = self.page.styled(e);
            &styled
        } else {
            e
        };
        let mut name = e.attr("name").unwrap_or("").to_string();
        let nth = std::mem::take(&mut self.ordinal);
        if self.card && nth > 0 {
            name = format!("{name}:{nth}");
        }
        path.push(name.clone());
        let names: Vec<&str> = path.iter().map(String::as_str).collect();
        let mut o = over(&names);
        if let Some(cs) = commands.get(&name) {
            for c in cs {
                if let Some(n) = c.attr("set_image_no").and_then(|v| v.trim().parse().ok()) {
                    o.image_no = Some(n);
                }
                if let Some(s) = c.attr("set_string_id") {
                    o.text = o.text.take().or_else(|| self.page.string(s).map(str::to_string));
                }
            }
        }
        let hidden = (e.attr("visible") == Some("false") && !o.show) || o.hide;
        if !hidden && e.name != "usuals" && e.name != "clone_targets" && name != "clone_targets" && name != "usuals" {
            // This node itself: its place from the node's own attributes (the start node's place is the caller's).
            let me = if path.len() == 1 {
                xf
            } else {
                let turn = o.turn.unwrap_or_else(|| numbers(e, "rot").get(2).copied().unwrap_or(0.0));
                let mut scale = o.scale.unwrap_or_else(|| pair(e, "scale", [1.0; 2]));
                // (a half turn about the vertical axis, `rot="0, 180, 0"`, mirrors the node across its left edge)
                if numbers(e, "rot").get(1).is_some_and(|y| (y.abs() - 180.0).abs() < 0.5) {
                    scale[0] = -scale[0];
                }
                let mut at = o.at.unwrap_or_else(|| pair(e, "pos", [0.0; 2]));
                // (`pos_ratio_x` and `pos_ratio_y`: a share of the parent's width and height added to the place)
                for (axis, name) in ["pos_ratio_x", "pos_ratio_y"].into_iter().enumerate() {
                    at[axis] += e.attr(name).and_then(|v| v.trim().parse::<f32>().ok()).unwrap_or(0.0) * self.parent[axis];
                }
                // (`pos_ratio="x, y, z"`: the same for both axes in one)
                if let [rx, ry, ..] = numbers(e, "pos_ratio")[..] {
                    at = [at[0] + rx * self.parent[0], at[1] + ry * self.parent[1]];
                }
                if e.attr("pivot_pos_preset") == Some("center") {
                    // Turned and scaled about the picture's centre: the corner moves so the centre stays.
                    let c = self.node_size(e).map(|v| v / 2.0);
                    let (sin, cos) = turn.to_radians().sin_cos();
                    let moved = [cos * scale[0] * c[0] - sin * scale[1] * c[1], sin * scale[0] * c[0] + cos * scale[1] * c[1]];
                    at = [at[0] + c[0] - moved[0], at[1] + c[1] - moved[1]];
                }
                xf.node(at, turn, scale)
            };
            let before = self.out.len();
            match e.name.as_str() {
                "image" => self.image(e, me, &o),
                "text" => self.text(e, me, &o),
                _ => {}
            }
            // A stencil's mask is the picture itself (in the trace, not on the screen); what a `masked` node draws is clipped by it.
            if e.attr("stencil_mask") == Some("create") && self.out.len() > before {
                self.mask = Some(self.out.len() - 1);
            }
            let masked_from = self.out.len();
            // A copied node's children: the template's (found by name) and its own; the clone's `command`s change them.
            let mut children: Vec<&Element> = Vec::new();
            let mut cmds: HashMap<String, Vec<Element>> = commands.clone();
            let page = self.page;
            children.extend(page.template_children(e, 0, self.mix));
            for c in &e.children {
                if c.name == "command" {
                    if let Some(t) = c.attr("target_node") {
                        cmds.entry(t.to_string()).or_default().push(c.clone());
                    }
                } else {
                    children.push(c);
                }
            }
            let size = if e.name == "image" { self.node_size(e) } else { [0.0; 2] };
            let outer = std::mem::replace(&mut self.parent, size);
            let mut seen: HashMap<String, usize> = HashMap::new();
            for c in children {
                if self.card {
                    let n = seen.entry(c.attr("name").unwrap_or("").to_string()).or_default();
                    self.ordinal = *n;
                    *n += 1;
                }
                self.walk(c, me, path, over, &cmds);
            }
            self.parent = outer;
            if e.name == "scroll_bar" {
                // The bar's slider (`uil_sbt_default.xml`: `_slider` at (9, 20) in the bar, 10 wide) where the game's code gives it one, after the bar's own pictures.
                if let Some([y, h]) = o.thumb {
                    let slider = Element { name: "image".into(), attributes: vec![("style".into(), "gen_a21_mix02_flame01_sb".into()), ("size_wh".into(), format!("10.333, {h}, 0"))], children: Vec::new() };
                    self.image(&slider, me.node([9.0, y], 0.0, [1.0; 2]), &Over::default());
                }
            }
            if let (Some(mask), Some("masked")) = (self.mask, e.attr("stencil_mask")) {
                self.clips.push(Clip { mask, quads: masked_from..self.out.len() });
            }
        }
        path.pop();
    }

    /// An image node's size in layout pixels: its own or its picture's.
    fn node_size(&self, e: &Element) -> [f32; 2] {
        let natural = e.attr("style").and_then(|st| self.page.picture(st, 0)).map_or([0.0; 2], |(_, p, _)| [p.uvwh[2] / p.per_layout, p.uvwh[3] / p.per_layout]);
        let wh = numbers(e, "size_wh");
        let mut size = if self.mix && wh.len() >= 2 { [wh[0], wh[1]] } else { [numbers(e, "size_w").first().or(wh.first()).copied().unwrap_or(natural[0]), numbers(e, "size_h").first().or(wh.get(1)).copied().unwrap_or(natural[1])] };
        for (axis, name) in ["parent_size_copy_w", "parent_size_copy_h"].into_iter().enumerate() {
            if e.attr(name) == Some("true") && self.parent[axis] > 0.0 {
                size[axis] = self.parent[axis];
            }
        }
        size
    }

    fn image(&mut self, e: &Element, xf: Xf, o: &Over) {
        // A mask picture with no style of its own (`stencil_mask="create"`) is the placeholder (`sys_no_image`) at the node's size, in the synthesis's screens.
        if let (true, None, Some("create")) = (self.mix, e.attr("style"), e.attr("stencil_mask")) {
            let size = self.node_size(e);
            if size[0] > 0.0 && size[1] > 0.0 {
                self.piece(xf, [0.0; 2], size, [0.0, 0.0, 1.0, 1.0], [1.0; 4], self.page.no_image_texture());
            }
            return;
        }
        let Some(style) = e.attr("style") else { return };
        // A picture with a `bold_size` is drawn nine times in black at half its opacity round its place, then itself (the synthesis's lock plates).
        if let (true, false, Some(bold)) = (self.mix, o.outlined, e.attr("bold_size").and_then(|v| v.trim().parse::<f32>().ok())) {
            let k = xf.a.hypot(xf.b);
            for dx in [-1.0, 0.0, 1.0] {
                for dy in [-1.0, 0.0, 1.0] {
                    self.image(e, Xf { x: xf.x + dx * bold * k * self.bold_scale, y: xf.y + dy * bold * k * self.bold_scale, ..xf }, &Over { outlined: true, paint: Some([0.0, 0.0, 0.0, 0.5]), ..o.clone() });
                }
            }
        }
        if style == "common_white_image" {
            // The common white (`ui/sys_white.g1t`, its first pixel of the 8 by 8): the stencil masks of the item card's lists.
            let size = self.node_size(e);
            self.piece(xf, [0.0; 2], size, [0.0, 0.0, 0.125, 0.125], [1.0; 4], WHITE);
            return;
        }
        let n = o.image_no.or_else(|| e.attr("image_no").and_then(|v| v.trim().parse().ok())).unwrap_or(0);
        let Some((texture, p, (aw, ah))) = self.page.picture(style, n).map(|(t, p, a)| (t, p, (a[0], a[1]))) else { return };
        let natural = [p.uvwh[2] / p.per_layout, p.uvwh[3] / p.per_layout];
        let size = {
            let wh = numbers(e, "size_wh");
            let (w, h) = (numbers(e, "size_w").first().copied(), numbers(e, "size_h").first().copied());
            // (the synthesis's layouts give a node both `size_w` and `size_wh`: the second is the size)
            let mut size = if self.mix && wh.len() >= 2 { [wh[0], wh[1]] } else { [w.or(wh.first().copied()).unwrap_or(natural[0]), h.or(wh.get(1).copied()).unwrap_or(natural[1])] };
            // (`parent_size_copy_w` and `_h`: the node takes its parent's size)
            for (axis, name) in ["parent_size_copy_w", "parent_size_copy_h"].into_iter().enumerate() {
                if e.attr(name) == Some("true") && self.parent[axis] > 0.0 {
                    size[axis] = self.parent[axis];
                }
            }
            if o.natural {
                size = natural;
            }
            if let Some(h) = o.size_h {
                size[1] = h;
            }
            size
        };
        let from = self.out.len();
        let c = numbers(e, "color");
        let mut color = o.paint.unwrap_or(if c.len() >= 4 { [c[0] / 255.0, c[1] / 255.0, c[2] / 255.0, c[3] / 255.0] } else { [1.0; 4] });
        color[3] *= o.alpha.unwrap_or(1.0);
        // (a picture at no opacity is not drawn: the game hides a node a fade has not reached)
        if color[3] <= 0.0 {
            return;
        }
        match p.edges {
            // A nine-slice: each margin at its size, the middle repeated (`options_view::segments`), rows then columns.
            Some([l, t, r, b]) => {
                let u = 1.0 / p.per_layout;
                // (margins that cover the picture's width, as `gen_a21_status01_list_mini`'s do, leave it whole across)
                let mut cols = if l + r >= p.uvwh[2] { vec![(0.0, size[0], 0.0, p.uvwh[2])] } else { segments(size[0], l, p.uvwh[2] - l - r, r, u) };
                // (a picture narrower than its margins keeps its middle as a piece of no width between them: `field-menu-guide` 2700, the Effects page's blanks)
                if l > 0.0 && r > 0.0 && l + r < p.uvwh[2] && cols.len() == 2 {
                    cols.insert(1, (cols[0].1, 0.0, l, 0.0));
                }
                let rows = segments(size[1], t, p.uvwh[3] - t - b, b, u);
                for row in &rows {
                    for col in &cols {
                        let src = [p.uvwh[0] + col.2, p.uvwh[1] + row.2, col.3, row.3];
                        let at = [col.0, row.0];
                        self.piece(xf, at, [col.1, row.1], [src[0] / aw, src[1] / ah, (src[0] + src[2]) / aw, (src[1] + src[3]) / ah], color, texture);
                    }
                }
            }
            None => {
                let mut uv = [p.uvwh[0] / aw, p.uvwh[1] / ah, (p.uvwh[0] + p.uvwh[2]) / aw, (p.uvwh[1] + p.uvwh[3]) / ah];
                let mut size = size;
                if let Some(share) = o.crop {
                    size[0] *= share;
                    uv[2] = uv[0] + (uv[2] - uv[0]) * share;
                }
                self.piece(xf, [0.0; 2], size, uv, color, texture);
            }
        }
        if e.attr("alpha_blend") == Some("add") {
            self.additive.extend(from..self.out.len());
        }
    }

    /// A quad of `size` layout pixels at `at` in the node `xf`.
    fn piece(&mut self, xf: Xf, at: [f32; 2], size: [f32; 2], uv: [f32; 4], color: [f32; 4], texture: usize) {
        let corners = [xf.apply(at), xf.apply([at[0] + size[0], at[1]]), xf.apply([at[0] + size[0], at[1] + size[1]]), xf.apply([at[0], at[1] + size[1]])];
        let (x0, x1) = corners.iter().fold((f32::MAX, f32::MIN), |(a, b), p| (a.min(p[0]), b.max(p[0])));
        let (y0, y1) = corners.iter().fold((f32::MAX, f32::MIN), |(a, b), p| (a.min(p[1]), b.max(p[1])));
        let upright = xf.b.abs() < 1e-6 && xf.c.abs() < 1e-6;
        // A half turn is upright but mirrored: the picture's corners swap (the game's uv runs backwards).
        let mut uv = uv;
        if upright && xf.a < 0.0 {
            uv.swap(0, 2);
        }
        if upright && xf.d < 0.0 {
            uv.swap(1, 3);
        }
        self.out.push(Quad { turn: false, spin: (!upright).then_some(Spin::Corners(corners)), rect: [x0, y0, x1, y1], uv, color, texture });
    }

    fn text(&mut self, e: &Element, xf: Xf, o: &Over) {
        let string = o.text.clone().or_else(|| e.attr("string_id").and_then(|s| self.page.string(s)).map(str::to_string));
        let Some(string) = string else { return };
        let ky = xf.c.hypot(xf.d);
        let mut at = 0;
        for (i, line) in self.wrap(e, &xf, o, &string).iter().enumerate() {
            self.run_base = at;
            self.text_line(e, Xf { y: xf.y + i as f32 * o.step.unwrap_or(LINE_STEP) * ky, ..xf }, o, line);
            at += line.chars().count();
        }
        self.run_base = 0;
    }

    /// A text's lines: the string as one, or broken at spaces so that no line (with the space it ends in) is wider than the
    /// node's `limit_width` (layout pixels).
    pub(super) fn wrap(&self, e: &Element, xf: &Xf, o: &Over, string: &str) -> Vec<String> {
        let Some(limit) = e.attr("limit_width").and_then(|v| v.trim().parse::<f32>().ok()) else { return vec![string.to_string()] };
        let glyphs = &self.page.glyphs[usize::from(e.attr("font") != Some("MainFont01"))];
        let size = e.attr("font_size").and_then(|v| v.trim().parse().ok()).or(o.size).unwrap_or(17.0);
        let s = size * xf.a.hypot(xf.b) / ATLAS_SIZE;
        let (mut lines, mut line, mut width) = (Vec::<String>::new(), String::new(), 0.0);
        for word in string.split_inclusive(' ') {
            let w: f32 = word.chars().filter_map(|c| glyphs.get(&c)).map(|g| g.advance as f32 * s).sum();
            if !line.is_empty() && width + w > limit * xf.a.hypot(xf.b) {
                lines.push(std::mem::take(&mut line));
                width = 0.0;
            }
            line.push_str(word);
            width += w;
        }
        lines.push(line);
        lines
    }

    fn text_line(&mut self, e: &Element, xf: Xf, o: &Over, string: &str) {
        let font = usize::from(e.attr("font") != Some("MainFont01"));
        let glyphs = &self.page.glyphs[font];
        // The node's scale may differ along the two axes (a label squeezed to fit).
        let (k, ky) = (xf.a.hypot(xf.b), xf.c.hypot(xf.d));
        let size = e.attr("font_size").and_then(|v| v.trim().parse().ok()).or(o.size).unwrap_or(17.0);
        let (s, sy) = (size * k / ATLAS_SIZE, size * ky / ATLAS_SIZE);
        // STAND-IN: the glyphs' spacing, as `field_menu`'s text: 0.25 px of 1080 lines for each layout size above 12 times `word_space`.
        // (A node squeezed to `kx` of the layout scale takes the cube of it: "Equip Quality" at 0.92.)
        let squeeze = (k / self.screen_k).powi(2);
        // (the item card's: a sixteenth of the layout size, fitted to its 14 pixel lines at `word_space` -1)
        let per_size = if self.card { size / 16.0 } else { 0.25 * (size - 12.0).max(0.0) };
        let spacing = e.attr("word_space").and_then(|v| v.trim().parse::<f32>().ok()).unwrap_or(0.0) * per_size * k / 1.5 * squeeze;
        let advance = |g: &Glyph| g.advance as f32 * s + spacing;
        let width: f32 = string.chars().filter_map(|c| glyphs.get(&c)).map(advance).sum();
        let [x, y] = xf.apply([0.0, 0.0]);
        let left = match e.attr("align") {
            Some("center") => x - width / 2.0,
            Some("right") => x - width,
            _ => x,
        };
        // STAND-IN: `align_v="center"` lifts the line by this share of the font size (fitted to the shoulder buttons' glyphs).
        let lift = if e.attr("align_v") == Some("center") { size * k * CENTER_LIFT } else { 0.0 };
        let top = y + TEXT_TOP * ky - lift;
        let color = {
            let c = numbers(e, "font_color");
            let c: [f32; 4] = if c.len() >= 4 {
                [c[0], c[1], c[2], c[3]]
            } else if e.attr("copy") == Some("TEXTCOLOR_BEIGE") {
                [255.0, 255.0, 228.0, 255.0]
            } else if let Some(&[r, g, b, a]) = numbers(e, "color").get(..4) {
                // (a text whose layout gives its `color` and no `font_color`: the skills list's AP)
                [r, g, b, a]
            } else {
                BROWN
            };
            o.tint.or_else(|| o.color.filter(|_| numbers(e, "font_color").len() < 4)).unwrap_or_else(|| c.map(|v| v / 255.0))
        };
        let base = self.run_base;
        let mut run = |dx: f32, dy: f32, color: [f32; 4], main: bool| {
            let mut pen = left + dx;
            for (n, g) in string.chars().enumerate().filter_map(|(n, c)| glyphs.get(&c).map(|g| (n, g))) {
                // (a text with colour runs, `Over::runs`: the glyph takes the colour of the last run that began at or before it)
                let color = if main { o.runs.iter().rev().find(|r| r.0 <= base + n).map_or(color, |r| r.1) } else { color };
                let (gx, gy, gw, gh) = (g.x as f32, g.y as f32, g.width as f32, g.height as f32);
                let (x0, y0) = (pen + g.x_offset as f32 * s, top + dy + g.y_offset as f32 * sy);
                let (x1, y1) = (x0 + gw * s, y0 + gh * sy);
                let uv = [gx / 4096.0, gy / 2048.0, (gx + gw) / 4096.0, (gy + gh) / 2048.0];
                let texture = if font == 0 { FONT01 } else { FONT00 };
                if xf.b.abs() > 1e-6 {
                    // A turned node: the glyph's corners turn about the node's place (the category's lines are 1 or 2 degrees off).
                    let (sin, cos) = (xf.b.atan2(xf.a)).sin_cos();
                    let turn = |p: [f32; 2]| [x + (p[0] - x) * cos - (p[1] - y) * sin, y + (p[0] - x) * sin + (p[1] - y) * cos];
                    let corners = [turn([x0, y0]), turn([x1, y0]), turn([x1, y1]), turn([x0, y1])];
                    let (bx0, bx1) = corners.iter().fold((f32::MAX, f32::MIN), |(a, b), p| (a.min(p[0]), b.max(p[0])));
                    let (by0, by1) = corners.iter().fold((f32::MAX, f32::MIN), |(a, b), p| (a.min(p[1]), b.max(p[1])));
                    self.out.push(Quad { turn: false, spin: Some(Spin::Corners(corners)), rect: [bx0, by0, bx1, by1], uv, color, texture });
                } else {
                    self.out.push(Quad { turn: false, spin: None, rect: [x0, y0, x1, y1], uv, color, texture });
                }
                pen += advance(g);
            }
        };
        // The outline: the text in `bold_color` at half its alpha at each of 3 x 3 offsets of `bold_size`, columns first.
        if let Some(bold) = e.attr("bold_size").and_then(|v| v.trim().parse::<f32>().ok()) {
            let b = numbers(e, "bold_color");
            let outline = if b.len() >= 4 { [b[0] / 255.0, b[1] / 255.0, b[2] / 255.0, b[3] / 255.0 * OUTLINE_ALPHA] } else { [0.0, 0.0, 0.0, OUTLINE_ALPHA] };
            for dx in [-1.0, 0.0, 1.0] {
                for dy in [-1.0, 0.0, 1.0] {
                    run(dx * bold * k * self.bold_scale, dy * bold * k * self.bold_scale, outline, false);
                }
            }
        }
        run(0.0, 0.0, color, true);
    }
}

/// The effect lines' text place in its slider (the layout's (-1, 2); the capture's lines are a layout pixel further right: STAND-IN).
const EFFECT_TEXT_AT: [f32; 2] = [0.0, 2.0];

/// The centre of the equippable faces in the card's `list` node (the `center` node's place less the list's), their size and the step
/// from one to the next: STAND-IN, 79 pixels of 1080 lines between the faces of both cards (the layout says 41.33 for its nine places).
const FACE_CENTER_X: f32 = 180.0;
const FACE_SIZE: f32 = 42.0;
const FACE_STEP: f32 = 52.667;

/// The node `parts` name below `first`, each the first of its name under the one before it.
fn descend<'a>(first: &'a Element, parts: std::str::Split<'_, char>) -> Option<&'a Element> {
    let mut at = first;
    for part in parts {
        at = at.children.iter().find_map(|c| find(c, part))?;
    }
    Some(at)
}

/// A cell cursor's colours at `t` frames of its loop (`anm_common_cursor_base` and `_add_75`, 120 frames up and down): the picture's, the
/// added glow's (its animation's times the picture's) and the share of the way to teal. The game's bytes: each animation's colour rounds
/// to a byte and their product is cut down to one.
fn cursor_colors(t: f32) -> ([f32; 3], [f32; 3], f32) {
    let t = t.rem_euclid(CURSOR_LOOP);
    let f = 1.0 - (t - CURSOR_LOOP / 2.0).abs() / (CURSOR_LOOP / 2.0);
    let lerp = |to: [f32; 3]| [255.0 + (to[0] - 255.0) * f, 255.0 + (to[1] - 255.0) * f, 255.0 + (to[2] - 255.0) * f].map(|v| v.round().clamp(0.0, 255.0));
    let (base, add) = (lerp([58.0, 205.0, 180.0]), lerp([58.0, 205.0, 205.0]));
    let both = [0, 1, 2].map(|i| (base[i] * add[i] / 255.0).floor() / 255.0);
    (base.map(|v| v / 255.0), both, f)
}

fn numbers_of(attributes: &[(String, String)], name: &str) -> Option<Vec<f32>> {
    let v = attributes.iter().find(|(k, _)| k == name)?;
    Some(v.1.split(',').filter_map(|t| t.trim().parse().ok()).collect())
}

fn find_with_children<'a>(e: &'a Element, name: &str) -> Option<&'a Element> {
    if e.attr("name") == Some(name) && !e.children.is_empty() {
        return Some(e);
    }
    e.children.iter().find_map(|c| find_with_children(c, name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_slide_in_is_fast_first_and_whole_after_30_frames() {
        // `field-menu-status` 2330 and 2340 (A on 2320): 179/255 and 246/255 of the opacity, 8.9 and 1.1 layout pixels from the end.
        let (a, b) = (Slide::in_from(10.0, [0.0, 30.0]), Slide::in_from(20.0, [0.0, 30.0]));
        assert_eq!((a.alpha * 255.0).round(), 179.0);
        assert!((a.dy - 8.89).abs() < 0.01 && (b.dy - 1.11).abs() < 0.01);
        assert_eq!((b.alpha * 255.0).round(), 246.0);
        let done = Slide::in_from(30.0, [-40.0, 0.0]);
        assert_eq!((done.alpha, done.dx), (1.0, 0.0));
        assert_eq!(Slide::in_from(-3.0, [0.0, 30.0]).alpha, 0.0);
    }

    #[test]
    fn the_skills_cursor_glows_up_and_down_in_192_frames() {
        // `field-menu-status` 2400, 80 frames after A: 83/255.
        assert_eq!((cursor_glow(80.0) * 255.0).round(), 83.0);
        assert_eq!(cursor_glow(0.0), 0.0);
        assert_eq!((cursor_glow(96.0) * 255.0).round(), 100.0);
        assert_eq!(cursor_glow(192.0 + 80.0), cursor_glow(80.0));
    }

    #[test]
    fn blink_goes_down_and_up_in_120_frames() {
        assert_eq!(blink(0.0), 1.0);
        assert_eq!(blink(60.0), 0.0);
        assert_eq!(blink(120.0), 1.0);
        // `field-menu-equip` 2410: 111 frames in, 217 of 255.
        assert_eq!(blink(111.0), 217.0 / 255.0);
        assert_eq!(blink(231.0), blink(111.0));
    }

    #[test]
    fn a_turned_node_keeps_its_centre() {
        let root = Xf { a: 1.0, b: 0.0, c: 0.0, d: 1.0, x: 0.0, y: 0.0 };
        // A 10 x 10 picture turned 90 degrees about its centre (5, 5): the corner moves to (10, 0).
        let (turn, c) = (90.0f32, [5.0f32, 5.0]);
        let (sin, cos) = turn.to_radians().sin_cos();
        let at = [c[0] - (cos * c[0] - sin * c[1]), c[1] - (sin * c[0] + cos * c[1])];
        let xf = root.node(at, turn, [1.0; 2]);
        let centre = xf.apply(c);
        assert!((centre[0] - 5.0).abs() < 1e-4 && (centre[1] - 5.0).abs() < 1e-4);
        let corner = xf.apply([0.0, 0.0]);
        assert!((corner[0] - 10.0).abs() < 1e-4 && corner[1].abs() < 1e-4);
    }

    #[test]
    fn a_wrap_picture_repeats_its_middle() {
        // A line 486 pixels wide (324 layout pixels at 1.5): 25 pixel caps and the middle in tiles of 140.
        let cols = segments(324.0, 50.0, 100.0, 50.0, 0.5);
        assert_eq!(cols.first().map(|c| c.1), Some(25.0));
        assert_eq!(cols.last().map(|c| c.1), Some(25.0));
        assert!(cols.iter().map(|c| c.1).sum::<f32>() - 324.0 < 0.01);
    }

    #[test]
    fn a_clone_path_names_the_first_node_of_each_name_below_the_one_before() {
        let doc = table::parse_str(r#"<root><node name="clone_targets"><node name="effect"><image name="effect"><node name="list"><node name="node"><text name="a"/></node></node></image></node></node></root>"#).unwrap();
        let found = descend(find(&doc, "clone_targets").unwrap(), "effect/node".split('/')).unwrap();
        assert_eq!(found.children.first().and_then(|c| c.attr("name")), Some("a"));
        assert!(descend(find(&doc, "clone_targets").unwrap(), "effect/none".split('/')).is_none());
    }

    #[test]
    fn the_cursor_cell_colours_are_the_games_bytes_at_the_capture_phases() {
        // `field-menu-pages` 2110: the picture at (88, 212, 191), the added glow at (30, 176, 158) with 162 of opacity.
        let (base, both, f) = cursor_colors(68.99);
        assert_eq!(base.map(|v| (v * 255.0).round() as u32), [88, 212, 191]);
        assert_eq!(both.map(|v| (v * 255.0).round() as u32), [30, 176, 158]);
        assert_eq!((191.0 * f).round() as u32, 162);
        // 2080: (127, 223, 206) and (63, 195, 180).
        let (base, both, _) = cursor_colors(38.99);
        assert_eq!(base.map(|v| (v * 255.0).round() as u32), [127, 223, 206]);
        assert_eq!(both.map(|v| (v * 255.0).round() as u32), [63, 195, 180]);
    }

    #[test]
    fn a_slide_over_forty_frames_is_half_way_in_the_first_nine() {
        // The Container page's parts on 2050, nine frames in: 0.533 of their opacity and 28 of the 60 pixels of their way left.
        let s = Slide::over(9.0, 40.0, [-40.0, 0.0]);
        assert!((s.alpha - 0.5333).abs() < 0.01, "{}", s.alpha);
        assert!((s.dx + 40.0 * (1.0 - s.alpha)).abs() < 1e-4);
        assert_eq!(Slide::over(40.0, 40.0, [-40.0, 0.0]).dx, 0.0);
    }

    #[test]
    fn the_equipment_slots_going_out_match_2510_and_2520() {
        // 10 and 20 frames after B: 149/255 and 42/255 of the opacity, 4.13 and 14.82 layout pixels down.
        let (a, b) = (Slide::slots_out(10.0), Slide::slots_out(20.0));
        assert_eq!([a.alpha, b.alpha].map(|v| (v * 255.0).round() as u32), [149, 43]);
        assert!((a.dy - 4.13).abs() < 0.02 && (b.dy - 14.82).abs() < 0.02, "{} {}", a.dy, b.dy);
        assert_eq!(Slide::slots_out(24.0).alpha, 0.0);
    }

    #[test]
    fn the_items_cards_tilt_about_the_centre_node_to_the_captured_corners() {
        // `field-menu-guide` 2400, 1080 lines: card 1's first piece, upright at (179.535, 229.425), is drawn with its top left corner at
        // (310.598, 188.700); card 2's (506.235, 229.425) with its top right (767.535, 229.425) at (594.184, 214.527).
        let k = 1.5;
        let tilted = |centre: [f32; 2], tilt: (f32, f32), corner: [f32; 2]| {
            let root = Xf { a: k, b: 0.0, c: 0.0, d: k, x: 0.0, y: 0.0 };
            let me = root.node(centre, tilt.0, [1.0, tilt.1]);
            // the corner in the node's own layout pixels: its offset from the node's screen place, over the scale
            let [cx, cy] = root.apply(centre);
            me.apply([(corner[0] - cx) / k, (corner[1] - cy) / k])
        };
        let [x, y] = tilted([206.0, 262.0], CARD_TILTS[0], [179.535, 229.425]);
        assert!((x - 310.598).abs() < 0.05 && (y - 188.700).abs() < 0.05, "{x} {y}");
        let [x, y] = tilted([206.0 + 217.8, 262.0], CARD_TILTS[1], [767.535, 229.425]);
        assert!((x - 594.184).abs() < 0.05 && (y - 214.527).abs() < 0.05, "{x} {y}");
    }

    #[test]
    fn the_animation_clock_loses_a_frame_between_2260_and_2290_and_again_by_2430() {
        let behind = |frame: f32| frame - animation_clock(frame);
        assert_eq!(behind(2290.0) - behind(2260.0), 1.0);
        assert_eq!(behind(2430.0) - behind(2420.0), 1.0);
        assert_eq!(behind(2420.0) - behind(2290.0), 0.0);
    }

    #[test]
    fn keyed_curves_are_linear_between_their_keys_and_step_on_a_repeated_frame() {
        let keys = [(0.0, 0.0), (20.0, 0.0), (50.0, 1.0), (105.0, 1.0), (105.0, 0.0)];
        assert_eq!(keyed(&keys, 10.0), 0.0);
        assert!((keyed(&keys, 30.0) - 1.0 / 3.0).abs() < 1e-6);
        assert_eq!(keyed(&keys, 100.0), 1.0);
        assert_eq!(keyed(&keys, 105.0), 0.0, "two keys on one frame make a step");
        assert_eq!(keyed(&keys, 500.0), 0.0);
    }

    #[test]
    fn the_level_up_plate_rises_on_the_captures_spline() {
        // `synth-atelier` 4130 to 4190: the plate's top over its rest, in layout pixels (the quads' y over 1.5 less 224).
        for (frame, want) in [(30.0, 71.2), (40.0, -24.4), (50.0, -47.94), (60.0, 7.75), (70.0, 18.81), (80.0, -13.0), (90.0, -5.26), (100.0, 0.0)] {
            let got = spline(&PLATE_RISE, frame);
            assert!((got - want).abs() < 0.01, "{frame}: {got} against {want}");
        }
    }

    #[test]
    fn the_whiteout_and_the_cards_rise_follow_the_capture() {
        // 3830 to 3880 (the frames since 3819): the full-screen white's opacity in bytes, 252, 146, 74, 31, 9 and 1.
        for (frames, byte) in [(11.0, 252.0), (21.0, 146.0), (31.0, 74.0), (41.0, 31.0), (51.0, 9.0), (61.0, 1.0)] {
            assert!((whiteout_all(frames) * 255.0 - byte).abs() < 1.0, "{frames}");
        }
        // The card's two rises at 3840 to 3870: 34, 22.2, 14.8 and 10.2 layout pixels low (the capture's 51, 33.3, 22.1 and 15.2 pixels over 1.5).
        let rise = |clock: f32| CARD_RISE * (1.0 - out_cubic((clock - 20.0) / 120.0));
        for (clock, want) in [(21.0, 34.1), (31.0, 22.3), (41.0, 14.8), (51.0, 10.2)] {
            let got = rise(clock) + rise(clock * PANEL_SPEED);
            assert!((got - want).abs() < 0.2, "{clock}: {got} against {want}");
        }
    }
}
