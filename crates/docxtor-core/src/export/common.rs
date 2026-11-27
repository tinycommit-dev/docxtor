//! What the HTML, EPUB, OpenDocument and RTF writers share: the rendered document, formatting
//! differences against a style, lists as a tree, images, escaping and zip output.
//!
//! The writers read the rendering view model (`render.rs`), so they see the document the way the
//! editor shows it: list numbers, note numbers and field results are already resolved. Nothing
//! here changes the document.

use crate::docprops::DocProps;
use crate::document::{Document, PageSetup};
use crate::numbering::ListLabel;
use crate::props::{Border, ParaFmt, RunFmt};
use crate::render::{RBlock, RPara};
use std::collections::HashMap;
use std::io::Write as _;

/// The document as the converters see it.
pub struct Source<'a> {
    pub doc: &'a Document,
    pub blocks: Vec<RBlock>,
    pub setup: PageSetup,
    pub props: DocProps,
    /// The document language (`en-US`).
    pub lang: String,
    /// The title: the document property, else the first heading, else "Untitled".
    pub title: String,
}

impl<'a> Source<'a> {
    pub fn new(doc: &'a Document) -> Source<'a> {
        let blocks = doc.render_snapshot().blocks;
        let mut src = Source::bare(doc);
        src.title = src.title_for(first_heading(&blocks));
        src.blocks = blocks;
        src
    }

    /// The source without the drawn blocks and without a title: for a writer that takes the body a block at a time with
    /// `stream_blocks` (which finds the first heading) and then asks for `title_for` it.
    pub fn bare(doc: &'a Document) -> Source<'a> {
        let setup = doc.page_setup();
        let props = doc.doc_props();
        let lang = setup.lang.clone().or_else(|| doc.language()).filter(|l| !l.is_empty()).unwrap_or_else(|| "en-US".into());
        Source { doc, blocks: Vec::new(), setup, props, lang, title: String::new() }
    }

    /// The title: the document property, else the first heading, else "Untitled".
    pub fn title_for(&self, first_heading: Option<String>) -> String {
        self.props.title.clone().filter(|t| !t.trim().is_empty()).or(first_heading).unwrap_or_else(|| "Untitled".into())
    }

    /// The note with this id (footnote or endnote).
    pub fn note(&self, id: &str, endnote: bool) -> Option<&crate::document::RNote> {
        let list = if endnote { &self.setup.endnotes } else { &self.setup.notes };
        list.iter().find(|n| n.id == id)
    }
}

/// The body drawn a block at a time and handed to `write` in runs of whole blocks, so the drawing of the whole document is never held:
/// the same calls to a writer's `blocks` as one call with the trailing empty paragraphs and unsupported blocks (`trim_end`) cut
/// off. A run ends on a block that closes any list and that is not one of those, so list nesting and trimming come out as for the
/// whole body. The first heading (for the title) is found on the way.
pub fn stream_blocks(doc: &Document, first: &mut Option<String>, write: &mut dyn FnMut(&[RBlock])) {
    let mut pend: Vec<RBlock> = Vec::new();
    doc.render_snapshot_each(&mut |b| {
        if first.is_none() {
            *first = first_heading(std::slice::from_ref(&b));
        }
        let ends_run = !matches!(&b, RBlock::Para(p) if is_list_item(p)) && !trimmable(&b);
        pend.push(b);
        if ends_run && pend.len() >= 128 {
            write(&pend);
            pend.clear();
        }
    });
    write(trim_end(&pend));
}

fn trimmable(b: &RBlock) -> bool {
    match b {
        RBlock::Para(p) => p.segs.is_empty() && p.list.is_none() && !p.hr,
        RBlock::Opaque { .. } => true,
        _ => false,
    }
}

fn first_heading(blocks: &[RBlock]) -> Option<String> {
    for b in blocks {
        match b {
            RBlock::Para(p) if heading_level(p).is_some() || p.style.as_deref() == Some("Title") => {
                let t = plain_text(p);
                if !t.trim().is_empty() {
                    return Some(t.trim().to_string());
                }
            }
            RBlock::Container { blocks, .. } => {
                if let Some(t) = first_heading(blocks) {
                    return Some(t);
                }
            }
            _ => {}
        }
    }
    None
}

/// The text of a paragraph: text, tabs as spaces and line breaks as spaces.
pub fn plain_text(p: &RPara) -> String {
    let mut s = String::new();
    for g in &p.segs {
        match g.k {
            "t" => s.push_str(g.x.as_deref().unwrap_or("")),
            "tab" | "br" => s.push(' '),
            _ => {}
        }
    }
    s
}

/// Heading level 1 to 6 for a paragraph in a heading style (or with an outline level).
pub fn heading_level(p: &RPara) -> Option<u8> {
    if let Some(n) = p.fmt.outline_lvl {
        if (0..9).contains(&n) {
            return Some((n as u8 + 1).min(6));
        }
    }
    let style = p.style.as_deref()?;
    let n: u8 = style.strip_prefix("Heading").or_else(|| style.strip_prefix("heading"))?.trim().parse().ok()?;
    (1..=9).contains(&n).then_some(n.min(6))
}

/// A paragraph that is an item of a list (headings with numbers are headings, not items).
pub fn is_list_item(p: &RPara) -> bool {
    p.list.is_some() && heading_level(p).is_none()
}

// ---------------------------------------------------------------------------
// Formatting differences
// ---------------------------------------------------------------------------

/// The formatting a paragraph style gives (document defaults, then the style's chain).
pub struct Bases<'a> {
    doc: &'a Document,
    cache: HashMap<Option<String>, (ParaFmt, RunFmt)>,
    /// Theme colours in a style's formatting are resolved, as they are in the runs they are compared with.
    theme: crate::shapes::theme::ShapeTheme,
}

impl<'a> Bases<'a> {
    pub fn new(doc: &'a Document) -> Self {
        Bases { doc, cache: HashMap::new(), theme: crate::shapes::theme::ShapeTheme::load(&doc.pkg) }
    }

