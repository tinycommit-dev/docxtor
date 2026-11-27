//! Formula fields: `= expression`.
//!
//! This module evaluates the body of a Word formula field, the text after the `=` in `{ = SUM(ABOVE) * 2 }`.
//! It knows numbers, the usual operators, cell references (`A1`, `B2:C4`), the position keywords `ABOVE`,
//! `BELOW`, `LEFT` and `RIGHT`, bookmark names, and Word's list of functions. The field engine and the
//! table-formula feature both call [`evaluate`]. The caller strips switches such as `\# "0.00"` first.
//!
//! Everything the formula can see comes through the [`Context`] trait, so the evaluator has no idea what a
//! document is. Errors are short English messages, and the caller shows exactly the text it gets back.

// ---------------------------------------------------------------------------
// Context
// ---------------------------------------------------------------------------

/// What a table cell holds, for formulas.
#[derive(Debug, Clone, PartialEq)]
pub enum Cell {
    /// A cell with no text.
    Empty,
    /// A cell whose text is a number.
    Number(f64),
    /// A cell with any other text.
    Text(String),
}

/// The surroundings of a formula: the table it sits in (if any) and the bookmarks of the document.
pub trait Context {
    /// The cell at a 0-based row and column of the table holding the formula. None outside the table or when
    /// there is no table.
    fn cell(&self, row: usize, col: usize) -> Option<Cell>;
    /// The (row, column), 0-based, of the cell holding the formula. None when the formula is not in a table.
    fn position(&self) -> Option<(usize, usize)>;
    /// How many rows and columns the table has.
    fn size(&self) -> (usize, usize);
    /// The number in a bookmark's text (Word lets a formula use a bookmark name as a number), or the value of a
    /// `SET` variable. None if unknown or not a number.
    fn name(&self, name: &str) -> Option<f64>;
}

/// A context with no table and no names.
pub struct NoContext;

impl Context for NoContext {
    fn cell(&self, _row: usize, _col: usize) -> Option<Cell> {
        None
    }
    fn position(&self) -> Option<(usize, usize)> {
        None
    }
    fn size(&self) -> (usize, usize) {
        (0, 0)
    }
    fn name(&self, _name: &str) -> Option<f64> {
        None
    }
}

// ---------------------------------------------------------------------------
// Cell names
// ---------------------------------------------------------------------------

/// The most letters a column name can have. `ZZZ` is column 17575. Longer words are bookmark names.
const MAX_COLUMN_LETTERS: usize = 3;

/// The `A1`-style name of a 0-based (row, column): (0, 0) is `A1` and (2, 27) is `AB3`.
pub fn cell_name(row: usize, col: usize) -> String {
    let mut letters = Vec::new();
    let mut n = col + 1;
    while n > 0 {
        n -= 1;
        letters.push((b'A' + (n % 26) as u8) as char);
        n /= 26;
    }
    letters.reverse();
    let mut s: String = letters.into_iter().collect();
    s.push_str(&(row + 1).to_string());
    s
}

/// The 0-based (row, column) of an `A1`-style name, in either case. `A0` and `A` are not cell names.
pub fn parse_cell_name(s: &str) -> Option<(usize, usize)> {
    let letters = s.bytes().take_while(u8::is_ascii_alphabetic).count();
    if letters == 0 || letters > MAX_COLUMN_LETTERS {
        return None;
    }
    let (head, digits) = s.split_at(letters);
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let row: usize = digits.parse().ok()?;
    if row == 0 {
        return None;
    }
    let mut col = 0usize;
    for b in head.bytes() {
        col = col * 26 + (b.to_ascii_uppercase() - b'A' + 1) as usize;
    }
    Some((row - 1, col - 1))
}

// ---------------------------------------------------------------------------
// Tokens
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Num(f64),
    Ident(String),
    Plus,
    Minus,
    Star,
    Slash,
    Caret,
    Pct,
    LParen,
    RParen,
    Comma,
    Colon,
    Eq,
    Lt,
    Le,
    Gt,
    Ge,
    Ne,
}

fn lex(src: &str) -> Result<Vec<Tok>, String> {
    let chars: Vec<char> = src.chars().collect();
    let mut toks = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let next = chars.get(i + 1).copied();
        match c {
            c if c.is_whitespace() => {
                i += 1;
            }
            '0'..='9' | '.' => {
                let start = i;
                while i < chars.len() && chars[i].is_ascii_digit() {
                    i += 1;
                }
                if i < chars.len() && chars[i] == '.' {
                    i += 1;
                    while i < chars.len() && chars[i].is_ascii_digit() {
                        i += 1;
                    }
                }
                let text: String = chars[start..i].iter().collect();
                if text == "." {
                    return Err("Syntax error".to_string());
                }
                let n = text.parse::<f64>().map_err(|_| "Syntax error".to_string())?;
                toks.push(Tok::Num(n));
            }
            c if c.is_alphabetic() || c == '_' => {
                let start = i;
                while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                    i += 1;
                }
                toks.push(Tok::Ident(chars[start..i].iter().collect()));
            }
            '+' => {
                toks.push(Tok::Plus);
                i += 1;
            }
            '-' => {
                toks.push(Tok::Minus);
                i += 1;
            }
            '*' => {
                toks.push(Tok::Star);
                i += 1;
            }
            '/' => {
                toks.push(Tok::Slash);
                i += 1;
            }
            '^' => {
                toks.push(Tok::Caret);
                i += 1;
            }
            '%' => {
                toks.push(Tok::Pct);
                i += 1;
            }
            '(' => {
                toks.push(Tok::LParen);
                i += 1;
            }
            ')' => {
                toks.push(Tok::RParen);
                i += 1;
            }
            ',' | ';' => {
                toks.push(Tok::Comma);
                i += 1;
            }
            ':' => {
                toks.push(Tok::Colon);
                i += 1;
            }
            '=' => {
                toks.push(Tok::Eq);
                i += 1;
            }
            '<' => match next {
                Some('=') => {
                    toks.push(Tok::Le);
                    i += 2;
                }
                Some('>') => {
                    toks.push(Tok::Ne);
                    i += 2;
                }
                _ => {
                    toks.push(Tok::Lt);
                    i += 1;
                }
            },
            '>' => {
                if next == Some('=') {
                    toks.push(Tok::Ge);
                    i += 2;
                } else {
                    toks.push(Tok::Gt);
                    i += 1;
                }
            }
            other => return Err(format!("Unexpected character: {other}")),
        }
    }
    Ok(toks)
}

