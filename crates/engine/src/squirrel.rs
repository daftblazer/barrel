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

// ---------------------------------------------------------------------------------------------
// The evaluator: a tree walker over the parsed statements.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

#[derive(Clone, Debug)]
pub enum Value {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(Rc<str>),
    Array(Rc<RefCell<Vec<Value>>>),
    /// A class of static members (or any table).
    Table(Rc<RefCell<HashMap<String, Value>>>),
    /// An object the host owns (`SQ`, a character), by the host's number.
    Host(u32),
    /// A host `Vector3` (`SQ.getVector3`), passed through scripts whole.
    Vec3([f64; 3]),
    Function(Rc<(Vec<String>, Vec<Stmt>)>),
}

impl PartialEq for Value {
    fn eq(&self, other: &Value) -> bool {
        match (self, other) {
            (Value::Null, Value::Null) => true,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::Str(a), Value::Str(b)) => a == b,
            (Value::Host(a), Value::Host(b)) => a == b,
            (Value::Vec3(a), Value::Vec3(b)) => a == b,
            (Value::Array(a), Value::Array(b)) => Rc::ptr_eq(a, b),
            (Value::Table(a), Value::Table(b)) => Rc::ptr_eq(a, b),
            (a, b) => match (a.number(), b.number()) {
                (Some(x), Some(y)) => x == y,
                _ => false,
            },
        }
    }
}

impl Value {
    pub fn number(&self) -> Option<f64> {
        match *self {
            Value::Int(v) => Some(v as f64),
            Value::Float(v) => Some(v),
            Value::Bool(b) => Some(b as i64 as f64),
            _ => None,
        }
    }

    pub fn truthy(&self) -> bool {
        match self {
            Value::Null => false,
            Value::Bool(b) => *b,
            Value::Int(v) => *v != 0,
            Value::Float(v) => *v != 0.0,
            _ => true,
        }
    }

    pub fn as_int(&self) -> i64 {
        match *self {
            Value::Int(v) => v,
            Value::Float(v) => v as i64,
            Value::Bool(b) => b as i64,
            _ => 0,
        }
    }
}

/// What a script reaches outside itself: the host's objects and its named constants.
pub trait Host {
    /// A name the script does not define (`SQ`, `MOTION_F_WAIT`, `ECT_...`), or `None`.
    fn global(&mut self, name: &str) -> Option<Value>;
    /// Calls method `name` of host object `object`.
    fn call(&mut self, object: u32, name: &str, args: &[Value]) -> Result<Value, Error>;
}

enum Flow {
    Normal,
    Break,
    Continue,
    Return(Value),
}

/// A script world: the root table (`::name`, functions, classes) and the call stack's scopes.
#[derive(Default)]
pub struct Interp {
    pub root: HashMap<String, Value>,
    scopes: Vec<HashMap<String, Value>>,
    /// Statements run, to stop a runaway script.
    pub budget: u64,
}

fn rt<T>(message: impl Into<String>) -> Result<T, Error> {
    Err(Error { line: 0, message: message.into() })
}

impl Interp {
    pub fn new() -> Interp {
        Interp { budget: 1_000_000, ..Interp::default() }
    }

    /// Runs top-level statements (a script file): functions and classes go into the root
    /// table, a bare block runs. Returns what a top-level `return` gave.
    pub fn run(&mut self, stmts: &[Stmt], host: &mut dyn Host) -> Result<Value, Error> {
        self.scopes.push(HashMap::new());
        let mut out = Value::Null;
        for s in stmts {
            if let Flow::Return(v) = self.exec(s, host)? {
                out = v;
                break;
            }
        }
        self.scopes.pop();
        Ok(out)
    }

    fn lookup(&mut self, name: &str, host: &mut dyn Host) -> Result<Value, Error> {
        for scope in self.scopes.iter().rev() {
            if let Some(v) = scope.get(name) {
                return Ok(v.clone());
            }
        }
        if let Some(v) = self.root.get(name) {
            return Ok(v.clone());
        }
        match host.global(name) {
            Some(v) => Ok(v),
            None => rt(format!("unknown name {name}")),
        }
    }

    fn assign_name(&mut self, name: &str, v: Value) {
        for scope in self.scopes.iter_mut().rev() {
            if let Some(slot) = scope.get_mut(name) {
                *slot = v;
                return;
            }
        }
        self.root.insert(name.to_string(), v);
    }

