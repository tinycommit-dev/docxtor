//! OMML to LaTeX, so an equation Word made can be edited in the equation box. Best effort: markup LaTeX cannot say
//! (a linear fraction, say) comes out as the nearest thing. For everything the reader in `latex` produces, writing
//! LaTeX and reading it again gives the same equation.

use super::symbols as sym;
use super::*;

pub fn omml_to_latex(el: &Element) -> String {
    let lines: Vec<&Element> = if el.name == "m:oMathPara" { el.children_named("m:oMath").collect() } else { vec![el] };
    let render = |e: &Element| {
        let mut w = Writer::default();
        w.children(e);
        w.out.trim().to_string()
    };
    if lines.len() > 1 {
        let rows: Vec<String> = lines.iter().map(|l| render(l)).collect();
        return format!("\\begin{{gathered}} {} \\end{{gathered}}", rows.join(" \\\\ "));
    }
    lines.first().map(|l| render(l)).unwrap_or_default()
}

#[derive(Default)]
struct Writer {
    out: String,
    /// Inside a function's name: upright words are function names.
    fname: bool,
}

/// Whether LaTeX reads `s` as one token (a character or a command), so it needs no braces.
fn is_token(s: &str) -> bool {
    let mut it = s.chars();
    match (it.next(), it.next()) {
        (Some(_), None) => !s.starts_with('\\') || s.len() == 1,
        (Some('\\'), Some(_)) => {
            let rest = &s[1..];
            rest.chars().all(|c| c.is_ascii_alphabetic()) || rest.chars().count() == 1
        }
        _ => false,
    }
}

/// Whether reading `s` as the body of a large operator would stop before its end: a `+`, `-`, `=`, `<`, `>`, `,`,
/// `;`, a comparison or a space command outside brackets (a sign at the very start belongs to the body).
fn breaks_operand_run(s: &str) -> bool {
    const STOPS: &[&str] = &["pm", "mp", "leq", "le", "geq", "ge", "neq", "ne", "approx", "equiv", "sim", "simeq", "cong", "propto", "to", "rightarrow", "Rightarrow", "Leftrightarrow", "iff", "implies", "in", "notin", "subset", "subseteq", "supset", "supseteq", "quad", "qquad"];
    let cs: Vec<char> = s.chars().collect();
    let mut depth = 0i32;
    let mut i = 0;
    while i < cs.len() {
        match cs[i] {
            '{' | '(' | '[' => depth += 1,
            '}' | ')' | ']' => depth -= 1,
            '\\' => {
                let mut j = i + 1;
                while j < cs.len() && cs[j].is_ascii_alphabetic() {
                    j += 1;
                }
                let name: String = cs[i + 1..j].iter().collect();
                match name.as_str() {
                    "left" | "begin" => depth += 1,
                    "right" | "end" => depth -= 1,
                    n if depth == 0 && STOPS.contains(&n) => return true,
                    _ => {}
                }
                // `\{`, `\,` and friends are one token.
                i = if j == i + 1 { i + 2 } else { j };
                continue;
            }
            '+' | '-' | '=' | '<' | '>' | ',' | ';' if depth == 0 && i > 0 => return true,
            _ => {}
        }
        i += 1;
    }
    false
}

fn braced(s: &str) -> String {
    format!("{{{s}}}")
}

fn script(s: &str) -> String {
    if is_token(s) {
        s.to_string()
    } else {
        braced(s)
    }
}

fn escape_text(s: &str) -> String {
    let mut o = String::new();
    for c in s.chars() {
        match c {
            '\\' | '{' | '}' | '%' | '$' | '&' | '#' | '_' | '^' | '~' => {
                o.push('\\');
                o.push(c);
            }
            c => o.push(c),
        }
    }
    o
}

impl Writer {
    /// Append, with a space when a command would run into a letter.
    fn push(&mut self, s: &str) {
        if s.is_empty() {
            return;
        }
        if s.starts_with(|c: char| c.is_ascii_alphabetic()) && self.ends_with_command() {
            self.out.push(' ');
        }
        self.out.push_str(s);
    }

