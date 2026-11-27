//! The document's design: its theme (colours, fonts), style sets and paragraph spacing, and editing styles.

use super::*;
use crate::props::{put_ordered, ParaFmt};
use crate::style_edit::{self, StyleDefinition, StyleDetails, StyleLook, StyleSetPreview, StyleUse, STYLE_ORDER};
use crate::style_sets::{self, Spacing, StyleSetDef};
use crate::themes::{self, ColorScheme, FontScheme, ThemeDef, ThemeInfo};
use std::sync::Arc;

/// The theme part as of an op. Undo steps hold it, so a theme change is one step with the rest.
#[derive(Debug, Clone, Default)]
pub struct ThemePart {
    /// Where the document's relationship points; None when the file has no theme.
    pub path: Option<String>,
    pub bytes: Option<Vec<u8>>,
}

const DEFAULT_THEME_PATH: &str = "word/theme/theme1.xml";

impl Document {
    /// Read the theme part when a file is opened.
    pub(super) fn load_theme(pkg: &Package, path: Option<String>) -> Arc<ThemePart> {
        let bytes = path.as_deref().and_then(|p| pkg.get(p)).map(|b| b.to_vec());
        Arc::new(ThemePart { path: path.filter(|_| bytes.is_some()), bytes })
    }

    fn theme_doc(&self) -> Option<XmlDoc> {
        self.theme.bytes.as_ref().and_then(|b| xml::parse(b).ok())
    }

    /// The document's theme: its name, colours and fonts. A file without a theme reports Office's.
    pub fn theme_info(&self) -> ThemeInfo {
        match self.theme_doc() {
            Some(d) => themes::read_info(&d.root),
            None => {
                let def = themes::office_theme();
                ThemeInfo { name: def.name, colors: def.colors, fonts: def.fonts }
            }
        }
    }

    /// Put a theme part in the package (creating it, and its relationship, when the file had none), or take it out.
    fn write_theme(&mut self, part: &ThemePart) {
        self.shape_theme = Default::default();
        let main = self.main_path.clone();
        match &part.bytes {
            Some(bytes) => {
                let path = part.path.clone().unwrap_or_else(|| DEFAULT_THEME_PATH.to_string());
                if !self.pkg.has(&path) || self.theme.path.is_none() {
                    let target = relative_target(&main, &path);
                    let linked = self.pkg.rels_for(&main).map(|r| r.iter().any(|r| r.rel_type == package::REL_THEME)).unwrap_or(false);
                    if !linked {
                        self.pkg.add_rel(&main, package::REL_THEME, &target, false);
                    }
                    self.pkg.ensure_override(&path, themes::CT_THEME);
                }
                self.pkg.set(&path, bytes.clone());
                self.styles.set_theme(xml::parse(bytes).map(|d| ThemeFonts::parse(&d)).unwrap_or_default());
            }
            None => {
                if let Some(path) = self.theme.path.clone() {
                    let target = path.clone();
                    self.pkg.remove_rels_to(&main, &target);
                    self.pkg.remove_override(&path);
                    self.pkg.remove(&path);
                }
                self.styles.set_theme(ThemeFonts::default());
            }
        }
    }

    /// Bring back the theme an undo step held.
    pub(super) fn restore_theme(&mut self, now: Arc<ThemePart>) {
        if Arc::ptr_eq(&self.theme, &now) {
            return;
        }
        self.write_theme(&now);
        self.theme = now;
        self.theme_dirty = true;
        self.redraw_all();
        self.stories_touched = true;
    }

    fn commit_theme(&mut self, doc: XmlDoc) {
        let part = ThemePart { path: self.theme.path.clone().or_else(|| Some(DEFAULT_THEME_PATH.to_string())), bytes: Some(doc.to_bytes()) };
        self.write_theme(&part);
        self.theme = Arc::new(part);
        self.theme_dirty = true;
        self.redraw_all();
        self.stories_touched = true;
    }

    pub(super) fn op_set_theme(&mut self, theme: Option<&str>, definition: Option<&ThemeDef>, colors: Option<&ColorScheme>, fonts: Option<&FontScheme>) -> Result<OpResult, EditError> {
        if theme.is_none() && definition.is_none() && colors.is_none() && fonts.is_none() {
            return Err(EditError::Invalid("name a theme, or give colours or fonts".into()));
        }
        let whole: Option<ThemeDef> = match (definition, theme) {
            (Some(d), _) => Some(d.clone()),
            (None, Some(name)) => Some(themes::find_theme(name).ok_or_else(|| EditError::Invalid(format!("there is no theme called \"{name}\"")))?),
            _ => None,
        };
        let mut doc = self.theme_doc().unwrap_or_else(|| themes::new_theme_doc(&themes::office_theme()));
        let invalid = |e: String| EditError::Invalid(e);
        if let Some(t) = &whole {
            themes::apply_colors(&mut doc.root, &t.colors).map_err(invalid)?;
            themes::apply_fonts(&mut doc.root, &t.fonts).map_err(invalid)?;
            themes::set_name(&mut doc.root, &t.name);
        }
        if let Some(c) = colors {
            // A scheme named alone (`{name: "Blue Warm"}`) is one of Word's.
            let full = if c.colors.is_empty() { themes::find_colors(&c.name).ok_or_else(|| EditError::Invalid(format!("there is no colour scheme called \"{}\"", c.name)))? } else { c.clone() };
            themes::apply_colors(&mut doc.root, &full).map_err(invalid)?;
        }
        if let Some(f) = fonts {
            let full = if f.major.is_empty() && f.minor.is_empty() { themes::find_fonts(&f.name).ok_or_else(|| EditError::Invalid(format!("there is no font scheme called \"{}\"", f.name)))? } else { f.clone() };
            themes::apply_fonts(&mut doc.root, &full).map_err(invalid)?;
        }
        self.commit_theme(doc);
        Ok(OpResult::default())
    }

