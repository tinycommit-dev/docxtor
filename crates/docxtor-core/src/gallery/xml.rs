//! The markup the page galleries write: runs, paragraphs, content controls bound to document properties, fields and
//! the shapes behind a cover page. Colours and fonts always name the theme, so Design, Themes restyles them; the hex
//! value next to a theme colour is what Word writes beside it (it is the theme's colour as it is now).

use crate::shapes::theme::ShapeTheme;
use std::collections::HashMap;

/// The store of the cover page properties (`customXml`), as Word names it.
pub const COVER_STORE: &str = "{55AF091B-3C7A-41E3-B477-F2FDAA23CFDA}";
pub const COVER_NS: &str = "http://schemas.microsoft.com/office/2006/coverPageProps";
const COVER_PREFIXES: &str = "xmlns:ns0='http://schemas.microsoft.com/office/2006/coverPageProps' ";

/// How much lighter or darker than the theme's colour (Word's "Lighter 40%", "Darker 25%").
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Lum {
    Plain,
    Darker(u32),
    Lighter(u32),
}

/// A colour of the theme.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Clr {
    /// DrawingML name: `accent1`, `tx1`, `tx2`, `bg1`, `bg2`.
    pub scheme: &'static str,
    /// WordprocessingML name: `accent1`, `text1`, `text2`, `background1`, `background2`.
    pub word: &'static str,
    pub lum: Lum,
}

impl Clr {
    pub const fn new(scheme: &'static str, word: &'static str, lum: Lum) -> Clr {
        Clr { scheme, word, lum }
    }
    pub const fn accent(n: u8, lum: Lum) -> Clr {
        let (scheme, word) = match n {
            1 => ("accent1", "accent1"),
            2 => ("accent2", "accent2"),
            3 => ("accent3", "accent3"),
            4 => ("accent4", "accent4"),
            5 => ("accent5", "accent5"),
            _ => ("accent6", "accent6"),
        };
        Clr { scheme, word, lum }
    }
    pub const fn text(lum: Lum) -> Clr {
        Clr { scheme: "tx1", word: "text1", lum }
    }
    pub const fn dark2(lum: Lum) -> Clr {
        Clr { scheme: "tx2", word: "text2", lum }
    }
    pub const fn light2(lum: Lum) -> Clr {
        Clr { scheme: "bg2", word: "background2", lum }
    }
    pub const WHITE: Clr = Clr { scheme: "bg1", word: "background1", lum: Lum::Plain };

    fn tint_shade(&self) -> (Option<String>, Option<String>) {
        let hex = |pct: u32| format!("{:02X}", (255 * (100 - pct.min(100)) + 50) / 100);
        match self.lum {
            Lum::Plain => (None, None),
            Lum::Darker(p) => (None, Some(hex(p))),
            Lum::Lighter(p) => (Some(hex(p)), None),
        }
    }

    /// The colour as the theme has it now, six hex digits.
    pub fn hex(&self, th: &ShapeTheme) -> String {
        let (tint, shade) = self.tint_shade();
        th.word_color(self.word, tint.as_deref(), shade.as_deref()).unwrap_or_else(|| "808080".into())
    }

