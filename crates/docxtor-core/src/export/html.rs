//! Web page export: one self-contained HTML5 file (pictures as data: URIs, one inline stylesheet).
//! The writer also produces the XHTML bodies of an EPUB (`epub.rs`), in the `Epub` mode.
//!
//! The markup is semantic (headings, lists, tables, figures, links, notes at the end with links back).
//! Formatting comes from Word's styles and direct formatting as classes: one class per paragraph style
//! (`s-Heading1`), and generated classes (`c1`, `c2`...) for what a paragraph, run, table or cell
//! changes against its style. The output is also well-formed XML, which an EPUB needs.

use super::common::*;
use crate::document::Document;
use crate::props::{ParaFmt, RunFmt};
use crate::render::{RBlock, RCell, RPara, RSeg, RTable};
use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;

/// The document as a web page.
pub fn to_html(doc: &Document) -> String {
    // The body a block at a time: the drawing of the whole document is not held.
    let src = Source::bare(doc);
    let mut w = Writer::new(&src, Mode::Html);
    let mut body = String::new();
    let mut first = None;
    stream_blocks(doc, &mut first, &mut |chunk| w.blocks(chunk, &mut body));
    let title = src.title_for(first);
    body.push_str(&w.take_notes());
    let css = w.stylesheet();
    // The head is written first, then put in front of the body, which is not copied into a second buffer.
    let mut out = String::with_capacity(css.len() + 1024);
    let lang = esc_attr(&src.lang);
    let _ = write!(out, "<!doctype html>\n<html lang=\"{lang}\">\n<head>\n<meta charset=\"utf-8\" />\n<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\" />\n<title>{}</title>\n", esc(&title));
    for (name, value) in [("author", &src.props.author), ("description", &src.props.description), ("keywords", &src.props.keywords)] {
        if let Some(v) = value {
            let _ = writeln!(out, "<meta name=\"{name}\" content=\"{}\" />", esc_attr(v));
        }
    }
    out.push_str("<meta name=\"generator\" content=\"Docxtor\" />\n<style>\n");
    out.push_str(&css);
    out.push_str("</style>\n</head>\n<body>\n");
    body.insert_str(0, &out);
    body.push_str("</body>\n</html>\n");
    body
}


#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Html,
    Epub,
}

/// A picture an EPUB carries as a file.
pub struct EpubImage {
    /// Path from the content folder (`images/img1.png`).
    pub path: String,
    pub mime: &'static str,
    pub data: Vec<u8>,
}

/// A heading found while writing (for an EPUB's contents).
pub struct Heading {
    pub level: u8,
    pub id: String,
    pub text: String,
}

pub struct Writer<'a> {
    src: &'a Source<'a>,
    bases: Bases<'a>,
    mode: Mode,
    rules: Vec<(String, String)>,
    /// Rules of paragraph styles: written before the generated classes, which change what a style says.
    style_rules: Vec<(String, String)>,
    classes: HashMap<String, String>,
    style_classes: HashMap<String, Option<String>>,
    /// Blocks that cannot sit inside a paragraph (text boxes, charts), written after it.
    hoist: Vec<String>,
    notes_used: Vec<(String, bool)>,
    /// Bookmark name to the file that holds it (EPUB only).
    pub anchors: HashMap<String, String>,
    pub images: Vec<EpubImage>,
    image_paths: HashMap<String, Option<String>>,
    pub headings: Vec<Heading>,
    heading_count: usize,
    /// Set when the body written since the last `take_flags` holds an equation.
    pub has_math: bool,
    ids: HashSet<String>,
}

fn flag(v: Option<bool>) -> bool {
    v.unwrap_or(false)
}

impl<'a> Writer<'a> {
    pub fn new(src: &'a Source<'a>, mode: Mode) -> Self {
        Writer {
            src,
            bases: Bases::new(src.doc),
            mode,
            rules: Vec::new(),
            style_rules: Vec::new(),
            classes: HashMap::new(),
            style_classes: HashMap::new(),
            hoist: Vec::new(),
            notes_used: Vec::new(),
            anchors: HashMap::new(),
            images: Vec::new(),
            image_paths: HashMap::new(),
            headings: Vec::new(),
            heading_count: 0,
            has_math: false,
            ids: HashSet::new(),
        }
    }

    // -----------------------------------------------------------------------
    // Classes and the stylesheet
    // -----------------------------------------------------------------------

    /// A generated class for a set of declarations (the same declarations share a class).
    fn class(&mut self, prefix: &str, decls: &str) -> Option<String> {
        if decls.is_empty() {
            return None;
        }
        let key = format!("{prefix}|{decls}");
        if let Some(c) = self.classes.get(&key) {
            return Some(c.clone());
        }
        let name = format!("{prefix}{}", self.rules.len() + 1);
        self.rules.push((format!(".{name}"), decls.to_string()));
        self.classes.insert(key, name.clone());
        Some(name)
    }

