//! OMML to MathML Core.
//!
//! The output is one `<math>` element that Chrome (and any browser with MathML Core) draws itself. Only elements and
//! attributes of MathML Core are used, plus `class` on a box and `columnalign` on table cells (the editor's style
//! sheet reads those).

use super::symbols as sym;
use super::*;

pub fn omml_to_mathml(el: &Element) -> String {
    let display = el.name == "m:oMathPara";
    let mut w = Writer { out: String::new(), display, upright: false, cases: false };
    w.out.push_str(&format!("<math xmlns=\"http://www.w3.org/1998/Math/MathML\" display=\"{}\">", if display { "block" } else { "inline" }));
    let lines: Vec<&Element> = if display { el.children_named("m:oMath").collect() } else { vec![el] };
    if lines.len() > 1 {
        w.out.push_str("<mtable>");
        for l in lines {
            w.out.push_str("<mtr><mtd><mrow>");
            w.children(l);
            w.out.push_str("</mrow></mtd></mtr>");
        }
        w.out.push_str("</mtable>");
    } else if let Some(l) = lines.first() {
        w.out.push_str("<mrow>");
        w.children(l);
        w.out.push_str("</mrow>");
    }
    w.out.push_str("</math>");
    w.out
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

struct Writer {
    out: String,
    display: bool,
    /// Inside a function's name: letters are upright.
    upright: bool,
    /// The next equation array sits in a `{` delimiter (cases): its cells align left.
    cases: bool,
}

fn space_width(c: char) -> Option<&'static str> {
    Some(match c {
        '\u{2009}' => "0.1667em",
        '\u{2005}' => "0.2222em",
        '\u{2003}' => "1em",
        '\u{2002}' => "0.5em",
        '\u{2004}' => "0.3333em",
        '\u{2006}' => "0.1667em",
        '\u{200A}' => "0.1em",
        '\u{00A0}' => "0.25em",
        _ => return None,
    })
}

fn is_upper_greek(c: char) -> bool {
    ('\u{0391}'..='\u{03A9}').contains(&c)
}

impl Writer {
    fn children(&mut self, el: &Element) {
        for c in el.elements() {
            self.node(c);
        }
    }

    /// The content of the child called `name`, wrapped as one group (empty when the child is missing).
    fn arg(&mut self, parent: &Element, name: &str) {
        self.out.push_str("<mrow>");
        if let Some(c) = parent.child(name) {
            self.children(c);
        }
        self.out.push_str("</mrow>");
    }

    fn mo(&mut self, attrs: &str, text: &str) {
        self.out.push_str(&format!("<mo{attrs}>{}</mo>", esc(text)));
    }

