//! The games' XML tables (`Saves*/**/*.xml`): items, recipes, shops, quests, characters and
//! the rest of the game data.
//!
//! The files are XML only loosely, and the games' own reader accepts all of them, so this
//! one is lenient in the same ways:
//! - The encoding is UTF-8 when the file starts with a byte-order mark or is valid UTF-8,
//!   and Shift-JIS (code page 932) otherwise, whatever the declaration says. The translated
//!   tables declare Shift-JIS but are UTF-8 with a BOM, and some Shift-JIS files have UTF-8
//!   comments.
//! - Attributes may follow each other without white space (`a="1"b="2"`).
//! - `&nbsp;` is accepted; an unknown entity is kept as written.
//!
//! The result is a tree of elements with their attributes in file order. The tables have no
//! text content, so text between elements is dropped.

use crate::Error;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Element {
    pub name: String,
    pub attributes: Vec<(String, String)>,
    pub children: Vec<Element>,
}

impl Element {
    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attributes.iter().find(|(k, _)| k == name).map(|(_, v)| v.as_str())
    }
}

/// Decode a table file's bytes to text, choosing the encoding as described above.
pub fn decode_text(bytes: &[u8]) -> String {
    if let Some(rest) = bytes.strip_prefix(b"\xef\xbb\xbf") {
        return String::from_utf8_lossy(rest).into_owned();
    }
    match std::str::from_utf8(bytes) {
        Ok(s) => s.to_owned(),
        Err(_) => encoding_rs::SHIFT_JIS.decode_without_bom_handling(bytes).0.into_owned(),
    }
}