    /// The document's defaults alone, before any style.
    pub fn defaults(&self) -> (ParaFmt, RunFmt) {
        let mut rf = self.doc.styles.finish_run(self.doc.styles.doc_rpr.clone());
        crate::charfmt::resolve_run(&mut rf, &self.theme);
        rf.style = None;
        rf.font_theme = None;
        (self.doc.styles.doc_ppr.clone(), rf)
    }

    /// Paragraph and run formatting of a style (None is the default paragraph style).
    pub fn get(&mut self, style: Option<&str>) -> &(ParaFmt, RunFmt) {
        let doc = self.doc;
        let theme = &self.theme;
        self.cache.entry(style.map(String::from)).or_insert_with(|| {
            let (sp, sr) = doc.styles.para_style(style);
            let pf = doc.styles.doc_ppr.overlay(&sp);
            let mut rf = doc.styles.finish_run(doc.styles.doc_rpr.overlay(&sr));
            crate::charfmt::resolve_run(&mut rf, theme);
            rf.style = None;
            rf.font_theme = None;
            (pf, rf)
        })
    }
}

fn flag(a: Option<bool>) -> bool {
    a.unwrap_or(false)
}

/// What a paragraph's formatting changes against its style: only the differing fields are set.
/// List indents are left out; the list gives them.
pub fn para_delta(f: &ParaFmt, base: &ParaFmt, in_list: bool) -> ParaFmt {
    macro_rules! diff {
        ($($x:ident),*) => { ParaFmt { $($x: if f.$x != base.$x { f.$x.clone() } else { None },)* ..Default::default() } };
    }
    let mut d = diff!(align, space_before, space_after, before_auto, after_auto, line, line_rule, ind_left, ind_right, ind_first_line, ind_hanging, shading, borders, bidi, tabs);
    for (a, b, set) in [
        (f.keep_next, base.keep_next, &mut d.keep_next),
        (f.keep_lines, base.keep_lines, &mut d.keep_lines),
        (f.page_break_before, base.page_break_before, &mut d.page_break_before),
        (f.contextual_spacing, base.contextual_spacing, &mut d.contextual_spacing),
    ] {
        if flag(a) != flag(b) {
            *set = Some(flag(a));
        }
    }
    if in_list {
        d.ind_left = None;
        d.ind_first_line = None;
        d.ind_hanging = None;
        d.tabs = None;
    }
    d
}

/// What a run's formatting changes against its paragraph's: only the differing fields are set.
pub fn run_delta(f: &RunFmt, base: &RunFmt) -> RunFmt {
    macro_rules! diff {
        ($($x:ident),*) => { RunFmt { $($x: if f.$x != base.$x { f.$x.clone() } else { None },)* ..Default::default() } };
    }
    let mut d = diff!(font, size, color, highlight, shading, vert_align, spacing);
    for (a, b, set) in [
        (f.bold, base.bold, &mut d.bold),
        (f.italic, base.italic, &mut d.italic),
        (f.strike, base.strike, &mut d.strike),
        (f.dstrike, base.dstrike, &mut d.dstrike),
        (f.caps, base.caps, &mut d.caps),
        (f.small_caps, base.small_caps, &mut d.small_caps),
        (f.vanish, base.vanish, &mut d.vanish),
        (f.rtl, base.rtl, &mut d.rtl),
    ] {
        if flag(a) != flag(b) {
            *set = Some(flag(a));
        }
    }
    let u = |x: &Option<String>| x.as_deref().filter(|v| *v != "none").map(String::from);
    if u(&f.underline) != u(&base.underline) {
        d.underline = Some(u(&f.underline).unwrap_or_else(|| "none".into()));
    }
    // A highlight or shading of "none"/"auto" on a base without one is no change.
    if d.highlight.as_deref() == Some("none") && base.highlight.is_none() {
        d.highlight = None;
    }
    if d.color.as_deref() == Some("auto") && base.color.is_none() {
        d.color = None;
    }
    if d.vert_align.as_deref() == Some("baseline") && base.vert_align.is_none() {
        d.vert_align = None;
    }
    d
}

/// A valid six-digit hex colour (upper case, no `#`), or None for `auto` and anything else.
pub fn hex_color(c: &str) -> Option<String> {
    let c = c.trim().trim_start_matches('#');
    (c.len() == 6 && c.chars().all(|x| x.is_ascii_hexdigit())).then(|| c.to_ascii_uppercase())
}

/// The colour of a highlight name (`yellow`, `darkBlue`...).
pub fn highlight_hex(name: &str) -> Option<&'static str> {
    Some(match name {
        "yellow" => "FFFF00",
        "green" => "00FF00",
        "cyan" => "00FFFF",
        "magenta" => "FF00FF",
        "blue" => "0000FF",
        "red" => "FF0000",
        "darkBlue" => "000080",
        "darkCyan" => "008080",
        "darkGreen" => "008000",
        "darkMagenta" => "800080",
        "darkRed" => "800000",
        "darkYellow" => "808000",
        "darkGray" => "808080",
        "lightGray" => "C0C0C0",
        "black" => "000000",
        "white" => "FFFFFF",
        _ => return None,
    })
}