    /// The class of a paragraph style, with a rule for what it changes against the document defaults.
    fn style_class(&mut self, style: Option<&str>) -> Option<String> {
        let id = style?;
        if Some(id) == self.src.doc.styles.default_para.as_deref() {
            return None;
        }
        if let Some(c) = self.style_classes.get(id) {
            return c.clone();
        }
        let (pf, rf) = self.bases.get(Some(id)).clone();
        let (dp, dr) = self.bases.get(None).clone();
        let decls = [para_css(&para_delta(&pf, &dp, false)), run_css(&run_delta(&rf, &dr), false)].iter().filter(|s| !s.is_empty()).cloned().collect::<Vec<_>>().join(";");
        let name = format!("s-{}", slug(id));
        if !decls.is_empty() {
            self.style_rules.push((format!(".{name}"), decls));
        }
        if flag(pf.contextual_spacing) {
            self.style_rules.push((format!(".{name}+.{name}"), "margin-top:0".into()));
            self.style_rules.push((format!(".{name}:has(+.{name})"), "margin-bottom:0".into()));
        }
        self.style_classes.insert(id.to_string(), Some(name.clone()));
        Some(name)
    }

    /// The stylesheet: page defaults from the document's defaults, then every class used.
    pub fn stylesheet(&mut self) -> String {
        let (pf, rf) = self.bases.get(None).clone();
        let mut css = String::new();
        let mut body = String::new();
        if let Some(f) = &rf.font {
            let _ = write!(body, "font-family:{};", font_family(f));
        }
        if let Some(s) = rf.size {
            let _ = write!(body, "font-size:{}pt;", num(s));
        }
        if let Some(c) = rf.color.as_deref().and_then(hex_color) {
            let _ = write!(body, "color:#{c};");
        }
        if let Some(l) = line_height(&pf) {
            let _ = write!(body, "line-height:{l};");
        }
        if self.mode == Mode::Html {
            let width = self.src.setup.sections.first().map(|s| s.width - s.margin[1] - s.margin[3]).filter(|w| *w > 0).unwrap_or(9360);
            let _ = write!(body, "max-width:{}pt;margin:0 auto;padding:2em 1em;", pt(width));
            if let Some(c) = self.src.setup.page_color.as_deref().and_then(hex_color) {
                let _ = write!(body, "background-color:#{c};");
            }
        }
        body.push_str("tab-size:36pt;");
        let _ = writeln!(css, "body{{{body}}}");
        let mut spacing = String::new();
        let before = if flag(pf.before_auto) { Some(280) } else { pf.space_before };
        let after = if flag(pf.after_auto) { Some(280) } else { pf.space_after };
        let _ = write!(spacing, "margin-top:{}pt;margin-bottom:{}pt;", pt(before.unwrap_or(0)), pt(after.unwrap_or(0)));
        if let Some(a) = pf.align.as_deref().and_then(align_css) {
            let _ = write!(spacing, "text-align:{a};");
        }
        let _ = writeln!(css, "p,li,h1,h2,h3,h4,h5,h6{{{spacing}white-space:pre-wrap}}");
        css.push_str("h1,h2,h3,h4,h5,h6{font-size:inherit;font-weight:inherit}\n");
        css.push_str("blockquote{margin:0}\nul,ol{margin:0;padding-left:36pt}\nli{position:relative}\n");
        css.push_str(".lbl{position:absolute;right:100%;padding-right:6pt;white-space:nowrap}\nli.lb{list-style:none}\n");
        css.push_str("img{max-width:100%;height:auto;vertical-align:baseline}\n.float-left{float:left;margin:0 1em .5em 0}\n.float-right{float:right;margin:0 0 .5em 1em}\n");
        css.push_str("table{border-collapse:collapse;border-spacing:0}\nth{font-weight:inherit;text-align:inherit}\n");
        css.push_str(".page-break{display:block;break-after:page}\n.object{font-style:italic}\n.textbox{border:.5pt solid #888;padding:4pt;margin:6pt 0}\n");
        css.push_str(".noteref{font-size:smaller}\n.footnotes,.endnotes{margin-top:2em;font-size:smaller}\n.footnotes hr,.endnotes hr{width:30%;margin-left:0}\n.note-label{margin-right:.4em}\n");
        css.push_str("figure.chart{margin:1em 0}\nfigure.chart table{font-size:smaller}\nfigure.chart td,figure.chart th{border:.5pt solid #bbb;padding:2pt 6pt;text-align:right}\nfigure.chart th:first-child,figure.chart td:first-child{text-align:left}\n");
        for (sel, decls) in self.style_rules.iter().chain(&self.rules) {
            let _ = writeln!(css, "{sel}{{{decls}}}");
        }
        css
    }

    // -----------------------------------------------------------------------
    // Blocks
    // -----------------------------------------------------------------------

    pub fn blocks(&mut self, blocks: &[RBlock], out: &mut String) {
        for node in group(blocks) {
            match node {
                Node::List(l) => self.list(&l, None, out),
                Node::Block(RBlock::Para(p)) => self.para(p, out),
                Node::Block(RBlock::Table(t)) => self.table(t, out),
                Node::Block(RBlock::Container { blocks, .. }) => self.blocks(blocks, out),
                Node::Block(RBlock::Opaque { .. }) => {}
            }
        }
    }

    fn id_attr(&mut self, id: &str) -> String {
        if self.ids.insert(id.to_string()) {
            format!(" id=\"{}\"", esc_attr(id))
        } else {
            String::new()
        }
    }

