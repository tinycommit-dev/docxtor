//! Style sheet (`styles.xml`) and theme fonts, with inheritance resolution.

use crate::props::{ParaFmt, RunFmt};
use crate::shapes::theme::ShapeTheme;
use crate::xml::{self, Element, Node, XmlDoc};
use serde::Serialize;
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StyleInfo {
    pub id: String,
    pub name: String,
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub based_on: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next: Option<String>,
    pub priority: i64,
    pub hidden: bool,
    pub quick: bool,
    /// Made by a person, not one of Word's built-in styles (`w:customStyle`).
    pub custom: bool,
    /// The style this one is linked to: a paragraph style and its character style apply as one in Word.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub link: Option<String>,
}

#[derive(Debug, Clone)]
struct StyleDef {
    info: StyleInfo,
    ppr: ParaFmt,
    rpr: RunFmt,
    /// The language in the style's run properties (`w:lang w:val`).
    lang: Option<String>,
    tblpr: Option<Element>,
    tcpr: Option<Element>,
    /// The conditional formatting of a table style (`w:tblStylePr`).
    conds: Vec<Element>,
}

/// `w:lang w:val` in a run-properties element.
pub(crate) fn lang_of(rpr: &Element) -> Option<String> {
    rpr.child("w:lang").and_then(|l| l.attr("w:val")).filter(|v| !v.is_empty()).map(String::from)
}

#[derive(Debug, Clone, Default)]
pub struct ThemeFonts {
    pub major: Option<String>,
    pub minor: Option<String>,
    /// The complex-script fonts (`majorBidi`, `minorBidi`): the theme's `a:cs` face, else its Arabic one.
    pub major_cs: Option<String>,
    pub minor_cs: Option<String>,
    /// The East Asian fonts (`majorEastAsia`, `minorEastAsia`): the theme's `a:ea` face, else the face it lists for
    /// the script of `ea_lang` (`Jpan`, `Hang`, `Hans`, `Hant`).
    pub major_ea: Option<String>,
    pub minor_ea: Option<String>,
    pub major_scripts: Vec<(String, String)>,
    pub minor_scripts: Vec<(String, String)>,
    /// The document's East Asian language (`ja-JP`), which says which script's face a theme font stands for.
    pub ea_lang: Option<String>,
    /// The theme's colours, for text colours that name one (`w:themeColor`); None when the file has no theme.
    pub colors: Option<ShapeTheme>,
}

impl ThemeFonts {
    pub fn parse(theme: &XmlDoc) -> ThemeFonts {
        let get = |n: &str| theme.root.find(n).and_then(|f| f.child("a:latin")).and_then(|l| l.attr("typeface")).map(String::from);
        let cs = |n: &str| {
            let f = theme.root.find(n)?;
            let face = |e: Option<&Element>| e.and_then(|e| e.attr("typeface")).filter(|t| !t.is_empty()).map(String::from);
            face(f.child("a:cs")).or_else(|| face(f.children_named("a:font").find(|e| e.attr("script") == Some("Arab"))))
        };
        let ea = |n: &str| theme.root.find(n).and_then(|f| f.child("a:ea")).and_then(|e| e.attr("typeface")).filter(|t| !t.is_empty()).map(String::from);
        let scripts = |n: &str| -> Vec<(String, String)> {
            theme.root.find(n).map(|f| f.children_named("a:font").filter_map(|e| Some((e.attr("script")?.to_string(), e.attr("typeface")?.to_string()))).collect()).unwrap_or_default()
        };
        ThemeFonts {
            major: get("a:majorFont"),
            minor: get("a:minorFont"),
            major_cs: cs("a:majorFont"),
            minor_cs: cs("a:minorFont"),
            major_ea: ea("a:majorFont"),
            minor_ea: ea("a:minorFont"),
            major_scripts: scripts("a:majorFont"),
            minor_scripts: scripts("a:minorFont"),
            ea_lang: None,
            colors: Some(ShapeTheme::from_doc(&theme.root)),
        }
    }

    /// The East Asian face of a theme font reference (`majorEastAsia`, `minorEastAsia`).
    pub fn resolve_ea(&self, theme_ref: &str) -> Option<String> {
        let major = theme_ref.starts_with("major");
        let direct = if major { &self.major_ea } else { &self.minor_ea };
        if direct.is_some() {
            return direct.clone();
        }
        let script = match self.ea_lang.as_deref()?.to_ascii_lowercase().as_str() {
            l if l.starts_with("ja") => "Jpan",
            l if l.starts_with("ko") => "Hang",
            "zh-tw" | "zh-hk" | "zh-mo" => "Hant",
            l if l.starts_with("zh") => "Hans",
            _ => return None,
        };
        let list = if major { &self.major_scripts } else { &self.minor_scripts };
        list.iter().find(|(s, _)| s == script).map(|(_, f)| f.clone())
    }

    /// The complex-script face of a theme font reference (`majorBidi`, `minorBidi`).
    pub fn resolve_cs(&self, theme_ref: &str) -> Option<String> {
        if theme_ref.starts_with("major") {
            self.major_cs.clone()
        } else {
            self.minor_cs.clone()
        }
    }

    pub fn resolve(&self, theme_ref: &str) -> Option<String> {
        if theme_ref.starts_with("major") {
            self.major.clone()
        } else {
            self.minor.clone()
        }
    }
}

/// A styles part for a file that has none. A file without styles gets Word's implicit defaults (Times New Roman,
/// 10 pt), so the new part says the same in `w:docDefaults`: without it the text would lose its font and its size
/// the moment the first style is added.
fn new_styles_doc() -> XmlDoc {
    let defaults = Element::new("w:docDefaults").with_child(Element::new("w:rPrDefault").with_child(
        Element::new("w:rPr")
            .with_child(Element::new("w:rFonts").with_attr("w:ascii", "Times New Roman").with_attr("w:hAnsi", "Times New Roman"))
            .with_child(Element::new("w:sz").with_attr("w:val", "20"))
            .with_child(Element::new("w:szCs").with_attr("w:val", "20")),
    ));
    XmlDoc::new(Element::new("w:styles").with_attr("xmlns:w", "http://schemas.openxmlformats.org/wordprocessingml/2006/main").with_child(defaults))
}

