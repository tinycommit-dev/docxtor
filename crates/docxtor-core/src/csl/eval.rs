//! Evaluating a style's rendering elements for one item: text, numbers, dates, names, labels, groups and
//! conditions. The result is formatted text (`Rich`).

use super::locale::{self, Locale};
use super::model::*;
use super::{names, text, vars};
use crate::cite_styles::Rich;
use crate::citations::{Name, Source};

/// Where a citation of an item stands among the citations of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Position {
    First,
    Subsequent,
    Ibid,
    IbidWithLocator,
}

/// The item being formatted and what the whole document says about it.
#[derive(Clone)]
pub struct Item<'a> {
    pub src: &'a Source,
    pub citation_number: Option<usize>,
    pub year_suffix: Option<String>,
    pub first_note: Option<usize>,
    pub disambiguate: bool,
}

/// What a citation says about its item.
#[derive(Debug, Clone)]
pub struct CiteCtx {
    pub position: Position,
    pub locator: Option<String>,
    pub suppress_author: bool,
    pub suppress_year: bool,
}

impl Default for CiteCtx {
    fn default() -> Self {
        CiteCtx { position: Position::First, locator: None, suppress_author: false, suppress_year: false }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Citation,
    Bibliography,
}

/// Apply the formatting of an element to its output. Nothing is added to empty output.
pub fn decorate(mut r: Rich, f: &Fmt, loc: &Locale) -> Rich {
    if r.is_empty() {
        return r;
    }
    if f.strip_periods {
        text::strip_periods(&mut r);
    }
    if let Some(c) = f.text_case {
        text::apply_case(&mut r, c);
    }
    if f.quotes {
        let mut q = Rich::new();
        q.t(&loc.term("open-quote", Form::Long, false).unwrap_or_else(|| "\u{201c}".into()));
        q.append(&r);
        q.t(&loc.term("close-quote", Form::Long, false).unwrap_or_else(|| "\u{201d}".into()));
        r = q;
    }
    let sup = f.vertical.map(|v| v == Vertical::Sup);
    if f.italic.is_some() || f.bold.is_some() || f.small_caps.is_some() || sup.is_some() {
        text::set_look(&mut r, f.italic, f.bold, f.small_caps, sup);
    }
    if f.prefix.is_empty() && f.suffix.is_empty() {
        return r;
    }
    let mut out = Rich::new();
    out.t(&f.prefix);
    out.append(&r);
    text::push_chunk(&mut out, &f.suffix);
    out
}

pub struct Eval<'a> {
    pub style: &'a Style,
    pub loc: &'a Locale<'a>,
    pub mode: Mode,
    pub item: &'a Item<'a>,
    pub cite: CiteCtx,
    /// Variables called and variables that had something, for the groups being evaluated.
    called: usize,
    populated: usize,
    suppressed: Vec<String>,
    depth: usize,
}

