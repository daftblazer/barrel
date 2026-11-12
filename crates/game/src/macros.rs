//! The command macros in Escha & Logy DX's Squirrel scripts (`Script/macro_*.nut`; `docs/
//! events.md`, "Script macros"): each function fills an array with event command words and
//! operands (`local temp = [ ECT_..., operand, ... ];`), copies it and runs it like an event's
//! action. The arrays are literal, so they are read here without a Squirrel VM: a macro's
//! words are command names, other named constants, the function's parameters and numbers.

use std::collections::HashMap;

/// One entry of a macro's array.
#[derive(Clone, Debug, PartialEq)]
pub enum Token {
    Number(i64),
    /// A command or constant name (`ECT_*`, `MOTION_*`, ...).
    Name(String),
    /// The function's parameter of this index.
    Param(usize),
}

/// A macro function: its name, its parameters and its array.
#[derive(Clone, Debug, PartialEq)]
pub struct Macro {
    pub name: String,
    pub params: Vec<String>,
    pub words: Vec<Token>,
}

/// Something a macro names that is not known (a constant missing from the tables).
#[derive(Debug, PartialEq)]
pub struct Unknown(pub String);

/// Removes `//` and `/* */` comments.
fn strip_comments(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    let mut rest = src;
    while !rest.is_empty() {
        if let Some(r) = rest.strip_prefix("//") {
            rest = r.find('\n').map_or("", |i| &r[i..]);
        } else if let Some(r) = rest.strip_prefix("/*") {
            rest = r.find("*/").map_or("", |i| &r[i + 2..]);
        } else {
            let c = rest.chars().next().unwrap();
            out.push(c);
            rest = &rest[c.len_utf8()..];
        }
    }
    out
}

/// The macros in a script's source (decoded to text; the files are Shift-JIS, but everything
/// outside the comments is ASCII).
pub fn read(src: &str) -> Vec<Macro> {
    let src = strip_comments(src);
    let mut macros = Vec::new();
    let mut rest = src.as_str();
    while let Some(at) = rest.find("function ") {
        rest = &rest[at + "function ".len()..];
        let Some(open) = rest.find('(') else { break };
        let name = rest[..open].trim().to_string();
        let Some(close) = rest.find(')') else { break };
        let params: Vec<String> = rest[open + 1..close].split(',').map(|p| p.trim().to_string()).filter(|p| !p.is_empty()).collect();
        // The function's array, before the next function.
        let body_end = rest.find("function ").unwrap_or(rest.len());
        let body = &rest[close..body_end];
        if let (Some(a), Some(b)) = (body.find('['), body.find(']')) {
            if a < b {
                let words = body[a + 1..b]
                    .split(',')
                    .map(str::trim)
                    .filter(|t| !t.is_empty())
                    .map(|t| match t.parse::<i64>() {
                        Ok(n) => Token::Number(n),
                        Err(_) => params.iter().position(|p| p == t).map_or_else(|| Token::Name(t.to_string()), Token::Param),
                    })
                    .collect();
                macros.push(Macro { name, params, words });
            }
        }
        rest = &rest[body_end..];
    }
    macros
}

/// Escha's names for the words: the command names, the other constants and the motions (by
/// their names in the executable's motion table, `MOTION_` + name).
pub struct Names {
    values: HashMap<String, u32>,
}

impl Names {
    pub fn escha(motions: &[String]) -> Names {
        let mut values: HashMap<String, u32> = crate::sq_consts::ESCHA.iter().map(|&(n, v)| (n.to_string(), v as u32)).collect();
        values.extend(crate::names::ESCHA.iter().map(|&(w, n)| (n.to_string(), w)));
        values.extend(motions.iter().enumerate().map(|(k, m)| (format!("MOTION_{m}"), k as u32)));
        Names { values }
    }
}

impl Names {
    /// A name's value.
    /// Every name and its value.
    pub fn all(&self) -> impl Iterator<Item = (&str, u32)> {
        self.values.iter().map(|(n, &v)| (n.as_str(), v))
    }

    pub fn value(&self, name: &str) -> Option<u32> {
        self.values.get(name).copied()
    }
}

impl Macro {
    /// The macro's words with `args` for its parameters, as an event action's words.
    pub fn words(&self, names: &Names, args: &[u32]) -> Result<Vec<u32>, Unknown> {
        self.words
            .iter()
            .map(|t| match t {
                Token::Number(n) => Ok(*n as i32 as u32),
                Token::Param(k) => args.get(*k).copied().ok_or_else(|| Unknown(self.params[*k].clone())),
                Token::Name(n) => names.values.get(n).copied().ok_or_else(|| Unknown(n.clone())),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_macro_reads_and_resolves() {
        let src = "//\u{250f}\u{250f} comment\n/*! @brief test */\nfunction forceEncount(s32PlayerTag, s32TargetCharaTag)\n{\n\tlocal temp = [ \n\t\tECT_FIELDMAP_CHARA_SET_EVENT_MODE, s32PlayerTag,\n\t\tECT_MACRO_TIME_WAIT, 50, // half a second\n\t\tECT_CHARA_MOTION_SET, s32TargetCharaTag, MOTION_EX10, 0, -20,\n\t\tECT_END\n\t];\n\tforeach (idx, val in temp) { SQ.copyProgramMacro(val, idx); }\n}\nfunction other() { return; }\n";
        let m = read(src);
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].name, "forceEncount");
        assert_eq!(m[0].params, ["s32PlayerTag", "s32TargetCharaTag"]);
        assert_eq!(m[0].words[1], Token::Param(0));
        assert_eq!(m[0].words[3], Token::Number(50));
        let motions: Vec<String> = (0..12).map(|k| if k == 11 { "EX10".into() } else { format!("M{k}") }).collect();
        let w = m[0].words(&Names::escha(&motions), &[10000, 10001]).unwrap();
        assert_eq!(w[1], 10000);
        assert_eq!(w[2], 0x7000_0001);
        assert_eq!(&w[5..9], &[10001, 11, 0, (-20i32) as u32]);
        assert_eq!(*w.last().unwrap(), 0x7f00_0000);
        // Without its arguments a parameter is unknown.
        assert_eq!(m[0].words(&Names::escha(&motions), &[1]), Err(Unknown("s32TargetCharaTag".into())));
    }
}