/// A border as a line style (`solid`, `double`, `dotted`, `dashed`), a width in points and a colour.
pub fn border_parts(b: &Border) -> (&'static str, f64, String) {
    let style = match b.val.as_str() {
        "double" | "triple" | "thinThickSmallGap" | "thickThinSmallGap" => "double",
        "dotted" => "dotted",
        "dashed" | "dashSmallGap" | "dotDash" | "dotDotDash" | "dashDotStroked" => "dashed",
        _ => "solid",
    };
    let width = (b.sz as f64 / 8.0).clamp(0.25, 12.0);
    let color = hex_color(&b.color).unwrap_or_else(|| "000000".into());
    (style, width, color)
}

/// Twentieths of a point as points, without trailing zeros (`12`, `10.5`).
pub fn pt(twips: i64) -> String {
    num(twips as f64 / 20.0)
}

/// A number with at most two decimals and no trailing zeros.
pub fn num(v: f64) -> String {
    let s = format!("{v:.2}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// A generic family to follow a font name: `serif`, `monospace` or `sans-serif`.
pub fn generic_font(name: &str) -> &'static str {
    let n = name.to_lowercase();
    if ["courier", "consolas", "mono", "menlo", "monaco", "lucida console", "code"].iter().any(|k| n.contains(k)) {
        "monospace"
    } else if ["times", "georgia", "cambria", "garamond", "palatino", "book antiqua", "bookman", "century", "serif", "minion", "constantia", "baskerville"].iter().any(|k| n.contains(k)) && !n.contains("sans") {
        "serif"
    } else {
        "sans-serif"
    }
}

// ---------------------------------------------------------------------------
// Lists as a tree
// ---------------------------------------------------------------------------

pub enum Node<'a> {
    Block(&'a RBlock),
    List(ListNode<'a>),
}

pub struct ListNode<'a> {
    pub num_id: i64,
    pub bullet: bool,
    pub level: i64,
    pub items: Vec<ListItem<'a>>,
}

pub struct ListItem<'a> {
    pub para: &'a RPara,
    pub sub: Vec<ListNode<'a>>,
}