    /// The attributes of a WordprocessingML colour: `w:val="4472C4" w:themeColor="accent1" w:themeShade="BF"`, under the
    /// attribute names `val`, `themeColor`... of the element they go on (`w:color`, `w:top`, `w:shd` use `w:fill`).
    pub fn attrs(&self, th: &ShapeTheme, val: &str, theme_color: &str, theme_tint: &str, theme_shade: &str) -> String {
        let (tint, shade) = self.tint_shade();
        let mut s = format!(r#"{val}="{}" {theme_color}="{}""#, self.hex(th), self.word);
        if let Some(t) = tint {
            s.push_str(&format!(r#" {theme_tint}="{t}""#));
        }
        if let Some(t) = shade {
            s.push_str(&format!(r#" {theme_shade}="{t}""#));
        }
        s
    }

    /// `<a:solidFill>` for a shape.
    pub fn fill(&self) -> String {
        let mods = match self.lum {
            Lum::Plain => String::new(),
            Lum::Darker(p) => format!(r#"<a:lumMod val="{}"/>"#, (100 - p.min(100)) * 1000),
            Lum::Lighter(p) => format!(r#"<a:lumMod val="{}"/><a:lumOff val="{}"/>"#, (100 - p.min(100)) * 1000, p.min(100) * 1000),
        };
        format!(r#"<a:solidFill><a:schemeClr val="{}">{mods}</a:schemeClr></a:solidFill>"#, self.scheme)
    }
}

// ---------------------------------------------------------------------------
// Text
// ---------------------------------------------------------------------------

/// The look of a run: theme font and colour, size in points.
#[derive(Debug, Clone, Copy)]
pub struct Look {
    pub size: f64,
    pub bold: bool,
    pub italic: bool,
    pub caps: bool,
    /// Character spacing in points (tracking).
    pub tracking: f64,
    pub color: Clr,
    /// The heading font of the theme; else the body font.
    pub major: bool,
}

impl Look {
    pub const fn new(size: f64, color: Clr) -> Look {
        Look { size, bold: false, italic: false, caps: false, tracking: 0.0, color, major: false }
    }
    pub const fn bold(mut self) -> Look {
        self.bold = true;
        self
    }
    pub const fn italic(mut self) -> Look {
        self.italic = true;
        self
    }
    pub const fn caps(mut self) -> Look {
        self.caps = true;
        self
    }
    pub const fn track(mut self, pt: f64) -> Look {
        self.tracking = pt;
        self
    }
    pub const fn heading(mut self) -> Look {
        self.major = true;
        self
    }

    /// `<w:rPr>` content, in the order the schema wants. `placeholder` adds the Placeholder Text style first.
    pub fn rpr(&self, th: &ShapeTheme, placeholder: bool) -> String {
        let mut s = String::from("<w:rPr>");
        if placeholder {
            s.push_str(r#"<w:rStyle w:val="PlaceholderText"/>"#);
        }
        let (a, e, c) = if self.major { ("major", "majorEastAsia", "majorBidi") } else { ("minor", "minorEastAsia", "minorBidi") };
        s.push_str(&format!(r#"<w:rFonts w:asciiTheme="{a}HAnsi" w:hAnsiTheme="{a}HAnsi" w:eastAsiaTheme="{e}" w:cstheme="{c}"/>"#));
        if self.bold {
            s.push_str("<w:b/><w:bCs/>");
        }
        if self.italic {
            s.push_str("<w:i/><w:iCs/>");
        }
        if self.caps {
            s.push_str("<w:caps/>");
        }
        s.push_str(&format!("<w:color {}/>", self.color.attrs(th, "w:val", "w:themeColor", "w:themeTint", "w:themeShade")));
        if self.tracking != 0.0 {
            s.push_str(&format!(r#"<w:spacing w:val="{}"/>"#, (self.tracking * 20.0).round() as i64));
        }
        let hp = (self.size * 2.0).round() as i64;
        s.push_str(&format!(r#"<w:sz w:val="{hp}"/><w:szCs w:val="{hp}"/></w:rPr>"#));
        s
    }

    /// The height of one line of this text, in twips (what Word's single spacing comes to).
    pub fn line(&self) -> i64 {
        (self.size * 20.0 * 1.22).round() as i64
    }
}

pub fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// The content of a run for some text: text elements, with a tab as `<w:tab/>` and a line break as `<w:br/>` (the same
/// split every other op makes, `model::text_items`).
pub fn text_els(text: &str) -> String {
    use crate::model::RunItem;
    let items = crate::model::text_items(text);
    if items.is_empty() {
        return r#"<w:t xml:space="preserve"></w:t>"#.to_string();
    }
    items
        .iter()
        .map(|i| match i {
            RunItem::Text(t) => format!(r#"<w:t xml:space="preserve">{}</w:t>"#, esc(t)),
            RunItem::Tab => "<w:tab/>".to_string(),
            _ => "<w:br/>".to_string(),
        })
        .collect()
}

/// A run of text.
pub fn run(th: &ShapeTheme, look: &Look, text: &str) -> String {
    format!("<w:r>{}{}</w:r>", look.rpr(th, false), text_els(text))
}

/// A rule drawn as a paragraph border.
#[derive(Debug, Clone, Copy)]
pub struct Rule {
    /// Eighths of a point.
    pub width: u32,
    pub color: Clr,
    /// `single` or `double`.
    pub style: &'static str,
    /// Points between the rule and the text.
    pub space: u32,
}

/// What a paragraph looks like.
#[derive(Debug, Clone)]
pub struct Para {
    pub align: &'static str,
    /// The paragraph style (Header, Footer); none is Normal.
    pub style: Option<String>,
    /// A centre tab stop and a right tab stop, in twips from the left indent.
    pub tabs: (Option<i64>, Option<i64>),
    /// Space before in twips (a cover sets space before only, so the gaps do not depend on how a reader collapses them).
    pub before: i64,
    pub left: i64,
    pub right: i64,
    pub top_rule: Option<Rule>,
    pub bottom_rule: Option<Rule>,
    pub left_rule: Option<Rule>,
    pub shade: Option<Clr>,
    pub keep: bool,
    /// Line spacing as a multiple of single (240 is single).
    pub line: i64,
}

impl Para {
    pub const fn new(align: &'static str) -> Para {
        Para { align, style: None, tabs: (None, None), before: 0, left: 0, right: 0, top_rule: None, bottom_rule: None, left_rule: None, shade: None, keep: false, line: 240 }
    }
    pub fn style(mut self, id: &str) -> Para {
        self.style = Some(id.to_string());
        self
    }
    pub fn tabs(mut self, center: Option<i64>, right: Option<i64>) -> Para {
        self.tabs = (center, right);
        self
    }
    /// Taller lines, so a shaded paragraph reads as a band.
    pub fn padded(mut self) -> Para {
        self.line = 360;
        self
    }
    pub fn before(mut self, twips: i64) -> Para {
        self.before = twips;
        self
    }
    pub fn indent(mut self, left: i64, right: i64) -> Para {
        self.left = left;
        self.right = right;
        self
    }
    pub fn bottom(mut self, r: Rule) -> Para {
        self.bottom_rule = Some(r);
        self
    }
    pub fn top(mut self, r: Rule) -> Para {
        self.top_rule = Some(r);
        self
    }
    pub fn left_bar(mut self, r: Rule) -> Para {
        self.left_rule = Some(r);
        self
    }
    pub fn shaded(mut self, c: Clr) -> Para {
        self.shade = Some(c);
        self
    }
    pub fn line(mut self, v: i64) -> Para {
        self.line = v;
        self
    }

    /// `<w:pPr>`, with `mark` as the paragraph mark's run properties (so an empty paragraph is as tall as its text).
    pub fn ppr(&self, th: &ShapeTheme, mark: Option<&Look>, extra: &str) -> String {
        let mut s = String::from("<w:pPr>");
        if let Some(st) = &self.style {
            s.push_str(&format!(r#"<w:pStyle w:val="{}"/>"#, esc(st)));
        }
        if self.keep {
            s.push_str("<w:keepNext/><w:keepLines/>");
        }
        let side = |tag: &str, r: &Rule| format!(r#"<w:{tag} w:val="{}" w:sz="{}" w:space="{}" {}/>"#, r.style, r.width, r.space, r.color.attrs(th, "w:color", "w:themeColor", "w:themeTint", "w:themeShade"));
        if self.top_rule.is_some() || self.bottom_rule.is_some() || self.left_rule.is_some() {
            s.push_str("<w:pBdr>");
            if let Some(r) = &self.top_rule {
                s.push_str(&side("top", r));
            }
            if let Some(r) = &self.left_rule {
                s.push_str(&side("left", r));
            }
            if let Some(r) = &self.bottom_rule {
                s.push_str(&side("bottom", r));
            }
            s.push_str("</w:pBdr>");
        }
        if let Some(c) = &self.shade {
            s.push_str(&format!(r#"<w:shd w:val="clear" w:color="auto" {}/>"#, c.attrs(th, "w:fill", "w:themeFill", "w:themeFillTint", "w:themeFillShade")));
        }
        if self.tabs.0.is_some() || self.tabs.1.is_some() {
            s.push_str("<w:tabs>");
            if let Some(c) = self.tabs.0 {
                s.push_str(&format!(r#"<w:tab w:val="center" w:pos="{c}"/>"#));
            }
            if let Some(r) = self.tabs.1 {
                s.push_str(&format!(r#"<w:tab w:val="right" w:pos="{r}"/>"#));
            }
            s.push_str("</w:tabs>");
        }
        s.push_str(&format!(r#"<w:spacing w:before="{}" w:after="0" w:line="{}" w:lineRule="auto"/>"#, self.before.max(0), self.line));
        if self.left != 0 || self.right != 0 {
            s.push_str(&format!(r#"<w:ind w:left="{}" w:right="{}"/>"#, self.left, self.right));
        }
        s.push_str(&format!(r#"<w:jc w:val="{}"/>"#, self.align));
        if let Some(m) = mark {
            s.push_str(&m.rpr(th, false));
        }
        s.push_str(extra);
        s.push_str("</w:pPr>");
        s
    }
}

/// A paragraph of the pieces given (runs, controls, fields).
pub fn para(th: &ShapeTheme, p: &Para, mark: &Look, inner: &str) -> String {
    format!("<w:p>{}{inner}</w:p>", p.ppr(th, Some(mark), ""))
}

/// A paragraph with no text; it is as tall as `mark` makes a line.
pub fn empty_para(th: &ShapeTheme, p: &Para, mark: &Look) -> String {
    para(th, p, mark, "")
}

pub fn page_break() -> String {
    r#"<w:p><w:r><w:br w:type="page"/></w:r></w:p>"#.to_string()
}

/// A field: begin, instruction, separate, the result shown, end.
pub fn field(th: &ShapeTheme, look: &Look, instr: &str, shown: &str) -> String {
    let rpr = look.rpr(th, false);
    format!(
        r#"<w:r>{rpr}<w:fldChar w:fldCharType="begin"/></w:r><w:r>{rpr}<w:instrText xml:space="preserve"> {instr} </w:instrText></w:r><w:r>{rpr}<w:fldChar w:fldCharType="separate"/></w:r><w:r>{rpr}{}</w:r><w:r>{rpr}<w:fldChar w:fldCharType="end"/></w:r>"#,
        text_els(shown)
    )
}

// ---------------------------------------------------------------------------
// Content controls bound to document properties
// ---------------------------------------------------------------------------

/// The things a cover page, header or footer fills in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Field {
    Title,
    Subtitle,
    Author,
    Company,
    Date,
    Abstract,
    /// Not tied to a property: the person types into it.
    Course,
    Supervisor,
    Type,
}

impl Field {
    pub fn key(self) -> &'static str {
        match self {
            Field::Title => "title",
            Field::Subtitle => "subtitle",
            Field::Author => "author",
            Field::Company => "company",
            Field::Date => "date",
            Field::Abstract => "abstract",
            Field::Course => "course",
            Field::Supervisor => "supervisor",
            Field::Type => "type",
        }
    }

    /// The name Word gives the control (its title, shown on the tab).
    pub fn alias(self) -> &'static str {
        match self {
            Field::Title => "Title",
            Field::Subtitle => "Subtitle",
            Field::Author => "Author",
            Field::Company => "Company",
            Field::Date => "Date",
            Field::Abstract => "Abstract",
            Field::Course => "Course",
            Field::Supervisor => "Supervisor",
            Field::Type => "Type here",
        }
    }

    /// The text a control shows while empty, in English; the caller can give others (`Labels`).
    pub fn placeholder(self) -> &'static str {
        match self {
            Field::Title => "[Document title]",
            Field::Subtitle => "[Document subtitle]",
            Field::Author => "[Author name]",
            Field::Company => "[Company name]",
            Field::Date => "[Date]",
            Field::Abstract => "[Abstract: a short summary of the document]",
            Field::Course => "[Course name]",
            Field::Supervisor => "[Supervisor name]",
            Field::Type => "[Type here]",
        }
    }

    /// Store id, XPath and prefix mappings of the node a control is bound to, if it has one.
    pub fn binding(self) -> Option<(&'static str, &'static str, &'static str)> {
        use crate::document::prop_binding::{APP_PREFIXES, APP_STORE, CORE_PREFIXES, CORE_STORE};
        Some(match self {
            Field::Title => (CORE_STORE, "/ns1:coreProperties[1]/ns0:title[1]", CORE_PREFIXES),
            Field::Subtitle => (CORE_STORE, "/ns1:coreProperties[1]/ns0:subject[1]", CORE_PREFIXES),
            Field::Author => (CORE_STORE, "/ns1:coreProperties[1]/ns0:creator[1]", CORE_PREFIXES),
            Field::Company => (APP_STORE, "/ns0:Properties[1]/ns0:Company[1]", APP_PREFIXES),
            Field::Date => (COVER_STORE, "/ns0:CoverPageProperties[1]/ns0:PublishDate[1]", COVER_PREFIXES),
            Field::Abstract => (COVER_STORE, "/ns0:CoverPageProperties[1]/ns0:Abstract[1]", COVER_PREFIXES),
            _ => return None,
        })
    }
}

/// Placeholder texts that replace the English ones (the interface language), by `Field::key`.
pub type Labels = HashMap<String, String>;

/// What the gallery builders need from the document: unique ids, placeholder building blocks, and the value a
/// bound control shows now.
pub trait Env {
    fn sdt_id(&mut self) -> i64;
    fn drawing_id(&mut self) -> u32;
    /// The name of the placeholder building block that holds this text.
    fn placeholder(&mut self, text: &str) -> String;
    /// The text of the node a field is bound to (the document's title...), empty when it has none.
    fn value(&self, field: Field) -> String;
    /// The date of the cover page property, `2026-10-14T00:00:00Z`, when there is one.
    fn date(&self) -> Option<String>;
    fn label(&self, field: Field) -> String;
    /// A word of the page itself (`Page`, `of`, `Submitted by`) in the interface language, else the English one.
    fn word(&self, key: &str, english: &str) -> String;
    /// The date's format and language for a new picker.
    fn date_format(&self) -> (String, String);
}

/// An inline content control for a field, as Word writes the ones on its cover pages and headers: a title, bound to the
/// property, with the placeholder building block and the look of its text. It shows the property's value, or its
/// placeholder while the property is empty.
pub fn control(env: &mut dyn Env, th: &ShapeTheme, f: Field, look: &Look) -> String {
    let id = env.sdt_id();
    let ph_text = env.label(f);
    let ph = env.placeholder(&ph_text);
    let (value, full) = match f {
        Field::Date => match env.date() {
            Some(d) => {
                let (fmt, lid) = env.date_format();
                match crate::controls::parse_date(&d) {
                    Some((y, m, dd)) => (crate::controls::format_date(&fmt, &lid, y, m, dd), Some(format!("{y:04}-{m:02}-{dd:02}T00:00:00Z"))),
                    None => (String::new(), None),
                }
            }
            None => (String::new(), None),
        },
        _ => (env.value(f), None),
    };
    let showing = value.trim().is_empty();
    let shown = if showing { ph_text.clone() } else { value };
    let mut pr = String::new();
    pr.push_str(&format!(r#"<w:alias w:val="{}"/><w:tag w:val=""/><w:id w:val="{id}"/>"#, esc(f.alias())));
    pr.push_str(&format!(r#"<w:placeholder><w:docPart w:val="{}"/></w:placeholder>"#, esc(&ph)));
    if showing {
        pr.push_str("<w:showingPlcHdr/>");
    }
    if let Some((store, xpath, prefixes)) = f.binding() {
        pr.push_str(&format!(r#"<w:dataBinding w:prefixMappings="{}" w:xpath="{}" w:storeItemID="{store}"/>"#, esc(prefixes), esc(xpath)));
    }
    match f {
        Field::Date => {
            let (fmt, lid) = env.date_format();
            let full_attr = full.map(|d| format!(r#" w:fullDate="{d}""#)).unwrap_or_default();
            pr.push_str(&format!(r#"<w:date{full_attr}><w:dateFormat w:val="{}"/><w:lid w:val="{}"/><w:storeMappedDataAs w:val="dateTime"/><w:calendar w:val="gregorian"/></w:date>"#, esc(&fmt), esc(&lid)));
        }
        Field::Abstract => pr.push_str(r#"<w:text w:multiLine="1"/>"#),
        _ => pr.push_str("<w:text/>"),
    }
    format!(
        "<w:sdt><w:sdtPr>{}{pr}</w:sdtPr><w:sdtContent><w:r>{}{}</w:r></w:sdtContent></w:sdt>",
        look.rpr(th, false),
        look.rpr(th, showing),
        text_els(&shown)
    )
}

// ---------------------------------------------------------------------------
// Shapes
// ---------------------------------------------------------------------------

const EMU_PER_TWIP: i64 = 635;

/// A rectangle or ellipse placed on the page behind the text, as Word writes a shape: DrawingML with a VML copy.
/// Positions and sizes are in twips from the top left corner of the page.
#[allow(clippy::too_many_arguments)]
pub fn shape(env: &mut dyn Env, th: &ShapeTheme, geometry: &str, name: &str, x: i64, y: i64, w: i64, h: i64, fill: Option<Clr>, line: Option<(Clr, f64)>, z: u32) -> String {
    let id = env.drawing_id();
    let (cx, cy) = (w * EMU_PER_TWIP, h * EMU_PER_TWIP);
    let (px, py) = (x * EMU_PER_TWIP, y * EMU_PER_TWIP);
    let relative = 251658240 + z as u64 * 1024;
    let fill_xml = fill.map(|c| c.fill()).unwrap_or_else(|| "<a:noFill/>".into());
    let ln = match line {
        Some((c, pt)) => format!(r#"<a:ln w="{}">{}<a:miter lim="800000"/></a:ln>"#, (pt * 12700.0).round() as i64, c.fill()),
        None => "<a:ln><a:noFill/></a:ln>".into(),
    };
    let vml_tag = match geometry {
        "ellipse" => "v:oval",
        _ => "v:rect",
    };
    let vml_fill = fill.map(|c| format!(r##" fillcolor="#{}""##, c.hex(th).to_lowercase())).unwrap_or_else(|| r#" filled="f""#.into());
    let vml_line = match line {
        Some((c, pt)) => format!(r##" strokecolor="#{}" strokeweight="{}pt""##, c.hex(th).to_lowercase(), pt),
        None => r#" stroked="f""#.into(),
    };
    let name = esc(&format!("{name} {id}"));
    format!(
        r#"<w:r><w:rPr><w:noProof/></w:rPr><mc:AlternateContent><mc:Choice Requires="wps"><w:drawing><wp:anchor distT="0" distB="0" distL="0" distR="0" simplePos="0" relativeHeight="{relative}" behindDoc="1" locked="0" layoutInCell="1" allowOverlap="1"><wp:simplePos x="0" y="0"/><wp:positionH relativeFrom="page"><wp:posOffset>{px}</wp:posOffset></wp:positionH><wp:positionV relativeFrom="page"><wp:posOffset>{py}</wp:posOffset></wp:positionV><wp:extent cx="{cx}" cy="{cy}"/><wp:effectExtent l="0" t="0" r="0" b="0"/><wp:wrapNone/><wp:docPr id="{id}" name="{name}"/><wp:cNvGraphicFramePr/><a:graphic><a:graphicData uri="http://schemas.microsoft.com/office/word/2010/wordprocessingShape"><wps:wsp><wps:cNvSpPr/><wps:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="{cx}" cy="{cy}"/></a:xfrm><a:prstGeom prst="{geometry}"><a:avLst/></a:prstGeom>{fill_xml}{ln}</wps:spPr><wps:bodyPr rot="0" vert="horz" wrap="square" lIns="91440" tIns="45720" rIns="91440" bIns="45720" anchor="ctr" anchorCtr="0"><a:noAutofit/></wps:bodyPr></wps:wsp></a:graphicData></a:graphic><wp14:sizeRelH relativeFrom="page"><wp14:pctWidth>0</wp14:pctWidth></wp14:sizeRelH><wp14:sizeRelV relativeFrom="page"><wp14:pctHeight>0</wp14:pctHeight></wp14:sizeRelV></wp:anchor></w:drawing></mc:Choice><mc:Fallback><w:pict><{vml_tag} id="{name}" o:spid="_x0000_s{}" style="position:absolute;margin-left:{}pt;margin-top:{}pt;width:{}pt;height:{}pt;z-index:-{};mso-position-horizontal-relative:page;mso-position-vertical-relative:page"{vml_fill}{vml_line}><w10:wrap anchorx="page" anchory="page"/></{vml_tag}></w:pict></mc:Fallback></mc:AlternateContent></w:r>"#,
        2048 + id,
        pt(x),
        pt(y),
        pt(w),
        pt(h),
        251658240u64 - z as u64 * 1024,
    )
}

fn pt(twips: i64) -> String {
    let v = twips as f64 / 20.0;
    if (v - v.round()).abs() < 0.005 {
        format!("{}", v.round() as i64)
    } else {
        format!("{v:.2}")
    }
}

/// A block-level content control around the paragraphs of a gallery part: `gallery` is `Cover Pages`, `Headers`...
pub fn gallery_sdt(env: &mut dyn Env, gallery: &str, unique: bool, inner: &str) -> String {
    let id = env.sdt_id();
    format!(
        r#"<w:sdt><w:sdtPr><w:id w:val="{id}"/><w:docPartObj><w:docPartGallery w:val="{}"/>{}</w:docPartObj></w:sdtPr><w:sdtEndPr/><w:sdtContent>{inner}</w:sdtContent></w:sdt>"#,
        esc(gallery),
        if unique { "<w:docPartUnique/>" } else { "" }
    )
}

/// The namespaces the markup above uses, as attributes for a wrapper element it is parsed under.
pub fn namespace_decls() -> String {
    let mut s = String::from(r#" xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main""#);
    for (p, u) in crate::shapes::build::ROOT_NS {
        s.push_str(&format!(r#" xmlns:{p}="{u}""#));
    }
    s.push_str(r#" xmlns:w14="http://schemas.microsoft.com/office/word/2010/wordml""#);
    s
}

/// Parse markup written with the helpers above into elements (without the namespace declarations).
pub fn parse(markup: &str) -> Result<Vec<crate::xml::Element>, String> {
    let src = format!("<w:root{}>{markup}</w:root>", namespace_decls());
    let doc = crate::xml::parse(src.as_bytes()).map_err(|e| format!("the page part is not valid: {e}"))?;
    Ok(doc.root.elements().cloned().collect())
}
