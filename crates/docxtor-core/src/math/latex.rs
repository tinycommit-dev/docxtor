//! LaTeX to OMML: the subset people type into an equation box.
//!
//! The reader works on tokens (so spaces never matter), builds Word's structures as it goes and reports problems in
//! plain words with the character they were found at.

use super::symbols as sym;
use super::*;

#[derive(Clone, Copy, PartialEq, Debug)]
enum Sty {
    Default,
    Plain,
    Bold,
    BoldItalic,
    Italic,
}

#[derive(Clone, Debug)]
struct RunSpec {
    text: String,
    sty: Sty,
    nor: bool,
    aln: bool,
}

#[derive(Clone, Debug)]
enum Item {
    Run(RunSpec),
    El(Element),
    Group(Vec<Item>),
}

#[derive(Clone, PartialEq, Debug)]
enum Tok {
    Char(char),
    Cmd(String),
    Open,
    Close,
    Sup,
    Sub,
    Amp,
    Break,
}

#[derive(Clone, Debug)]
struct Token {
    tok: Tok,
    pos: usize,
    end: usize,
}

#[derive(Clone, Copy, PartialEq)]
enum Stop {
    Eof,
    Brace,
    Bracket,
    Left,
    Env,
}

fn run(text: impl Into<String>) -> Item {
    Item::Run(RunSpec { text: text.into(), sty: Sty::Default, nor: false, aln: false })
}

fn lex(src: &[char]) -> Result<Vec<Token>, String> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < src.len() {
        let c = src[i];
        let (tok, end) = match c {
            c if c.is_whitespace() => {
                i += 1;
                continue;
            }
            // People paste `$x$`; the dollar signs mean nothing here.
            '$' => {
                i += 1;
                continue;
            }
            '{' => (Tok::Open, i + 1),
            '}' => (Tok::Close, i + 1),
            '^' => (Tok::Sup, i + 1),
            '_' => (Tok::Sub, i + 1),
            '&' => (Tok::Amp, i + 1),
            '\\' => {
                let Some(&d) = src.get(i + 1) else {
                    return Err(format!("A backslash at the end needs a command after it, like \\alpha (character {}).", i + 1));
                };
                if d.is_ascii_alphabetic() {
                    let mut j = i + 1;
                    while j < src.len() && src[j].is_ascii_alphabetic() {
                        j += 1;
                    }
                    (Tok::Cmd(src[i + 1..j].iter().collect()), j)
                } else if d == '\\' {
                    (Tok::Break, i + 2)
                } else {
                    (Tok::Cmd(d.to_string()), i + 2)
                }
            }
            c => (Tok::Char(c), i + 1),
        };
        out.push(Token { tok, pos: i, end });
        i = end;
    }
    Ok(out)
}

/// Read LaTeX into an `m:oMath` element.
pub fn latex_to_omml(src: &str) -> Result<Element, String> {
    let chars: Vec<char> = src.chars().collect();
    let toks = lex(&chars)?;
    if toks.is_empty() {
        return Err("The equation is empty.".into());
    }
    let mut p = Parser { src: chars, toks, i: 0 };
    let items = p.seq(Stop::Eof)?;
    if let Some(t) = p.peek() {
        // `seq` only stops early at something that does not belong here.
        return Err(format!("Unexpected {} at character {}.", describe(&t.tok), t.pos + 1));
    }
    let mut omath = Element::new("m:oMath");
    for e in finish(items) {
        omath.children.push(Node::Element(e));
    }
    if omath.children.is_empty() {
        return Err("The equation is empty.".into());
    }
    Ok(omath)
}

fn describe(t: &Tok) -> String {
    match t {
        Tok::Char(c) => format!("\"{c}\""),
        Tok::Cmd(c) => format!("\\{c}"),
        Tok::Open => "{".into(),
        Tok::Close => "}".into(),
        Tok::Sup => "^".into(),
        Tok::Sub => "_".into(),
        Tok::Amp => "&".into(),
        Tok::Break => "\\\\".into(),
    }
}

struct Parser {
    src: Vec<char>,
    toks: Vec<Token>,
    i: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Token> {
        self.toks.get(self.i)
    }

    fn peek_tok(&self) -> Option<&Tok> {
        self.toks.get(self.i).map(|t| &t.tok)
    }

    fn next(&mut self) -> Option<Token> {
        let t = self.toks.get(self.i).cloned();
        if t.is_some() {
            self.i += 1;
        }
        t
    }

    fn at_cmd(&self, name: &str) -> bool {
        matches!(self.peek_tok(), Some(Tok::Cmd(c)) if c == name)
    }

    /// Whether something that can be an argument comes next.
    fn has_arg(&self) -> bool {
        match self.peek_tok() {
            None | Some(Tok::Close | Tok::Amp | Tok::Break) => false,
            Some(Tok::Cmd(c)) => !matches!(c.as_str(), "right" | "middle" | "end"),
            _ => true,
        }
    }

    // ---- sequences -------------------------------------------------------