    fn anchors_html(&mut self, p: &RPara) -> String {
        let mut s = String::new();
        for b in &p.bookmarks {
            let id = format!("bm-{}", slug(b));
            if self.ids.insert(id.clone()) {
                let _ = write!(s, "<a id=\"{}\"></a>", esc_attr(&id));
            }
        }
        s
    }

    fn classes_for(&mut self, p: &RPara, in_list: bool) -> String {
        let mut names: Vec<String> = Vec::new();
        names.extend(self.style_class(p.style.as_deref()));
        let (base, _) = self.bases.get(p.style.as_deref()).clone();
        let d = para_delta(&p.fmt, &base, in_list);
        let decls = para_css(&d);
        names.extend(self.class("c", &decls));
        if names.is_empty() {
            String::new()
        } else {
            format!(" class=\"{}\"", names.join(" "))
        }
    }

    fn para(&mut self, p: &RPara, out: &mut String) {
        if p.hr {
            out.push_str("<hr />\n");
            return;
        }
        let level = heading_level(p);
        let is_title = p.style.as_deref() == Some("Title");
        let tag = match level {
            Some(n) => format!("h{n}"),
            None if is_title => "h1".into(),
            None => "p".into(),
        };
        let class = self.classes_for(p, false);
        let mut inner = String::new();
        // A numbered heading shows its number.
        if let (Some(l), Some(_)) = (&p.list, level) {
            if !l.text.trim().is_empty() {
                let _ = write!(inner, "<span class=\"num\">{}</span> ", esc(l.text.trim()));
            }
        }
        let mut id = String::new();
        let anchors = self.anchors_html(p);
        let content = self.inline(p);
        if let Some(n) = level {
            self.heading_count += 1;
            let hid = format!("sec-{}", self.heading_count);
            id = self.id_attr(&hid);
            self.headings.push(Heading { level: n, id: hid, text: plain_text(p).trim().to_string() });
        }
        inner.push_str(&anchors);
        inner.push_str(&content);
        let hoisted = std::mem::take(&mut self.hoist);
        if inner.is_empty() && p.segs.is_empty() {
            let _ = writeln!(out, "<{tag}{class}{id}><br /></{tag}>");
        } else if !inner.is_empty() {
            if tag == "p" && p.style.as_deref().is_some_and(|s| s.contains("Quote")) {
                let _ = writeln!(out, "<blockquote><p{class}{id}>{inner}</p></blockquote>");
            } else {
                let _ = writeln!(out, "<{tag}{class}{id}>{inner}</{tag}>");
            }
        }
        for h in hoisted {
            out.push_str(&h);
        }
    }

    // -----------------------------------------------------------------------
    // Lists
    // -----------------------------------------------------------------------

    fn list(&mut self, l: &ListNode, parent_left: Option<i64>, out: &mut String) {
        let doc = self.src.doc;
        let lv = doc.numbering.level(l.num_id, l.level);
        let left = lv.and_then(|v| v.ppr.ind_left).unwrap_or(720 * (l.level + 1));
        let pad = (left - parent_left.unwrap_or(0)).max(0);
        let first = l.items[0].para.list.as_ref().unwrap();
        let fmt = num_fmt(doc, first);
        // Plain "1." / "a." / "iv." labels use the browser's own numbers; anything else shows its labels.
        let native_num = !l.bullet && l.items.iter().all(|i| simple_label(&i.para.list.as_ref().unwrap().text));
        let native_bullet = l.bullet && !first.checklist;
        let literal = !native_num && !native_bullet;
        let tag = if l.bullet { "ul" } else { "ol" };
        let mut decls = format!("padding-left:{}pt", pt(pad));
        if native_num {
            decls.push_str(match fmt {
                NumFmt::Decimal => ";list-style-type:decimal",
                NumFmt::LowerRoman => ";list-style-type:lower-roman",
                NumFmt::UpperRoman => ";list-style-type:upper-roman",
                NumFmt::LowerAlpha => ";list-style-type:lower-alpha",
                NumFmt::UpperAlpha => ";list-style-type:upper-alpha",
            });
        } else if native_bullet {
            decls.push_str(&bullet_style(&first.text));
        }
        let class = self.class("l", &decls).map(|c| format!(" class=\"{c}\"")).unwrap_or_default();
        let start = if native_num { label_value(&first.text, fmt).filter(|v| *v != 1).map(|v| format!(" start=\"{v}\"")).unwrap_or_default() } else { String::new() };
        let _ = writeln!(out, "<{tag}{class}{start}>");
        for item in &l.items {
            let label = item.para.list.as_ref().unwrap();
            let mut li_class = self.classes_for(item.para, true);
            if literal {
                li_class = add_class(&li_class, "lb");
            }
            let anchors = self.anchors_html(item.para);
            let mut inner = anchors;
            if literal {
                let _ = write!(inner, "<span class=\"lbl\">{}</span>", esc(label.text.trim()));
            }
            inner.push_str(&self.inline(item.para));
            let _ = write!(out, "<li{li_class}>{inner}");
            for h in std::mem::take(&mut self.hoist) {
                out.push_str(&h);
            }
            for sub in &item.sub {
                self.list(sub, Some(left), out);
            }
            out.push_str("</li>\n");
        }
        let _ = writeln!(out, "</{tag}>");
    }

