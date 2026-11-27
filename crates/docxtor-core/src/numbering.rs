//! Lists: `numbering.xml` definitions, label computation and list creation.

use crate::model::{Block, NodeId};
use crate::props::{ParaFmt, RunFmt};
use crate::styles::StyleSheet;
use crate::xml::{self, Element, Node, XmlDoc};
use serde::Serialize;
use std::collections::HashMap;

mod list_def;
pub use list_def::{LevelDef, LevelPreview, ListDef, ListDefinition};

#[derive(Debug, Clone, Default)]
pub struct Level {
    pub start: i64,
    pub fmt: String,
    pub text: String,
    pub ppr: ParaFmt,
    pub rpr: RunFmt,
    pub is_lgl: bool,
    pub suff: String,
    /// `left`, `center` or `right` (`w:lvlJc`): where the label sits in the space before the text.
    pub jc: String,
    /// `w:lvlRestart`: 0 never restarts the level; n restarts it after a paragraph of level n or higher.
    pub restart: Option<i64>,
    /// `w:lvlPicBulletId`: the picture bullet that draws the label.
    pub pic: Option<i64>,
    /// `w:pStyle`: the paragraph style linked to this level (a heading).
    pub pstyle: Option<String>,
}

#[derive(Debug, Clone, Default)]
struct AbstractNum {
    levels: HashMap<i64, Level>,
    /// `w:name`; Docxtor's presets are recognised by `Docxtor {preset}`.
    name: Option<String>,
    /// `w:styleLink`: this definition is the list of a list style.
    style_link: Option<String>,
    /// `w:numStyleLink`: the levels come from the list style of that name (`levels` is a copy of its levels).
    num_style_link: Option<String>,
    /// The definition holding the levels when `num_style_link` names a list style.
    link_target: Option<i64>,
}

#[derive(Debug, Clone, Default)]
struct Num {
    abstract_id: i64,
    start_overrides: HashMap<i64, i64>,
    level_overrides: HashMap<i64, Level>,
}

/// A picture bullet (`w:numPicBullet`).
#[derive(Debug, Clone, Default)]
struct PicBullet {
    /// The relationship (of `numbering.xml`) to the picture.
    rid: String,
    /// Size in points.
    w: f64,
    h: f64,
}

#[derive(Debug, Clone, Default)]
pub struct Numbering {
    pub doc: Option<XmlDoc>,
    abstracts: HashMap<i64, AbstractNum>,
    nums: HashMap<i64, Num>,
    pic_bullets: HashMap<i64, PicBullet>,
    /// Relationship id of `numbering.xml` to the part it points at (picture bullets). Set by the document.
    pub rels: HashMap<String, String>,
    pub dirty: bool,
    /// Changes whenever the definitions or the pictures they point at do (and is new for every parse), so a result
    /// that depends on them can tell it is out of date without comparing the part, which can be megabytes.
    pub gen: u64,
}

/// One counter for the whole program, so two numbering parts never share a generation, whichever thread made them.
static GEN: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// A picture that draws a list label.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct LabelPic {
    /// Part name of the picture (`word/media/image1.png`).
    pub src: String,
    /// Size in points.
    pub w: f64,
    pub h: f64,
}

/// What the label count reads from one list paragraph (see `Numbering::list_input`).
#[derive(Debug, Clone, PartialEq)]
pub struct ListInput {
    pub id: NodeId,
    pub num_id: i64,
    /// The level the paragraph or its style names, if either does.
    pub ilvl: Option<i64>,
    /// The paragraph's own style id.
    pub style: Option<String>,
}

/// A computed list label for one paragraph.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ListLabel {
    pub text: String,
    pub level: i64,
    pub num_id: i64,
    /// The count the label shows (1 for "1.", 3 for "c)").
    pub value: i64,
    pub bullet: bool,
    /// A checklist item: a bullet whose glyph is a checkbox (☐ unchecked, ☑ or ☒ checked).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub checklist: bool,
    /// A checked checklist item.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub checked: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bold: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub italic: Option<bool>,
    /// Underline type (`single`, `double`, ...).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub underline: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strike: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub highlight: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub caps: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub small_caps: Option<bool>,
    /// `center` or `right` when the label isn't left-aligned in its space (`w:lvlJc`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub align: Option<String>,
    /// A picture bullet: the label is this picture, not text.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pic: Option<LabelPic>,
    /// `tab`, `space` or `nothing`.
    pub suffix: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ListKind {
    Bullet,
    Decimal,
}

fn parse_level(l: &Element) -> Level {
    let val = |n: &str| l.child(n).and_then(|e| e.attr("w:val")).map(String::from);
    Level {
        start: val("w:start").and_then(|v| v.parse().ok()).unwrap_or(1),
        fmt: val("w:numFmt").unwrap_or_else(|| "decimal".into()),
        text: val("w:lvlText").unwrap_or_default(),
        ppr: l.child("w:pPr").map(ParaFmt::from_ppr).unwrap_or_default(),
        rpr: l.child("w:rPr").map(RunFmt::from_rpr).unwrap_or_default(),
        is_lgl: l.child("w:isLgl").is_some(),
        suff: val("w:suff").unwrap_or_else(|| "tab".into()),
        jc: match val("w:lvlJc").as_deref() {
            Some("right" | "end") => "right".into(),
            Some("center") => "center".into(),
            _ => "left".into(),
        },
        restart: val("w:lvlRestart").and_then(|v| v.parse().ok()),
        pic: val("w:lvlPicBulletId").and_then(|v| v.parse().ok()),
        pstyle: val("w:pStyle"),
    }
}

