//! The prompt before the synthesis starts: the game's balloon select (`uil_mt_balloon_select.xml`'s window and cursor, `uil_lf_balloon_select.xml`'s line) with
//! "OK" and "Start Over", the same box the Settings screen's leave dialog asks with (`options_view::ask`), here laid out from the layout by the walker.

use super::recipe_view::with_attr;
use super::*;

/// The "OK" string (`str_menu_define`); "Start Over" is 9961511.
pub(super) const OK_STRING: u32 = 4_194_318;
const START_OVER_STRING: u32 = 9_961_511;
/// Where the box stands on the screen and how wide it is (layout pixels).
// STAND-IN: read off the capture's window quads (`synth-atelier` 3510: x 810 to 1147.5 and y 600 to 781.5 of 1080 lines, a nine-slice of the layout's 121 height); the
// layout's own width is 215 and the game widens the window to its longest line.
const AT: [f32; 2] = [540.0, 400.0];
const WIDTH: f32 = 225.0;
/// The cursor's width is the window's less this (the layout's `size_copy_w_offset`).
const CURSOR_MARGIN: f32 = 54.0;
/// The rows' pitch (the line format's height: `ASK_ROW_PITCH` of the options' box).
const ROW_PITCH: f32 = 34.0;

/// The box as it is on a frame.
#[derive(Clone, Copy)]
pub struct Prompt {
    /// The whole box's opacity (it fades in and out over 15 frames).
    pub alpha: f32,
    /// The frames of the cursor's loop (`anm_common_cursor_base`) it has played.
    pub age: f32,
    /// The cursor's glow is drawn (it is not once A has chosen: `synth-atelier` 3520 and 3530 have the base only).
    pub glow: bool,
}

impl Page {
    /// The window, the cursor on "OK" and the two words, faded by `prompt.alpha`.
    pub(super) fn prompt_box(&self, w: &mut Walker, root: &Xf, prompt: &Prompt) {
        let docs = &self.mix_docs;
        let Some(window) = find(&docs.select, "__uimenu_window") else { return };
        let Some(line) = find(&docs.select_line, "text") else { return };
        let window = with_attr(window, "__uimenu_window", "size_wh", &format!("{WIDTH}, 121"));
        let window = with_attr(&with_attr(&window, "base", "size_w", &(WIDTH - CURSOR_MARGIN).to_string()), "add", "size_w", &(WIDTH - CURSOR_MARGIN).to_string());
        let (base, both, f) = cursor_colors(prompt.age);
        let origin = root.node(AT, 0.0, [1.0; 2]);
        let words = [self.strings.get(&OK_STRING).cloned(), self.strings.get(&START_OVER_STRING).cloned()];
        let over = |p: &[&str]| -> Over {
            match p {
                ["__uimenu_window", "__uimenu_scroll_bar", ..] => Over { hide: true, ..Default::default() },
                ["__uimenu_window", "__uimenu_body", "__uimenu_cursor", "__uimenu_cursor_inactive"] => Over { hide: true, ..Default::default() },
                ["__uimenu_window", "__uimenu_body", "__uimenu_cursor", "__uimenu_cursor_active", "base"] => Over { paint: Some([base[0], base[1], base[2], 1.0]), ..Default::default() },
                ["__uimenu_window", "__uimenu_body", "__uimenu_cursor", "__uimenu_cursor_active", "add"] if !prompt.glow => Over { hide: true, ..Default::default() },
                ["__uimenu_window", "__uimenu_body", "__uimenu_cursor", "__uimenu_cursor_active", "add"] => Over { paint: Some([both[0], both[1], both[2], (191.0 * f).round() / 255.0]), ..Default::default() },
                _ => Over::default(),
            }
        };
        w.slid(Slide { alpha: prompt.alpha, dx: 0.0, dy: 0.0 }, |w| {
            w.walk(&window, origin, &mut Vec::new(), &over, &HashMap::new());
            let body = origin.node(pair(find(&window, "__uimenu_body").unwrap_or(&window), "pos", [0.0; 2]), 0.0, [1.0; 2]);
            let line_at = pair(find(&docs.select_line, "lf_balloon_select").unwrap_or(line), "pos", [0.0; 2]);
            for (n, word) in words.into_iter().enumerate() {
                let text_at = pair(line, "pos", [0.0; 2]);
                let at = body.node([line_at[0] + text_at[0], line_at[1] + text_at[1] + ROW_PITCH * n as f32], 0.0, [1.0; 2]);
                let over = |p: &[&str]| -> Over { if p.len() == 1 { Over { text: word.clone(), show: true, ..Default::default() } } else { Over::default() } };
                w.walk(line, at, &mut Vec::new(), &over, &HashMap::new());
            }
        });
    }
}