fn close_top<'a>(stack: &mut Vec<ListNode<'a>>, out: &mut Vec<Node<'a>>) {
    if let Some(node) = stack.pop() {
        match stack.last_mut().and_then(|p| p.items.last_mut()) {
            Some(item) => item.sub.push(node),
            None => out.push(Node::List(node)),
        }
    }
}

/// Consecutive list paragraphs as nested lists; every other block as it is.
pub fn group(blocks: &[RBlock]) -> Vec<Node<'_>> {
    let mut out = Vec::new();
    let mut stack: Vec<ListNode> = Vec::new();
    for b in blocks {
        match b {
            RBlock::Para(p) if is_list_item(p) => {
                let l = p.list.as_ref().unwrap();
                while stack.last().is_some_and(|t| t.level > l.level) {
                    close_top(&mut stack, &mut out);
                }
                if stack.last().is_some_and(|t| t.level == l.level && (t.num_id != l.num_id || t.bullet != l.bullet)) {
                    close_top(&mut stack, &mut out);
                }
                if stack.last().is_none_or(|t| t.level < l.level) {
                    stack.push(ListNode { num_id: l.num_id, bullet: l.bullet, level: l.level, items: Vec::new() });
                }
                stack.last_mut().unwrap().items.push(ListItem { para: p, sub: Vec::new() });
            }
            other => {
                while !stack.is_empty() {
                    close_top(&mut stack, &mut out);
                }
                out.push(Node::Block(other));
            }
        }
    }
    while !stack.is_empty() {
        close_top(&mut stack, &mut out);
    }
    out
}

/// A list's number format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NumFmt {
    Decimal,
    LowerRoman,
    UpperRoman,
    LowerAlpha,
    UpperAlpha,
}

pub fn num_fmt(doc: &Document, l: &ListLabel) -> NumFmt {
    match doc.numbering.level(l.num_id, l.level).map(|x| x.fmt.as_str()) {
        Some("lowerRoman") => NumFmt::LowerRoman,
        Some("upperRoman") => NumFmt::UpperRoman,
        Some("lowerLetter") => NumFmt::LowerAlpha,
        Some("upperLetter") => NumFmt::UpperAlpha,
        _ => NumFmt::Decimal,
    }
}

/// The number a list label stands for (`3.` is 3, `c)` is 3, `iv.` is 4); the last number in the label.
pub fn label_value(text: &str, fmt: NumFmt) -> Option<u32> {
    match fmt {
        NumFmt::Decimal => {
            let mut last = None;
            let mut cur = String::new();
            for c in text.chars().chain(std::iter::once(' ')) {
                if c.is_ascii_digit() {
                    cur.push(c);
                } else if !cur.is_empty() {
                    last = cur.parse().ok();
                    cur.clear();
                }
            }
            last
        }
        NumFmt::LowerAlpha | NumFmt::UpperAlpha => {
            let letters: String = text.chars().filter(|c| c.is_ascii_alphabetic()).collect();
            if letters.is_empty() {
                return None;
            }
            // a..z, then aa..zz (Word repeats the letter).
            let first = letters.chars().next()?.to_ascii_lowercase();
            Some((first as u32 - 'a' as u32 + 1) + 26 * (letters.len() as u32 - 1))
        }
        NumFmt::LowerRoman | NumFmt::UpperRoman => {
            let roman: String = text.chars().filter(|c| c.is_ascii_alphabetic()).collect::<String>().to_ascii_lowercase();
            let val = |c: char| match c {
                'i' => 1,
                'v' => 5,
                'x' => 10,
                'l' => 50,
                'c' => 100,
                'd' => 500,
                'm' => 1000,
                _ => 0,
            };
            let vals: Vec<u32> = roman.chars().map(val).collect();
            if vals.is_empty() || vals.contains(&0) {
                return None;
            }
            let mut total = 0;
            for (i, v) in vals.iter().enumerate() {
                if vals.get(i + 1).is_some_and(|n| n > v) {
                    total -= *v as i64;
                } else {
                    total += *v as i64;
                }
            }
            u32::try_from(total).ok().filter(|t| *t > 0)
        }
    }
}

// ---------------------------------------------------------------------------
// Images
// ---------------------------------------------------------------------------