    fn exec(&mut self, s: &Stmt, host: &mut dyn Host) -> Result<Flow, Error> {
        if self.budget == 0 {
            return rt("the script ran too long");
        }
        self.budget -= 1;
        match s {
            Stmt::Local(vars) => {
                for (name, e) in vars {
                    let v = match e {
                        Some(e) => self.eval(e, host)?,
                        None => Value::Null,
                    };
                    self.scopes.last_mut().unwrap().insert(name.clone(), v);
                }
            }
            Stmt::Const(name, e) => {
                let v = self.eval(e, host)?;
                self.scopes.last_mut().unwrap().insert(name.clone(), v);
            }
            Stmt::Expr(e) => {
                self.eval(e, host)?;
            }
            Stmt::If(c, a, b) => {
                if self.eval(c, host)?.truthy() {
                    return self.exec(a, host);
                } else if let Some(b) = b {
                    return self.exec(b, host);
                }
            }
            Stmt::While(c, body) => {
                while self.eval(c, host)?.truthy() {
                    match self.exec(body, host)? {
                        Flow::Break => break,
                        Flow::Return(v) => return Ok(Flow::Return(v)),
                        _ => {}
                    }
                }
            }
            Stmt::For(init, c, step, body) => {
                self.scopes.push(HashMap::new());
                if let Some(i) = init {
                    self.exec(i, host)?;
                }
                let flow = loop {
                    if let Some(c) = c
                        && !self.eval(c, host)?.truthy() {
                            break Flow::Normal;
                        }
                    match self.exec(body, host)? {
                        Flow::Break => break Flow::Normal,
                        Flow::Return(v) => break Flow::Return(v),
                        _ => {}
                    }
                    if let Some(st) = step {
                        self.eval(st, host)?;
                    }
                };
                self.scopes.pop();
                return Ok(flow);
            }
            Stmt::Foreach(key, value, over, body) => {
                let items: Vec<(Value, Value)> = match self.eval(over, host)? {
                    Value::Array(a) => a.borrow().iter().enumerate().map(|(i, v)| (Value::Int(i as i64), v.clone())).collect(),
                    Value::Table(t) => t.borrow().iter().map(|(k, v)| (Value::Str(k.as_str().into()), v.clone())).collect(),
                    _ => return rt("foreach over something that is not an array or table"),
                };
                for (k, v) in items {
                    self.scopes.push(HashMap::new());
                    if let Some(key) = key {
                        self.scopes.last_mut().unwrap().insert(key.clone(), k);
                    }
                    self.scopes.last_mut().unwrap().insert(value.clone(), v);
                    let flow = self.exec(body, host)?;
                    self.scopes.pop();
                    match flow {
                        Flow::Break => break,
                        Flow::Return(v) => return Ok(Flow::Return(v)),
                        _ => {}
                    }
                }
            }
            Stmt::Switch(v, cases) => {
                let v = self.eval(v, host)?;
                // Squirrel switches fall through from the first matching case (or default).
                let mut start = None;
                for (i, (label, _)) in cases.iter().enumerate() {
                    if let Some(l) = label
                        && self.eval(l, host)? == v {
                            start = Some(i);
                            break;
                        }
                }
                let start = start.or_else(|| cases.iter().position(|(l, _)| l.is_none()));
                if let Some(start) = start {
                    for (_, body) in &cases[start..] {
                        for st in body {
                            match self.exec(st, host)? {
                                Flow::Break => return Ok(Flow::Normal),
                                Flow::Normal => {}
                                other => return Ok(other),
                            }
                        }
                    }
                }
            }
            Stmt::Break => return Ok(Flow::Break),
            Stmt::Continue => return Ok(Flow::Continue),
            Stmt::Return(e) => {
                let v = match e {
                    Some(e) => self.eval(e, host)?,
                    None => Value::Null,
                };
                return Ok(Flow::Return(v));
            }
            Stmt::Block(body) => {
                self.scopes.push(HashMap::new());
                for st in body {
                    let flow = self.exec(st, host)?;
                    if !matches!(flow, Flow::Normal) {
                        self.scopes.pop();
                        return Ok(flow);
                    }
                }
                self.scopes.pop();
            }
            Stmt::Function(name, params, body) => {
                self.root.insert(name.clone(), Value::Function(Rc::new((params.clone(), body.clone()))));
            }
            Stmt::Class(name, members) => {
                let mut t = HashMap::new();
                for (m, e) in members {
                    let v = self.eval(e, host)?;
                    t.insert(m.clone(), v);
                }
                self.root.insert(name.clone(), Value::Table(Rc::new(RefCell::new(t))));
            }
        }
        Ok(Flow::Normal)
    }

