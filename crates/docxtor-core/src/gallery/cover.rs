//! Docxtor's own cover page designs. Each one is built for the page it goes on (the first section's paper and margins),
//! from the theme's colours and fonts, and is written as Word writes a cover page from its gallery: a block-level
//! content control (`w:docPartObj`, gallery `Cover Pages`) whose boxes for title, subtitle, author, date and the rest are
//! content controls bound to the document's properties, with the shapes behind the text in the first paragraph and a
//! page break (or a section break) at the end.

use super::xml::*;
use crate::shapes::theme::ShapeTheme;

/// A built-in design: its id, the name the gallery shows (English), and whether it ends with a section break.
pub struct Design {
    pub id: &'static str,
    pub name: &'static str,
    /// The cover is a section of its own (the design centres its text on the page, say). Otherwise it ends with a
    /// page break and the first section shows a different first page.
    pub section: bool,
}

pub const DESIGNS: &[Design] = &[
    Design { id: "docxtor.simple", name: "Simple", section: false },
    Design { id: "docxtor.band", name: "Band", section: false },
    Design { id: "docxtor.sidebar", name: "Side bar", section: false },
    Design { id: "docxtor.frame", name: "Frame", section: false },
    Design { id: "docxtor.academic", name: "Academic", section: false },
    Design { id: "docxtor.report", name: "Report", section: false },
    Design { id: "docxtor.blocks", name: "Blocks", section: false },
    Design { id: "docxtor.centered", name: "Centered", section: true },
    Design { id: "docxtor.abstract", name: "Abstract", section: false },
];

pub fn design(id: &str) -> Option<&'static Design> {
    DESIGNS.iter().find(|d| d.id == id)
}

/// The paper and margins of the page the cover goes on, in twips.
#[derive(Debug, Clone, Copy)]
pub struct Geom {
    pub w: i64,
    pub h: i64,
    pub ml: i64,
    pub mr: i64,
    pub mt: i64,
    pub mb: i64,
}

impl Geom {
    pub fn text_w(&self) -> i64 {
        (self.w - self.ml - self.mr).max(1440)
    }
}

/// A cover page built: the content control as markup, with `SECTION_MARK` where a section break's properties go.
pub struct Built {
    pub xml: String,
    /// The cover ends with a section break: the properties it needs besides the first section's.
    pub section: Option<SectionTweak>,
}

pub const SECTION_MARK: &str = "@@SECTION@@";

#[derive(Debug, Clone, Default)]
pub struct SectionTweak {
    pub v_align: Option<&'static str>,
}

struct Page<'a> {
    env: &'a mut dyn Env,
    th: &'a ShapeTheme,
    g: Geom,
    /// Where the next paragraph starts, in twips from the top of the page (what the flow has used so far).
    y: i64,
    shapes: String,
    z: u32,
    paras: Vec<String>,
}

const DARK: Clr = Clr::dark2(Lum::Plain);
const MUTED: Clr = Clr::text(Lum::Lighter(35));
const ACCENT: Clr = Clr::accent(1, Lum::Plain);
const ACCENT_DEEP: Clr = Clr::accent(1, Lum::Darker(25));
const ACCENT2: Clr = Clr::accent(2, Lum::Plain);
const WHITE: Clr = Clr::WHITE;

impl<'a> Page<'a> {
    fn new(env: &'a mut dyn Env, th: &'a ShapeTheme, g: Geom) -> Self {
        let y = g.mt;
        Page { env, th, g, y, shapes: String::new(), z: 0, paras: Vec::new() }
    }

    fn pct_w(&self, f: f64) -> i64 {
        (self.g.w as f64 * f).round() as i64
    }
    fn pct_h(&self, f: f64) -> i64 {
        (self.g.h as f64 * f).round() as i64
    }

    fn rect(&mut self, x: i64, y: i64, w: i64, h: i64, fill: Clr) {
        self.z += 1;
        let s = shape(self.env, self.th, "rect", "Rectangle", x, y, w, h, Some(fill), None, self.z);
        self.shapes.push_str(&s);
    }

    fn outline(&mut self, x: i64, y: i64, w: i64, h: i64, color: Clr, pt: f64) {
        self.z += 1;
        let s = shape(self.env, self.th, "rect", "Rectangle", x, y, w, h, None, Some((color, pt)), self.z);
        self.shapes.push_str(&s);
    }

    /// The space before a paragraph that puts its top `target` twips from the top of the page.
    fn to(&self, target: i64) -> i64 {
        (target - self.y).max(0)
    }

