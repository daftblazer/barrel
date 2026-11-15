//! The game's yes / no question (the recording `colseit-arrival-60hz.mkv` at 1:05, "Return to
//! World Map?" on an area map's Back; the executable's string at `0x140949850`, asked through
//! the question window class `0x14017e6a0`): the system message window (layout 701 at
//! (540, 840): its piece is `a15_common.g1t` texture 4's (2, 530)–(758, 738), which the game
//! stretches to 900 × 222 keeping its ends) with the question at 701's text node, and over its
//! top edge the choice box (the neighbours' labels go while it asks): texture 23's ornate panel (1, 239)–(169, 383) stretched to
//! (1124, 676)–(1388, 862) keeping its corners, dark bars over and under the rows, the lit row
//! in texture 3's yellow band that fades to the right ((460, 673)–(712, 716)), the blue feather (texture 16) at its left, Yes then No.
//!
//! Not yet the game's: the bars (flat fills of the recording's colour; their piece is not
//! found), the rows' rules, the feather's place (by eye) and its motion, the text colours (the
//! recording's dark brown, not 701's node colour), the question's layout ids (701 is read off
//! the recording's window place; the class's own reads are not traced).

use crate::play::hud::{Element, nine};

const COMMON: &str = "a15_common.g1t";
const WINDOW: u32 = 701;
/// Dark brown, as the recording's question and choices read.
const INK: [u8; 4] = [0xff, 0x3c, 0x28, 0x14];
const BAR: [f32; 4] = [0.54, 0.50, 0.37, 1.0];

/// The question window asking `question`, Yes lit when `yes`.
pub fn elements(pe: &barrel_formats::pe::Pe, question: &str, yes: bool) -> Vec<Element> {
    let mut out = Vec::new();
    // The window: its ends whole, its middle stretched across.
    let src = [2.0, 530.0, 758.0, 738.0];
    let rect = [542, 843, 1442, 1066];
    let end = 64.0;
    out.push(Element::Slice { pack: COMMON.into(), index: 4, src: [src[0], src[1], src[0] + end, src[3]], rect: [rect[0], rect[1], rect[0] + end as i32, rect[3]] });
    out.push(Element::Slice { pack: COMMON.into(), index: 4, src: [src[0] + end, src[1], src[2] - end, src[3]], rect: [rect[0] + end as i32, rect[1], rect[2] - end as i32, rect[3]] });
    out.push(Element::Slice { pack: COMMON.into(), index: 4, src: [src[2] - end, src[1], src[2], src[3]], rect: [rect[2] - end as i32, rect[1], rect[2], rect[3]] });
    if let Some(t) = barrel_escha::data::layout::placed(pe, WINDOW).iter().filter_map(|p| barrel_escha::data::layout::text(pe, p)).find(|t| t.filled_in()) {
        out.push(Element::Label(barrel_escha::data::layout::Text { text: question.into(), colour: INK, ..t }));
    }
    // The choice box over the window's top edge.
    out.extend(nine(COMMON, 23, [1.0, 239.0, 169.0, 383.0], [1124, 676, 1388, 862], 24.0));
    out.push(Element::Fill { rect: [1136, 700, 1376, 722], colour: BAR });
    out.push(Element::Fill { rect: [1136, 822, 1376, 848], colour: BAR });
    let lit = if yes { 728 } else { 774 };
    out.push(Element::Slice { pack: COMMON.into(), index: 3, src: [460.0, 673.0, 712.0, 716.0], rect: [1136, lit, 1376, lit + 42] });
    for (k, word) in ["Yes", "No"].into_iter().enumerate() {
        out.push(Element::Label(barrel_escha::data::layout::Text { at: [1172, 732 + 46 * k as i32], size: 36.0, colour: INK, text: word.into(), kind: 21 }));
    }
    out.push(Element::Slice { pack: COMMON.into(), index: 16, src: [27.0, 5.0, 88.0, 92.0], rect: [1096, lit - 52, 1144, lit + 16] });
    out
}
