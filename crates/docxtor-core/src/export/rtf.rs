//! Rich Text Format export (RTF 1.9): font and colour tables, a stylesheet from Word's paragraph
//! styles, paragraphs with their formatting, multilevel lists (`\listtable`), tables (`\trowd`),
//! pictures as hex (`\pngblip`, `\jpegblip`, `\emfblip`), links as fields, bookmarks, footnotes and
//! endnotes, the page size and margins of the first section and its header and footer.
//!
//! Text outside ASCII is written as `\uN?` (or `\'hh` for Latin-1), so the file is plain ASCII.
//! Equations are written as their LaTeX text, and charts, text boxes and plain shapes as a placeholder.

use super::common::*;
use crate::document::Document;
use crate::numbering::ListLabel;
use crate::props::{ParaFmt, RunFmt};
use crate::render::{RBlock, RCell, RPara, RSeg, RTable};
use std::collections::{BTreeMap, HashMap};
use std::fmt::Write as _;

/// The document as Rich Text.
pub fn to_rtf(doc: &Document) -> String {
    let src = Source::new(doc);
    let mut w = Rtf::new(&src);
    w.collect_labels(&src.blocks);

    let mut body = String::new();
    w.blocks(trim_end(&src.blocks), &mut body, false);
    let (header, footer) = w.page_parts();
    let styles = w.stylesheet();
    let lists = w.list_table();

    let mut out = String::with_capacity(body.len() + 4096);
    let lcid = lcid(&src.lang);
    let _ = writeln!(out, "{{\\rtf1\\ansi\\ansicpg1252\\uc1\\deff0\\deflang{lcid}\\deflangfe{lcid}");
    out.push_str("{\\fonttbl");
    for (i, (name, family)) in w.fonts.iter().enumerate() {
        let _ = write!(out, "{{\\f{i}\\{family}\\fcharset0 {};}}", rtf_text(name).replace(';', ""));
    }
    out.push_str("}\n{\\colortbl;");
    for c in &w.colors {
        let (r, g, b) = (u8::from_str_radix(&c[0..2], 16).unwrap_or(0), u8::from_str_radix(&c[2..4], 16).unwrap_or(0), u8::from_str_radix(&c[4..6], 16).unwrap_or(0));
        let _ = write!(out, "\\red{r}\\green{g}\\blue{b};");
    }
    out.push_str("}\n");
    out.push_str(&styles);
    out.push_str(&lists);
    out.push_str("{\\info");
    for (word, value) in [("title", Some(src.title.as_str())), ("subject", src.props.subject.as_deref()), ("author", src.props.author.as_deref()), ("keywords", src.props.keywords.as_deref()), ("doccomm", src.props.description.as_deref())] {
        if let Some(v) = value.filter(|v| !v.trim().is_empty()) {
            let _ = write!(out, "{{\\{word} {}}}", rtf_text(v));
        }
    }
    let (created, _) = core_dates(doc);
    if let Some(t) = created.as_deref().and_then(timestamp) {
        let n = |a: usize, b: usize| t[a..b].parse::<u32>().unwrap_or(0);
        let _ = write!(out, "{{\\creatim\\yr{}\\mo{}\\dy{}\\hr{}\\min{}}}", n(0, 4), n(5, 7), n(8, 10), n(11, 13), n(14, 16));
    }
    out.push_str("}\n");
    let sec = src.setup.sections.first();
    let (pw, ph, m, hd, fd) = match sec {
        Some(s) => (s.width, s.height, s.margin, s.header, s.footer),
        None => (src.setup.width, src.setup.height, src.setup.margin, src.setup.header, src.setup.footer),
    };
    let _ = writeln!(out, "\\paperw{pw}\\paperh{ph}{}\\margl{}\\margr{}\\margt{}\\margb{}\\gutter0\\headery{hd}\\footery{fd}", if pw > ph { "\\landscape" } else { "" }, m[3], m[1], m[0], m[2]);
    if !src.setup.endnotes.is_empty() {
        out.push_str("\\fet2\\ftnbj\\aenddoc\\aftnnar\n");
    }
    out.push_str("\\viewkind1\\viewscale100\n");
    if let Some(h) = header {
        let _ = writeln!(out, "{{\\header {h}}}");
    }
    if let Some(f) = footer {
        let _ = writeln!(out, "{{\\footer {f}}}");
    }
    out.push_str(&body);
    out.push_str("}\n");
    out
}

