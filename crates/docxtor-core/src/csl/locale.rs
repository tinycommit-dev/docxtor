//! Terms: Docxtor's own small en-US locale, with the terms a style defines for itself taking precedence.

use super::model::{Form, Style, TermDef};

/// (name, form, single, multiple). A term with no plural form uses the single form for both.
const EN_US: &[(&str, &str, &str, &str)] = &[
    ("accessed", "long", "accessed", ""),
    ("and", "long", "and", ""),
    ("and", "symbol", "&", ""),
    ("and others", "long", "and others", ""),
    ("anonymous", "long", "anonymous", ""),
    ("anonymous", "short", "anon.", ""),
    ("at", "long", "at", ""),
    ("available at", "long", "available at", ""),
    ("by", "long", "by", ""),
    ("circa", "long", "circa", ""),
    ("circa", "short", "ca.", ""),
    ("cited", "long", "cited", ""),
    ("edition", "long", "edition", "editions"),
    ("edition", "short", "ed.", "eds."),
    ("et-al", "long", "et al.", ""),
    ("forthcoming", "long", "forthcoming", ""),
    ("from", "long", "from", ""),
    ("ibid", "long", "ibid.", ""),
    ("ibid", "short", "ibid.", ""),
    ("in", "long", "in", ""),
    ("in press", "long", "in press", ""),
    ("internet", "long", "internet", ""),
    ("interview", "long", "interview", ""),
    ("letter", "long", "letter", ""),
    ("no date", "long", "no date", ""),
    ("no date", "short", "n.d.", ""),
    ("online", "long", "online", ""),
    ("presented at", "long", "presented at", ""),
    ("reference", "long", "reference", "references"),
    ("reference", "short", "ref.", "refs."),
    ("retrieved", "long", "retrieved", ""),
    ("version", "long", "version", ""),
    ("editor", "long", "editor", "editors"),
    ("editor", "short", "ed.", "eds."),
    ("editor", "verb", "edited by", ""),
    ("editor", "verb-short", "ed.", ""),
    ("translator", "long", "translator", "translators"),
    ("translator", "short", "trans.", ""),
    ("translator", "verb", "translated by", ""),
    ("translator", "verb-short", "trans.", ""),
    ("editortranslator", "long", "editor & translator", "editors & translators"),
    ("editortranslator", "short", "ed. & trans.", "eds. & trans."),
    ("editortranslator", "verb", "edited & translated by", ""),
    ("editortranslator", "verb-short", "ed. & trans.", ""),
    ("editor-translator", "long", "editor & translator", "editors & translators"),
    ("editor-translator", "short", "ed. & trans.", "eds. & trans."),
    ("editor-translator", "verb", "edited & translated by", ""),
    ("editor-translator", "verb-short", "ed. & trans.", ""),
    ("director", "long", "director", "directors"),
    ("director", "short", "dir.", "dirs."),
    ("director", "verb", "directed by", ""),
    ("compiler", "long", "compiler", "compilers"),
    ("compiler", "short", "comp.", "comps."),
    ("compiler", "verb", "compiled by", ""),
    ("collection-editor", "long", "editor", "editors"),
    ("collection-editor", "short", "ed.", "eds."),
    ("collection-editor", "verb", "edited by", ""),
    ("book", "long", "book", "books"),
    ("book", "short", "bk.", "bks."),
    ("chapter", "long", "chapter", "chapters"),
    ("chapter", "short", "chap.", "chaps."),
    ("column", "long", "column", "columns"),
    ("column", "short", "col.", "cols."),
    ("figure", "long", "figure", "figures"),
    ("figure", "short", "fig.", "figs."),
    ("folio", "long", "folio", "folios"),
    ("folio", "short", "fol.", "fols."),
    ("issue", "long", "issue", "issues"),
    ("issue", "short", "no.", "nos."),
    ("line", "long", "line", "lines"),
    ("line", "short", "l.", "ll."),
    ("note", "long", "note", "notes"),
    ("note", "short", "n.", "nn."),
    ("opus", "long", "opus", "opera"),
    ("opus", "short", "op.", "opp."),
    ("page", "long", "page", "pages"),
    ("page", "short", "p.", "pp."),
    ("paragraph", "long", "paragraph", "paragraphs"),
    ("paragraph", "short", "para.", "paras."),
    ("paragraph", "symbol", "\u{b6}", "\u{b6}\u{b6}"),
    ("part", "long", "part", "parts"),
    ("part", "short", "pt.", "pts."),
    ("section", "long", "section", "sections"),
    ("section", "short", "sec.", "secs."),
    ("section", "symbol", "\u{a7}", "\u{a7}\u{a7}"),
    ("sub verbo", "long", "sub verbo", "sub verbis"),
    ("sub verbo", "short", "s.v.", "s.vv."),
    ("verse", "long", "verse", "verses"),
    ("verse", "short", "v.", "vv."),
    ("volume", "long", "volume", "volumes"),
    ("volume", "short", "vol.", "vols."),
    ("page-range-delimiter", "long", "\u{2013}", ""),
    ("open-quote", "long", "\u{201c}", ""),
    ("close-quote", "long", "\u{201d}", ""),
    ("open-inner-quote", "long", "\u{2018}", ""),
    ("close-inner-quote", "long", "\u{2019}", ""),
    ("month-01", "long", "January", ""),
    ("month-02", "long", "February", ""),
    ("month-03", "long", "March", ""),
    ("month-04", "long", "April", ""),
    ("month-05", "long", "May", ""),
    ("month-06", "long", "June", ""),
    ("month-07", "long", "July", ""),
    ("month-08", "long", "August", ""),
    ("month-09", "long", "September", ""),
    ("month-10", "long", "October", ""),
    ("month-11", "long", "November", ""),
    ("month-12", "long", "December", ""),
    ("month-01", "short", "Jan.", ""),
    ("month-02", "short", "Feb.", ""),
    ("month-03", "short", "Mar.", ""),
    ("month-04", "short", "Apr.", ""),
    ("month-05", "short", "May", ""),
    ("month-06", "short", "Jun.", ""),
    ("month-07", "short", "Jul.", ""),
    ("month-08", "short", "Aug.", ""),
    ("month-09", "short", "Sep.", ""),
    ("month-10", "short", "Oct.", ""),
    ("month-11", "short", "Nov.", ""),
    ("month-12", "short", "Dec.", ""),
    ("season-01", "long", "Spring", ""),
    ("season-02", "long", "Summer", ""),
    ("season-03", "long", "Autumn", ""),
    ("season-04", "long", "Winter", ""),
];