    // -----------------------------------------------------------------------
    // Tables
    // -----------------------------------------------------------------------

    fn table(&mut self, t: &RTable, out: &mut String) {
        let ncols = t.grid.len().max(t.rows.iter().map(|r| r.cells.iter().map(|c| c.span.max(1) as usize).sum()).max().unwrap_or(0));
        if ncols == 0 || t.rows.is_empty() {
            return;
        }
        // Where each cell starts, and how many rows a vertically merged cell covers.
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
        let mut decls = String::new();
        let total: i64 = t.grid.iter().sum();
        if t.width_pct {
            if let Some(w) = t.width.filter(|w| *w > 0) {
                let _ = write!(decls, "width:{}%;", num(w as f64 / 50.0));
            }
        } else if let Some(w) = t.width.filter(|w| *w > 0).or((total > 0).then_some(total)) {
            let _ = write!(decls, "width:{}pt;max-width:100%;", pt(w));
        }
        match t.align.as_deref() {
            Some("center") => decls.push_str("margin-left:auto;margin-right:auto;"),
            Some("right" | "end") => decls.push_str("margin-left:auto;"),
            _ => {
                if let Some(i) = t.indent.filter(|i| *i != 0) {
                    let _ = write!(decls, "margin-left:{}pt;", pt(i));
                }
            }
        }
        let class = self.class("t", decls.trim_end_matches(';')).map(|c| format!(" class=\"{c}\"")).unwrap_or_default();
        let _ = writeln!(out, "<table{class}>");
        if t.grid.len() == ncols && total > 0 {
            out.push_str("<colgroup>");
            for w in &t.grid {
                let c = self.class("w", &format!("width:{}pt", pt(*w))).unwrap_or_default();
                let _ = write!(out, "<col class=\"{c}\" />");
            }
            out.push_str("</colgroup>\n");
        }
        let header_rows = t.rows.iter().take_while(|r| r.header).count();
        for (ri, row) in t.rows.iter().enumerate() {
            if ri == 0 && header_rows > 0 {
                out.push_str("<thead>\n");
            }
            if ri == header_rows && header_rows > 0 {
                out.push_str("</thead>\n");
            }
            if ri == header_rows {
                out.push_str("<tbody>\n");
            }
            let rclass = row.height.filter(|h| *h > 0).and_then(|h| self.class("r", &format!("height:{}pt", pt(h)))).map(|c| format!(" class=\"{c}\"")).unwrap_or_default();
            let _ = write!(out, "<tr{rclass}>");
            for (col, cell) in &placed[ri] {
                if cell.vmerge.as_deref() == Some("continue") {
                    continue;
                }
                let span = cell.span.max(1) as usize;
                let mut rows_covered = 1;
                if cell.vmerge.as_deref() == Some("restart") {
                    while continues(ri + rows_covered, *col) {
                        rows_covered += 1;
                    }
                }
                let cdecls = self.cell_css(t, cell, ri, ri + rows_covered - 1, *col, *col + span - 1, t.rows.len(), ncols);
                let cclass = self.class("d", &cdecls).map(|c| format!(" class=\"{c}\"")).unwrap_or_default();
                let tag = if ri < header_rows { "th" } else { "td" };
                let scope = if ri < header_rows { " scope=\"col\"" } else { "" };
                let mut attrs = String::new();
                if span > 1 {
                    let _ = write!(attrs, " colspan=\"{span}\"");
                }
                if rows_covered > 1 {
                    let _ = write!(attrs, " rowspan=\"{rows_covered}\"");
                }
                let mut inner = String::new();
                self.blocks(&cell.blocks, &mut inner);
                let _ = write!(out, "<{tag}{scope}{attrs}{cclass}>{inner}</{tag}>");
            }
            out.push_str("</tr>\n");
        }
        out.push_str("</tbody>\n</table>\n");
    }

    #[allow(clippy::too_many_arguments)]
    fn cell_css(&self, t: &RTable, c: &RCell, row: usize, last_row: usize, col: usize, last_col: usize, nrows: usize, ncols: usize) -> String {
        let mut d = String::new();
        let b = &t.borders;
        let sides = [
            ("top", c.borders.top.as_ref().or(if row == 0 { b.top.as_ref() } else { b.inside_h.as_ref() }).filter(|b| !b.is_nil())),
            ("bottom", c.borders.bottom.as_ref().or(if last_row + 1 >= nrows { b.bottom.as_ref() } else { b.inside_h.as_ref() }).filter(|b| !b.is_nil())),
            ("left", c.borders.left.as_ref().or(if col == 0 { b.left.as_ref() } else { b.inside_v.as_ref() }).filter(|b| !b.is_nil())),
            ("right", c.borders.right.as_ref().or(if last_col + 1 >= ncols { b.right.as_ref() } else { b.inside_v.as_ref() }).filter(|b| !b.is_nil())),
        ];
        for (side, border) in sides {
            if let Some(border) = border {
                let (style, w, color) = border_parts(border);
                let _ = write!(d, "border-{side}:{}pt {style} #{color};", num(w));
            }
        }
        let [top, right, bottom, left] = t.cell_mar;
        let _ = write!(d, "padding:{}pt {}pt {}pt {}pt;", pt(top), pt(right), pt(bottom), pt(left));
        if let Some(fill) = c.shading.as_deref().and_then(hex_color) {
            let _ = write!(d, "background-color:#{fill};");
        }
        match c.valign.as_deref() {
            Some("center") => d.push_str("vertical-align:middle;"),
            Some("bottom") => d.push_str("vertical-align:bottom;"),
            _ => d.push_str("vertical-align:top;"),
        }
        d.trim_end_matches(';').to_string()
    }

