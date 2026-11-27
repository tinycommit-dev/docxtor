//! OpenDocument text export (ODF 1.3): a zip with `mimetype` first and stored, `META-INF/manifest.xml`,
//! `content.xml`, `styles.xml` and `meta.xml`, pictures in `Pictures/`.
//!
//! Word's paragraph styles become named styles (with headings keeping their outline levels); what a
//! paragraph or run changes against its style becomes an automatic style. Lists, tables with column
//! widths and cell borders, pictures, links, bookmarks, footnotes and endnotes, page breaks, the first
//! section's page size and margins, and its header and footer are written. Equations are written as
//! their LaTeX text, and charts and plain shapes as a placeholder.

use super::common::*;
use crate::document::Document;
use crate::numbering::ListLabel;
use crate::props::{ParaFmt, RunFmt};
use crate::render::{RBlock, RCell, RPara, RSeg, RTable};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt::Write as _;

const NS: &str = "xmlns:office=\"urn:oasis:names:tc:opendocument:xmlns:office:1.0\" xmlns:style=\"urn:oasis:names:tc:opendocument:xmlns:style:1.0\" xmlns:text=\"urn:oasis:names:tc:opendocument:xmlns:text:1.0\" xmlns:table=\"urn:oasis:names:tc:opendocument:xmlns:table:1.0\" xmlns:draw=\"urn:oasis:names:tc:opendocument:xmlns:drawing:1.0\" xmlns:fo=\"urn:oasis:names:tc:opendocument:xmlns:xsl-fo-compatible:1.0\" xmlns:xlink=\"http://www.w3.org/1999/xlink\" xmlns:dc=\"http://purl.org/dc/elements/1.1/\" xmlns:meta=\"urn:oasis:names:tc:opendocument:xmlns:meta:1.0\" xmlns:svg=\"urn:oasis:names:tc:opendocument:xmlns:svg-compatible:1.0\"";
const MIME: &str = "application/vnd.oasis.opendocument.text";

/// The document as an OpenDocument text file.
pub fn to_odt(doc: &Document) -> Result<Vec<u8>, String> {
    let src = Source::new(doc);
    let mut w = Odt::new(&src);
    w.collect_labels(&src.blocks);

    let mut body = String::new();
    w.blocks(trim_end(&src.blocks), &mut body);
    if body.is_empty() {
        body.push_str("<text:p text:style-name=\"Standard\"/>");
    }
    let content_auto = std::mem::replace(&mut w.auto, Auto::new("MP"));
    let (header, footer, layout) = w.master_page();
    let page_auto = std::mem::replace(&mut w.auto, Auto::new(""));
    let named = w.named_styles();
    let lang = &src.lang;
    let (language, country) = lang.split_once('-').map(|(l, c)| (l.to_string(), Some(c.to_string()))).unwrap_or((lang.clone(), None));
    let (dp, dr) = w.bases.defaults();
    let default_text = with_lang(w.text_props(&dr), &language, country.as_deref());
    let default_para = format!("{}{default_text}", paragraph_props(&para_delta(&dp, &ParaFmt::default(), false)));

    let content = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<office:document-content {NS} office:version=\"1.3\">\n<office:scripts/>\n<office:font-face-decls>{}</office:font-face-decls>\n<office:automatic-styles>{}</office:automatic-styles>\n<office:body>\n<office:text>\n{body}</office:text>\n</office:body>\n</office:document-content>\n",
        w.font_decls(),
        content_auto.xml
    );
    let master = format!(
        "<style:master-page style:name=\"Standard\" style:page-layout-name=\"pm1\">{}{}</style:master-page>",
        header.map(|h| format!("<style:header>{h}</style:header>")).unwrap_or_default(),
        footer.map(|f| format!("<style:footer>{f}</style:footer>")).unwrap_or_default()
    );
    let styles = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<office:document-styles {NS} office:version=\"1.3\">\n<office:font-face-decls>{}</office:font-face-decls>\n<office:styles>\n<style:default-style style:family=\"paragraph\">{default_para}</style:default-style>\n<style:style style:name=\"Graphics\" style:family=\"graphic\"><style:graphic-properties text:anchor-type=\"paragraph\" svg:x=\"0pt\" svg:y=\"0pt\" style:wrap=\"dynamic\" style:number-wrapped-paragraphs=\"no-limit\" style:wrap-contour=\"false\" style:vertical-pos=\"top\" style:vertical-rel=\"paragraph\" style:horizontal-pos=\"center\" style:horizontal-rel=\"paragraph\"/></style:style>\n{named}</office:styles>\n<office:automatic-styles>\n{layout}{}</office:automatic-styles>\n<office:master-styles>\n{master}\n</office:master-styles>\n</office:document-styles>\n",
        w.font_decls(),
        page_auto.xml
    );

    let (created, modified) = core_dates(doc);
    let mut meta = format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<office:document-meta xmlns:office=\"urn:oasis:names:tc:opendocument:xmlns:office:1.0\" xmlns:xlink=\"http://www.w3.org/1999/xlink\" xmlns:dc=\"http://purl.org/dc/elements/1.1/\" xmlns:meta=\"urn:oasis:names:tc:opendocument:xmlns:meta:1.0\" office:version=\"1.3\">\n<office:meta>\n<meta:generator>Docxtor</meta:generator>\n<dc:title>{}</dc:title>\n", esc(&src.title));
    if let Some(s) = &src.props.subject {
        let _ = writeln!(meta, "<dc:subject>{}</dc:subject>", esc(s));
    }
    if let Some(s) = &src.props.description {
        let _ = writeln!(meta, "<dc:description>{}</dc:description>", esc(s));
    }
    if let Some(k) = &src.props.keywords {
        for word in k.split([',', ';']).map(str::trim).filter(|w| !w.is_empty()) {
            let _ = writeln!(meta, "<meta:keyword>{}</meta:keyword>", esc(word));
        }
    }
    if let Some(a) = &src.props.author {
        let _ = writeln!(meta, "<dc:creator>{}</dc:creator>", esc(a));
    }
    if let Some(d) = created.as_deref().and_then(timestamp) {
        let _ = writeln!(meta, "<meta:creation-date>{d}</meta:creation-date>");
    }
    if let Some(d) = modified.as_deref().and_then(timestamp) {
        let _ = writeln!(meta, "<dc:date>{d}</dc:date>");
    }
    let _ = writeln!(meta, "<dc:language>{}</dc:language>\n</office:meta>\n</office:document-meta>", esc(lang));

    let mut manifest = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<manifest:manifest xmlns:manifest=\"urn:oasis:names:tc:opendocument:xmlns:manifest:1.0\" manifest:version=\"1.3\">\n");
    let _ = writeln!(manifest, " <manifest:file-entry manifest:full-path=\"/\" manifest:version=\"1.3\" manifest:media-type=\"{MIME}\"/>");
    for name in ["content.xml", "styles.xml", "meta.xml"] {
        let _ = writeln!(manifest, " <manifest:file-entry manifest:full-path=\"{name}\" manifest:media-type=\"text/xml\"/>");
    }
    for (path, mime, _) in &w.pictures {
        let _ = writeln!(manifest, " <manifest:file-entry manifest:full-path=\"{}\" manifest:media-type=\"{mime}\"/>", esc_attr(path));
    }
    manifest.push_str("</manifest:manifest>\n");

    let mut entries = vec![
        ZipEntry::stored("mimetype", MIME),
        ZipEntry::new("META-INF/manifest.xml", manifest),
        ZipEntry::new("content.xml", content),
        ZipEntry::new("styles.xml", styles),
        ZipEntry::new("meta.xml", meta),
    ];
    for (path, _, data) in w.pictures {
        entries.push(ZipEntry::new(path, data));
    }
    write_zip(&entries)
}