    fn seq(&mut self, stop: Stop) -> Result<Vec<Item>, String> {
        let mut items = Vec::new();
        while let Some(t) = self.peek().cloned() {
            match (&t.tok, stop) {
                (Tok::Close, Stop::Brace) => return Ok(items),
                (Tok::Close, _) => return Err(format!("There is a }} at character {} with no {{ before it.", t.pos + 1)),
                (Tok::Char(']'), Stop::Bracket) => return Ok(items),
                (Tok::Cmd(c), Stop::Left) if c == "right" || c == "middle" => return Ok(items),
                (Tok::Cmd(c), Stop::Env) if c == "end" => return Ok(items),
                (Tok::Amp | Tok::Break, Stop::Env) => return Ok(items),
                (Tok::Amp, _) => return Err(format!("An & (character {}) only works inside \\begin{{matrix}} ... \\end{{matrix}}, cases or aligned.", t.pos + 1)),
                (Tok::Break, _) => return Err(format!("A line break \\\\ (character {}) only works inside \\begin{{matrix}} ... \\end{{matrix}}, cases or aligned.", t.pos + 1)),
                (Tok::Cmd(c), _) if c == "right" => return Err(format!("\\right at character {} has no \\left before it.", t.pos + 1)),
                (Tok::Cmd(c), _) if c == "middle" => return Err(format!("\\middle at character {} has no \\left before it.", t.pos + 1)),
                (Tok::Cmd(c), _) if c == "end" => return Err(format!("\\end at character {} has no \\begin before it.", t.pos + 1)),
                _ => items.push(self.atom_with_scripts()?),
            }
        }
        Ok(items)
    }

    fn braced(&mut self) -> Result<Vec<Item>, String> {
        let open = self.next().expect("caller checked for a brace");
        let items = self.seq(Stop::Brace)?;
        match self.next() {
            Some(Token { tok: Tok::Close, .. }) => Ok(items),
            _ => Err(format!("The {{ at character {} is never closed. Add a }} after it.", open.pos + 1)),
        }
    }

    /// An argument: a braced group, or the single token after a command (`\frac12`).
    fn arg(&mut self, what: &str) -> Result<Vec<Item>, String> {
        match self.peek_tok() {
            Some(Tok::Open) => self.braced(),
            _ if self.has_arg() => Ok(vec![self.atom()?]),
            _ => Err(format!("{what} needs something after it.")),
        }
    }

    // ---- atoms and scripts -------------------------------------------------

    fn atom_with_scripts(&mut self) -> Result<Item, String> {
        let base = if matches!(self.peek_tok(), Some(Tok::Sup | Tok::Sub)) { Item::Group(vec![]) } else { self.atom()? };
        self.scripts(base)
    }

    #[allow(clippy::type_complexity)]
    fn script_parts(&mut self) -> Result<(Option<Vec<Item>>, Option<Vec<Item>>), String> {
        let (mut sub, mut sup): (Option<Vec<Item>>, Option<Vec<Item>>) = (None, None);
        loop {
            match self.peek_tok() {
                Some(Tok::Sup) => {
                    let t = self.next().unwrap();
                    if sup.is_some() {
                        return Err(format!("A second ^ at character {}. Put everything in one group, like x^{{a+b}}.", t.pos + 1));
                    }
                    if !self.has_arg() {
                        return Err(format!("The ^ at character {} needs something after it, like x^2 or x^{{n+1}}.", t.pos + 1));
                    }
                    sup = Some(self.arg("^")?);
                }
                Some(Tok::Sub) => {
                    let t = self.next().unwrap();
                    if sub.is_some() {
                        return Err(format!("A second _ at character {}. Put everything in one group, like x_{{a,b}}.", t.pos + 1));
                    }
                    if !self.has_arg() {
                        return Err(format!("The _ at character {} needs something after it, like x_1 or x_{{n+1}}.", t.pos + 1));
                    }
                    sub = Some(self.arg("_")?);
                }
                Some(Tok::Char('\'')) => {
                    let mut n = 0;
                    while matches!(self.peek_tok(), Some(Tok::Char('\''))) {
                        self.next();
                        n += 1;
                    }
                    let prime = match n {
                        1 => "′",
                        2 => "″",
                        _ => "‴",
                    };
                    let mut items = vec![run(prime)];
                    if matches!(self.peek_tok(), Some(Tok::Sup)) {
                        self.next();
                        items.extend(self.arg("^")?);
                    }
                    if sup.is_some() {
                        return Err("A prime and a ^ on the same symbol: put them in one group.".into());
                    }
                    sup = Some(items);
                }
                _ => break,
            }
        }
        Ok((sub, sup))
    }

    fn scripts(&mut self, base: Item) -> Result<Item, String> {
        let (sub, sup) = self.script_parts()?;
        if sub.is_none() && sup.is_none() {
            return Ok(base);
        }
        Ok(Item::El(build_scripts(base, sub, sup)))
    }