/// The Windows locale number for a language tag (RTF's `\deflang`).
fn lcid(lang: &str) -> u32 {
    match lang.to_lowercase().as_str() {
        "en-gb" => 2057,
        "en-au" => 3081,
        "en-ca" => 4105,
        "es" | "es-es" => 3082,
        "es-mx" => 2058,
        "fr" | "fr-fr" => 1036,
        "fr-ca" => 3084,
        "de" | "de-de" => 1031,
        "it" | "it-it" => 1040,
        "pt-br" => 1046,
        "pt" | "pt-pt" => 2070,
        "nl" | "nl-nl" => 1043,
        _ => 1033,
    }
}

struct Rtf<'a> {
    src: &'a Source<'a>,
    bases: Bases<'a>,
    /// Fonts by name with their family word (`froman`, `fswiss`, `fmodern`).
    fonts: Vec<(String, &'static str)>,
    colors: Vec<String>,
    /// Paragraph styles in use: style id to RTF style number.
    styles: BTreeMap<String, usize>,
    /// Lists in use: num id to the `\ls` number.
    lists: BTreeMap<i64, usize>,
    labels: HashMap<(i64, i64), ListLabel>,
    /// Set while writing something that sits in a cell, and the mark a note starts with.
    prefix: Option<String>,
}

impl<'a> Rtf<'a> {
    fn new(src: &'a Source<'a>) -> Self {
        let mut r = Rtf { src, bases: Bases::new(src.doc), fonts: Vec::new(), colors: Vec::new(), styles: BTreeMap::new(), lists: BTreeMap::new(), labels: HashMap::new(), prefix: None };
        // Font 0 is the document's default font.
        let default_font = r.bases.defaults().1.font.unwrap_or_else(|| "Times New Roman".into());
        r.font(&default_font);
        r
    }

    fn font(&mut self, name: &str) -> usize {
        if let Some(i) = self.fonts.iter().position(|(n, _)| n == name) {
            return i;
        }
        let family = match generic_font(name) {
            "serif" => "froman",
            "monospace" => "fmodern",
            _ => "fswiss",
        };
        self.fonts.push((name.to_string(), family));
        self.fonts.len() - 1
    }