/// Text properties with the document language added.
fn with_lang(props: String, language: &str, country: Option<&str>) -> String {
    let mut attrs = format!("fo:language=\"{}\"", esc_attr(language));
    if let Some(c) = country {
        let _ = write!(attrs, " fo:country=\"{}\"", esc_attr(c));
    }
    if props.is_empty() {
        format!("<style:text-properties {attrs}/>")
    } else {
        props.replacen("<style:text-properties ", &format!("<style:text-properties {attrs} "), 1)
    }
}

/// Automatic styles of one file (the body, or the page's header and footer), by what they hold.
struct Auto {
    prefix: &'static str,
    xml: String,
    names: HashMap<String, String>,
    count: usize,
    lists: HashSet<i64>,
}

impl Auto {
    fn new(prefix: &'static str) -> Auto {
        Auto { prefix, xml: String::new(), names: HashMap::new(), count: 0, lists: HashSet::new() }
    }

    /// The name of a style with this key, written by `build` the first time.
    fn add(&mut self, kind: &str, key: &str, build: impl FnOnce(&str) -> String) -> String {
        let k = format!("{kind}|{key}");
        if let Some(n) = self.names.get(&k) {
            return n.clone();
        }
        self.count += 1;
        let name = format!("{}{kind}{}", self.prefix, self.count);
        self.xml.push_str(&build(&name));
        self.xml.push('\n');
        self.names.insert(k, name.clone());
        name
    }
}

struct Odt<'a> {
    src: &'a Source<'a>,
    bases: Bases<'a>,
    auto: Auto,
    fonts: BTreeMap<String, &'static str>,
    pictures: Vec<(String, &'static str, Vec<u8>)>,
    picture_names: HashMap<String, Option<String>>,
    /// Named paragraph styles in use: style id to (name, display name).
    styles: BTreeMap<String, (String, String)>,
    taken_names: HashSet<String>,
    /// The first label seen at each list level (for the bullet glyph).
    labels: HashMap<(i64, i64), ListLabel>,
    frames: usize,
    notes: usize,
}

fn len(twips: i64) -> String {
    format!("{}pt", pt(twips))
}

impl<'a> Odt<'a> {
    fn new(src: &'a Source<'a>) -> Self {
        Odt {
            src,
            bases: Bases::new(src.doc),
            auto: Auto::new(""),
            fonts: BTreeMap::new(),
            pictures: Vec::new(),
            picture_names: HashMap::new(),
            styles: BTreeMap::new(),
            taken_names: HashSet::from(["Standard".to_string(), "Graphics".to_string()]),
            labels: HashMap::new(),
            frames: 0,
            notes: 0,
        }
    }

    /// The first label seen at each list level, in the body and the notes.
    fn collect_labels(&mut self, blocks: &[RBlock]) {
        self.walk_labels(blocks);
        let src = self.src;
        for n in src.setup.notes.iter().chain(&src.setup.endnotes) {
            self.walk_labels(&n.blocks);
        }
    }

    fn walk_labels(&mut self, blocks: &[RBlock]) {
        for b in blocks {
            match b {
                RBlock::Para(p) => {
                    if let Some(l) = &p.list {
                        self.labels.entry((l.num_id, l.level)).or_insert_with(|| l.clone());
                    }
                }
                RBlock::Table(t) => {
                    for c in t.rows.iter().flat_map(|r| &r.cells) {
                        self.walk_labels(&c.blocks);
                    }
                }
                RBlock::Container { blocks, .. } => self.walk_labels(blocks),
                RBlock::Opaque { .. } => {}
            }
        }
    }

    // -----------------------------------------------------------------------
    // Styles
    // -----------------------------------------------------------------------

    fn font(&mut self, name: &str) -> String {
        let name = name.replace(['"', '<', '>', '&'], "");
        self.fonts.entry(name.clone()).or_insert_with(|| generic_font(&name));
        name
    }

    fn font_decls(&self) -> String {
        let mut out = String::new();
        for (name, generic) in &self.fonts {
            let family = if name.contains(' ') { format!("'{name}'") } else { name.clone() };
            let g = match *generic {
                "serif" => "roman",
                "monospace" => "modern",
                _ => "swiss",
            };
            let _ = write!(out, "<style:font-face style:name=\"{}\" svg:font-family=\"{}\" style:font-family-generic=\"{g}\"/>", esc_attr(name), esc_attr(&family));
        }
        out
    }

    /// The name of a paragraph style, registering it (and the styles it builds on) for `styles.xml`.
    fn style_name(&mut self, id: Option<&str>) -> String {
        let doc = self.src.doc;
        let Some(id) = id.filter(|i| Some(*i) != doc.styles.default_para.as_deref()) else { return "Standard".into() };
        if let Some((n, _)) = self.styles.get(id) {
            return n.clone();
        }
        let info = doc.styles.get(id);
        let display = info.map(|i| i.name.clone()).unwrap_or_else(|| id.to_string());
        let (mut name, mut display) = odf_style_name(&display);
        while !self.taken_names.insert(name.clone()) {
            name.push('x');
            display.push(' ');
        }
        self.styles.insert(id.to_string(), (name.clone(), display));
        if let Some(parent) = info.and_then(|i| i.based_on.clone()) {
            self.style_name(Some(&parent));
        }
        if let Some(next) = info.and_then(|i| i.next.clone()) {
            self.style_name(Some(&next));
        }
        name
    }

