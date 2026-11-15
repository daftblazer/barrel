//! Valve's text key-value format (`libraryfolders.vdf`, `appmanifest_*.acf`): quoted keys and
//! values, and `{ }` blocks of more keys.

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Node {
    Text(String),
    Block(Vec<(String, Node)>),
}

impl Node {
    /// The first child `key` of a block (any case).
    pub fn get(&self, key: &str) -> Option<&Node> {
        match self {
            Node::Block(c) => c.iter().find(|(k, _)| k.eq_ignore_ascii_case(key)).map(|(_, n)| n),
            Node::Text(_) => None,
        }
    }

    pub fn text(&self) -> Option<&str> {
        match self {
            Node::Text(t) => Some(t),
            Node::Block(_) => None,
        }
    }

    pub fn children(&self) -> &[(String, Node)] {
        match self {
            Node::Block(c) => c,
            Node::Text(_) => &[],
        }
    }
}

enum Token {
    Text(String),
    Open,
    Close,
}

fn tokens(s: &str) -> Vec<Token> {
    let mut out = Vec::new();
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '{' => out.push(Token::Open),
            '}' => out.push(Token::Close),
            '"' => {
                let mut t = String::new();
                while let Some(c) = chars.next() {
                    match c {
                        '"' => break,
                        '\\' => match chars.next() {
                            Some('n') => t.push('\n'),
                            Some('t') => t.push('\t'),
                            Some(o) => t.push(o),
                            None => {}
                        },
                        c => t.push(c),
                    }
                }
                out.push(Token::Text(t));
            }
            '/' if chars.peek() == Some(&'/') => {
                for c in chars.by_ref() {
                    if c == '\n' {
                        break;
                    }
                }
            }
            _ => {}
        }
    }
    out
}

fn block(it: &mut std::iter::Peekable<std::vec::IntoIter<Token>>) -> Vec<(String, Node)> {
    let mut out = Vec::new();
    while let Some(t) = it.next() {
        match t {
            Token::Close => break,
            Token::Open => {}
            Token::Text(key) => match it.peek() {
                Some(Token::Open) => {
                    it.next();
                    out.push((key, Node::Block(block(it))));
                }
                Some(Token::Text(_)) => {
                    if let Some(Token::Text(v)) = it.next() {
                        out.push((key, Node::Text(v)));
                    }
                }
                _ => {}
            },
        }
    }
    out
}

/// The top level of `text` as a block.
pub fn parse(text: &str) -> Node {
    Node::Block(block(&mut tokens(text).into_iter().peekable()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_nested_blocks() {
        let n = parse("\"libraryfolders\"\n{\n\t\"0\"\n\t{\n\t\t\"path\"\t\t\"/a b\"\n\t\t\"apps\" { \"1\" \"2\" }\n\t}\n\t\"1\" { \"path\" \"C:\\\\Games\" } // c\n}\n");
        let lib = n.get("LibraryFolders").unwrap();
        assert_eq!(lib.children().len(), 2);
        assert_eq!(lib.get("0").and_then(|l| l.get("path")).and_then(Node::text), Some("/a b"));
        assert_eq!(lib.get("1").and_then(|l| l.get("path")).and_then(Node::text), Some("C:\\Games"));
        assert_eq!(lib.get("0").and_then(|l| l.get("apps")).and_then(|a| a.get("1")).and_then(Node::text), Some("2"));
    }
}
