//! The system text tables (`Saves[_EN]/systemMessage/`; the Japanese copies are Shift-JIS, the
//! English ones UTF-8 with a byte-order mark), each read by row number:
//!
//! - `SysMess.xml`: 409 lines the characters say outside events (synthesis prompts and the
//!   like), each with its speaker (`CharaTag`, `NameTag`), a `BucTag`, a balloon mode, a voice
//!   number (-1 none) and the text.
//! - `SysInfo.xml`: 118 plain lines (`Text`), such as the menus' descriptions.
//! - `Menu_Line_Help.xml`: 160 button-help lines (`LineHelp`) with `<IMnn>` button icon tags.
//!
//! Two more text tables are read here too: `poster/PosterData.xml` (the atelier's reference
//! posters: a name, an index and the text, `<CR>` for line breaks) and
//! `subject/SubjectTalkData.xml` (what Marion says around the assignments: name and character
//! tags, a voice number and the text).

use crate::table::Element;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SysMess {
    pub chara: String,
    pub name: String,
    pub buc: String,
    pub balloon: String,
    pub voice: i32,
    pub text: String,
}

pub fn sys_mess(t: &Element) -> Vec<SysMess> {
    let a = |e: &Element, n: &str| e.attr(n).unwrap_or("").to_string();
    t.children
        .iter()
        .filter(|e| e.name == "SysMess")
        .map(|e| SysMess {
            chara: a(e, "CharaTag"),
            name: a(e, "NameTag"),
            buc: a(e, "BucTag"),
            balloon: a(e, "BalloonMode"),
            voice: e.attr("VoiceNo").and_then(|v| v.trim().parse().ok()).unwrap_or(-1),
            text: a(e, "Text"),
        })
        .collect()
}

/// The `Text` (`SysInfo.xml`) or `LineHelp` (`Menu_Line_Help.xml`) of each row, in order.
pub fn lines(t: &Element, attribute: &str) -> Vec<String> {
    t.children.iter().map(|e| e.attr(attribute).unwrap_or("").to_string()).collect()
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Poster {
    pub name: String,
    pub index: i32,
    pub text: String,
}

pub fn posters(t: &Element) -> Vec<Poster> {
    t.children
        .iter()
        .filter(|e| e.name == "PosterData")
        .map(|e| Poster {
            name: e.attr("posterName").unwrap_or("").to_string(),
            index: e.attr("index").and_then(|v| v.trim().parse().ok()).unwrap_or(-1),
            text: e.attr("text").unwrap_or("").to_string(),
        })
        .collect()
}

/// A line of `SubjectTalkData.xml`, as [`SysMess`] without the balloon fields.
pub fn subject_talk(t: &Element) -> Vec<SysMess> {
    let a = |e: &Element, n: &str| e.attr(n).unwrap_or("").to_string();
    t.children
        .iter()
        .filter(|e| e.name == "SubjectTalkData")
        .map(|e| SysMess {
            chara: a(e, "CharaTag"),
            name: a(e, "NameTag"),
            buc: String::new(),
            balloon: String::new(),
            voice: e.attr("VoiceNo").and_then(|v| v.trim().parse().ok()).unwrap_or(-1),
            text: a(e, "Text"),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn reads_rows() {
        let x = br#"<Root><SysMess CharaTag="CHARA_ESCHA" NameTag="NAME_ESCHA" BucTag="BUC_NONE" BalloonMode="BALLOON_MODE_SBUC" VoiceNo="-1" Text="Hmm"/></Root>"#;
        let m = super::sys_mess(&crate::table::parse(x).unwrap());
        assert_eq!((m[0].chara.as_str(), m[0].voice, m[0].text.as_str()), ("CHARA_ESCHA", -1, "Hmm"));
        let h = br#"<Root><Menu_Line_Help LineHelp="&lt;IM09&gt;Select"/></Root>"#;
        assert_eq!(super::lines(&crate::table::parse(h).unwrap(), "LineHelp"), ["<IM09>Select"]);
    }

    #[test]
    fn posters_and_talk() {
        let p = br#"<Root><PosterData posterName="Colseit" index="0" text="a&lt;CR&gt;b"/></Root>"#;
        let p = super::posters(&crate::table::parse(p).unwrap());
        assert_eq!((p[0].index, p[0].text.as_str()), (0, "a<CR>b"));
        let t = br#"<Root><SubjectTalkData NameTag="NAME_MARION" CharaTag="CHARA_MARION" VoiceNo="11083" Text="x"/></Root>"#;
        assert_eq!(super::subject_talk(&crate::table::parse(t).unwrap())[0].voice, 11083);
    }
}