    /// The named styles for `styles.xml`.
    fn named_styles(&mut self) -> String {
        let doc = self.src.doc;
        let mut out = String::new();
        // "Standard" is Word's Normal style.
        let (defaults_p, defaults_r) = self.bases.defaults();
        let (np, nr) = self.bases.get(None).clone();
        let (dp, dr) = (para_delta(&np, &defaults_p, false), run_delta(&nr, &defaults_r));
        let _ = writeln!(out, "<style:style style:name=\"Standard\" style:family=\"paragraph\" style:class=\"text\">{}{}</style:style>", paragraph_props(&dp), self.text_props(&dr));
        // New names can appear while writing (a style's `next` style), so loop until there are none left.
        let mut done: HashSet<String> = HashSet::new();
        loop {
            let pending: Vec<(String, (String, String))> = self.styles.iter().filter(|(id, _)| !done.contains(*id)).map(|(k, v)| (k.clone(), v.clone())).collect();
            if pending.is_empty() {
                break;
            }
            for (id, (name, display)) in pending {
                done.insert(id.clone());
                let info = doc.styles.get(&id);
                let parent_id = info.and_then(|i| i.based_on.clone()).filter(|p| doc.styles.exists(p));
                let (pf, rf) = self.bases.get(Some(&id)).clone();
                let (bp, br) = match &parent_id {
                    Some(p) => self.bases.get(Some(p)).clone(),
                    None => (defaults_p.clone(), defaults_r.clone()),
                };
                let parent = parent_id.as_deref().map(|p| self.style_name(Some(p)));
                let next = info.and_then(|i| i.next.clone()).filter(|n| *n != id).map(|n| self.style_name(Some(&n)));
                let mut attrs = format!("style:name=\"{}\"", esc_attr(&name));
                if display != name {
                    let _ = write!(attrs, " style:display-name=\"{}\"", esc_attr(&display));
                }
                attrs.push_str(" style:family=\"paragraph\"");
                if let Some(p) = &parent {
                    let _ = write!(attrs, " style:parent-style-name=\"{}\"", esc_attr(p));
                }
                if let Some(n) = &next {
                    let _ = write!(attrs, " style:next-style-name=\"{}\"", esc_attr(n));
                }
                attrs.push_str(" style:class=\"text\"");
                if let Some(l) = pf.outline_lvl.filter(|l| (0..9).contains(l)) {
                    let _ = write!(attrs, " style:default-outline-level=\"{}\"", l + 1);
                }
                let _ = writeln!(out, "<style:style {attrs}>{}{}</style:style>", paragraph_props(&para_delta(&pf, &bp, false)), self.text_props(&run_delta(&rf, &br)));
            }
        }
        out
    }

    /// `style:text-properties` for what a run changes (empty when nothing).
    fn text_props(&mut self, d: &RunFmt) -> String {
        let mut a: Vec<String> = Vec::new();
        if let Some(f) = &d.font {
            let n = self.font(f);
            a.push(format!("style:font-name=\"{}\"", esc_attr(&n)));
        }
        if let Some(s) = d.size {
            a.push(format!("fo:font-size=\"{}pt\"", num(s)));
        }
        if let Some(c) = d.color.as_deref().and_then(hex_color) {
            a.push(format!("fo:color=\"#{c}\""));
        }
        let shade = d.highlight.as_deref().and_then(highlight_hex).map(String::from).or_else(|| d.shading.as_deref().and_then(hex_color));
        if let Some(c) = shade {
            a.push(format!("fo:background-color=\"#{c}\""));
        }
        if let Some(b) = d.bold {
            a.push(format!("fo:font-weight=\"{}\"", if b { "bold" } else { "normal" }));
        }
        if let Some(i) = d.italic {
            a.push(format!("fo:font-style=\"{}\"", if i { "italic" } else { "normal" }));
        }
        match d.underline.as_deref() {
            Some("none") => a.push("style:text-underline-style=\"none\"".into()),
            Some(u) => {
                let style = match u {
                    "dotted" | "dottedHeavy" => "dotted",
                    "dash" | "dashLong" | "dashedHeavy" | "dashLongHeavy" => "dash",
                    "wave" | "wavyHeavy" | "wavyDouble" => "wave",
                    _ => "solid",
                };
                a.push(format!("style:text-underline-style=\"{style}\" style:text-underline-width=\"auto\" style:text-underline-color=\"font-color\""));
                if u == "double" {
                    a.push("style:text-underline-type=\"double\"".into());
                }
            }
            None => {}
        }
        match (d.dstrike, d.strike) {
            (Some(true), _) => a.push("style:text-line-through-style=\"solid\" style:text-line-through-type=\"double\"".into()),
            (_, Some(true)) => a.push("style:text-line-through-style=\"solid\"".into()),
            (_, Some(false)) | (Some(false), _) => a.push("style:text-line-through-style=\"none\"".into()),
            _ => {}
        }
        if let Some(c) = d.caps {
            a.push(format!("fo:text-transform=\"{}\"", if c { "uppercase" } else { "none" }));
        }
        if let Some(c) = d.small_caps {
            a.push(format!("fo:font-variant=\"{}\"", if c { "small-caps" } else { "normal" }));
        }
        match d.vert_align.as_deref() {
            Some("superscript") => a.push("style:text-position=\"super 58%\"".into()),
            Some("subscript") => a.push("style:text-position=\"sub 58%\"".into()),
            Some("baseline") => a.push("style:text-position=\"0% 100%\"".into()),
            _ => {}
        }
        if let Some(s) = d.spacing.filter(|s| *s != 0) {
            a.push(format!("fo:letter-spacing=\"{}\"", len(s)));
        }
        if d.vanish == Some(true) {
            a.push("text:display=\"none\"".into());
        }
        if a.is_empty() {
            String::new()
        } else {
            format!("<style:text-properties {}/>", a.join(" "))
        }
    }

    /// The name of the style for a paragraph: the named style, or an automatic one when it changes something.
    fn para_style(&mut self, named: &str, d: &ParaFmt) -> String {
        let props = paragraph_props(d);
        if props.is_empty() {
            return named.to_string();
        }
        self.auto.add("P", &format!("{named}|{props}"), |n| format!("<style:style style:name=\"{n}\" style:family=\"paragraph\" style:parent-style-name=\"{}\">{props}</style:style>", esc_attr(named)))
    }

    // -----------------------------------------------------------------------
    // Blocks
    // -----------------------------------------------------------------------