    fn ends_with_command(&self) -> bool {
        let b = self.out.as_bytes();
        let mut i = b.len();
        while i > 0 && b[i - 1].is_ascii_alphabetic() {
            i -= 1;
        }
        i > 0 && i < b.len() && b[i - 1] == b'\\' && !(i >= 2 && b[i - 2] == b'\\')
    }

    fn children(&mut self, el: &Element) {
        for c in el.elements() {
            self.node(c);
        }
    }

    /// The text of a child's content.
    fn sub(&self, parent: &Element, name: &str) -> String {
        let mut w = Writer { out: String::new(), fname: self.fname && name == "m:fName" };
        if let Some(c) = parent.child(name) {
            w.children(c);
        }
        w.out
    }

    /// Content of an argument, with braces unless it is a single token.
    fn arg_braced(&self, parent: &Element, name: &str) -> String {
        braced(&self.sub(parent, name))
    }

    /// A base for scripts: braces unless it is one token.
    fn base(&self, parent: &Element) -> String {
        let s = self.sub(parent, "m:e");
        // One bracketed group, fraction, root or accent is a single atom to LaTeX: scripts apply to all of it.
        let one_atom = parent.child("m:e").is_some_and(|e| {
            let els: Vec<&Element> = e.elements().filter(|x| !x.name.ends_with("Pr")).collect();
            matches!(els.as_slice(), [x] if matches!(x.name.as_str(), "m:d" | "m:f" | "m:rad" | "m:acc"))
        });
        if is_token(&s) || one_atom {
            s
        } else {
            braced(&s)
        }
    }

    /// Whether an argument holds one element that LaTeX reads as one operand.
    fn single_operand(&self, parent: &Element, name: &str) -> bool {
        let Some(c) = parent.child(name) else { return true };
        let els: Vec<&Element> = c.elements().filter(|e| !e.name.ends_with("Pr")).collect();
        match els.as_slice() {
            [] => true,
            [e] if e.name == "m:r" => {
                let t: String = e.elements().filter(|x| x.name == "m:t" || x.name == "w:t").map(|t| t.text()).collect();
                let nor = e.child("m:rPr").is_some_and(|p| p.child("m:nor").is_some());
                !nor && t.chars().filter(|c| *c != ' ').count() <= 1
            }
            [_] => true,
            _ => false,
        }
    }

    fn operand(&self, parent: &Element, name: &str) -> String {
        let s = self.sub(parent, name);
        if self.single_operand(parent, name) {
            s
        } else {
            braced(&s)
        }
    }

