//! A CSL 1.0.2 style read into a tree the processor walks: the options that cascade, the macros, the
//! citation and bibliography layouts, and the rendering elements inside them.

use crate::xml::{self, Element};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Class {
    #[default]
    InText,
    Note,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextCase {
    Lowercase,
    Uppercase,
    CapitalizeFirst,
    CapitalizeAll,
    Title,
    Sentence,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Vertical {
    Baseline,
    Sup,
    Sub,
}

/// Formatting that any rendering element can carry.
#[derive(Debug, Clone, Default)]
pub struct Fmt {
    pub prefix: String,
    pub suffix: String,
    /// `Some(false)` is an explicit "normal".
    pub italic: Option<bool>,
    pub bold: Option<bool>,
    pub small_caps: Option<bool>,
    pub vertical: Option<Vertical>,
    pub text_case: Option<TextCase>,
    pub quotes: bool,
    pub strip_periods: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Form {
    #[default]
    Long,
    Short,
    Verb,
    VerbShort,
    Symbol,
}

impl Form {
    pub fn parse(s: &str) -> Form {
        match s {
            "short" => Form::Short,
            "verb" => Form::Verb,
            "verb-short" => Form::VerbShort,
            "symbol" => Form::Symbol,
            _ => Form::Long,
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            Form::Long => "long",
            Form::Short => "short",
            Form::Verb => "verb",
            Form::VerbShort => "verb-short",
            Form::Symbol => "symbol",
        }
    }
}

#[derive(Debug, Clone)]
pub enum TextSource {
    Variable(String),
    Macro(String),
    Term(String),
    Value(String),
}

#[derive(Debug, Clone)]
pub struct TextNode {
    pub fmt: Fmt,
    pub src: TextSource,
    pub form: Form,
    pub plural: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NumForm {
    #[default]
    Numeric,
    Ordinal,
    LongOrdinal,
    Roman,
}

#[derive(Debug, Clone)]
pub struct NumberNode {
    pub fmt: Fmt,
    pub variable: String,
    pub form: NumForm,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Plural {
    #[default]
    Contextual,
    Always,
    Never,
}

#[derive(Debug, Clone)]
pub struct LabelNode {
    pub fmt: Fmt,
    pub variable: String,
    pub form: Form,
    pub plural: Plural,
}

#[derive(Debug, Clone)]
pub struct GroupNode {
    pub fmt: Fmt,
    pub delimiter: String,
    pub children: Vec<Node>,
}

#[derive(Debug, Clone)]
pub enum Test {
    Type(String),
    Variable(String),
    IsNumeric(String),
    IsUncertainDate(String),
    Locator(String),
    Position(String),
    Disambiguate(bool),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchMode {
    All,
    Any,
    None,
    Nand,
}

#[derive(Debug, Clone)]
pub struct Branch {
    pub tests: Vec<Test>,
    pub mode: MatchMode,
    /// An `else` branch always applies.
    pub is_else: bool,
    pub body: Vec<Node>,
}

#[derive(Debug, Clone)]
pub struct ChooseNode {
    pub branches: Vec<Branch>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DateForm {
    Text,
    Numeric,
}

#[derive(Debug, Clone)]
pub struct DatePart {
    pub name: String,
    pub form: String,
    pub fmt: Fmt,
}

#[derive(Debug, Clone)]
pub struct DateNode {
    pub fmt: Fmt,
    pub variable: String,
    pub form: Option<DateForm>,
    /// `year`, `year-month` or `year-month-day`.
    pub date_parts: Option<String>,
    pub delimiter: String,
    pub parts: Vec<DatePart>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum And {
    Text,
    Symbol,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Precedes {
    Contextual,
    AfterInverted,
    Always,
    Never,
}

impl Precedes {
    fn parse(s: &str) -> Option<Precedes> {
        Some(match s {
            "contextual" => Precedes::Contextual,
            "after-inverted-name" => Precedes::AfterInverted,
            "always" => Precedes::Always,
            "never" => Precedes::Never,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortOrder {
    First,
    All,
}

/// The options of a name list. They cascade: style, then citation or bibliography, then `names`, then `name`.
#[derive(Debug, Clone, Default)]
pub struct NameOpts {
    pub and: Option<And>,
    pub delimiter: Option<String>,
    pub delimiter_precedes_et_al: Option<Precedes>,
    pub delimiter_precedes_last: Option<Precedes>,
    pub et_al_min: Option<usize>,
    pub et_al_use_first: Option<usize>,
    pub et_al_use_last: Option<bool>,
    pub et_al_subsequent_min: Option<usize>,
    pub et_al_subsequent_use_first: Option<usize>,
    pub initialize: Option<bool>,
    pub initialize_with: Option<String>,
    pub initialize_with_hyphen: Option<bool>,
    pub name_as_sort_order: Option<SortOrder>,
    pub sort_separator: Option<String>,
    /// `long`, `short` or `count`.
    pub form: Option<String>,
}

impl NameOpts {
    /// `self` with every option `over` sets taking its place.
    pub fn merged(&self, over: &NameOpts) -> NameOpts {
        macro_rules! pick {
            ($f:ident) => {
                over.$f.clone().or_else(|| self.$f.clone())
            };
        }
        NameOpts {
            and: pick!(and),
            delimiter: pick!(delimiter),
            delimiter_precedes_et_al: pick!(delimiter_precedes_et_al),
            delimiter_precedes_last: pick!(delimiter_precedes_last),
            et_al_min: pick!(et_al_min),
            et_al_use_first: pick!(et_al_use_first),
            et_al_use_last: pick!(et_al_use_last),
            et_al_subsequent_min: pick!(et_al_subsequent_min),
            et_al_subsequent_use_first: pick!(et_al_subsequent_use_first),
            initialize: pick!(initialize),
            initialize_with: pick!(initialize_with),
            initialize_with_hyphen: pick!(initialize_with_hyphen),
            name_as_sort_order: pick!(name_as_sort_order),
            sort_separator: pick!(sort_separator),
            form: pick!(form),
        }
    }

    fn read(el: &Element, name_element: bool) -> NameOpts {
        let s = |k: &str| el.attr(k).map(str::to_string);
        let n = |k: &str| el.attr(k).and_then(|v| v.trim().parse::<usize>().ok());
        let b = |k: &str| el.attr(k).map(|v| v == "true");
        NameOpts {
            and: el.attr("and").and_then(|v| match v {
                "text" => Some(And::Text),
                "symbol" => Some(And::Symbol),
                _ => None,
            }),
            delimiter: if name_element { s("delimiter") } else { None },
            delimiter_precedes_et_al: el.attr("delimiter-precedes-et-al").and_then(Precedes::parse),
            delimiter_precedes_last: el.attr("delimiter-precedes-last").and_then(Precedes::parse),
            et_al_min: n("et-al-min"),
            et_al_use_first: n("et-al-use-first"),
            et_al_use_last: b("et-al-use-last"),
            et_al_subsequent_min: n("et-al-subsequent-min"),
            et_al_subsequent_use_first: n("et-al-subsequent-use-first"),
            initialize: b("initialize"),
            initialize_with: s("initialize-with"),
            initialize_with_hyphen: b("initialize-with-hyphen"),
            name_as_sort_order: el.attr("name-as-sort-order").and_then(|v| match v {
                "first" => Some(SortOrder::First),
                "all" => Some(SortOrder::All),
                _ => None,
            }),
            sort_separator: s("sort-separator"),
            form: if name_element { s("form") } else { None },
        }
    }
}

#[derive(Debug, Clone)]
pub struct NameNode {
    pub opts: NameOpts,
    pub fmt: Fmt,
    pub family: Option<Fmt>,
    pub given: Option<Fmt>,
}

#[derive(Debug, Clone)]
pub struct NamesNode {
    pub fmt: Fmt,
    pub variables: Vec<String>,
    pub opts: NameOpts,
    /// The delimiter between the lists of different variables.
    pub delimiter: String,
    pub name: Option<NameNode>,
    pub et_al: Option<(Fmt, String)>,
    pub label: Option<LabelNode>,
    pub label_first: bool,
    pub substitute: Vec<Node>,
}

#[derive(Debug, Clone)]
pub enum Node {
    Text(TextNode),
    Number(NumberNode),
    Date(DateNode),
    Names(Box<NamesNode>),
    Label(LabelNode),
    Group(GroupNode),
    Choose(ChooseNode),
}

#[derive(Debug, Clone)]
pub struct TermDef {
    pub name: String,
    pub form: Form,
    pub single: String,
    pub multiple: String,
}

#[derive(Debug, Clone)]
pub struct SortKey {
    pub variable: Option<String>,
    pub macro_name: Option<String>,
    pub descending: bool,
}

#[derive(Debug, Clone, Default)]
pub struct Layout {
    pub fmt: Fmt,
    pub delimiter: String,
    pub body: Vec<Node>,
}

#[derive(Debug, Clone, Default)]
pub struct Citation {
    pub layout: Layout,
    pub sort: Vec<SortKey>,
    pub opts: NameOpts,
    pub add_year_suffix: bool,
    pub collapse: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct Bibliography {
    pub layout: Layout,
    pub sort: Vec<SortKey>,
    pub opts: NameOpts,
    pub hanging_indent: bool,
    pub second_field_align: Option<String>,
    pub subsequent_author_substitute: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct Style {
    pub id: String,
    pub title: String,
    pub class: Class,
    /// `numeric`, `author-date`, `author`, `label` or `note`, from the style's own category.
    pub citation_format: String,
    pub opts: NameOpts,
    pub page_range_format: Option<String>,
    pub macros: HashMap<String, Vec<Node>>,
    pub terms: Vec<TermDef>,
    pub citation: Citation,
    pub bibliography: Option<Bibliography>,
    /// Whether the style places the year suffix itself, with `text variable="year-suffix"`.
    pub year_suffix_var: bool,
}

fn fmt_of(el: &Element) -> Fmt {
    let on = |v: &str, yes: &[&str], no: &[&str]| -> Option<bool> {
        if yes.contains(&v) {
            Some(true)
        } else if no.contains(&v) {
            Some(false)
        } else {
            None
        }
    };
    let a = |k: &str| el.attr(k).unwrap_or("");
    Fmt {
        prefix: a("prefix").to_string(),
        suffix: a("suffix").to_string(),
        italic: on(a("font-style"), &["italic", "oblique"], &["normal"]),
        bold: on(a("font-weight"), &["bold"], &["normal", "light"]),
        small_caps: on(a("font-variant"), &["small-caps"], &["normal"]),
        vertical: match a("vertical-align") {
            "sup" => Some(Vertical::Sup),
            "sub" => Some(Vertical::Sub),
            "baseline" => Some(Vertical::Baseline),
            _ => None,
        },
        text_case: match a("text-case") {
            "lowercase" => Some(TextCase::Lowercase),
            "uppercase" => Some(TextCase::Uppercase),
            "capitalize-first" => Some(TextCase::CapitalizeFirst),
            "capitalize-all" => Some(TextCase::CapitalizeAll),
            "title" => Some(TextCase::Title),
            "sentence" => Some(TextCase::Sentence),
            _ => None,
        },
        quotes: a("quotes") == "true",
        strip_periods: a("strip-periods") == "true",
    }
}

fn words(s: &str) -> Vec<String> {
    s.split_whitespace().map(str::to_string).collect()
}

fn parse_tests(el: &Element) -> (Vec<Test>, MatchMode) {
    let mut tests = Vec::new();
    for (k, v) in &el.attrs {
        match k.as_str() {
            "type" => tests.extend(words(v).into_iter().map(Test::Type)),
            "variable" => tests.extend(words(v).into_iter().map(Test::Variable)),
            "is-numeric" => tests.extend(words(v).into_iter().map(Test::IsNumeric)),
            "is-uncertain-date" => tests.extend(words(v).into_iter().map(Test::IsUncertainDate)),
            "locator" => tests.extend(words(v).into_iter().map(Test::Locator)),
            "position" => tests.extend(words(v).into_iter().map(Test::Position)),
            "disambiguate" => tests.push(Test::Disambiguate(v == "true")),
            _ => {}
        }
    }
    let mode = match el.attr("match") {
        Some("any") => MatchMode::Any,
        Some("none") => MatchMode::None,
        Some("nand") => MatchMode::Nand,
        _ => MatchMode::All,
    };
    (tests, mode)
}

fn parse_nodes(parent: &Element, style: &mut Style) -> Result<Vec<Node>, String> {
    let mut out = Vec::new();
    for c in parent.elements() {
        if let Some(n) = parse_node(c, style)? {
            out.push(n);
        }
    }
    Ok(out)
}

fn parse_label(el: &Element) -> LabelNode {
    LabelNode {
        fmt: fmt_of(el),
        variable: el.attr("variable").unwrap_or("").to_string(),
        form: Form::parse(el.attr("form").unwrap_or("long")),
        plural: match el.attr("plural") {
            Some("always") => Plural::Always,
            Some("never") => Plural::Never,
            _ => Plural::Contextual,
        },
    }
}

fn parse_node(el: &Element, style: &mut Style) -> Result<Option<Node>, String> {
    Ok(Some(match el.local() {
        "text" => {
            let src = if let Some(v) = el.attr("variable") {
                if v == "year-suffix" {
                    style.year_suffix_var = true;
                }
                TextSource::Variable(v.to_string())
            } else if let Some(m) = el.attr("macro") {
                TextSource::Macro(m.to_string())
            } else if let Some(t) = el.attr("term") {
                TextSource::Term(t.to_string())
            } else if let Some(v) = el.attr("value") {
                TextSource::Value(v.to_string())
            } else {
                return Ok(None);
            };
            Node::Text(TextNode { fmt: fmt_of(el), src, form: Form::parse(el.attr("form").unwrap_or("long")), plural: el.attr("plural").map(|p| p != "false" && p != "never") })
        }
        "number" => Node::Number(NumberNode {
            fmt: fmt_of(el),
            variable: el.attr("variable").unwrap_or("").to_string(),
            form: match el.attr("form") {
                Some("ordinal") => NumForm::Ordinal,
                Some("long-ordinal") => NumForm::LongOrdinal,
                Some("roman") => NumForm::Roman,
                _ => NumForm::Numeric,
            },
        }),
        "label" => Node::Label(parse_label(el)),
        "group" => Node::Group(GroupNode { fmt: fmt_of(el), delimiter: el.attr("delimiter").unwrap_or("").to_string(), children: parse_nodes(el, style)? }),
        "choose" => {
            let mut branches = Vec::new();
            for b in el.elements() {
                let is_else = b.local() == "else";
                if !is_else && b.local() != "if" && b.local() != "else-if" {
                    continue;
                }
                let (tests, mode) = if is_else { (Vec::new(), MatchMode::All) } else { parse_tests(b) };
                branches.push(Branch { tests, mode, is_else, body: parse_nodes(b, style)? });
            }
            Node::Choose(ChooseNode { branches })
        }
        "date" => {
            let mut parts = Vec::new();
            for p in el.elements().filter(|p| p.local() == "date-part") {
                parts.push(DatePart { name: p.attr("name").unwrap_or("").to_string(), form: p.attr("form").unwrap_or("").to_string(), fmt: fmt_of(p) });
            }
            Node::Date(DateNode {
                fmt: fmt_of(el),
                variable: el.attr("variable").unwrap_or("").to_string(),
                form: match el.attr("form") {
                    Some("text") => Some(DateForm::Text),
                    Some("numeric") => Some(DateForm::Numeric),
                    _ => None,
                },
                date_parts: el.attr("date-parts").map(str::to_string),
                delimiter: el.attr("delimiter").unwrap_or("").to_string(),
                parts,
            })
        }
        "names" => {
            let mut n = NamesNode {
                fmt: fmt_of(el),
                variables: words(el.attr("variable").unwrap_or("")),
                opts: NameOpts::read(el, false),
                delimiter: el.attr("delimiter").unwrap_or("").to_string(),
                name: None,
                et_al: None,
                label: None,
                label_first: false,
                substitute: Vec::new(),
            };
            for c in el.elements() {
                match c.local() {
                    "name" => {
                        let mut family = None;
                        let mut given = None;
                        for p in c.elements().filter(|p| p.local() == "name-part") {
                            match p.attr("name") {
                                Some("family") => family = Some(fmt_of(p)),
                                Some("given") => given = Some(fmt_of(p)),
                                _ => {}
                            }
                        }
                        n.name = Some(NameNode { opts: NameOpts::read(c, true), fmt: fmt_of(c), family, given });
                        // A label written before the name comes first; without a name element it follows the names.
                        n.label_first = n.label.is_some();
                    }
                    "et-al" => n.et_al = Some((fmt_of(c), c.attr("term").unwrap_or("et-al").to_string())),
                    "label" => n.label = Some(parse_label(c)),
                    "substitute" => n.substitute = parse_nodes(c, style)?,
                    _ => {}
                }
            }
            Node::Names(Box::new(n))
        }
        _ => return Ok(None),
    }))
}

fn parse_sort(el: &Element) -> Vec<SortKey> {
    el.elements()
        .find(|s| s.local() == "sort")
        .map(|s| {
            s.elements()
                .filter(|k| k.local() == "key")
                .map(|k| SortKey { variable: k.attr("variable").map(str::to_string), macro_name: k.attr("macro").map(str::to_string), descending: k.attr("sort") == Some("descending") })
                .collect()
        })
        .unwrap_or_default()
}

fn parse_layout(el: &Element, style: &mut Style) -> Result<Layout, String> {
    let Some(l) = el.elements().find(|c| c.local() == "layout") else { return Err(format!("The style's {} has no layout.", el.local())) };
    Ok(Layout { fmt: fmt_of(l), delimiter: l.attr("delimiter").unwrap_or("").to_string(), body: parse_nodes(l, style)? })
}

fn parse_terms(locale: &Element, out: &mut Vec<TermDef>) {
    let Some(terms) = locale.elements().find(|t| t.local() == "terms") else { return };
    for t in terms.elements().filter(|t| t.local() == "term") {
        let Some(name) = t.attr("name") else { continue };
        let form = Form::parse(t.attr("form").unwrap_or("long"));
        let single = t.elements().find(|s| s.local() == "single").map(|s| s.text());
        let multiple = t.elements().find(|s| s.local() == "multiple").map(|s| s.text());
        let whole = t.text();
        let (single, multiple) = match (single, multiple) {
            (Some(s), Some(m)) => (s, m),
            (Some(s), None) => (s.clone(), s),
            (None, Some(m)) => (m.clone(), m),
            (None, None) => (whole.trim().to_string(), whole.trim().to_string()),
        };
        out.push(TermDef { name: name.to_string(), form, single, multiple });
    }
}

impl Style {
    /// Read a CSL style. The message is fit to show.
    pub fn parse(text: &str) -> Result<Style, String> {
        crate::budget::within(text.len(), crate::budget::caps::LIST_BYTES).map_err(|e| e.to_string())?;
        let doc = xml::parse(text.trim_start_matches('\u{feff}').as_bytes()).map_err(|e| format!("That is not a CSL style: {e}"))?;
        let root = &doc.root;
        if root.local() != "style" {
            return Err("That is not a CSL style: the file does not start with a style element.".into());
        }
        let mut style = Style {
            class: if root.attr("class") == Some("note") { Class::Note } else { Class::InText },
            opts: NameOpts::read(root, false),
            page_range_format: root.attr("page-range-format").map(str::to_string),
            ..Default::default()
        };
        // The style-level name options include the delimiter, as `name-delimiter`.
        style.opts.delimiter = root.attr("name-delimiter").map(str::to_string);
        if let Some(info) = root.elements().find(|e| e.local() == "info") {
            style.id = info.elements().find(|e| e.local() == "id").map(|e| e.text().trim().to_string()).unwrap_or_default();
            style.title = info.elements().find(|e| e.local() == "title").map(|e| e.text().trim().to_string()).unwrap_or_default();
            style.citation_format = info.elements().filter(|e| e.local() == "category").find_map(|e| e.attr("citation-format")).unwrap_or("").to_string();
        }
        for loc in root.elements().filter(|e| e.local() == "locale") {
            let lang = loc.attr("xml:lang").unwrap_or("");
            // Docxtor formats in English: terms for English, or for every language, apply.
            if lang.is_empty() || lang == "en" || lang == "en-US" {
                parse_terms(loc, &mut style.terms);
            }
        }
        // Macros first, so every one is known before the layouts that call them.
        for m in root.elements().filter(|e| e.local() == "macro") {
            let name = m.attr("name").ok_or("A macro in the style has no name.")?.to_string();
            let nodes = parse_nodes(m, &mut style)?;
            style.macros.insert(name, nodes);
        }
        let citation = root.elements().find(|e| e.local() == "citation").ok_or("The style has no citation element.")?;
        let layout = parse_layout(citation, &mut style)?;
        style.citation = Citation {
            layout,
            sort: parse_sort(citation),
            opts: NameOpts::read(citation, false),
            add_year_suffix: citation.attr("disambiguate-add-year-suffix") == Some("true"),
            collapse: citation.attr("collapse").map(str::to_string),
        };
        if let Some(b) = root.elements().find(|e| e.local() == "bibliography") {
            let layout = parse_layout(b, &mut style)?;
            style.bibliography = Some(Bibliography {
                layout,
                sort: parse_sort(b),
                opts: NameOpts::read(b, false),
                hanging_indent: b.attr("hanging-indent") == Some("true"),
                second_field_align: b.attr("second-field-align").map(str::to_string),
                subsequent_author_substitute: b.attr("subsequent-author-substitute").map(str::to_string),
            });
        }
        if style.title.is_empty() {
            style.title = "Untitled style".into();
        }
        Ok(style)
    }

    /// Whether in-text citations are numbers that point into the bibliography.
    pub fn is_numeric(&self) -> bool {
        self.citation_format == "numeric" || self.citation_format == "label"
    }
}