// ---------------------------------------------------------------------------
// Syntax tree
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
enum Dir {
    Above,
    Below,
    Left,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Pow,
    Eq,
    Lt,
    Le,
    Gt,
    Ge,
    Ne,
}

#[derive(Debug, Clone)]
enum Node {
    Num(f64),
    Neg(Box<Node>),
    Pct(Box<Node>),
    Bin(BinOp, Box<Node>, Box<Node>),
    Call(&'static Func, Vec<Node>),
    /// A cell reference: 0-based row and column, and the text as written (for messages and bookmark lookup).
    Cell(usize, usize, String),
    /// A range: the two corners, 0-based (row, column).
    Range((usize, usize), (usize, usize)),
    Dir(Dir),
    Name(String),
}

/// A function's name and how many arguments it takes.
#[derive(Debug)]
struct Func {
    name: &'static str,
    min: usize,
    max: usize,
}

const MANY: usize = usize::MAX;

static FUNCS: &[Func] = &[
    Func { name: "ABS", min: 1, max: 1 },
    Func { name: "AND", min: 2, max: 2 },
    Func { name: "AVERAGE", min: 1, max: MANY },
    Func { name: "COUNT", min: 1, max: MANY },
    Func { name: "DEFINED", min: 1, max: 1 },
    Func { name: "FALSE", min: 0, max: 0 },
    Func { name: "IF", min: 3, max: 3 },
    Func { name: "INT", min: 1, max: 1 },
    Func { name: "MAX", min: 1, max: MANY },
    Func { name: "MIN", min: 1, max: MANY },
    Func { name: "MOD", min: 2, max: 2 },
    Func { name: "NOT", min: 1, max: 1 },
    Func { name: "OR", min: 2, max: 2 },
    Func { name: "PRODUCT", min: 1, max: MANY },
    Func { name: "ROUND", min: 2, max: 2 },
    Func { name: "SIGN", min: 1, max: 1 },
    Func { name: "SUM", min: 1, max: MANY },
    Func { name: "TRUE", min: 0, max: 0 },
];

fn find_func(name: &str) -> Option<&'static Func> {
    FUNCS.iter().find(|f| f.name.eq_ignore_ascii_case(name))
}

// ---------------------------------------------------------------------------
// Parser
// ---------------------------------------------------------------------------

struct Parser {
    toks: Vec<Tok>,
    pos: usize,
    /// How many `comparison` and `unary` calls are open, so crafted nesting ends in an error, not a stack overflow.
    depth: usize,
}

/// The deepest nesting of parentheses, function arguments and signs a formula may have.
const MAX_DEPTH: usize = 64;
/// The longest formula text the evaluator accepts. Word's own limit is far lower.
const MAX_LEN: usize = 2048;