    fn atom(&mut self) -> Result<Item, String> {
        let Some(t) = self.next() else {
            return Err("The equation ends too soon: something is missing at the end.".into());
        };
        match &t.tok {
            Tok::Open => {
                self.i -= 1;
                Ok(Item::Group(self.braced()?))
            }
            Tok::Close => Err(format!("There is a }} at character {} with no {{ before it.", t.pos + 1)),
            Tok::Sup | Tok::Sub => Err(format!("The {} at character {} needs something before it.", describe(&t.tok), t.pos + 1)),
            Tok::Amp | Tok::Break => Err(format!("{} at character {} is only for matrices, cases and aligned equations.", describe(&t.tok), t.pos + 1)),
            Tok::Char('-') => Ok(run("−")),
            Tok::Char('~') => Ok(run("\u{00A0}")),
            Tok::Char('\'') => Ok(run("′")),
            Tok::Char(c) => Ok(run(c.to_string())),
            Tok::Cmd(name) => self.command(name, &t),
        }
    }

    // ---- commands ----------------------------------------------------------

    fn command(&mut self, name: &str, t: &Token) -> Result<Item, String> {
        if let Some(c) = sym::symbol(name) {
            return Ok(run(c));
        }
        if let Some((chr, under)) = sym::nary(name) {
            return self.nary(chr, under);
        }
        if let Some(limits) = sym::function(name) {
            return self.function(name, limits);
        }
        if let Some(chr) = sym::accent(name) {
            let body = self.arg(&format!("\\{name}"))?;
            return Ok(Item::El(structure("m:acc", "m:accPr", vec![val("m:chr", chr)], vec![arg("m:e", finish(body))])));
        }
        if let Some(sp) = sym::space(name) {
            return Ok(run(sp));
        }
        let cmd = format!("\\{name}");
        match name {
            "frac" | "dfrac" | "tfrac" | "cfrac" => {
                if !self.has_arg() {
                    return Err(format!("\\frac at character {} needs two groups, like \\frac{{a}}{{b}}.", t.pos + 1));
                }
                let a = self.arg(&cmd)?;
                if !self.has_arg() {
                    return Err(format!("\\frac at character {} needs a second group for the bottom, like \\frac{{a}}{{b}}.", t.pos + 1));
                }
                let b = self.arg(&cmd)?;
                Ok(Item::El(structure("m:f", "m:fPr", vec![], vec![arg("m:num", finish(a)), arg("m:den", finish(b))])))
            }
            // Linear and skewed fractions (Word's `m:type` "lin" and "skw") and the one without a bar ("noBar"): see
            // the `m:f` arm of `tolatex` for how they are written.
            "nicefrac" | "sfrac" => {
                if !self.has_arg() {
                    return Err(format!("{cmd} at character {} needs two groups, like {cmd}{{a}}{{b}}.", t.pos + 1));
                }
                let a = self.arg(&cmd)?;
                if !self.has_arg() {
                    return Err(format!("{cmd} at character {} needs a second group, like {cmd}{{a}}{{b}}.", t.pos + 1));
                }
                let b = self.arg(&cmd)?;
                let ty = if name == "nicefrac" { "lin" } else { "skw" };
                Ok(Item::El(structure("m:f", "m:fPr", vec![val("m:type", ty)], vec![arg("m:num", finish(a)), arg("m:den", finish(b))])))
            }
            "genfrac" => {
                // \genfrac{left}{right}{thickness}{style}{top}{bottom}: a zero thickness is a fraction without a bar,
                // and delimiters wrap it (so `\genfrac(){0pt}{}{n}{k}` is `\binom{n}{k}`). The style is read and ignored.
                let usage = "\\genfrac needs six groups, like \\genfrac{}{}{0pt}{}{a}{b}";
                let mut raw = Vec::new();
                for n in 0..4 {
                    // The two delimiters may be a single character without braces, as TeX allows: \genfrac(){0pt}{}.
                    match self.peek_tok().cloned() {
                        Some(Tok::Open) => raw.push(self.raw_group(&cmd)?),
                        Some(Tok::Char(c)) if n < 2 => {
                            self.next();
                            raw.push(c.to_string());
                        }
                        Some(Tok::Cmd(c)) if n < 2 => {
                            self.next();
                            raw.push(c);
                        }
                        _ => return Err(format!("{usage} (character {}).", t.pos + 1)),
                    }
                }
                if !self.has_arg() {
                    return Err(format!("{usage} (character {}).", t.pos + 1));
                }
                let a = self.arg(&cmd)?;
                if !self.has_arg() {
                    return Err(format!("{usage} (character {}).", t.pos + 1));
                }
                let b = self.arg(&cmd)?;
                let thickness = raw[2].trim().trim_end_matches("pt").trim_end_matches("em").trim_end_matches("ex").trim();
                let bar = thickness.is_empty() || (thickness.parse::<f64>() != Ok(0.0));
                let kids = vec![arg("m:num", finish(a)), arg("m:den", finish(b))];
                let f = structure("m:f", "m:fPr", if bar { vec![] } else { vec![val("m:type", "noBar")] }, kids);
                let delim = |s: &str| s.trim().to_string();
                let (l, r) = (delim(&raw[0]), delim(&raw[1]));
                if l.is_empty() && r.is_empty() {
                    Ok(Item::El(f))
                } else {
                    Ok(Item::El(delimited(&l, &r, None, vec![vec![f]])))
                }
            }
            "binom" => {
                if !self.has_arg() {
                    return Err(format!("\\binom at character {} needs two groups, like \\binom{{n}}{{k}}.", t.pos + 1));
                }
                let a = self.arg(&cmd)?;
                if !self.has_arg() {
                    return Err(format!("\\binom at character {} needs a second group, like \\binom{{n}}{{k}}.", t.pos + 1));
                }
                let b = self.arg(&cmd)?;
                let f = structure("m:f", "m:fPr", vec![val("m:type", "noBar")], vec![arg("m:num", finish(a)), arg("m:den", finish(b))]);
                Ok(Item::El(delimited("(", ")", None, vec![vec![f]])))
            }
            "sqrt" => {
                let deg = if matches!(self.peek_tok(), Some(Tok::Char('['))) {
                    let open = self.next().unwrap();
                    let items = self.seq(Stop::Bracket)?;
                    match self.next() {
                        Some(Token { tok: Tok::Char(']'), .. }) => Some(items),
                        _ => return Err(format!("The [ at character {} is never closed. Add a ] after the root's degree.", open.pos + 1)),
                    }
                } else {
                    None
                };
                if !self.has_arg() {
                    return Err(format!("\\sqrt at character {} needs a group, like \\sqrt{{x}}.", t.pos + 1));
                }
                let body = self.arg(&cmd)?;
                let mut props = vec![];
                if deg.is_none() {
                    props.push(val("m:degHide", "1"));
                }
                Ok(Item::El(structure("m:rad", "m:radPr", props, vec![arg("m:deg", deg.map(finish).unwrap_or_default()), arg("m:e", finish(body))])))
            }
            "left" => self.left_right(t),
            "begin" => self.environment(t),
            "text" | "textrm" | "textnormal" | "mbox" | "textit" | "textbf" => {
                let raw = self.raw_group(&cmd)?;
                Ok(Item::Run(RunSpec { text: raw, sty: Sty::Default, nor: true, aln: false }))
            }
            "mathbf" | "boldsymbol" | "bm" | "mathrm" | "mathup" | "mathit" | "mathsf" | "mathtt" | "mathbb" | "mathcal" | "mathscr" => {
                let items = self.arg(&cmd)?;
                Ok(Item::Group(restyle(items, name)))
            }
            "operatorname" => {
                if matches!(self.peek_tok(), Some(Tok::Char('*'))) {
                    self.next();
                }
                let raw = self.raw_group(&cmd)?;
                let under = matches!(raw.as_str(), "lim" | "limsup" | "liminf" | "max" | "min" | "sup" | "inf" | "det" | "gcd" | "argmax" | "argmin");
                self.function(&raw, under)
            }
            "overline" | "underline" => {
                let body = self.arg(&cmd)?;
                let pos = if name == "overline" { "top" } else { "bot" };
                Ok(Item::El(structure("m:bar", "m:barPr", vec![val("m:pos", pos)], vec![arg("m:e", finish(body))])))
            }
            "overbrace" | "underbrace" => {
                let body = self.arg(&cmd)?;
                let (chr, pos, vert) = if name == "overbrace" { ("⏞", "top", "bot") } else { ("⏟", "bot", "top") };
                Ok(Item::El(structure("m:groupChr", "m:groupChrPr", vec![val("m:chr", chr), val("m:pos", pos), val("m:vertJc", vert)], vec![arg("m:e", finish(body))])))
            }
            "overset" | "stackrel" | "underset" => {
                if !self.has_arg() {
                    return Err(format!("{cmd} needs two groups, like {cmd}{{a}}{{b}}."));
                }
                let a = self.arg(&cmd)?;
                if !self.has_arg() {
                    return Err(format!("{cmd} needs a second group, like {cmd}{{a}}{{b}}."));
                }
                let b = self.arg(&cmd)?;
                let (tag, ptag, key) = if name == "underset" { ("m:limLow", "m:limLowPr", "m:lim") } else { ("m:limUpp", "m:limUppPr", "m:lim") };
                Ok(Item::El(structure(tag, ptag, vec![], vec![arg("m:e", finish(b)), arg(key, finish(a))])))
            }
            "boxed" => {
                let body = self.arg(&cmd)?;
                Ok(Item::El(structure("m:borderBox", "m:borderBoxPr", vec![], vec![arg("m:e", finish(body))])))
            }
            "phantom" | "hphantom" | "vphantom" => {
                let body = self.arg(&cmd)?;
                Ok(Item::El(structure("m:phant", "m:phantPr", vec![val("m:show", "0")], vec![arg("m:e", finish(body))])))
            }
            "prescript" => {
                let sup = self.arg(&cmd)?;
                let sub = self.arg(&cmd)?;
                let base = self.arg(&cmd)?;
                Ok(Item::El(structure("m:sPre", "m:sPrePr", vec![], vec![arg("m:sub", finish(sub)), arg("m:sup", finish(sup)), arg("m:e", finish(base))])))
            }
            // Size and style switches have no meaning in Word's equations: leave them out.
            "displaystyle" | "textstyle" | "scriptstyle" | "scriptscriptstyle" | "limits" | "nolimits" | "big" | "Big" | "bigg" | "Bigg" | "bigl" | "bigr" | "Bigl" | "Bigr" | "biggl" | "biggr" | "Biggl" | "Biggr" | "!" | "relax" => Ok(Item::Group(vec![])),
            "^" | "~" => Ok(run(name)),
            "{" => Ok(run("{")),
            "}" => Ok(run("}")),
            "%" | "&" | "$" | "#" | "_" => Ok(run(name)),
            "|" => Ok(run("‖")),
            "lbrack" => Ok(run("[")),
            "rbrack" => Ok(run("]")),
            "right" | "middle" | "end" => Err(format!("{cmd} at character {} has nothing to match.", t.pos + 1)),
            _ => Err(format!("Unknown command {cmd} at character {}. Check the spelling, or type the symbol itself.", t.pos + 1)),
        }
    }