#[derive(Debug, Clone, Default)]
pub struct StyleSheet {
    pub doc: Option<XmlDoc>,
    styles: HashMap<String, StyleDef>,
    order: Vec<String>,
    pub default_para: Option<String>,
    pub default_char: Option<String>,
    pub doc_rpr: RunFmt,
    pub doc_ppr: ParaFmt,
    pub theme: ThemeFonts,
    pub dirty: bool,
}

impl StyleSheet {
    pub fn parse(doc: Option<XmlDoc>, theme: ThemeFonts) -> StyleSheet {
        let mut ss = StyleSheet { theme, ..Default::default() };
        if let Some(d) = &doc {
            ss.load(&d.root);
        } else {
            // Word's implicit defaults when styles.xml is absent.
            ss.doc_rpr = RunFmt { font: Some("Times New Roman".into()), size: Some(10.0), ..Default::default() };
        }
        ss.doc = doc;
        ss
    }

    pub(crate) fn load(&mut self, root: &Element) {
        self.styles.clear();
        self.order.clear();
        if let Some(dd) = root.child("w:docDefaults") {
            if let Some(r) = dd.path(&["w:rPrDefault", "w:rPr"]) {
                self.doc_rpr = RunFmt::from_rpr(r);
            }
            self.theme.ea_lang = self.doc_rpr.ext.ea.lang_ea.clone();
            if let Some(p) = dd.path(&["w:pPrDefault", "w:pPr"]) {
                self.doc_ppr = ParaFmt::from_ppr(p);
            }
        } else {
            self.doc_rpr = RunFmt::default();
        }
        // Without a size in the defaults the text is 10 pt (the size Word uses: `w:sz` defaults to 20 half-points).
        // Setting it here whatever the defaults hold keeps the sheet reading the same before and after an edit adds
        // `w:docDefaults` to a sheet that had none, and before and after undo and redo.
        if self.doc_rpr.size.is_none() {
            self.doc_rpr.size = Some(10.0);
        }
        // Colours that name a theme colour (borders, fills) are looked up in the theme once, here.
        let theme = self.theme.colors.clone();
        let themed = |e: &Element| {
            let mut c = e.clone();
            if let Some(t) = &theme {
                crate::shapes::theme::resolve_theme_attrs(&mut c, t);
            }
            c
        };
        for s in root.children_named("w:style") {
            let Some(id) = s.attr("w:styleId") else { continue };
            let kind = s.attr("w:type").unwrap_or("paragraph").to_string();
            let name = s.child("w:name").and_then(|n| n.attr("w:val")).unwrap_or(id).to_string();
            let is_default = matches!(s.attr("w:default"), Some("1") | Some("true"));
            if is_default && kind == "paragraph" {
                self.default_para = Some(id.to_string());
            }
            if is_default && kind == "character" {
                self.default_char = Some(id.to_string());
            }
            let info = StyleInfo {
                id: id.to_string(),
                name,
                kind,
                based_on: s.child("w:basedOn").and_then(|n| n.attr("w:val")).map(String::from),
                next: s.child("w:next").and_then(|n| n.attr("w:val")).map(String::from),
                priority: s.child("w:uiPriority").and_then(|n| n.attr("w:val")).and_then(|v| v.parse().ok()).unwrap_or(99),
                hidden: s.child("w:hidden").is_some() || s.child("w:semiHidden").is_some(),
                quick: s.child("w:qFormat").is_some(),
                custom: matches!(s.attr("w:customStyle"), Some("1") | Some("true")),
                link: s.child("w:link").and_then(|n| n.attr("w:val")).map(String::from),
            };
            let ppr = s.child("w:pPr").map(|e| ParaFmt::from_ppr(&themed(e))).unwrap_or_default();
            let rpr = s.child("w:rPr").map(|e| RunFmt::from_rpr(&themed(e))).unwrap_or_default();
            self.order.push(id.to_string());
            let tblpr = s.child("w:tblPr").map(&themed);
            let tcpr = s.child("w:tcPr").map(&themed);
            let lang = s.child("w:rPr").and_then(lang_of);
            let conds = s.children_named("w:tblStylePr").map(&themed).collect();
            self.styles.insert(id.to_string(), StyleDef { info, ppr, rpr, lang, tblpr, tcpr, conds });
        }
    }

    pub fn list(&self) -> Vec<StyleInfo> {
        self.order.iter().filter_map(|id| self.styles.get(id)).map(|s| s.info.clone()).collect()
    }

    pub fn get(&self, id: &str) -> Option<&StyleInfo> {
        self.styles.get(id).map(|s| &s.info)
    }

    pub fn exists(&self, id: &str) -> bool {
        self.styles.contains_key(id)
    }

    /// Find a style id by id or display name (case-insensitive, spaces ignored).
    pub fn lookup(&self, name: &str) -> Option<String> {
        if self.styles.contains_key(name) {
            return Some(name.to_string());
        }
        let norm = |s: &str| s.to_lowercase().replace([' ', '-', '_'], "");
        let n = norm(name);
        self.order.iter().find(|id| norm(id) == n || norm(&self.styles[*id].info.name) == n).cloned()
    }

    fn chain(&self, id: &str) -> Vec<&StyleDef> {
        let mut out = Vec::new();
        let mut cur = Some(id.to_string());
        while let Some(c) = cur {
            if out.len() > 16 {
                break;
            }
            let Some(def) = self.styles.get(&c) else { break };
            out.push(def);
            cur = def.info.based_on.clone();
        }
        out.reverse();
        out
    }