impl<'a> Eval<'a> {
    pub fn new(style: &'a Style, loc: &'a Locale<'a>, mode: Mode, item: &'a Item<'a>, cite: CiteCtx) -> Eval<'a> {
        Eval { style, loc, mode, item, cite, called: 0, populated: 0, suppressed: Vec::new(), depth: 0 }
    }

    /// The cascading name options for this part of the style.
    fn base_opts(&self) -> NameOpts {
        let part = match self.mode {
            Mode::Citation => &self.style.citation.opts,
            Mode::Bibliography => self.style.bibliography.as_ref().map(|b| &b.opts).unwrap_or(&self.style.citation.opts),
        };
        self.style.opts.merged(part)
    }

    fn range_delimiter(&self) -> String {
        self.loc.term("page-range-delimiter", Form::Long, false).unwrap_or_else(|| "\u{2013}".into())
    }

    /// Evaluate a list of elements one after another.
    pub fn run(&mut self, nodes: &[Node]) -> Rich {
        let mut out = Rich::new();
        for n in nodes {
            let r = self.node(n);
            out.append(&r);
        }
        out
    }

    /// Each element's output on its own (empty ones left out), for layouts that treat the first one specially.
    pub fn run_each(&mut self, nodes: &[Node]) -> Vec<Rich> {
        let mut out = Vec::new();
        for n in nodes {
            self.pieces(n, &mut out);
        }
        out.retain(|r| !r.is_empty());
        out
    }

    pub fn node(&mut self, n: &Node) -> Rich {
        match n {
            Node::Text(t) => self.text(t),
            Node::Number(n) => self.number(n),
            Node::Label(l) => self.label(l),
            Node::Date(d) => self.date(d),
            Node::Names(n) => self.names(n),
            Node::Group(g) => self.group(g),
            Node::Choose(c) => self.choose(c),
        }
    }

    // ----- variables ----------------------------------------------------------

    fn has_var(&self, name: &str) -> bool {
        match name {
            "locator" => self.cite.locator.is_some(),
            "issued" => !self.cite.suppress_year && vars::date(self.item.src, name).is_some(),
            "accessed" => vars::date(self.item.src, name).is_some(),
            "citation-number" => self.item.citation_number.is_some(),
            "year-suffix" => self.item.year_suffix.is_some(),
            "first-reference-note-number" => self.item.first_note.is_some(),
            "author" | "editor" => !vars::names(self.item.src, name).is_empty() && (name != "author" || !self.cite.suppress_author),
            _ => vars::text(self.item.src, name).is_some(),
        }
    }

    /// The value of a text variable, in a form.
    fn var_value(&self, name: &str, form: Form) -> Option<Rich> {
        let delim = self.range_delimiter();
        let pr = self.style.page_range_format.as_deref();
        let v = match name {
            "locator" => self.cite.locator.as_ref().map(|l| text::page_range(l, pr, &delim)),
            "page" => vars::text(self.item.src, "page").map(|p| text::page_range(&p, pr, &delim)),
            "citation-number" => self.item.citation_number.map(|n| n.to_string()),
            "year-suffix" => self.item.year_suffix.clone(),
            "first-reference-note-number" => self.item.first_note.map(|n| n.to_string()),
            "title" if form == Form::Short => vars::text(self.item.src, "title").map(|t| vars::short_title(&t)),
            "issued" | "accessed" => vars::date(self.item.src, name).and_then(|d| d.year).map(|y| y.to_string()),
            _ => vars::text(self.item.src, name),
        };
        v.filter(|v| !v.trim().is_empty()).map(|v| Rich::plain_of(&v))
    }

    // ----- text, number, label -------------------------------------------------

    fn text(&mut self, t: &TextNode) -> Rich {
        let r = match &t.src {
            TextSource::Variable(v) => {
                self.called += 1;
                if (v == "author" && self.cite.suppress_author) || self.suppressed.contains(v) {
                    return Rich::new();
                }
                let r = self.var_value(v, t.form).unwrap_or_default();
                if !r.is_empty() {
                    self.populated += 1;
                }
                r
            }
            TextSource::Macro(m) => {
                if self.depth > 40 {
                    return Rich::new();
                }
                let Some(body) = self.style.macros.get(m) else { return Rich::new() };
                self.depth += 1;
                let r = self.run(body);
                self.depth -= 1;
                r
            }
            TextSource::Term(name) => {
                let plural = t.plural.unwrap_or(false);
                match self.loc.term(name, t.form, plural) {
                    Some(s) => Rich::plain_of(&s),
                    None => Rich::new(),
                }
            }
            TextSource::Value(v) => Rich::plain_of(v),
        };
        decorate(r, &t.fmt, self.loc)
    }

    fn number(&mut self, n: &NumberNode) -> Rich {
        self.called += 1;
        let Some(raw) = self.var_value(&n.variable, Form::Long).map(|r| r.plain()) else { return Rich::new() };
        self.populated += 1;
        let one = |s: &str| -> String {
            let digits: String = s.chars().take_while(|c| c.is_ascii_digit()).collect();
            let Ok(v) = digits.parse::<i64>() else { return s.trim().to_string() };
            // A number with letters around it ("2nd", "B1") is shown as written.
            if digits.len() != s.trim().len() {
                return s.trim().to_string();
            }
            match n.form {
                NumForm::Numeric => v.to_string(),
                NumForm::Ordinal => locale::ordinal(v),
                NumForm::LongOrdinal => locale::long_ordinal(v),
                NumForm::Roman => locale::roman(v),
            }
        };
        let delim = self.range_delimiter();
        let mut out = String::new();
        let mut cur = String::new();
        let flush = |cur: &mut String, out: &mut String| {
            if !cur.trim().is_empty() {
                out.push_str(&one(cur));
            }
            cur.clear();
        };
        for c in raw.chars() {
            match c {
                '-' | '\u{2013}' => {
                    flush(&mut cur, &mut out);
                    out.push_str(&delim);
                }
                ',' | '&' => {
                    flush(&mut cur, &mut out);
                    out.push(c);
                    out.push(' ');
                }
                c => cur.push(c),
            }
        }
        flush(&mut cur, &mut out);
        decorate(Rich::plain_of(out.trim()), &n.fmt, self.loc)
    }

    /// Whether the value of a variable counts as plural for a label.
    fn plural_for(&self, variable: &str, value: &str) -> bool {
        let numbers: Vec<&str> = value.split(|c: char| !c.is_ascii_digit()).filter(|s| !s.is_empty()).collect();
        if variable.starts_with("number-of-") {
            return value.trim().parse::<f64>().map(|v| v > 1.0).unwrap_or(false);
        }
        value.contains(['-', '\u{2013}', ',', '&']) && numbers.len() > 1
    }

    fn label_text(&self, l: &LabelNode, term: &str, plural: bool) -> Rich {
        let t = self.loc.term(term, l.form, plural).unwrap_or_default();
        decorate(Rich::plain_of(&t), &l.fmt, self.loc)
    }

    fn label(&mut self, l: &LabelNode) -> Rich {
        let value = match l.variable.as_str() {
            "locator" => self.cite.locator.clone(),
            v => self.var_value(v, Form::Long).map(|r| r.plain()),
        };
        let Some(value) = value else { return Rich::new() };
        let term = match l.variable.as_str() {
            "locator" | "number-of-pages" => "page",
            "chapter-number" => "chapter",
            "number-of-volumes" | "collection-number" => "volume",
            v => v,
        };
        let plural = match l.plural {
            Plural::Always => true,
            Plural::Never => false,
            Plural::Contextual => self.plural_for(&l.variable, &value),
        };
        self.label_text(l, term, plural)
    }

    // ----- dates ----------------------------------------------------------------

    fn date(&mut self, d: &DateNode) -> Rich {
        self.called += 1;
        if d.variable == "issued" && self.cite.suppress_year {
            return Rich::new();
        }
        let Some(dv) = vars::date(self.item.src, &d.variable) else { return Rich::new() };
        self.populated += 1;
        let part_node = |name: &str| d.parts.iter().find(|p| p.name == name);
        if let Some(lit) = &dv.literal {
            return decorate(Rich::plain_of(lit), &d.fmt, self.loc);
        }
        let (year, month, day) = (dv.year, dv.month, dv.day);
        let wanted = |part: &str| -> bool {
            match d.date_parts.as_deref() {
                Some("year") => part == "year",
                Some("year-month") => part != "day",
                _ => true,
            }
        };
        let suffix = if d.variable == "issued" && !self.style.year_suffix_var { self.item.year_suffix.clone() } else { None };
        let render = |name: &str, default_form: &str| -> Option<Rich> {
            let node = part_node(name);
            let form = node.map(|p| p.form.as_str()).filter(|f| !f.is_empty()).unwrap_or(default_form);
            let s = match name {
                "year" => year.map(|y| {
                    let mut s = if form == "short" { format!("{:02}", y.rem_euclid(100)) } else { y.to_string() };
                    if let Some(x) = &suffix {
                        s.push_str(x);
                    }
                    s
                }),
                "month" => month.map(|m| match form {
                    "numeric" => m.to_string(),
                    "numeric-leading-zeros" => format!("{m:02}"),
                    "short" => self.loc.month(m, true),
                    _ => self.loc.month(m, false),
                }),
                "day" => day.map(|v| match form {
                    "numeric-leading-zeros" => format!("{v:02}"),
                    "ordinal" => locale::ordinal(v as i64),
                    _ => v.to_string(),
                }),
                _ => None,
            }?;
            let r = Rich::plain_of(&s);
            Some(match node {
                Some(p) => decorate(r, &p.fmt, self.loc),
                None => r,
            })
        };
        let out = match d.form {
            Some(DateForm::Text) => {
                let (m, dd, y) = (if wanted("month") { render("month", "long") } else { None }, if wanted("day") { render("day", "numeric") } else { None }, if wanted("year") { render("year", "long") } else { None });
                let mut out = Rich::new();
                if let Some(m) = &m {
                    out.append(m);
                }
                if let Some(dd) = &dd {
                    if m.is_some() {
                        out.t(" ");
                    }
                    out.append(dd);
                }
                if let Some(y) = &y {
                    if dd.is_some() {
                        out.t(", ");
                    } else if m.is_some() {
                        out.t(" ");
                    }
                    out.append(y);
                }
                out
            }
            Some(DateForm::Numeric) => {
                let parts: Vec<Rich> = [
                    if wanted("month") { render("month", "numeric") } else { None },
                    if wanted("day") { render("day", "numeric") } else { None },
                    if wanted("year") { render("year", "long") } else { None },
                ]
                .into_iter()
                .flatten()
                .collect();
                text::join(parts, "/")
            }
            None => {
                let parts: Vec<Rich> = d.parts.iter().filter(|p| wanted(&p.name)).filter_map(|p| render(&p.name, "")).collect();
                text::join(parts, &d.delimiter)
            }
        };
        decorate(out, &d.fmt, self.loc)
    }

    // ----- names ------------------------------------------------------------------

    fn names(&mut self, n: &NamesNode) -> Rich {
        let default_name = NameNode { opts: NameOpts::default(), fmt: Fmt::default(), family: None, given: None };
        let name_node = n.name.as_ref().unwrap_or(&default_name);
        let opts = self.base_opts().merged(&n.opts).merged(&name_node.opts);
        let subsequent = self.cite.position != Position::First && self.mode == Mode::Citation;
        let mut blocks: Vec<Rich> = Vec::new();
        for v in &n.variables {
            self.called += 1;
            if self.suppressed.contains(v) || (v == "author" && self.cite.suppress_author) {
                continue;
            }
            let list: Vec<&Name> = vars::names(self.item.src, v).iter().collect();
            if list.is_empty() {
                continue;
            }
            self.populated += 1;
            let mut o = opts.clone();
            if o.form.is_none() {
                o.form = Some("long".into());
            }
            let mut r = names::list(&list, &o, Some(name_node), n.et_al.as_ref(), self.loc, subsequent);
            r = decorate(r, &name_node.fmt, self.loc);
            if let Some(l) = &n.label {
                let plural = match l.plural {
                    Plural::Always => true,
                    Plural::Never => false,
                    Plural::Contextual => list.len() > 1,
                };
                let lt = self.label_text(l, v, plural);
                r = if n.label_first { join_two(lt, r) } else { join_two(r, lt) };
            }
            blocks.push(r);
        }
        if blocks.is_empty() && !n.substitute.is_empty() {
            if let Some(r) = self.substitute(n) {
                return decorate(r, &n.fmt, self.loc);
            }
        }
        decorate(text::join(blocks, &n.delimiter), &n.fmt, self.loc)
    }

    /// The first substitute that has something to show.
    fn substitute(&mut self, n: &NamesNode) -> Option<Rich> {
        for child in &n.substitute {
            let r = match child {
                Node::Names(cn) => {
                    let mut c = (**cn).clone();
                    if c.name.is_none() {
                        c.name = n.name.clone();
                    }
                    if c.et_al.is_none() {
                        c.et_al = n.et_al.clone();
                    }
                    if c.label.is_none() {
                        c.label = n.label.clone();
                        c.label_first = n.label_first;
                    }
                    c.opts = n.opts.merged(&c.opts);
                    let r = self.names(&c);
                    if !r.is_empty() {
                        self.suppressed.extend(c.variables.iter().cloned());
                    }
                    r
                }
                other => {
                    let r = self.node(other);
                    if !r.is_empty() {
                        if let Node::Text(TextNode { src: TextSource::Variable(v), .. }) = other {
                            self.suppressed.push(v.clone());
                        }
                    }
                    r
                }
            };
            if !r.is_empty() {
                return Some(r);
            }
        }
        None
    }

    // ----- group, choose ----------------------------------------------------------

    fn group(&mut self, g: &GroupNode) -> Rich {
        let (called, populated) = (self.called, self.populated);
        self.called = 0;
        self.populated = 0;
        let parts = self.run_each(&g.children);
        let (c, p) = (self.called, self.populated);
        // What the group asked for counts for the group around it.
        self.called = called + c;
        self.populated = populated + p;
        // A group that asked for variables and got none of them is left out.
        if c > 0 && p == 0 {
            return Rich::new();
        }
        decorate(text::join(parts, &g.delimiter), &g.fmt, self.loc)
    }

    fn test(&self, t: &Test) -> bool {
        match t {
            Test::Type(ty) => vars::csl_type(self.item.src) == ty,
            Test::Variable(v) => self.has_var(v),
            Test::IsNumeric(v) => self.var_value(v, Form::Long).is_some_and(|r| vars::is_numeric(&r.plain())),
            Test::IsUncertainDate(v) => vars::date(self.item.src, v).is_some_and(|d| d.uncertain),
            Test::Locator(l) => self.cite.locator.is_some() && l == "page",
            Test::Position(p) => {
                use Position::*;
                matches!(
                    (p.as_str(), self.cite.position),
                    ("first", First) | ("subsequent", Subsequent | Ibid | IbidWithLocator) | ("ibid", Ibid | IbidWithLocator) | ("ibid-with-locator", IbidWithLocator)
                )
            }
            Test::Disambiguate(v) => self.item.disambiguate == *v,
        }
    }

    /// The body of the first branch whose conditions hold.
    fn pick<'n>(&self, c: &'n ChooseNode) -> Option<&'n [Node]> {
        for b in &c.branches {
            let hit = if b.is_else {
                true
            } else {
                match b.mode {
                    MatchMode::All => b.tests.iter().all(|t| self.test(t)),
                    MatchMode::Any => b.tests.iter().any(|t| self.test(t)),
                    MatchMode::None => !b.tests.iter().any(|t| self.test(t)),
                    MatchMode::Nand => !b.tests.iter().all(|t| self.test(t)),
                }
            };
            if hit {
                return Some(&b.body);
            }
        }
        None
    }

    fn choose(&mut self, c: &ChooseNode) -> Rich {
        match self.pick(c) {
            Some(body) => self.run(body),
            None => Rich::new(),
        }
    }

    /// The outputs an element stands for inside a group. A condition is transparent: the elements of the
    /// branch it picks count as elements of the group, so the group's delimiter goes between them.
    fn pieces(&mut self, n: &Node, out: &mut Vec<Rich>) {
        match n {
            Node::Choose(c) => {
                if let Some(body) = self.pick(c) {
                    for child in body {
                        self.pieces(child, out);
                    }
                }
            }
            other => out.push(self.node(other)),
        }
    }
}

fn join_two(a: Rich, b: Rich) -> Rich {
    let mut out = Rich::new();
    out.append(&a);
    out.append(&b);
    out
}