    fn val<'a>(el: &'a Element, pr: &str, child: &str) -> Option<&'a str> {
        el.child(pr).and_then(|p| p.child(child)).and_then(|c| c.attr("m:val"))
    }

    fn on(el: &Element, pr: &str, child: &str) -> bool {
        el.child(pr).and_then(|p| p.child(child)).map(flag).unwrap_or(false)
    }

    fn node(&mut self, c: &Element) {
        let name = c.name.as_str();
        if name.ends_with("Pr") || matches!(name, "w:bookmarkStart" | "w:bookmarkEnd" | "w:proofErr" | "w:del" | "m:ctrlPr") {
            return;
        }
        match name {
            "m:r" => self.run(c),
            "m:f" => {
                match Self::val(c, "m:fPr", "m:type").unwrap_or("bar") {
                    "lin" | "skw" => {
                        self.out.push_str("<mrow>");
                        self.arg(c, "m:num");
                        self.mo("", "/");
                        self.arg(c, "m:den");
                        self.out.push_str("</mrow>");
                    }
                    "noBar" => {
                        self.out.push_str("<mfrac linethickness=\"0\">");
                        self.arg(c, "m:num");
                        self.arg(c, "m:den");
                        self.out.push_str("</mfrac>");
                    }
                    _ => {
                        self.out.push_str("<mfrac>");
                        self.arg(c, "m:num");
                        self.arg(c, "m:den");
                        self.out.push_str("</mfrac>");
                    }
                };
            }
            "m:sSub" => {
                self.out.push_str("<msub>");
                self.arg(c, "m:e");
                self.arg(c, "m:sub");
                self.out.push_str("</msub>");
            }
            "m:sSup" => {
                self.out.push_str("<msup>");
                self.arg(c, "m:e");
                self.arg(c, "m:sup");
                self.out.push_str("</msup>");
            }
            "m:sSubSup" => {
                self.out.push_str("<msubsup>");
                self.arg(c, "m:e");
                self.arg(c, "m:sub");
                self.arg(c, "m:sup");
                self.out.push_str("</msubsup>");
            }
            "m:sPre" => {
                self.out.push_str("<mmultiscripts>");
                self.arg(c, "m:e");
                self.out.push_str("<mprescripts/>");
                self.arg(c, "m:sub");
                self.arg(c, "m:sup");
                self.out.push_str("</mmultiscripts>");
            }
            "m:rad" => {
                let hide = Self::on(c, "m:radPr", "m:degHide") || c.child("m:deg").is_none_or(|d| d.elements().next().is_none());
                if hide {
                    self.out.push_str("<msqrt>");
                    self.arg(c, "m:e");
                    self.out.push_str("</msqrt>");
                } else {
                    self.out.push_str("<mroot>");
                    self.arg(c, "m:e");
                    self.arg(c, "m:deg");
                    self.out.push_str("</mroot>");
                }
            }
            "m:nary" => self.nary(c),
            "m:d" => self.delimited(c),
            "m:m" => {
                self.out.push_str("<mtable>");
                for r in c.children_named("m:mr") {
                    self.out.push_str("<mtr>");
                    for e in r.children_named("m:e") {
                        self.out.push_str("<mtd><mrow>");
                        self.children(e);
                        self.out.push_str("</mrow></mtd>");
                    }
                    self.out.push_str("</mtr>");
                }
                self.out.push_str("</mtable>");
            }
            "m:eqArr" => self.eq_array(c),
            "m:func" => {
                self.out.push_str("<mrow>");
                let was = std::mem::replace(&mut self.upright, true);
                self.arg(c, "m:fName");
                self.upright = was;
                self.mo("", "\u{2061}");
                self.arg(c, "m:e");
                self.out.push_str("</mrow>");
            }
            "m:limLow" => {
                self.out.push_str("<munder>");
                self.arg(c, "m:e");
                self.arg(c, "m:lim");
                self.out.push_str("</munder>");
            }
            "m:limUpp" => {
                self.out.push_str("<mover>");
                self.arg(c, "m:e");
                self.arg(c, "m:lim");
                self.out.push_str("</mover>");
            }
            "m:acc" => {
                let chr = Self::val(c, "m:accPr", "m:chr").unwrap_or("\u{302}");
                self.out.push_str("<mover accent=\"true\">");
                self.arg(c, "m:e");
                self.mo("", sym::spacing_accent(chr));
                self.out.push_str("</mover>");
            }
            "m:bar" => {
                if Self::val(c, "m:barPr", "m:pos") == Some("top") {
                    self.out.push_str("<mover accent=\"true\">");
                    self.arg(c, "m:e");
                    self.mo(" stretchy=\"true\"", "\u{AF}");
                    self.out.push_str("</mover>");
                } else {
                    self.out.push_str("<munder accentunder=\"true\">");
                    self.arg(c, "m:e");
                    self.mo(" stretchy=\"true\"", "_");
                    self.out.push_str("</munder>");
                }
            }
            "m:groupChr" => {
                let top = Self::val(c, "m:groupChrPr", "m:pos") == Some("top");
                let chr = Self::val(c, "m:groupChrPr", "m:chr").unwrap_or(if top { "\u{23DE}" } else { "\u{23DF}" });
                let tag = if top { "mover" } else { "munder" };
                self.out.push_str(&format!("<{tag}>"));
                self.arg(c, "m:e");
                self.mo(" stretchy=\"true\"", chr);
                self.out.push_str(&format!("</{tag}>"));
            }
            "m:box" => self.arg(c, "m:e"),
            "m:borderBox" => {
                self.out.push_str("<mrow class=\"m-box\">");
                self.arg(c, "m:e");
                self.out.push_str("</mrow>");
            }
            "m:phant" => {
                if Self::val(c, "m:phantPr", "m:show").is_some_and(|v| matches!(v, "0" | "false" | "off")) {
                    self.out.push_str("<mphantom>");
                    self.arg(c, "m:e");
                    self.out.push_str("</mphantom>");
                } else {
                    self.arg(c, "m:e");
                }
            }
            "m:oMath" | "w:ins" | "m:e" | "m:num" | "m:den" | "m:sub" | "m:sup" | "m:lim" | "m:fName" | "m:deg" => {
                self.out.push_str("<mrow>");
                self.children(c);
                self.out.push_str("</mrow>");
            }
            "w:r" => {
                let t: String = c.children_named("w:t").map(|t| t.text()).collect();
                if !t.is_empty() {
                    self.out.push_str(&format!("<mtext>{}</mtext>", esc(&t.replace(' ', "\u{A0}"))));
                }
            }
            // Anything else: its structure if it has any, otherwise its text, so nothing disappears.
            _ => {
                if c.elements().next().is_some() {
                    self.out.push_str("<mrow>");
                    self.children(c);
                    self.out.push_str("</mrow>");
                } else {
                    let t = c.text();
                    if !t.trim().is_empty() {
                        self.out.push_str(&format!("<mtext>{}</mtext>", esc(&t.replace(' ', "\u{A0}"))));
                    }
                }
            }
        }
    }

    fn nary(&mut self, c: &Element) {
        let pr = c.child("m:naryPr");
        let chr = pr.and_then(|p| p.child("m:chr")).and_then(|x| x.attr("m:val")).unwrap_or("∫");
        let under_over = match pr.and_then(|p| p.child("m:limLoc")).and_then(|x| x.attr("m:val")) {
            Some(v) => v == "undOvr",
            None => sym::nary_default_under_over(chr),
        };
        let has_sub = !Self::on(c, "m:naryPr", "m:subHide");
        let has_sup = !Self::on(c, "m:naryPr", "m:supHide");
        self.out.push_str("<mrow>");
        // Word sets the limits of a sum beside it in a line of text, to keep the line height.
        let under = under_over && self.display;
        let op = format!("<mo largeop=\"true\" stretchy=\"false\">{}</mo>", esc(chr));
        let (open, close) = match (has_sub, has_sup, under) {
            (false, false, _) => ("", ""),
            (true, false, true) => ("<munder>", "</munder>"),
            (false, true, true) => ("<mover>", "</mover>"),
            (true, true, true) => ("<munderover>", "</munderover>"),
            (true, false, false) => ("<msub>", "</msub>"),
            (false, true, false) => ("<msup>", "</msup>"),
            (true, true, false) => ("<msubsup>", "</msubsup>"),
        };
        self.out.push_str(open);
        self.out.push_str(&op);
        if has_sub {
            self.arg(c, "m:sub");
        }
        if has_sup {
            self.arg(c, "m:sup");
        }
        self.out.push_str(close);
        self.arg(c, "m:e");
        self.out.push_str("</mrow>");
    }

    fn delimited(&mut self, c: &Element) {
        let pr = c.child("m:dPr");
        let get = |n: &str, d: &'static str| -> String { pr.and_then(|p| p.child(n)).map(|x| x.attr("m:val").unwrap_or("").to_string()).unwrap_or_else(|| d.to_string()) };
        let (beg, sep, end) = (get("m:begChr", "("), get("m:sepChr", "|"), get("m:endChr", ")"));
        self.out.push_str("<mrow>");
        if !beg.is_empty() {
            self.mo(" fence=\"true\" stretchy=\"true\" form=\"prefix\"", &beg);
        }
        // `{` with nothing on the right around equations on several lines: the cases layout.
        let es: Vec<&Element> = c.children_named("m:e").collect();
        for (i, e) in es.iter().enumerate() {
            if i > 0 && !sep.is_empty() {
                self.mo(" separator=\"true\" stretchy=\"true\"", &sep);
            }
            self.cases = beg == "{" && end.is_empty() && e.elements().count() == 1 && e.child("m:eqArr").is_some();
            self.out.push_str("<mrow>");
            self.children(e);
            self.out.push_str("</mrow>");
            self.cases = false;
        }
        if !end.is_empty() {
            self.mo(" fence=\"true\" stretchy=\"true\" form=\"postfix\"", &end);
        }
        self.out.push_str("</mrow>");
    }

    /// Rows are cut into cells at alignment points (`m:aln` on a run).
    fn eq_array(&mut self, c: &Element) {
        let cases = std::mem::take(&mut self.cases);
        let mut rows: Vec<Vec<Vec<&Element>>> = Vec::new();
        for e in c.children_named("m:e") {
            let mut cells: Vec<Vec<&Element>> = vec![Vec::new()];
            for x in e.elements() {
                let aligns = x.name == "m:r" && x.child("m:rPr").is_some_and(|p| p.child("m:aln").is_some());
                if aligns {
                    cells.push(Vec::new());
                }
                cells.last_mut().expect("one cell at least").push(x);
            }
            rows.push(cells);
        }
        // Right-aligned first column when the second starts with a relation (a = b), as in aligned equations.
        let first_right = !cases && rows.iter().any(|r| r.len() > 1);
        self.out.push_str("<mtable>");
        for r in rows {
            self.out.push_str("<mtr>");
            for (i, cell) in r.iter().enumerate() {
                let align = if first_right && i % 2 == 0 { "right" } else { "left" };
                self.out.push_str(&format!("<mtd columnalign=\"{align}\"><mrow>"));
                for x in cell {
                    self.node(x);
                }
                self.out.push_str("</mrow></mtd>");
            }
            self.out.push_str("</mtr>");
        }
        self.out.push_str("</mtable>");
    }

    fn run(&mut self, r: &Element) {
        let rpr = r.child("m:rPr");
        let nor = rpr.and_then(|p| p.child("m:nor")).map(flag).unwrap_or(false);
        let sty = rpr.and_then(|p| p.child("m:sty")).and_then(|s| s.attr("m:val")).unwrap_or("");
        let mut text = String::new();
        for c in r.elements() {
            match c.name.as_str() {
                "m:t" | "w:t" => text.push_str(&c.text()),
                "w:tab" => text.push('\u{2003}'),
                _ => {}
            }
        }
        if text.is_empty() {
            return;
        }
        if nor {
            self.out.push_str(&format!("<mtext>{}</mtext>", esc(&text.replace(' ', "\u{A0}"))));
            return;
        }
        let plain = sty == "p" || self.upright;
        let chars: Vec<char> = text.chars().collect();
        let style = |c: char| -> char {
            match sty {
                "b" => sym::bold(c),
                "bi" => sym::bold_italic(c),
                _ => c,
            }
        };
        let mut i = 0;
        while i < chars.len() {
            let c = chars[i];
            if c == ' ' || c == '\t' || c == '\n' || c == '\r' {
                i += 1;
            } else if let Some(w) = space_width(c) {
                self.out.push_str(&format!("<mspace width=\"{w}\"/>"));
                i += 1;
            } else if c.is_ascii_digit() {
                let mut j = i;
                while j < chars.len() && (chars[j].is_ascii_digit() || (chars[j] == '.' && chars.get(j + 1).is_some_and(|d| d.is_ascii_digit()) && j > i)) {
                    j += 1;
                }
                let s: String = chars[i..j].iter().map(|&d| if sty == "b" || sty == "bi" { sym::bold(d) } else { d }).collect();
                self.out.push_str(&format!("<mn>{}</mn>", esc(&s)));
                i = j;
            } else if c.is_alphabetic() {
                if plain && c.is_ascii_alphabetic() {
                    // Upright words (sin, lim, d in dx) read as one identifier.
                    let mut j = i;
                    while j < chars.len() && chars[j].is_ascii_alphabetic() {
                        j += 1;
                    }
                    let s: String = chars[i..j].iter().map(|&d| style(d)).collect();
                    self.out.push_str(&format!("<mi mathvariant=\"normal\">{}</mi>", esc(&s)));
                    i = j;
                } else {
                    let upright = plain || is_upper_greek(c) || sty == "b" || !(c.is_ascii_alphabetic() || ('\u{3B1}'..='\u{3C9}').contains(&c));
                    let attr = if upright { " mathvariant=\"normal\"" } else { "" };
                    self.out.push_str(&format!("<mi{attr}>{}</mi>", esc(&style(c).to_string())));
                    i += 1;
                }
            } else if matches!(c, '∞' | '∂' | '∇' | '∅' | 'ℵ') {
                self.out.push_str(&format!("<mi mathvariant=\"normal\">{c}</mi>"));
                i += 1;
            } else if "()[]{}|‖⟨⟩⌊⌋⌈⌉".contains(c) {
                // A bracket typed in a line of math stays its own size; only `m:d` brackets grow with their contents.
                self.mo(" stretchy=\"false\"", &c.to_string());
                i += 1;
            } else {
                self.mo("", &c.to_string());
                i += 1;
            }
        }
    }
}