    fn blocks(&mut self, blocks: &[RBlock], out: &mut String) {
        for node in group(blocks) {
            match node {
                Node::List(l) => self.list(&l, out),
                Node::Block(RBlock::Para(p)) => self.para(p, out, false),
                Node::Block(RBlock::Table(t)) => self.table(t, out),
                Node::Block(RBlock::Container { blocks, .. }) => self.blocks(blocks, out),
                Node::Block(RBlock::Opaque { .. }) => {}
            }
        }
    }

    fn para(&mut self, p: &RPara, out: &mut String, in_list: bool) {
        let level = heading_level(p);
        let (base_p, base_r) = self.bases.get(p.style.as_deref()).clone();
        let delta = para_delta(&p.fmt, &base_p, in_list);
        let named = self.style_name(p.style.as_deref());
        // A page break in the middle starts a new paragraph on a new page.
        let mut chunks: Vec<Vec<&RSeg>> = vec![Vec::new()];
        for s in &p.segs {
            if s.k == "pbr" {
                chunks.push(Vec::new());
            } else {
                chunks.last_mut().unwrap().push(s);
            }
        }
        let mut first = true;
        for (i, chunk) in chunks.iter().enumerate() {
            if i == 0 && chunk.is_empty() && chunks.len() > 1 {
                continue;
            }
            let mut d = delta.clone();
            if i > 0 {
                d.page_break_before = Some(true);
            }
            let style = self.para_style(&named, &d);
            let mut inner = String::new();
            if first {
                for b in &p.bookmarks {
                    let _ = write!(inner, "<text:bookmark text:name=\"{}\"/>", esc_attr(b));
                }
                if level.is_some() {
                    if let Some(l) = p.list.as_ref().filter(|l| !l.text.trim().is_empty()) {
                        let _ = write!(inner, "{} ", esc(l.text.trim()));
                    }
                }
            }
            inner.push_str(&self.inline(chunk, &base_r));
            match level {
                Some(n) if first => {
                    let _ = writeln!(out, "<text:h text:style-name=\"{}\" text:outline-level=\"{n}\">{inner}</text:h>", esc_attr(&style));
                }
                _ if inner.is_empty() => {
                    let _ = writeln!(out, "<text:p text:style-name=\"{}\"/>", esc_attr(&style));
                }
                _ => {
                    let _ = writeln!(out, "<text:p text:style-name=\"{}\">{inner}</text:p>", esc_attr(&style));
                }
            }
            first = false;
        }
    }

    // -----------------------------------------------------------------------
    // Lists
    // -----------------------------------------------------------------------

    fn list_style(&mut self, num_id: i64) -> String {
        let name = format!("{}L{num_id}", self.auto.prefix);
        if !self.auto.lists.insert(num_id) {
            return name;
        }
        let doc = self.src.doc;
        let mut xml = format!("<text:list-style style:name=\"{name}\">");
        for lvl in 0..9i64 {
            let lv = doc.numbering.level(num_id, lvl);
            let seen = self.labels.get(&(num_id, lvl));
            if lv.is_none() && seen.is_none() {
                continue;
            }
            let ppr = lv.map(|l| &l.ppr);
            let left = ppr.and_then(|p| p.ind_left).unwrap_or(720 * (lvl + 1));
            let hanging = ppr.and_then(|p| p.ind_hanging).or_else(|| ppr.and_then(|p| p.ind_first_line).map(|f| -f)).unwrap_or(360);
            let follow = match lv.map(|l| l.suff.as_str()) {
                Some("space") => "space",
                Some("nothing") => "nothing",
                _ => "listtab",
            };
            let align = format!(
                "<style:list-level-properties text:list-level-position-and-space-mode=\"label-alignment\"><style:list-level-label-alignment text:label-followed-by=\"{follow}\" text:list-tab-stop-position=\"{}\" fo:text-indent=\"{}\" fo:margin-left=\"{}\"/></style:list-level-properties>",
                len(left),
                len(-hanging),
                len(left)
            );
            let bullet = lv.map(|l| l.fmt == "bullet").unwrap_or(true) || seen.is_some_and(|s| s.bullet);
            if bullet {
                let glyph = bullet_glyph(seen.map(|s| s.text.as_str()).or(lv.map(|l| l.text.as_str())).unwrap_or(""));
                let _ = write!(xml, "<text:list-level-style-bullet text:level=\"{}\" text:bullet-char=\"{}\">{align}</text:list-level-style-bullet>", lvl + 1, esc_attr(&glyph.to_string()));
            } else {
                let lv = lv.unwrap();
                let fmt = match lv.fmt.as_str() {
                    "lowerRoman" => "i",
                    "upperRoman" => "I",
                    "lowerLetter" => "a",
                    "upperLetter" => "A",
                    _ => "1",
                };
                let (prefix, suffix, shown) = split_pattern(&lv.text);
                let mut attrs = String::new();
                if !prefix.is_empty() {
                    let _ = write!(attrs, " style:num-prefix=\"{}\"", esc_attr(&prefix));
                }
                if !suffix.is_empty() {
                    let _ = write!(attrs, " style:num-suffix=\"{}\"", esc_attr(&suffix));
                }
                let _ = write!(xml, "<text:list-level-style-number text:level=\"{}\"{attrs} style:num-format=\"{fmt}\" text:display-levels=\"{shown}\" text:start-value=\"{}\">{align}</text:list-level-style-number>", lvl + 1, lv.start.max(0));
            }
        }
        xml.push_str("</text:list-style>\n");
        self.auto.xml.push_str(&xml);
        name
    }

    fn list(&mut self, l: &ListNode, out: &mut String) {
        let style = self.list_style(l.num_id);
        let _ = writeln!(out, "<text:list text:style-name=\"{style}\">");
        self.list_items(l, out);
        out.push_str("</text:list>\n");
    }

    fn list_items(&mut self, l: &ListNode, out: &mut String) {
        let doc = self.src.doc;
        for (i, item) in l.items.iter().enumerate() {
            let label = item.para.list.as_ref().unwrap();
            // A numbered list that carries on after something else starts at the number it shows.
            let mut start = String::new();
            if i == 0 && !l.bullet {
                let lv_start = doc.numbering.level(label.num_id, label.level).map(|v| v.start).unwrap_or(1);
                if let Some(v) = label_value(&label.text, num_fmt(doc, label)).filter(|v| *v as i64 != lv_start) {
                    start = format!(" text:start-value=\"{v}\"");
                }
            }
            let _ = write!(out, "<text:list-item{start}>");
            self.para(item.para, out, true);
            for sub in &item.sub {
                out.push_str("<text:list>");
                self.list_items(sub, out);
                out.push_str("</text:list>");
            }
            out.push_str("</text:list-item>\n");
        }
    }

