//! Insert, Cover page and Insert, Header and Footer: Docxtor's ready-made designs (see `gallery`) put into the document,
//! a cover page replaced or removed, and a small document of every design for the gallery to show. A cover page is
//! a block-level content control of the Cover Pages gallery at the very start of the document, as Word writes it;
//! Word's own covers count too.

use super::*;
use crate::gallery::xml::{Env, Field, COVER_NS, COVER_STORE};
use crate::gallery::{cover, headers, xml as gx};
use crate::fragment::{self, Fragment};
use crate::shapes::theme::ShapeTheme;
use std::collections::BTreeMap;

/// The placeholder texts and words of the page parts in the interface language, by key: the `Field` keys
/// (`title`, `date`...), `page`, `of`, `submitted_by`, `course`, `supervisor`, `abstract`, and `dateFormat` and `lang`
/// for the date pickers. Anything missing is English.
pub type GalleryLabels = BTreeMap<String, String>;

/// A design a gallery shows.
#[derive(Debug, Clone, Serialize)]
pub struct GalleryDesign {
    pub id: String,
    /// The English name; the interface translates it.
    pub name: String,
}

/// What the builders read and make: the values a control shows now, and fresh ids.
struct DocEnv<'a> {
    doc: &'a mut Document,
    values: HashMap<Field, String>,
    date: Option<String>,
    labels: &'a GalleryLabels,
    sdt: i64,
}

impl<'a> DocEnv<'a> {
    fn new(doc: &'a mut Document, values: HashMap<Field, String>, date: Option<String>, labels: &'a GalleryLabels) -> Self {
        let sdt = doc.next_sdt_id();
        DocEnv { doc, values, date, labels, sdt }
    }
}

impl Env for DocEnv<'_> {
    fn sdt_id(&mut self) -> i64 {
        let id = self.sdt;
        self.sdt += 1;
        id
    }
    fn drawing_id(&mut self) -> u32 {
        self.doc.drawing_id += 1;
        self.doc.drawing_id
    }
    fn placeholder(&mut self, text: &str) -> String {
        let main = self.doc.main_path.clone();
        crate::controls::ensure_placeholder(&mut self.doc.pkg, &main, text, None)
    }
    fn value(&self, field: Field) -> String {
        self.values.get(&field).cloned().unwrap_or_default()
    }
    fn date(&self) -> Option<String> {
        self.date.clone()
    }
    fn label(&self, field: Field) -> String {
        self.labels.get(field.key()).filter(|s| !s.trim().is_empty()).cloned().unwrap_or_else(|| field.placeholder().into())
    }
    fn word(&self, key: &str, english: &str) -> String {
        self.labels.get(key).filter(|s| !s.trim().is_empty()).cloned().unwrap_or_else(|| english.into())
    }
    fn date_format(&self) -> (String, String) {
        let fmt = self.labels.get("dateFormat").filter(|s| !s.is_empty()).cloned().unwrap_or_else(|| "MMMM d, yyyy".into());
        let lang = self.labels.get("lang").filter(|s| !s.is_empty()).cloned().unwrap_or_else(|| "en-US".into());
        (fmt, lang)
    }
}

/// Every field a design can show, for reading their values up front.
const FIELDS: [Field; 6] = [Field::Title, Field::Subtitle, Field::Author, Field::Company, Field::Date, Field::Abstract];

/// The tag Docxtor puts on a cover page it turned "Different first page" on for, so Remove and Replace take back what
/// the cover added and leave a first-page setting the person made.
const TITLE_PAGE_TAG: &str = "DocxtorCoverTitlePage";

/// Whether the block is a cover page that turned the different first page on.
fn added_title_page(b: &Block) -> bool {
    matches!(b, Block::Container(c) if is_cover(b) && c.shell.child("w:sdtPr").and_then(|p| p.child("w:tag")).and_then(|t| t.attr("w:val")) == Some(TITLE_PAGE_TAG))
}

/// A run holding the first watermark of a header's blocks (only the watermark, whatever else the run held).
fn watermark_run_of(blocks: &[Block]) -> Option<Run> {
    let pict = marks::first_watermark(blocks)?.clone();
    Some(marks::watermark_run(pict))
}