    /// The text between braces after a command, as typed.
    fn raw_group(&mut self, cmd: &str) -> Result<String, String> {
        let Some(open) = self.peek().cloned().filter(|t| t.tok == Tok::Open) else {
            return Err(format!("{cmd} needs its text in braces, like {cmd}{{some words}}."));
        };
        let mut depth = 0;
        let mut j = self.i;
        let close = loop {
            let Some(t) = self.toks.get(j) else {
                return Err(format!("The {{ at character {} is never closed. Add a }} after it.", open.pos + 1));
            };
            match t.tok {
                Tok::Open => depth += 1,
                Tok::Close => {
                    depth -= 1;
                    if depth == 0 {
                        break t.clone();
                    }
                }
                _ => {}
            }
            j += 1;
        };
        let raw: Vec<char> = self.src[open.end..close.pos].to_vec();
        self.i = j + 1;
        let mut out = String::new();
        let mut k = 0;
        while k < raw.len() {
            match raw[k] {
                '\\' if k + 1 < raw.len() => {
                    out.push(raw[k + 1]);
                    k += 2;
                }
                '{' | '}' => k += 1,
                c => {
                    out.push(c);
                    k += 1;
                }
            }
        }
        Ok(out)
    }

    // ---- large operators and functions -------------------------------------------

