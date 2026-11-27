//! Style sets (Word's Design, Style Set gallery) and paragraph spacing presets.
//!
//! A style set rewrites the definitions of Normal, Title, Subtitle, Heading 1 to 9 and Quote, and the paragraph spacing
//! of the document defaults. Word keeps its style sets in template files Docxtor does not read, so these are written to look
//! like Word's: the fonts, sizes, colours, rules and spacing follow what each set is known for, but they are Docxtor's own
//! approximations, not copies. The colours name theme colours, so a set follows the document's theme.

use crate::props::put_ordered;
use crate::shapes::theme::ShapeTheme;
use crate::style_edit::{self, STYLE_ORDER};
use crate::styles::StyleSheet;
use crate::xml::{self, Element, Node};
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

/// A style set an extension brings, or a built-in one written out: `w:style` elements and the default spacing.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StyleSetDef {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    /// One or more `w:style` elements, as XML. A style the document has keeps its name and what it builds on; its
    /// paragraph and character formatting are replaced. A style the document lacks is added.
    pub styles: String,
    /// The paragraph spacing of the document defaults.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spacing: Option<Spacing>,
}

/// Spacing between paragraphs and lines. `before` and `after` are in twips; `line` is in 240ths of a line.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct Spacing {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<i64>,
}

/// What the Style set gallery needs to list a built-in set.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StyleSetInfo {
    pub id: String,
    pub name: String,
    pub description: String,
}

/// Paragraph spacing presets (Word's Design, Paragraph Spacing): `(id, name, before, after, line in 240ths)`. Default is what
/// a new Docxtor document has: 8 pt after a paragraph and 1.15 lines.
pub const SPACING_PRESETS: &[(&str, &str, i64, i64, i64)] = &[
    ("default", "Default", 0, 160, 276),
    ("none", "No paragraph space", 0, 0, 240),
    ("compact", "Compact", 0, 80, 240),
    ("tight", "Tight", 0, 120, 276),
    ("open", "Open", 0, 200, 276),
    ("relaxed", "Relaxed", 0, 120, 360),
    ("double", "Double", 0, 160, 480),
];

pub fn spacing_preset(id: &str) -> Option<Spacing> {
    let n = id.to_lowercase().replace([' ', '-', '_'], "");
    SPACING_PRESETS
        .iter()
        .find(|p| p.0 == n || p.1.to_lowercase().replace([' ', '-', '_'], "") == n)
        .map(|p| Spacing { before: Some(p.2), after: Some(p.3), line: Some(p.4) })
}

// ---------------------------------------------------------------------------
// Writing styles
// ---------------------------------------------------------------------------