    fn node(&mut self, c: &Element) {
        let name = c.name.as_str();
        if name.ends_with("Pr") || matches!(name, "w:bookmarkStart" | "w:bookmarkEnd" | "w:proofErr" | "w:del") {
            return;
        }
        match name {
            "m:r" => self.run(c),
            "m:f" => {
                // Word's fraction kinds, in LaTeX that `latex` reads back: a bar is \frac, a linear one (a/b) is
                // \nicefrac, a skewed one \sfrac and one without a bar \genfrac with a zero thickness.
                let head = match c.child("m:fPr").and_then(|p| p.child("m:type")).and_then(|t| t.attr("m:val")).unwrap_or("bar") {
                    "lin" => "\\nicefrac",
                    "skw" => "\\sfrac",
                    "noBar" => "\\genfrac{}{}{0pt}{}",
                    _ => "\\frac",
                };
                let s = format!("{head}{}{}", self.arg_braced(c, "m:num"), self.arg_braced(c, "m:den"));
                self.push(&s);
            }
            "m:sSub" => {
                let s = format!("{}_{}", self.base(c), script(&self.sub(c, "m:sub")));
                self.push(&s);
            }
            "m:sSup" => {
                let s = format!("{}^{}", self.base(c), script(&self.sub(c, "m:sup")));
                self.push(&s);
            }
            "m:sSubSup" => {
                let s = format!("{}_{}^{}", self.base(c), script(&self.sub(c, "m:sub")), script(&self.sub(c, "m:sup")));
                self.push(&s);
            }
            "m:sPre" => {
                let s = format!("\\prescript{}{}{}", self.arg_braced(c, "m:sup"), self.arg_braced(c, "m:sub"), self.arg_braced(c, "m:e"));
                self.push(&s);
            }
            "m:rad" => {
                let hide = c.child("m:radPr").and_then(|p| p.child("m:degHide")).map(flag).unwrap_or(false) || c.child("m:deg").is_none_or(|d| d.elements().next().is_none());
                let s = if hide { format!("\\sqrt{}", self.arg_braced(c, "m:e")) } else { format!("\\sqrt[{}]{}", self.sub(c, "m:deg"), self.arg_braced(c, "m:e")) };
                self.push(&s);
            }
            "m:nary" => self.nary(c),
            "m:d" => self.delimited(c),
            "m:m" => {
                let s = self.matrix(c, "matrix");
                self.push(&s);
            }
            "m:eqArr" => {
                let s = self.eq_array(c, "aligned");
                self.push(&s);
            }
            "m:func" => {
                let mut w = Writer { out: String::new(), fname: true };
                if let Some(f) = c.child("m:fName") {
                    w.children(f);
                }
                let name_tex = w.out;
                let body = self.operand(c, "m:e");
                self.push(&name_tex);
                if !body.is_empty() {
                    self.push(&body);
                }
            }
            "m:limLow" | "m:limUpp" => {
                let low = name == "m:limLow";
                let e = self.sub(c, "m:e");
                let lim = self.sub(c, "m:lim");
                if self.fname {
                    // A function's limit: \lim_{x \to 0}
                    let s = format!("{}{}{}", e, if low { "_" } else { "^" }, script(&lim));
                    self.push(&s);
                } else {
                    let base = self.base(c);
                    let is_brace = c.child("m:e").is_some_and(|x| x.elements().any(|g| g.name == "m:groupChr"));
                    let s = if is_brace { format!("{}{}{}", base, if low { "_" } else { "^" }, script(&lim)) } else { format!("\\{}{}{}", if low { "underset" } else { "overset" }, braced(&lim), braced(&e)) };
                    self.push(&s);
                }
            }
            "m:acc" => {
                let chr = c.child("m:accPr").and_then(|p| p.child("m:chr")).and_then(|x| x.attr("m:val")).unwrap_or("\u{302}");
                let cmd = sym::accent_name(chr).unwrap_or("hat");
                let s = format!("\\{cmd}{}", self.arg_braced(c, "m:e"));
                self.push(&s);
            }
            "m:bar" => {
                let top = c.child("m:barPr").and_then(|p| p.child("m:pos")).and_then(|x| x.attr("m:val")) == Some("top");
                let s = format!("\\{}{}", if top { "overline" } else { "underline" }, self.arg_braced(c, "m:e"));
                self.push(&s);
            }
            "m:groupChr" => {
                let pr = c.child("m:groupChrPr");
                let top = pr.and_then(|p| p.child("m:pos")).and_then(|x| x.attr("m:val")) == Some("top");
                let s = format!("\\{}{}", if top { "overbrace" } else { "underbrace" }, self.arg_braced(c, "m:e"));
                self.push(&s);
            }
            "m:box" => {
                let s = self.arg_braced(c, "m:e");
                self.push(&s);
            }
            "m:borderBox" => {
                let s = format!("\\boxed{}", self.arg_braced(c, "m:e"));
                self.push(&s);
            }
            "m:phant" => {
                let s = format!("\\phantom{}", self.arg_braced(c, "m:e"));
                self.push(&s);
            }
            "m:oMath" | "w:ins" | "m:e" | "m:num" | "m:den" | "m:sub" | "m:sup" | "m:lim" | "m:fName" | "m:deg" => self.children(c),
            "w:r" => {
                let t: String = c.children_named("w:t").map(|t| t.text()).collect();
                if !t.is_empty() {
                    let s = format!("\\text{{{}}}", escape_text(&t));
                    self.push(&s);
                }
            }
            _ => {
                if c.elements().next().is_some() {
                    self.children(c);
                } else {
                    let t = c.text();
                    if !t.trim().is_empty() {
                        let s = format!("\\text{{{}}}", escape_text(&t));
                        self.push(&s);
                    }
                }
            }
        }
    }