    fn nary(&mut self, chr: &str, under_default: bool) -> Result<Item, String> {
        let mut limits: Option<bool> = None;
        loop {
            if self.at_cmd("limits") {
                limits = Some(true);
            } else if self.at_cmd("nolimits") {
                limits = Some(false);
            } else {
                break;
            }
            self.next();
        }
        let (sub, sup) = self.script_parts()?;
        while self.at_cmd("limits") || self.at_cmd("nolimits") {
            limits = Some(self.at_cmd("limits"));
            self.next();
        }
        let body = self.operands(true)?;
        let under = limits.unwrap_or(under_default);
        let mut props = vec![val("m:chr", chr), val("m:limLoc", if under { "undOvr" } else { "subSup" }), val("m:grow", "1")];
        if sub.is_none() {
            props.push(val("m:subHide", "1"));
        }
        if sup.is_none() {
            props.push(val("m:supHide", "1"));
        }
        Ok(Item::El(structure(
            "m:nary",
            "m:naryPr",
            props,
            vec![arg("m:sub", sub.map(finish).unwrap_or_default()), arg("m:sup", sup.map(finish).unwrap_or_default()), arg("m:e", finish(body))],
        )))
    }

    fn function(&mut self, name: &str, limits: bool) -> Result<Item, String> {
        let name_item = Item::Run(RunSpec { text: name.to_string(), sty: Sty::Plain, nor: false, aln: false });
        let (sub, sup) = self.script_parts()?;
        let mut fname = Element::new("m:fName");
        if limits && sub.is_some() && sup.is_none() {
            fname.children.push(Node::Element(structure("m:limLow", "m:limLowPr", vec![], vec![arg("m:e", finish(vec![name_item])), arg("m:lim", finish(sub.unwrap_or_default()))])));
        } else if sub.is_some() || sup.is_some() {
            fname.children.push(Node::Element(build_scripts(name_item, sub, sup)));
        } else {
            for x in finish(vec![name_item]) {
                fname.children.push(Node::Element(x));
            }
        }
        let body = self.operand_opt()?;
        Ok(Item::El(func_el(fname, body)))
    }

    fn operand_opt(&mut self) -> Result<Vec<Item>, String> {
        if self.has_arg() {
            self.operand()
        } else {
            Ok(vec![])
        }
    }