    /// Resolved (paragraph, run) formatting contributed by a paragraph style.
    pub fn para_style(&self, id: Option<&str>) -> (ParaFmt, RunFmt) {
        let id = id.map(String::from).or_else(|| self.default_para.clone());
        let mut p = ParaFmt::default();
        let mut r = RunFmt::default();
        if let Some(id) = id {
            for d in self.chain(&id) {
                p = p.overlay(&d.ppr);
                r = r.overlay(&d.rpr);
            }
        }
        (p, r)
    }

    /// The paragraph properties each style along a style's inheritance chain sets itself, base style first.
    pub fn para_chain(&self, id: Option<&str>) -> Vec<ParaFmt> {
        let id = id.map(String::from).or_else(|| self.default_para.clone());
        id.map(|id| self.chain(&id).into_iter().map(|d| d.ppr.clone()).collect()).unwrap_or_default()
    }

    /// The list a style puts its paragraphs in (`numPr`): `(numId, level)`, from the style or one it is based on.
    pub fn style_numbering(&self, id: &str) -> Option<(i64, i64)> {
        let f = self.para_style(Some(id)).0;
        f.num_id.filter(|n| *n > 0).map(|n| (n, f.ilvl.unwrap_or(0)))
    }

    /// Put a style in a list (its `numPr`, as Word writes it for a heading linked to a list level: the level
    /// only when it isn't the first), or take it out with `None`. Returns whether the style exists.
    pub fn set_style_numbering(&mut self, id: &str, num: Option<(i64, i64)>) -> bool {
        let Some(doc) = self.doc.as_mut() else { return false };
        let Some(style) = doc.root.elements_mut().find(|e| e.name == "w:style" && e.attr("w:styleId") == Some(id)) else { return false };
        if style.child("w:pPr").is_none() {
            let at = style
                .children
                .iter()
                .position(|n| matches!(n, Node::Element(e) if matches!(e.name.as_str(), "w:rPr" | "w:tblPr" | "w:trPr" | "w:tcPr" | "w:tblStylePr")))
                .unwrap_or(style.children.len());
            style.children.insert(at, Node::Element(Element::new("w:pPr")));
        }
        let ppr = style.child_mut("w:pPr").unwrap();
        ppr.remove_children("w:numPr");
        if let Some((num_id, ilvl)) = num {
            let mut np = Element::new("w:numPr");
            if ilvl > 0 {
                np = np.with_child(Element::new("w:ilvl").with_attr("w:val", ilvl.to_string()));
            }
            np = np.with_child(Element::new("w:numId").with_attr("w:val", num_id.to_string()));
            crate::props::put_ppr(ppr, np);
        }
        if ppr.children.is_empty() {
            style.remove_children("w:pPr");
        }
        let root = doc.root.clone();
        self.load(&root);
        self.dirty = true;
        true
    }

    /// The language a paragraph style gives its text: the style's own, else the one it is based on, else
    /// the document default.
    pub fn para_lang(&self, id: Option<&str>) -> Option<String> {
        let id = id.map(String::from).or_else(|| self.default_para.clone());
        let from_styles = id.and_then(|id| self.chain(&id).into_iter().rev().find_map(|d| d.lang.clone()));
        // Read from the XML each time: an op can change the default without reloading the styles.
        from_styles.or_else(|| self.doc.as_ref().and_then(|d| d.root.path(&["w:docDefaults", "w:rPrDefault", "w:rPr"])).and_then(lang_of))
    }

    /// Table-level properties (`w:tblPr`) along a table style's inheritance chain, base first.
    pub fn table_style_props(&self, id: &str) -> Vec<&Element> {
        self.chain(id).into_iter().filter_map(|d| d.tblpr.as_ref()).collect()
    }

    pub fn table_style_cell_props(&self, id: &str) -> Vec<&Element> {
        self.chain(id).into_iter().filter_map(|d| d.tcpr.as_ref()).collect()
    }

    /// The conditional formatting (`w:tblStylePr`) of a table style and of the styles it is based on, base first.
    pub fn table_style_conds(&self, id: &str) -> Vec<&Element> {
        self.chain(id).into_iter().flat_map(|d| d.conds.iter()).collect()
    }

    /// (paragraph, run) formatting a table style applies to its content.
    pub fn table_style_text(&self, id: &str) -> (ParaFmt, RunFmt) {
        let mut p = ParaFmt::default();
        let mut r = RunFmt::default();
        for d in self.chain(id) {
            p = p.overlay(&d.ppr);
            r = r.overlay(&d.rpr);
        }
        (p, r)
    }

    pub fn default_table_style(&self) -> Option<String> {
        self.order.iter().find(|id| self.styles[*id].info.kind == "table" && self.styles[*id].info.id == "TableNormal").cloned()
    }

    pub fn char_style(&self, id: &str) -> RunFmt {
        let mut r = RunFmt::default();
        for d in self.chain(id) {
            r = r.overlay(&d.rpr);
        }
        r
    }

    /// Read the styles again after the XML was edited directly.
    pub fn reload(&mut self) {
        if let Some(d) = &self.doc {
            let root = d.root.clone();
            self.load(&root);
        }
        self.dirty = true;
    }

