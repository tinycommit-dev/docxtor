//! Exports to other formats: Markdown (GitHub-flavoured, with footnotes) and plain text here, and a
//! web page, an EPUB book, OpenDocument text and Rich Text in the modules below. All are lossy by
//! nature and read from the rendered view model, so they see the document the way the editor shows
//! it (list numbers, footnote numbers, field results). None of them changes the document.

pub mod common;
pub mod epub;
pub mod html;
pub mod odt;
pub mod rtf;
pub mod svg_clean;

use crate::ai::{para_md_as, Flavor};
use crate::document::Document;
use crate::model::NodeId;
use crate::ops::Pos;
use crate::render::{RBlock, RPara, RTable};
use std::fmt::Write as _;

/// The body as Markdown, followed by footnote definitions. Headers and footers are left out.
pub fn to_markdown(doc: &Document) -> String {
    let mut out = String::new();
    let mut st = MdState::default();
    // A block at a time: the drawing of the whole document is not held, only the Markdown written so far.
    doc.render_snapshot_each(&mut |b| md_blocks(std::slice::from_ref(&b), &mut out, &mut st));
    let setup = doc.page_setup();
    for (notes, key) in [(&setup.notes, ""), (&setup.endnotes, "end:")] {
        if notes.is_empty() {
            continue;
        }
        out.push('\n');
        for n in notes {
            let text: Vec<String> = n.blocks.iter().filter_map(|b| if let RBlock::Para(p) = b { Some(para_md_as(p, Flavor::Markdown)) } else { None }).collect();
            let text = text.join(" ");
            // The note's own reference mark opens its text; the definition syntax already says which note it is.
            let text = text.trim().strip_prefix(&format!("[^{key}{}]", n.label)).unwrap_or(text.trim());
            let _ = writeln!(out, "[^{key}{}]: {}", n.label, text.trim());
        }
    }
    // Cut and ended in place: a second copy of a long document's Markdown is not made.
    out.truncate(out.trim_end().len());
    out.push('\n');
    out
}

/// The text between two positions in the body as Markdown (for "Copy as Markdown"). Whole blocks
/// in between are kept as they are; the first and last paragraphs are cut at the positions. None
/// when either end isn't in the body (a header, footer or footnote).
pub fn range_to_markdown(doc: &Document, from: Pos, to: Pos) -> Option<String> {
    let (a, b) = (from.p.0, to.p.0);
    // A block at a time: only the blocks from the first of the two ends to the second are kept.
    let (mut idx, mut i, mut j, mut done) = (0usize, None, None, false);
    let mut kept: Vec<RBlock> = Vec::new();
    doc.render_snapshot_each(&mut |blk| {
        if i.is_none() && contains(&blk, a) {
            i = Some(idx);
        }
        if j.is_none() && contains(&blk, b) {
            j = Some(idx);
        }
        idx += 1;
        // Keeping starts at the first end found and stops once both are.
        if !done && (i.is_some() || j.is_some()) {
            kept.push(blk);
            done = i.is_some() && j.is_some();
        }
    });
    let (i, j) = (i?, j?);
    let ((i, a, ao), (j, b, bo)) = if (i, from.o) <= (j, to.o) { ((i, a, from.o), (j, b, to.o)) } else { ((j, b, to.o), (i, a, from.o)) };
    let mut blocks: Vec<RBlock> = kept;
    blocks.truncate(j - i + 1);
    let last = blocks.len() - 1;
    if let RBlock::Para(p) = &mut blocks[last] {
        if p.id == b {
            cut(p, 0, bo);
        }
    }
    if let RBlock::Para(p) = &mut blocks[0] {
        if p.id == a {
            // Cutting the end left offsets as they were, so this works within one paragraph too.
            cut(p, ao, usize::MAX);
        }
    }
    let mut out = String::new();
    md_blocks(&blocks, &mut out, &mut MdState::default());
    Some(format!("{}\n", out.trim_end()))
}