    /// The things a large operator applies to: everything up to the next `+`, `-`, `=`, comparison or the end of
    /// the group. Brackets keep their contents together.
    fn operands(&mut self, stop_ops: bool) -> Result<Vec<Item>, String> {
        let mut out: Vec<Item> = Vec::new();
        while let Some(t) = self.peek().cloned() {
            match &t.tok {
                Tok::Close | Tok::Amp | Tok::Break => break,
                Tok::Char(')' | ']') => break,
                Tok::Cmd(c) if matches!(c.as_str(), "right" | "middle" | "end") => break,
                Tok::Char(c) if stop_ops && matches!(c, '+' | '-' | '=' | '<' | '>' | ',' | ';') && !(out.is_empty() && matches!(c, '+' | '-')) => break,
                Tok::Cmd(c) if stop_ops && matches!(c.as_str(), "pm" | "mp" | "leq" | "le" | "geq" | "ge" | "neq" | "ne" | "approx" | "equiv" | "sim" | "simeq" | "cong" | "propto" | "to" | "rightarrow" | "Rightarrow" | "Leftrightarrow" | "iff" | "implies" | "in" | "notin" | "subset" | "subseteq" | "supset" | "supseteq" | "quad" | "qquad") => break,
                _ => {}
            }
            out.extend(self.operand()?);
        }
        Ok(out)
    }

    /// One operand: a bracketed group, a number, or one atom with its scripts.
    fn operand(&mut self) -> Result<Vec<Item>, String> {
        match self.peek_tok().cloned() {
            Some(Tok::Char(c @ ('(' | '['))) => {
                let close = if c == '(' { ')' } else { ']' };
                let mut out = vec![self.atom()?];
                loop {
                    match self.peek_tok() {
                        None | Some(Tok::Close | Tok::Amp | Tok::Break) => break,
                        Some(Tok::Cmd(c)) if matches!(c.as_str(), "right" | "middle" | "end") => break,
                        Some(Tok::Char(ch)) if *ch == close => {
                            out.push(self.atom_with_scripts()?);
                            break;
                        }
                        _ => out.extend(self.operand()?),
                    }
                }
                Ok(out)
            }
            Some(Tok::Char(c)) if c.is_ascii_digit() => {
                let mut out = Vec::new();
                loop {
                    let more = match self.toks.get(self.i + 1).map(|t| &t.tok) {
                        Some(Tok::Char(d)) if d.is_ascii_digit() => true,
                        Some(Tok::Char('.')) => matches!(self.toks.get(self.i + 2).map(|t| &t.tok), Some(Tok::Char(d)) if d.is_ascii_digit()),
                        _ => false,
                    };
                    if more {
                        out.push(self.atom()?);
                    } else {
                        out.push(self.atom_with_scripts()?);
                        break;
                    }
                }
                Ok(out)
            }
            _ => Ok(vec![self.atom_with_scripts()?]),
        }
    }

    // ---- delimiters and environments -------------------------------------------------

    fn delim_after(&mut self, what: &str) -> Result<String, String> {
        let Some(t) = self.next() else {
            return Err(format!("{what} needs a bracket after it, like {what}(."));
        };
        let found = match &t.tok {
            Tok::Char(c) => sym::delimiter(&c.to_string()),
            Tok::Cmd(n) if n == "|" => Some("‖"),
            Tok::Cmd(n) => sym::delimiter(n),
            _ => None,
        };
        found.map(String::from).ok_or_else(|| format!("{what} at character {} can't be followed by {}. Use ( [ \\{{ | \\langle or . for none.", t.pos + 1, describe(&t.tok)))
    }

    fn left_right(&mut self, t: &Token) -> Result<Item, String> {
        let beg = self.delim_after("\\left")?;
        let mut cols: Vec<Vec<Item>> = Vec::new();
        let mut sep: Option<String> = None;
        let end = loop {
            cols.push(self.seq(Stop::Left)?);
            match self.next() {
                Some(Token { tok: Tok::Cmd(c), .. }) if c == "middle" => {
                    let d = self.delim_after("\\middle")?;
                    sep.get_or_insert(d);
                }
                Some(Token { tok: Tok::Cmd(c), .. }) if c == "right" => break self.delim_after("\\right")?,
                _ => return Err(format!("\\left at character {} has no matching \\right.", t.pos + 1)),
            }
        };
        let cols = cols.into_iter().map(finish).collect();
        Ok(Item::El(delimited(&beg, &end, sep.as_deref(), cols)))
    }