    fn advance(&mut self, before: i64, look: &Look, lines: i64, rule: Option<&Rule>) {
        self.y += before + look.line() * lines;
        if let Some(r) = rule {
            self.y += (r.space as i64 + (r.width as i64 + 7) / 8) * 20;
        }
    }

    /// A paragraph holding a content control.
    fn field(&mut self, f: Field, look: Look, p: Para) {
        let c = control(self.env, self.th, f, &look);
        let xml = para(self.th, &p, &look, &c);
        self.advance(p.before, &look, 1, p.bottom_rule.as_ref());
        self.paras.push(xml);
    }

    /// A paragraph of plain words.
    fn words(&mut self, text: &str, look: Look, p: Para) {
        let xml = para(self.th, &p, &look, &run(self.th, &look, text));
        self.advance(p.before, &look, 1, p.bottom_rule.as_ref());
        self.paras.push(xml);
    }

    /// An empty paragraph: a rule when the paragraph has a border.
    fn gap(&mut self, look: Look, p: Para) {
        let xml = empty_para(self.th, &p, &look);
        self.advance(p.before, &look, 1, p.bottom_rule.as_ref());
        self.paras.push(xml);
    }

    fn label(&mut self, key: &str, english: &str, look: Look, p: Para) {
        let text = self.env.word(key, english);
        self.words(&text, look, p);
    }

    /// The paragraphs in a content control of the Cover Pages gallery, ending as the design asks.
    fn finish(mut self, section: Option<SectionTweak>) -> Built {
        let mut body = String::new();
        if let Some(first) = self.paras.first_mut() {
            // The shapes ride in the first paragraph, after its properties, as in Word's own covers.
            if let Some(i) = first.find("</w:pPr>") {
                first.insert_str(i + "</w:pPr>".len(), &self.shapes);
            }
        }
        for p in &self.paras {
            body.push_str(p);
        }
        if section.is_some() {
            // The last paragraph ends the section: its properties go in the paragraph's own.
            if let Some(last) = self.paras.last().cloned() {
                let ended = last.replacen("</w:pPr>", &format!("{SECTION_MARK}</w:pPr>"), 1);
                body = body.replacen(&last, &ended, 1);
            }
        } else {
            body.push_str(&page_break());
        }
        Built { xml: gallery_sdt(self.env, "Cover Pages", true, &body), section }
    }
}

/// Build a design for a page. None when the id is not one of Docxtor's.
pub fn build(id: &str, env: &mut dyn Env, th: &ShapeTheme, g: Geom) -> Option<Built> {
    let d = design(id)?;
    let mut p = Page::new(env, th, g);
    let section = match d.id {
        "docxtor.simple" => simple(&mut p),
        "docxtor.band" => band(&mut p),
        "docxtor.sidebar" => sidebar(&mut p),
        "docxtor.frame" => frame(&mut p),
        "docxtor.academic" => academic(&mut p),
        "docxtor.report" => report(&mut p),
        "docxtor.blocks" => blocks(&mut p),
        "docxtor.centered" => centered(&mut p),
        "docxtor.abstract" => abstract_page(&mut p),
        _ => return None,
    };
    Some(p.finish(section))
}

const RULE: Rule = Rule { width: 18, color: ACCENT, style: "single", space: 1 };

fn simple(p: &mut Page) -> Option<SectionTweak> {
    let b = p.g.text_w();
    let top = p.pct_h(0.30);
    p.field(Field::Title, Look::new(44.0, DARK).heading().bold(), Para::new("center").before(p.to(top)));
    p.gap(Look::new(6.0, DARK), Para::new("center").before(300).indent(b * 36 / 100, b * 36 / 100).bottom(RULE));
    p.field(Field::Subtitle, Look::new(20.0, MUTED), Para::new("center").before(360));
    let low = p.pct_h(0.74);
    p.field(Field::Author, Look::new(15.0, DARK).bold(), Para::new("center").before(p.to(low)));
    p.field(Field::Date, Look::new(12.0, MUTED), Para::new("center").before(120));
    None
}

fn band(p: &mut Page) -> Option<SectionTweak> {
    let (w, top, h) = (p.g.w, p.pct_h(0.27), p.pct_h(0.23));
    p.rect(0, top, w, h, ACCENT);
    p.rect(0, top + h, w, p.pct_h(0.012), ACCENT2);
    p.field(Field::Title, Look::new(40.0, WHITE).heading().bold(), Para::new("left").before(p.to(top + p.pct_h(0.035))));
    p.field(Field::Subtitle, Look::new(18.0, WHITE), Para::new("left").before(160));
    let low = top + h + p.pct_h(0.09);
    p.field(Field::Author, Look::new(16.0, DARK).bold(), Para::new("left").before(p.to(low)));
    p.field(Field::Date, Look::new(12.0, MUTED), Para::new("left").before(120));
    let foot = p.pct_h(0.88);
    p.field(Field::Company, Look::new(12.0, ACCENT_DEEP).caps().bold().track(1.5), Para::new("left").before(p.to(foot)));
    None
}

