//! Drawing EQ fields. Word's equation field writes an equation as switches and arguments: `EQ \f(1,2)` is a
//! fraction, `EQ \r(3,x)` a root, `EQ \s\up6(2)` a superscript. This turns the common ones into MathML Core, which
//! the browser draws: `\f` fraction, `\r` radical, `\s` superscript and subscript, `\b` brackets, `\o` overstrike,
//! `\a` array, `\x` box, `\l` list, `\i` integral, sum and product, and `\d` displacement.

use crate::fields::Instr;

#[derive(Debug, Clone, PartialEq)]
enum Node {
    Text(String),
    Switch(Switch),
}

#[derive(Debug, Clone, PartialEq)]
struct Switch {
    name: char,
    /// Option groups in order: each is the options in front of an argument list, and the arguments.
    groups: Vec<Group>,
}

#[derive(Debug, Clone, PartialEq, Default)]
struct Group {
    /// `up6` is ("up", "6"); `bc\{` is ("bc", "{").
    opts: Vec<(String, String)>,
    args: Vec<Vec<Node>>,
}

struct Parser<'a> {
    c: &'a [char],
    i: usize,
}

impl Parser<'_> {
    fn peek(&self) -> Option<char> {
        self.c.get(self.i).copied()
    }

    /// Items up to the end, or up to the `)` or `,` that ends an argument.
    fn seq(&mut self, in_arg: bool) -> Vec<Node> {
        let mut out = Vec::new();
        let mut text = String::new();
        let mut depth = 0usize;
        while let Some(ch) = self.peek() {
            match ch {
                '\\' => {
                    let next = self.c.get(self.i + 1).copied();
                    if next.is_some_and(|n| n.is_ascii_alphabetic()) {
                        if !text.is_empty() {
                            out.push(Node::Text(std::mem::take(&mut text)));
                        }
                        out.push(Node::Switch(self.switch()));
                    } else {
                        // An escaped character is just that character.
                        if let Some(n) = next {
                            text.push(n);
                        }
                        self.i += 2;
                    }
                }
                ')' if in_arg && depth == 0 => break,
                ',' if in_arg && depth == 0 => break,
                '(' => {
                    depth += 1;
                    text.push(ch);
                    self.i += 1;
                }
                ')' => {
                    depth = depth.saturating_sub(1);
                    text.push(ch);
                    self.i += 1;
                }
                _ => {
                    text.push(ch);
                    self.i += 1;
                }
            }
        }
        if !text.is_empty() {
            out.push(Node::Text(text));
        }
        out
    }

    fn switch(&mut self) -> Switch {
        self.i += 1; // the backslash
        let name = self.peek().unwrap_or('f').to_ascii_lowercase();
        self.i += 1;
        let mut groups = Vec::new();
        loop {
            let round_start = self.i;
            let mut g = Group::default();
            // Options: `\up6`, `\co2`, `\bc\{`.
            while self.peek() == Some('\\') && self.c.get(self.i + 1).is_some_and(|c| c.is_ascii_alphabetic()) {
                let save = self.i;
                self.i += 1;
                let mut word = String::new();
                while let Some(c) = self.peek().filter(char::is_ascii_alphabetic) {
                    word.push(c.to_ascii_lowercase());
                    self.i += 1;
                }
                // A group of options may belong to the next switch (`\s\up6(2)\do6(i)` is one switch; `\f(1,2)\f(3,4)` is two).
                if groups.is_empty() || matches!(word.as_str(), "up" | "do" | "ai" | "di") && name == 's' {
                    // fine: part of this switch
                } else {
                    self.i = save;
                    break;
                }
                let mut val = String::new();
                if matches!(word.as_str(), "bc" | "lc" | "rc") {
                    if self.peek() == Some('\\') {
                        if let Some(c) = self.c.get(self.i + 1) {
                            val.push(*c);
                            self.i += 2;
                        }
                    } else if let Some(c) = self.peek().filter(|c| *c != '(') {
                        val.push(c);
                        self.i += 1;
                    }
                } else {
                    while let Some(c) = self.peek().filter(|c| c.is_ascii_digit() || *c == '-') {
                        val.push(c);
                        self.i += 1;
                    }
                }
                g.opts.push((word, val));
            }
            if self.peek() == Some('(') {
                self.i += 1;
                loop {
                    let arg = self.seq(true);
                    g.args.push(arg);
                    match self.peek() {
                        Some(',') => self.i += 1,
                        Some(')') => {
                            self.i += 1;
                            break;
                        }
                        _ => break,
                    }
                }
            }
            let more = name == 's' && self.peek() == Some('\\') && {
                // The whole word must be an option, the rule the option reader uses.
                let rest: String = self.c[self.i + 1..].iter().take_while(|c| c.is_ascii_alphabetic()).collect::<String>().to_ascii_lowercase();
                matches!(rest.as_str(), "up" | "do" | "ai" | "di")
            };
            groups.push(g);
            // A round that consumed nothing can never make progress.
            if !more || self.i == round_start {
                break;
            }
        }
        Switch { name, groups }
    }
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