    fn call_function(&mut self, f: &(Vec<String>, Vec<Stmt>), args: Vec<Value>, host: &mut dyn Host) -> Result<Value, Error> {
        let (params, body) = f;
        let mut scope = HashMap::new();
        let mut rest = Vec::new();
        for (i, a) in args.into_iter().enumerate() {
            match params.get(i) {
                Some(p) if p != "..." => {
                    scope.insert(p.clone(), a);
                }
                _ => rest.push(a),
            }
        }
        for p in params.iter().filter(|p| *p != "...") {
            scope.entry(p.clone()).or_insert(Value::Null);
        }
        scope.insert("vargc".into(), Value::Int(rest.len() as i64));
        scope.insert("vargv".into(), Value::Array(Rc::new(RefCell::new(rest))));
        // A function sees its own scope and the root, not its caller's locals.
        let saved = std::mem::replace(&mut self.scopes, vec![scope]);
        let mut out = Ok(Value::Null);
        for st in body {
            match self.exec(st, host) {
                Ok(Flow::Return(v)) => {
                    out = Ok(v);
                    break;
                }
                Ok(_) => {}
                Err(e) => {
                    out = Err(e);
                    break;
                }
            }
        }
        self.scopes = saved;
        out
    }

    pub fn eval(&mut self, e: &Expr, host: &mut dyn Host) -> Result<Value, Error> {
        Ok(match e {
            Expr::Int(v) => Value::Int(*v),
            Expr::Float(v) => Value::Float(*v),
            Expr::Str(s) => Value::Str(s.as_str().into()),
            Expr::Bool(b) => Value::Bool(*b),
            Expr::Null => Value::Null,
            Expr::Name(n) => self.lookup(n, host)?,
            Expr::Root(n) => match self.root.get(n) {
                Some(v) => v.clone(),
                None => host.global(n).map_or_else(|| rt(format!("unknown name ::{n}")), Ok)?,
            },
            Expr::Array(items) => {
                let mut out = Vec::with_capacity(items.len());
                for i in items {
                    out.push(self.eval(i, host)?);
                }
                Value::Array(Rc::new(RefCell::new(out)))
            }
            Expr::Member(obj, name) => match self.eval(obj, host)? {
                Value::Table(t) => t.borrow().get(name).cloned().map_or_else(|| rt(format!("no member {name}")), Ok)?,
                other => return rt(format!("member {name} of {other:?}")),
            },
            Expr::Index(obj, i) => {
                let i = self.eval(i, host)?;
                match self.eval(obj, host)? {
                    Value::Array(a) => a.borrow().get(i.as_int() as usize).cloned().map_or_else(|| rt("index out of range"), Ok)?,
                    Value::Table(t) => match i {
                        Value::Str(k) => t.borrow().get(&*k).cloned().unwrap_or(Value::Null),
                        _ => Value::Null,
                    },
                    other => return rt(format!("indexing {other:?}")),
                }
            }
            Expr::Call(callee, args) => {
                let mut vals = Vec::with_capacity(args.len());
                for a in args {
                    vals.push(self.eval(a, host)?);
                }
                match &**callee {
                    Expr::Member(obj, name) => {
                        let target = self.eval(obj, host)?;
                        self.method(target, name, vals, host)?
                    }
                    other => match self.eval(other, host)? {
                        Value::Function(f) => self.call_function(&f, vals, host)?,
                        v => return rt(format!("calling {v:?}")),
                    },
                }
            }
            Expr::Unary(op, x) => {
                let v = self.eval(x, host)?;
                match (*op, v) {
                    ("-", Value::Int(i)) => Value::Int(-i),
                    ("-", Value::Float(f)) => Value::Float(-f),
                    ("!", v) => Value::Bool(!v.truthy()),
                    ("~", v) => Value::Int(!v.as_int()),
                    (op, v) => return rt(format!("{op} on {v:?}")),
                }
            }
            Expr::Binary(op, a, b) => {
                if *op == "&&" {
                    let a = self.eval(a, host)?;
                    return Ok(if a.truthy() { self.eval(b, host)? } else { a });
                }
                if *op == "||" {
                    let a = self.eval(a, host)?;
                    return Ok(if a.truthy() { a } else { self.eval(b, host)? });
                }
                let (a, b) = (self.eval(a, host)?, self.eval(b, host)?);
                binary(op, &a, &b)?
            }
            Expr::Ternary(c, a, b) => {
                if self.eval(c, host)?.truthy() {
                    self.eval(a, host)?
                } else {
                    self.eval(b, host)?
                }
            }
            Expr::Assign(op, target, value) => {
                let v = self.eval(value, host)?;
                let v = match *op {
                    "=" | "<-" => v,
                    _ => {
                        let old = self.eval(target, host)?;
                        binary(&op[..1], &old, &v)?
                    }
                };
                self.store(target, v.clone(), host)?;
                v
            }
            Expr::Step(target, by, prefix) => {
                let old = self.eval(target, host)?;
                let new = binary("+", &old, &Value::Int(*by))?;
                self.store(target, new.clone(), host)?;
                if *prefix { new } else { old }
            }
        })
    }