/// The size of a picture bullet in points, from the style of its `v:shape` (`width:9pt;height:9pt`) or the
/// extent of its drawing.
fn pic_bullet_size(el: &Element) -> (f64, f64) {
    let get = |st: &str, key: &str| -> Option<f64> {
        st.split(';').find_map(|part| {
            let (k, v) = part.split_once(':')?;
            if k.trim() != key {
                return None;
            }
            let v = v.trim();
            let digits_end = v.find(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-')).unwrap_or(v.len());
            let num: f64 = v[..digits_end].parse().ok()?;
            Some(match &v[digits_end..] {
                "in" => num * 72.0,
                "cm" => num * 72.0 / 2.54,
                "mm" => num * 72.0 / 25.4,
                "px" => num * 0.75,
                _ => num,
            })
        })
    };
    if let Some(style) = el.find("v:shape").and_then(|s| s.attr("style")) {
        if let (Some(w), Some(h)) = (get(style, "width"), get(style, "height")) {
            return (w, h);
        }
    }
    if let Some(ext) = el.find("wp:extent") {
        let emu = |a: &str| ext.attr(a).and_then(|v| v.parse::<f64>().ok()).map(|v| v / 12700.0);
        if let (Some(w), Some(h)) = (emu("cx"), emu("cy")) {
            return (w, h);
        }
    }
    (9.0, 9.0)
}

impl Numbering {
    pub fn parse(doc: Option<XmlDoc>) -> Numbering {
        let mut n = Numbering { doc, ..Default::default() };
        n.reload();
        n
    }

    /// Note that the definitions changed (see `gen`).
    pub fn touch(&mut self) {
        self.gen = GEN.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
    }

    fn reload(&mut self) {
        self.touch();
        self.abstracts.clear();
        self.nums.clear();
        self.pic_bullets.clear();
        let Some(doc) = &self.doc else { return };
        for pb in doc.root.children_named("w:numPicBullet") {
            let Some(id) = pb.attr("w:numPicBulletId").and_then(|v| v.parse().ok()) else { continue };
            let rid = pb.find("v:imagedata").and_then(|e| e.attr("r:id")).or_else(|| pb.find("a:blip").and_then(|e| e.attr("r:embed")));
            let Some(rid) = rid else { continue };
            let (w, h) = pic_bullet_size(pb);
            self.pic_bullets.insert(id, PicBullet { rid: rid.to_string(), w, h });
        }
        for a in doc.root.children_named("w:abstractNum") {
            let Some(id) = a.attr("w:abstractNumId").and_then(|v| v.parse().ok()) else { continue };
            let mut levels = HashMap::new();
            for l in a.children_named("w:lvl") {
                let ilvl = l.attr("w:ilvl").and_then(|v| v.parse().ok()).unwrap_or(0);
                levels.insert(ilvl, parse_level(l));
            }
            let name = a.child("w:name").and_then(|e| e.attr("w:val")).map(String::from);
            let link = |n: &str| a.child(n).and_then(|e| e.attr("w:val")).map(String::from);
            self.abstracts.insert(id, AbstractNum { levels, name, style_link: link("w:styleLink"), num_style_link: link("w:numStyleLink"), link_target: None });
        }
        // A definition that names a list style (`numStyleLink`) takes the levels of the definition that is that
        // style's list (`styleLink`). Links can be chained, so follow them a few times.
        for _ in 0..4 {
            let links: Vec<(i64, i64)> = self
                .abstracts
                .iter()
                .filter_map(|(id, a)| {
                    let name = a.num_style_link.as_deref()?;
                    let target = self.abstracts.iter().filter(|(tid, t)| *tid != id && t.style_link.as_deref() == Some(name)).map(|(tid, _)| *tid).min()?;
                    Some((*id, target))
                })
                .collect();
            for (id, target) in links {
                let levels = self.abstracts[&target].levels.clone();
                let end = self.abstracts[&target].link_target.unwrap_or(target);
                let a = self.abstracts.get_mut(&id).unwrap();
                a.levels = levels;
                a.link_target = Some(end);
            }
        }
        for n in doc.root.children_named("w:num") {
            let Some(id) = n.attr("w:numId").and_then(|v| v.parse().ok()) else { continue };
            let abstract_id = n.child("w:abstractNumId").and_then(|e| e.attr("w:val")).and_then(|v| v.parse().ok()).unwrap_or(-1);
            let mut num = Num { abstract_id, ..Default::default() };
            for o in n.children_named("w:lvlOverride") {
                let ilvl = o.attr("w:ilvl").and_then(|v| v.parse().ok()).unwrap_or(0);
                if let Some(s) = o.child("w:startOverride").and_then(|e| e.attr("w:val")).and_then(|v| v.parse().ok()) {
                    num.start_overrides.insert(ilvl, s);
                }
                if let Some(l) = o.child("w:lvl") {
                    num.level_overrides.insert(ilvl, parse_level(l));
                }
            }
            self.nums.insert(id, num);
        }
    }

    pub fn level(&self, num_id: i64, ilvl: i64) -> Option<&Level> {
        let num = self.nums.get(&num_id)?;
        if let Some(l) = num.level_overrides.get(&ilvl) {
            return Some(l);
        }
        self.abstracts.get(&num.abstract_id)?.levels.get(&ilvl)
    }

    pub fn is_bullet(&self, num_id: i64) -> bool {
        self.level(num_id, 0).is_some_and(|l| l.fmt == "bullet")
    }

    /// The definition that holds a definition's levels: itself, or the list style it names.
    fn effective_abstract(&self, abstract_id: i64) -> i64 {
        self.abstracts.get(&abstract_id).and_then(|a| a.link_target).unwrap_or(abstract_id)
    }

    /// The level a paragraph style is linked to (`w:pStyle` in the level), for a style whose `numPr` names a
    /// list but no level.
    fn level_for_style(&self, num_id: i64, style: &str) -> Option<i64> {
        let num = self.nums.get(&num_id)?;
        let abs = self.abstracts.get(&num.abstract_id)?;
        (0..9).find(|l| num.level_overrides.get(l).or_else(|| abs.levels.get(l)).and_then(|lv| lv.pstyle.as_deref()) == Some(style))
    }

    /// The picture a picture bullet draws, once the document has told us where its relationships point.
    fn label_pic(&self, level: &Level) -> Option<LabelPic> {
        let pb = self.pic_bullets.get(&level.pic?)?;
        let src = self.rels.get(&pb.rid)?;
        Some(LabelPic { src: src.clone(), w: pb.w, h: pb.h })
    }

    /// What a paragraph tells the label count: its list, level and style, after the style's own list settings. `None`
    /// when it is in no list. Holds nothing that depends on other paragraphs, so a block's can be kept while it is
    /// unchanged (see `document/facts.rs`).
    pub fn list_input(p: &crate::model::Paragraph, styles: &StyleSheet) -> Option<ListInput> {
        let direct = p.ppr.as_ref().map(ParaFmt::from_ppr).unwrap_or_default();
        let (style_p, _) = styles.para_style(direct.style.as_deref());
        let f = style_p.overlay(&direct);
        let num_id = f.num_id.filter(|n| *n > 0)?;
        Some(ListInput { id: p.id, num_id, ilvl: f.ilvl, style: direct.style })
    }

    /// Compute labels for every list paragraph in document order.
    pub fn labels<'a>(&self, body: impl IntoIterator<Item = &'a Block>, styles: &StyleSheet) -> HashMap<NodeId, ListLabel> {
        let mut inputs = Vec::new();
        // Imported chunks count their own lists (see `alt_chunk`).
        crate::model::walk_paragraphs_local(body, &mut |p| inputs.extend(Self::list_input(p, styles)));
        self.labels_of(inputs.iter(), styles)
    }

    /// The labels for list paragraphs given as their `ListInput`s, in document order.
    pub fn labels_of<'a>(&self, inputs: impl Iterator<Item = &'a ListInput>, styles: &StyleSheet) -> HashMap<NodeId, ListLabel> {
        let mut out = HashMap::new();
        // counters keyed by list identity (abstract id unless the num restarts) -> per level.
        let mut counters: HashMap<String, [i64; 9]> = HashMap::new();
        let mut seen_levels: HashMap<String, [bool; 9]> = HashMap::new();
        for inp in inputs {
            let ListInput { id, num_id, ilvl: f_ilvl, style } = inp;
            let num_id = *num_id;
            let Some(num) = self.nums.get(&num_id) else { continue };
            let ilvl = match f_ilvl {
                Some(l) => (*l).clamp(0, 8),
                None => style.as_deref().and_then(|s| self.level_for_style(num_id, s)).unwrap_or(0),
            };
            let Some(level) = self.level(num_id, ilvl) else { continue };
            // Lists on the same definition (or on the same list style) count together, unless a num restarts.
            let key = if num.start_overrides.is_empty() && num.level_overrides.is_empty() {
                format!("a{}", self.effective_abstract(num.abstract_id))
            } else {
                format!("n{num_id}")
            };
            let start_of = |l: i64| -> i64 {
                num.start_overrides.get(&l).copied().unwrap_or_else(|| self.level(num_id, l).map(|x| x.start).unwrap_or(1))
            };
            let c = counters.entry(key.clone()).or_insert([0; 9]);
            let seen = seen_levels.entry(key).or_insert([false; 9]);
            let i = ilvl as usize;
            if !seen[i] {
                c[i] = start_of(ilvl);
                seen[i] = true;
            } else {
                c[i] += 1;
            }
            // The levels below start again, unless their own `lvlRestart` says otherwise.
            for (j, s) in seen.iter_mut().enumerate().skip(i + 1) {
                let again = match self.level(num_id, j as i64).and_then(|l| l.restart) {
                    Some(0) => false,
                    Some(after) => (i as i64) < after.min(j as i64),
                    None => true,
                };
                if again {
                    *s = false;
                }
            }
            let bullet = level.fmt == "bullet";
            let pic = if bullet { self.label_pic(level) } else { None };
            let lang = std::cell::OnceCell::new();
            let text = if bullet {
                crate::list_glyphs::map_bullet(&level.text, level.rpr.font.as_deref())
            } else {
                let mut t = level.text.clone();
                for l in 0..=8usize {
                    let ph = format!("%{}", l + 1);
                    if t.contains(&ph) {
                        let lv = self.level(num_id, l as i64);
                        let n = if seen[l] { c[l] } else { start_of(l as i64) };
                        let fmt = if level.is_lgl { "decimal" } else { lv.map(|x| x.fmt.as_str()).unwrap_or("decimal") };
                        let lang: &Option<String> = lang.get_or_init(|| styles.para_lang(style.as_deref()));
                        t = t.replace(&ph, &crate::numfmt::format(n, fmt, lang.as_deref()));
                    }
                }
                t
            };
            let (checklist, checked) = checkbox_state(bullet, &text).map_or((false, false), |c| (true, c));
            let font = if bullet && crate::list_glyphs::is_symbol_font(level.rpr.font.as_deref()) { None } else { level.rpr.font.clone() };
            out.insert(
                *id,
                ListLabel {
                    text,
                    level: ilvl,
                    num_id,
                    value: c[i],
                    bullet,
                    checklist,
                    checked,
                    font,
                    color: level.rpr.color.clone(),
                    size: level.rpr.size,
                    bold: level.rpr.bold,
                    italic: level.rpr.italic,
                    underline: level.rpr.underline.clone(),
                    strike: level.rpr.strike,
                    highlight: level.rpr.highlight.clone(),
                    caps: level.rpr.caps,
                    small_caps: level.rpr.small_caps,
                    align: Some(level.jc.clone()).filter(|j| j == "center" || j == "right"),
                    pic,
                    suffix: level.suff.clone(),
                },
            );
        }
        out
    }

    fn ensure_doc(&mut self) -> &mut Element {
        if self.doc.is_none() {
            self.doc = Some(XmlDoc::new(
                Element::new("w:numbering").with_attr("xmlns:w", "http://schemas.openxmlformats.org/wordprocessingml/2006/main"),
            ));
        }
        &mut self.doc.as_mut().unwrap().root
    }

    fn next_abstract_id(&self) -> i64 {
        self.abstracts.keys().max().map(|m| m + 1).unwrap_or(0)
    }

    fn next_num_id(&self) -> i64 {
        self.nums.keys().max().map(|m| m + 1).unwrap_or(1)
    }

    /// Insert an `abstractNum` (schema order: all abstractNums precede all nums) and reload.
    fn insert_abstract(&mut self, el: Element) {
        let root = self.ensure_doc();
        let pos = root
            .children
            .iter()
            .position(|n| matches!(n, Node::Element(e) if e.name == "w:num" || e.name == "w:numIdMacAtCleanup"))
            .unwrap_or(root.children.len());
        root.children.insert(pos, Node::Element(el));
        self.dirty = true;
        self.reload();
    }

    /// Append a `num` (before `numIdMacAtCleanup`) and reload.
    fn insert_num(&mut self, el: Element) {
        let root = self.ensure_doc();
        let pos = root
            .children
            .iter()
            .position(|n| matches!(n, Node::Element(e) if e.name == "w:numIdMacAtCleanup"))
            .unwrap_or(root.children.len());
        root.children.insert(pos, Node::Element(el));
        self.dirty = true;
        self.reload();
    }

    /// The `w:num` element with this id, and the `w:abstractNum` it uses.
    pub fn list_elements(&self, num_id: i64) -> Option<(Element, Element)> {
        let root = &self.doc.as_ref()?.root;
        let num = root.children_named("w:num").find(|n| n.attr("w:numId").and_then(|v| v.parse::<i64>().ok()) == Some(num_id))?;
        let abs_id = num.child("w:abstractNumId")?.attr("w:val")?;
        let abs = root.children_named("w:abstractNum").find(|a| a.attr("w:abstractNumId") == Some(abs_id))?;
        Some((num.clone(), abs.clone()))
    }

    /// Bring in a list from another document. With `reuse`, an existing list with the same definition is used
    /// (so a list copied within a document continues); otherwise the list is added as a new one that counts on its own.
    pub fn import_list(&mut self, abs: &Element, num: &Element, reuse: bool) -> i64 {
        let canon = |e: &Element| {
            let mut c = e.clone();
            c.remove_attr("w:abstractNumId");
            c.remove_children("w:nsid");
            c.remove_children("w:tmpl");
            c.to_xml()
        };
        if reuse {
            let want_abs = canon(abs);
            let want_over: String = num.children_named("w:lvlOverride").map(|e| e.to_xml()).collect();
            let found = self.doc.as_ref().and_then(|d| {
                d.root.children_named("w:num").find(|n| {
                    let Some(a) = n.child("w:abstractNumId").and_then(|e| e.attr("w:val")) else { return false };
                    let over: String = n.children_named("w:lvlOverride").map(|e| e.to_xml()).collect();
                    over == want_over && d.root.children_named("w:abstractNum").any(|x| x.attr("w:abstractNumId") == Some(a) && canon(x) == want_abs)
                })
            });
            if let Some(n) = found.and_then(|n| n.attr("w:numId")).and_then(|v| v.parse().ok()) {
                return n;
            }
        }
        let abs_id = self.next_abstract_id();
        let num_id = self.next_num_id();
        let mut a = abs.clone();
        a.set_attr("w:abstractNumId", abs_id.to_string());
        a.remove_children("w:nsid");
        a.remove_children("w:tmpl");
        let mut n = num.clone();
        n.set_attr("w:numId", num_id.to_string());
        n.remove_children("w:abstractNumId");
        n.children.insert(0, Node::Element(Element::new("w:abstractNumId").with_attr("w:val", abs_id.to_string())));
        self.insert_abstract(a);
        self.insert_num(n);
        num_id
    }

    /// Create a new list definition and return its numId.
    pub fn create_list(&mut self, kind: ListKind) -> i64 {
        let abs_id = self.next_abstract_id();
        let num_id = self.next_num_id();
        let mut abs = format!(r#"<w:abstractNum w:abstractNumId="{abs_id}"><w:multiLevelType w:val="hybridMultilevel"/>"#);
        for l in 0..9 {
            let left = 720 * (l + 1);
            let (fmt, text, font) = match kind {
                ListKind::Bullet => {
                    let b = ["\u{2022}", "\u{25E6}", "\u{25AA}"][l % 3];
                    ("bullet", b.to_string(), r#"<w:rPr><w:rFonts w:ascii="Arial" w:hAnsi="Arial" w:hint="default"/></w:rPr>"#)
                }
                ListKind::Decimal => {
                    let f = ["decimal", "lowerLetter", "lowerRoman"][l % 3];
                    (f, format!("%{}.", l + 1), "")
                }
            };
            let jc = if fmt == "lowerRoman" { "right" } else { "left" };
            abs.push_str(&format!(
                r#"<w:lvl w:ilvl="{l}"><w:start w:val="1"/><w:numFmt w:val="{fmt}"/><w:lvlText w:val="{text}"/><w:lvlJc w:val="{jc}"/><w:pPr><w:ind w:left="{left}" w:hanging="360"/></w:pPr>{font}</w:lvl>"#
            ));
        }
        abs.push_str("</w:abstractNum>");
        let num = format!(r#"<w:num w:numId="{num_id}"><w:abstractNumId w:val="{abs_id}"/></w:num>"#);
        let parsed = xml::parse(format!(r#"<x xmlns:w="w">{abs}{num}</x>"#).as_bytes()).expect("valid list xml");
        let mut els = parsed.root.elements().cloned();
        let abs_el = els.next().unwrap();
        let num_el = els.next().unwrap();
        self.insert_abstract(abs_el);
        self.insert_num(num_el);
        num_id
    }

    // -----------------------------------------------------------------------
    // Presets, restarts
    // -----------------------------------------------------------------------

    /// The abstract definition of a num.
    pub fn abstract_of(&self, num_id: i64) -> Option<i64> {
        self.nums.get(&num_id).map(|n| n.abstract_id)
    }

    /// The Docxtor preset (`disc`, `decimal`, `checklist`, ...) a num's definition was made from.
    /// Recognised only by the definition's `Docxtor {preset}` name.
    pub fn preset_of(&self, num_id: i64) -> Option<&'static str> {
        let name = self.abstracts.get(&self.nums.get(&num_id)?.abstract_id)?.name.as_deref()?;
        preset_from_name(name)
    }

    /// Whether a num carries start/level overrides (so it counts on its own).
    pub fn has_overrides(&self, num_id: i64) -> bool {
        self.nums.get(&num_id).is_some_and(|n| !n.start_overrides.is_empty() || !n.level_overrides.is_empty())
    }

    /// A num's start-value overrides, by level.
    pub fn start_overrides(&self, num_id: i64) -> Vec<(i64, i64)> {
        let mut v: Vec<_> = self.nums.get(&num_id).map(|n| n.start_overrides.iter().map(|(a, b)| (*a, *b)).collect()).unwrap_or_default();
        v.sort();
        v
    }

    /// Whether the label glyph at this level is a checkbox: `Some(checked)` for checklist items.
    pub fn checkbox_at(&self, num_id: i64, ilvl: i64) -> Option<bool> {
        let level = self.level(num_id, ilvl)?;
        if level.fmt != "bullet" {
            return None;
        }
        checkbox_state(true, &crate::list_glyphs::map_bullet(&level.text, level.rpr.font.as_deref()))
    }

    /// The abstract definition of a preset, created on first use.
    fn ensure_preset_abstract(&mut self, preset: &str) -> Option<i64> {
        let name = preset_name(preset)?;
        if let Some((id, _)) = self.abstracts.iter().filter(|(_, a)| a.name.as_deref() == Some(name.as_str())).min_by_key(|(id, _)| **id) {
            return Some(*id);
        }
        let id = self.next_abstract_id();
        let xml = preset_abstract_xml(preset, id, &name)?;
        let parsed = xml::parse(format!(r#"<x xmlns:w="w">{xml}</x>"#).as_bytes()).expect("valid list xml");
        self.insert_abstract(parsed.root.elements().next().cloned().unwrap());
        Some(id)
    }

    /// The document's shared num for a preset (no overrides), created on first use.
    /// Right for bullets and checklists, where continuing a count doesn't matter.
    pub fn shared_num(&mut self, preset: &str) -> Option<i64> {
        let abs = self.ensure_preset_abstract(preset)?;
        let mut ids: Vec<_> = self.nums.iter().filter(|(_, n)| n.abstract_id == abs && n.start_overrides.is_empty() && n.level_overrides.is_empty()).map(|(id, _)| *id).collect();
        ids.sort();
        if let Some(id) = ids.first() {
            return Some(*id);
        }
        Some(self.add_num(abs, &[]))
    }

    /// A new num for a preset that counts on its own, starting at 1 (plus any given start overrides).
    pub fn fresh_num(&mut self, preset: &str, starts: &[(i64, i64)]) -> Option<i64> {
        let abs = self.ensure_preset_abstract(preset)?;
        let mut starts = starts.to_vec();
        if !starts.iter().any(|(l, _)| *l == 0) {
            starts.push((0, 1));
        }
        starts.sort();
        Some(self.add_num(abs, &starts))
    }

    /// A new num on a definition that counts on its own, starting at 1 (plus any given start overrides).
    pub fn fresh_num_on(&mut self, abs: i64, starts: &[(i64, i64)]) -> i64 {
        let mut starts = starts.to_vec();
        if !starts.iter().any(|(l, _)| *l == 0) {
            starts.push((0, 1));
        }
        starts.sort();
        self.add_num(abs, &starts)
    }

    /// The num of a definition that has no overrides (so lists on it continue each other), made when there is none.
    pub fn shared_num_on(&mut self, abs: i64) -> i64 {
        let mut ids: Vec<_> = self.nums.iter().filter(|(_, n)| n.abstract_id == abs && n.start_overrides.is_empty() && n.level_overrides.is_empty()).map(|(id, _)| *id).collect();
        ids.sort();
        match ids.first() {
            Some(id) => *id,
            None => self.add_num(abs, &[]),
        }
    }

    fn add_num(&mut self, abs: i64, starts: &[(i64, i64)]) -> i64 {
        let num_id = self.next_num_id();
        let mut el = Element::new("w:num").with_attr("w:numId", num_id.to_string()).with_child(Element::new("w:abstractNumId").with_attr("w:val", abs.to_string()));
        for (l, v) in starts {
            el.children.push(Node::Element(start_override(*l, *v)));
        }
        self.insert_num(el);
        num_id
    }

    /// A copy of a num (same definition, same overrides) whose count starts at `start` on level `ilvl`.
    pub fn restarted_num(&mut self, from: i64, ilvl: i64, start: i64) -> Option<i64> {
        let src = self.doc.as_ref()?.root.children_named("w:num").find(|n| n.attr("w:numId").and_then(|v| v.parse::<i64>().ok()) == Some(from))?.clone();
        let mut el = src;
        el.set_attr("w:numId", self.next_num_id().to_string());
        let num_id: i64 = el.attr("w:numId")?.parse().ok()?;
        let existing = el.children.iter().position(|n| matches!(n, Node::Element(e) if e.name == "w:lvlOverride" && e.attr("w:ilvl").and_then(|v| v.parse::<i64>().ok()) == Some(ilvl)));
        match existing {
            Some(i) => {
                let Node::Element(o) = &mut el.children[i] else { unreachable!() };
                o.remove_children("w:startOverride");
                o.children.insert(0, Node::Element(Element::new("w:startOverride").with_attr("w:val", start.to_string())));
            }
            None => {
                // Keep lvlOverrides in level order, after the abstractNumId.
                let at = el
                    .children
                    .iter()
                    .position(|n| matches!(n, Node::Element(e) if e.name == "w:lvlOverride" && e.attr("w:ilvl").and_then(|v| v.parse::<i64>().ok()).is_some_and(|l| l > ilvl)))
                    .unwrap_or(el.children.len());
                el.children.insert(at, Node::Element(start_override(ilvl, start)));
            }
        }
        self.insert_num(el);
        Some(num_id)
    }

    /// Find an existing list of this kind to reuse (so adjacent lists continue numbering).
    pub fn find_list(&self, kind: ListKind) -> Option<i64> {
        let mut ids: Vec<_> = self.nums.keys().copied().collect();
        ids.sort();
        ids.into_iter().find(|id| {
            let b = self.is_bullet(*id);
            match kind {
                ListKind::Bullet => b && self.checkbox_at(*id, 0).is_none(),
                ListKind::Decimal => !b && self.level(*id, 0).is_some_and(|l| l.fmt == "decimal"),
            }
        })
    }
}

fn start_override(ilvl: i64, start: i64) -> Element {
    Element::new("w:lvlOverride")
        .with_attr("w:ilvl", ilvl.to_string())
        .with_child(Element::new("w:startOverride").with_attr("w:val", start.to_string()))
}

/// `Some(checked)` when a bullet's label is a checkbox glyph.
fn checkbox_state(bullet: bool, text: &str) -> Option<bool> {
    if !bullet {
        return None;
    }
    match text.trim() {
        "\u{2610}" => Some(false),
        "\u{2611}" | "\u{2612}" => Some(true),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Presets
// ---------------------------------------------------------------------------

/// The list styles `set_list_style` accepts (besides `none`).
pub const LIST_STYLES: &[&str] = &[
    "disc", "diamond", "arrow", "star", "arrowhead", "box", "decimal", "paren", "outline", "upper", "roman", "zero", "checklist",
];

/// Every preset Docxtor can write, including the checked variant of the checklist.
const ALL_PRESETS: &[&str] = &[
    "disc", "diamond", "arrow", "star", "arrowhead", "box", "decimal", "paren", "outline", "upper", "roman", "zero", "checklist", "checklist_checked",
];

/// Whether `style` is a name `set_list_style` accepts.
pub fn is_list_style(style: &str) -> bool {
    LIST_STYLES.contains(&style)
}

/// Numbered presets count; the rest are bullets or checkboxes.
pub fn is_numbered_style(style: &str) -> bool {
    matches!(style, "decimal" | "paren" | "outline" | "upper" | "roman" | "zero")
}

fn preset_name(preset: &str) -> Option<String> {
    ALL_PRESETS.contains(&preset).then(|| format!("Docxtor {}", preset.replace('_', " ")))
}

fn preset_from_name(name: &str) -> Option<&'static str> {
    let rest = name.strip_prefix("Docxtor ")?;
    ALL_PRESETS.iter().copied().find(|p| p.replace('_', " ") == rest)
}

const ARIAL: &str = "Arial";
const SYMBOLS: &str = "Segoe UI Symbol";

/// Bullet glyph and font for the three repeating levels of a bullet preset.
fn bullet_glyphs(preset: &str) -> Option<[(&'static str, &'static str); 3]> {
    Some(match preset {
        "disc" => [("\u{25CF}", ARIAL), ("\u{25CB}", ARIAL), ("\u{25A0}", ARIAL)],
        "diamond" => [("\u{2756}", SYMBOLS), ("\u{27A2}", SYMBOLS), ("\u{25A0}", ARIAL)],
        "arrow" => [("\u{2794}", SYMBOLS), ("\u{25C6}", ARIAL), ("\u{25CF}", ARIAL)],
        "star" => [("\u{2605}", SYMBOLS), ("\u{25CB}", ARIAL), ("\u{25A0}", ARIAL)],
        "arrowhead" => [("\u{27A2}", SYMBOLS), ("\u{25CB}", ARIAL), ("\u{25A0}", ARIAL)],
        "box" => [("\u{2751}", SYMBOLS), ("\u{27A2}", SYMBOLS), ("\u{25A0}", ARIAL)],
        "checklist" => [("\u{2610}", SYMBOLS); 3],
        "checklist_checked" => [("\u{2611}", SYMBOLS); 3],
        _ => return None,
    })
}

/// Number format and `lvlText` of level `l` of a numbered preset.
fn numbered_level(preset: &str, l: usize) -> Option<(&'static str, String)> {
    let cycle = |f: [&'static str; 3]| f[l % 3];
    let own = format!("%{}.", l + 1);
    Some(match preset {
        "decimal" => (cycle(["decimal", "lowerLetter", "lowerRoman"]), own),
        "paren" => (cycle(["decimal", "lowerLetter", "lowerRoman"]), format!("%{})", l + 1)),
        "outline" => ("decimal", format!("{}.", (1..=l + 1).map(|i| format!("%{i}")).collect::<Vec<_>>().join("."))),
        "upper" => (cycle(["upperLetter", "lowerLetter", "lowerRoman"]), own),
        "roman" => (cycle(["upperRoman", "upperLetter", "decimal"]), own),
        "zero" => (cycle(["decimalZero", "lowerLetter", "lowerRoman"]), own),
        _ => return None,
    })
}

fn preset_abstract_xml(preset: &str, abs_id: i64, name: &str) -> Option<String> {
    let multi = if preset == "outline" { "multilevel" } else { "hybridMultilevel" };
    let mut abs = format!(r#"<w:abstractNum w:abstractNumId="{abs_id}"><w:multiLevelType w:val="{multi}"/><w:name w:val="{name}"/>"#);
    for l in 0..9usize {
        let left = 720 * (l + 1);
        let (fmt, text, font, jc) = if let Some(g) = bullet_glyphs(preset) {
            let (glyph, font) = g[l % 3];
            ("bullet", glyph.to_string(), format!(r#"<w:rPr><w:rFonts w:ascii="{font}" w:hAnsi="{font}" w:hint="default"/></w:rPr>"#), "left")
        } else {
            let (f, t) = numbered_level(preset, l)?;
            (f, t, String::new(), if f == "lowerRoman" { "right" } else { "left" })
        };
        abs.push_str(&format!(
            r#"<w:lvl w:ilvl="{l}"><w:start w:val="1"/><w:numFmt w:val="{fmt}"/><w:lvlText w:val="{text}"/><w:lvlJc w:val="{jc}"/><w:pPr><w:ind w:left="{left}" w:hanging="360"/></w:pPr>{font}</w:lvl>"#
        ));
    }
    abs.push_str("</w:abstractNum>");
    Some(abs)
}

pub fn format_number(n: i64, fmt: &str) -> String {
    crate::numfmt::format(n, fmt, None)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn formats() {
        assert_eq!(format_number(4, "lowerRoman"), "iv");
        assert_eq!(format_number(28, "upperLetter"), "BB");
        assert_eq!(format_number(3, "ordinal"), "3rd");
    }

    #[test]
    fn create_and_find() {
        let mut n = Numbering::parse(None);
        let b = n.create_list(ListKind::Bullet);
        let d = n.create_list(ListKind::Decimal);
        assert_ne!(b, d);
        assert!(n.is_bullet(b));
        assert_eq!(n.find_list(ListKind::Decimal), Some(d));
        assert_eq!(n.level(d, 1).unwrap().fmt, "lowerLetter");
    }

    #[test]
    fn presets_are_named_and_recognised() {
        let mut n = Numbering::parse(None);
        for style in LIST_STYLES {
            let num = n.shared_num(style).unwrap();
            assert_eq!(n.preset_of(num), Some(*style), "{style}");
            // One definition and one shared num per preset.
            assert_eq!(n.shared_num(style), Some(num));
        }
        let checked = n.shared_num("checklist_checked").unwrap();
        assert_eq!(n.preset_of(checked), Some("checklist_checked"));
        assert_eq!(n.checkbox_at(checked, 4), Some(true));
        let (open, disc) = (n.shared_num("checklist").unwrap(), n.shared_num("disc").unwrap());
        assert_eq!(n.checkbox_at(open, 0), Some(false));
        assert_eq!(n.checkbox_at(disc, 0), None);
        // Plain lists made by create_list have no preset.
        let plain = n.create_list(ListKind::Bullet);
        assert_eq!(n.preset_of(plain), None);
        let found = n.find_list(ListKind::Bullet).unwrap();
        assert!(n.is_bullet(found) && n.checkbox_at(found, 0).is_none(), "checkboxes are not plain bullets");
        // Schema order: the name follows multiLevelType and precedes the levels.
        let doc = n.doc.as_ref().unwrap();
        let abs = doc.root.children_named("w:abstractNum").find(|a| a.child("w:name").is_some_and(|e| e.attr("w:val") == Some("Docxtor outline"))).unwrap();
        let names: Vec<_> = abs.elements().map(|e| e.name.as_str()).take(3).collect();
        assert_eq!(names, ["w:multiLevelType", "w:name", "w:lvl"]);
        assert_eq!(abs.child("w:multiLevelType").unwrap().attr("w:val"), Some("multilevel"));
        let last = doc.root.elements().map(|e| e.name.as_str()).collect::<Vec<_>>();
        let first_num = last.iter().position(|n| *n == "w:num").unwrap();
        assert!(last[first_num..].iter().all(|n| *n == "w:num"), "abstractNums stay before nums: {last:?}");
    }

    #[test]
    fn fresh_and_restarted_nums() {
        let mut n = Numbering::parse(None);
        let a = n.fresh_num("decimal", &[]).unwrap();
        let b = n.fresh_num("decimal", &[(2, 4)]).unwrap();
        assert_ne!(a, b);
        assert_eq!(n.abstract_of(a), n.abstract_of(b));
        assert_eq!(n.start_overrides(a), [(0, 1)]);
        assert_eq!(n.start_overrides(b), [(0, 1), (2, 4)]);
        assert!(n.has_overrides(a));
        // The shared num has no overrides, so it is distinct from fresh ones.
        let shared = n.shared_num("decimal").unwrap();
        assert!(!n.has_overrides(shared));
        let r = n.restarted_num(b, 1, 7).unwrap();
        assert_eq!(n.abstract_of(r), n.abstract_of(b));
        assert_eq!(n.start_overrides(r), [(0, 1), (1, 7), (2, 4)]);
        let r2 = n.restarted_num(r, 1, 3).unwrap();
        assert_eq!(n.start_overrides(r2), [(0, 1), (1, 3), (2, 4)]);
        let r3 = n.restarted_num(shared, 0, 5).unwrap();
        assert_eq!(n.start_overrides(r3), [(0, 5)]);
    }
}
