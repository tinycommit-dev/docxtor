//! Docxtor's own headers and footers, for Insert, Header and Insert, Footer. Each is a few paragraphs in the Header or
//! Footer style, built for the width of the section's text: boxes for the document's title, author, date or company
//! (content controls bound to the properties, as on a cover page), the page number as a PAGE field, and tab stops at
//! the middle and the right edge for the three-part ones. Colours and fonts name the theme.

use super::xml::*;
use crate::shapes::theme::ShapeTheme;

/// A built-in design: its id and the name the gallery shows (English).
pub struct Design {
    pub id: &'static str,
    pub name: &'static str,
}

pub const HEADERS: &[Design] = &[
    Design { id: "docxtor.blank", name: "Blank" },
    Design { id: "docxtor.three", name: "Blank (three columns)" },
    Design { id: "docxtor.title", name: "Title" },
    Design { id: "docxtor.titleDate", name: "Title and date" },
    Design { id: "docxtor.banded", name: "Banded" },
    Design { id: "docxtor.titlePage", name: "Title and page number" },
];

pub const FOOTERS: &[Design] = &[
    Design { id: "docxtor.blank", name: "Blank" },
    Design { id: "docxtor.three", name: "Blank (three columns)" },
    Design { id: "docxtor.pageOf", name: "Page number" },
    Design { id: "docxtor.authorPage", name: "Author and page number" },
    Design { id: "docxtor.banded", name: "Banded" },
    Design { id: "docxtor.companyDate", name: "Company and date" },
];

pub fn designs(footer: bool) -> &'static [Design] {
    if footer {
        FOOTERS
    } else {
        HEADERS
    }
}

const DARK: Clr = Clr::text(Lum::Plain);
const MUTED: Clr = Clr::text(Lum::Lighter(35));
const ACCENT: Clr = Clr::accent(1, Lum::Plain);
const ACCENT_DEEP: Clr = Clr::accent(1, Lum::Darker(25));

/// The paragraphs of a design, for a section whose text is `text_w` twips wide, in the paragraph style `style`
/// (Header or Footer). None when the id is not one of Docxtor's.
pub fn build(id: &str, footer: bool, env: &mut dyn Env, th: &ShapeTheme, text_w: i64, style: &str) -> Option<String> {
    designs(footer).iter().find(|d| d.id == id)?;
    let mid = text_w / 2;
    let right = text_w;
    let p = |align: &'static str| Para::new(align).style(style);
    let three = p("left").tabs(Some(mid), Some(right));
    let body = Look::new(10.0, DARK);
    let small = Look::new(9.0, MUTED);
    let tab = || "<w:r><w:tab/></w:r>".to_string();
    let rule = Rule { width: 8, color: ACCENT, style: "single", space: 4 };
    let out = match (footer, id) {
        (_, "docxtor.blank") => para(th, &p("center"), &body, &control(env, th, Field::Type, &body)),
        (_, "docxtor.three") => {
            let inner = [control(env, th, Field::Type, &body), tab(), control(env, th, Field::Type, &body), tab(), control(env, th, Field::Type, &body)].concat();
            para(th, &three, &body, &inner)
        }
        (false, "docxtor.title") => {
            let look = Look::new(11.0, ACCENT_DEEP).heading().caps().track(1.0);
            para(th, &p("left").bottom(rule), &look, &control(env, th, Field::Title, &look))
        }
        (false, "docxtor.titleDate") => {
            let inner = [control(env, th, Field::Title, &body), tab(), tab(), control(env, th, Field::Date, &small)].concat();
            para(th, &three.bottom(Rule { width: 4, color: MUTED, style: "single", space: 4 }), &body, &inner)
        }
        (false, "docxtor.banded") => {
            let look = Look::new(10.0, Clr::WHITE).heading().caps().bold().track(1.5);
            para(th, &p("center").shaded(ACCENT).padded(), &look, &control(env, th, Field::Title, &look))
        }
        (false, "docxtor.titlePage") => {
            let page = Look::new(10.0, ACCENT_DEEP).bold();
            let inner = [control(env, th, Field::Title, &body), tab(), tab(), field(th, &page, "PAGE", "1")].concat();
            para(th, &three, &body, &inner)
        }
        (true, "docxtor.pageOf") => {
            let page = env.word("page", "Page");
            let of = env.word("of", "of");
            let inner = [run(th, &body, &format!("{page} ")), field(th, &body.bold(), "PAGE", "1"), run(th, &body, &format!(" {of} ")), field(th, &body.bold(), "NUMPAGES", "1")].concat();
            para(th, &p("center"), &body, &inner)
        }
        (true, "docxtor.authorPage") => {
            let page = Look::new(10.0, ACCENT_DEEP).bold();
            let inner = [control(env, th, Field::Author, &small), tab(), tab(), field(th, &page, "PAGE", "1")].concat();
            para(th, &three.top(rule), &body, &inner)
        }
        (true, "docxtor.banded") => {
            let look = Look::new(10.0, Clr::WHITE).bold();
            let inner = [control(env, th, Field::Title, &Look::new(9.0, Clr::WHITE).caps().track(1.0)), tab(), tab(), field(th, &look, "PAGE", "1")].concat();
            para(th, &three.shaded(ACCENT).padded(), &look, &inner)
        }
        (true, "docxtor.companyDate") => {
            let inner = [control(env, th, Field::Company, &small.caps().track(1.0)), tab(), tab(), control(env, th, Field::Date, &small)].concat();
            para(th, &three, &small, &inner)
        }
        _ => return None,
    };
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    struct Fake(i64, u32);
    impl Env for Fake {
        fn sdt_id(&mut self) -> i64 {
            self.0 += 1;
            self.0
        }
        fn drawing_id(&mut self) -> u32 {
            self.1 += 1;
            self.1
        }
        fn placeholder(&mut self, _text: &str) -> String {
            "DefaultPlaceholder".into()
        }
        fn value(&self, f: Field) -> String {
            if f == Field::Title {
                "Annual report".into()
            } else {
                String::new()
            }
        }
        fn date(&self) -> Option<String> {
            None
        }
        fn label(&self, f: Field) -> String {
            f.placeholder().into()
        }
        fn word(&self, _key: &str, english: &str) -> String {
            english.into()
        }
        fn date_format(&self) -> (String, String) {
            ("MMMM d, yyyy".into(), "en-US".into())
        }
    }

    #[test]
    fn every_design_parses_and_shows_the_title_or_its_placeholder() {
        let th = ShapeTheme::default();
        let _ = HashMap::<String, String>::new();
        for footer in [false, true] {
            for d in designs(footer) {
                let mut env = Fake(0, 0);
                let xml = build(d.id, footer, &mut env, &th, 9360, if footer { "Footer" } else { "Header" }).expect("a design");
                let els = parse(&xml).expect("valid markup");
                assert_eq!(els.len(), 1, "{} is one paragraph", d.id);
                assert!(xml.contains(r#"<w:pStyle w:val=""#), "{} has its style", d.id);
                if xml.contains("w:alias w:val=\"Title\"") {
                    assert!(xml.contains("Annual report"), "{} shows the title", d.id);
                }
            }
        }
        assert!(build("nope", false, &mut Fake(0, 0), &th, 9360, "Header").is_none());
    }
}