/// The content control of the Cover Pages gallery, if a block is one.
fn is_cover(b: &Block) -> bool {
    matches!(b, Block::Container(c) if c.shell.find("w:docPartGallery").and_then(|g| g.attr("w:val")).is_some_and(|v| v.eq_ignore_ascii_case("Cover Pages")))
}

impl Document {
    /// Docxtor's cover pages, or its headers or footers (`kind`: `cover`, `header`, `footer`).
    pub fn page_gallery(kind: &str) -> Vec<GalleryDesign> {
        let pick = |id: &str, name: &str| GalleryDesign { id: id.into(), name: name.into() };
        match kind {
            "cover" => cover::DESIGNS.iter().map(|d| pick(d.id, d.name)).collect(),
            "footer" => headers::FOOTERS.iter().map(|d| pick(d.id, d.name)).collect(),
            _ => headers::HEADERS.iter().map(|d| pick(d.id, d.name)).collect(),
        }
    }

    /// Whether the document starts with a cover page.
    pub fn has_cover_page(&self) -> bool {
        self.body.first().is_some_and(is_cover)
    }

    /// A header or footer asked for from inside the cover page goes to the section after it (its first paragraph): a
    /// cover with a section of its own ("Centered") would otherwise hold the only reference to it, and take it away when
    /// the cover goes.
    pub(super) fn past_cover(&self, section: Option<Id>) -> Option<Id> {
        let (Some(s), Some(first)) = (section, self.body.first().filter(|b| is_cover(b))) else { return section };
        if find_block(std::slice::from_ref(first), s.0).is_none() {
            return section;
        }
        paragraph_order(self.body.iter().skip(1)).first().map(|p| Id(*p)).or(section)
    }

    /// The values the fields show now: the document's properties and the cover page date.
    fn gallery_values(&self) -> (HashMap<Field, String>, Option<String>) {
        let mut values = HashMap::new();
        for f in FIELDS {
            if let Some((store, xpath, prefixes)) = f.binding() {
                if let Some(v) = self.bound_get(store, xpath, prefixes) {
                    values.insert(f, v);
                }
            }
        }
        let date = values.get(&Field::Date).filter(|d| !d.trim().is_empty()).cloned();
        (values, date)
    }

    /// The paper and margins of the first section.
    fn first_geom(&self) -> cover::Geom {
        let first = self.section_refs().first().and_then(|r| r.1).cloned().or_else(|| self.final_sect_pr());
        let n = |tag: &str, a: &str, d: i64| first.as_ref().and_then(|s| s.child(tag)).and_then(|e| e.attr(a)).and_then(|v| v.parse::<f64>().ok()).map(|v| v as i64).unwrap_or(d);
        cover::Geom { w: n("w:pgSz", "w:w", 12240), h: n("w:pgSz", "w:h", 15840), ml: n("w:pgMar", "w:left", 1440), mr: n("w:pgMar", "w:right", 1440), mt: n("w:pgMar", "w:top", 1440), mb: n("w:pgMar", "w:bottom", 1440) }
    }

