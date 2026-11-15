//! The small subset of TOML Barrel's own settings files use: `key = "text"`, `key = ["a", "b"]`
//! (over several lines if need be), bare numbers and booleans (read as text), `#` comments.
//! No tables: each file is one flat list of keys.

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Value {
    Text(String),
    List(Vec<String>),
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Table {
    pub entries: Vec<(String, Value)>,
}

impl Table {
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.entries.iter().rev().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    /// A text value (a bare number or boolean included).
    pub fn text(&self, key: &str) -> Option<&str> {
        match self.get(key)? {
            Value::Text(t) => Some(t),
            Value::List(_) => None,
        }
    }

    /// A list of texts; a lone text is a list of one.
    pub fn list(&self, key: &str) -> Vec<String> {
        match self.get(key) {
            Some(Value::List(l)) => l.clone(),
            Some(Value::Text(t)) => vec![t.clone()],
            None => Vec::new(),
        }
    }

    pub fn set(&mut self, key: &str, value: Value) {
        match self.entries.iter_mut().find(|(k, _)| k == key) {
            Some(e) => e.1 = value,
            None => self.entries.push((key.to_string(), value)),
        }
    }

    pub fn to_text(&self) -> String {
        let mut out = String::new();
        for (k, v) in &self.entries {
            match v {
                Value::Text(t) => out.push_str(&format!("{k} = {}\n", quote(t))),
                Value::List(l) => out.push_str(&format!("{k} = [{}]\n", l.iter().map(|s| quote(s)).collect::<Vec<_>>().join(", "))),
            }
        }
        out
    }
}

fn quote(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

/// `s` up to a `#` that is not inside quotes.
fn strip_comment(s: &str) -> &str {
    let (mut quoted, mut escaped) = (false, false);
    for (i, c) in s.char_indices() {
        match c {
            '\\' if quoted => escaped = !escaped,
            '"' if !escaped => quoted = !quoted,
            '#' if !quoted => return &s[..i],
            _ => {}
        }
        if c != '\\' {
            escaped = false;
        }
    }
    s
}

/// The quoted texts in `s`, with the escapes undone.
fn quoted_texts(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur: Option<String> = None;
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        match (&mut cur, c) {
            (None, '"') => cur = Some(String::new()),
            (Some(_), '"') => out.push(cur.take().unwrap()),
            (Some(t), '\\') => t.extend(chars.next()),
            (Some(t), c) => t.push(c),
            (None, _) => {}
        }
    }
    out
}

/// Parses `text`; a line that is not `key = value` is skipped.
pub fn parse(text: &str) -> Table {
    let mut table = Table::default();
    let mut lines = text.lines();
    while let Some(line) = lines.next() {
        let line = strip_comment(line).trim();
        let Some((key, value)) = line.split_once('=') else { continue };
        let (key, mut value) = (key.trim().trim_matches('"').to_string(), value.trim().to_string());
        if key.is_empty() {
            continue;
        }
        if value.starts_with('[') {
            while !value.trim_end().ends_with(']') {
                let Some(next) = lines.next() else { break };
                value.push(' ');
                value.push_str(strip_comment(next).trim());
            }
            table.entries.push((key, Value::List(quoted_texts(&value))));
        } else if value.starts_with('"') {
            table.entries.push((key, Value::Text(quoted_texts(&value).into_iter().next().unwrap_or_default())));
        } else {
            table.entries.push((key, Value::Text(value)));
        }
    }
    table
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_texts_lists_and_comments() {
        let t = parse("# mods\nname = \"Big # Pack\"  # trailing\nversion = 1.2\nenabled = [\n  \"a\", # first\n  \"b\\\"q\",\n]\nempty = []\n");
        assert_eq!(t.text("name"), Some("Big # Pack"));
        assert_eq!(t.text("version"), Some("1.2"));
        assert_eq!(t.list("enabled"), ["a", "b\"q"]);
        assert!(t.list("empty").is_empty());
        assert!(t.list("none").is_empty());
    }

    #[test]
    fn written_text_reads_back() {
        let mut t = Table::default();
        t.set("escha", Value::Text("/games/Atelier \"E\"".into()));
        t.set("enabled", Value::List(vec!["x".into(), "y z".into()]));
        t.set("escha", Value::Text("/other".into()));
        assert_eq!(parse(&t.to_text()), t);
    }
}