impl Parser {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.pos)
    }

    fn eat(&mut self, t: &Tok) -> bool {
        if self.peek() == Some(t) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    /// The error for a missing closing parenthesis: at the end of the text, the parentheses are unbalanced.
    fn expected_close(&self) -> String {
        if self.peek().is_none() {
            "Unbalanced parentheses".to_string()
        } else {
            "Syntax error".to_string()
        }
    }

    /// comparison := additive (( = | < | <= | > | >= | <> ) additive)*
    fn comparison(&mut self) -> Result<Node, String> {
        self.depth += 1;
        let out = if self.depth > MAX_DEPTH { Err("Syntax error".to_string()) } else { self.comparison_inner() };
        self.depth -= 1;
        out
    }

    fn comparison_inner(&mut self) -> Result<Node, String> {
        let mut left = self.additive()?;
        loop {
            let op = match self.peek() {
                Some(Tok::Eq) => BinOp::Eq,
                Some(Tok::Lt) => BinOp::Lt,
                Some(Tok::Le) => BinOp::Le,
                Some(Tok::Gt) => BinOp::Gt,
                Some(Tok::Ge) => BinOp::Ge,
                Some(Tok::Ne) => BinOp::Ne,
                _ => return Ok(left),
            };
            self.pos += 1;
            let right = self.additive()?;
            left = Node::Bin(op, Box::new(left), Box::new(right));
        }
    }

    /// additive := multiplicative (( + | - ) multiplicative)*
    fn additive(&mut self) -> Result<Node, String> {
        let mut left = self.multiplicative()?;
        loop {
            let op = match self.peek() {
                Some(Tok::Plus) => BinOp::Add,
                Some(Tok::Minus) => BinOp::Sub,
                _ => return Ok(left),
            };
            self.pos += 1;
            let right = self.multiplicative()?;
            left = Node::Bin(op, Box::new(left), Box::new(right));
        }
    }

    /// multiplicative := unary (( * | / ) unary)*
    fn multiplicative(&mut self) -> Result<Node, String> {
        let mut left = self.unary()?;
        loop {
            let op = match self.peek() {
                Some(Tok::Star) => BinOp::Mul,
                Some(Tok::Slash) => BinOp::Div,
                _ => return Ok(left),
            };
            self.pos += 1;
            let right = self.unary()?;
            left = Node::Bin(op, Box::new(left), Box::new(right));
        }
    }

    /// unary := ( - | + ) unary | power
    fn unary(&mut self) -> Result<Node, String> {
        self.depth += 1;
        let out = if self.depth > MAX_DEPTH { Err("Syntax error".to_string()) } else { self.unary_inner() };
        self.depth -= 1;
        out
    }

    fn unary_inner(&mut self) -> Result<Node, String> {
        if self.eat(&Tok::Minus) {
            return Ok(Node::Neg(Box::new(self.unary()?)));
        }
        if self.eat(&Tok::Plus) {
            return self.unary();
        }
        self.power()
    }

    /// power := postfix ( ^ unary )?   (right-associative, and the exponent may carry a sign)
    fn power(&mut self) -> Result<Node, String> {
        let base = self.postfix()?;
        if self.eat(&Tok::Caret) {
            let exp = self.unary()?;
            return Ok(Node::Bin(BinOp::Pow, Box::new(base), Box::new(exp)));
        }
        Ok(base)
    }

    /// postfix := primary %*
    fn postfix(&mut self) -> Result<Node, String> {
        let mut node = self.primary()?;
        while self.eat(&Tok::Pct) {
            node = Node::Pct(Box::new(node));
        }
        Ok(node)
    }

    fn primary(&mut self) -> Result<Node, String> {
        let Some(tok) = self.peek().cloned() else {
            return Err("Unexpected end of formula".to_string());
        };
        self.pos += 1;
        match tok {
            Tok::Num(n) => Ok(Node::Num(n)),
            Tok::LParen => {
                let inner = self.comparison()?;
                if !self.eat(&Tok::RParen) {
                    return Err(self.expected_close());
                }
                Ok(inner)
            }
            Tok::Ident(word) => self.word(word),
            Tok::RParen => Err("Unbalanced parentheses".to_string()),
            _ => Err("Syntax error".to_string()),
        }
    }

    /// A function call, a keyword, a cell or range, or a bookmark name.
    fn word(&mut self, word: String) -> Result<Node, String> {
        if self.peek() == Some(&Tok::LParen) {
            let func = find_func(&word).ok_or_else(|| format!("Unknown function: {}", word.to_uppercase()))?;
            self.pos += 1;
            let args = self.args()?;
            if args.len() < func.min || args.len() > func.max {
                return Err(format!("Wrong number of arguments for {}", func.name));
            }
            return Ok(Node::Call(func, args));
        }
        match word.to_ascii_uppercase().as_str() {
            "TRUE" => return Ok(Node::Num(1.0)),
            "FALSE" => return Ok(Node::Num(0.0)),
            "ABOVE" => return Ok(Node::Dir(Dir::Above)),
            "BELOW" => return Ok(Node::Dir(Dir::Below)),
            "LEFT" => return Ok(Node::Dir(Dir::Left)),
            "RIGHT" => return Ok(Node::Dir(Dir::Right)),
            _ => {}
        }
        if let Some((r, c)) = parse_cell_name(&word) {
            if self.eat(&Tok::Colon) {
                let Some(Tok::Ident(end)) = self.peek().cloned() else {
                    return Err("Syntax error".to_string());
                };
                let (r2, c2) = parse_cell_name(&end).ok_or_else(|| format!("Not a table cell: {end}"))?;
                self.pos += 1;
                return Ok(Node::Range((r.min(r2), c.min(c2)), (r.max(r2), c.max(c2))));
            }
            return Ok(Node::Cell(r, c, word));
        }
        Ok(Node::Name(word))
    }

    /// The arguments after an opening parenthesis, up to and including the closing one.
    fn args(&mut self) -> Result<Vec<Node>, String> {
        let mut args = Vec::new();
        if self.eat(&Tok::RParen) {
            return Ok(args);
        }
        loop {
            args.push(self.comparison()?);
            if self.eat(&Tok::Comma) {
                continue;
            }
            if self.eat(&Tok::RParen) {
                return Ok(args);
            }
            return Err(self.expected_close());
        }
    }
}

fn parse(expr: &str) -> Result<Node, String> {
    if expr.chars().count() > MAX_LEN {
        return Err("Syntax error".to_string());
    }
    let toks = lex(expr)?;
    let mut p = Parser { toks, pos: 0, depth: 0 };
    let node = p.comparison()?;
    match p.peek() {
        None => Ok(node),
        Some(Tok::RParen) => Err("Unbalanced parentheses".to_string()),
        Some(_) => Err("Syntax error".to_string()),
    }
}

// ---------------------------------------------------------------------------
// Evaluation
// ---------------------------------------------------------------------------

/// Evaluate a formula body (the text after `=`, such as `SUM(ABOVE)*2` or `(A1+B2)/3` or `IF(A1>5,1,0)`).
/// Booleans are 1.0 (true) and 0.0 (false).
pub fn evaluate(expr: &str, ctx: &dyn Context) -> Result<f64, String> {
    let node = parse(expr)?;
    eval(&node, ctx)
}

fn bool_num(b: bool) -> f64 {
    if b {
        1.0
    } else {
        0.0
    }
}

/// Two numbers are equal when they differ by less than rounding noise.
fn nearly_equal(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-12 * a.abs().max(b.abs()).max(1.0)
}

fn check_finite(n: f64) -> Result<f64, String> {
    if n.is_finite() {
        Ok(n)
    } else {
        Err("Number out of range".to_string())
    }
}