    /// The character style a paragraph style is linked to, made from the paragraph style's run properties when
    /// the file only names it (Word writes both, but a style Docxtor added on demand has no twin yet).
    pub fn linked_char_style(&mut self, para_id: &str) -> Option<String> {
        let def = self.styles.get(para_id)?;
        if def.info.kind != "paragraph" {
            return None;
        }
        let link = def.info.link.clone()?;
        if self.styles.get(&link).is_some_and(|s| s.info.kind == "character") {
            return Some(link);
        }
        let (name, priority) = (def.info.name.clone(), def.info.priority);
        let src = self.doc.as_ref()?.root.children_named("w:style").find(|e| e.attr("w:styleId") == Some(para_id))?;
        let rpr = src.child("w:rPr").cloned();
        if !self.styles.contains_key("DefaultParagraphFont") {
            self.ensure_builtin("DefaultParagraphFont");
        }
        let mut style = Element::new("w:style")
            .with_attr("w:type", "character")
            .with_attr("w:customStyle", "1")
            .with_attr("w:styleId", &link)
            .with_child(Element::new("w:name").with_attr("w:val", format!("{name} Char")))
            .with_child(Element::new("w:basedOn").with_attr("w:val", "DefaultParagraphFont"))
            .with_child(Element::new("w:link").with_attr("w:val", para_id))
            .with_child(Element::new("w:uiPriority").with_attr("w:val", priority.to_string()));
        if let Some(r) = rpr {
            style.children.push(Node::Element(r));
        }
        self.doc.as_mut()?.root.children.push(Node::Element(style));
        self.reload();
        Some(link)
    }

    /// Resolve theme font references into concrete family names.
    pub fn finish_run(&self, mut f: RunFmt) -> RunFmt {
        if f.font.is_none() {
            if let Some(t) = &f.font_theme {
                f.font = self.theme.resolve(t);
            }
        }
        if f.font_cs.is_none() {
            if let Some(t) = &f.font_cs_theme {
                f.font_cs = self.theme.resolve_cs(t);
            }
        }
        if f.font_ea.is_none() {
            if let Some(t) = &f.font_ea_theme {
                f.font_ea = self.theme.resolve_ea(t);
            }
        }
        f
    }

    /// Make sure a built-in style exists in styles.xml (adding Word's definition if needed).
    /// Returns the style id to use.
    /// Point the defaults and every style that names font `from` at `to`. Returns whether anything changed.
    pub fn replace_font(&mut self, from: &str, to: &str) -> bool {
        let Some(doc) = self.doc.as_mut() else { return false };
        if !crate::fonts::replace_in(&mut doc.root, from, to, &self.theme) {
            return false;
        }
        let root = doc.root.clone();
        self.load(&root);
        self.dirty = true;
        true
    }

    /// Use another theme: styles are read again, so the colours they name come out of the new theme.
    pub fn set_theme(&mut self, theme: ThemeFonts) {
        self.theme = theme;
        if let Some(d) = &self.doc {
            let root = d.root.clone();
            self.load(&root);
        }
    }

    /// Change the style sheet's XML (creating it when the file has none), then read it again.
    pub(crate) fn edit_xml<R>(&mut self, f: impl FnOnce(&mut Element) -> R) -> R {
        let doc = self.doc.get_or_insert_with(new_styles_doc);
        let out = f(&mut doc.root);
        let root = doc.root.clone();
        self.load(&root);
        self.dirty = true;
        out
    }

    /// The formatting a style sets itself, without what it builds on.
    pub fn own_format(&self, id: &str) -> Option<(&ParaFmt, &RunFmt)> {
        self.styles.get(id).map(|d| (&d.ppr, &d.rpr))
    }

    /// The ids of a style and the styles it builds on, base style first.
    pub fn chain_ids(&self, id: &str) -> Vec<String> {
        self.chain(id).into_iter().map(|d| d.info.id.clone()).collect()
    }

    /// The ids of every style, in file order.
    pub fn ids(&self) -> &[String] {
        &self.order
    }

    /// The language a style sets itself (`w:lang`).
    pub fn own_lang(&self, id: &str) -> Option<String> {
        self.styles.get(id).and_then(|d| d.lang.clone())
    }

    /// The `w:style` element with this id.
    pub fn style_element(&self, id: &str) -> Option<&Element> {
        self.doc.as_ref()?.root.children_named("w:style").find(|s| s.attr("w:styleId") == Some(id))
    }

    /// The ids a style builds on or links to (`basedOn`, `link`, `next`), for copying a style with what it needs.
    pub fn style_deps(&self, id: &str) -> Vec<String> {
        let Some(s) = self.style_element(id) else { return Vec::new() };
        ["w:basedOn", "w:link", "w:next"].iter().filter_map(|n| s.child(n)?.attr("w:val").map(String::from)).collect()
    }

    /// Bring in style definitions from another document. A style whose id exists here is left as it is;
    /// one whose name exists here under another id maps to that id; the rest are added. `nums` maps list ids
    /// used inside style definitions. Returns each source id with the id it has here.
    pub fn import_styles(&mut self, defs: &[Element], nums: &HashMap<i64, i64>) -> HashMap<String, String> {
        let mut map: HashMap<String, String> = HashMap::new();
        let mut add: Vec<&Element> = Vec::new();
        for d in defs {
            let Some(id) = d.attr("w:styleId") else { continue };
            let name = d.child("w:name").and_then(|n| n.attr("w:val")).unwrap_or(id);
            if self.styles.contains_key(id) {
                map.insert(id.into(), id.into());
            } else if let Some(other) = self.lookup(name).filter(|o| self.styles.get(o).is_some_and(|s| s.info.kind == d.attr("w:type").unwrap_or("paragraph"))) {
                map.insert(id.into(), other);
            } else {
                map.insert(id.into(), id.into());
                add.push(d);
            }
        }
        if add.is_empty() {
            return map;
        }
        // What the new styles build on has to exist here (Normal and the default styles are made when missing).
        for d in &add {
            for dep in ["w:basedOn", "w:link", "w:next"] {
                let Some(target) = d.child(dep).and_then(|e| e.attr("w:val")) else { continue };
                if !map.contains_key(target) && !self.styles.contains_key(target) {
                    self.ensure_builtin(target);
                }
            }
        }
        if self.doc.is_none() {
            self.doc = Some(new_styles_doc());
        }
        for d in add {
            let mut el = d.clone();
            crate::fragment::walk_mut(&mut el, &mut |e| {
                match e.name.as_str() {
                    "w:basedOn" | "w:link" | "w:next" => {
                        if let Some(v) = e.attr("w:val").map(String::from) {
                            let to = map.get(&v).cloned().unwrap_or(v);
                            e.set_attr("w:val", to);
                        }
                    }
                    "w:numId" => {
                        if let Some(n) = e.attr("w:val").and_then(|v| v.parse::<i64>().ok()) {
                            if let Some(m) = nums.get(&n) {
                                e.set_attr("w:val", m.to_string());
                            }
                        }
                    }
                    _ => {}
                }
            });
            // A dangling reference would make Word complain.
            for dep in ["w:basedOn", "w:link", "w:next"] {
                let gone = el.child(dep).and_then(|e| e.attr("w:val")).is_some_and(|v| !self.styles.contains_key(v) && !map.values().any(|m| m == v));
                if gone {
                    el.remove_children(dep);
                }
            }
            self.doc.as_mut().unwrap().root.children.push(Node::Element(el));
        }
        let root = self.doc.as_ref().unwrap().root.clone();
        self.load(&root);
        self.dirty = true;
        map
    }