    // -----------------------------------------------------------------------
    // Inline content
    // -----------------------------------------------------------------------

    fn inline(&mut self, p: &RPara) -> String {
        let (_, base_run) = self.bases.get(p.style.as_deref()).clone();
        let mut out = String::new();
        let mut open: Option<String> = None;
        for s in &p.segs {
            let link = s.link.as_deref().map(|l| self.href(l)).filter(|l| !l.is_empty());
            if link != open {
                if open.is_some() {
                    out.push_str("</a>");
                }
                if let Some(h) = &link {
                    let _ = write!(out, "<a href=\"{}\">", esc_attr(h));
                }
                open = link.clone();
            }
            let piece = self.seg(s, &base_run, link.is_some());
            out.push_str(&piece);
        }
        if open.is_some() {
            out.push_str("</a>");
        }
        out
    }

    /// The address a link goes to: bookmarks become `#bm-name`, in an EPUB with the file that holds them.
    fn href(&self, link: &str) -> String {
        if let Some(name) = link.strip_prefix('#') {
            let id = format!("bm-{}", slug(name));
            return match self.anchors.get(name) {
                Some(file) => format!("{file}#{id}"),
                None => format!("#{id}"),
            };
        }
        safe_link(link).map(String::from).unwrap_or_default()
    }

    fn seg(&mut self, s: &RSeg, base: &RunFmt, in_link: bool) -> String {
        match s.k {
            "t" => {
                let text = s.x.as_deref().unwrap_or("");
                if text.is_empty() {
                    return String::new();
                }
                self.wrap(&esc(text), &s.f, base, in_link)
            }
            "tab" => self.wrap("\t", &s.f, base, in_link),
            "br" | "cbr" => "<br />".into(),
            "pbr" => "<span class=\"page-break\"></span>".into(),
            "img" => self.image(s),
            "math" => self.math(s),
            "note" => self.note_ref(s),
            "chart" => {
                let fig = self.chart(s);
                self.hoist.push(fig);
                String::new()
            }
            "shape" => self.shape(s),
            "obj" => match s.label.as_deref().filter(|l| !l.is_empty()) {
                Some(l) => format!("<span class=\"object\">[{}]</span>", esc(l)),
                None => String::new(),
            },
            // Deleted text is not part of the document.
            _ => String::new(),
        }
    }

    /// Text in its tags: semantic ones for bold, italic, underline, strike and super/subscript, and a class
    /// for the rest of what differs from the paragraph.
    fn wrap(&mut self, text: &str, f: &RunFmt, base: &RunFmt, in_link: bool) -> String {
        let mut d = run_delta(f, base);
        let mut tags: Vec<&str> = Vec::new();
        if d.bold == Some(true) {
            tags.push("strong");
            d.bold = None;
        }
        if d.italic == Some(true) {
            tags.push("em");
            d.italic = None;
        }
        if d.underline.as_deref() == Some("single") {
            if !in_link {
                tags.push("u");
            }
            d.underline = None;
        }
        if d.strike == Some(true) && d.dstrike.is_none() {
            tags.push("s");
            d.strike = None;
        }
        match d.vert_align.as_deref() {
            Some("superscript") => {
                tags.push("sup");
                d.vert_align = None;
            }
            Some("subscript") => {
                tags.push("sub");
                d.vert_align = None;
            }
            _ => {}
        }
        let class = self.class("c", &run_css(&d, true));
        let mut out = String::new();
        if let Some(c) = &class {
            let _ = write!(out, "<span class=\"{c}\">");
        }
        for t in &tags {
            let _ = write!(out, "<{t}>");
        }
        out.push_str(text);
        for t in tags.iter().rev() {
            let _ = write!(out, "</{t}>");
        }
        if class.is_some() {
            out.push_str("</span>");
        }
        out
    }

    fn image_src(&mut self, part: &str) -> Option<String> {
        if part.starts_with("http://") || part.starts_with("https://") {
            return (self.mode == Mode::Html).then(|| part.to_string());
        }
        let doc = self.src.doc;
        match self.mode {
            Mode::Html => {
                let img = image_data(doc, part)?;
                if web_image(img.mime) {
                    return Some(format!("data:{};base64,{}", img.mime, base64(&img.bytes)));
                }
                // TIFF, EMF and WMF go in as the PNG or SVG the editor shows.
                doc.media_display(part).map(|d| format!("data:{};base64,{}", d.mime, base64(&d.bytes)))
            }
            Mode::Epub => {
                if let Some(p) = self.image_paths.get(part) {
                    return p.clone();
                }
                let shown = image_data(doc, part).and_then(|i| {
                    if epub_image(i.mime) {
                        Some((i.ext, i.mime, i.bytes.to_vec()))
                    } else {
                        doc.media_display(part).map(|d| (d.ext, d.mime, d.bytes))
                    }
                });
                let path = shown.map(|(ext, mime, data)| {
                    let path = format!("images/img{}.{}", self.images.len() + 1, ext);
                    self.images.push(EpubImage { path: path.clone(), mime, data });
                    path
                });
                self.image_paths.insert(part.to_string(), path.clone());
                path
            }
        }
    }