    /// Word keeps a cover page's date and abstract in a custom XML part of its own; make it when a cover needs it.
    fn ensure_cover_props(&mut self) {
        if crate::binding::find_part(&self.pkg, COVER_STORE).is_some() {
            return;
        }
        let mut n = 1;
        let path = loop {
            let name = format!("customXml/item{n}.xml");
            if !self.pkg.has(&name) && !self.pkg.has(&format!("customXml/itemProps{n}.xml")) {
                break name;
            }
            n += 1;
        };
        let props = format!("customXml/itemProps{n}.xml");
        let item = format!(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<CoverPageProperties xmlns="{COVER_NS}"><PublishDate></PublishDate><Abstract></Abstract><CompanyAddress></CompanyAddress><CompanyPhone></CompanyPhone><CompanyFax></CompanyFax><CompanyEmail></CompanyEmail></CoverPageProperties>"#);
        let ds = format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="no"?>
<ds:datastoreItem ds:itemID="{COVER_STORE}" xmlns:ds="http://schemas.openxmlformats.org/officeDocument/2006/customXml"><ds:schemaRefs><ds:schemaRef ds:uri="{COVER_NS}"/></ds:schemaRefs></ds:datastoreItem>"#
        );
        self.pkg.set(&path, item.into_bytes());
        self.pkg.set(&props, ds.into_bytes());
        self.pkg.add_rel(&path, crate::citations::REL_CUSTOM_XML_PROPS, &format!("itemProps{n}.xml"), false);
        self.pkg.ensure_override(&props, "application/vnd.openxmlformats-officedocument.customXmlProperties+xml");
        self.pkg.ensure_default("xml", "application/xml");
        let main = self.main_path.clone();
        self.pkg.add_rel(&main, crate::citations::REL_CUSTOM_XML, &crate::citations::relative_to(&main, &path), false);
    }