fn tokens(text: &str, out: &mut String) {
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c.is_whitespace() {
            continue;
        }
        if c.is_ascii_digit() || c == '.' && chars.peek().is_some_and(char::is_ascii_digit) {
            let mut n = String::from(c);
            while let Some(d) = chars.peek().copied().filter(|d| d.is_ascii_digit() || *d == '.') {
                n.push(d);
                chars.next();
            }
            out.push_str(&format!("<mn>{n}</mn>"));
        } else if c.is_alphabetic() {
            out.push_str(&format!("<mi>{}</mi>", esc(&c.to_string())));
        } else {
            out.push_str(&format!("<mo>{}</mo>", esc(&c.to_string())));
        }
    }
}

fn row(nodes: &[Node]) -> String {
    format!("<mrow>{}</mrow>", seq_ml(nodes))
}

fn seq_ml(nodes: &[Node]) -> String {
    let mut out = String::new();
    // Rendered pieces, so a script can take the piece before it as its base.
    let mut pieces: Vec<String> = Vec::new();
    for n in nodes {
        match n {
            Node::Text(t) => {
                let mut s = String::new();
                tokens(t, &mut s);
                // One piece per token.
                let mut rest = s.as_str();
                while let Some(end) = rest.find("</m").and_then(|i| rest[i..].find('>').map(|j| i + j + 1)) {
                    pieces.push(rest[..end].to_string());
                    rest = &rest[end..];
                }
            }
            Node::Switch(sw) if sw.name == 's' => {
                let base = pieces.pop().unwrap_or_else(|| "<mrow></mrow>".into());
                let mut sup = None;
                let mut sub = None;
                for g in &sw.groups {
                    let arg = g.args.first().map(|a| row(a));
                    match g.opts.first().map(|(w, _)| w.as_str()) {
                        Some("do") | Some("di") => sub = arg,
                        _ => sup = arg,
                    }
                }
                pieces.push(match (sup, sub) {
                    (Some(p), Some(b)) => format!("<msubsup>{base}{b}{p}</msubsup>"),
                    (Some(p), None) => format!("<msup>{base}{p}</msup>"),
                    (None, Some(b)) => format!("<msub>{base}{b}</msub>"),
                    (None, None) => base,
                });
            }
            Node::Switch(sw) => pieces.push(switch_ml(sw)),
        }
    }
    for p in pieces {
        out.push_str(&p);
    }
    out
}

fn mirror(c: &str) -> &'static str {
    match c {
        "(" => ")",
        ")" => "(",
        "[" => "]",
        "]" => "[",
        "{" => "}",
        "}" => "{",
        "<" => ">",
        ">" => "<",
        "|" => "|",
        _ => "",
    }
}

fn opt<'a>(g: &'a Group, name: &str) -> Option<&'a str> {
    g.opts.iter().find(|(w, _)| w == name).map(|(_, v)| v.as_str())
}