    // -----------------------------------------------------------------------
    // Tables
    // -----------------------------------------------------------------------

    fn table(&mut self, t: &RTable, out: &mut String) {
        let ncols = t.grid.len().max(t.rows.iter().map(|r| r.cells.iter().map(|c| c.span.max(1) as usize).sum()).max().unwrap_or(0));
        if ncols == 0 || t.rows.is_empty() {
            return;
        }
        let text_width = self.src.setup.sections.first().map(|s| s.width - s.margin[1] - s.margin[3]).filter(|w| *w > 0).unwrap_or(9360);
        let total: i64 = t.grid.iter().sum();
        let width = if t.width_pct { t.width.map(|w| text_width * w / 5000) } else { t.width }.filter(|w| *w > 0).or((total > 0).then_some(total)).unwrap_or(text_width);
        let mut props = format!("style:width=\"{}\"", len(width));
        match t.align.as_deref() {
            Some("center") => props.push_str(" table:align=\"center\""),
            Some("right" | "end") => props.push_str(" table:align=\"right\""),
            _ => {
                props.push_str(" table:align=\"left\"");
                if let Some(i) = t.indent.filter(|i| *i != 0) {
                    let _ = write!(props, " fo:margin-left=\"{}\"", len(i));
                }
            }
        }
        let tname = self.auto.add("Tbl", &props, |n| format!("<style:style style:name=\"{n}\" style:family=\"table\"><style:table-properties {props}/></style:style>"));
        let _ = writeln!(out, "<table:table table:name=\"{tname}\" table:style-name=\"{tname}\">");
        // Column widths: the grid, or an even share when the grid is missing.
        let widths: Vec<i64> = if t.grid.len() == ncols { t.grid.clone() } else { vec![width / ncols as i64; ncols] };
        for w in &widths {
            let cname = self.auto.add("Col", &w.to_string(), |n| format!("<style:style style:name=\"{n}\" style:family=\"table-column\"><style:table-column-properties style:column-width=\"{}\"/></style:style>", len(*w)));
            let _ = writeln!(out, "<table:table-column table:style-name=\"{cname}\"/>");
        }
        let placed: Vec<Vec<(usize, &RCell)>> = t
            .rows
            .iter()
            .map(|r| {
                let mut col = 0;
                r.cells
                    .iter()
                    .map(|c| {
                        let at = col;
                        col += c.span.max(1) as usize;
                        (at, c)
                    })
                    .collect()
            })
            .collect();
        let continues = |row: usize, col: usize| placed.get(row).is_some_and(|r| r.iter().any(|(c, cell)| *c == col && cell.vmerge.as_deref() == Some("continue")));
        let header_rows = t.rows.iter().take_while(|r| r.header).count();
        for (ri, row) in t.rows.iter().enumerate() {
            if ri == 0 && header_rows > 0 {
                out.push_str("<table:table-header-rows>\n");
            }
            let rstyle = row.height.filter(|h| *h > 0).map(|h| self.auto.add("Row", &h.to_string(), |n| format!("<style:style style:name=\"{n}\" style:family=\"table-row\"><style:table-row-properties style:min-row-height=\"{}\"/></style:style>", len(h))));
            match rstyle {
                Some(s) => {
                    let _ = write!(out, "<table:table-row table:style-name=\"{s}\">");
                }
                None => out.push_str("<table:table-row>"),
            }
            let mut col = 0;
            for (c, cell) in &placed[ri] {
                let span = cell.span.max(1) as usize;
                debug_assert_eq!(*c, col);
                if cell.vmerge.as_deref() == Some("continue") {
                    for _ in 0..span {
                        out.push_str("<table:covered-table-cell/>");
                    }
                } else {
                    let mut rows_covered = 1;
                    if cell.vmerge.as_deref() == Some("restart") {
                        while continues(ri + rows_covered, *c) {
                            rows_covered += 1;
                        }
                    }
                    let cprops = self.cell_props(t, cell, ri, ri + rows_covered - 1, *c, *c + span - 1, t.rows.len(), ncols);
                    let cname = self.auto.add("Cell", &cprops, |n| format!("<style:style style:name=\"{n}\" style:family=\"table-cell\"><style:table-cell-properties {cprops}/></style:style>"));
                    let mut attrs = format!(" table:style-name=\"{cname}\" office:value-type=\"string\"");
                    if span > 1 {
                        let _ = write!(attrs, " table:number-columns-spanned=\"{span}\"");
                    }
                    if rows_covered > 1 {
                        let _ = write!(attrs, " table:number-rows-spanned=\"{rows_covered}\"");
                    }
                    let mut inner = String::new();
                    self.blocks(&cell.blocks, &mut inner);
                    if inner.is_empty() {
                        inner.push_str("<text:p text:style-name=\"Standard\"/>");
                    }
                    let _ = write!(out, "<table:table-cell{attrs}>{inner}</table:table-cell>");
                    for _ in 1..span {
                        out.push_str("<table:covered-table-cell/>");
                    }
                }
                col += span;
            }
            // A short row is filled up so every row has the same number of cells.
            for _ in col..ncols {
                out.push_str("<table:table-cell office:value-type=\"string\"><text:p text:style-name=\"Standard\"/></table:table-cell>");
            }
            out.push_str("</table:table-row>\n");
            if ri + 1 == header_rows {
                out.push_str("</table:table-header-rows>\n");
            }
        }
        out.push_str("</table:table>\n");
    }

    #[allow(clippy::too_many_arguments)]
    fn cell_props(&self, t: &RTable, c: &RCell, row: usize, last_row: usize, col: usize, last_col: usize, nrows: usize, ncols: usize) -> String {
        let b = &t.borders;
        let sides = [
            ("top", c.borders.top.as_ref().or(if row == 0 { b.top.as_ref() } else { b.inside_h.as_ref() }).filter(|b| !b.is_nil())),
            ("bottom", c.borders.bottom.as_ref().or(if last_row + 1 >= nrows { b.bottom.as_ref() } else { b.inside_h.as_ref() }).filter(|b| !b.is_nil())),
            ("left", c.borders.left.as_ref().or(if col == 0 { b.left.as_ref() } else { b.inside_v.as_ref() }).filter(|b| !b.is_nil())),
            ("right", c.borders.right.as_ref().or(if last_col + 1 >= ncols { b.right.as_ref() } else { b.inside_v.as_ref() }).filter(|b| !b.is_nil())),
        ];
        let [top, right, bottom, left] = t.cell_mar;
        let mut a = vec![format!("fo:padding-top=\"{}\" fo:padding-right=\"{}\" fo:padding-bottom=\"{}\" fo:padding-left=\"{}\"", len(top), len(right), len(bottom), len(left))];
        for (side, border) in sides {
            let v = match border {
                Some(border) => {
                    let (style, w, color) = border_parts(border);
                    format!("{}pt {style} #{color}", num(w))
                }
                None => "none".into(),
            };
            a.push(format!("fo:border-{side}=\"{v}\""));
        }
        if let Some(fill) = c.shading.as_deref().and_then(hex_color) {
            a.push(format!("fo:background-color=\"#{fill}\""));
        }
        a.push(
            match c.valign.as_deref() {
                Some("center") => "style:vertical-align=\"middle\"",
                Some("bottom") => "style:vertical-align=\"bottom\"",
                _ => "style:vertical-align=\"top\"",
            }
            .into(),
        );
        a.join(" ")
    }

