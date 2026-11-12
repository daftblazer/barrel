//! A Squirrel 2.2 front end for the subset Escha's `Script/*.nut` use (the field creatures'
//! behaviour, `ai_*.nut`, and the command macros): locals, constants, `if`, `switch`, `for`,
//! `foreach`, `while`, functions, classes of static members, arrays, calls and members. The
//! scripts are plain source (Shift-JIS, converted before parsing), so no bytecode is involved.

use std::fmt;

#[derive(Clone, Debug, PartialEq)]
pub enum Token {
    Int(i64),
    Float(f64),
    Str(String),
    Ident(String),
    Punct(&'static str),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Error {
    pub line: usize,
    pub message: String,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "line {}: {}", self.line, self.message)
    }
}

const PUNCTS: [&str; 41] = [
    "::", "++", "--", "+=", "-=", "*=", "/=", "==", "!=", "<=", ">=", "&&", "||", "<-", "<<", ">>", "{", "}", "(", ")", "[", "]", ";", ",", ".", ":",
    "?", "+", "-", "*", "/", "%", "=", "<", ">", "!", "&", "|", "^", "~", "@",
];

/// Splits source into tokens with their lines, dropping comments.
pub fn lex(src: &str) -> Result<Vec<(Token, usize)>, Error> {
    let b = src.as_bytes();
    let (mut i, mut line) = (0, 1);
    let mut out = Vec::new();
    while i < b.len() {
        let c = b[i];
        if c == b'\n' {
            line += 1;
            i += 1;
        } else if c.is_ascii_whitespace() {
            i += 1;
        } else if b[i..].starts_with(b"//") {
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
        } else if b[i..].starts_with(b"/*") {
            i += 2;
            while i < b.len() && !b[i..].starts_with(b"*/") {
                if b[i] == b'\n' {
                    line += 1;
                }
                i += 1;
            }
            i += 2;
        } else if c.is_ascii_digit() {
            let start = i;
            if b[i..].starts_with(b"0x") || b[i..].starts_with(b"0X") {
                i += 2;
                while i < b.len() && b[i].is_ascii_hexdigit() {
                    i += 1;
                }
                let v = i64::from_str_radix(&src[start + 2..i], 16).map_err(|e| Error { line, message: e.to_string() })?;
                out.push((Token::Int(v), line));
                continue;
            }
            let mut float = false;
            while i < b.len() && (b[i].is_ascii_digit() || (b[i] == b'.' && !float && b.get(i + 1).is_some_and(|d| d.is_ascii_digit()))) {
                float |= b[i] == b'.';
                i += 1;
            }
            let text = &src[start..i];
            if i < b.len() && (b[i] == b'f' || b[i] == b'F') {
                i += 1;
                float = true;
            }
            let t = if float {
                Token::Float(text.parse().map_err(|_| Error { line, message: format!("bad number {text}") })?)
            } else {
                Token::Int(text.parse().map_err(|_| Error { line, message: format!("bad number {text}") })?)
            };
            out.push((t, line));
        } else if c.is_ascii_alphabetic() || c == b'_' {
            let start = i;
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_') {
                i += 1;
            }
            out.push((Token::Ident(src[start..i].to_string()), line));
        } else if c == b'"' {
            i += 1;
            let mut s = String::new();
            while i < b.len() && b[i] != b'"' {
                if b[i] == b'\\' && i + 1 < b.len() {
                    i += 1;
                    s.push(match b[i] {
                        b'n' => '\n',
                        b't' => '\t',
                        other => other as char,
                    });
                    i += 1;
                } else {
                    let ch = src[i..].chars().next().unwrap();
                    s.push(ch);
                    i += ch.len_utf8();
                }
            }
            i += 1;
            out.push((Token::Str(s), line));
        } else if let Some(p) = PUNCTS.iter().find(|p| b[i..].starts_with(p.as_bytes())) {
            out.push((Token::Punct(p), line));
            i += p.len();
        } else {
            // Anything else outside strings and comments (stray full-width spaces and the like).
            let ch = src[i..].chars().next().unwrap();
            if ch.is_whitespace() {
                i += ch.len_utf8();
            } else {
                return Err(Error { line, message: format!("unexpected {ch:?}") });
            }
        }
    }
    Ok(out)
}