    fn image(&mut self, s: &RSeg) -> String {
        let Some(img) = &s.img else { return String::new() };
        let alt = img.alt.clone().unwrap_or_default();
        let Some(src) = self.image_src(&img.src) else {
            return if alt.is_empty() { String::new() } else { format!("<span class=\"object\">[{}]</span>", esc(&alt)) };
        };
        let mut attrs = String::new();
        if img.w > 0.0 && img.h > 0.0 {
            let _ = write!(attrs, " width=\"{}\" height=\"{}\"", (img.w * 4.0 / 3.0).round() as i64, (img.h * 4.0 / 3.0).round() as i64);
        }
        let float = match (img.layout.wrap.as_str(), img.layout.align.as_deref()) {
            ("square" | "tight", Some("left")) => " class=\"float-left\"",
            ("square" | "tight", Some("right")) => " class=\"float-right\"",
            _ => "",
        };
        format!("<img src=\"{}\" alt=\"{}\"{attrs}{float} />", esc_attr(&src), esc_attr(&alt))
    }

    fn math(&mut self, s: &RSeg) -> String {
        let Some(m) = &s.math else { return String::new() };
        if m.mathml.trim().is_empty() {
            return format!("<code>{}</code>", esc(&m.latex));
        }
        self.has_math = true;
        m.mathml.clone()
    }

    fn note_ref(&mut self, s: &RSeg) -> String {
        let id = s.x.as_deref().unwrap_or("");
        if id.is_empty() {
            return String::new();
        }
        let endnote = s.note == Some("endnote");
        if !self.notes_used.iter().any(|(n, e)| n == id && *e == endnote) {
            self.notes_used.push((id.to_string(), endnote));
        }
        let key = format!("{}{}", if endnote { "en" } else { "fn" }, slug(id));
        let epub = if self.mode == Mode::Epub { " epub:type=\"noteref\"" } else { "" };
        format!("<sup class=\"noteref\"><a id=\"ref-{key}\" href=\"#{key}\"{epub} role=\"doc-noteref\">{}</a></sup>", esc(s.label.as_deref().unwrap_or("")))
    }

    fn shape(&mut self, s: &RSeg) -> String {
        let Some(sh) = &s.shape else { return String::new() };
        if let Some(text) = sh.text.as_ref().filter(|t| !t.is_empty()) {
            let mut inner = String::new();
            self.blocks(text, &mut inner);
            if !inner.trim().is_empty() {
                self.hoist.push(format!("<aside class=\"textbox\">\n{inner}</aside>\n"));
                return String::new();
            }
        }
        match sh.alt.as_deref().filter(|a| !a.is_empty()) {
            Some(alt) => format!("<span class=\"object\" role=\"img\" aria-label=\"{}\">[{}]</span>", esc_attr(alt), esc(alt)),
            None => String::new(),
        }
    }

    /// A chart as a figure with its data in a table.
    fn chart(&mut self, s: &RSeg) -> String {
        let Some(c) = &s.chart else {
            // One of the new chart types: its name and title, as text.
            return match &s.chartex {
                Some(x) => {
                    let text = x.summary();
                    format!("<figure class=\"chart\" role=\"img\" aria-label=\"{}\"><figcaption>{}</figcaption></figure>\n", esc_attr(&text), esc(&text))
                }
                None => String::new(),
            };
        };
        let title = c.title.clone().or_else(|| s.img.as_ref().and_then(|i| i.alt.clone())).unwrap_or_else(|| "Chart".into());
        if c.series.is_empty() {
            return format!("<figure class=\"chart\" role=\"img\" aria-label=\"{}\"><figcaption>{}</figcaption></figure>\n", esc_attr(&title), esc(&title));
        }
        let mut out = format!("<figure class=\"chart\" role=\"group\" aria-label=\"{}\">\n<figcaption>{}</figcaption>\n<table>\n<thead><tr><th scope=\"col\"></th>", esc_attr(&title), esc(&title));
        for se in &c.series {
            let _ = write!(out, "<th scope=\"col\">{}</th>", esc(&se.name));
        }
        out.push_str("</tr></thead>\n<tbody>\n");
        let rows = c.series.iter().map(|x| x.values.len()).max().unwrap_or(0).max(c.categories.len());
        for i in 0..rows {
            let label = c.categories.get(i).cloned().or_else(|| c.series[0].x_values.as_ref().and_then(|x| x.get(i).copied().flatten()).map(num)).unwrap_or_else(|| (i + 1).to_string());
            let _ = write!(out, "<tr><th scope=\"row\">{}</th>", esc(&label));
            for se in &c.series {
                let _ = write!(out, "<td>{}</td>", se.values.get(i).copied().flatten().map(num).unwrap_or_default());
            }
            out.push_str("</tr>\n");
        }
        out.push_str("</tbody>\n</table>\n</figure>\n");
        out
    }