    pub fn ensure_builtin(&mut self, requested: &str) -> Option<String> {
        if let Some(id) = self.lookup(requested) {
            return Some(id);
        }
        let xml_src = builtin_style_xml(requested)?;
        self.insert_style_xml(requested, &xml_src)
    }

    /// Add style definitions (one or more `w:style` elements as XML) that are not in the sheet yet. Returns the
    /// id of the first one added, else the style `requested` names.
    pub fn insert_style_xml(&mut self, requested: &str, xml_src: &str) -> Option<String> {
        if self.doc.is_none() {
            self.doc = Some(new_styles_doc());
        }
        let parsed = xml::parse(format!(r#"<x xmlns:w="w">{xml_src}</x>"#).as_bytes()).ok()?;
        let doc = self.doc.as_mut().unwrap();
        let mut added = None;
        let mut seen: Vec<String> = Vec::new();
        for e in parsed.root.elements() {
            let id = e.attr("w:styleId").unwrap_or_default().to_string();
            if self.styles.contains_key(&id) || seen.contains(&id) {
                continue;
            }
            seen.push(id.clone());
            if added.is_none() {
                added = Some(id);
            }
            doc.root.children.push(Node::Element(e.clone()));
        }
        let root = doc.root.clone();
        self.load(&root);
        self.dirty = true;
        added.or_else(|| self.lookup(requested))
    }
}

/// Definitions matching Word's defaults for the styles Docxtor's UI and AI tools rely on.
pub(crate) fn builtin_style_xml(name: &str) -> Option<String> {
    let key = name.to_lowercase().replace([' ', '-', '_'], "");
    if let Some(x) = crate::builtin_styles::xml(&key) {
        return Some(x);
    }
    let heading = |n: u8, size: u8, color: &str, before: u16| {
        format!(
            r#"<w:style w:type="paragraph" w:styleId="Heading{n}"><w:name w:val="heading {n}"/><w:basedOn w:val="Normal"/><w:next w:val="Normal"/><w:link w:val="Heading{n}Char"/><w:uiPriority w:val="9"/><w:qFormat/><w:pPr><w:keepNext/><w:keepLines/><w:spacing w:before="{before}" w:after="80"/><w:outlineLvl w:val="{lvl}"/></w:pPr><w:rPr><w:rFonts w:asciiTheme="majorHAnsi" w:hAnsiTheme="majorHAnsi" w:eastAsiaTheme="majorEastAsia" w:cstheme="majorBidi"/><w:color w:val="{color}"/><w:sz w:val="{size}"/><w:szCs w:val="{size}"/></w:rPr></w:style>"#,
            lvl = n - 1
        )
    };
    let normal = r#"<w:style w:type="paragraph" w:default="1" w:styleId="Normal"><w:name w:val="Normal"/><w:qFormat/></w:style>"#;
    Some(match key.as_str() {
        "normal" => normal.to_string(),
        "heading1" => heading(1, 40, "0F4761", 360),
        "heading2" => heading(2, 32, "0F4761", 160),
        "heading3" => heading(3, 28, "0F4761", 160),
        "heading4" => heading(4, 24, "0F4761", 80),
        "heading5" => heading(5, 24, "0F4761", 80),
        "heading6" => heading(6, 24, "595959", 40),
        "title" => r#"<w:style w:type="paragraph" w:styleId="Title"><w:name w:val="Title"/><w:basedOn w:val="Normal"/><w:next w:val="Normal"/><w:uiPriority w:val="10"/><w:qFormat/><w:pPr><w:spacing w:after="80" w:line="240" w:lineRule="auto"/><w:contextualSpacing/></w:pPr><w:rPr><w:rFonts w:asciiTheme="majorHAnsi" w:hAnsiTheme="majorHAnsi"/><w:spacing w:val="-10"/><w:kern w:val="28"/><w:sz w:val="56"/><w:szCs w:val="56"/></w:rPr></w:style>"#.into(),
        "subtitle" => r#"<w:style w:type="paragraph" w:styleId="Subtitle"><w:name w:val="Subtitle"/><w:basedOn w:val="Normal"/><w:next w:val="Normal"/><w:uiPriority w:val="11"/><w:qFormat/><w:pPr><w:spacing w:after="160"/></w:pPr><w:rPr><w:color w:val="595959"/><w:spacing w:val="15"/><w:sz w:val="28"/><w:szCs w:val="28"/></w:rPr></w:style>"#.into(),
        "quote" => r#"<w:style w:type="paragraph" w:styleId="Quote"><w:name w:val="Quote"/><w:basedOn w:val="Normal"/><w:next w:val="Normal"/><w:uiPriority w:val="29"/><w:qFormat/><w:pPr><w:spacing w:before="160"/><w:ind w:left="864" w:right="864"/><w:jc w:val="center"/></w:pPr><w:rPr><w:i/><w:iCs/><w:color w:val="404040"/></w:rPr></w:style>"#.into(),
        "listparagraph" => r#"<w:style w:type="paragraph" w:styleId="ListParagraph"><w:name w:val="List Paragraph"/><w:basedOn w:val="Normal"/><w:uiPriority w:val="34"/><w:qFormat/><w:pPr><w:ind w:left="720"/><w:contextualSpacing/></w:pPr></w:style>"#.into(),
        "defaultparagraphfont" => r#"<w:style w:type="character" w:default="1" w:styleId="DefaultParagraphFont"><w:name w:val="Default Paragraph Font"/><w:uiPriority w:val="1"/><w:semiHidden/><w:unhideWhenUsed/></w:style>"#.into(),
        "strong" => r#"<w:style w:type="character" w:styleId="Strong"><w:name w:val="Strong"/><w:basedOn w:val="DefaultParagraphFont"/><w:uiPriority w:val="22"/><w:qFormat/><w:rPr><w:b/><w:bCs/></w:rPr></w:style>"#.into(),
        "emphasis" => r#"<w:style w:type="character" w:styleId="Emphasis"><w:name w:val="Emphasis"/><w:basedOn w:val="DefaultParagraphFont"/><w:uiPriority w:val="20"/><w:qFormat/><w:rPr><w:i/><w:iCs/></w:rPr></w:style>"#.into(),
        "subtleemphasis" => r#"<w:style w:type="character" w:styleId="SubtleEmphasis"><w:name w:val="Subtle Emphasis"/><w:basedOn w:val="DefaultParagraphFont"/><w:uiPriority w:val="19"/><w:qFormat/><w:rPr><w:i/><w:iCs/><w:color w:val="404040" w:themeColor="text1" w:themeTint="BF"/></w:rPr></w:style>"#.into(),
        "intenseemphasis" => r#"<w:style w:type="character" w:styleId="IntenseEmphasis"><w:name w:val="Intense Emphasis"/><w:basedOn w:val="DefaultParagraphFont"/><w:uiPriority w:val="21"/><w:qFormat/><w:rPr><w:i/><w:iCs/><w:color w:val="0F4761" w:themeColor="accent1" w:themeShade="BF"/></w:rPr></w:style>"#.into(),
        "subtlereference" => r#"<w:style w:type="character" w:styleId="SubtleReference"><w:name w:val="Subtle Reference"/><w:basedOn w:val="DefaultParagraphFont"/><w:uiPriority w:val="31"/><w:qFormat/><w:rPr><w:smallCaps/><w:color w:val="5A5A5A" w:themeColor="text1" w:themeTint="A5"/></w:rPr></w:style>"#.into(),
        "intensereference" => r#"<w:style w:type="character" w:styleId="IntenseReference"><w:name w:val="Intense Reference"/><w:basedOn w:val="DefaultParagraphFont"/><w:uiPriority w:val="32"/><w:qFormat/><w:rPr><w:b/><w:bCs/><w:smallCaps/><w:color w:val="0F4761" w:themeColor="accent1" w:themeShade="BF"/><w:spacing w:val="5"/></w:rPr></w:style>"#.into(),
        "booktitle" => r#"<w:style w:type="character" w:styleId="BookTitle"><w:name w:val="Book Title"/><w:basedOn w:val="DefaultParagraphFont"/><w:uiPriority w:val="33"/><w:qFormat/><w:rPr><w:b/><w:bCs/><w:i/><w:iCs/><w:spacing w:val="5"/></w:rPr></w:style>"#.into(),
        "hyperlink" => r#"<w:style w:type="character" w:styleId="Hyperlink"><w:name w:val="Hyperlink"/><w:basedOn w:val="DefaultParagraphFont"/><w:uiPriority w:val="99"/><w:unhideWhenUsed/><w:rPr><w:color w:val="0563C1"/><w:u w:val="single"/></w:rPr></w:style>"#.into(),
        "placeholdertext" => r#"<w:style w:type="character" w:styleId="PlaceholderText"><w:name w:val="Placeholder Text"/><w:basedOn w:val="DefaultParagraphFont"/><w:uiPriority w:val="99"/><w:semiHidden/><w:rPr><w:color w:val="808080"/></w:rPr></w:style>"#.into(),
        "header" | "footer" => {
            let (id, name) = if key == "header" { ("Header", "header") } else { ("Footer", "footer") };
            format!(
                r#"<w:style w:type="paragraph" w:styleId="{id}"><w:name w:val="{name}"/><w:basedOn w:val="Normal"/><w:uiPriority w:val="99"/><w:unhideWhenUsed/><w:pPr><w:tabs><w:tab w:val="center" w:pos="4680"/><w:tab w:val="right" w:pos="9360"/></w:tabs><w:spacing w:after="0" w:line="240" w:lineRule="auto"/></w:pPr></w:style>"#
            )
        }
        "tocheading" => r#"<w:style w:type="paragraph" w:styleId="TOCHeading"><w:name w:val="TOC Heading"/><w:basedOn w:val="Heading1"/><w:next w:val="Normal"/><w:uiPriority w:val="39"/><w:unhideWhenUsed/><w:qFormat/><w:pPr><w:outlineLvl w:val="9"/></w:pPr></w:style>"#.into(),
        k if k.len() == 4 && k.starts_with("toc") && k.as_bytes()[3].is_ascii_digit() && k.as_bytes()[3] != b'0' => {
            let n = (k.as_bytes()[3] - b'0') as i64;
            format!(
                r#"<w:style w:type="paragraph" w:styleId="TOC{n}"><w:name w:val="toc {n}"/><w:basedOn w:val="Normal"/><w:next w:val="Normal"/><w:autoRedefine/><w:uiPriority w:val="39"/><w:unhideWhenUsed/><w:pPr><w:spacing w:after="100"/>{ind}</w:pPr></w:style>"#,
                ind = if n > 1 { format!(r#"<w:ind w:left="{}"/>"#, (n - 1) * 220) } else { String::new() }
            )
        }
        "footnotetext" => r#"<w:style w:type="paragraph" w:styleId="FootnoteText"><w:name w:val="footnote text"/><w:basedOn w:val="Normal"/><w:uiPriority w:val="99"/><w:unhideWhenUsed/><w:pPr><w:spacing w:after="0" w:line="240" w:lineRule="auto"/></w:pPr><w:rPr><w:sz w:val="20"/><w:szCs w:val="20"/></w:rPr></w:style>"#.into(),
        "footnotereference" => r#"<w:style w:type="character" w:styleId="FootnoteReference"><w:name w:val="footnote reference"/><w:basedOn w:val="DefaultParagraphFont"/><w:uiPriority w:val="99"/><w:unhideWhenUsed/><w:rPr><w:vertAlign w:val="superscript"/></w:rPr></w:style>"#.into(),
        "endnotetext" => r#"<w:style w:type="paragraph" w:styleId="EndnoteText"><w:name w:val="endnote text"/><w:basedOn w:val="Normal"/><w:uiPriority w:val="99"/><w:unhideWhenUsed/><w:pPr><w:spacing w:after="0" w:line="240" w:lineRule="auto"/></w:pPr><w:rPr><w:sz w:val="20"/><w:szCs w:val="20"/></w:rPr></w:style>"#.into(),
        "endnotereference" => r#"<w:style w:type="character" w:styleId="EndnoteReference"><w:name w:val="endnote reference"/><w:basedOn w:val="DefaultParagraphFont"/><w:uiPriority w:val="99"/><w:unhideWhenUsed/><w:rPr><w:vertAlign w:val="superscript"/></w:rPr></w:style>"#.into(),
        "caption" => r#"<w:style w:type="paragraph" w:styleId="Caption"><w:name w:val="caption"/><w:basedOn w:val="Normal"/><w:next w:val="Normal"/><w:uiPriority w:val="35"/><w:unhideWhenUsed/><w:qFormat/><w:pPr><w:spacing w:after="200" w:line="240" w:lineRule="auto"/></w:pPr><w:rPr><w:i/><w:iCs/><w:color w:val="44546A" w:themeColor="text2"/><w:sz w:val="18"/><w:szCs w:val="18"/></w:rPr></w:style>"#.into(),
        "tableoffigures" => r#"<w:style w:type="paragraph" w:styleId="TableofFigures"><w:name w:val="table of figures"/><w:basedOn w:val="Normal"/><w:next w:val="Normal"/><w:uiPriority w:val="99"/><w:unhideWhenUsed/><w:pPr><w:spacing w:after="0"/></w:pPr></w:style>"#.into(),
        "tableofauthorities" => r#"<w:style w:type="paragraph" w:styleId="TableofAuthorities"><w:name w:val="table of authorities"/><w:basedOn w:val="Normal"/><w:next w:val="Normal"/><w:uiPriority w:val="99"/><w:unhideWhenUsed/><w:pPr><w:spacing w:after="0"/><w:ind w:left="220" w:hanging="220"/></w:pPr></w:style>"#.into(),
        "toaheading" => r#"<w:style w:type="paragraph" w:styleId="TOAHeading"><w:name w:val="toa heading"/><w:basedOn w:val="Normal"/><w:next w:val="Normal"/><w:uiPriority w:val="99"/><w:unhideWhenUsed/><w:pPr><w:spacing w:before="120" w:after="60"/></w:pPr><w:rPr><w:rFonts w:asciiTheme="majorHAnsi" w:eastAsiaTheme="majorEastAsia" w:hAnsiTheme="majorHAnsi" w:cstheme="majorBidi"/><w:b/><w:bCs/><w:sz w:val="24"/><w:szCs w:val="24"/></w:rPr></w:style>"#.into(),
        "indexheading" => r#"<w:style w:type="paragraph" w:styleId="IndexHeading"><w:name w:val="index heading"/><w:basedOn w:val="Normal"/><w:next w:val="Index1"/><w:uiPriority w:val="99"/><w:unhideWhenUsed/><w:pPr><w:spacing w:before="240" w:after="120"/></w:pPr><w:rPr><w:rFonts w:asciiTheme="majorHAnsi" w:eastAsiaTheme="majorEastAsia" w:hAnsiTheme="majorHAnsi" w:cstheme="majorBidi"/><w:b/><w:bCs/><w:sz w:val="26"/><w:szCs w:val="26"/></w:rPr></w:style>"#.into(),
        k if k.len() == 6 && k.starts_with("index") && k.as_bytes()[5].is_ascii_digit() && k.as_bytes()[5] != b'0' => {
            let n = (k.as_bytes()[5] - b'0') as i64;
            format!(
                r#"<w:style w:type="paragraph" w:styleId="Index{n}"><w:name w:val="index {n}"/><w:basedOn w:val="Normal"/><w:next w:val="Normal"/><w:autoRedefine/><w:uiPriority w:val="99"/><w:unhideWhenUsed/><w:pPr><w:spacing w:after="0" w:line="240" w:lineRule="auto"/><w:ind w:left="{}" w:hanging="220"/></w:pPr></w:style>"#,
                n * 220
            )
        }
        "code" | "sourcecode" => r#"<w:style w:type="paragraph" w:customStyle="1" w:styleId="Code"><w:name w:val="Code"/><w:basedOn w:val="Normal"/><w:qFormat/><w:pPr><w:shd w:val="clear" w:color="auto" w:fill="F3F4F6"/><w:spacing w:after="0" w:line="240" w:lineRule="auto"/></w:pPr><w:rPr><w:rFonts w:ascii="Consolas" w:hAnsi="Consolas" w:cs="Consolas"/><w:sz w:val="20"/><w:szCs w:val="20"/></w:rPr></w:style>"#.into(),
        "bibliography" => r#"<w:style w:type="paragraph" w:styleId="Bibliography"><w:name w:val="Bibliography"/><w:basedOn w:val="Normal"/><w:next w:val="Normal"/><w:uiPriority w:val="37"/><w:unhideWhenUsed/></w:style>"#.into(),
        "tablenormal" | "normaltable" => r#"<w:style w:type="table" w:default="1" w:styleId="TableNormal"><w:name w:val="Normal Table"/><w:uiPriority w:val="99"/><w:semiHidden/><w:unhideWhenUsed/><w:tblPr><w:tblInd w:w="0" w:type="dxa"/><w:tblCellMar><w:top w:w="0" w:type="dxa"/><w:left w:w="108" w:type="dxa"/><w:bottom w:w="0" w:type="dxa"/><w:right w:w="108" w:type="dxa"/></w:tblCellMar></w:tblPr></w:style>"#.into(),
        "tablegrid" => {
            let b = |n: &str| format!(r#"<w:{n} w:val="single" w:sz="4" w:space="0" w:color="auto"/>"#);
            format!(
                r#"<w:style w:type="table" w:default="1" w:styleId="TableNormal"><w:name w:val="Normal Table"/><w:uiPriority w:val="99"/><w:semiHidden/><w:unhideWhenUsed/><w:tblPr><w:tblInd w:w="0" w:type="dxa"/><w:tblCellMar><w:top w:w="0" w:type="dxa"/><w:left w:w="108" w:type="dxa"/><w:bottom w:w="0" w:type="dxa"/><w:right w:w="108" w:type="dxa"/></w:tblCellMar></w:tblPr></w:style><w:style w:type="table" w:styleId="TableGrid"><w:name w:val="Table Grid"/><w:basedOn w:val="TableNormal"/><w:uiPriority w:val="39"/><w:pPr><w:spacing w:after="0" w:line="240" w:lineRule="auto"/></w:pPr><w:tblPr><w:tblBorders>{}{}{}{}{}{}</w:tblBorders></w:tblPr></w:style>"#,
                b("top"), b("left"), b("bottom"), b("right"), b("insideH"), b("insideV")
            )
        }
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inheritance_and_builtin() {
        let src = r#"<w:styles xmlns:w="w"><w:docDefaults><w:rPrDefault><w:rPr><w:rFonts w:asciiTheme="minorHAnsi"/><w:sz w:val="22"/></w:rPr></w:rPrDefault></w:docDefaults>
<w:style w:type="paragraph" w:default="1" w:styleId="Normal"><w:name w:val="Normal"/></w:style>
<w:style w:type="paragraph" w:styleId="Big"><w:name w:val="Big Text"/><w:basedOn w:val="Normal"/><w:rPr><w:sz w:val="48"/></w:rPr></w:style>
<w:style w:type="paragraph" w:styleId="Bigger"><w:name w:val="Bigger"/><w:basedOn w:val="Big"/><w:rPr><w:b/></w:rPr></w:style></w:styles>"#;
        let theme = ThemeFonts { major: Some("Aptos Display".into()), minor: Some("Aptos".into()), ..Default::default() };
        let mut ss = StyleSheet::parse(Some(xml::parse(src.as_bytes()).unwrap()), theme);
        let (_, r) = ss.para_style(Some("Bigger"));
        assert_eq!(r.size, Some(24.0));
        assert_eq!(r.bold, Some(true));
        assert_eq!(ss.lookup("big text").as_deref(), Some("Big"));
        let f = ss.finish_run(ss.doc_rpr.clone());
        assert_eq!(f.font.as_deref(), Some("Aptos"));
        assert_eq!(ss.ensure_builtin("Heading 1").as_deref(), Some("Heading1"));
        assert!(ss.exists("Heading1"));
        assert!(ss.dirty);
    }

    #[test]
    fn language_follows_the_style_chain() {
        let src = r#"<w:styles xmlns:w="w"><w:docDefaults><w:rPrDefault><w:rPr><w:lang w:val="en-US"/></w:rPr></w:rPrDefault></w:docDefaults>
<w:style w:type="paragraph" w:default="1" w:styleId="Normal"><w:name w:val="Normal"/></w:style>
<w:style w:type="paragraph" w:styleId="Spanish"><w:name w:val="Spanish"/><w:basedOn w:val="Normal"/><w:rPr><w:lang w:val="es-ES"/></w:rPr></w:style>
<w:style w:type="paragraph" w:styleId="SpanishQuote"><w:name w:val="Quote"/><w:basedOn w:val="Spanish"/></w:style>
<w:style w:type="paragraph" w:styleId="French"><w:name w:val="French"/><w:basedOn w:val="Spanish"/><w:rPr><w:lang w:val="fr-FR"/></w:rPr></w:style></w:styles>"#;
        let ss = StyleSheet::parse(Some(xml::parse(src.as_bytes()).unwrap()), ThemeFonts::default());
        assert_eq!(ss.para_lang(None).as_deref(), Some("en-US"));
        assert_eq!(ss.para_lang(Some("Normal")).as_deref(), Some("en-US"));
        assert_eq!(ss.para_lang(Some("Spanish")).as_deref(), Some("es-ES"));
        // Inherited from the style it is based on, and overridden by its own.
        assert_eq!(ss.para_lang(Some("SpanishQuote")).as_deref(), Some("es-ES"));
        assert_eq!(ss.para_lang(Some("French")).as_deref(), Some("fr-FR"));
        assert_eq!(ss.para_lang(Some("Missing")).as_deref(), Some("en-US"));
    }
}