    // -----------------------------------------------------------------------
    // Style sets and paragraph spacing
    // -----------------------------------------------------------------------

    pub(super) fn op_set_style_set(&mut self, set: Option<&str>, definition: Option<&StyleSetDef>) -> Result<OpResult, EditError> {
        let def = match (definition, set) {
            (Some(d), _) => d.clone(),
            (None, Some(name)) => style_sets::style_set(name).ok_or_else(|| EditError::Invalid(format!("there is no style set called \"{name}\"")))?,
            (None, None) => return Err(EditError::Invalid("name a style set".into())),
        };
        self.styles.apply_style_set(&def).map_err(EditError::Invalid)?;
        self.restyled();
        Ok(OpResult::default())
    }

    pub(super) fn op_set_paragraph_spacing(&mut self, preset: Option<&str>, spacing: Option<&Spacing>) -> Result<OpResult, EditError> {
        let sp = match (spacing, preset) {
            (Some(s), _) => *s,
            (None, Some(p)) => style_sets::spacing_preset(p).ok_or_else(|| EditError::Invalid(format!("there is no spacing preset called \"{p}\"")))?,
            (None, None) => return Err(EditError::Invalid("name a spacing preset, or give the spacing".into())),
        };
        self.styles.set_default_spacing(&sp);
        self.restyled();
        Ok(OpResult::default())
    }

    /// How a style set would look in this document (its fonts, colours and theme): the title, two headings and the body text.
    pub fn style_set_preview(&self, def: &StyleSetDef) -> Result<StyleSetPreview, String> {
        let mut sheet = self.styles.clone();
        sheet.apply_style_set(def)?;
        let samples = ["Title", "Heading1", "Heading2", "Normal"]
            .iter()
            .filter_map(|id| {
                let info = sheet.get(id)?.clone();
                let (para, run) = self.look_of(&sheet, id, &info.kind);
                Some(StyleLook { id: info.id, name: style_edit::display_name(&info.name, false), kind: info.kind, quick: info.quick, hidden: false, priority: info.priority, custom: false, latent: false, link: info.link.clone(), uses: 0, run, para })
            })
            .collect();
        Ok(StyleSetPreview { id: def.id.clone(), name: def.name.clone(), description: String::new(), samples })
    }

    /// Every built-in style set, drawn in this document's theme.
    pub fn style_set_previews(&self) -> Vec<StyleSetPreview> {
        style_sets::style_sets()
            .into_iter()
            .filter_map(|(info, def)| {
                let mut p = self.style_set_preview(&def).ok()?;
                p.description = info.description;
                Some(p)
            })
            .collect()
    }

    /// A blank document with a theme and a style set already applied (what Set as default keeps). The design is part of
    /// the document from the start: nothing to undo, and the document counts as unchanged.
    pub fn new_blank_with_design(theme: Option<&ThemeDef>, set: Option<&StyleSetDef>) -> Result<Document, EditError> {
        let mut doc = Document::new_blank();
        if let Some(t) = theme {
            doc.op_set_theme(None, Some(t), None, None)?;
        }
        if let Some(s) = set {
            doc.op_set_style_set(None, Some(s))?;
        }
        doc.flush_parts();
        doc.undo.clear();
        doc.redo.clear();
        doc.undo_labels.clear();
        doc.redo_labels.clear();
        doc.changed.clear();
        Ok(doc)
    }

    /// The design as it is now, as a theme and a style set: what Set as default keeps for new documents.
    pub fn design_snapshot(&self) -> (ThemeDef, StyleSetDef) {
        let info = self.theme_info();
        let theme = ThemeDef { id: String::new(), name: info.name, colors: info.colors, fonts: info.fonts };
        let mut styles = String::new();
        for id in ["Normal", "Title", "Subtitle", "Heading1", "Heading2", "Heading3", "Heading4", "Heading5", "Heading6", "Heading7", "Heading8", "Heading9", "Quote"] {
            if let Some(el) = self.styles.style_element(id) {
                styles.push_str(&el.to_xml());
            }
        }
        let sp = self.styles.doc.as_ref().and_then(|d| d.root.path(&["w:docDefaults", "w:pPrDefault", "w:pPr", "w:spacing"])).map(|s| {
            let n = |a: &str| s.attr(a).and_then(|v| v.parse::<i64>().ok());
            Spacing { before: n("w:before"), after: n("w:after"), line: n("w:line") }
        });
        (theme, StyleSetDef { id: String::new(), name: "Default".into(), styles, spacing: sp })
    }