    // -----------------------------------------------------------------------
    // Inline content
    // -----------------------------------------------------------------------

    fn inline(&mut self, segs: &[&RSeg], base: &RunFmt) -> String {
        let mut out = String::new();
        let mut open: Option<String> = None;
        for s in segs {
            let link = s.link.as_deref().and_then(safe_link).map(String::from);
            if link != open {
                if open.is_some() {
                    out.push_str("</text:a>");
                }
                if let Some(h) = &link {
                    let _ = write!(out, "<text:a xlink:type=\"simple\" xlink:href=\"{}\">", esc_attr(h));
                }
                open = link.clone();
            }
            out.push_str(&self.seg(s, base));
        }
        if open.is_some() {
            out.push_str("</text:a>");
        }
        out
    }

    fn span(&mut self, f: &RunFmt, base: &RunFmt, inner: &str) -> String {
        let d = run_delta(f, base);
        let props = self.text_props(&d);
        if props.is_empty() {
            return inner.to_string();
        }
        let name = self.auto.add("T", &props, |n| format!("<style:style style:name=\"{n}\" style:family=\"text\">{props}</style:style>"));
        format!("<text:span text:style-name=\"{name}\">{inner}</text:span>")
    }

    fn seg(&mut self, s: &RSeg, base: &RunFmt) -> String {
        match s.k {
            "t" => {
                let text = s.x.as_deref().unwrap_or("");
                if text.is_empty() {
                    return String::new();
                }
                let inner = match s.fld.as_deref() {
                    Some("PAGE") => format!("<text:page-number text:select-page=\"current\">{}</text:page-number>", esc(text)),
                    Some("NUMPAGES") => format!("<text:page-count>{}</text:page-count>", esc(text)),
                    _ => odf_text(text),
                };
                self.span(&s.f, base, &inner)
            }
            "tab" => self.span(&s.f, base, "<text:tab/>"),
            "br" | "cbr" => "<text:line-break/>".into(),
            "img" => self.image(s),
            "math" => match &s.math {
                Some(m) => self.span(&s.f, base, &odf_text(&if m.display { format!("$${}$$", m.latex) } else { format!("${}$", m.latex) })),
                None => String::new(),
            },
            "note" => self.note(s),
            "chart" => {
                let title = s.chart.as_ref().and_then(|c| c.title.clone()).or_else(|| s.chartex.as_ref().and_then(|c| c.title.clone())).or_else(|| s.img.as_ref().and_then(|i| i.alt.clone())).unwrap_or_default();
                let label = if title.is_empty() { "[Chart]".to_string() } else { format!("[Chart: {title}]") };
                self.span(&s.f, base, &odf_text(&label))
            }
            "shape" => self.shape(s, base),
            "obj" => match s.label.as_deref().filter(|l| !l.is_empty()) {
                Some(l) => self.span(&s.f, base, &odf_text(&format!("[{l}]"))),
                None => String::new(),
            },
            _ => String::new(),
        }
    }

    fn frame_name(&mut self, kind: &str) -> String {
        self.frames += 1;
        format!("{kind}{}", self.frames)
    }

    fn picture(&mut self, part: &str) -> Option<String> {
        if let Some(p) = self.picture_names.get(part) {
            return p.clone();
        }
        let path = image_data(self.src.doc, part).map(|img| {
            let path = format!("Pictures/image{}.{}", self.pictures.len() + 1, img.ext);
            self.pictures.push((path.clone(), img.mime, img.bytes.to_vec()));
            path
        });
        self.picture_names.insert(part.to_string(), path.clone());
        path
    }

    fn image(&mut self, s: &RSeg) -> String {
        let Some(img) = &s.img else { return String::new() };
        let alt = img.alt.clone().unwrap_or_default();
        let Some(path) = self.picture(&img.src) else {
            return if alt.is_empty() { String::new() } else { odf_text(&format!("[{alt}]")) };
        };
        let (anchor, gstyle) = match (img.layout.wrap.as_str(), img.layout.align.as_deref()) {
            ("square" | "tight", Some("left")) => ("paragraph", "style:wrap=\"parallel\" style:number-wrapped-paragraphs=\"no-limit\" style:horizontal-pos=\"left\" style:horizontal-rel=\"paragraph\" style:vertical-pos=\"top\" style:vertical-rel=\"paragraph\" fo:margin-right=\"8pt\" fo:margin-bottom=\"4pt\""),
            ("square" | "tight", Some("right")) => ("paragraph", "style:wrap=\"parallel\" style:number-wrapped-paragraphs=\"no-limit\" style:horizontal-pos=\"right\" style:horizontal-rel=\"paragraph\" style:vertical-pos=\"top\" style:vertical-rel=\"paragraph\" fo:margin-left=\"8pt\" fo:margin-bottom=\"4pt\""),
            _ => ("as-char", "style:wrap=\"none\" style:vertical-pos=\"top\" style:vertical-rel=\"baseline\""),
        };
        let fr = self.auto.add("fr", gstyle, |n| format!("<style:style style:name=\"{n}\" style:family=\"graphic\" style:parent-style-name=\"Graphics\"><style:graphic-properties {gstyle}/></style:style>"));
        let name = self.frame_name("Image");
        let (w, h) = (if img.w > 0.0 { img.w } else { 72.0 }, if img.h > 0.0 { img.h } else { 72.0 });
        let desc = if alt.is_empty() { String::new() } else { format!("<svg:title/><svg:desc>{}</svg:desc>", esc(&alt)) };
        format!(
            "<draw:frame draw:style-name=\"{fr}\" draw:name=\"{name}\" text:anchor-type=\"{anchor}\" svg:width=\"{}pt\" svg:height=\"{}pt\" draw:z-index=\"0\"><draw:image xlink:href=\"{}\" xlink:type=\"simple\" xlink:show=\"embed\" xlink:actuate=\"onLoad\"/>{desc}</draw:frame>",
            num(w),
            num(h),
            esc_attr(&path)
        )
    }