#[derive(Clone, Copy)]
enum Font {
    Major,
    Minor,
    Named(&'static str),
}

/// A colour: a theme colour (with Word's tint or shade) or a plain hex colour.
#[derive(Clone, Copy)]
enum Col {
    Theme(&'static str, Option<&'static str>, Option<&'static str>),
    Hex(&'static str),
}

const TEXT: Col = Col::Theme("text1", None, None);
const ACCENT: Col = Col::Theme("accent1", None, Some("BF"));
const GREY: Col = Col::Theme("text1", Some("A6"), None);

fn office() -> &'static ShapeTheme {
    static T: OnceLock<ShapeTheme> = OnceLock::new();
    T.get_or_init(|| ShapeTheme::from_doc(&crate::themes::new_theme_doc(&crate::themes::office_theme()).root))
}

impl Col {
    /// `w:color w:val="…" w:themeColor="…"` (the hex is the colour in the Office theme, as Word writes a fallback).
    fn attrs(&self, tag: &str) -> String {
        match self {
            Col::Hex(h) => format!(r#"w:{tag}="{h}""#),
            Col::Theme(name, tint, shade) => {
                let hex = office().word_color(name, *tint, *shade).unwrap_or_else(|| "000000".into());
                let mut a = format!(r#"w:{tag}="{hex}" w:theme{}="{name}""#, if tag == "fill" { "Fill" } else { "Color" });
                if let Some(t) = tint {
                    a.push_str(&format!(r#" w:theme{}Tint="{t}""#, if tag == "fill" { "Fill" } else { "" }));
                }
                if let Some(s) = shade {
                    a.push_str(&format!(r#" w:theme{}Shade="{s}""#, if tag == "fill" { "Fill" } else { "" }));
                }
                a
            }
        }
    }
}

/// One rule (border line) above or below a paragraph.
#[derive(Clone, Copy)]
struct Rule {
    /// Eighths of a point.
    width: u32,
    /// Points between the text and the line.
    space: u32,
    color: Col,
}

#[derive(Clone)]
struct Spec {
    id: &'static str,
    name: &'static str,
    next: Option<&'static str>,
    priority: u8,
    font: Option<Font>,
    /// Points.
    size: Option<f32>,
    bold: bool,
    italic: bool,
    caps: bool,
    small_caps: bool,
    color: Option<Col>,
    /// Character spacing in twentieths of a point.
    spacing: Option<i32>,
    before: Option<i32>,
    after: Option<i32>,
    line: Option<i32>,
    jc: Option<&'static str>,
    keep: bool,
    outline: Option<u8>,
    top: Option<Rule>,
    bottom: Option<Rule>,
    fill: Option<Col>,
    ind: Option<(i32, i32)>,
    contextual: bool,
    kern: Option<i32>,
}

impl Spec {
    fn new(id: &'static str, name: &'static str) -> Spec {
        Spec {
            id,
            name,
            next: Some("Normal"),
            priority: 9,
            font: None,
            size: None,
            bold: false,
            italic: false,
            caps: false,
            small_caps: false,
            color: None,
            spacing: None,
            before: None,
            after: None,
            line: None,
            jc: None,
            keep: false,
            outline: None,
            top: None,
            bottom: None,
            fill: None,
            ind: None,
            contextual: false,
            kern: None,
        }
    }

    fn ppr(&self) -> String {
        let mut s = String::new();
        if self.keep {
            s.push_str("<w:keepNext/><w:keepLines/>");
        }
        if self.top.is_some() || self.bottom.is_some() {
            s.push_str("<w:pBdr>");
            for (tag, r) in [("top", self.top), ("bottom", self.bottom)] {
                if let Some(r) = r {
                    s.push_str(&format!(r#"<w:{tag} w:val="single" w:sz="{}" w:space="{}" {}/>"#, r.width, r.space, r.color.attrs("color")));
                }
            }
            s.push_str("</w:pBdr>");
        }
        if let Some(f) = self.fill {
            s.push_str(&format!(r#"<w:shd w:val="clear" w:color="auto" {}/>"#, f.attrs("fill")));
        }
        if self.before.is_some() || self.after.is_some() || self.line.is_some() {
            s.push_str("<w:spacing");
            if let Some(b) = self.before {
                s.push_str(&format!(r#" w:before="{b}""#));
            }
            if let Some(a) = self.after {
                s.push_str(&format!(r#" w:after="{a}""#));
            }
            if let Some(l) = self.line {
                s.push_str(&format!(r#" w:line="{l}" w:lineRule="auto""#));
            }
            s.push_str("/>");
        }
        if let Some((l, r)) = self.ind {
            s.push_str(&format!(r#"<w:ind w:left="{l}" w:right="{r}"/>"#));
        }
        if self.contextual {
            s.push_str("<w:contextualSpacing/>");
        }
        if let Some(j) = self.jc {
            s.push_str(&format!(r#"<w:jc w:val="{j}"/>"#));
        }
        if let Some(o) = self.outline {
            s.push_str(&format!(r#"<w:outlineLvl w:val="{o}"/>"#));
        }
        s
    }

    fn rpr(&self) -> String {
        let mut s = String::new();
        match self.font {
            Some(Font::Major) => s.push_str(r#"<w:rFonts w:asciiTheme="majorHAnsi" w:eastAsiaTheme="majorEastAsia" w:hAnsiTheme="majorHAnsi" w:cstheme="majorBidi"/>"#),
            Some(Font::Minor) => s.push_str(r#"<w:rFonts w:asciiTheme="minorHAnsi" w:eastAsiaTheme="minorEastAsia" w:hAnsiTheme="minorHAnsi" w:cstheme="minorBidi"/>"#),
            Some(Font::Named(n)) => s.push_str(&format!(r#"<w:rFonts w:ascii="{n}" w:hAnsi="{n}" w:cs="{n}"/>"#)),
            None => {}
        }
        if self.bold {
            s.push_str("<w:b/><w:bCs/>");
        }
        if self.italic {
            s.push_str("<w:i/><w:iCs/>");
        }
        if self.caps {
            s.push_str("<w:caps/>");
        }
        if self.small_caps {
            s.push_str("<w:smallCaps/>");
        }
        if let Some(c) = self.color {
            s.push_str(&format!("<w:color {}/>", c.attrs("val")));
        }
        if let Some(v) = self.spacing {
            s.push_str(&format!(r#"<w:spacing w:val="{v}"/>"#));
        }
        if let Some(k) = self.kern {
            s.push_str(&format!(r#"<w:kern w:val="{k}"/>"#));
        }
        if let Some(pt) = self.size {
            let half = (pt * 2.0).round() as i32;
            s.push_str(&format!(r#"<w:sz w:val="{half}"/><w:szCs w:val="{half}"/>"#));
        }
        s
    }

    /// The style as a `w:style` element. Normal is the default paragraph style and builds on nothing.
    fn xml(&self) -> String {
        let normal = self.id == "Normal";
        let mut s = if normal {
            r#"<w:style w:type="paragraph" w:default="1" w:styleId="Normal"><w:name w:val="Normal"/><w:qFormat/>"#.to_string()
        } else {
            let next = self.next.map(|n| format!(r#"<w:next w:val="{n}"/>"#)).unwrap_or_default();
            format!(
                r#"<w:style w:type="paragraph" w:styleId="{}"><w:name w:val="{}"/><w:basedOn w:val="Normal"/>{next}<w:uiPriority w:val="{}"/><w:qFormat/>"#,
                self.id, self.name, self.priority
            )
        };
        let ppr = self.ppr();
        if !ppr.is_empty() {
            s.push_str(&format!("<w:pPr>{ppr}</w:pPr>"));
        }
        let rpr = self.rpr();
        if !rpr.is_empty() {
            s.push_str(&format!("<w:rPr>{rpr}</w:rPr>"));
        }
        s.push_str("</w:style>");
        s
    }
}

// ---------------------------------------------------------------------------
// The built-in sets
// ---------------------------------------------------------------------------

/// How a set is built: the fonts, colours and shapes that give it its character.
struct Look {
    /// Heading font.
    heading: Font,
    /// Body font and size in points, with the spacing under a paragraph (twips) and the line (240ths).
    body: Font,
    body_size: f32,
    after: i32,
    line: i32,
    /// Title: size, colour, bold, caps, alignment, and a rule under (or above and under) it.
    title_size: f32,
    title_color: Col,
    title_bold: bool,
    title_caps: bool,
    title_jc: Option<&'static str>,
    title_rule: Option<Rule>,
    title_top_rule: Option<Rule>,
    title_fill: Option<Col>,
    /// Subtitle colour, italic and alignment.
    sub_color: Col,
    sub_italic: bool,
    /// Heading 1 to 9 sizes in points, then how they look.
    sizes: [f32; 9],
    h_color: Col,
    h_alt_color: Col,
    h_bold: bool,
    h_caps: bool,
    h_small_caps: bool,
    h_italic_from: usize,
    h_jc: Option<&'static str>,
    h_before: i32,
    h_after: i32,
    h_rule: Option<Rule>,
    h_top_rule: Option<Rule>,
    h_fill: Option<Col>,
    /// Heading 1's colour when it is on a fill (white).
    h_on_fill: bool,
    quote_italic: bool,
    quote_jc: Option<&'static str>,
}

impl Default for Look {
    fn default() -> Look {
        Look {
            heading: Font::Major,
            body: Font::Minor,
            body_size: 11.0,
            after: 160,
            line: 259,
            title_size: 28.0,
            title_color: TEXT,
            title_bold: false,
            title_caps: false,
            title_jc: None,
            title_rule: None,
            title_top_rule: None,
            title_fill: None,
            sub_color: GREY,
            sub_italic: false,
            sizes: [16.0, 13.0, 12.0, 11.0, 11.0, 11.0, 11.0, 10.5, 10.5],
            h_color: ACCENT,
            h_alt_color: Col::Theme("accent1", None, Some("7F")),
            h_bold: false,
            h_caps: false,
            h_small_caps: false,
            h_italic_from: 3,
            h_jc: None,
            h_before: 240,
            h_after: 0,
            h_rule: None,
            h_top_rule: None,
            h_fill: None,
            h_on_fill: false,
            quote_italic: true,
            quote_jc: Some("center"),
        }
    }
}

impl Look {
    fn specs(&self) -> Vec<Spec> {
        let mut normal = Spec::new("Normal", "Normal");
        normal.font = Some(self.body);
        normal.size = Some(self.body_size);
        normal.after = Some(self.after);
        normal.line = Some(self.line);
        let mut title = Spec::new("Title", "Title");
        title.priority = 10;
        title.font = Some(self.heading);
        title.size = Some(self.title_size);
        title.color = Some(self.title_color);
        title.bold = self.title_bold;
        title.caps = self.title_caps;
        title.jc = self.title_jc;
        title.bottom = self.title_rule;
        title.top = self.title_top_rule;
        title.fill = self.title_fill;
        title.after = Some(if self.title_rule.is_some() { 300 } else { 80 });
        title.line = Some(240);
        title.contextual = true;
        title.spacing = Some(-10);
        title.kern = Some(28);
        let mut sub = Spec::new("Subtitle", "Subtitle");
        sub.priority = 11;
        sub.font = Some(self.heading);
        sub.size = Some((self.body_size + 1.0).max(11.0));
        sub.color = Some(self.sub_color);
        sub.italic = self.sub_italic;
        sub.jc = self.title_jc;
        sub.spacing = Some(15);
        sub.after = Some(160);
        let mut out = vec![normal, title, sub];
        for n in 1..=9usize {
            let mut h = Spec::new(["Heading1", "Heading2", "Heading3", "Heading4", "Heading5", "Heading6", "Heading7", "Heading8", "Heading9"][n - 1], ["heading 1", "heading 2", "heading 3", "heading 4", "heading 5", "heading 6", "heading 7", "heading 8", "heading 9"][n - 1]);
            h.font = Some(self.heading);
            h.size = Some(self.sizes[n - 1]);
            h.keep = true;
            h.outline = Some(n as u8 - 1);
            h.bold = self.h_bold;
            h.caps = self.h_caps;
            h.small_caps = self.h_small_caps;
            h.italic = n > self.h_italic_from && n % 2 == 0;
            h.color = Some(if n <= 2 || !self.h_on_fill && n <= 5 { self.h_color } else { self.h_alt_color });
            h.jc = self.h_jc;
            h.before = Some(if n == 1 { self.h_before } else { self.h_before / 6 * 2 });
            h.after = Some(self.h_after);
            if n == 1 {
                h.bottom = self.h_rule;
                h.top = self.h_top_rule;
                h.fill = self.h_fill;
                if self.h_on_fill {
                    h.color = Some(Col::Hex("FFFFFF"));
                }
            }
            if n >= 7 {
                h.priority = 9;
            }
            out.push(h);
        }
        let mut q = Spec::new("Quote", "Quote");
        q.priority = 29;
        q.italic = self.quote_italic;
        q.color = Some(Col::Theme("text1", Some("BF"), None));
        q.jc = self.quote_jc;
        q.ind = Some((864, 864));
        q.before = Some(200);
        q.after = Some(160);
        out.push(q);
        out
    }

    fn def(&self, id: &str, name: &str) -> StyleSetDef {
        StyleSetDef { id: id.into(), name: name.into(), styles: self.specs().iter().map(Spec::xml).collect(), spacing: Some(Spacing { before: Some(0), after: Some(self.after as i64), line: Some(self.line as i64) }) }
    }
}

const THIN: Rule = Rule { width: 4, space: 1, color: Col::Theme("accent1", None, None) };
const BAR: Rule = Rule { width: 8, space: 4, color: Col::Theme("accent1", None, None) };

/// Every built-in set, in the order of the gallery.
pub fn style_sets() -> Vec<(StyleSetInfo, StyleSetDef)> {
    let info = |id: &str, name: &str, description: &str| StyleSetInfo { id: id.into(), name: name.into(), description: description.into() };
    let mut out: Vec<(StyleSetInfo, StyleSetDef)> = Vec::new();
    let mut add = |id: &str, name: &str, description: &str, look: Look| out.push((info(id, name, description), look.def(id, name)));

    // Word 2013 and later: Calibri Light headings in dark blue, 8 pt after a paragraph.
    add("word-2013", "Word 2013+", "Light headings in dark blue and a little room after each paragraph.", Look::default());

    // Word 2010: Cambria headings, a blue rule under the title, 10 pt after a paragraph.
    add(
        "word-2010",
        "Word 2010",
        "Bold blue Cambria headings and a rule under the title.",
        Look {
            heading: Font::Named("Cambria"),
            title_size: 26.0,
            title_color: Col::Theme("text2", None, Some("BF")),
            title_rule: Some(Rule { width: 8, space: 4, color: Col::Theme("accent1", None, None) }),
            sub_color: Col::Theme("accent1", None, None),
            sub_italic: true,
            sizes: [14.0, 13.0, 11.0, 11.0, 11.0, 11.0, 11.0, 10.0, 10.0],
            h_color: Col::Theme("accent1", None, Some("BF")),
            h_alt_color: Col::Theme("accent1", None, None),
            h_bold: true,
            h_before: 480,
            after: 200,
            line: 276,
            quote_jc: None,
            ..Look::default()
        },
    );

    // Word 2003: Times New Roman text, Arial headings, single spacing and no room after a paragraph.
    add(
        "word-2003",
        "Word 2003",
        "Times New Roman text, Arial headings and single spacing.",
        Look {
            heading: Font::Named("Arial"),
            body: Font::Named("Times New Roman"),
            body_size: 12.0,
            after: 0,
            line: 240,
            title_size: 16.0,
            title_color: TEXT,
            title_bold: true,
            title_jc: Some("center"),
            sub_color: TEXT,
            sizes: [16.0, 14.0, 13.0, 14.0, 13.0, 11.0, 12.0, 12.0, 11.0],
            h_color: TEXT,
            h_alt_color: TEXT,
            h_bold: true,
            h_before: 240,
            h_after: 60,
            quote_italic: false,
            quote_jc: None,
            ..Look::default()
        },
    );

    // Basic: plain, with the title as the only thing set apart.
    add(
        "basic-elegant",
        "Basic (Elegant)",
        "Garamond with small capitals in the headings.",
        Look {
            heading: Font::Named("Garamond"),
            body: Font::Named("Garamond"),
            body_size: 12.0,
            title_size: 30.0,
            title_color: TEXT,
            title_jc: Some("center"),
            title_rule: Some(Rule { width: 4, space: 4, color: TEXT }),
            sub_color: GREY,
            sub_italic: true,
            sizes: [18.0, 15.0, 13.0, 12.0, 12.0, 12.0, 12.0, 12.0, 12.0],
            h_color: TEXT,
            h_alt_color: TEXT,
            h_small_caps: true,
            h_before: 360,
            h_after: 80,
            ..Look::default()
        },
    );
    add(
        "basic-simple",
        "Basic (Simple)",
        "Calibri throughout, headings in bold black.",
        Look {
            heading: Font::Minor,
            title_size: 26.0,
            title_color: TEXT,
            title_bold: true,
            sub_color: GREY,
            sizes: [16.0, 14.0, 12.0, 11.0, 11.0, 11.0, 11.0, 11.0, 11.0],
            h_color: TEXT,
            h_alt_color: TEXT,
            h_bold: true,
            h_before: 240,
            h_after: 60,
            quote_jc: None,
            ..Look::default()
        },
    );
    add(
        "basic-stylish",
        "Basic (Stylish)",
        "A centred title between two rules and spaced capitals in the headings.",
        Look {
            title_size: 28.0,
            title_color: ACCENT,
            title_jc: Some("center"),
            title_caps: true,
            title_rule: Some(THIN),
            title_top_rule: Some(THIN),
            sub_color: GREY,
            sizes: [15.0, 13.0, 12.0, 11.0, 11.0, 11.0, 11.0, 11.0, 11.0],
            h_caps: true,
            h_before: 300,
            h_after: 80,
            ..Look::default()
        },
    );

    // Black and white: no colour.
    add(
        "bw-classic",
        "Black & White (Classic)",
        "Times New Roman, everything in black.",
        Look {
            heading: Font::Named("Times New Roman"),
            body: Font::Named("Times New Roman"),
            body_size: 12.0,
            title_size: 24.0,
            title_color: TEXT,
            title_bold: true,
            title_jc: Some("center"),
            sub_color: TEXT,
            sub_italic: true,
            sizes: [16.0, 14.0, 13.0, 12.0, 12.0, 12.0, 12.0, 12.0, 12.0],
            h_color: TEXT,
            h_alt_color: TEXT,
            h_bold: true,
            h_before: 240,
            h_after: 60,
            quote_jc: None,
            ..Look::default()
        },
    );
    add(
        "bw-modern",
        "Black & White (Modern)",
        "Sans-serif headings in black with a heavy rule under the title.",
        Look {
            title_size: 30.0,
            title_color: TEXT,
            title_bold: true,
            title_rule: Some(Rule { width: 18, space: 4, color: TEXT }),
            sub_color: TEXT,
            sizes: [16.0, 13.0, 12.0, 11.0, 11.0, 11.0, 11.0, 11.0, 11.0],
            h_color: TEXT,
            h_alt_color: TEXT,
            h_bold: true,
            h_caps: true,
            quote_jc: None,
            ..Look::default()
        },
    );
    add(
        "bw-word-2003",
        "Black & White (Word 2003)",
        "Word 2003 headings in black.",
        Look {
            heading: Font::Named("Arial"),
            body: Font::Named("Times New Roman"),
            body_size: 12.0,
            after: 0,
            line: 240,
            title_size: 16.0,
            title_color: TEXT,
            title_bold: true,
            title_jc: Some("center"),
            sub_color: TEXT,
            sizes: [16.0, 14.0, 13.0, 14.0, 13.0, 11.0, 12.0, 12.0, 11.0],
            h_color: TEXT,
            h_alt_color: TEXT,
            h_bold: true,
            h_before: 240,
            h_after: 60,
            quote_italic: false,
            quote_jc: None,
            ..Look::default()
        },
    );

    add(
        "casual",
        "Casual",
        "Rounded, friendly type in warm colours.",
        Look {
            heading: Font::Named("Segoe Print"),
            body: Font::Named("Segoe UI"),
            body_size: 11.0,
            title_size: 28.0,
            title_color: Col::Theme("accent2", None, None),
            sub_color: Col::Theme("accent2", None, Some("BF")),
            sizes: [16.0, 14.0, 12.0, 11.0, 11.0, 11.0, 11.0, 11.0, 11.0],
            h_color: Col::Theme("accent2", None, Some("BF")),
            h_alt_color: Col::Theme("accent2", None, Some("7F")),
            h_before: 280,
            h_after: 60,
            ..Look::default()
        },
    );
    add(
        "centered",
        "Centered",
        "The title and every heading centred.",
        Look {
            title_jc: Some("center"),
            title_size: 28.0,
            title_color: ACCENT,
            sub_color: GREY,
            h_jc: Some("center"),
            sizes: [16.0, 13.0, 12.0, 11.0, 11.0, 11.0, 11.0, 11.0, 11.0],
            ..Look::default()
        },
    );

    add(
        "lines-distinctive",
        "Lines (Distinctive)",
        "A heavy rule above and below the title and a rule under each main heading.",
        Look {
            title_size: 28.0,
            title_color: TEXT,
            title_caps: true,
            title_jc: Some("center"),
            title_rule: Some(BAR),
            title_top_rule: Some(BAR),
            sub_color: GREY,
            sizes: [16.0, 13.0, 12.0, 11.0, 11.0, 11.0, 11.0, 11.0, 11.0],
            h_color: ACCENT,
            h_rule: Some(THIN),
            h_before: 300,
            h_after: 80,
            ..Look::default()
        },
    );
    add(
        "lines-simple",
        "Lines (Simple)",
        "A single rule under the title and under each main heading.",
        Look {
            title_size: 26.0,
            title_color: TEXT,
            title_rule: Some(Rule { width: 4, space: 4, color: TEXT }),
            sub_color: GREY,
            sizes: [15.0, 13.0, 12.0, 11.0, 11.0, 11.0, 11.0, 11.0, 11.0],
            h_color: TEXT,
            h_alt_color: TEXT,
            h_rule: Some(Rule { width: 4, space: 1, color: Col::Theme("text1", Some("A6"), None) }),
            h_before: 280,
            h_after: 80,
            quote_jc: None,
            ..Look::default()
        },
    );
    add(
        "lines-stylish",
        "Lines (Stylish)",
        "Capitals and a rule above and below the title and above each main heading.",
        Look {
            title_size: 28.0,
            title_color: ACCENT,
            title_caps: true,
            title_jc: Some("center"),
            title_rule: Some(THIN),
            title_top_rule: Some(THIN),
            sub_color: GREY,
            sizes: [14.0, 12.0, 11.0, 11.0, 11.0, 11.0, 11.0, 11.0, 11.0],
            h_caps: true,
            h_color: ACCENT,
            h_top_rule: Some(THIN),
            h_before: 320,
            h_after: 80,
            ..Look::default()
        },
    );

    add(
        "minimalist",
        "Minimalist",
        "Light type, plenty of room and no colour but grey.",
        Look {
            title_size: 32.0,
            title_color: Col::Theme("text1", Some("BF"), None),
            sub_color: GREY,
            sizes: [18.0, 14.0, 12.0, 11.0, 11.0, 11.0, 11.0, 11.0, 11.0],
            h_color: Col::Theme("text1", Some("BF"), None),
            h_alt_color: GREY,
            h_before: 360,
            h_after: 120,
            after: 200,
            line: 288,
            quote_jc: None,
            ..Look::default()
        },
    );
    add(
        "shaded",
        "Shaded",
        "Main headings and the title in white on a band of the accent colour.",
        Look {
            title_size: 28.0,
            title_color: Col::Hex("FFFFFF"),
            title_fill: Some(Col::Theme("accent1", None, Some("BF"))),
            sub_color: GREY,
            sizes: [15.0, 13.0, 12.0, 11.0, 11.0, 11.0, 11.0, 11.0, 11.0],
            h_color: ACCENT,
            h_fill: Some(Col::Theme("accent1", None, Some("BF"))),
            h_on_fill: true,
            h_before: 300,
            h_after: 80,
            ..Look::default()
        },
    );
    out
}

pub fn style_set(id: &str) -> Option<StyleSetDef> {
    let n = id.to_lowercase().replace([' ', '-', '_'], "");
    style_sets().into_iter().find(|(i, _)| i.id.replace('-', "") == n || i.name.to_lowercase().replace([' ', '-', '_'], "") == n).map(|(_, d)| d)
}

pub fn style_set_infos() -> Vec<StyleSetInfo> {
    style_sets().into_iter().map(|(i, _)| i).collect()
}

/// Replace a style's paragraph and character formatting with another's, keeping its name and the rest.
fn replace_formatting(target: &mut Element, source: &Element) {
    for name in ["w:pPr", "w:rPr"] {
        target.remove_children(name);
        if let Some(e) = source.child(name) {
            put_ordered(target, e.clone(), STYLE_ORDER);
        }
    }
    for flag in ["w:next", "w:uiPriority"] {
        if let Some(v) = source.child(flag) {
            put_ordered(target, v.clone(), STYLE_ORDER);
        }
    }
    if source.child("w:qFormat").is_some() {
        style_edit::set_flag(target, "w:qFormat", true);
    }
}

impl StyleSheet {
    /// Apply a style set: the styles it defines are written again (added where the sheet lacks them) and the spacing
    /// of the document defaults follows.
    pub fn apply_style_set(&mut self, def: &StyleSetDef) -> Result<(), String> {
        let parsed = xml::parse(format!(r#"<x xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">{}</x>"#, def.styles).as_bytes()).map_err(|e| format!("the style set is not valid XML: {e}"))?;
        let incoming: Vec<Element> = parsed.root.elements().filter(|e| e.name == "w:style").cloned().collect();
        if incoming.is_empty() {
            return Err("the style set has no styles".into());
        }
        if incoming.iter().any(|s| s.attr("w:styleId").is_none()) {
            return Err("every style in a style set needs a styleId".into());
        }
        // What the new styles build on has to exist.
        for s in &incoming {
            for dep in ["w:basedOn", "w:next"] {
                if let Some(target) = s.child(dep).and_then(|e| e.attr("w:val")) {
                    if !self.exists(target) && !incoming.iter().any(|i| i.attr("w:styleId") == Some(target)) {
                        self.ensure_builtin(target);
                    }
                }
            }
        }
        let missing: Vec<&Element> = incoming.iter().filter(|s| !self.exists(s.attr("w:styleId").unwrap_or_default())).collect();
        if !missing.is_empty() {
            let xml_src: String = missing.iter().map(|e| e.to_xml()).collect();
            self.insert_style_xml("", &xml_src);
        }
        self.edit_xml(|root| {
            for s in &incoming {
                let id = s.attr("w:styleId").unwrap_or_default();
                let Some(target) = root.elements_mut().find(|e| e.name == "w:style" && e.attr("w:styleId") == Some(id)) else { continue };
                replace_formatting(target, s);
                // The character style that goes with a linked paragraph style looks the same.
                if let Some(link) = target.child("w:link").and_then(|l| l.attr("w:val")).map(String::from) {
                    if let Some(partner) = root.elements_mut().find(|e| e.name == "w:style" && e.attr("w:styleId") == Some(link.as_str()) && e.attr("w:type") == Some("character")) {
                        partner.remove_children("w:rPr");
                        if let Some(r) = s.child("w:rPr") {
                            if partner.child("w:uiPriority").is_none() {
                                style_edit::set_val(partner, "w:uiPriority", Some("1"));
                            }
                            put_ordered(partner, r.clone(), STYLE_ORDER);
                        }
                    }
                }
            }
        });
        if let Some(sp) = def.spacing {
            self.set_default_spacing(&sp);
        }
        Ok(())
    }

    /// Write paragraph spacing into the document defaults, and take the spacing out of Normal so the defaults apply.
    pub fn set_default_spacing(&mut self, sp: &Spacing) {
        self.edit_xml(|root| {
            if root.child("w:docDefaults").is_none() {
                root.children.insert(0, Node::Element(Element::new("w:docDefaults")));
            }
            let dd = root.child_mut("w:docDefaults").unwrap();
            if dd.child("w:pPrDefault").is_none() {
                dd.children.push(Node::Element(Element::new("w:pPrDefault").with_child(Element::new("w:pPr"))));
            }
            let ppr = dd.child_mut("w:pPrDefault").unwrap();
            if ppr.child("w:pPr").is_none() {
                ppr.children.push(Node::Element(Element::new("w:pPr")));
            }
            let ppr = ppr.child_mut("w:pPr").unwrap();
            let mut el = ppr.child("w:spacing").cloned().unwrap_or_else(|| Element::new("w:spacing"));
            if let Some(b) = sp.before {
                el.set_attr("w:before", b.to_string());
            }
            if let Some(a) = sp.after {
                el.set_attr("w:after", a.to_string());
            }
            if let Some(l) = sp.line {
                el.set_attr("w:line", l.to_string());
                el.set_attr("w:lineRule", "auto");
            }
            crate::props::put_ppr(ppr, el);
            // Normal says nothing of its own about spacing, so the defaults show.
            if let Some(normal) = root.elements_mut().find(|e| e.name == "w:style" && e.attr("w:styleId") == Some("Normal")) {
                if let Some(ppr) = normal.child_mut("w:pPr") {
                    if let Some(s) = ppr.child_mut("w:spacing") {
                        for a in ["w:before", "w:after", "w:line", "w:lineRule", "w:beforeLines", "w:afterLines", "w:beforeAutospacing", "w:afterAutospacing"] {
                            let given = (a == "w:before" && sp.before.is_some()) || (a == "w:after" && sp.after.is_some()) || ((a == "w:line" || a == "w:lineRule") && sp.line.is_some());
                            if given || a.ends_with("Lines") || a.ends_with("Autospacing") {
                                s.remove_attr(a);
                            }
                        }
                        if s.attrs.is_empty() {
                            ppr.remove_children("w:spacing");
                        }
                    }
                }
                style_edit::drop_if_empty(normal, "w:pPr");
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::xml;

    #[test]
    fn every_set_is_well_formed_and_covers_the_styles() {
        for (info, def) in style_sets() {
            let doc = xml::parse(format!(r#"<w:styles xmlns:w="w">{}</w:styles>"#, def.styles).as_bytes()).unwrap_or_else(|e| panic!("{}: {e}", info.name));
            let ids: Vec<_> = doc.root.elements().filter_map(|s| s.attr("w:styleId")).collect();
            for want in ["Normal", "Title", "Subtitle", "Heading1", "Heading9", "Quote"] {
                assert!(ids.contains(&want), "{} lacks {want}", info.name);
            }
            assert_eq!(ids.len(), 13);
            assert!(def.spacing.is_some());
        }
    }

    #[test]
    fn spacing_presets_are_found_by_name() {
        assert_eq!(spacing_preset("No paragraph space").unwrap().after, Some(0));
        assert_eq!(spacing_preset("double").unwrap().line, Some(480));
        assert!(spacing_preset("huge").is_none());
    }
}