fn contains(b: &RBlock, id: NodeId) -> bool {
    match b {
        RBlock::Para(p) => p.id == id,
        RBlock::Table(t) => t.rows.iter().flat_map(|r| &r.cells).any(|c| c.blocks.iter().any(|x| contains(x, id))),
        RBlock::Container { blocks, .. } => blocks.iter().any(|x| contains(x, id)),
        RBlock::Opaque { .. } => false,
    }
}

/// Keep only the part of a paragraph between two UTF-16 offsets.
fn cut(p: &mut RPara, from: usize, to: usize) {
    let mut segs = Vec::new();
    for mut s in std::mem::take(&mut p.segs) {
        let len = match (s.k, &s.x) {
            ("t", Some(x)) => x.encode_utf16().count(),
            ("ghost", _) => 0,
            _ => 1,
        };
        let (start, end) = (s.o, s.o + len);
        if (len > 0 && end <= from) || (len == 0 && start < from) || start >= to {
            continue;
        }
        if let ("t", Some(x)) = (s.k, &s.x) {
            let units: Vec<u16> = x.encode_utf16().collect();
            let lo = from.saturating_sub(start).min(units.len());
            let hi = to.saturating_sub(start).min(units.len());
            s.x = Some(String::from_utf16_lossy(&units[lo..hi]));
        }
        s.o = s.o.saturating_sub(from);
        segs.push(s);
    }
    p.segs = segs;
    p.len = to.min(p.len).saturating_sub(from);
}

#[derive(Default)]
struct MdState {
    /// The previous block was a list item.
    in_list: bool,
    /// Width of the last marker at each list level ("- " is 2, "10. " is 4): nested items indent by these.
    widths: Vec<usize>,
}

fn md_blocks(blocks: &[RBlock], out: &mut String, st: &mut MdState) {
    for b in blocks {
        match b {
            RBlock::Para(p) => md_para(p, out, st),
            RBlock::Table(t) => {
                sep(out, false, st);
                md_table(t, out);
            }
            RBlock::Container { blocks, .. } => md_blocks(blocks, out, st),
            RBlock::Opaque { .. } => {}
        }
    }
}

/// Blank line between blocks, except between items of the same list.
fn sep(out: &mut String, list: bool, st: &mut MdState) {
    let same_list = list && st.in_list;
    if !out.is_empty() && !same_list {
        out.push('\n');
    }
    if !list {
        st.widths.clear();
    }
    st.in_list = list;
}

fn heading_level(style: &str) -> Option<usize> {
    if style == "Title" {
        return Some(1);
    }
    let n: usize = style.strip_prefix("Heading")?.trim().parse().ok()?;
    (1..=9).contains(&n).then_some(n.min(6))
}

fn md_para(p: &RPara, out: &mut String, st: &mut MdState) {
    // A checked task item is struck through by the editor; Markdown says it with `[x]`, so don't double it as `~~`.
    let text = match &p.list {
        Some(l) if l.checked => {
            let mut q = p.clone();
            for s in &mut q.segs {
                s.f.strike = None;
            }
            para_md_as(&q, Flavor::Markdown)
        }
        _ => para_md_as(p, Flavor::Markdown),
    };
    let text = text.trim_end();
    if text.trim().is_empty() {
        return;
    }
    let style = p.style.as_deref().unwrap_or("Normal");
    if let Some(l) = &p.list {
        sep(out, true, st);
        let level = l.level.max(0) as usize;
        st.widths.resize(level, 2);
        let pad = " ".repeat(st.widths.iter().sum());
        let label = l.text.trim();
        let marker = if l.bullet {
            "-".to_string()
        } else if label.strip_suffix(['.', ')']).is_some_and(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit())) {
            label.to_string()
        } else {
            "1.".to_string()
        };
        let task = match (l.checklist, l.checked) {
            (true, true) => "[x] ",
            (true, false) => "[ ] ",
            _ => "",
        };
        let _ = writeln!(out, "{pad}{marker} {task}{}", text.trim_start());
        st.widths.push(marker.len() + 1);
        return;
    }
    sep(out, false, st);
    if let Some(n) = heading_level(style) {
        let _ = writeln!(out, "{} {}", "#".repeat(n), text.trim().replace("\\\n", " "));
    } else if style.contains("Quote") {
        for line in text.split('\n') {
            let _ = writeln!(out, "> {line}");
        }
    } else {
        let _ = writeln!(out, "{}", text.trim_start());
    }
}