    /// After styles change: every paragraph, header and footer may look different.
    fn restyled(&mut self) {
        // In a story the body is the story's; the main text is touched when the op returns.
        self.restyled_main |= self.parked.is_some();
        self.touch_all();
        self.stories_touched = true;
    }

    // -----------------------------------------------------------------------
    // Editing one style
    // -----------------------------------------------------------------------

    fn style_kind(&self, id: &str) -> String {
        self.styles.get(id).map(|s| s.kind.clone()).unwrap_or_else(|| "paragraph".into())
    }

    /// A style by id or name; Word's built-in styles the file does not define yet are added.
    fn style_ref(&mut self, name: &str) -> Result<String, EditError> {
        self.styles.lookup(name).or_else(|| self.styles.ensure_builtin(name)).ok_or_else(|| EditError::Invalid(format!("there is no style called \"{name}\"")))
    }

    /// What formatting a style gets from the document defaults and the style it is based on (`based`).
    fn inherited_for(&self, kind: &str, based: Option<&str>) -> (ParaFmt, RunFmt) {
        let (bp, br) = match based {
            Some(b) if kind == "character" => (ParaFmt::default(), self.styles.char_style(b)),
            Some(b) => self.styles.para_style(Some(b)),
            None => (ParaFmt::default(), RunFmt::default()),
        };
        (self.styles.doc_ppr.overlay(&bp), self.styles.doc_rpr.overlay(&br))
    }

    pub(super) fn op_set_style_definition(&mut self, style: Option<&str>, def: &StyleDefinition) -> Result<OpResult, EditError> {
        match style.filter(|s| !s.is_empty()) {
            Some(name) => {
                let id = self.style_ref(name)?;
                self.modify_style(&id, def)?;
            }
            None => {
                self.new_style(def)?;
            }
        }
        self.restyled();
        Ok(OpResult::default())
    }

    /// Does `candidate`'s chain of base styles pass through `id`?
    fn builds_on(&self, candidate: &str, id: &str) -> bool {
        self.styles.chain_ids(candidate).iter().any(|c| c == id)
    }

    fn modify_style(&mut self, id: &str, def: &StyleDefinition) -> Result<(), EditError> {
        let kind = self.style_kind(id);
        let (custom, current_name) = {
            let el = self.styles.style_element(id).ok_or_else(|| EditError::Invalid(format!("there is no style \"{id}\"")))?;
            (el.attr("w:customStyle").is_some_and(|v| v == "1" || v == "true"), style_edit::val(el, "w:name").unwrap_or(id).to_string())
        };
        // The name: only a style a person made can be renamed, and names are unique.
        let new_name = match def.name.as_deref().map(str::trim) {
            Some(n) if n != current_name => {
                if n.is_empty() {
                    return Err(EditError::Invalid("a style needs a name".into()));
                }
                if !custom {
                    return Err(EditError::Invalid(format!("\"{current_name}\" is one of Word's styles and keeps its name")));
                }
                let clash = self.styles.list().iter().any(|s| s.id != id && s.name.eq_ignore_ascii_case(n));
                if clash {
                    return Err(EditError::Invalid(format!("there is already a style called \"{n}\"")));
                }
                Some(n.to_string())
            }
            _ => None,
        };
        let based_on: Option<Option<String>> = match def.based_on.as_deref() {
            None => None,
            Some("") => Some(None),
            Some(b) => {
                let b = self.style_ref(b)?;
                if b == id || self.builds_on(&b, id) {
                    return Err(EditError::Invalid("a style can't be based on itself or on a style that is based on it".into()));
                }
                if self.style_kind(&b) != kind {
                    return Err(EditError::Invalid(format!("a {kind} style can only be based on another {kind} style")));
                }
                Some(Some(b))
            }
        };
        let next: Option<Option<String>> = match def.next.as_deref() {
            None => None,
            Some("") => Some(None),
            Some(n) => {
                let n = self.style_ref(n)?;
                Some(if n == id { None } else { Some(n) })
            }
        };
        if let Some(n) = &next {
            if n.is_some() && kind != "paragraph" {
                return Err(EditError::Invalid("only a paragraph style has a style for the following paragraph".into()));
            }
        }
        let based_now = match &based_on {
            Some(b) => b.clone(),
            None => self.styles.get(id).and_then(|s| s.based_on.clone()),
        };
        let (inh_p, inh_r) = self.inherited_for(&kind, based_now.as_deref());
        // A linked pair looks alike: the character style gets the same run formatting.
        let partner = self.styles.style_element(id).and_then(|e| style_edit::val(e, "w:link")).map(String::from).filter(|l| self.styles.exists(l));
        let partner_inh = partner.as_deref().map(|p| self.inherited_for("character", self.styles.get(p).and_then(|s| s.based_on.clone()).as_deref()).1);
        let lang = def.lang.as_deref().map(|l| if l.is_empty() { Ok(String::new()) } else { crate::docprops::normalize_lang(l) }).transpose().map_err(EditError::Invalid)?;
        self.styles.edit_xml(|root| {
            let apply_run = |style: &mut Element, inh: &RunFmt| {
                if def.rpr.is_some() || lang.is_some() {
                    let rpr = style_edit::props_mut(style, "w:rPr");
                    if let Some(patch) = &def.rpr {
                        patch.apply(rpr, inh);
                    }
                    if let Some(l) = &lang {
                        rpr.remove_children("w:lang");
                        if !l.is_empty() {
                            crate::props::put_rpr(rpr, Element::new("w:lang").with_attr("w:val", l));
                        }
                    }
                    style_edit::drop_if_empty(style, "w:rPr");
                }
            };
            if let Some(partner_id) = &partner {
                if let Some(p) = root.elements_mut().find(|e| e.name == "w:style" && e.attr("w:styleId") == Some(partner_id.as_str())) {
                    apply_run(p, partner_inh.as_ref().unwrap_or(&inh_r));
                }
            }
            let Some(s) = root.elements_mut().find(|e| e.name == "w:style" && e.attr("w:styleId") == Some(id)) else { return };
            if let Some(n) = &new_name {
                style_edit::set_val(s, "w:name", Some(n));
                if let Some(partner_id) = &partner {
                    let pname = format!("{n} Char");
                    if let Some(p) = root.elements_mut().find(|e| e.name == "w:style" && e.attr("w:styleId") == Some(partner_id.as_str())) {
                        style_edit::set_val(p, "w:name", Some(&pname));
                    }
                }
            }
            let Some(s) = root.elements_mut().find(|e| e.name == "w:style" && e.attr("w:styleId") == Some(id)) else { return };
            if let Some(b) = &based_on {
                style_edit::set_val(s, "w:basedOn", b.as_deref());
            }
            if let Some(n) = &next {
                style_edit::set_val(s, "w:next", n.as_deref());
            }
            if let Some(q) = def.quick {
                style_edit::set_flag(s, "w:qFormat", q);
            }
            if let Some(a) = def.auto_update {
                style_edit::set_flag(s, "w:autoRedefine", a);
            }
            if let Some(h) = def.hidden {
                style_edit::set_flag(s, "w:semiHidden", h);
                if !h {
                    style_edit::set_flag(s, "w:unhideWhenUsed", false);
                }
            }
            if let Some(p) = def.priority {
                style_edit::set_val(s, "w:uiPriority", Some(&p.to_string()));
            }
            if let Some(patch) = &def.ppr {
                let ppr = style_edit::props_mut(s, "w:pPr");
                // A style has no style of its own to name.
                let mut patch = patch.clone();
                patch.style = None;
                patch.apply_with(ppr, &inh_p);
                style_edit::drop_if_empty(s, "w:pPr");
            }
            apply_run(s, &inh_r);
        });
        Ok(())
    }