    fn environment(&mut self, t: &Token) -> Result<Item, String> {
        if !matches!(self.peek_tok(), Some(Tok::Open)) {
            return Err(format!("\\begin at character {} needs a name in braces, like \\begin{{matrix}}.", t.pos + 1));
        }
        self.next();
        let mut name = String::new();
        loop {
            match self.next() {
                Some(Token { tok: Tok::Char(c), .. }) => name.push(c),
                Some(Token { tok: Tok::Close, .. }) => break,
                _ => return Err(format!("\\begin at character {} is missing the closing }} after its name.", t.pos + 1)),
            }
        }
        if name == "array" && matches!(self.peek_tok(), Some(Tok::Open)) {
            // The column layout means nothing here.
            self.raw_group("\\begin{array}")?;
        }
        let mut rows: Vec<Vec<Vec<Item>>> = Vec::new();
        let mut row: Vec<Vec<Item>> = Vec::new();
        loop {
            let cell = self.seq(Stop::Env)?;
            match self.next() {
                Some(Token { tok: Tok::Amp, .. }) => row.push(cell),
                Some(Token { tok: Tok::Break, .. }) => {
                    row.push(cell);
                    rows.push(std::mem::take(&mut row));
                }
                Some(Token { tok: Tok::Cmd(c), .. }) if c == "end" => {
                    row.push(cell);
                    rows.push(std::mem::take(&mut row));
                    let mut closing = String::new();
                    if matches!(self.peek_tok(), Some(Tok::Open)) {
                        self.next();
                        loop {
                            match self.next() {
                                Some(Token { tok: Tok::Char(c), .. }) => closing.push(c),
                                Some(Token { tok: Tok::Close, .. }) => break,
                                _ => break,
                            }
                        }
                    }
                    if closing != name {
                        return Err(format!("\\begin{{{name}}} at character {} ends with \\end{{{closing}}}. It should end with \\end{{{name}}}.", t.pos + 1));
                    }
                    break;
                }
                _ => return Err(format!("\\begin{{{name}}} at character {} has no matching \\end{{{name}}}.", t.pos + 1)),
            }
        }
        // A trailing `\\` leaves an empty last row.
        if rows.len() > 1 && rows.last().is_some_and(|r| r.iter().all(|c| c.is_empty())) {
            rows.pop();
        }
        let rows: Vec<Vec<Vec<Element>>> = rows.into_iter().map(|r| r.into_iter().map(finish).collect()).collect();
        let (beg, end) = match name.as_str() {
            "pmatrix" => ("(", ")"),
            "bmatrix" => ("[", "]"),
            "Bmatrix" => ("{", "}"),
            "vmatrix" => ("|", "|"),
            "Vmatrix" => ("‖", "‖"),
            _ => ("", ""),
        };
        let el = match name.as_str() {
            "matrix" | "pmatrix" | "bmatrix" | "Bmatrix" | "vmatrix" | "Vmatrix" | "smallmatrix" | "array" => {
                let m = matrix(rows);
                if beg.is_empty() {
                    m
                } else {
                    delimited(beg, end, None, vec![vec![m]])
                }
            }
            "cases" => delimited("{", "", None, vec![vec![eq_array(rows)]]),
            "aligned" | "align" | "align*" | "split" | "gathered" | "gather" | "eqnarray" => eq_array(rows),
            other => return Err(format!("The environment \"{other}\" isn't supported. Use matrix, pmatrix, bmatrix, vmatrix, cases or aligned.")),
        };
        Ok(Item::El(el))
    }
}

// ---------------------------------------------------------------------------
// Building Word's structures
// ---------------------------------------------------------------------------

/// `<tag><ptag>props + ctrlPr</ptag> children...</tag>`.
fn structure(tag: &'static str, ptag: &'static str, props: Vec<Element>, children: Vec<Element>) -> Element {
    let mut e = Element::new(tag);
    e.children.push(Node::Element(pr(ptag, props)));
    for c in children {
        e.children.push(Node::Element(c));
    }
    e
}

fn func_el(fname: Element, body: Vec<Item>) -> Element {
    structure("m:func", "m:funcPr", vec![], vec![fname, arg("m:e", finish(body))])
}

fn delimited(beg: &str, end: &str, sep: Option<&str>, cols: Vec<Vec<Element>>) -> Element {
    let mut props = vec![val("m:begChr", beg)];
    if let Some(s) = sep {
        props.push(val("m:sepChr", s));
    }
    props.push(val("m:endChr", end));
    let mut e = structure("m:d", "m:dPr", props, vec![]);
    for c in cols {
        e.children.push(Node::Element(arg("m:e", c)));
    }
    e
}

fn matrix(rows: Vec<Vec<Vec<Element>>>) -> Element {
    let cols = rows.iter().map(|r| r.len()).max().unwrap_or(1).max(1);
    let mc = Element::new("m:mc").with_child(Element::new("m:mcPr").with_child(val("m:count", &cols.to_string())).with_child(val("m:mcJc", "center")));
    let mut m = Element::new("m:m");
    m.children.push(Node::Element(pr("m:mPr", vec![Element::new("m:mcs").with_child(mc)])));
    for mut r in rows {
        r.resize(cols, Vec::new());
        let mut mr = Element::new("m:mr");
        for cell in r {
            mr.children.push(Node::Element(arg("m:e", cell)));
        }
        m.children.push(Node::Element(mr));
    }
    m
}

/// Equation array: one `m:e` per row; the cells of a row are told apart by alignment points.
fn eq_array(rows: Vec<Vec<Vec<Element>>>) -> Element {
    let mut e = Element::new("m:eqArr");
    e.children.push(Node::Element(pr("m:eqArrPr", vec![])));
    for r in rows {
        let mut content: Vec<Element> = Vec::new();
        for (i, cell) in r.into_iter().enumerate() {
            if i > 0 {
                content.push(run_el(&RunSpec { text: String::new(), sty: Sty::Default, nor: false, aln: true }));
            }
            content.extend(cell);
        }
        e.children.push(Node::Element(arg("m:e", content)));
    }
    e
}