    fn nary(&mut self, c: &Element) {
        let pr = c.child("m:naryPr");
        let chr = pr.and_then(|p| p.child("m:chr")).and_then(|x| x.attr("m:val")).unwrap_or("∫");
        let cmd = sym::nary_name(chr).unwrap_or("int");
        let default_under = sym::nary_default_under_over(chr);
        let under = match pr.and_then(|p| p.child("m:limLoc")).and_then(|x| x.attr("m:val")) {
            Some(v) => v == "undOvr",
            None => default_under,
        };
        let hidden = |n: &str| pr.and_then(|p| p.child(n)).map(flag).unwrap_or(false);
        let mut s = format!("\\{cmd}");
        if under != default_under {
            s.push_str(if under { "\\limits" } else { "\\nolimits" });
        }
        let sub = self.sub(c, "m:sub");
        let sup = self.sub(c, "m:sup");
        if !hidden("m:subHide") && !sub.is_empty() {
            s.push('_');
            s.push_str(&braced(&sub));
        }
        if !hidden("m:supHide") && !sup.is_empty() {
            s.push('^');
            s.push_str(&braced(&sup));
        }
        self.push(&s);
        // The body runs to the next + - = or comparison, so it needs braces only when it holds one.
        let body = self.sub(c, "m:e");
        if !body.is_empty() {
            self.out.push(' ');
            if breaks_operand_run(&body) {
                self.out.push_str(&braced(&body));
            } else {
                self.out.push_str(&body);
            }
        }
    }

    fn matrix(&self, m: &Element, env: &str) -> String {
        let rows: Vec<String> = m
            .children_named("m:mr")
            .map(|r| {
                r.children_named("m:e")
                    .map(|e| {
                        let mut w = Writer::default();
                        w.children(e);
                        w.out
                    })
                    .collect::<Vec<_>>()
                    .join(" & ")
            })
            .collect();
        format!("\\begin{{{env}}} {} \\end{{{env}}}", rows.join(" \\\\ "))
    }

    fn eq_array(&self, a: &Element, env: &str) -> String {
        let rows: Vec<String> = a
            .children_named("m:e")
            .map(|e| {
                let mut cells: Vec<Writer> = vec![Writer::default()];
                for x in e.elements() {
                    if x.name == "m:r" && x.child("m:rPr").is_some_and(|p| p.child("m:aln").is_some()) {
                        cells.push(Writer::default());
                        // The marker run may carry text of its own.
                    }
                    cells.last_mut().expect("a cell").node(x);
                }
                cells.into_iter().map(|w| w.out.trim().to_string()).collect::<Vec<_>>().join(" & ")
            })
            .collect();
        format!("\\begin{{{env}}} {} \\end{{{env}}}", rows.join(" \\\\ "))
    }