fn unescape(raw: &str) -> String {
    if !raw.contains('&') {
        return raw.to_owned();
    }
    let mut out = String::with_capacity(raw.len());
    let mut rest = raw;
    while let Some(at) = rest.find('&') {
        out.push_str(&rest[..at]);
        rest = &rest[at..];
        let Some(end) = rest.bytes().take(12).position(|b| b == b';') else {
            out.push('&');
            rest = &rest[1..];
            continue;
        };
        let entity = &rest[1..end];
        let ch = match entity {
            "lt" => Some('<'),
            "gt" => Some('>'),
            "amp" => Some('&'),
            "apos" => Some('\''),
            "quot" => Some('"'),
            "nbsp" => Some('\u{a0}'),
            _ => {
                let code = if let Some(hex) = entity.strip_prefix("#x").or(entity.strip_prefix("#X")) {
                    u32::from_str_radix(hex, 16).ok()
                } else if let Some(dec) = entity.strip_prefix('#') {
                    dec.parse().ok()
                } else {
                    None
                };
                code.and_then(char::from_u32)
            }
        };
        match ch {
            Some(c) => {
                out.push(c);
                rest = &rest[end + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

struct Parser<'a> {
    s: &'a str,
    pos: usize,
    /// Names of the elements currently open, outermost first.
    open: Vec<String>,
}

impl<'a> Parser<'a> {
    fn rest(&self) -> &'a str {
        &self.s[self.pos..]
    }

    fn skip_ws(&mut self) {
        let r = self.rest();
        self.pos += r.len() - r.trim_start().len();
    }

    fn skip_past(&mut self, pat: &str) -> Result<(), Error> {
        let at = self.rest().find(pat).ok_or(Error::Corrupt("xml: unterminated markup"))?;
        self.pos += at + pat.len();
        Ok(())
    }

    fn name(&mut self) -> Result<String, Error> {
        let r = self.rest();
        let len = r
            .find(|c: char| c.is_whitespace() || matches!(c, '=' | '/' | '>' | '"' | '\''))
            .unwrap_or(r.len());
        if len == 0 {
            return Err(Error::Corrupt("xml: expected a name"));
        }
        self.pos += len;
        Ok(r[..len].to_owned())
    }

    /// Skip text, comments, declarations and processing instructions up to the next tag.
    /// Returns false at the end of the input.
    fn to_next_tag(&mut self) -> Result<bool, Error> {
        loop {
            match self.rest().find('<') {
                None => {
                    self.pos = self.s.len();
                    return Ok(false);
                }
                Some(at) => self.pos += at,
            }
            let r = self.rest();
            if r.starts_with("<!--") {
                self.skip_past("-->")?;
            } else if r.starts_with("<?") {
                self.skip_past("?>")?;
            } else if r.starts_with("<!") {
                self.skip_past(">")?;
            } else {
                return Ok(true);
            }
        }
    }

    /// Parse an element starting at `<`.
    fn element(&mut self, depth: usize) -> Result<Element, Error> {
        if depth > 64 {
            return Err(Error::Corrupt("xml: nested too deeply"));
        }
        self.pos += 1;
        let mut e = Element { name: self.name()?, ..Default::default() };
        loop {
            self.skip_ws();
            let r = self.rest();
            if r.starts_with("/>") {
                self.pos += 2;
                return Ok(e);
            }
            if r.starts_with('>') {
                self.pos += 1;
                break;
            }
            if r.is_empty() {
                return Err(Error::Corrupt("xml: tag not closed"));
            }
            let key = self.name()?;
            self.skip_ws();
            if !self.rest().starts_with('=') {
                // A bare attribute name: keep it with an empty value.
                e.attributes.push((key, String::new()));
                continue;
            }
            self.pos += 1;
            self.skip_ws();
            let quote = self.rest().chars().next().ok_or(Error::Corrupt("xml: attribute value missing"))?;
            if quote != '"' && quote != '\'' {
                // A value missing its opening quote (Ayesha's `L_POINT_09_C.param.xml` has
                // `fogDistanceMin=-60.0"`): taken to a quote, whitespace or the tag's end, the
                // stray closing quote dropped.
                let r = self.rest();
                let end = r.find(|c: char| matches!(c, '"' | '\'' | '>' | '/') || c.is_whitespace()).ok_or(Error::Corrupt("xml: attribute value not closed"))?;
                if end == 0 {
                    return Err(Error::Corrupt("xml: attribute value not quoted"));
                }
                let value = unescape(&r[..end]);
                let stray = matches!(r[end..].chars().next(), Some('"' | '\''));
                self.pos += end + usize::from(stray);
                e.attributes.push((key, value));
                continue;
            }
            self.pos += 1;
            let end = self.rest().find(quote).ok_or(Error::Corrupt("xml: attribute value not closed"))?;
            let value = unescape(&self.rest()[..end]);
            self.pos += end + 1;
            e.attributes.push((key, value));
        }
        loop {
            if !self.to_next_tag()? {
                return Err(Error::Corrupt("xml: element not closed"));
            }
            if self.rest().starts_with("</") {
                let tag_start = self.pos;
                self.pos += 2;
                let close = self.name()?;
                if close == e.name {
                    self.skip_past(">")?;
                    return Ok(e);
                }
                // A closing tag for an element further out closes this one implicitly; leave
                // the tag for the parent to read.
                if self.open.iter().any(|n| *n == close) {
                    self.pos = tag_start;
                    return Ok(e);
                }
                return Err(Error::Corrupt("xml: closing tag with no open element"));
            }
            self.open.push(e.name.clone());
            let child = self.element(depth + 1);
            self.open.pop();
            e.children.push(child?);
        }
    }
}

/// Parse a table's text into its root element.
pub fn parse_str(text: &str) -> Result<Element, Error> {
    let mut p = Parser { s: text, pos: 0, open: Vec::new() };
    if !p.to_next_tag()? {
        return Err(Error::Corrupt("xml: no root element"));
    }
    let root = p.element(0)?;
    if p.to_next_tag()? {
        return Err(Error::Corrupt("xml: more than one root element"));
    }
    Ok(root)
}

/// Parse a table file's bytes.
pub fn parse(bytes: &[u8]) -> Result<Element, Error> {
    parse_str(&decode_text(bytes))
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_value_missing_its_opening_quote_reads() {
        let t = parse_str(r#"<root><fog fogDensity="0.0" fogDistanceMin=-60.0" fogDistanceMax="500.0" /></root>"#).unwrap();
        let fog = &t.children[0];
        assert_eq!((fog.attr("fogDistanceMin"), fog.attr("fogDistanceMax")), (Some("-60.0"), Some("500.0")));
    }

    use super::*;

    #[test]
    fn lenient_table() {
        let text = "<?xml version=\"1.0\" encoding=\"SHIFT-JIS\"?>\r\n<!-- note -->\r\n<Root>\r\n\
            <Item Tag=\"A\" Val=\"1\"Next=\"x&lt;y&nbsp;&bogus;\"/>\r\n<Group n='2'><Sub/></Group>\r\n</Root>\r\n";
        let root = parse_str(text).unwrap();
        assert_eq!(root.name, "Root");
        assert_eq!(root.children.len(), 2);
        let item = &root.children[0];
        assert_eq!(item.attr("Tag"), Some("A"));
        assert_eq!(item.attr("Next"), Some("x<y\u{a0}&bogus;"));
        assert_eq!(root.children[1].children[0].name, "Sub");
    }

    #[test]
    fn encodings() {
        // "アイテム" in Shift-JIS, then the same in UTF-8 with a BOM.
        let sjis = b"<R a=\"\x83\x41\x83\x43\x83\x65\x83\x80\"/>";
        assert_eq!(parse(sjis).unwrap().attr("a"), Some("アイテム"));
        let utf8 = "\u{feff}<R a=\"アイテム\"/>";
        assert_eq!(parse(utf8.as_bytes()).unwrap().attr("a"), Some("アイテム"));
    }

    #[test]
    fn unclosed_child_closed_by_parent() {
        let root = parse_str("<root><wind_root><wind a=\"1\"/></root>").unwrap();
        assert_eq!(root.children[0].name, "wind_root");
        assert_eq!(root.children[0].children[0].attr("a"), Some("1"));
    }

    #[test]
    fn entity_near_multibyte_text() {
        assert_eq!(unescape("&スキル"), "&スキル");
        assert_eq!(unescape("a&amp;ス"), "a&ス");
    }

    #[test]
    fn errors() {
        assert!(parse_str("<Root><A></B></Root>").is_err());
        assert!(parse_str("<Root><A></Root>").is_ok());
        assert!(parse_str("<Root a=\"1").is_err());
    }
}