    fn note(&mut self, s: &RSeg) -> String {
        let id = s.x.as_deref().unwrap_or("");
        if id.is_empty() {
            return String::new();
        }
        let endnote = s.note == Some("endnote");
        let Some(note) = self.src.note(id, endnote) else { return String::new() };
        self.notes += 1;
        let (class, prefix) = if endnote { ("endnote", "edn") } else { ("footnote", "ftn") };
        let mut body = String::new();
        self.blocks(&note.blocks, &mut body);
        if body.is_empty() {
            body.push_str("<text:p text:style-name=\"Standard\"/>");
        }
        format!("<text:note text:id=\"{prefix}{}\" text:note-class=\"{class}\"><text:note-citation>{}</text:note-citation><text:note-body>{body}</text:note-body></text:note>", self.notes, esc(&note.label))
    }

    fn shape(&mut self, s: &RSeg, base: &RunFmt) -> String {
        let Some(sh) = &s.shape else { return String::new() };
        if let Some(text) = sh.text.as_ref().filter(|t| !t.is_empty()) {
            let mut inner = String::new();
            self.blocks(text, &mut inner);
            if !inner.trim().is_empty() {
                let fr = self.auto.add("fr", "textbox", |n| format!("<style:style style:name=\"{n}\" style:family=\"graphic\" style:parent-style-name=\"Graphics\"><style:graphic-properties style:wrap=\"none\" style:vertical-pos=\"top\" style:vertical-rel=\"baseline\" fo:padding=\"3pt\" fo:border=\"0.5pt solid #888888\"/></style:style>"));
                let name = self.frame_name("Text box");
                let w = if sh.w > 0.0 { sh.w } else { 144.0 };
                return format!("<draw:frame draw:style-name=\"{fr}\" draw:name=\"{name}\" text:anchor-type=\"as-char\" svg:width=\"{}pt\" draw:z-index=\"0\"><draw:text-box>{inner}</draw:text-box></draw:frame>", num(w));
            }
        }
        match sh.alt.as_deref().filter(|a| !a.is_empty()) {
            Some(alt) => self.span(&s.f, base, &odf_text(&format!("[{alt}]"))),
            None => String::new(),
        }
    }

    // -----------------------------------------------------------------------
    // Page layout, header and footer
    // -----------------------------------------------------------------------

    /// The first section's header and footer content and the page layout (`pm1`) for `styles.xml`.
    fn master_page(&mut self) -> (Option<String>, Option<String>, String) {
        let setup = &self.src.setup;
        let sec = setup.sections.first();
        let (w, h, margin, header, footer) = match sec {
            Some(s) => (s.width, s.height, s.margin, s.header, s.footer),
            None => (setup.width, setup.height, setup.margin, setup.header, setup.footer),
        };
        let story = |map: Option<&HashMap<String, String>>| -> Option<Vec<RBlock>> {
            let part = map?.get("default")?;
            let blocks = setup.parts.get(part)?.blocks.clone();
            (!blocks.is_empty()).then_some(blocks)
        };
        let head = story(sec.map(|s| &s.headers));
        let foot = story(sec.map(|s| &s.footers));
        let mut header_xml = None;
        let mut footer_xml = None;
        if let Some(blocks) = head {
            let mut s = String::new();
            self.blocks(&blocks, &mut s);
            header_xml = Some(s);
        }
        if let Some(blocks) = foot {
            let mut s = String::new();
            self.blocks(&blocks, &mut s);
            footer_xml = Some(s);
        }
        let mut props = format!("fo:page-width=\"{}\" fo:page-height=\"{}\" style:print-orientation=\"{}\"", len(w), len(h), if w > h { "landscape" } else { "portrait" });
        let (top, bottom) = (if header_xml.is_some() { header.min(margin[0]) } else { margin[0] }, if footer_xml.is_some() { footer.min(margin[2]) } else { margin[2] });
        let _ = write!(props, " fo:margin-top=\"{}\" fo:margin-bottom=\"{}\" fo:margin-left=\"{}\" fo:margin-right=\"{}\"", len(top), len(bottom), len(margin[3]), len(margin[1]));
        if let Some(c) = setup.page_color.as_deref().and_then(hex_color) {
            let _ = write!(props, " fo:background-color=\"#{c}\"");
        }
        let mut layout = format!("<style:page-layout style:name=\"pm1\"><style:page-layout-properties {props}/>");
        if header_xml.is_some() {
            let _ = write!(layout, "<style:header-style><style:header-footer-properties fo:min-height=\"{}\" fo:margin-left=\"0pt\" fo:margin-right=\"0pt\" fo:margin-bottom=\"0pt\"/></style:header-style>", len((margin[0] - top).max(0)));
        }
        if footer_xml.is_some() {
            let _ = write!(layout, "<style:footer-style><style:header-footer-properties fo:min-height=\"{}\" fo:margin-left=\"0pt\" fo:margin-right=\"0pt\" fo:margin-top=\"0pt\"/></style:footer-style>", len((margin[2] - bottom).max(0)));
        }
        layout.push_str("</style:page-layout>\n");
        (header_xml, footer_xml, layout)
    }
}