fn build_scripts(base: Item, sub: Option<Vec<Item>>, sup: Option<Vec<Item>>) -> Element {
    // Limits on a brace (`\underbrace{x}_{n}`) sit under or over it.
    if let Item::El(b) = &base {
        if b.name == "m:groupChr" {
            let pos = b.child("m:groupChrPr").and_then(|p| p.child("m:pos")).and_then(|p| p.attr("m:val")).unwrap_or("bot");
            if sup.is_none() && sub.is_some() && pos == "bot" {
                return structure("m:limLow", "m:limLowPr", vec![], vec![arg("m:e", finish(vec![base])), arg("m:lim", finish(sub.unwrap_or_default()))]);
            }
            if sub.is_none() && sup.is_some() && pos == "top" {
                return structure("m:limUpp", "m:limUppPr", vec![], vec![arg("m:e", finish(vec![base])), arg("m:lim", finish(sup.unwrap_or_default()))]);
            }
        }
    }
    let e = arg("m:e", finish(vec![base]));
    match (sub, sup) {
        (Some(b), Some(p)) => structure("m:sSubSup", "m:sSubSupPr", vec![], vec![e, arg("m:sub", finish(b)), arg("m:sup", finish(p))]),
        (None, Some(p)) => structure("m:sSup", "m:sSupPr", vec![], vec![e, arg("m:sup", finish(p))]),
        (Some(b), None) => structure("m:sSub", "m:sSubPr", vec![], vec![e, arg("m:sub", finish(b))]),
        (None, None) => unreachable!("scripts() returns early without any"),
    }
}

// ---------------------------------------------------------------------------
// Runs
// ---------------------------------------------------------------------------

fn restyle(items: Vec<Item>, how: &str) -> Vec<Item> {
    items
        .into_iter()
        .map(|it| match it {
            Item::Run(mut r) => {
                match how {
                    "mathbf" => {
                        if r.sty == Sty::Default || r.sty == Sty::Italic {
                            r.sty = Sty::Bold;
                        }
                    }
                    "boldsymbol" | "bm" => {
                        if r.sty == Sty::Default || r.sty == Sty::Italic {
                            r.sty = Sty::BoldItalic;
                        }
                    }
                    "mathit" => r.sty = Sty::Italic,
                    "mathbb" => {
                        r.text = r.text.chars().map(|c| sym::blackboard(c).unwrap_or(c)).collect();
                        r.sty = Sty::Plain;
                    }
                    "mathcal" | "mathscr" => {
                        r.text = r.text.chars().map(|c| sym::script(c).unwrap_or(c)).collect();
                        r.sty = Sty::Plain;
                    }
                    // mathrm, mathup, mathsf, mathtt
                    _ => r.sty = Sty::Plain,
                }
                Item::Run(r)
            }
            Item::Group(g) => Item::Group(restyle(g, how)),
            other => other,
        })
        .collect()
}

fn flatten(items: Vec<Item>, out: &mut Vec<Item>) {
    for it in items {
        match it {
            Item::Group(g) => flatten(g, out),
            other => out.push(other),
        }
    }
}

/// Turn parsed items into elements: groups flatten and neighbouring runs of the same style join, as in Word.
fn finish(items: Vec<Item>) -> Vec<Element> {
    let mut flat = Vec::new();
    flatten(items, &mut flat);
    let mut merged: Vec<Item> = Vec::new();
    for it in flat {
        if let (Item::Run(cur), Some(Item::Run(prev))) = (&it, merged.last_mut()) {
            if !cur.aln && !prev.aln && cur.sty == prev.sty && cur.nor == prev.nor {
                prev.text.push_str(&cur.text);
                continue;
            }
            if cur.aln || prev.aln {
                // An alignment marker stays a run of its own.
            }
        }
        merged.push(it);
    }
    merged
        .into_iter()
        .filter_map(|it| match it {
            Item::Run(r) => Some(run_el(&r)),
            Item::El(e) => Some(e),
            Item::Group(_) => None,
        })
        .collect()
}

fn run_el(r: &RunSpec) -> Element {
    let mut rpr = Element::new("m:rPr");
    if r.nor {
        rpr.children.push(Node::Element(Element::new("m:nor")));
    }
    let sty = match r.sty {
        Sty::Default => None,
        Sty::Plain => Some("p"),
        Sty::Bold => Some("b"),
        Sty::BoldItalic => Some("bi"),
        Sty::Italic => Some("i"),
    };
    if let Some(s) = sty {
        rpr.children.push(Node::Element(val("m:sty", s)));
    }
    if r.aln {
        rpr.children.push(Node::Element(Element::new("m:aln")));
    }
    let mut e = Element::new("m:r");
    if !rpr.children.is_empty() {
        e.children.push(Node::Element(rpr));
    }
    e.children.push(Node::Element(font_rpr()));
    let mut t = Element::new("m:t");
    if r.text.starts_with(' ') || r.text.ends_with(' ') || r.text.contains("  ") {
        t.set_attr("xml:space", "preserve");
    }
    if !r.text.is_empty() {
        t.children.push(Node::Text(r.text.clone()));
    }
    e.children.push(Node::Element(t));
    e
}