    /// Add a style: a paragraph style, a character style, or both linked. Returns the paragraph (or character) style's id.
    fn new_style(&mut self, def: &StyleDefinition) -> Result<String, EditError> {
        let name = def.name.as_deref().map(str::trim).unwrap_or_default().to_string();
        if name.is_empty() {
            return Err(EditError::Invalid("a style needs a name".into()));
        }
        if self.styles.list().iter().any(|s| s.name.eq_ignore_ascii_case(&name)) {
            return Err(EditError::Invalid(format!("there is already a style called \"{name}\"")));
        }
        let kind = def.kind.as_deref().unwrap_or("paragraph");
        if !matches!(kind, "paragraph" | "character" | "linked") {
            return Err(EditError::Invalid(format!("a new style is a paragraph, character or linked style, not \"{kind}\"")));
        }
        let ty = if kind == "character" { "character" } else { "paragraph" };
        let id = style_edit::style_id_for(&name, &|i| self.styles.exists(i));
        let base: Option<String> = match def.based_on.as_deref() {
            Some("") => None,
            Some(b) => Some(self.style_ref(b)?),
            None if ty == "paragraph" => self.styles.default_para.clone().or_else(|| self.styles.ensure_builtin("Normal")),
            None => self.styles.default_char.clone().or_else(|| self.styles.exists("DefaultParagraphFont").then(|| "DefaultParagraphFont".to_string())),
        };
        if let Some(b) = &base {
            if self.style_kind(b) != ty {
                return Err(EditError::Invalid(format!("a {ty} style can only be based on another {ty} style")));
            }
        }
        let char_id = (kind == "linked").then(|| style_edit::style_id_for(&format!("{id}Char"), &|i| self.styles.exists(i) || i == id));
        let mut el = Element::new("w:style").with_attr("w:type", ty).with_attr("w:customStyle", "1").with_attr("w:styleId", id.clone());
        style_edit::set_val(&mut el, "w:name", Some(&name));
        style_edit::set_val(&mut el, "w:basedOn", base.as_deref());
        if let Some(c) = &char_id {
            style_edit::set_val(&mut el, "w:link", Some(c));
        }
        let mut char_el = char_id.as_ref().map(|c| {
            let mut e = Element::new("w:style").with_attr("w:type", "character").with_attr("w:customStyle", "1").with_attr("w:styleId", c.clone());
            style_edit::set_val(&mut e, "w:name", Some(&format!("{name} Char")));
            style_edit::set_val(&mut e, "w:basedOn", self.styles.default_char.clone().or_else(|| self.styles.exists("DefaultParagraphFont").then(|| "DefaultParagraphFont".to_string())).as_deref());
            style_edit::set_val(&mut e, "w:link", Some(&id));
            style_edit::set_val(&mut e, "w:uiPriority", Some("1"));
            e
        });
        self.styles.edit_xml(|root| {
            root.children.push(Node::Element(el));
            if let Some(c) = char_el.take() {
                root.children.push(Node::Element(c));
            }
        });
        // The rest is a change to the style just made.
        let mut rest = def.clone();
        rest.name = None;
        rest.kind = None;
        rest.based_on = None;
        if rest.quick.is_none() {
            rest.quick = Some(true);
        }
        // An error leaves nothing half made: the batch is undone as a whole.
        self.modify_style(&id, &rest)?;
        Ok(id)
    }