fn switch_ml(sw: &Switch) -> String {
    let g = &sw.groups[0];
    let arg = |k: usize| -> String { g.args.get(k).map(|a| row(a)).unwrap_or_else(|| "<mrow></mrow>".into()) };
    match sw.name {
        'f' => format!("<mfrac>{}{}</mfrac>", arg(0), arg(1)),
        'r' => {
            if g.args.len() >= 2 && !g.args[0].is_empty() {
                format!("<mroot>{}{}</mroot>", arg(1), arg(0))
            } else {
                format!("<msqrt>{}</msqrt>", if g.args.len() >= 2 { arg(1) } else { arg(0) })
            }
        }
        'b' => {
            let both = opt(g, "bc");
            let left = opt(g, "lc").or(both).unwrap_or("(").to_string();
            let right = opt(g, "rc").map(String::from).unwrap_or_else(|| match both {
                Some(b) => mirror(b).to_string(),
                None if opt(g, "lc").is_some() => String::new(),
                None => ")".to_string(),
            });
            let mut s = String::from("<mrow>");
            if !left.is_empty() {
                s.push_str(&format!("<mo stretchy=\"true\">{}</mo>", esc(&left)));
            }
            s.push_str(&arg(0));
            if !right.is_empty() {
                s.push_str(&format!("<mo stretchy=\"true\">{}</mo>", esc(&right)));
            }
            s.push_str("</mrow>");
            s
        }
        'o' => {
            // Overstrike: every character after the first sits on top of it, so they take no width.
            let items: Vec<String> = g.args.iter().map(|a| row(a)).collect();
            let mut s = String::from("<mrow>");
            let last = items.len().saturating_sub(1);
            for (k, it) in items.iter().enumerate() {
                if k == last {
                    s.push_str(it);
                } else {
                    s.push_str(&format!("<mpadded width=\"0\">{it}</mpadded>"));
                }
            }
            s.push_str("</mrow>");
            s
        }
        'a' => {
            let cols = opt(g, "co").and_then(|v| v.parse::<usize>().ok()).filter(|c| *c > 0).unwrap_or(1);
            let align = if opt(g, "ar").is_some() {
                "right"
            } else if opt(g, "ac").is_some() {
                "center"
            } else {
                "left"
            };
            let mut s = String::from("<mtable>");
            for chunk in g.args.chunks(cols) {
                s.push_str("<mtr>");
                for cell in chunk {
                    s.push_str(&format!("<mtd columnalign=\"{align}\">{}</mtd>", row(cell)));
                }
                s.push_str("</mtr>");
            }
            s.push_str("</mtable>");
            s
        }
        'x' => format!("<mrow class=\"m-box\">{}</mrow>", arg(0)),
        'l' => {
            let mut s = String::from("<mrow>");
            for (k, a) in g.args.iter().enumerate() {
                if k > 0 {
                    s.push_str("<mo>,</mo>");
                }
                s.push_str(&row(a));
            }
            s.push_str("</mrow>");
            s
        }
        'i' => {
            let sign = if opt(g, "su").is_some() {
                "\u{2211}"
            } else if opt(g, "pr").is_some() {
                "\u{220F}"
            } else {
                "\u{222B}"
            };
            let inline = opt(g, "in").is_some();
            let (lo, hi, body) = match g.args.len() {
                0 | 1 => (String::new(), String::new(), arg(0)),
                2 => (String::new(), arg(0), arg(1)),
                _ => (arg(0), arg(1), arg(2)),
            };
            let op = format!("<mo largeop=\"true\">{sign}</mo>");
            let scripts = if lo.is_empty() && hi.is_empty() {
                op
            } else if inline {
                format!("<msubsup>{op}{}{}</msubsup>", if lo.is_empty() { "<mrow></mrow>".to_string() } else { lo }, if hi.is_empty() { "<mrow></mrow>".to_string() } else { hi })
            } else {
                format!("<munderover>{op}{}{}</munderover>", if lo.is_empty() { "<mrow></mrow>".to_string() } else { lo }, if hi.is_empty() { "<mrow></mrow>".to_string() } else { hi })
            };
            format!("<mrow>{scripts}{body}</mrow>")
        }
        'd' => {
            let pts = |name: &str| opt(g, name).and_then(|v| v.parse::<f64>().ok());
            let width = pts("fo").unwrap_or(0.0) - pts("ba").unwrap_or(0.0);
            if width > 0.0 {
                format!("<mspace width=\"{width}pt\"/>")
            } else {
                String::new()
            }
        }
        _ => row(&g.args.first().cloned().unwrap_or_default()),
    }
}