    /// The colour table number of a colour (the table starts at 1; 0 is the automatic colour).
    fn color(&mut self, hex: &str) -> Option<usize> {
        let h = hex_color(hex)?;
        if let Some(i) = self.colors.iter().position(|c| *c == h) {
            return Some(i + 1);
        }
        self.colors.push(h);
        Some(self.colors.len())
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
    // Formatting words
    // -----------------------------------------------------------------------

    fn run_words(&mut self, d: &RunFmt) -> String {
        let mut s = String::new();
        if let Some(f) = &d.font {
            let i = self.font(f);
            let _ = write!(s, "\\f{i}");
        }
        if let Some(z) = d.size {
            let _ = write!(s, "\\fs{}", (z * 2.0).round() as i64);
        }
        if let Some(c) = d.color.as_deref().and_then(|c| self.color(c)) {
            let _ = write!(s, "\\cf{c}");
        }
        let shade = d.highlight.as_deref().and_then(highlight_hex).map(String::from);
        if let Some(c) = shade.as_deref().and_then(|c| self.color(c)) {
            let _ = write!(s, "\\highlight{c}");
        } else if let Some(c) = d.shading.as_deref().and_then(|c| self.color(c)) {
            let _ = write!(s, "\\chcbpat{c}");
        }
        for (v, on, off) in [(d.bold, "\\b", "\\b0"), (d.italic, "\\i", "\\i0"), (d.caps, "\\caps", "\\caps0"), (d.small_caps, "\\scaps", "\\scaps0"), (d.vanish, "\\v", "\\v0")] {
            match v {
                Some(true) => s.push_str(on),
                Some(false) => s.push_str(off),
                None => {}
            }
        }
        match d.underline.as_deref() {
            Some("none") => s.push_str("\\ulnone"),
            Some("double") => s.push_str("\\uldb"),
            Some("dotted" | "dottedHeavy") => s.push_str("\\uld"),
            Some("dash" | "dashLong" | "dashedHeavy" | "dashLongHeavy") => s.push_str("\\uldash"),
            Some("wave" | "wavyHeavy" | "wavyDouble") => s.push_str("\\ulwave"),
            Some(_) => s.push_str("\\ul"),
            None => {}
        }
        match (d.dstrike, d.strike) {
            (Some(true), _) => s.push_str("\\striked1"),
            (_, Some(true)) => s.push_str("\\strike"),
            (_, Some(false)) | (Some(false), _) => s.push_str("\\strike0\\striked0"),
            _ => {}
        }
        match d.vert_align.as_deref() {
            Some("superscript") => s.push_str("\\super"),
            Some("subscript") => s.push_str("\\sub"),
            Some("baseline") => s.push_str("\\nosupersub"),
            _ => {}
        }
        if let Some(x) = d.spacing.filter(|x| *x != 0) {
            let _ = write!(s, "\\expndtw{x}");
        }
        s
    }

    fn para_words(&mut self, d: &ParaFmt) -> String {
        let mut s = String::new();
        match d.align.as_deref() {
            Some("center") => s.push_str("\\qc"),
            Some("right" | "end") => s.push_str("\\qr"),
            Some("both") => s.push_str("\\qj"),
            Some("distribute") => s.push_str("\\qd"),
            Some("left" | "start") => s.push_str("\\ql"),
            _ => {}
        }
        if d.before_auto == Some(true) {
            s.push_str("\\sbauto1");
        } else if let Some(x) = d.space_before {
            let _ = write!(s, "\\sb{x}");
        }
        if d.after_auto == Some(true) {
            s.push_str("\\saauto1");
        } else if let Some(x) = d.space_after {
            let _ = write!(s, "\\sa{x}");
        }
        if let Some(l) = d.line.filter(|l| *l > 0) {
            match d.line_rule.as_deref() {
                Some("exact") => {
                    let _ = write!(s, "\\sl-{l}\\slmult0");
                }
                Some("atLeast") => {
                    let _ = write!(s, "\\sl{l}\\slmult0");
                }
                _ => {
                    let _ = write!(s, "\\sl{l}\\slmult1");
                }
            }
        }
        if let Some(x) = d.ind_left {
            let _ = write!(s, "\\li{x}");
        }
        if let Some(x) = d.ind_right {
            let _ = write!(s, "\\ri{x}");
        }
        if let Some(x) = d.ind_hanging.filter(|x| *x > 0) {
            let _ = write!(s, "\\fi-{x}");
        } else if let Some(x) = d.ind_first_line {
            let _ = write!(s, "\\fi{x}");
        }
        match d.keep_next {
            Some(true) => s.push_str("\\keepn"),
            Some(false) => s.push_str("\\keepn0"),
            None => {}
        }
        match d.keep_lines {
            Some(true) => s.push_str("\\keep"),
            Some(false) => s.push_str("\\keep0"),
            None => {}
        }
        if d.page_break_before == Some(true) {
            s.push_str("\\pagebb");
        }
        if d.contextual_spacing == Some(true) {
            s.push_str("\\contextualspace");
        }
        if d.bidi == Some(true) {
            s.push_str("\\rtlpar");
        }
        if let Some(b) = &d.borders {
            for (word, border) in [("\\brdrt", &b.top), ("\\brdrl", &b.left), ("\\brdrb", &b.bottom), ("\\brdrr", &b.right)] {
                if let Some(border) = border {
                    s.push_str(word);
                    s.push_str(&self.border_words(border));
                    let _ = write!(s, "\\brsp{}", border.space * 20);
                }
            }
        }
        if let Some(c) = d.shading.as_deref().and_then(|c| self.color(c)) {
            let _ = write!(s, "\\cbpat{c}");
        }
        if let Some(tabs) = &d.tabs {
            for t in tabs {
                match t.val.as_str() {
                    "center" => s.push_str("\\tqc"),
                    "right" | "end" => s.push_str("\\tqr"),
                    "decimal" => s.push_str("\\tqdec"),
                    "bar" => {
                        let _ = write!(s, "\\tb{}", t.pos);
                        continue;
                    }
                    "clear" => continue,
                    _ => {}
                }
                match t.leader.as_deref() {
                    Some("dot" | "middleDot") => s.push_str("\\tldot"),
                    Some("hyphen") => s.push_str("\\tlhyph"),
                    Some("underscore") => s.push_str("\\tlul"),
                    _ => {}
                }
                let _ = write!(s, "\\tx{}", t.pos);
            }
        }
        s
    }

    /// `\brdrs\brdrw10\brdrcf2`: a border's line type, width in twips and colour.
    fn border_words(&mut self, b: &crate::props::Border) -> String {
        let (style, w, color) = border_parts(b);
        let kind = match style {
            "double" => "\\brdrdb",
            "dotted" => "\\brdrdot",
            "dashed" => "\\brdrdash",
            _ => "\\brdrs",
        };
        let c = self.color(&color).unwrap_or(0);
        format!("{kind}\\brdrw{}\\brdrcf{c}", ((w * 20.0).round() as i64).clamp(5, 75))
    }

    // -----------------------------------------------------------------------
    // Stylesheet and lists
    // -----------------------------------------------------------------------

    /// The RTF style number for a paragraph style (0 is Normal).
    fn style_no(&mut self, id: Option<&str>) -> usize {
        let doc = self.src.doc;
        let Some(id) = id.filter(|i| Some(*i) != doc.styles.default_para.as_deref()) else { return 0 };
        if let Some(n) = self.styles.get(id) {
            return *n;
        }
        let n = self.styles.len() + 1;
        self.styles.insert(id.to_string(), n);
        n
    }

    fn stylesheet(&mut self) -> String {
        let doc = self.src.doc;
        let (np, nr) = self.bases.get(None).clone();
        // Normal carries the document's whole default formatting.
        let normal_p = self.para_words(&para_delta(&np, &ParaFmt::default(), false));
        let normal_r = self.run_words(&run_delta(&nr, &RunFmt::default()));
        let mut out = format!("{{\\stylesheet{{\\ql \\li0\\ri0\\sa0{normal_p}{normal_r} \\snext0 \\sqformat Normal;}}");
        let mut done = 0;
        while done < self.styles.len() {
            // Styles named while writing a style (its next style) are added to the end.
            let pending: Vec<(String, usize)> = self.styles.iter().filter(|(_, n)| **n > done).map(|(k, v)| (k.clone(), *v)).collect();
            let mut pending = pending;
            pending.sort_by_key(|(_, n)| *n);
            for (id, n) in pending {
                done = done.max(n);
                let info = doc.styles.get(&id);
                let name = info.map(|i| i.name.clone()).unwrap_or_else(|| id.clone());
                let (pf, rf) = self.bases.get(Some(&id)).clone();
                let mut words = self.para_words(&para_delta(&pf, &np, false));
                words.push_str(&self.run_words(&run_delta(&rf, &nr)));
                if let Some(l) = pf.outline_lvl.filter(|l| (0..9).contains(l)) {
                    let _ = write!(words, "\\outlinelevel{l}");
                }
                let next = info.and_then(|i| i.next.clone()).filter(|x| *x != id).map(|x| self.style_no(Some(&x))).unwrap_or(n);
                let _ = write!(out, "{{\\s{n}\\sbasedon0 \\snext{next} {words} {};}}", rtf_text(&name).replace(';', ""));
            }
        }
        out.push_str("}\n");
        out
    }

    /// `\ls` number of a list, registering it.
    fn list_no(&mut self, num_id: i64) -> usize {
        let n = self.lists.len() + 1;
        *self.lists.entry(num_id).or_insert(n)
    }

    fn list_table(&mut self) -> String {
        if self.lists.is_empty() {
            return String::new();
        }
        let doc = self.src.doc;
        let mut table = String::from("{\\*\\listtable");
        let mut overrides = String::from("{\\*\\listoverridetable");
        let lists: Vec<(i64, usize)> = self.lists.iter().map(|(k, v)| (*k, *v)).collect();
        for (num_id, ls) in lists {
            let _ = write!(table, "{{\\list\\listtemplateid{}", 1000 + ls);
            for lvl in 0..9i64 {
                let lv = doc.numbering.level(num_id, lvl);
                let seen = self.labels.get(&(num_id, lvl));
                let ppr = lv.map(|l| &l.ppr);
                let left = ppr.and_then(|p| p.ind_left).unwrap_or(720 * (lvl + 1));
                let hanging = ppr.and_then(|p| p.ind_hanging).or_else(|| ppr.and_then(|p| p.ind_first_line).map(|f| -f)).unwrap_or(360);
                let follow = match lv.map(|l| l.suff.as_str()) {
                    Some("space") => 1,
                    Some("nothing") => 2,
                    _ => 0,
                };
                let bullet = lv.map(|l| l.fmt == "bullet").unwrap_or(true) || seen.is_some_and(|s| s.bullet);
                let (nfc, text, nums) = if bullet {
                    let glyph = bullet_char(seen.map(|s| s.text.as_str()).or(lv.map(|l| l.text.as_str())).unwrap_or(""));
                    (23, format!("\\'01{}", unicode_word(glyph)), String::new())
                } else {
                    let lv = lv.unwrap();
                    let nfc = match lv.fmt.as_str() {
                        "upperRoman" => 1,
                        "lowerRoman" => 2,
                        "upperLetter" => 3,
                        "lowerLetter" => 4,
                        _ => 0,
                    };
                    let (text, nums) = level_text(&lv.text);
                    (nfc, text, nums)
                };
                let start = lv.map(|l| l.start).unwrap_or(1);
                let _ = write!(table, "\n{{\\listlevel\\levelnfc{nfc}\\levelnfcn{nfc}\\leveljc0\\leveljcn0\\levelfollow{follow}\\levelstartat{start}\\levelspace0\\levelindent0{{\\leveltext {text};}}{{\\levelnumbers{nums};}}\\fi{}\\li{left}\\lin{left}}}", -hanging);
            }
            let _ = writeln!(table, "{{\\listname ;}}\\listid{}}}", 2000 + ls);
            let _ = write!(overrides, "{{\\listoverride\\listid{}\\listoverridecount0\\ls{ls}}}", 2000 + ls);
        }
        table.push_str("}\n");
        overrides.push_str("}\n");
        table + &overrides
    }

    // -----------------------------------------------------------------------
    // Blocks
    // -----------------------------------------------------------------------

    fn blocks(&mut self, blocks: &[RBlock], out: &mut String, in_cell: bool) {
        let mut after_table = false;
        for b in blocks {
            match b {
                RBlock::Para(p) => {
                    self.para(p, out, in_cell);
                    after_table = false;
                }
                RBlock::Table(t) => {
                    if in_cell {
                        // A table inside a cell is written as its cells' paragraphs.
                        for c in t.rows.iter().flat_map(|r| &r.cells) {
                            self.blocks(&c.blocks, out, true);
                        }
                    } else {
                        if after_table {
                            out.push_str("\\pard\\par\n");
                        }
                        self.table(t, out);
                        after_table = true;
                    }
                }
                RBlock::Container { blocks, .. } => {
                    self.blocks(blocks, out, in_cell);
                    after_table = false;
                }
                RBlock::Opaque { .. } => {}
            }
        }
    }

    fn para(&mut self, p: &RPara, out: &mut String, in_cell: bool) {
        let (base_p, base_r) = self.bases.get(p.style.as_deref()).clone();
        let level = heading_level(p);
        let item = is_list_item(p);
        let mut delta = para_delta(&p.fmt, &base_p, item);
        if item {
            // A list item states its indents; the list level gives them.
            delta.ind_left = p.fmt.ind_left;
            delta.ind_hanging = p.fmt.ind_hanging;
            delta.ind_first_line = p.fmt.ind_first_line;
        }
        let style = self.style_no(p.style.as_deref());
        let mut head = format!("\\pard{}\\plain\\s{style}", if in_cell { "\\intbl\\itap1" } else { "" });
        if let (true, Some(l)) = (item, &p.list) {
            let ls = self.list_no(l.num_id);
            let _ = write!(head, "\\ls{ls}\\ilvl{}", l.level.clamp(0, 8));
        }
        head.push_str(&self.para_words(&delta));
        if let Some(n) = level {
            let _ = write!(head, "\\outlinelevel{}", n - 1);
        }
        head.push(' ');
        let mut body = String::new();
        for b in &p.bookmarks {
            let name = bookmark_name(b);
            let _ = write!(body, "{{\\*\\bkmkstart {name}}}{{\\*\\bkmkend {name}}}");
        }
        if let Some(l) = p.list.as_ref().filter(|_| item) {
            // For readers that do not know lists: the label as text.
            let _ = write!(body, "{{\\listtext {}\\tab}}", rtf_text(l.text.trim()));
        } else if let (Some(_), Some(l)) = (level, &p.list) {
            if !l.text.trim().is_empty() {
                let _ = write!(body, "{} ", rtf_text(l.text.trim()));
            }
        }
        if let Some(prefix) = self.prefix.take() {
            body.push_str(&prefix);
        }
        body.push_str(&self.inline(p, &base_r));
        out.push_str(&head);
        out.push_str(&body);
        out.push_str("\\par\n");
    }

    fn table(&mut self, t: &RTable, out: &mut String) {
        let ncols = t.grid.len().max(t.rows.iter().map(|r| r.cells.iter().map(|c| c.span.max(1) as usize).sum()).max().unwrap_or(0));
        if ncols == 0 || t.rows.is_empty() {
            return;
        }
        let text_width = self.src.setup.sections.first().map(|s| s.width - s.margin[1] - s.margin[3]).filter(|w| *w > 0).unwrap_or(9360);
        let total: i64 = t.grid.iter().sum();
        let width = if t.width_pct { t.width.map(|w| text_width * w / 5000) } else { t.width }.filter(|w| *w > 0).or((total > 0).then_some(total)).unwrap_or(text_width);
        let widths: Vec<i64> = if t.grid.len() == ncols { t.grid.clone() } else { vec![width / ncols as i64; ncols] };
        let table_width: i64 = widths.iter().sum();
        let left = match t.align.as_deref() {
            Some("center") => ((text_width - table_width) / 2).max(0),
            Some("right" | "end") => (text_width - table_width).max(0),
            _ => t.indent.unwrap_or(0),
        };
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
        let [mt, mr, mb, ml] = t.cell_mar;
        for (ri, row) in t.rows.iter().enumerate() {
            let mut defs = String::new();
            let mut cells = String::new();
            let mut edge = left;
            // Right edge of each grid column.
            let edges: Vec<i64> = widths
                .iter()
                .map(|w| {
                    edge += w;
                    edge
                })
                .collect();
            for (c, cell) in &placed[ri] {
                let span = cell.span.max(1) as usize;
                let continued = cell.vmerge.as_deref() == Some("continue");
                let mut rows_covered = 1;
                if cell.vmerge.as_deref() == Some("restart") {
                    while continues(ri + rows_covered, *c) {
                        rows_covered += 1;
                    }
                }
                let b = &t.borders;
                let (last_row, last_col) = (ri + rows_covered - 1, *c + span - 1);
                let sides = [
                    ("\\clbrdrt", cell.borders.top.as_ref().or(if ri == 0 { b.top.as_ref() } else { b.inside_h.as_ref() }).filter(|b| !b.is_nil())),
                    ("\\clbrdrl", cell.borders.left.as_ref().or(if *c == 0 { b.left.as_ref() } else { b.inside_v.as_ref() }).filter(|b| !b.is_nil())),
                    ("\\clbrdrb", cell.borders.bottom.as_ref().or(if last_row + 1 >= t.rows.len() { b.bottom.as_ref() } else { b.inside_h.as_ref() }).filter(|b| !b.is_nil())),
                    ("\\clbrdrr", cell.borders.right.as_ref().or(if last_col + 1 >= ncols { b.right.as_ref() } else { b.inside_v.as_ref() }).filter(|b| !b.is_nil())),
                ];
                for k in 0..span {
                    let col = *c + k;
                    if col >= edges.len() {
                        break;
                    }
                    if continued {
                        defs.push_str("\\clvmrg");
                    } else if cell.vmerge.as_deref() == Some("restart") {
                        defs.push_str("\\clvmgf");
                    }
                    if span > 1 {
                        defs.push_str(if k == 0 { "\\clmgf" } else { "\\clmrg" });
                    }
                    defs.push_str(match cell.valign.as_deref() {
                        Some("center") => "\\clvertalc",
                        Some("bottom") => "\\clvertalb",
                        _ => "\\clvertalt",
                    });
                    for (word, border) in &sides {
                        if let Some(border) = border {
                            defs.push_str(word);
                            defs.push_str(&self.border_words(border));
                        }
                    }
                    if let Some(c) = cell.shading.as_deref().and_then(|c| self.color(c)) {
                        let _ = write!(defs, "\\clcbpat{c}");
                    }
                    let _ = writeln!(defs, "\\cellx{}", edges[col]);
                    // The first cell of a merge holds the content; the others are empty.
                    if k == 0 && !continued {
                        let mut inner = String::new();
                        self.blocks(&cell.blocks, &mut inner, true);
                        if inner.is_empty() {
                            inner.push_str("\\pard\\intbl\\itap1\\plain\\s0 ");
                        } else {
                            // The last paragraph ends with the cell mark, not a paragraph mark.
                            inner = inner.trim_end_matches("\\par\n").to_string();
                        }
                        cells.push_str(&inner);
                    } else {
                        cells.push_str("\\pard\\intbl\\itap1\\plain\\s0 ");
                    }
                    cells.push_str("\\cell\n");
                }
            }
            let mut row_head = format!("\\trowd\\trgaph{}\\trleft{left}\\trpaddl{ml}\\trpaddr{mr}\\trpaddt{mt}\\trpaddb{mb}\\trpaddfl3\\trpaddfr3\\trpaddft3\\trpaddfb3", (ml + mr) / 2);
            if let Some(h) = row.height.filter(|h| *h > 0) {
                let _ = write!(row_head, "\\trrh{h}");
            }
            if row.header {
                row_head.push_str("\\trhdr");
            }
            row_head.push('\n');
            out.push_str(&row_head);
            out.push_str(&defs);
            out.push_str(&cells);
            out.push_str("\\row\n");
        }
    }

    // -----------------------------------------------------------------------
    // Inline content
    // -----------------------------------------------------------------------

    fn inline(&mut self, p: &RPara, base: &RunFmt) -> String {
        let mut out = String::new();
        let mut i = 0;
        while i < p.segs.len() {
            let link = p.segs[i].link.as_deref().and_then(safe_link).map(String::from);
            let mut j = i;
            while j < p.segs.len() && p.segs[j].link.as_deref().and_then(safe_link).map(String::from) == link {
                j += 1;
            }
            let mut run = String::new();
            for s in &p.segs[i..j] {
                run.push_str(&self.seg(s, base));
            }
            match link {
                Some(l) => {
                    // The field's text is `HYPERLINK "url"` or `HYPERLINK \l "bookmark"` (the backslash is escaped).
                    let target = match l.strip_prefix('#') {
                        Some(name) => format!("\\\\l \"{}\"", bookmark_name(name)),
                        None => format!("\"{}\"", rtf_text(&l.replace('"', "%22"))),
                    };
                    let _ = write!(out, "{{\\field{{\\*\\fldinst{{HYPERLINK {target}}}}}{{\\fldrslt {{{run}}}}}}}");
                }
                None => out.push_str(&run),
            }
            i = j;
        }
        out
    }

    /// Text or a control with its formatting in a group.
    fn group(&mut self, f: &RunFmt, base: &RunFmt, inner: &str) -> String {
        let words = self.run_words(&run_delta(f, base));
        if words.is_empty() {
            inner.to_string()
        } else {
            format!("{{{words} {inner}}}")
        }
    }

    fn seg(&mut self, s: &RSeg, base: &RunFmt) -> String {
        match s.k {
            "t" => {
                let text = s.x.as_deref().unwrap_or("");
                if text.is_empty() {
                    return String::new();
                }
                let inner = match s.fld.as_deref() {
                    Some(f @ ("PAGE" | "NUMPAGES")) => format!("{{\\field{{\\*\\fldinst {f}}}{{\\fldrslt {}}}}}", rtf_text(text)),
                    _ => rtf_text(text),
                };
                self.group(&s.f, base, &inner)
            }
            "tab" => self.group(&s.f, base, "\\tab "),
            "br" => "\\line ".into(),
            "pbr" => "\\page ".into(),
            "cbr" => "\\column ".into(),
            "img" => self.image(s),
            "math" => match &s.math {
                Some(m) => self.group(&s.f, base, &rtf_text(&if m.display { format!("$${}$$", m.latex) } else { format!("${}$", m.latex) })),
                None => String::new(),
            },
            "note" => self.note(s),
            "chart" => {
                let title = s.chart.as_ref().and_then(|c| c.title.clone()).or_else(|| s.chartex.as_ref().and_then(|c| c.title.clone())).or_else(|| s.img.as_ref().and_then(|i| i.alt.clone())).unwrap_or_default();
                let label = if title.is_empty() { "[Chart]".to_string() } else { format!("[Chart: {title}]") };
                self.group(&s.f, base, &rtf_text(&label))
            }
            "shape" => {
                let Some(sh) = &s.shape else { return String::new() };
                let text = sh.text.as_ref().map(|b| block_text(b)).filter(|t| !t.trim().is_empty());
                let label = text.or_else(|| sh.alt.clone().filter(|a| !a.is_empty()));
                match label {
                    Some(l) => self.group(&s.f, base, &rtf_text(&format!("[{l}]"))),
                    None => String::new(),
                }
            }
            "obj" => match s.label.as_deref().filter(|l| !l.is_empty()) {
                Some(l) => self.group(&s.f, base, &rtf_text(&format!("[{l}]"))),
                None => String::new(),
            },
            _ => String::new(),
        }
    }

    fn image(&mut self, s: &RSeg) -> String {
        let Some(img) = &s.img else { return String::new() };
        let alt = img.alt.clone().unwrap_or_default();
        let placeholder = || if alt.is_empty() { String::new() } else { rtf_text(&format!("[{alt}]")) };
        let Some(data) = image_data(self.src.doc, &img.src) else { return placeholder() };
        let blip = match data.mime {
            "image/png" => "\\pngblip",
            "image/jpeg" => "\\jpegblip",
            "image/x-emf" => "\\emfblip",
            _ => return placeholder(),
        };
        let (w, h) = (if img.w > 0.0 { img.w } else { 72.0 }, if img.h > 0.0 { img.h } else { 72.0 });
        let mut hex = String::with_capacity(data.bytes.len() * 2 + data.bytes.len() / 32 + 8);
        for (i, b) in data.bytes.iter().enumerate() {
            if i % 64 == 0 {
                hex.push('\n');
            }
            let _ = write!(hex, "{b:02x}");
        }
        format!(
            "{{\\pict{blip}\\picw{}\\pich{}\\picwgoal{}\\pichgoal{}\\picscalex100\\picscaley100{hex}\n}}",
            (w * 35.2778).round() as i64,
            (h * 35.2778).round() as i64,
            (w * 20.0).round() as i64,
            (h * 20.0).round() as i64
        )
    }

    fn note(&mut self, s: &RSeg) -> String {
        let id = s.x.as_deref().unwrap_or("");
        if id.is_empty() {
            return String::new();
        }
        let endnote = s.note == Some("endnote");
        let Some(note) = self.src.note(id, endnote) else { return String::new() };
        let mut body = String::new();
        self.prefix = Some("{\\super\\chftn }".into());
        self.blocks(&note.blocks, &mut body, false);
        self.prefix = None;
        format!("{{\\super\\chftn{{\\footnote{} {}}}}}", if endnote { "\\ftnalt" } else { "" }, body.trim_end_matches('\n'))
    }

    // -----------------------------------------------------------------------
    // Header and footer
    // -----------------------------------------------------------------------

    /// The first section's default header and footer, written for `\header` and `\footer`.
    fn page_parts(&mut self) -> (Option<String>, Option<String>) {
        let setup = &self.src.setup;
        let Some(sec) = setup.sections.first() else { return (None, None) };
        let story = |map: &HashMap<String, String>| setup.parts.get(map.get("default")?).map(|s| s.blocks.clone()).filter(|b| !b.is_empty());
        let (h, f) = (story(&sec.headers), story(&sec.footers));
        let mut write = |blocks: Option<Vec<RBlock>>| {
            blocks.map(|b| {
                let mut s = String::new();
                self.blocks(&b, &mut s, false);
                s
            })
        };
        let header = write(h);
        let footer = write(f);
        (header, footer)
    }
}

/// The plain text of some blocks, paragraphs joined with a slash.
fn block_text(blocks: &[RBlock]) -> String {
    let mut parts = Vec::new();
    for b in blocks {
        match b {
            RBlock::Para(p) => parts.push(plain_text(p).trim().to_string()),
            RBlock::Table(t) => {
                for c in t.rows.iter().flat_map(|r| &r.cells) {
                    parts.push(block_text(&c.blocks));
                }
            }
            RBlock::Container { blocks, .. } => parts.push(block_text(blocks)),
            RBlock::Opaque { .. } => {}
        }
    }
    parts.retain(|p| !p.is_empty());
    parts.join(" / ")
}

/// A bookmark name RTF readers accept: letters, digits and underscores, at most 40 characters.
fn bookmark_name(name: &str) -> String {
    let n: String = name.chars().map(|c| if c.is_ascii_alphanumeric() || c == '_' { c } else { '_' }).take(40).collect();
    if n.is_empty() {
        "_bm".into()
    } else {
        n
    }
}

fn bullet_char(label: &str) -> char {
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

/// A character as `\uc0\uN ` (no fallback character), inside a group's text.
fn unicode_word(c: char) -> String {
    let mut s = String::from("\\uc0");
    let mut buf = [0u16; 2];
    for u in c.encode_utf16(&mut buf) {
        let _ = write!(s, "\\u{} ", *u as i16);
    }
    s
}

/// A numbering pattern (`%1.`) as `\leveltext` content (a length byte, `\'00` for a level's number and
/// the other characters) and the `\levelnumbers` content (where the numbers are).
fn level_text(pattern: &str) -> (String, String) {
    let chars: Vec<char> = pattern.chars().collect();
    let mut text = String::new();
    let mut nums = String::new();
    let mut count = 0;
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '%' && chars.get(i + 1).is_some_and(|c| c.is_ascii_digit()) {
            let d = chars[i + 1].to_digit(10).unwrap_or(1).max(1) - 1;
            let _ = write!(text, "\\'{d:02x}");
            count += 1;
            let _ = write!(nums, "\\'{count:02x}");
            i += 2;
        } else {
            let c = chars[i];
            count += 1;
            if c.is_ascii() && !matches!(c, '\\' | '{' | '}' | ';') {
                text.push(c);
            } else if (c as u32) < 0x100 {
                let _ = write!(text, "\\'{:02x}", c as u32);
            } else {
                let _ = write!(text, "{}", unicode_word(c));
            }
            i += 1;
        }
    }
    (format!("\\'{count:02x}{text}"), nums)
}

/// Text for RTF: ASCII as it is (with `\ { }` escaped), Latin-1 as `\'hh`, everything else as `\uN?`.
fn rtf_text(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '{' => out.push_str("\\{"),
            '}' => out.push_str("\\}"),
            '\t' => out.push_str("\\tab "),
            '\n' => out.push_str("\\line "),
            '\r' => {}
            c if (c as u32) < 0x20 || c == '\u{7F}' => {}
            c if c.is_ascii() => out.push(c),
            c if (0xA0..=0xFF).contains(&(c as u32)) => {
                let _ = write!(out, "\\'{:02x}", c as u32);
            }
            c => {
                let mut buf = [0u16; 2];
                for u in c.encode_utf16(&mut buf) {
                    let _ = write!(out, "\\u{}?", *u as i16);
                }
            }
        }
    }
    out
}