/// The cell at a reference. With no table, the text is tried as a bookmark or variable name instead.
fn cell_at(ctx: &dyn Context, r: usize, c: usize, text: &str) -> Result<Cell, String> {
    if let Some(cell) = ctx.cell(r, c) {
        return Ok(cell);
    }
    if ctx.size() == (0, 0) {
        return match ctx.name(text) {
            Some(n) => Ok(Cell::Number(n)),
            None => Err(format!("Undefined name: {text}")),
        };
    }
    Err(format!("Not a table cell: {text}"))
}

fn eval(node: &Node, ctx: &dyn Context) -> Result<f64, String> {
    match node {
        Node::Num(n) => Ok(*n),
        Node::Neg(inner) => Ok(-eval(inner, ctx)?),
        Node::Pct(inner) => Ok(eval(inner, ctx)? / 100.0),
        Node::Bin(op, l, r) => {
            let a = eval(l, ctx)?;
            let b = eval(r, ctx)?;
            match op {
                BinOp::Add => check_finite(a + b),
                BinOp::Sub => check_finite(a - b),
                BinOp::Mul => check_finite(a * b),
                BinOp::Div => {
                    if b == 0.0 {
                        Err("Division by zero".to_string())
                    } else {
                        check_finite(a / b)
                    }
                }
                BinOp::Pow => check_finite(a.powf(b)),
                BinOp::Eq => Ok(bool_num(nearly_equal(a, b))),
                BinOp::Ne => Ok(bool_num(!nearly_equal(a, b))),
                BinOp::Lt => Ok(bool_num(a < b && !nearly_equal(a, b))),
                BinOp::Le => Ok(bool_num(a < b || nearly_equal(a, b))),
                BinOp::Gt => Ok(bool_num(a > b && !nearly_equal(a, b))),
                BinOp::Ge => Ok(bool_num(a > b || nearly_equal(a, b))),
            }
        }
        Node::Call(func, args) => call(func, args, ctx),
        Node::Cell(r, c, text) => match cell_at(ctx, *r, *c, text)? {
            Cell::Number(n) => Ok(n),
            Cell::Empty | Cell::Text(_) => Ok(0.0),
        },
        Node::Name(name) => ctx.name(name).ok_or_else(|| format!("Undefined name: {name}")),
        Node::Range(..) | Node::Dir(_) => Err("Syntax error".to_string()),
    }
}

/// Add the numbers an argument stands for to a list. Ranges and directions give their numeric cells, a cell
/// gives its number or nothing, and anything else is evaluated as one value.
fn collect(node: &Node, ctx: &dyn Context, out: &mut Vec<f64>) -> Result<(), String> {
    match node {
        Node::Cell(r, c, text) => {
            if let Cell::Number(n) = cell_at(ctx, *r, *c, text)? {
                out.push(n);
            }
        }
        Node::Range((r1, c1), (r2, c2)) => {
            // Only the cells the table has: `A1:ZZZ999999999` must not loop over billions of names.
            let (rows, cols) = ctx.size();
            for r in *r1..=(*r2).min(rows.saturating_sub(1)) {
                for c in *c1..=(*c2).min(cols.saturating_sub(1)) {
                    if let Some(Cell::Number(n)) = ctx.cell(r, c) {
                        out.push(n);
                    }
                }
            }
        }
        Node::Dir(dir) => collect_direction(*dir, ctx, out)?,
        other => out.push(eval(other, ctx)?),
    }
    Ok(())
}

/// The numbers above, below, left or right of the formula's cell. Leading empty cells are skipped, then the run
/// of numbers is taken and ends at the first empty cell, text cell or table edge.
fn collect_direction(dir: Dir, ctx: &dyn Context, out: &mut Vec<f64>) -> Result<(), String> {
    let (row, col) = ctx.position().ok_or_else(|| "The formula is not in a table".to_string())?;
    let (rows, cols) = ctx.size();
    let mut found = false;
    let mut step = 1usize;
    loop {
        let (r, c) = match dir {
            Dir::Above => match row.checked_sub(step) {
                Some(r) => (r, col),
                None => break,
            },
            Dir::Left => match col.checked_sub(step) {
                Some(c) => (row, c),
                None => break,
            },
            Dir::Below => (row + step, col),
            Dir::Right => (row, col + step),
        };
        if r >= rows || c >= cols {
            break;
        }
        match ctx.cell(r, c) {
            Some(Cell::Number(n)) => {
                out.push(n);
                found = true;
            }
            Some(Cell::Empty) if !found => {}
            _ => break,
        }
        step += 1;
    }
    Ok(())
}

fn numbers(args: &[Node], ctx: &dyn Context) -> Result<Vec<f64>, String> {
    let mut out = Vec::new();
    for a in args {
        collect(a, ctx, &mut out)?;
    }
    Ok(out)
}

/// Round half away from zero. Negative digits round to the left of the point.
fn round_to(x: f64, digits: f64) -> f64 {
    let d = digits.trunc();
    if d >= 15.0 {
        return x;
    }
    if d >= 0.0 {
        let s = 10f64.powi(d as i32);
        (x * s).round() / s
    } else {
        let s = 10f64.powi((-d).min(300.0) as i32);
        (x / s).round() * s
    }
}