fn cell_text(blocks: &[RBlock]) -> String {
    let mut parts = Vec::new();
    for b in blocks {
        match b {
            RBlock::Para(p) => parts.push(para_md_as(p, Flavor::Markdown).trim().to_string()),
            RBlock::Table(t) => {
                for r in &t.rows {
                    for c in &r.cells {
                        parts.push(cell_text(&c.blocks));
                    }
                }
            }
            RBlock::Container { blocks, .. } => parts.push(cell_text(blocks)),
            RBlock::Opaque { .. } => {}
        }
    }
    parts.retain(|p| !p.is_empty());
    parts.join("<br>").replace("\\\n", "<br>").replace('\n', " ").replace('|', "\\|")
}

fn md_table(t: &RTable, out: &mut String) {
    let cols = t.rows.iter().map(|r| r.cells.iter().map(|c| c.span.max(1) as usize).sum::<usize>()).max().unwrap_or(0);
    if cols == 0 {
        return;
    }
    for (ri, r) in t.rows.iter().enumerate() {
        let mut cells: Vec<String> = Vec::with_capacity(cols);
        for c in &r.cells {
            let mut text = if c.vmerge.as_deref() == Some("continue") { String::new() } else { cell_text(&c.blocks) };
            // Header cells render bold anyway.
            if ri == 0 && text.len() > 4 && text.starts_with("**") && text.ends_with("**") && !text[2..text.len() - 2].contains("**") {
                text = text[2..text.len() - 2].to_string();
            }
            cells.push(text);
            for _ in 1..c.span.max(1) {
                cells.push(String::new());
            }
        }
        cells.resize(cols, String::new());
        let _ = writeln!(out, "| {} |", cells.join(" | "));
        if ri == 0 {
            let _ = writeln!(out, "|{}", " --- |".repeat(cols));
        }
    }
}

/// The body as plain text: one line per paragraph, list labels kept, table cells separated by tabs.
pub fn to_text(doc: &Document) -> String {
    let mut out = String::new();
    doc.render_snapshot_each(&mut |b| text_blocks(std::slice::from_ref(&b), &mut out));
    // Word keeps an empty paragraph after a final table; a text file doesn't need it.
    out.truncate(out.trim_end_matches('\n').len());
    out.push('\n');
    out
}

fn para_text(p: &RPara) -> String {
    let mut t = String::new();
    if let Some(l) = &p.list {
        t.push_str(&"\t".repeat(l.level.max(0) as usize));
        t.push_str(if l.bullet && !l.checklist { "•" } else { l.text.trim() });
        t.push(' ');
    }
    for s in &p.segs {
        match s.k {
            "t" => t.push_str(s.x.as_deref().unwrap_or("")),
            "tab" => t.push('\t'),
            "br" => t.push('\n'),
            "note" => {
                let _ = write!(t, "[{}]", s.label.as_deref().unwrap_or(""));
            }
            // An equation is its LaTeX between dollar signs.
            "math" => {
                if let Some(m) = &s.math {
                    let _ = if m.display { write!(t, "$${}$$", m.latex) } else { write!(t, "${}$", m.latex) };
                }
            }
            _ => {}
        }
    }
    t
}

fn text_blocks(blocks: &[RBlock], out: &mut String) {
    for b in blocks {
        match b {
            RBlock::Para(p) => {
                out.push_str(&para_text(p));
                out.push('\n');
            }
            RBlock::Table(t) => {
                for r in &t.rows {
                    let cells: Vec<String> = r
                        .cells
                        .iter()
                        .map(|c| {
                            let mut s = String::new();
                            text_blocks(&c.blocks, &mut s);
                            s.trim_end().replace('\n', " ")
                        })
                        .collect();
                    out.push_str(&cells.join("\t"));
                    out.push('\n');
                }
            }
            RBlock::Container { blocks, .. } => text_blocks(blocks, out),
            RBlock::Opaque { .. } => {}
        }
    }
}