    /// Every paragraph and run holding `style_id` (as `pStyle` or `rStyle`) set to `to` (None removes the reference).
    fn rewrite_style_refs(&mut self, style_id: &str, para_to: Option<&str>, char_to: Option<&str>) {
        fn runs(list: &mut [Inline], id: &str, to: Option<&str>) -> bool {
            let mut any = false;
            for i in list {
                match i {
                    Inline::Run(r) => {
                        if let Some(rpr) = r.rpr.as_mut() {
                            if rpr.child("w:rStyle").and_then(|s| s.attr("w:val")) == Some(id) {
                                any = true;
                                match to {
                                    Some(t) => crate::props::put_rpr(rpr, Element::new("w:rStyle").with_attr("w:val", t)),
                                    None => {
                                        rpr.remove_children("w:rStyle");
                                    }
                                }
                            }
                        }
                    }
                    Inline::Container(c) => any |= runs(&mut c.children, id, to),
                    Inline::Opaque(_) => {}
                }
            }
            any
        }
        let go = |blocks: &mut [Block]| -> bool {
            let mut any = false;
            walk_paragraphs_mut(blocks, &mut |p| {
                if let Some(ppr) = p.ppr.as_mut() {
                    if ppr.child("w:pStyle").and_then(|s| s.attr("w:val")) == Some(style_id) {
                        any = true;
                        match para_to {
                            Some(t) => put_ppr(ppr, Element::new("w:pStyle").with_attr("w:val", t)),
                            None => {
                                ppr.remove_children("w:pStyle");
                            }
                        }
                    }
                }
                any |= runs(&mut p.inlines, style_id, char_to);
            });
            any
        };
        for id in self.body.edit_each(|s| go(s)) {
            self.touch(id);
        }
        for i in 0..self.stories.len() {
            go(&mut self.stories[i].blocks);
            self.story_gen += 1;
            self.stories[i].gen = self.story_gen;
            self.dirty_parts.insert(self.stories[i].part.clone());
        }
    }

    pub(super) fn op_delete_style(&mut self, style: &str) -> Result<OpResult, EditError> {
        let id = self.styles.lookup(style).ok_or_else(|| EditError::Invalid(format!("there is no style called \"{style}\"")))?;
        let kind = self.style_kind(&id);
        let (custom, is_default, name, based, partner) = {
            let el = self.styles.style_element(&id).expect("the style exists");
            (
                el.attr("w:customStyle").is_some_and(|v| v == "1" || v == "true"),
                el.attr("w:default").is_some_and(|v| v == "1" || v == "true"),
                style_edit::val(el, "w:name").unwrap_or(&id).to_string(),
                style_edit::val(el, "w:basedOn").map(String::from),
                style_edit::val(el, "w:link").map(String::from),
            )
        };
        if !custom || is_default {
            return Err(EditError::Invalid(format!("\"{name}\" is one of Word's styles and can't be deleted (change it instead)")));
        }
        if !matches!(kind.as_str(), "paragraph" | "character") {
            return Err(EditError::Invalid("only paragraph and character styles can be deleted here".into()));
        }
        // A linked pair goes together (the character style is hidden in Word's lists).
        let mut gone = vec![id.clone()];
        if let Some(p) = partner.filter(|p| self.styles.exists(p)) {
            let partner_custom = self.styles.style_element(&p).is_some_and(|e| e.attr("w:customStyle").is_some_and(|v| v == "1" || v == "true"));
            if partner_custom {
                gone.push(p);
            }
        }
        for g in &gone {
            let g_kind = self.style_kind(g);
            let g_based = self.styles.get(g).and_then(|s| s.based_on.clone()).filter(|b| !gone.contains(b));
            let para_to = (g_kind == "paragraph").then(|| g_based.clone()).flatten();
            let char_to = (g_kind == "character").then(|| g_based.clone()).filter(|b| b.as_deref() != self.styles.default_char.as_deref()).flatten();
            self.rewrite_style_refs(g, para_to.as_deref(), char_to.as_deref());
        }
        let fallback = based.filter(|b| !gone.contains(b));
        self.styles.edit_xml(|root| {
            // Styles built on a deleted one build on what it was built on.
            for s in root.elements_mut().filter(|e| e.name == "w:style") {
                for dep in ["w:basedOn", "w:next", "w:link"] {
                    if s.child(dep).and_then(|e| e.attr("w:val")).is_some_and(|v| gone.iter().any(|g| g == v)) {
                        let to = if dep == "w:basedOn" { fallback.clone() } else { None };
                        style_edit::set_val(s, dep, to.as_deref());
                    }
                }
            }
            root.children.retain(|n| !matches!(n, Node::Element(e) if e.name == "w:style" && e.attr("w:styleId").is_some_and(|v| gone.iter().any(|g| g == v))));
        });
        // A list level linked to the style lets go of it.
        if let Some(doc) = self.numbering.doc.as_mut() {
            fn unlink(e: &mut Element, gone: &[String]) -> bool {
                let before = e.children.len();
                if e.name == "w:lvl" {
                    e.children.retain(|n| !matches!(n, Node::Element(c) if c.name == "w:pStyle" && c.attr("w:val").is_some_and(|v| gone.iter().any(|g| g == v))));
                }
                let mut changed = e.children.len() != before;
                for c in e.elements_mut() {
                    changed |= unlink(c, gone);
                }
                changed
            }
            if unlink(&mut doc.root, &gone) {
                self.numbering.dirty = true;
                self.numbering.touch();
            }
        }
        self.restyled();
        Ok(OpResult::default())
    }