    fn store(&mut self, target: &Expr, v: Value, host: &mut dyn Host) -> Result<(), Error> {
        match target {
            Expr::Name(n) => self.assign_name(n, v),
            Expr::Root(n) => {
                self.root.insert(n.clone(), v);
            }
            Expr::Index(obj, i) => {
                let i = self.eval(i, host)?;
                match self.eval(obj, host)? {
                    Value::Array(a) => {
                        let mut a = a.borrow_mut();
                        let k = i.as_int() as usize;
                        if k >= a.len() {
                            return rt("index out of range");
                        }
                        a[k] = v;
                    }
                    Value::Table(t) => {
                        if let Value::Str(k) = i {
                            t.borrow_mut().insert(k.to_string(), v);
                        }
                    }
                    other => return rt(format!("storing into {other:?}")),
                }
            }
            Expr::Member(obj, name) => match self.eval(obj, host)? {
                Value::Table(t) => {
                    t.borrow_mut().insert(name.clone(), v);
                }
                other => return rt(format!("storing member {name} of {other:?}")),
            },
            other => return rt(format!("cannot assign to {other:?}")),
        }
        Ok(())
    }

    fn method(&mut self, target: Value, name: &str, args: Vec<Value>, host: &mut dyn Host) -> Result<Value, Error> {
        match (&target, name) {
            (Value::Host(h), _) => host.call(*h, name, &args),
            (Value::Array(a), "len") => Ok(Value::Int(a.borrow().len() as i64)),
            (Value::Array(a), "append" | "push") => {
                a.borrow_mut().extend(args);
                Ok(Value::Null)
            }
            (Value::Str(s), "len") => Ok(Value::Int(s.len() as i64)),
            (_, "tofloat") => Ok(Value::Float(target.number().unwrap_or(0.0))),
            (_, "tointeger") => Ok(Value::Int(target.as_int())),
            (Value::Table(t), _) => {
                let f = t.borrow().get(name).cloned();
                match f {
                    Some(Value::Function(f)) => self.call_function(&f, args, host),
                    _ => rt(format!("no method {name}")),
                }
            }
            _ => rt(format!("method {name} of {target:?}")),
        }
    }
}

fn binary(op: &str, a: &Value, b: &Value) -> Result<Value, Error> {
    if op == "==" {
        return Ok(Value::Bool(a == b));
    }
    if op == "!=" {
        return Ok(Value::Bool(a != b));
    }
    if op == "+"
        && let (Value::Str(x), y) | (y, Value::Str(x)) = (a, b) {
            let y = match y {
                Value::Str(s) => s.to_string(),
                Value::Int(i) => i.to_string(),
                Value::Float(f) => f.to_string(),
                other => format!("{other:?}"),
            };
            return Ok(Value::Str(if matches!(a, Value::Str(_)) { format!("{x}{y}") } else { format!("{y}{x}") }.into()));
        }
    match (a, b) {
        (Value::Int(x), Value::Int(y)) => {
            let (x, y) = (*x, *y);
            Ok(match op {
                "+" => Value::Int(x.wrapping_add(y)),
                "-" => Value::Int(x.wrapping_sub(y)),
                "*" => Value::Int(x.wrapping_mul(y)),
                "/" => {
                    if y == 0 {
                        return rt("division by zero");
                    }
                    Value::Int(x / y)
                }
                "%" => {
                    if y == 0 {
                        return rt("division by zero");
                    }
                    Value::Int(x % y)
                }
                "<" => Value::Bool(x < y),
                "<=" => Value::Bool(x <= y),
                ">" => Value::Bool(x > y),
                ">=" => Value::Bool(x >= y),
                "&" => Value::Int(x & y),
                "|" => Value::Int(x | y),
                "^" => Value::Int(x ^ y),
                "<<" => Value::Int(x << (y & 63)),
                ">>" => Value::Int(x >> (y & 63)),
                _ => return rt(format!("{op} on integers")),
            })
        }
        _ => {
            let (Some(x), Some(y)) = (a.number(), b.number()) else { return rt(format!("{op} on {a:?} and {b:?}")) };
            Ok(match op {
                "+" => Value::Float(x + y),
                "-" => Value::Float(x - y),
                "*" => Value::Float(x * y),
                "/" => Value::Float(x / y),
                "%" => Value::Float(x % y),
                "<" => Value::Bool(x < y),
                "<=" => Value::Bool(x <= y),
                ">" => Value::Bool(x > y),
                ">=" => Value::Bool(x >= y),
                _ => return rt(format!("{op} on numbers")),
            })
        }
    }
}