pub struct ImageData<'a> {
    /// The picture's bytes; an SVG's are the cleaned copy (`svg_clean`), every other picture's the file's own.
    pub bytes: std::borrow::Cow<'a, [u8]>,
    /// File extension without the dot (`png`, `jpg`).
    pub ext: &'static str,
    pub mime: &'static str,
}

/// A picture of the package: bytes with the kind found by looking at them (the extension otherwise).
pub fn image_data<'a>(doc: &'a Document, part: &str) -> Option<ImageData<'a>> {
    let bytes = doc.media(part)?;
    let (ext, mime) = sniff(bytes).or_else(|| by_extension(part))?;
    // An SVG can hold script and outside references: a picture that is not a clean SVG document is left out.
    let bytes = if mime == "image/svg+xml" { std::borrow::Cow::Owned(super::svg_clean::clean_svg(bytes)?) } else { std::borrow::Cow::Borrowed(bytes) };
    Some(ImageData { bytes, ext, mime })
}

fn sniff(b: &[u8]) -> Option<(&'static str, &'static str)> {
    if b.starts_with(&[0x89, b'P', b'N', b'G']) {
        Some(("png", "image/png"))
    } else if b.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some(("jpg", "image/jpeg"))
    } else if b.starts_with(b"GIF8") {
        Some(("gif", "image/gif"))
    } else if b.len() > 12 && &b[0..4] == b"RIFF" && &b[8..12] == b"WEBP" {
        Some(("webp", "image/webp"))
    } else if b.starts_with(b"BM") {
        Some(("bmp", "image/bmp"))
    } else if b.len() > 44 && b[0..4] == [1, 0, 0, 0] && &b[40..44] == b" EMF" {
        Some(("emf", "image/x-emf"))
    } else if b.starts_with(&[b'I', b'I', 42, 0]) || b.starts_with(&[b'M', b'M', 0, 42]) {
        Some(("tiff", "image/tiff"))
    } else if b.starts_with(&[0xD7, 0xCD, 0xC6, 0x9A]) {
        Some(("wmf", "image/x-wmf"))
    } else {
        let head = String::from_utf8_lossy(&b[..b.len().min(512)]).to_lowercase();
        head.contains("<svg").then_some(("svg", "image/svg+xml"))
    }
}

fn by_extension(part: &str) -> Option<(&'static str, &'static str)> {
    let ext = part.rsplit('.').next()?.to_ascii_lowercase();
    Some(match ext.as_str() {
        "png" => ("png", "image/png"),
        "jpg" | "jpeg" => ("jpg", "image/jpeg"),
        "gif" => ("gif", "image/gif"),
        "svg" => ("svg", "image/svg+xml"),
        "webp" => ("webp", "image/webp"),
        "bmp" => ("bmp", "image/bmp"),
        "tif" | "tiff" => ("tiff", "image/tiff"),
        "emf" => ("emf", "image/x-emf"),
        "wmf" => ("wmf", "image/x-wmf"),
        _ => return None,
    })
}

/// Pictures a web browser shows.
pub fn web_image(mime: &str) -> bool {
    matches!(mime, "image/png" | "image/jpeg" | "image/gif" | "image/svg+xml" | "image/webp" | "image/bmp")
}

/// Pictures an EPUB reader must show (its core media types, and WebP from EPUB 3.3).
pub fn epub_image(mime: &str) -> bool {
    matches!(mime, "image/png" | "image/jpeg" | "image/gif" | "image/svg+xml" | "image/webp")
}

pub fn base64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for c in data.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if c.len() > 1 { T[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if c.len() > 2 { T[n as usize & 63] as char } else { '=' });
    }
    out
}

// ---------------------------------------------------------------------------
// Text and links
// ---------------------------------------------------------------------------

fn xml_ok(c: char) -> bool {
    matches!(c, '\t' | '\n' | '\r') || (c >= ' ' && c != '\u{FFFE}' && c != '\u{FFFF}')
}

/// Text for XML or HTML content: `& < >` escaped, characters XML cannot hold dropped.
pub fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            c if xml_ok(c) => out.push(c),
            _ => {}
        }
    }
    out
}

/// Text for an attribute value in double quotes.
pub fn esc_attr(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\n' => out.push_str("&#10;"),
            '\r' => out.push_str("&#13;"),
            '\t' => out.push_str("&#9;"),
            c if xml_ok(c) => out.push(c),
            _ => {}
        }
    }
    out
}