/// Paragraph properties for what a paragraph changes (empty when nothing).
fn paragraph_props(d: &ParaFmt) -> String {
    let mut a: Vec<String> = Vec::new();
    if let Some(al) = d.align.as_deref() {
        let v = match al {
            "center" => Some("center"),
            "right" | "end" => Some("end"),
            "both" | "distribute" => Some("justify"),
            "left" | "start" => Some("start"),
            _ => None,
        };
        if let Some(v) = v {
            a.push(format!("fo:text-align=\"{v}\""));
        }
    }
    if d.before_auto == Some(true) {
        a.push("fo:margin-top=\"14pt\"".into());
    } else if let Some(x) = d.space_before {
        a.push(format!("fo:margin-top=\"{}\"", len(x)));
    }
    if d.after_auto == Some(true) {
        a.push("fo:margin-bottom=\"14pt\"".into());
    } else if let Some(x) = d.space_after {
        a.push(format!("fo:margin-bottom=\"{}\"", len(x)));
    }
    if let Some(l) = d.line.filter(|l| *l > 0) {
        match d.line_rule.as_deref() {
            Some("exact") => a.push(format!("fo:line-height=\"{}\"", len(l))),
            Some("atLeast") => a.push(format!("style:line-height-at-least=\"{}\"", len(l))),
            _ => a.push(format!("fo:line-height=\"{}%\"", (l as f64 / 240.0 * 100.0).round() as i64)),
        }
    }
    if let Some(x) = d.ind_left {
        a.push(format!("fo:margin-left=\"{}\"", len(x)));
    }
    if let Some(x) = d.ind_right {
        a.push(format!("fo:margin-right=\"{}\"", len(x)));
    }
    if let Some(x) = d.ind_hanging.filter(|x| *x > 0) {
        a.push(format!("fo:text-indent=\"{}\"", len(-x)));
    } else if let Some(x) = d.ind_first_line {
        a.push(format!("fo:text-indent=\"{}\"", len(x)));
    }
    if d.page_break_before == Some(true) {
        a.push("fo:break-before=\"page\"".into());
    }
    if let Some(k) = d.keep_next {
        a.push(format!("fo:keep-with-next=\"{}\"", if k { "always" } else { "auto" }));
    }
    if let Some(k) = d.keep_lines {
        a.push(format!("fo:keep-together=\"{}\"", if k { "always" } else { "auto" }));
    }
    if let Some(c) = d.contextual_spacing {
        a.push(format!("style:contextual-spacing=\"{c}\""));
    }
    if let Some(c) = d.shading.as_deref().and_then(hex_color) {
        a.push(format!("fo:background-color=\"#{c}\""));
    }
    if let Some(b) = &d.borders {
        for (side, border) in [("top", &b.top), ("bottom", &b.bottom), ("left", &b.left), ("right", &b.right)] {
            if let Some(border) = border {
                let (style, w, color) = border_parts(border);
                a.push(format!("fo:border-{side}=\"{}pt {style} #{color}\"", num(w)));
                if border.space > 0 {
                    a.push(format!("fo:padding-{side}=\"{}pt\"", border.space));
                }
            }
        }
    }
    if d.bidi == Some(true) {
        a.push("style:writing-mode=\"rl-tb\"".into());
    }
    let mut tabs = String::new();
    if let Some(stops) = &d.tabs {
        for t in stops {
            let ty = match t.val.as_str() {
                "center" => "center",
                "right" | "end" => "right",
                "decimal" => "char",
                "bar" | "clear" => continue,
                _ => "left",
            };
            let mut attrs = format!("style:position=\"{}\" style:type=\"{ty}\"", len(t.pos));
            if ty == "char" {
                attrs.push_str(" style:char=\".\"");
            }
            match t.leader.as_deref() {
                Some("dot") => attrs.push_str(" style:leader-style=\"dotted\" style:leader-text=\".\""),
                Some("hyphen") => attrs.push_str(" style:leader-style=\"dash\" style:leader-text=\"-\""),
                Some("underscore") => attrs.push_str(" style:leader-style=\"solid\" style:leader-text=\"_\""),
                Some("middleDot") => attrs.push_str(" style:leader-style=\"dotted\" style:leader-text=\"\u{B7}\""),
                _ => {}
            }
            let _ = write!(tabs, "<style:tab-stop {attrs}/>");
        }
    }
    if a.is_empty() && tabs.is_empty() {
        return String::new();
    }
    if tabs.is_empty() {
        format!("<style:paragraph-properties {}/>", a.join(" "))
    } else {
        format!("<style:paragraph-properties {}><style:tab-stops>{tabs}</style:tab-stops></style:paragraph-properties>", a.join(" "))
    }
}

/// An ODF style name and display name for a Word style name (`heading 1` is `Heading_20_1`).
fn odf_style_name(word: &str) -> (String, String) {
    let lower = word.to_lowercase();
    let display = match lower.strip_prefix("heading ") {
        Some(n) if n.parse::<u8>().is_ok() => format!("Heading {n}"),
        _ => {
            let mut c = word.chars();
            match c.next() {
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                None => "Style".into(),
            }
        }
    };
    let mut name = String::new();
    for c in display.chars() {
        if c.is_ascii_alphanumeric() || c == '-' || c == '.' {
            name.push(c);
        } else if c == ' ' {
            name.push_str("_20_");
        } else {
            let _ = write!(name, "_{:x}_", c as u32);
        }
    }
    if !name.chars().next().is_some_and(|c| c.is_ascii_alphabetic()) {
        name.insert(0, 'S');
    }
    (name, display)
}

/// The character a bullet list shows.
fn bullet_glyph(label: &str) -> char {
    match label.trim().chars().next() {
        None => '\u{2022}',
        Some('o') => '\u{25E6}',
        Some('\u{F0A7}' | '\u{00A7}') => '\u{25AA}',
        Some('\u{F0D8}') => '\u{27A2}',
        Some('\u{F0FC}') => '\u{2713}',
        Some(c) if ('\u{E000}'..='\u{F8FF}').contains(&c) => '\u{2022}',
        Some(c) if c.is_ascii_alphanumeric() => '\u{2022}',
        Some(c) => c,
    }
}

/// A numbering pattern (`%1.`, `(%1)`, `%1.%2.`) as its prefix, its suffix and how many levels it shows.
fn split_pattern(pattern: &str) -> (String, String, usize) {
    let b: Vec<char> = pattern.chars().collect();
    let (mut first, mut last, mut shown) = (None, None, 0);
    let mut i = 0;
    while i + 1 < b.len() {
        if b[i] == '%' && b[i + 1].is_ascii_digit() {
            first.get_or_insert(i);
            last = Some(i + 2);
            shown += 1;
            i += 2;
        } else {
            i += 1;
        }
    }
    match (first, last) {
        (Some(f), Some(l)) => (b[..f].iter().collect(), b[l..].iter().collect(), shown.max(1)),
        _ => (String::new(), String::new(), 1),
    }
}

/// Text as ODF holds it: runs of spaces, tabs and line breaks are elements.
fn odf_text(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        match c {
            ' ' => {
                let mut n = 1;
                while chars.get(i + n) == Some(&' ') {
                    n += 1;
                }
                let edge = i == 0 || i + n == chars.len();
                if n == 1 && !edge {
                    out.push(' ');
                } else if n == 1 {
                    out.push_str("<text:s/>");
                } else {
                    let _ = write!(out, "<text:s text:c=\"{n}\"/>");
                }
                i += n;
                continue;
            }
            '\t' => out.push_str("<text:tab/>"),
            '\n' => out.push_str("<text:line-break/>"),
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            c => out.push_str(&esc(&c.to_string())),
        }
        i += 1;
    }
    out
}