fn call(func: &Func, args: &[Node], ctx: &dyn Context) -> Result<f64, String> {
    let one = |i: usize| eval(&args[i], ctx);
    match func.name {
        "ABS" => Ok(one(0)?.abs()),
        "AND" => Ok(bool_num(one(0)? != 0.0 && one(1)? != 0.0)),
        "OR" => Ok(bool_num(one(0)? != 0.0 || one(1)? != 0.0)),
        "NOT" => Ok(bool_num(one(0)? == 0.0)),
        "TRUE" => Ok(1.0),
        "FALSE" => Ok(0.0),
        "IF" => {
            if one(0)? != 0.0 {
                one(1)
            } else {
                one(2)
            }
        }
        "INT" => Ok(one(0)?.trunc()),
        "SIGN" => {
            let x = one(0)?;
            Ok(if x > 0.0 {
                1.0
            } else if x < 0.0 {
                -1.0
            } else {
                0.0
            })
        }
        "MOD" => {
            let (x, y) = (one(0)?, one(1)?);
            if y == 0.0 {
                Err("Division by zero".to_string())
            } else {
                // The result takes the sign of the divisor, like a spreadsheet.
                Ok(x - y * (x / y).floor())
            }
        }
        "ROUND" => Ok(round_to(one(0)?, one(1)?)),
        "DEFINED" => Ok(bool_num(defined(&args[0], ctx))),
        "SUM" => check_finite(numbers(args, ctx)?.iter().sum()),
        "PRODUCT" => {
            let v = numbers(args, ctx)?;
            if v.is_empty() {
                Ok(0.0)
            } else {
                check_finite(v.iter().product())
            }
        }
        "COUNT" => Ok(numbers(args, ctx)?.len() as f64),
        "AVERAGE" => {
            let v = numbers(args, ctx)?;
            if v.is_empty() {
                Err("No numbers to use".to_string())
            } else {
                check_finite(v.iter().sum::<f64>() / v.len() as f64)
            }
        }
        "MIN" => numbers(args, ctx)?.into_iter().reduce(f64::min).ok_or_else(|| "No numbers to use".to_string()),
        "MAX" => numbers(args, ctx)?.into_iter().reduce(f64::max).ok_or_else(|| "No numbers to use".to_string()),
        other => Err(format!("Unknown function: {other}")),
    }
}