    // -----------------------------------------------------------------------
    // Styles from the formatting of a selection
    // -----------------------------------------------------------------------

    /// The formatting of the selection a style would take: the paragraph's direct properties and the run properties all
    /// the selected text has in common. A collapsed selection stands for its whole paragraph.
    fn selection_formatting(&mut self, from: Pos, to: Pos, want_para: bool) -> Result<Selected, EditError> {
        let (from, to) = self.ordered(from, to)?;
        let order = paragraph_order(&self.body);
        let a = order.iter().position(|x| *x == from.p.0).ok_or(EditError::NotFound(from.p.0))?;
        let b = order.iter().position(|x| *x == to.p.0).ok_or(EditError::NotFound(to.p.0))?;
        let pids: Vec<NodeId> = order[a..=b].to_vec();
        let collapsed = from == to;
        let mut common: Option<Vec<Element>> = None;
        for (k, pid) in pids.iter().enumerate() {
            let p = self.para_mut(*pid)?;
            let len = p.len16();
            let (s, e) = if collapsed { (0, len) } else { (if k == 0 { from.o.min(len) } else { 0 }, if a + k == b { to.o.min(len) } else { len }) };
            edit::format_range(&mut p.inlines, s, e, &mut |run| {
                let props: Vec<Element> = run.rpr.as_ref().map(style_edit::formatting_of).unwrap_or_default();
                common = Some(match common.take() {
                    None => props,
                    Some(c) => c.into_iter().filter(|x| props.iter().any(|y| style_edit::same_prop(x, y))).collect(),
                });
            });
        }
        let para = if want_para { self.para_ref(pids[0])?.ppr.as_ref().map(style_edit::formatting_of).unwrap_or_default() } else { Vec::new() };
        Ok(Selected { pids, para, run: common.unwrap_or_default() })
    }

    /// Move formatting from the selection into a style: the style carries it, the text no longer does.
    fn absorb(&mut self, style_id: &str, sel: &Selected, from: Pos, to: Pos, paragraph_style: bool) -> Result<(), EditError> {
        let Selected { pids, para, run } = sel;
        let collapsed = from == to;
        let (from, to) = self.ordered(from, to)?;
        let names_p: Vec<Name> = para.iter().map(|e| e.name.clone()).collect();
        self.styles.edit_xml(|root| {
            let Some(s) = root.elements_mut().find(|e| e.name == "w:style" && e.attr("w:styleId") == Some(style_id)) else { return };
            if paragraph_style && !para.is_empty() {
                let ppr = style_edit::props_mut(s, "w:pPr");
                for e in para {
                    crate::props::put_ppr(ppr, e.clone());
                }
            }
            if !run.is_empty() {
                let rpr = style_edit::props_mut(s, "w:rPr");
                for e in run {
                    crate::props::put_rpr(rpr, e.clone());
                }
            }
            // Keep a linked pair alike.
            if let Some(link) = style_edit::val(s, "w:link").map(String::from) {
                let rpr = s.child("w:rPr").cloned();
                if let Some(p) = root.elements_mut().find(|e| e.name == "w:style" && e.attr("w:styleId") == Some(link.as_str()) && e.attr("w:type") == Some("character")) {
                    p.remove_children("w:rPr");
                    if let Some(r) = rpr {
                        put_ordered(p, r, STYLE_ORDER);
                    }
                }
            }
        });
        for (k, pid) in pids.iter().enumerate() {
            self.touch(*pid);
            let p = self.para_mut(*pid)?;
            if paragraph_style {
                let ppr = p.ppr.get_or_insert_with(|| Element::new("w:pPr"));
                put_ppr(ppr, Element::new("w:pStyle").with_attr("w:val", style_id));
                ppr.children.retain(|n| !matches!(n, Node::Element(e) if names_p.contains(&e.name)));
                if let Some(mark) = ppr.child_mut("w:rPr") {
                    mark.children.retain(|n| !matches!(n, Node::Element(e) if run.contains(e)));
                }
                if ppr.child("w:rPr").is_some_and(|m| m.children.is_empty()) {
                    ppr.remove_children("w:rPr");
                }
            }
            let len = p.len16();
            let (s, e) = if collapsed || paragraph_style { (0, len) } else { (if k == 0 { from.o.min(len) } else { 0 }, if k == pids.len() - 1 { to.o.min(len) } else { len }) };
            edit::format_range(&mut p.inlines, s, e, &mut |r| {
                let rpr = r.rpr.get_or_insert_with(|| Element::new("w:rPr"));
                rpr.children.retain(|n| !matches!(n, Node::Element(e) if run.contains(e)));
                if !paragraph_style {
                    crate::props::put_rpr(rpr, Element::new("w:rStyle").with_attr("w:val", style_id));
                }
                if rpr.children.is_empty() {
                    r.rpr = None;
                }
            });
        }
        Ok(())
    }