/// Looks terms up in the style's own list first, then in Docxtor's en-US set.
pub struct Locale<'a> {
    style: &'a [TermDef],
}

impl<'a> Locale<'a> {
    pub fn new(style: &'a Style) -> Locale<'a> {
        Locale { style: &style.terms }
    }

    fn find(&self, name: &str, form: Form, plural: bool) -> Option<String> {
        // A term defined by the style wins, the last definition first.
        if let Some(t) = self.style.iter().rev().find(|t| t.name == name && t.form == form) {
            return Some(if plural { t.multiple.clone() } else { t.single.clone() });
        }
        EN_US.iter().find(|(n, f, _, _)| *n == name && *f == form.id()).map(|(_, _, s, m)| {
            if plural && !m.is_empty() {
                m.to_string()
            } else {
                s.to_string()
            }
        })
    }

    /// A term in a form; a missing form falls back the way CSL says: verb-short to verb, symbol to short, short to long.
    pub fn term(&self, name: &str, form: Form, plural: bool) -> Option<String> {
        let chain: &[Form] = match form {
            Form::Long => &[Form::Long],
            Form::Short => &[Form::Short, Form::Long],
            Form::Verb => &[Form::Verb, Form::Long],
            Form::VerbShort => &[Form::VerbShort, Form::Verb, Form::Long],
            Form::Symbol => &[Form::Symbol, Form::Short, Form::Long],
        };
        chain.iter().find_map(|f| self.find(name, *f, plural))
    }

    pub fn month(&self, month: u32, short: bool) -> String {
        self.term(&format!("month-{month:02}"), if short { Form::Short } else { Form::Long }, false).unwrap_or_default()
    }
}

/// "1st", "2nd", "3rd", "4th".
pub fn ordinal(n: i64) -> String {
    let suffix = match (n % 100, n % 10) {
        (11..=13, _) => "th",
        (_, 1) => "st",
        (_, 2) => "nd",
        (_, 3) => "rd",
        _ => "th",
    };
    format!("{n}{suffix}")
}

/// "first" to "tenth"; larger numbers stay numeric ordinals.
pub fn long_ordinal(n: i64) -> String {
    const NAMES: [&str; 10] = ["first", "second", "third", "fourth", "fifth", "sixth", "seventh", "eighth", "ninth", "tenth"];
    if (1..=10).contains(&n) {
        NAMES[n as usize - 1].to_string()
    } else {
        ordinal(n)
    }
}

pub fn roman(mut n: i64) -> String {
    if !(1..4000).contains(&n) {
        return n.to_string();
    }
    let table = [(1000, "m"), (900, "cm"), (500, "d"), (400, "cd"), (100, "c"), (90, "xc"), (50, "l"), (40, "xl"), (10, "x"), (9, "ix"), (5, "v"), (4, "iv"), (1, "i")];
    let mut out = String::new();
    for (v, s) in table {
        while n >= v {
            out.push_str(s);
            n -= v;
        }
    }
    out
}