/// A link a reader may follow: web, mail and phone links and places in the document. Anything else
/// (`javascript:`, `file:`...) is dropped.
pub fn safe_link(href: &str) -> Option<&str> {
    let h = href.trim();
    let lower = h.to_ascii_lowercase();
    if h.is_empty() {
        return None;
    }
    let ok = h.starts_with('#') || ["http://", "https://", "mailto:", "tel:", "ftp://"].iter().any(|s| lower.starts_with(s));
    ok.then_some(h)
}

/// A name that is safe as an id or a file name: letters, digits, `-` and `_`.
pub fn slug(s: &str) -> String {
    s.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect()
}

/// Dates in `docProps/core.xml`: (created, modified), as the file writes them.
pub fn core_dates(doc: &Document) -> (Option<String>, Option<String>) {
    let Some(bytes) = doc.pkg.get("docProps/core.xml") else { return (None, None) };
    let Ok(x) = crate::xml::parse(bytes) else { return (None, None) };
    let get = |n: &str| x.root.elements().find(|e| e.local() == n).map(|e| e.text().trim().to_string()).filter(|t| !t.is_empty());
    (get("created"), get("modified"))
}

/// A time as EPUB and OpenDocument want it, `CCYY-MM-DDThh:mm:ssZ`, from an ISO 8601 time.
pub fn timestamp(s: &str) -> Option<String> {
    let s = s.trim();
    let b = s.as_bytes();
    let digits = |r: std::ops::Range<usize>| b.get(r).is_some_and(|x| x.iter().all(u8::is_ascii_digit));
    let ok = b.len() >= 19 && digits(0..4) && b[4] == b'-' && digits(5..7) && b[7] == b'-' && digits(8..10) && b[10] == b'T' && digits(11..13) && b[13] == b':' && digits(14..16) && b[16] == b':' && digits(17..19);
    ok.then(|| format!("{}Z", &s[..19]))
}

/// A stable id in the shape of a UUID, made from the document's text (so exporting twice gives one id).
pub fn stable_uuid(seed: &str) -> String {
    let hash = |salt: u64| {
        let mut h: u64 = 0xcbf29ce484222325 ^ salt;
        for b in seed.bytes() {
            h ^= b as u64;
            h = h.wrapping_mul(0x100000001b3);
        }
        h
    };
    let (a, b) = (hash(1), hash(2));
    format!(
        "{:08x}-{:04x}-4{:03x}-{:04x}-{:012x}",
        (a >> 32) as u32,
        (a >> 16) as u16,
        a as u16 & 0xfff,
        ((b >> 48) as u16 & 0x3fff) | 0x8000,
        b & 0xffff_ffff_ffff
    )
}

// ---------------------------------------------------------------------------
// Zip
// ---------------------------------------------------------------------------

pub struct ZipEntry {
    pub name: String,
    pub data: Vec<u8>,
    /// Stored without compression (the `mimetype` file must be).
    pub stored: bool,
}

impl ZipEntry {
    pub fn new(name: impl Into<String>, data: impl Into<Vec<u8>>) -> Self {
        ZipEntry { name: name.into(), data: data.into(), stored: false }
    }

    pub fn stored(name: impl Into<String>, data: impl Into<Vec<u8>>) -> Self {
        ZipEntry { name: name.into(), data: data.into(), stored: true }
    }
}

pub fn write_zip(entries: &[ZipEntry]) -> Result<Vec<u8>, String> {
    let mut w = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for e in entries {
        let method = if e.stored { zip::CompressionMethod::Stored } else { zip::CompressionMethod::Deflated };
        let opts = zip::write::SimpleFileOptions::default().compression_method(method).last_modified_time(zip::DateTime::default());
        w.start_file(e.name.as_str(), opts).map_err(|x| x.to_string())?;
        w.write_all(&e.data).map_err(|x| x.to_string())?;
    }
    Ok(w.finish().map_err(|x| x.to_string())?.into_inner())
}

/// The blocks of a document without the empty paragraphs that end it (Word keeps one after a final table).
pub fn trim_end(blocks: &[RBlock]) -> &[RBlock] {
    let mut n = blocks.len();
    while n > 0 {
        match &blocks[n - 1] {
            RBlock::Para(p) if p.segs.is_empty() && p.list.is_none() && !p.hr => n -= 1,
            RBlock::Opaque { .. } => n -= 1,
            _ => break,
        }
    }
    &blocks[..n]
}