#[cfg(test)]
mod eval_tests {
    use super::*;

    /// A host with `SQ` (object 0) and one creature (object 1), logging every call.
    struct Mock {
        log: Vec<String>,
        state: i64,
        motion_ended: bool,
    }

    impl Host for Mock {
        fn global(&mut self, name: &str) -> Option<Value> {
            match name {
                "SQ" => Some(Value::Host(0)),
                "MOTION_F_WAIT" => Some(Value::Int(31)),
                _ => None,
            }
        }
        fn call(&mut self, object: u32, name: &str, args: &[Value]) -> Result<Value, Error> {
            self.log.push(format!("{object}.{name}{:?}", args.iter().map(|a| a.number().unwrap_or(-1.0)).collect::<Vec<_>>()));
            Ok(match (object, name) {
                (0, "getPlayer") => Value::Host(2),
                (0, "getActiveChara") => Value::Host(1),
                (1, "getState") => Value::Int(self.state),
                (1, "setState") => {
                    self.state = args[0].as_int();
                    Value::Null
                }
                (1, "isEndMotin") => Value::Bool(self.motion_ended),
                (1, "getOldMotionTag") => Value::Int(31),
                _ => Value::Null,
            })
        }
    }

    #[test]
    fn runs_a_wait_script() {
        let utility = parse(
            r#"
            class Val { static MOTION_BLEND_FRAME = 0.2; static MOTION_SPEED = 1.0; };
            class Enum { static STATE_SELECT = 0; static STATE_WAIT = 1; };
            function isRangeInOut(a, b) { return false; }
        "#,
        )
        .unwrap();
        let wait = parse(
            r#"{
            local pcPlayer = SQ.getPlayer();
            local pcOwn = SQ.getActiveChara();
            pcOwn.stopMovePosition();
            if (pcOwn.isEndMotin()) {
                local s32MotionTag = MOTION_F_WAIT;
                pcOwn.setMotionTag(s32MotionTag, 0, Val.MOTION_BLEND_FRAME, Val.MOTION_SPEED, false, true);
                if (s32MotionTag != pcOwn.getOldMotionTag()) { pcOwn.setRandomMotion(); }
            }
            switch (pcOwn.getState()) {
                case Enum.STATE_SELECT: pcOwn.setState(Enum.STATE_WAIT);
                case 5: pcOwn.fellThrough();
                break;
                default: pcOwn.never();
            }
            if (::isRangeInOut(pcOwn, pcPlayer)) { pcOwn.setState(Enum.STATE_SELECT); }
            return;
        }"#,
        )
        .unwrap();
        let mut host = Mock { log: Vec::new(), state: 0, motion_ended: true };
        let mut vm = Interp::new();
        vm.run(&utility, &mut host).unwrap();
        vm.run(&wait, &mut host).unwrap();
        assert_eq!(host.state, 1);
        assert!(host.log.contains(&"1.setMotionTag[31.0, 0.0, 0.2, 1.0, 0.0, 1.0]".to_string()), "{:?}", host.log);
        assert!(host.log.iter().any(|l| l.starts_with("1.fellThrough")), "a case falls through to the next");
        assert!(!host.log.iter().any(|l| l.starts_with("1.never") || l.starts_with("1.setRandomMotion")));
    }

    #[test]
    fn functions_loops_and_arrays() {
        let s = parse(
            r#"
            function sum(a) { local t = 0; foreach (v in a) { t += v; } return t; }
            {
                local a = [1, 2, 3];
                local n = 0;
                for (local i = 0; i < 4; i++) { n += i; }
                local w = 0;
                while (w < 3) { w++; }
                return sum(a) * 100 + n * 10 + w + a.len() * 1000;
            }
        "#,
        )
        .unwrap();
        let mut host = Mock { log: Vec::new(), state: 0, motion_ended: false };
        let v = Interp::new().run(&s, &mut host).unwrap();
        assert_eq!(v.as_int(), 3000 + 600 + 60 + 3);
    }
}