fn sidebar(p: &mut Page) -> Option<SectionTweak> {
    let (bar, h) = (p.pct_w(0.17), p.g.h);
    p.rect(0, 0, bar, h, ACCENT);
    p.rect(bar, 0, p.pct_w(0.012), h, ACCENT2);
    let left = (bar + p.pct_w(0.07) - p.g.ml).max(0);
    let top = p.pct_h(0.26);
    p.field(Field::Title, Look::new(44.0, DARK).heading().bold(), Para::new("left").before(p.to(top)).indent(left, 0));
    p.field(Field::Subtitle, Look::new(20.0, ACCENT_DEEP), Para::new("left").before(240).indent(left, 0));
    let low = p.pct_h(0.76);
    let rule = Rule { width: 12, color: ACCENT2, style: "single", space: 6 };
    p.field(Field::Author, Look::new(15.0, DARK).bold(), Para::new("left").before(p.to(low)).indent(left, 0).top(rule));
    p.field(Field::Date, Look::new(12.0, MUTED), Para::new("left").before(100).indent(left, 0));
    p.field(Field::Company, Look::new(12.0, MUTED).caps().track(1.5), Para::new("left").before(100).indent(left, 0));
    None
}

fn frame(p: &mut Page) -> Option<SectionTweak> {
    let b = p.g.text_w();
    let inset = 720;
    let (w, h) = (p.g.w - 2 * inset, p.g.h - 2 * inset);
    p.outline(inset, inset, w, h, ACCENT, 3.0);
    p.outline(inset + 150, inset + 150, w - 300, h - 300, ACCENT, 0.75);
    let top = p.pct_h(0.30);
    p.field(Field::Title, Look::new(38.0, DARK).heading().bold(), Para::new("center").before(p.to(top)));
    let double = Rule { width: 6, color: ACCENT, style: "double", space: 1 };
    p.gap(Look::new(6.0, DARK), Para::new("center").before(280).indent(b * 40 / 100, b * 40 / 100).bottom(double));
    p.field(Field::Subtitle, Look::new(18.0, MUTED).italic(), Para::new("center").before(320));
    let low = p.pct_h(0.66);
    p.field(Field::Author, Look::new(15.0, DARK).bold(), Para::new("center").before(p.to(low)));
    p.field(Field::Date, Look::new(12.0, MUTED), Para::new("center").before(120));
    p.field(Field::Company, Look::new(12.0, ACCENT_DEEP).caps().bold().track(1.5), Para::new("center").before(p.to(p.pct_h(0.82))));
    None
}

fn academic(p: &mut Page) -> Option<SectionTweak> {
    let b = p.g.text_w();
    let rule = Rule { width: 12, color: ACCENT, style: "single", space: 8 };
    p.field(Field::Company, Look::new(16.0, DARK).caps().bold().track(2.0), Para::new("center").before(0).bottom(rule));
    let top = p.pct_h(0.27);
    p.field(Field::Title, Look::new(30.0, DARK).heading().bold(), Para::new("center").before(p.to(top)));
    p.field(Field::Subtitle, Look::new(16.0, MUTED).italic(), Para::new("center").before(240));
    let small = Look::new(10.0, MUTED).caps().track(1.5);
    let mid = p.pct_h(0.52);
    p.label("submitted_by", "Submitted by", small, Para::new("center").before(p.to(mid)));
    p.field(Field::Author, Look::new(16.0, DARK).bold(), Para::new("center").before(60));
    p.label("course_label", "Course", small, Para::new("center").before(360));
    p.field(Field::Course, Look::new(14.0, DARK), Para::new("center").before(60));
    p.label("supervisor_label", "Supervisor", small, Para::new("center").before(360));
    p.field(Field::Supervisor, Look::new(14.0, DARK), Para::new("center").before(60));
    let _ = b;
    p.field(Field::Date, Look::new(12.0, MUTED), Para::new("center").before(p.to(p.pct_h(0.86))));
    None
}