    // -----------------------------------------------------------------------
    // Notes
    // -----------------------------------------------------------------------

    /// The notes referenced since the last call, as sections with links back.
    pub fn take_notes(&mut self) -> String {
        let used = std::mem::take(&mut self.notes_used);
        let mut out = String::new();
        for endnote in [false, true] {
            let mut section = String::new();
            for (id, en) in used.iter().filter(|(_, e)| *e == endnote) {
                let Some(note) = self.src.note(id, *en) else { continue };
                let key = format!("{}{}", if *en { "en" } else { "fn" }, slug(id));
                let mut body = String::new();
                self.blocks(&note.blocks, &mut body);
                self.hoist.clear();
                let epub = if self.mode == Mode::Epub { format!(" epub:type=\"{}\"", if *en { "endnote" } else { "footnote" }) } else { String::new() };
                let role = if *en { "doc-endnote" } else { "doc-footnote" };
                let label = format!("<sup class=\"note-label\">{}</sup>", esc(&note.label));
                let back = format!(" <a href=\"#ref-{key}\"{} role=\"doc-backlink\">\u{21A9}</a>", if self.mode == Mode::Epub { " epub:type=\"backlink\"" } else { "" });
                let body = splice_note(&body, &label, &back);
                let _ = write!(section, "<aside id=\"{key}\" role=\"{role}\"{epub}>\n{body}</aside>\n");
            }
            if !section.is_empty() {
                let (class, role) = if endnote { ("endnotes", "doc-endnotes") } else { ("footnotes", "doc-footnotes") };
                let epub = if self.mode == Mode::Epub { format!(" epub:type=\"{class}\"") } else { String::new() };
                let _ = write!(out, "<section class=\"{class}\" role=\"{role}\"{epub}>\n<hr />\n{section}</section>\n");
            }
        }
        out
    }

    /// Start a new page of a book: ids may repeat from file to file.
    pub fn begin_page(&mut self) {
        self.ids.clear();
        self.hoist.clear();
        self.notes_used.clear();
    }

    /// Whether the body written since the last call had equations; clears the flag.
    pub fn take_math(&mut self) -> bool {
        std::mem::take(&mut self.has_math)
    }
}

/// A note's paragraphs with its number in front of the first and a link back at the end of the last.
fn splice_note(body: &str, label: &str, back: &str) -> String {
    let mut s = body.to_string();
    if let Some(end) = s.rfind("</p>") {
        s.insert_str(end, back);
    } else {
        let _ = writeln!(s, "<p>{}</p>", back.trim());
    }
    if s.starts_with("<p") {
        if let Some(gt) = s.find('>') {
            // Word's note text starts with a space after the number.
            let space = if s[gt + 1..].starts_with(' ') { "" } else { " " };
            s.insert_str(gt + 1, &format!("{label}{space}"));
        }
    }
    s
}

fn add_class(attr: &str, name: &str) -> String {
    if let Some(rest) = attr.strip_prefix(" class=\"") {
        format!(" class=\"{name} {rest}")
    } else {
        format!(" class=\"{name}\"")
    }
}

/// A label that the browser's own numbering reproduces: letters or digits and a full stop or bracket.
fn simple_label(text: &str) -> bool {
    let t = text.trim();
    let body = t.trim_end_matches(['.', ')']);
    !body.is_empty() && body.len() + 1 >= t.len() && body.len() < 8 && body.chars().all(|c| c.is_ascii_alphanumeric())
}

fn bullet_style(glyph: &str) -> String {
    match glyph.trim() {
        "" | "\u{2022}" | "\u{00B7}" | "\u{F0B7}" | "-" | "*" => String::new(),
        "o" | "\u{25E6}" | "\u{25CB}" => ";list-style-type:circle".into(),
        "\u{25AA}" | "\u{25A0}" | "\u{00A7}" | "\u{F0A7}" => ";list-style-type:square".into(),
        g if g.chars().count() == 1 && !g.is_ascii() => format!(";list-style-type:\"{} \"", g.replace(['"', '\\'], "")),
        _ => String::new(),
    }
}

fn align_css(a: &str) -> Option<&'static str> {
    Some(match a {
        "center" => "center",
        "right" | "end" => "right",
        "both" | "distribute" => "justify",
        "left" | "start" => "left",
        _ => return None,
    })
}

/// A font name as a CSS string followed by the generic family. Only letters, digits, spaces and a few marks stay: a name from a file
/// cannot end the string, end the style sheet (`</style>`) or add declarations of its own.
fn font_family(name: &str) -> String {
    let clean: String = name.chars().filter(|c| c.is_alphanumeric() || matches!(c, ' ' | '-' | '_' | '.' | ',' | '+' | '&' | '#')).collect();
    format!("'{}',{}", clean.trim(), generic_font(name))
}

/// CSS line-height for Word's line spacing.
fn line_height(f: &ParaFmt) -> Option<String> {
    let line = f.line.filter(|l| *l > 0)?;
    match f.line_rule.as_deref() {
        Some("exact" | "atLeast") => Some(format!("{}pt", pt(line))),
        // A percentage, as Word writes it: the reader takes a bare number as a multiple of the font size (Google Docs'
        // 1.38) and a percentage as Word's multiple, so this comes back as the same spacing.
        _ => Some(format!("{}%", num(line as f64 / 240.0 * 100.0))),
    }
}