/// Whether `DEFINED(x)` is true: a known name, a cell holding a number, or any expression that evaluates.
fn defined(arg: &Node, ctx: &dyn Context) -> bool {
    match arg {
        Node::Name(name) => ctx.name(name).is_some(),
        Node::Cell(r, c, text) => matches!(cell_at(ctx, *r, *c, text), Ok(Cell::Number(_))),
        other => eval(other, ctx).is_ok(),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// A table of cells, the position of the formula in it, and some names.
    struct Grid {
        cells: Vec<Vec<Cell>>,
        pos: Option<(usize, usize)>,
        names: Vec<(&'static str, f64)>,
    }

    /// Run an evaluation on a thread and fail if it does not come back quickly.
    fn finishes(expr: String) -> Result<f64, String> {
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::Builder::new()
            .stack_size(256 * 1024)
            .spawn(move || {
                let _ = tx.send(evaluate(&expr, &NoContext));
            })
            .unwrap();
        rx.recv_timeout(std::time::Duration::from_secs(5)).expect("the formula hung or overflowed the stack")
    }

    #[test]
    fn crafted_formulas_end_fast() {
        // A range far larger than the table is clamped to it.
        let g = sales();
        assert_eq!(evaluate("SUM(A1:ZZZ999999999)", &g), Ok(10.0 + 2.0 + 20.0 + 3.0 + 30.0 + 5.0));
        // Deep nesting is an error, not a stack overflow, on a small stack.
        assert!(finishes("(".repeat(30_000)).is_err());
        assert!(finishes("-".repeat(30_000)).is_err());
        assert!(finishes(format!("{}1{}", "(".repeat(100), ")".repeat(100))).is_err());
        assert_eq!(finishes(format!("{}1{}", "(".repeat(20), ")".repeat(20))), Ok(1.0));
        // A very long formula is refused outright.
        assert!(finishes("1+".repeat(5000) + "1").is_err());
    }

    fn n(x: f64) -> Cell {
        Cell::Number(x)
    }

    fn t(s: &str) -> Cell {
        Cell::Text(s.to_string())
    }

    impl Context for Grid {
        fn cell(&self, row: usize, col: usize) -> Option<Cell> {
            self.cells.get(row).and_then(|r| r.get(col)).cloned()
        }
        fn position(&self) -> Option<(usize, usize)> {
            self.pos
        }
        fn size(&self) -> (usize, usize) {
            (self.cells.len(), self.cells.iter().map(Vec::len).max().unwrap_or(0))
        }
        fn name(&self, name: &str) -> Option<f64> {
            self.names.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| *v)
        }
    }

    /// A header row, three item rows and a total row whose formula sits at (4, 1).
    ///
    /// ```text
    ///   Item   Price  Qty
    ///   a      10     2
    ///   b      20     3
    ///   c      30     (empty)
    ///   Total  (here) 5
    /// ```
    fn sales() -> Grid {
        Grid {
            cells: vec![
                vec![t("Item"), t("Price"), t("Qty")],
                vec![t("a"), n(10.0), n(2.0)],
                vec![t("b"), n(20.0), n(3.0)],
                vec![t("c"), n(30.0), Cell::Empty],
                vec![t("Total"), Cell::Empty, n(5.0)],
            ],
            pos: Some((4, 1)),
            names: vec![("Rate", 0.5), ("Total_1", 7.0)],
        }
    }

    fn ev(expr: &str) -> Result<f64, String> {
        evaluate(expr, &sales())
    }

    fn ok(expr: &str) -> f64 {
        match ev(expr) {
            Ok(v) => v,
            Err(e) => panic!("{expr}: {e}"),
        }
    }

    fn err(expr: &str) -> String {
        match ev(expr) {
            Ok(v) => panic!("{expr} gave {v}, expected an error"),
            Err(e) => e,
        }
    }

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    #[test]
    fn cell_names_round_trip() {
        assert_eq!(cell_name(0, 0), "A1");
        assert_eq!(cell_name(2, 27), "AB3");
        assert_eq!(cell_name(0, 25), "Z1");
        assert_eq!(cell_name(9, 26), "AA10");
        assert_eq!(parse_cell_name("ab3"), Some((2, 27)));
        assert_eq!(parse_cell_name("AA1"), Some((0, 26)));
        assert_eq!(parse_cell_name("A1"), Some((0, 0)));
        assert_eq!(parse_cell_name("A0"), None);
        assert_eq!(parse_cell_name("A"), None);
        assert_eq!(parse_cell_name("1A"), None);
        assert_eq!(parse_cell_name("A1B"), None);
        assert_eq!(parse_cell_name(""), None);
        assert_eq!(parse_cell_name("Total1"), None);
        for col in [0, 1, 25, 26, 27, 51, 52, 701, 702, 703, 17575] {
            assert_eq!(parse_cell_name(&cell_name(4, col)), Some((4, col)));
        }
    }

    #[test]
    fn numbers_and_operators() {
        assert_eq!(ok("12"), 12.0);
        assert_eq!(ok("3.5"), 3.5);
        assert_eq!(ok(".5"), 0.5);
        assert_eq!(ok("1+2*3"), 7.0);
        assert_eq!(ok("(1+2)*3"), 9.0);
        assert_eq!(ok("10-4-3"), 3.0);
        assert_eq!(ok("100/5/2"), 10.0);
        assert_eq!(ok("  1 +\t2  "), 3.0);
        assert!(close(ok("1/3*3"), 1.0));
    }

    #[test]
    fn unary_signs() {
        assert_eq!(ok("-5"), -5.0);
        assert_eq!(ok("+5"), 5.0);
        assert_eq!(ok("--5"), 5.0);
        assert_eq!(ok("3--2"), 5.0);
        assert_eq!(ok("-(2+3)"), -5.0);
        assert_eq!(ok("2*-3"), -6.0);
    }

    #[test]
    fn power_binds_tighter_than_unary_minus() {
        assert_eq!(ok("2^3"), 8.0);
        assert_eq!(ok("-2^2"), -4.0);
        assert_eq!(ok("(-2)^2"), 4.0);
        assert_eq!(ok("2^3^2"), 512.0);
        assert_eq!(ok("2^-1"), 0.5);
        assert_eq!(ok("2*3^2"), 18.0);
        assert_eq!(ok("4^0.5"), 2.0);
    }

    #[test]
    fn percentages() {
        assert!(close(ok("50%"), 0.5));
        assert!(close(ok("200*10%"), 20.0));
        assert!(close(ok("200+200*10%"), 220.0));
        assert!(close(ok("50%%"), 0.005));
        assert!(close(ok("(10+10)%"), 0.2));
        assert!(close(ok("50%^2"), 0.25));
    }

    #[test]
    fn comparisons() {
        assert_eq!(ok("1<2"), 1.0);
        assert_eq!(ok("2<1"), 0.0);
        assert_eq!(ok("2<=2"), 1.0);
        assert_eq!(ok("3>2"), 1.0);
        assert_eq!(ok("2>=3"), 0.0);
        assert_eq!(ok("2<>3"), 1.0);
        assert_eq!(ok("2<>2"), 0.0);
        assert_eq!(ok("2=2"), 1.0);
        assert_eq!(ok("1+1=2"), 1.0);
        assert_eq!(ok("0.1+0.2=0.3"), 1.0);
        assert_eq!(ok("1+1<1+2"), 1.0);
    }

    #[test]
    fn booleans() {
        assert_eq!(ok("TRUE"), 1.0);
        assert_eq!(ok("FALSE"), 0.0);
        assert_eq!(ok("TRUE()"), 1.0);
        assert_eq!(ok("FALSE()"), 0.0);
        assert_eq!(ok("AND(1,1)"), 1.0);
        assert_eq!(ok("AND(1,0)"), 0.0);
        assert_eq!(ok("AND(2>1,3>2)"), 1.0);
        assert_eq!(ok("OR(0,0)"), 0.0);
        assert_eq!(ok("OR(0,5)"), 1.0);
        assert_eq!(ok("OR(1>2,3>2)"), 1.0);
        assert_eq!(ok("NOT(0)"), 1.0);
        assert_eq!(ok("NOT(7)"), 0.0);
        assert_eq!(ok("NOT(TRUE)"), 0.0);
        assert_eq!(ok("IF(AND(TRUE,NOT(FALSE)),10,20)"), 10.0);
    }

    #[test]
    fn sum_above_and_left() {
        assert_eq!(ok("SUM(ABOVE)"), 60.0);
        assert_eq!(ok("SUM(ABOVE)*2"), 120.0);
        // Left of (4, 1) is "Total", which is text, so nothing.
        assert_eq!(ok("SUM(LEFT)"), 0.0);
        let g = Grid { cells: vec![vec![n(1.0), n(2.0), n(3.0), Cell::Empty, n(9.0)]], pos: Some((0, 3)), names: vec![] };
        assert_eq!(evaluate("SUM(LEFT)", &g), Ok(6.0));
        assert_eq!(evaluate("SUM(RIGHT)", &g), Ok(9.0));
        assert_eq!(evaluate("sum(left)+sum(right)", &g), Ok(15.0));
    }

    #[test]
    fn direction_rules() {
        let g = Grid {
            cells: vec![
                vec![n(100.0)],
                vec![t("x")],
                vec![n(1.0)],
                vec![n(2.0)],
                vec![Cell::Empty],
                vec![n(4.0)],
                vec![n(8.0)],
                vec![Cell::Empty],
                vec![n(16.0)],
            ],
            pos: Some((4, 0)),
            names: vec![],
        };
        // Above stops at the text cell, so 100 is out.
        assert_eq!(evaluate("SUM(ABOVE)", &g), Ok(3.0));
        // Below: 4 and 8, then an empty cell ends the run.
        assert_eq!(evaluate("SUM(BELOW)", &g), Ok(12.0));
        let g2 = Grid { cells: vec![vec![Cell::Empty], vec![Cell::Empty], vec![n(5.0)], vec![Cell::Empty]], pos: Some((3, 0)), names: vec![] };
        // Leading empty cells are skipped.
        assert_eq!(evaluate("SUM(ABOVE)", &g2), Ok(5.0));
        let g3 = Grid { cells: vec![vec![Cell::Empty], vec![Cell::Empty]], pos: Some((1, 0)), names: vec![] };
        assert_eq!(evaluate("SUM(ABOVE)", &g3), Ok(0.0));
        assert_eq!(evaluate("COUNT(ABOVE)", &g3), Ok(0.0));
    }

    #[test]
    fn direction_needs_a_table() {
        assert_eq!(evaluate("SUM(ABOVE)", &NoContext), Err("The formula is not in a table".to_string()));
        let g = Grid { cells: vec![vec![n(1.0)]], pos: None, names: vec![] };
        assert!(evaluate("SUM(LEFT)", &g).is_err());
    }

    #[test]
    fn ranges() {
        assert_eq!(ok("SUM(B2:C3)"), 35.0);
        assert_eq!(ok("SUM(A1:C5)"), 10.0 + 2.0 + 20.0 + 3.0 + 30.0 + 5.0);
        assert_eq!(ok("sum(b2:b4)"), 60.0);
        // Corners in reverse order give the same rectangle.
        assert_eq!(ok("SUM(C3:B2)"), 35.0);
        assert_eq!(ok("COUNT(A1:C5)"), 6.0);
        assert_eq!(ok("SUM(B2:B4,C2)"), 62.0);
    }

    #[test]
    fn average_min_max_count_product() {
        assert_eq!(ok("AVERAGE(ABOVE)"), 20.0);
        assert_eq!(ok("AVERAGE(B2:B4)"), 20.0);
        assert_eq!(ok("AVERAGE(1,2,3,4)"), 2.5);
        assert_eq!(ok("MIN(B2:B4)"), 10.0);
        assert_eq!(ok("MAX(B2:B4)"), 30.0);
        assert_eq!(ok("MAX(1,5,3)"), 5.0);
        assert_eq!(ok("MIN(-1,5,3)"), -1.0);
        assert_eq!(ok("COUNT(B1:C4)"), 5.0);
        assert_eq!(ok("COUNT(1,2,3)"), 3.0);
        assert_eq!(ok("PRODUCT(B2:B4)"), 6000.0);
        assert_eq!(ok("PRODUCT(2,3,4)"), 24.0);
        assert_eq!(ok("SUM(1,2;3)"), 6.0);
    }

    #[test]
    fn empty_lists() {
        assert_eq!(ok("SUM(A1:A1)"), 0.0);
        assert_eq!(ok("PRODUCT(A1:A1)"), 0.0);
        assert_eq!(err("AVERAGE(A1:A1)"), "No numbers to use");
        assert_eq!(err("MIN(A1:A1)"), "No numbers to use");
        assert_eq!(err("MAX(A1:A1)"), "No numbers to use");
    }

    #[test]
    fn cell_references_in_arithmetic() {
        assert_eq!(ok("B2+B3"), 30.0);
        assert_eq!(ok("(B2+B3)/3"), 10.0);
        assert_eq!(ok("b2*c2"), 20.0);
        // Text and empty cells count as zero in arithmetic.
        assert_eq!(ok("A1+5"), 5.0);
        assert_eq!(ok("C4+1"), 1.0);
        // They are skipped in lists.
        assert_eq!(ok("SUM(A2,B2)"), 10.0);
        assert_eq!(ok("COUNT(A1,B2,C4)"), 1.0);
    }

    #[test]
    fn cell_references_outside_the_table() {
        assert_eq!(err("Z9+1"), "Not a table cell: Z9");
        assert_eq!(err("SUM(Z9)"), "Not a table cell: Z9");
        assert_eq!(err("SUM(A5:)"), "Syntax error");
        assert_eq!(err("SUM(A1:B)"), "Not a table cell: B");
    }

    #[test]
    fn if_with_comparison() {
        assert_eq!(ok("IF(B2>5,1,0)"), 1.0);
        assert_eq!(ok("IF(B2>50,1,0)"), 0.0);
        assert_eq!(ok("IF(SUM(ABOVE)>=60,SUM(ABOVE)*0.9,SUM(ABOVE))"), 54.0);
        assert_eq!(ok("IF(B2<>10,1,2)"), 2.0);
        assert_eq!(ok("IF(0,1/0,7)"), 7.0);
        assert_eq!(ok("IF(1,7,1/0)"), 7.0);
    }

    #[test]
    fn rounding() {
        assert_eq!(ok("ROUND(2.5,0)"), 3.0);
        assert_eq!(ok("ROUND(-2.5,0)"), -3.0);
        assert_eq!(ok("ROUND(1234,-2)"), 1200.0);
        assert_eq!(ok("ROUND(1250,-2)"), 1300.0);
        assert_eq!(ok("ROUND(-1250,-2)"), -1300.0);
        assert!(close(ok("ROUND(2.71828,2)"), 2.72));
        assert!(close(ok("ROUND(1/3,3)"), 0.333));
        assert_eq!(ok("ROUND(7,0)"), 7.0);
        assert_eq!(ok("ROUND(2.4,0)"), 2.0);
    }

    #[test]
    fn other_functions() {
        assert_eq!(ok("ABS(-4)"), 4.0);
        assert_eq!(ok("ABS(4)"), 4.0);
        assert_eq!(ok("INT(3.9)"), 3.0);
        assert_eq!(ok("INT(-1.5)"), -1.0);
        assert_eq!(ok("MOD(10,3)"), 1.0);
        assert_eq!(ok("MOD(9,3)"), 0.0);
        assert_eq!(ok("MOD(-7,3)"), 2.0);
        assert_eq!(ok("SIGN(-9)"), -1.0);
        assert_eq!(ok("SIGN(0)"), 0.0);
        assert_eq!(ok("SIGN(9)"), 1.0);
    }

    #[test]
    fn nested_functions() {
        assert!(close(ok("ROUND(AVERAGE(1,2,4),1)"), 2.3));
        assert_eq!(ok("MAX(SUM(1,2),SUM(3,4),ABS(-5))"), 7.0);
        assert_eq!(ok("SUM(ABS(-1),MAX(2,3),MIN(ABOVE))"), 1.0 + 3.0 + 10.0);
        assert_eq!(ok("IF(MOD(SUM(ABOVE),7)=4,100,200)"), 100.0);
        assert!(close(ok("ROUND(SUM(ABOVE)/7,2)"), 8.57));
        assert_eq!(ok("SUM((1+2)*3,-ABS(-4))"), 5.0);
    }

    #[test]
    fn names_from_the_context() {
        assert_eq!(ok("Rate*100"), 50.0);
        assert_eq!(ok("rate*100"), 50.0);
        assert_eq!(ok("SUM(Rate,Rate,1)"), 2.0);
        assert_eq!(ok("Total_1+1"), 8.0);
        assert_eq!(ok("SUM(ABOVE)*Rate"), 30.0);
        assert_eq!(err("Missing+1"), "Undefined name: Missing");
        assert_eq!(err("SUM(Missing)"), "Undefined name: Missing");
    }

    #[test]
    fn cell_like_names_without_a_table() {
        let g = Grid { cells: vec![], pos: None, names: vec![("A1", 4.0), ("Net", 2.0)] };
        assert_eq!(evaluate("A1*2", &g), Ok(8.0));
        assert_eq!(evaluate("SUM(A1,Net)", &g), Ok(6.0));
        assert_eq!(evaluate("B2+1", &g), Err("Undefined name: B2".to_string()));
        assert_eq!(evaluate("2+3", &NoContext), Ok(5.0));
    }

    #[test]
    fn defined_function() {
        assert_eq!(ok("DEFINED(Rate)"), 1.0);
        assert_eq!(ok("DEFINED(Nope)"), 0.0);
        assert_eq!(ok("DEFINED(B2)"), 1.0);
        assert_eq!(ok("DEFINED(A1)"), 0.0);
        assert_eq!(ok("DEFINED(C4)"), 0.0);
        assert_eq!(ok("DEFINED(Z9)"), 0.0);
        assert_eq!(ok("DEFINED(1/0)"), 0.0);
        assert_eq!(ok("DEFINED(1+1)"), 1.0);
        assert_eq!(ok("IF(DEFINED(Nope),Nope,-1)"), -1.0);
        let g = Grid { cells: vec![], pos: None, names: vec![("A1", 4.0)] };
        assert_eq!(evaluate("DEFINED(A1)", &g), Ok(1.0));
    }

    #[test]
    fn case_insensitivity() {
        assert_eq!(ok("sum(above)"), 60.0);
        assert_eq!(ok("Sum(Above)"), 60.0);
        assert_eq!(ok("if(b2>5,true,false)"), 1.0);
        assert_eq!(ok("Round(2.5,0)"), 3.0);
        assert_eq!(ok("b2+B3"), 30.0);
        assert_eq!(ok("aNd(tRuE,fAlSe)"), 0.0);
    }

    #[test]
    fn errors() {
        assert_eq!(err("1/0"), "Division by zero");
        assert_eq!(err("5/(2-2)"), "Division by zero");
        assert_eq!(err("MOD(5,0)"), "Division by zero");
        assert_eq!(err("FOO(1)"), "Unknown function: FOO");
        assert_eq!(err("foo(1)"), "Unknown function: FOO");
        assert_eq!(err("1+"), "Unexpected end of formula");
        assert_eq!(err(""), "Unexpected end of formula");
        assert_eq!(err("   "), "Unexpected end of formula");
        assert_eq!(err("1 2"), "Syntax error");
        assert_eq!(err("*3"), "Syntax error");
        assert_eq!(err("1+*2"), "Syntax error");
        assert_eq!(err("(1+2"), "Unbalanced parentheses");
        assert_eq!(err("1+2)"), "Unbalanced parentheses");
        assert_eq!(err(")"), "Unbalanced parentheses");
        assert_eq!(err("SUM(1,2"), "Unbalanced parentheses");
        assert_eq!(err("SUM(1 2)"), "Syntax error");
        assert_eq!(err("SUM()"), "Wrong number of arguments for SUM");
        assert_eq!(err("ROUND(1)"), "Wrong number of arguments for ROUND");
        assert_eq!(err("IF(1,2)"), "Wrong number of arguments for IF");
        assert_eq!(err("TRUE(1)"), "Wrong number of arguments for TRUE");
        assert_eq!(err("1 $ 2"), "Unexpected character: $");
        assert_eq!(err("."), "Syntax error");
        assert_eq!(err("ABOVE"), "Syntax error");
        assert_eq!(err("A1:B2+1"), "Syntax error");
        assert_eq!(err("(-8)^0.5"), "Number out of range");
    }

    #[test]
    fn errors_inside_lists_propagate() {
        assert_eq!(err("SUM(1,1/0)"), "Division by zero");
        assert_eq!(err("MAX(1,FOO(2))"), "Unknown function: FOO");
    }
}