fn report(p: &mut Page) -> Option<SectionTweak> {
    let (w, h) = (p.g.w, p.g.h);
    let top = p.pct_h(0.46);
    p.rect(0, top, w, h - top, ACCENT);
    p.rect(p.pct_w(0.60), top - p.pct_h(0.06), w - p.pct_w(0.60), p.pct_h(0.06), ACCENT_DEEP);
    p.rect(p.pct_w(0.84), 0, w - p.pct_w(0.84), p.pct_h(0.10), ACCENT2);
    p.field(Field::Company, Look::new(13.0, DARK).caps().bold().track(2.0), Para::new("left").before(0));
    p.field(Field::Date, Look::new(12.0, MUTED), Para::new("left").before(100));
    p.field(Field::Title, Look::new(46.0, WHITE).heading().bold(), Para::new("left").before(p.to(top + p.pct_h(0.045))));
    p.field(Field::Subtitle, Look::new(20.0, WHITE), Para::new("left").before(200));
    let rule = Rule { width: 8, color: WHITE, style: "single", space: 6 };
    p.field(Field::Author, Look::new(16.0, WHITE).bold(), Para::new("left").before(p.to(p.pct_h(0.86))).top(rule));
    None
}

fn blocks(p: &mut Page) -> Option<SectionTweak> {
    let (w, b) = (p.g.w, p.g.text_w());
    p.rect(p.pct_w(0.50), 0, w - p.pct_w(0.50), p.pct_h(0.33), ACCENT);
    p.rect(p.pct_w(0.33), p.pct_h(0.20), p.pct_w(0.28), p.pct_h(0.19), ACCENT2);
    p.rect(p.pct_w(0.61), p.pct_h(0.33), w - p.pct_w(0.61), p.pct_h(0.06), ACCENT_DEEP);
    let top = p.pct_h(0.50);
    p.field(Field::Title, Look::new(50.0, DARK).heading().bold(), Para::new("left").before(p.to(top)).indent(0, b * 12 / 100));
    p.gap(Look::new(6.0, DARK), Para::new("left").before(260).indent(0, b * 76 / 100).bottom(Rule { width: 24, color: ACCENT, style: "single", space: 1 }));
    p.field(Field::Subtitle, Look::new(20.0, MUTED), Para::new("left").before(300));
    p.field(Field::Author, Look::new(15.0, DARK).bold(), Para::new("left").before(p.to(p.pct_h(0.84))));
    p.field(Field::Date, Look::new(12.0, MUTED), Para::new("left").before(100));
    None
}

fn centered(p: &mut Page) -> Option<SectionTweak> {
    let b = p.g.text_w();
    let double = Rule { width: 6, color: ACCENT, style: "double", space: 1 };
    p.gap(Look::new(6.0, DARK), Para::new("center").before(0).indent(b * 30 / 100, b * 30 / 100).bottom(double));
    p.field(Field::Title, Look::new(40.0, DARK).heading().bold(), Para::new("center").before(360));
    p.field(Field::Subtitle, Look::new(18.0, MUTED), Para::new("center").before(240));
    p.gap(Look::new(6.0, DARK), Para::new("center").before(360).indent(b * 30 / 100, b * 30 / 100).bottom(double));
    p.field(Field::Author, Look::new(15.0, DARK).bold(), Para::new("center").before(600));
    p.field(Field::Date, Look::new(12.0, MUTED), Para::new("center").before(120));
    Some(SectionTweak { v_align: Some("center") })
}

fn abstract_page(p: &mut Page) -> Option<SectionTweak> {
    let (w, h) = (p.g.w, p.g.h);
    p.rect(0, 0, w, p.pct_h(0.05), ACCENT);
    p.rect(0, p.pct_h(0.05), w, p.pct_h(0.012), ACCENT2);
    p.rect(0, h - p.pct_h(0.062), w, p.pct_h(0.012), ACCENT2);
    p.rect(0, h - p.pct_h(0.05), w, p.pct_h(0.05), ACCENT);
    let top = p.pct_h(0.16);
    p.field(Field::Title, Look::new(40.0, DARK).heading().bold(), Para::new("left").before(p.to(top)));
    p.field(Field::Subtitle, Look::new(18.0, ACCENT_DEEP), Para::new("left").before(200));
    p.field(Field::Author, Look::new(14.0, DARK).bold(), Para::new("left").before(360));
    p.field(Field::Date, Look::new(12.0, MUTED), Para::new("left").before(80));
    let label_at = p.pct_h(0.50);
    p.label("abstract_label", "Abstract", Look::new(11.0, ACCENT_DEEP).caps().bold().track(2.0), Para::new("left").before(p.to(label_at)));
    let bar = Rule { width: 24, color: ACCENT2, style: "single", space: 10 };
    p.field(Field::Abstract, Look::new(11.5, DARK), Para::new("both").before(160).indent(240, 0).left_bar(bar).line(276));
    p.field(Field::Company, Look::new(12.0, MUTED).caps().track(1.5), Para::new("left").before(p.to(p.pct_h(0.86))));
    None
}