    pub(super) fn op_create_style(&mut self, name: &str, kind: Option<&str>, from: Pos, to: Pos, extra: Option<&StyleDefinition>) -> Result<OpResult, EditError> {
        let kind = kind.unwrap_or("paragraph");
        if !matches!(kind, "paragraph" | "character" | "linked") {
            return Err(EditError::Invalid(format!("a new style is a paragraph, character or linked style, not \"{kind}\"")));
        }
        let paragraph = kind != "character";
        if !paragraph && from == to {
            return Err(EditError::Invalid("select the text the character style should format".into()));
        }
        let sel = self.selection_formatting(from, to, paragraph)?;
        let base = if paragraph { self.para_style_of(sel.pids[0]).filter(|s| self.styles.exists(s)) } else { None };
        let mut def = extra.cloned().unwrap_or_default();
        def.name = Some(name.to_string());
        def.kind = Some(kind.to_string());
        if def.based_on.is_none() {
            def.based_on = base;
        }
        let id = self.new_style(&def)?;
        self.absorb(&id, &sel, from, to, paragraph)?;
        self.restyled();
        Ok(OpResult::default())
    }

    pub(super) fn op_update_style(&mut self, style: &str, from: Pos, to: Pos) -> Result<OpResult, EditError> {
        let id = self.style_ref(style)?;
        let kind = self.style_kind(&id);
        if !matches!(kind.as_str(), "paragraph" | "character") {
            return Err(EditError::Invalid("only paragraph and character styles can be updated from the selection".into()));
        }
        let paragraph = kind == "paragraph";
        if !paragraph && from == to {
            return Err(EditError::Invalid("select the text the character style should match".into()));
        }
        let sel = self.selection_formatting(from, to, paragraph)?;
        self.absorb(&id, &sel, from, to, paragraph)?;
        self.restyled();
        Ok(OpResult::default())
    }

    // -----------------------------------------------------------------------
    // What the style gallery and panels read
    // -----------------------------------------------------------------------

    /// How many paragraphs (or, for character styles, runs) use each style in the main text.
    fn style_counts(&self) -> HashMap<String, usize> {
        fn runs(list: &[Inline], counts: &mut HashMap<String, usize>) {
            for i in list {
                match i {
                    Inline::Run(r) => {
                        if let Some(s) = r.rpr.as_ref().and_then(|e| e.child("w:rStyle")).and_then(|s| s.attr("w:val")) {
                            *counts.entry(s.to_string()).or_default() += 1;
                        }
                    }
                    Inline::Container(c) => runs(&c.children, counts),
                    Inline::Opaque(_) => {}
                }
            }
        }
        let mut counts: HashMap<String, usize> = HashMap::new();
        let default = self.styles.default_para.clone().unwrap_or_else(|| "Normal".into());
        walk_paragraphs(self.main_body(), &mut |p| {
            let s = p.ppr.as_ref().and_then(|pp| pp.child("w:pStyle")).and_then(|s| s.attr("w:val")).map(String::from).unwrap_or_else(|| default.clone());
            *counts.entry(s).or_default() += 1;
            runs(&p.inlines, &mut counts);
        });
        counts
    }

    fn look_of(&self, sheet: &StyleSheet, id: &str, kind: &str) -> (ParaFmt, RunFmt) {
        let (p, r) = if kind == "character" { (ParaFmt::default(), sheet.char_style(id)) } else { sheet.para_style(Some(id)) };
        let mut run = sheet.finish_run(sheet.doc_rpr.overlay(&r));
        // Theme colours (and an underline's) come from the theme the document has now, not the hex the file wrote beside them.
        if let Some(theme) = &sheet.theme.colors {
            crate::charfmt::resolve_run(&mut run, theme);
        }
        (sheet.doc_ppr.overlay(&p), run)
    }