/// The MathML for an `EQ` field instruction, or None when it isn't one or says nothing.
pub fn eq_to_mathml(instr: &str) -> Option<String> {
    let i = Instr::parse(instr);
    if i.code != "EQ" {
        return None;
    }
    let body = instr.trim_start().get(2..)?.trim();
    let chars: Vec<char> = body.chars().collect();
    let mut p = Parser { c: &chars, i: 0 };
    let nodes = p.seq(false);
    if nodes.is_empty() {
        return None;
    }
    Some(format!("<math xmlns=\"http://www.w3.org/1998/Math/MathML\" display=\"inline\"><mrow displaystyle=\"true\">{}</mrow></math>", seq_ml(&nodes)))
}

/// The text of an `EQ` field as plain characters, for a place that can't draw it (a text export).
pub fn eq_plain(instr: &str) -> String {
    let m = eq_to_mathml(instr).unwrap_or_default();
    let mut out = String::new();
    let mut in_tag = false;
    for c in m.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ml(instr: &str) -> String {
        eq_to_mathml(instr).unwrap_or_default()
    }

    #[test]
    fn crafted_option_words_finish() {
        // `\s` followed by a longer word that merely starts with an option name used to loop forever.
        for instr in [r" EQ x\s\up6(2)\dog ", r" EQ x\s\up6(2)\dial ", r" EQ \s\ai\upper "] {
            let (tx, rx) = std::sync::mpsc::channel();
            let owned = instr.to_string();
            std::thread::spawn(move || {
                let _ = tx.send(eq_to_mathml(&owned));
            });
            assert!(rx.recv_timeout(std::time::Duration::from_secs(5)).is_ok(), "hung on {instr}");
        }
    }

    #[test]
    fn fraction() {
        let m = ml(r" EQ \f(1,2) ");
        assert!(m.contains("<mfrac><mrow><mn>1</mn></mrow><mrow><mn>2</mn></mrow></mfrac>"), "{m}");
    }

    #[test]
    fn radicals() {
        assert!(ml(r" EQ \r(x) ").contains("<msqrt><mrow><mi>x</mi></mrow></msqrt>"));
        assert!(ml(r" EQ \r(3,x) ").contains("<mroot><mrow><mi>x</mi></mrow><mrow><mn>3</mn></mrow></mroot>"));
        assert!(ml(r" EQ \r(,x) ").contains("<msqrt>"));
    }

    #[test]
    fn scripts_take_the_piece_before() {
        let m = ml(r" EQ x\s\up6(2) ");
        assert!(m.contains("<msup><mi>x</mi><mrow><mn>2</mn></mrow></msup>"), "{m}");
        let m = ml(r" EQ x\s\do2(i) ");
        assert!(m.contains("<msub><mi>x</mi><mrow><mi>i</mi></mrow></msub>"), "{m}");
        let m = ml(r" EQ x\s\up6(2)\do2(i) ");
        assert!(m.contains("<msubsup>"), "{m}");
    }

    #[test]
    fn brackets() {
        assert!(ml(r" EQ \b(x) ").contains("<mo stretchy=\"true\">(</mo><mrow><mi>x</mi></mrow><mo stretchy=\"true\">)</mo>"));
        let m = ml(r" EQ \b\bc\{ (x) ");
        assert!(m.contains(">{</mo>") && m.contains(">}</mo>"), "{m}");
        let m = ml(r" EQ \b\lc\[ \rc\| (x) ");
        assert!(m.contains(">[</mo>") && m.contains(">|</mo>"), "{m}");
    }

    #[test]
    fn arrays_boxes_lists_integrals() {
        let m = ml(r" EQ \a\al\co2(a,b,c,d) ");
        assert_eq!(m.matches("<mtr>").count(), 2, "{m}");
        assert!(m.contains("columnalign=\"left\""));
        assert!(ml(r" EQ \x(abc) ").contains("m-box"));
        assert!(ml(r" EQ \l(1,2,3) ").contains("<mo>,</mo>"));
        let m = ml(r" EQ \i(0,1,x) ");
        assert!(m.contains("<munderover><mo largeop=\"true\">\u{222B}</mo>"), "{m}");
        assert!(ml(r" EQ \i\su(1,n,k) ").contains('\u{2211}'));
        assert!(ml(r" EQ \o(c,/) ").contains("<mpadded width=\"0\">"));
    }

    #[test]
    fn other_fields_are_not_equations() {
        assert!(eq_to_mathml(" PAGE ").is_none());
        assert!(eq_to_mathml(" EQ ").is_none());
        assert_eq!(eq_plain(r" EQ \f(1,2) "), "12");
    }
}