/// CSS declarations for what a paragraph changes (no trailing semicolon).
fn para_css(d: &ParaFmt) -> String {
    let mut v: Vec<String> = Vec::new();
    if let Some(a) = d.align.as_deref().and_then(align_css) {
        v.push(format!("text-align:{a}"));
    }
    if flag(d.before_auto) {
        v.push("margin-top:14pt".into());
    } else if let Some(x) = d.space_before {
        v.push(format!("margin-top:{}pt", pt(x)));
    }
    if flag(d.after_auto) {
        v.push("margin-bottom:14pt".into());
    } else if let Some(x) = d.space_after {
        v.push(format!("margin-bottom:{}pt", pt(x)));
    }
    if let Some(l) = line_height(d) {
        v.push(format!("line-height:{l}"));
    }
    if let Some(x) = d.ind_left {
        v.push(format!("margin-left:{}pt", pt(x)));
    }
    if let Some(x) = d.ind_right {
        v.push(format!("margin-right:{}pt", pt(x)));
    }
    if let Some(x) = d.ind_hanging.filter(|x| *x > 0) {
        v.push(format!("text-indent:-{}pt", pt(x)));
    } else if let Some(x) = d.ind_first_line {
        v.push(format!("text-indent:{}pt", pt(x)));
    }
    if d.keep_next == Some(true) {
        v.push("break-after:avoid".into());
    }
    if d.keep_lines == Some(true) {
        v.push("break-inside:avoid".into());
    }
    if d.page_break_before == Some(true) {
        v.push("break-before:page".into());
    }
    if let Some(c) = d.shading.as_deref().and_then(hex_color) {
        v.push(format!("background-color:#{c}"));
    }
    if let Some(b) = &d.borders {
        for (side, border) in [("top", &b.top), ("bottom", &b.bottom), ("left", &b.left), ("right", &b.right)] {
            if let Some(border) = border {
                let (style, w, color) = border_parts(border);
                v.push(format!("border-{side}:{}pt {style} #{color}", num(w)));
                if border.space > 0 {
                    v.push(format!("padding-{side}:{}pt", border.space));
                }
            }
        }
    }
    if d.bidi == Some(true) {
        v.push("direction:rtl".into());
    }
    v.join(";")
}

/// CSS declarations for what a run changes. With `semantic`, bold, italic, underline, strike and
/// super/subscript are left to tags (their "off" values stay).
fn run_css(d: &RunFmt, semantic: bool) -> String {
    let mut v: Vec<String> = Vec::new();
    if let Some(f) = &d.font {
        v.push(format!("font-family:{}", font_family(f)));
    }
    if let Some(s) = d.size {
        v.push(format!("font-size:{}pt", num(s)));
    }
    if let Some(c) = d.color.as_deref().and_then(hex_color) {
        v.push(format!("color:#{c}"));
    }
    let shade = d.highlight.as_deref().and_then(highlight_hex).map(String::from).or_else(|| d.shading.as_deref().and_then(hex_color));
    if let Some(c) = shade {
        v.push(format!("background-color:#{c}"));
    }
    match d.bold {
        Some(true) if !semantic => v.push("font-weight:bold".into()),
        Some(false) => v.push("font-weight:normal".into()),
        _ => {}
    }
    match d.italic {
        Some(true) if !semantic => v.push("font-style:italic".into()),
        Some(false) => v.push("font-style:normal".into()),
        _ => {}
    }
    let mut lines: Vec<&str> = Vec::new();
    let mut deco_style = "";
    match d.underline.as_deref() {
        Some("none") => lines.push("none"),
        Some(u) if !(semantic && u == "single") => {
            lines.push("underline");
            deco_style = match u {
                "double" => " double",
                "dotted" | "dottedHeavy" => " dotted",
                "dash" | "dashLong" | "dashedHeavy" | "dashLongHeavy" => " dashed",
                "wave" | "wavyHeavy" | "wavyDouble" => " wavy",
                _ => "",
            };
        }
        _ => {}
    }
    if d.dstrike == Some(true) {
        lines.push("line-through");
        deco_style = " double";
    } else if d.strike == Some(true) && !semantic {
        lines.push("line-through");
    }
    if !lines.is_empty() {
        if lines == ["none"] {
            v.push("text-decoration:none".into());
        } else {
            lines.retain(|l| *l != "none");
            v.push(format!("text-decoration:{}{deco_style}", lines.join(" ")));
        }
    }
    if d.caps == Some(true) {
        v.push("text-transform:uppercase".into());
    }
    if d.small_caps == Some(true) {
        v.push("font-variant:small-caps".into());
    }
    if let Some(s) = d.spacing.filter(|s| *s != 0) {
        v.push(format!("letter-spacing:{}pt", pt(s)));
    }
    if d.vanish == Some(true) {
        v.push("display:none".into());
    }
    if !semantic {
        match d.vert_align.as_deref() {
            Some("superscript") => v.push("vertical-align:super;font-size:smaller".into()),
            Some("subscript") => v.push("vertical-align:sub;font-size:smaller".into()),
            _ => {}
        }
    }
    v.join(";")
}