#[derive(Clone, Debug, PartialEq)]
pub enum Expr {
    Int(i64),
    Float(f64),
    Str(String),
    Bool(bool),
    Null,
    Name(String),
    /// `::name`.
    Root(String),
    Member(Box<Expr>, String),
    Index(Box<Expr>, Box<Expr>),
    Call(Box<Expr>, Vec<Expr>),
    Array(Vec<Expr>),
    Unary(&'static str, Box<Expr>),
    Binary(&'static str, Box<Expr>, Box<Expr>),
    Ternary(Box<Expr>, Box<Expr>, Box<Expr>),
    /// `target op value` (`=`, `+=`, `-=`, ...; `<-` too).
    Assign(&'static str, Box<Expr>, Box<Expr>),
    /// `++x`/`--x` (`prefix` true) or `x++`/`x--`, as +1 or -1.
    Step(Box<Expr>, i64, bool),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Stmt {
    Local(Vec<(String, Option<Expr>)>),
    Const(String, Expr),
    Expr(Expr),
    If(Expr, Box<Stmt>, Option<Box<Stmt>>),
    While(Expr, Box<Stmt>),
    For(Option<Box<Stmt>>, Option<Expr>, Option<Expr>, Box<Stmt>),
    Foreach(Option<String>, String, Expr, Box<Stmt>),
    /// The value, then each case's value (`None` for `default`) and statements.
    Switch(Expr, Vec<(Option<Expr>, Vec<Stmt>)>),
    Break,
    Continue,
    Return(Option<Expr>),
    Block(Vec<Stmt>),
    Function(String, Vec<String>, Vec<Stmt>),
    /// A class of static members: its name and each member's name and value.
    Class(String, Vec<(String, Expr)>),
}

pub struct Parser {
    tokens: Vec<(Token, usize)>,
    at: usize,
}

/// Parses a script into its top-level statements.
pub fn parse(src: &str) -> Result<Vec<Stmt>, Error> {
    let mut p = Parser { tokens: lex(src)?, at: 0 };
    let mut out = Vec::new();
    while p.at < p.tokens.len() {
        out.push(p.statement()?);
    }
    Ok(out)
}

impl Parser {
    fn line(&self) -> usize {
        self.tokens.get(self.at).or(self.tokens.last()).map_or(0, |t| t.1)
    }

    fn err<T>(&self, message: impl Into<String>) -> Result<T, Error> {
        Err(Error { line: self.line(), message: message.into() })
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.at).map(|t| &t.0)
    }

    fn is(&self, p: &str) -> bool {
        matches!(self.peek(), Some(Token::Punct(q)) if *q == p)
    }

    fn is_word(&self, w: &str) -> bool {
        matches!(self.peek(), Some(Token::Ident(q)) if q == w)
    }

    fn eat(&mut self, p: &str) -> bool {
        if self.is(p) {
            self.at += 1;
            true
        } else {
            false
        }
    }

    fn eat_word(&mut self, w: &str) -> bool {
        if self.is_word(w) {
            self.at += 1;
            true
        } else {
            false
        }
    }

    fn expect(&mut self, p: &str) -> Result<(), Error> {
        if self.eat(p) { Ok(()) } else { self.err(format!("expected {p}, found {:?}", self.peek())) }
    }

    fn ident(&mut self) -> Result<String, Error> {
        match self.peek() {
            Some(Token::Ident(s)) => {
                let s = s.clone();
                self.at += 1;
                Ok(s)
            }
            other => self.err(format!("expected a name, found {other:?}")),
        }
    }

    fn statement(&mut self) -> Result<Stmt, Error> {
        if self.eat(";") {
            return Ok(Stmt::Block(Vec::new()));
        }
        if self.eat("{") {
            let mut body = Vec::new();
            while !self.eat("}") {
                if self.at >= self.tokens.len() {
                    return self.err("unclosed block");
                }
                body.push(self.statement()?);
            }
            return Ok(Stmt::Block(body));
        }
        if self.eat_word("local") {
            let mut vars = Vec::new();
            loop {
                let name = self.ident()?;
                let value = if self.eat("=") { Some(self.expr()?) } else { None };
                vars.push((name, value));
                if !self.eat(",") {
                    break;
                }
            }
            self.eat(";");
            return Ok(Stmt::Local(vars));
        }
        if self.eat_word("const") {
            let name = self.ident()?;
            self.expect("=")?;
            let v = self.expr()?;
            self.eat(";");
            return Ok(Stmt::Const(name, v));
        }
        if self.eat_word("if") {
            self.expect("(")?;
            let c = self.expr()?;
            self.expect(")")?;
            let then = Box::new(self.statement()?);
            let otherwise = if self.eat_word("else") { Some(Box::new(self.statement()?)) } else { None };
            return Ok(Stmt::If(c, then, otherwise));
        }
        if self.eat_word("while") {
            self.expect("(")?;
            let c = self.expr()?;
            self.expect(")")?;
            return Ok(Stmt::While(c, Box::new(self.statement()?)));
        }
        if self.eat_word("for") {
            self.expect("(")?;
            let init = if self.is(";") { None } else { Some(Box::new(self.simple()?)) };
            self.expect(";")?;
            let cond = if self.is(";") { None } else { Some(self.expr()?) };
            self.expect(";")?;
            let step = if self.is(")") { None } else { Some(self.expr()?) };
            self.expect(")")?;
            return Ok(Stmt::For(init, cond, step, Box::new(self.statement()?)));
        }
        if self.eat_word("foreach") {
            self.expect("(")?;
            let first = self.ident()?;
            let (key, value) = if self.eat(",") { (Some(first), self.ident()?) } else { (None, first) };
            if !self.eat_word("in") {
                return self.err("expected in");
            }
            let over = self.expr()?;
            self.expect(")")?;
            return Ok(Stmt::Foreach(key, value, over, Box::new(self.statement()?)));
        }
        if self.eat_word("switch") {
            self.expect("(")?;
            let v = self.expr()?;
            self.expect(")")?;
            self.expect("{")?;
            let mut cases = Vec::new();
            while !self.eat("}") {
                let label = if self.eat_word("case") {
                    let e = self.expr()?;
                    Some(e)
                } else if self.eat_word("default") {
                    None
                } else {
                    return self.err("expected case or default");
                };
                self.expect(":")?;
                let mut body = Vec::new();
                while !self.is_word("case") && !self.is_word("default") && !self.is("}") {
                    body.push(self.statement()?);
                }
                cases.push((label, body));
            }
            return Ok(Stmt::Switch(v, cases));
        }
        if self.eat_word("break") {
            self.eat(";");
            return Ok(Stmt::Break);
        }
        if self.eat_word("continue") {
            self.eat(";");
            return Ok(Stmt::Continue);
        }
        if self.eat_word("return") {
            let v = if self.is(";") || self.is("}") { None } else { Some(self.expr()?) };
            self.eat(";");
            return Ok(Stmt::Return(v));
        }
        if self.eat_word("function") {
            let mut name = self.ident()?;
            while self.eat("::") || self.eat(".") {
                name = format!("{name}.{}", self.ident()?);
            }
            self.expect("(")?;
            let mut params = Vec::new();
            while !self.eat(")") {
                // `...` takes the rest (read through `vargc`/`vargv`).
                if self.eat(".") {
                    self.expect(".")?;
                    self.expect(".")?;
                    params.push("...".to_string());
                } else {
                    params.push(self.ident()?);
                }
                self.eat(",");
            }
            let Stmt::Block(body) = self.statement()? else { return self.err("expected a function body") };
            return Ok(Stmt::Function(name, params, body));
        }
        if self.eat_word("class") {
            let name = self.ident()?;
            self.expect("{")?;
            let mut members = Vec::new();
            while !self.eat("}") {
                self.eat_word("static");
                let m = self.ident()?;
                self.expect("=")?;
                members.push((m, self.expr()?));
                self.eat(";");
            }
            self.eat(";");
            return Ok(Stmt::Class(name, members));
        }
        let s = self.simple()?;
        self.eat(";");
        Ok(s)
    }

    /// A `local` or an expression, as a `for` header allows.
    fn simple(&mut self) -> Result<Stmt, Error> {
        if self.is_word("local") {
            self.at += 1;
            let name = self.ident()?;
            let value = if self.eat("=") { Some(self.expr()?) } else { None };
            return Ok(Stmt::Local(vec![(name, value)]));
        }
        Ok(Stmt::Expr(self.expr()?))
    }

    pub fn expr(&mut self) -> Result<Expr, Error> {
        let target = self.ternary()?;
        for op in ["=", "+=", "-=", "*=", "/=", "<-"] {
            if self.eat(op) {
                let value = self.expr()?;
                return Ok(Expr::Assign(op, Box::new(target), Box::new(value)));
            }
        }
        Ok(target)
    }

    fn ternary(&mut self) -> Result<Expr, Error> {
        let c = self.binary(0)?;
        if self.eat("?") {
            let a = self.expr()?;
            self.expect(":")?;
            let b = self.expr()?;
            return Ok(Expr::Ternary(Box::new(c), Box::new(a), Box::new(b)));
        }
        Ok(c)
    }

    fn binary(&mut self, level: usize) -> Result<Expr, Error> {
        const LEVELS: [&[&str]; 9] =
            [&["||"], &["&&"], &["|"], &["^"], &["&"], &["==", "!="], &["<", "<=", ">", ">="], &["<<", ">>", "+", "-"], &["*", "/", "%"]];
        if level == LEVELS.len() {
            return self.unary();
        }
        let mut left = self.binary(level + 1)?;
        'more: loop {
            for &op in LEVELS[level] {
                // `<<`/`>>` share a level with + and - here; the scripts do not mix them.
                if self.eat(op) {
                    let right = self.binary(level + 1)?;
                    left = Expr::Binary(op, Box::new(left), Box::new(right));
                    continue 'more;
                }
            }
            return Ok(left);
        }
    }

    fn unary(&mut self) -> Result<Expr, Error> {
        for op in ["-", "!", "~"] {
            if self.eat(op) {
                return Ok(Expr::Unary(op, Box::new(self.unary()?)));
            }
        }
        if self.eat("++") {
            return Ok(Expr::Step(Box::new(self.unary()?), 1, true));
        }
        if self.eat("--") {
            return Ok(Expr::Step(Box::new(self.unary()?), -1, true));
        }
        self.postfix()
    }

    fn postfix(&mut self) -> Result<Expr, Error> {
        let mut e = self.primary()?;
        loop {
            if self.eat(".") {
                e = Expr::Member(Box::new(e), self.ident()?);
            } else if self.eat("[") {
                let i = self.expr()?;
                self.expect("]")?;
                e = Expr::Index(Box::new(e), Box::new(i));
            } else if self.eat("(") {
                let mut args = Vec::new();
                while !self.eat(")") {
                    args.push(self.expr()?);
                    self.eat(",");
                }
                e = Expr::Call(Box::new(e), args);
            } else if self.eat("++") {
                e = Expr::Step(Box::new(e), 1, false);
            } else if self.eat("--") {
                e = Expr::Step(Box::new(e), -1, false);
            } else {
                return Ok(e);
            }
        }
    }

    fn primary(&mut self) -> Result<Expr, Error> {
        let Some(t) = self.peek().cloned() else { return self.err("unexpected end") };
        self.at += 1;
        Ok(match t {
            Token::Int(v) => Expr::Int(v),
            Token::Float(v) => Expr::Float(v),
            Token::Str(s) => Expr::Str(s),
            Token::Ident(s) => match s.as_str() {
                "true" => Expr::Bool(true),
                "false" => Expr::Bool(false),
                "null" => Expr::Null,
                _ => Expr::Name(s),
            },
            Token::Punct("::") => Expr::Root(self.ident()?),
            Token::Punct("(") => {
                let e = self.expr()?;
                self.expect(")")?;
                e
            }
            Token::Punct("[") => {
                let mut items = Vec::new();
                while !self.eat("]") {
                    items.push(self.expr()?);
                    self.eat(",");
                }
                Expr::Array(items)
            }
            other => {
                self.at -= 1;
                return self.err(format!("unexpected {other:?}"));
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_shapes_the_scripts_use() {
        let src = r#"
            class Enum { static STATE_WAIT = 1; static SPEED = 0.35; };
            {
                local pcOwn = SQ.getActiveChara();
                const of32Walk = 0.7;
                local a = [ Enum.STATE_WAIT, 2, ];
                switch(pcOwn.getState()) {
                    case Enum.STATE_WAIT:
                        pcOwn.setTimer(pcOwn.getWaitTime() * 2);
                        if (pcOwn.isMovePosition() || !x) { y = a[SQ.randRange(0, a.len()-1)]; } else z--;
                    break;
                    default: break;
                }
                foreach (i, v in a) { ::g += v; }
                for (local i = 0; i < 3; i++) {}
                local t = c ? 800 : 200; // a comment
                /* block
                   comment */
                SQ.execProgramMacroFast("wandAttack");
            }
            function f(a, b) { return a + b; }
            function printf(s, ...) { return vargc; }
        "#;
        let s = parse(src).unwrap();
        assert_eq!(s.len(), 4);
        assert!(matches!(&s[0], Stmt::Class(n, m) if n == "Enum" && m.len() == 2));
        assert!(matches!(&s[2], Stmt::Function(n, p, _) if n == "f" && p.len() == 2));
    }
}