    /// The styles to show in the gallery and the Styles panel, with how each looks: the ones the document defines, then
    /// the built-in ones it doesn't define yet (choosing one adds it).
    pub fn style_gallery(&self) -> Vec<StyleLook> {
        let counts = self.style_counts();
        let mut out = Vec::new();
        for info in self.styles.list() {
            if !matches!(info.kind.as_str(), "paragraph" | "character" | "table") {
                continue;
            }
            let custom = self.styles.style_element(&info.id).is_some_and(|e| e.attr("w:customStyle").is_some_and(|v| v == "1" || v == "true"));
            let (para, run) = self.look_of(&self.styles, &info.id, &info.kind);
            out.push(StyleLook { id: info.id.clone(), name: style_edit::display_name(&info.name, custom), kind: info.kind.clone(), quick: info.quick, hidden: info.hidden, priority: info.priority, custom, latent: false, link: info.link.clone(), uses: counts.get(&info.id).copied().unwrap_or(0), run, para });
        }
        // Word's built-in styles that are not in the file: drawn from a copy that has them (added in one go, as
        // each addition parses the styles again).
        let missing: Vec<(&str, &str)> = crate::builtin_styles::GALLERY.iter().filter(|(id, _)| !self.styles.exists(id)).map(|(id, name)| (*id, *name)).collect();
        if !missing.is_empty() {
            let mut with = self.styles.clone();
            let xml: String = missing.iter().filter_map(|(_, name)| crate::styles::builtin_style_xml(name)).collect();
            with.insert_style_xml("", &xml);
            for (id, _) in missing {
                let Some(info) = with.get(id).cloned() else { continue };
                let (para, run) = self.look_of(&with, id, &info.kind);
                out.push(StyleLook { id: info.id.clone(), name: style_edit::display_name(&info.name, false), kind: info.kind.clone(), quick: info.quick, hidden: false, priority: info.priority, custom: false, latent: true, link: info.link.clone(), uses: 0, run, para });
            }
        }
        out
    }

    /// Everything about one style, for the Modify style dialog. A built-in style the file lacks is described as Word writes it.
    pub fn style_details(&self, style: &str) -> Option<StyleDetails> {
        // Only a style the file lacks needs a copy of the sheet that has it.
        let latent = self.styles.lookup(style).is_none();
        let copy;
        let (sheet, id) = if latent {
            let mut c = self.styles.clone();
            let id = c.ensure_builtin(style)?;
            copy = c;
            (&copy, id)
        } else {
            (&self.styles, self.styles.lookup(style)?)
        };
        let info = sheet.get(&id)?.clone();
        let el = sheet.style_element(&id)?.clone();
        let (own_para, own_run) = sheet.own_format(&id).map(|(p, r)| (p.clone(), r.clone()))?;
        let (para, run) = self.look_of(sheet, &id, &info.kind);
        let uses = self.style_counts().get(&id).copied().unwrap_or(0);
        let custom = el.attr("w:customStyle").is_some_and(|v| v == "1" || v == "true");
        Some(StyleDetails {
            id,
            name: style_edit::display_name(&info.name, custom),
            kind: info.kind,
            based_on: info.based_on,
            next: info.next,
            link: style_edit::val(&el, "w:link").map(String::from),
            quick: info.quick,
            auto_update: el.child("w:autoRedefine").is_some(),
            hidden: info.hidden,
            priority: info.priority,
            custom,
            latent,
            own_para,
            own_run,
            para,
            run,
            lang: sheet.own_lang(&info.id.clone()),
            uses,
        })
    }

    /// Where a style is used in the main text, to select every instance.
    pub fn style_uses(&self, style: &str) -> Vec<StyleUse> {
        let Some(id) = self.styles.lookup(style) else { return Vec::new() };
        let character = self.style_kind(&id) == "character";
        let default = self.styles.default_para.clone().unwrap_or_else(|| "Normal".into());
        let mut out = Vec::new();
        walk_paragraphs(self.main_body(), &mut |p| {
            if !character {
                let s = p.ppr.as_ref().and_then(|pp| pp.child("w:pStyle")).and_then(|s| s.attr("w:val")).unwrap_or(&default);
                if s == id {
                    out.push(StyleUse { p: p.id, from: 0, to: p.len16() });
                }
                return;
            }
            let mut start = 0;
            let mut spans: Vec<(usize, usize)> = Vec::new();
            walk_runs(&p.inlines, &mut start, &mut |r, s, _| {
                let len = r.len16();
                if len > 0 && r.rpr.as_ref().and_then(|e| e.child("w:rStyle")).and_then(|x| x.attr("w:val")) == Some(id.as_str()) {
                    match spans.last_mut() {
                        Some(last) if last.1 == s => last.1 = s + len,
                        _ => spans.push((s, s + len)),
                    }
                }
            });
            out.extend(spans.into_iter().map(|(from, to)| StyleUse { p: p.id, from, to }));
        });
        out
    }
}

/// What a selection holds that a style can take: the paragraphs it covers, their own formatting and the run formatting
/// all the selected text shares.
struct Selected {
    pids: Vec<NodeId>,
    para: Vec<Element>,
    run: Vec<Element>,
}