    fn delimited(&mut self, c: &Element) {
        let pr = c.child("m:dPr");
        let get = |n: &str, d: &'static str| -> String { pr.and_then(|p| p.child(n)).map(|x| x.attr("m:val").unwrap_or("").to_string()).unwrap_or_else(|| d.to_string()) };
        let (beg, sep, end) = (get("m:begChr", "("), get("m:sepChr", "|"), get("m:endChr", ")"));
        let es: Vec<&Element> = c.children_named("m:e").collect();
        // Shapes with an environment of their own.
        if let [e] = es.as_slice() {
            let els: Vec<&Element> = e.elements().filter(|x| !x.name.ends_with("Pr")).collect();
            if let [only] = els.as_slice() {
                let env = match (only.name.as_str(), beg.as_str(), end.as_str()) {
                    ("m:m", "(", ")") => Some(("pmatrix", true)),
                    ("m:m", "[", "]") => Some(("bmatrix", true)),
                    ("m:m", "{", "}") => Some(("Bmatrix", true)),
                    ("m:m", "|", "|") => Some(("vmatrix", true)),
                    ("m:m", "‖", "‖") => Some(("Vmatrix", true)),
                    ("m:eqArr", "{", "") => Some(("cases", false)),
                    _ => None,
                };
                if let Some((env, is_matrix)) = env {
                    let s = if is_matrix { self.matrix(only, env) } else { self.eq_array(only, env) };
                    self.push(&s);
                    return;
                }
                if only.name == "m:f" && beg == "(" && end == ")" && only.child("m:fPr").and_then(|p| p.child("m:type")).and_then(|t| t.attr("m:val")) == Some("noBar") {
                    let s = format!("\\binom{}{}", self.arg_braced(only, "m:num"), self.arg_braced(only, "m:den"));
                    self.push(&s);
                    return;
                }
            }
        }
        let mut s = format!("\\left{} ", sym_delim(&beg));
        for (i, e) in es.iter().enumerate() {
            if i > 0 {
                s.push_str(&format!(" \\middle{} ", sym_delim(&sep)));
            }
            let mut w = Writer::default();
            w.children(e);
            s.push_str(&w.out);
        }
        s.push_str(&format!(" \\right{}", sym_delim(&end)));
        self.push(&s);
    }

    fn run(&mut self, r: &Element) {
        let rpr = r.child("m:rPr");
        let nor = rpr.and_then(|p| p.child("m:nor")).map(flag).unwrap_or(false);
        let sty = rpr.and_then(|p| p.child("m:sty")).and_then(|s| s.attr("m:val")).unwrap_or("");
        let aln = rpr.is_some_and(|p| p.child("m:aln").is_some());
        let mut text = String::new();
        for c in r.elements() {
            match c.name.as_str() {
                "m:t" | "w:t" => text.push_str(&c.text()),
                "w:tab" => text.push('\u{2003}'),
                _ => {}
            }
        }
        if aln && text.is_empty() {
            return;
        }
        if text.is_empty() {
            return;
        }
        if nor {
            let s = format!("\\text{{{}}}", escape_text(&text));
            self.push(&s);
            return;
        }
        let chars: Vec<char> = text.chars().filter(|c| !matches!(c, ' ' | '\t' | '\n' | '\r')).collect();
        let word_like = !chars.is_empty() && chars.iter().all(|c| c.is_ascii_alphabetic());
        if (sty == "p" || self.fname) && word_like {
            let word: String = chars.iter().collect();
            let s = if sym::function(&word).is_some() {
                format!("\\{word}")
            } else if self.fname {
                format!("\\operatorname{{{word}}}")
            } else {
                format!("\\mathrm{{{word}}}")
            };
            self.push(&s);
            return;
        }
        let mapped = chars.iter().map(|&c| char_tex(c)).collect::<Vec<_>>();
        match sty {
            "b" => {
                let s = format!("\\mathbf{{{}}}", mapped.concat());
                self.push(&s);
            }
            "bi" => {
                let s = format!("\\boldsymbol{{{}}}", mapped.concat());
                self.push(&s);
            }
            _ => {
                for m in mapped {
                    self.push(&m);
                }
            }
        }
    }
}

fn sym_delim(d: &str) -> String {
    sym::delimiter_name(d)
}

/// One character of a math run as LaTeX.
fn char_tex(c: char) -> String {
    match c {
        '−' => "-".into(),
        '{' => "\\{".into(),
        '}' => "\\}".into(),
        '\\' => "\\setminus ".into(),
        '%' | '$' | '&' | '#' | '_' | '^' | '~' => format!("\\{c}"),
        '\u{2009}' => "\\,".into(),
        '\u{2005}' => "\\;".into(),
        '\u{2003}' => "\\quad ".into(),
        '\u{00A0}' => "~".into(),
        '\'' => "'".into(),
        c => {
            if let Some((cmd, l)) = sym::unstyle(c) {
                return format!("\\{cmd}{{{l}}}");
            }
            match sym::symbol_name(&c.to_string()) {
                Some(n) if !c.is_ascii() => format!("\\{n}"),
                _ => c.to_string(),
            }
        }
    }
}