    /// Build a design's markup with this document's theme, ids and values.
    fn build_page_part(&mut self, kind: &str, design: &str, labels: &GalleryLabels, text_w: i64, style: &str) -> Result<(String, Option<cover::SectionTweak>), EditError> {
        // The boxes of a design show placeholder text.
        self.ensure_placeholder_style();
        let th = ShapeTheme::load(&self.pkg);
        let geom = self.first_geom();
        let (values, date) = self.gallery_values();
        // The designs write Word's id for the placeholder style; the file may define that style under another id.
        let placeholder = self.styles.lookup("PlaceholderText").unwrap_or_else(|| "PlaceholderText".into());
        let mut env = DocEnv::new(self, values, date, labels);
        let unknown = || EditError::Invalid(format!("there is no {kind} design called '{design}'"));
        let rename = |xml: String| xml.replace(r#"<w:rStyle w:val="PlaceholderText"/>"#, &format!(r#"<w:rStyle w:val="{}"/>"#, crate::gallery::xml::esc(&placeholder)));
        match kind {
            "cover" => {
                let built = cover::build(design, &mut env, &th, geom).ok_or_else(unknown)?;
                Ok((rename(built.xml), built.section))
            }
            _ => Ok((rename(headers::build(design, kind == "footer", &mut env, &th, text_w, style).ok_or_else(unknown)?), None)),
        }
    }

    /// The cover's own section: the first section's paper and margins, with the design's tweaks and a first page of
    /// its own, so no header, footer or page number shows on it.
    fn cover_section(&self, tweak: &cover::SectionTweak) -> Element {
        let first = self.section_refs().first().and_then(|r| r.1).cloned().or_else(|| self.final_sect_pr()).unwrap_or_else(|| Element::new("w:sectPr"));
        let mut s = Element::new("w:sectPr");
        for tag in ["w:pgSz", "w:pgMar"] {
            if let Some(e) = first.child(tag) {
                s.children.push(Node::Element(e.clone()));
            }
        }
        if let Some(v) = tweak.v_align {
            crate::props::put_ordered(&mut s, Element::new("w:vAlign").with_attr("w:val", v), SECT_ORDER);
        }
        crate::props::put_ordered(&mut s, Element::new("w:titlePg"), SECT_ORDER);
        s
    }

    /// Insert, Cover page: one of Docxtor's designs, or a saved one (`fragment`), at the start of the document, in place of
    /// the cover page there. A design that ends with a page break turns on a different first page for the first section,
    /// so the cover has no header, footer or page number.
    pub(super) fn op_insert_cover_page(&mut self, design: Option<&str>, fragment: Option<&Fragment>, labels: &GalleryLabels) -> Result<OpResult, EditError> {
        if self.active_part != self.main_path {
            return Err(EditError::Invalid("a cover page goes in the main text".into()));
        }
        // The cover being replaced gives back the different first page it asked for; the new one asks again if it needs it.
        if self.remove_cover_blocks() == Some(true) {
            self.drop_cover_title_page()?;
        }
        let (blocks, own_section) = match (design, fragment) {
            (Some(d), _) => {
                let (mut xml, section) = self.build_page_part("cover", d, labels, 0, "")?;
                if xml.contains(COVER_STORE) {
                    self.ensure_cover_props();
                }
                if let Some(t) = &section {
                    xml = xml.replace(cover::SECTION_MARK, &self.cover_section(t).to_xml());
                } else {
                    xml = xml.replace(cover::SECTION_MARK, "");
                }
                let els = gx::parse(&xml).map_err(EditError::Invalid)?;
                let wrapper = Element { children: els.into_iter().map(Node::Element).collect(), ..Element::new("w:body") };
                (parse_blocks(&wrapper, &mut self.ids), section.is_some())
            }
            (None, Some(_)) => {
                let shell = format!(
                    r#"<w:sdt><w:sdtPr><w:id w:val="{}"/><w:docPartObj><w:docPartGallery w:val="Cover Pages"/><w:docPartUnique/></w:docPartObj></w:sdtPr><w:sdtEndPr/><w:sdtContent><w:p/>{}</w:sdtContent></w:sdt>"#,
                    self.next_sdt_id(),
                    gx::page_break()
                );
                let els = gx::parse(&shell).map_err(EditError::Invalid)?;
                let wrapper = Element { children: els.into_iter().map(Node::Element).collect(), ..Element::new("w:body") };
                (parse_blocks(&wrapper, &mut self.ids), false)
            }
            (None, None) => return Err(EditError::Invalid("choose a cover page design".into())),
        };
        crate::shapes::build::ensure_root_ns(&mut self.doc_xml.root);
        self.ensure_ns("w14", "http://schemas.microsoft.com/office/word/2010/wordml");
        let mut created = Vec::new();
        for b in &blocks {
            walk_paragraphs(std::slice::from_ref(b), &mut |p| created.push(p.id));
        }
        let created_blocks = blocks.len();
        for (k, b) in blocks.into_iter().enumerate() {
            self.body.insert(k, b);
        }
        if !own_section {
            let first = created.first().map(|&p| Id(p));
            let mut added = false;
            self.with_sect(first, &mut |s| {
                if s.child("w:titlePg").is_none() {
                    crate::props::put_ordered(s, Element::new("w:titlePg"), SECT_ORDER);
                    added = true;
                }
            })?;
            if added {
                self.mark_cover_title_page();
            }
        } else {
            // The cover has a section of its own: the text after it starts on a new page, whatever its first section said.
            let after = paragraph_order(self.body.iter().skip(created_blocks)).first().map(|&p| Id(p));
            self.with_sect(after, &mut |s| {
                if s.child("w:type").and_then(|t| t.attr("w:val")) == Some("continuous") {
                    s.remove_children("w:type");
                }
            })?;
        }
        self.touch_all();
        let mut result = OpResult { created: created.clone(), caret: created.first().map(|&p| Pos::new(p, 0)), ..Default::default() };
        if let (None, Some(frag)) = (design, fragment) {
            let at = Pos::new(created[0], 0);
            let r = self.op_insert_fragment(at, frag, fragment::PasteMode::Keep)?;
            result.created.extend(r.created);
        }
        // Controls bound to the properties show their values.
        self.sync_bound_controls();
        Ok(result)
    }

    /// Take the cover page out (Word's Remove current cover page). Returns None when there was none, else whether
    /// it had turned the different first page on.
    fn remove_cover_blocks(&mut self) -> Option<bool> {
        if !self.has_cover_page() {
            return None;
        }
        let added = added_title_page(&self.body[0]);
        self.body.remove(0);
        self.touch_all();
        Some(added)
    }

    /// Take back the different first page a cover turned on, unless the first section has a first-page header or
    /// footer of its own by now.
    fn drop_cover_title_page(&mut self) -> Result<(), EditError> {
        let first = self.section_refs().first().and_then(|r| r.0).map(Id);
        self.with_sect(first, &mut |s| {
            let own = s.elements().any(|e| matches!(e.name.as_str(), "w:headerReference" | "w:footerReference") && e.attr("w:type") == Some("first"));
            if !own {
                s.remove_children("w:titlePg");
            }
        })
    }

    /// Note on the cover (the first block) that Docxtor turned the different first page on.
    fn mark_cover_title_page(&mut self) {
        let Some(Block::Container(c)) = self.body.first_mut() else { return };
        let Some(pr) = c.shell.child_mut("w:sdtPr") else { return };
        // Schema order: `w:rPr`, `w:alias`, then `w:tag`, then `w:id`.
        let at = pr.children.iter().position(|n| !matches!(n, Node::Element(e) if matches!(e.name.as_str(), "w:rPr" | "w:alias"))).unwrap_or(pr.children.len());
        pr.children.retain(|n| !matches!(n, Node::Element(e) if e.name == "w:tag"));
        pr.children.insert(at.min(pr.children.len()), Node::Element(Element::new("w:tag").with_attr("w:val", TITLE_PAGE_TAG)));
    }

    pub(super) fn op_remove_cover_page(&mut self) -> Result<OpResult, EditError> {
        let Some(added) = self.remove_cover_blocks() else {
            return Err(EditError::Invalid("the document has no cover page".into()));
        };
        // The different first page the cover asked for goes too (not one the person set before, or since).
        if added {
            self.drop_cover_title_page()?;
        }
        Ok(OpResult::default())
    }

    /// Insert, Header or Insert, Footer: one of Docxtor's designs in place of what the section's (default) header or footer
    /// holds; it is made when the section has none.
    pub(super) fn op_insert_page_part(&mut self, kind: HeaderKind, design: &str, section: Option<Id>, labels: &GalleryLabels) -> Result<OpResult, EditError> {
        let made = self.add_header_footer_variant(kind, "default", self.past_cover(section))?;
        let pid = made.caret.map(|c| c.p.0).ok_or_else(|| EditError::Invalid("the header or footer was not made".into()))?;
        let si = self.stories.iter().position(|s| find_para(&s.blocks, pid).is_some()).ok_or_else(|| EditError::Invalid("the header or footer was not found".into()))?;
        let (name, gallery) = match kind {
            HeaderKind::Header => ("Header", "header"),
            HeaderKind::Footer => ("Footer", "footer"),
        };
        let style = self.styles.ensure_builtin(name).unwrap_or_else(|| name.into());
        let sect = self.sect_of(section)?;
        let n = |tag: &str, a: &str, d: i64| sect.as_ref().and_then(|s| s.child(tag)).and_then(|e| e.attr(a)).and_then(|v| v.parse::<f64>().ok()).map(|v| v as i64).unwrap_or(d);
        let text_w = (n("w:pgSz", "w:w", 12240) - n("w:pgMar", "w:left", 1440) - n("w:pgMar", "w:right", 1440)).max(1440);
        let (xml, _) = self.build_page_part(gallery, design, labels, text_w, &style)?;
        if xml.contains(COVER_STORE) {
            self.ensure_cover_props();
        }
        let els = gx::parse(&xml).map_err(EditError::Invalid)?;
        let wrapper = Element { children: els.into_iter().map(Node::Element).collect(), ..Element::new("w:hdr") };
        let mut blocks = parse_blocks(&wrapper, &mut self.ids);
        // A watermark of the header is the document's, not part of what the design replaces: it stays.
        if kind == HeaderKind::Header {
            if let Some(run) = watermark_run_of(&self.stories[si].blocks) {
                marks::put_watermark(&mut blocks, None, run, self.ids.fresh(), &style);
            }
        }
        let created: Vec<NodeId> = blocks.iter().map(|b| b.id()).collect();
        self.story_gen += 1;
        let gen = self.story_gen;
        let story = &mut self.stories[si];
        story.blocks = blocks;
        story.gen = gen;
        let part = story.part.clone();
        if let Some(d) = self.story_parts.get_mut(&part) {
            crate::shapes::build::ensure_root_ns(&mut d.root);
        }
        self.dirty_parts.insert(part);
        self.stories_touched = true;
        self.sync_bound_controls();
        Ok(OpResult { caret: created.first().map(|&p| Pos::new(p, 0)), created, ..Default::default() })
    }

    /// A small document of every design of a gallery (`cover`, `header`, `footer`), one to a page, in this document's
    /// theme, styles, paper and properties, for the gallery to show: what each will look like here.
    pub fn page_gallery_preview(&self, kind: &str, labels: &GalleryLabels) -> Result<Vec<u8>, String> {
        let (theme, set) = self.design_snapshot();
        // A document with no styles of its own has nothing to carry over but the theme.
        let set = (!set.styles.trim().is_empty()).then_some(set);
        let mut d = Document::new_blank_with_design(Some(&theme), set.as_ref()).map_err(|e| e.to_string())?;
        // The properties are saved with it: opening it shows them in the boxes.
        d.core = self.core.clone();
        d.app = self.app.clone();
        d.core_dirty = d.core.is_some();
        d.app_dirty = d.app.is_some();
        if let Some(part) = crate::binding::find_part(&self.pkg, COVER_STORE) {
            if let Some(bytes) = self.pkg.get(&part) {
                d.ensure_cover_props();
                if let Some(own) = crate::binding::find_part(&d.pkg, COVER_STORE) {
                    d.pkg.set(&own, bytes.to_vec());
                }
            }
        }
        // The first section's paper and margins.
        let geom = self.first_geom();
        d.with_sect(None, &mut |s| {
            crate::props::put_ordered(s, Element::new("w:pgSz").with_attr("w:w", geom.w.to_string()).with_attr("w:h", geom.h.to_string()), SECT_ORDER);
            if let Some(m) = s.child_mut("w:pgMar") {
                for (a, v) in [("w:top", geom.mt), ("w:bottom", geom.mb), ("w:left", geom.ml), ("w:right", geom.mr)] {
                    m.set_attr(a, v.to_string());
                }
            }
        })
        .map_err(|e| e.to_string())?;
        let text_w = geom.text_w();
        let mut blocks: Vec<Block> = Vec::new();
        let header_style = d.styles.ensure_builtin("Header").unwrap_or_else(|| "Header".into());
        let footer_style = d.styles.ensure_builtin("Footer").unwrap_or_else(|| "Footer".into());
        let list = Self::page_gallery(kind);
        for (i, g) in list.iter().enumerate() {
            let style = if kind == "footer" { &footer_style } else { &header_style };
            let (mut xml, section) = d.build_page_part(kind, &g.id, labels, text_w, style).map_err(|e| e.to_string())?;
            if kind == "cover" {
                xml = match &section {
                    Some(t) => xml.replace(cover::SECTION_MARK, &d.cover_section(t).to_xml()),
                    None => xml.replace(cover::SECTION_MARK, ""),
                };
            } else if i + 1 < list.len() {
                // Each header or footer on a page of its own that counts from 1, as it would on a first page.
                let mut sect = d.final_sect_pr().unwrap_or_else(|| Element::new("w:sectPr"));
                crate::props::put_ordered(&mut sect, Element::new("w:pgNumType").with_attr("w:start", "1"), SECT_ORDER);
                xml.push_str(&format!("<w:p><w:pPr>{}</w:pPr></w:p>", sect.to_xml()));
            }
            let els = gx::parse(&xml)?;
            let wrapper = Element { children: els.into_iter().map(Node::Element).collect(), ..Element::new("w:body") };
            blocks.extend(parse_blocks(&wrapper, &mut d.ids));
        }
        if kind != "cover" {
            d.with_sect(None, &mut |s| crate::props::put_ordered(s, Element::new("w:pgNumType").with_attr("w:start", "1"), SECT_ORDER)).map_err(|e| e.to_string())?;
        }
        crate::shapes::build::ensure_root_ns(&mut d.doc_xml.root);
        let tail: Vec<Block> = d.body.drain(..).into_iter().filter(|b| matches!(b, Block::Opaque(o) if o.el.name == "w:sectPr")).collect();
        d.body = Body::from(blocks);
        d.body.extend(tail);
        d.touch_all();
        d.body_dirty = true;
        d.sync_bound_controls();
        d.save().map_err(|e| e.to_string())
    }
}
