//! Go to: the places of one kind in the main text (headings, tables, pictures, equations, fields, notes and
//! sections), in document order, so the interface can jump to the next or the previous one. Pages and lines depend
//! on the layout, so the interface works those out; bookmarks have their own list (`bookmarks`).

use super::*;
use crate::fields;
use serde::Serialize;

/// A place the interface can go to.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GotoTarget {
    pub p: NodeId,
    pub o: usize,
    /// Where a selection of the thing ends (a field or equation selects whole); the start for a point.
    pub end_p: NodeId,
    pub end_o: usize,
    /// A short name: "Heading 2: Methods", "Table 3", "Footnote 2".
    pub label: String,
}

fn short(s: &str, n: usize) -> String {
    let s = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if s.chars().count() > n {
        format!("{}...", s.chars().take(n).collect::<String>().trim_end())
    } else {
        s
    }
}

fn push_point(out: &mut Vec<GotoTarget>, p: NodeId, o: usize, label: String) {
    out.push(GotoTarget { p, o, end_p: p, end_o: o, label });
}

impl Document {
    /// The places of a kind: `heading`, `table`, `graphic`, `equation`, `field`, `footnote`, `endnote` or `section`.
    pub fn goto_targets(&self, kind: &str) -> Result<Vec<GotoTarget>, EditError> {
        let mut out: Vec<GotoTarget> = Vec::new();
        let body = &self.body;
        match kind {
            "heading" => {
                walk_paragraphs(body, &mut |p| {
                    if let Some(level) = self.heading_level(p) {
                        let text = p.plain_text();
                        if !text.trim().is_empty() {
                            push_point(&mut out, p.id, 0, format!("Heading {level}: {}", short(&text, 60)));
                        }
                    }
                });
            }
            "table" => {
                let mut n = 0;
                fn tables<'a>(blocks: impl IntoIterator<Item = &'a Block>, f: &mut dyn FnMut(&'a Table)) {
                    for b in blocks {
                        match b {
                            Block::Table(t) => {
                                f(t);
                                for c in t.rows().flat_map(|r| r.cells()) {
                                    tables(&c.blocks, f);
                                }
                            }
                            Block::Container(c) => tables(&c.blocks, f),
                            _ => {}
                        }
                    }
                }
                tables(body, &mut |t| {
                    let first = t.rows().flat_map(|r| r.cells()).flat_map(|c| c.blocks.iter()).find_map(|b| if let Block::Para(p) = b { Some(p.id) } else { None });
                    if let Some(id) = first {
                        n += 1;
                        push_point(&mut out, id, 0, format!("Table {n}"));
                    }
                });
            }
            "graphic" | "equation" => {
                let want_math = kind == "equation";
                let mut n = 0;
                walk_paragraphs(body, &mut |p| {
                    let mut at = 0;
                    fn go(inlines: &[Inline], at: &mut usize, want_math: bool, hits: &mut Vec<usize>) {
                        for i in inlines {
                            match i {
                                Inline::Run(r) => {
                                    for it in &r.items {
                                        if let RunItem::Object(e) = it {
                                            let drawn = matches!(e.name.as_str(), "w:drawing" | "w:pict" | "w:object" | "mc:AlternateContent");
                                            if !want_math && drawn && !crate::marks::is_watermark_pict(e) {
                                                hits.push(*at);
                                            }
                                        }
                                        *at += it.len16();
                                    }
                                }
                                Inline::Container(c) => go(&c.children, at, want_math, hits),
                                Inline::Opaque(e) => {
                                    if want_math && crate::math::is_math(e) {
                                        hits.push(*at);
                                    }
                                    *at += i.len16();
                                }
                            }
                        }
                    }
                    let mut hits = Vec::new();
                    go(&p.inlines, &mut at, want_math, &mut hits);
                    for o in hits {
                        n += 1;
                        out.push(GotoTarget { p: p.id, o, end_p: p.id, end_o: o + 1, label: format!("{} {n}", if want_math { "Equation" } else { "Graphic" }) });
                    }
                });
            }
            "field" => {
                let mut n = 0;
                fields::scan_blocks(body, &mut |id, s| {
                    n += 1;
                    let code = fields::Instr::parse(s.instr).code;
                    let len = s.result.encode_utf16().count();
                    out.push(GotoTarget { p: id, o: s.offset, end_p: id, end_o: s.offset + len, label: format!("Field {n}: {code}") });
                });
            }
            "footnote" | "endnote" => {
                let tag = if kind == "footnote" { "w:footnoteReference" } else { "w:endnoteReference" };
                let story_kind = if kind == "footnote" { StoryKind::Footnote } else { StoryKind::Endnote };
                let mut n = 0;
                walk_paragraphs(body, &mut |p| {
                    let mut at = 0;
                    walk_runs(&p.inlines, &mut at, &mut |r, start, _| {
                        let mut pos = start;
                        for it in &r.items {
                            if let RunItem::Object(e) = it {
                                if e.name == tag {
                                    n += 1;
                                    let text = e.attr("w:id").and_then(|id| self.stories.iter().find(|s| s.kind == story_kind && s.note_id.as_deref() == Some(id))).map(|s| {
                                        let mut t = String::new();
                                        walk_paragraphs(&s.blocks, &mut |np| t.push_str(&np.plain_text()));
                                        short(&t, 40)
                                    });
                                    let name = if kind == "footnote" { "Footnote" } else { "Endnote" };
                                    let label = match text.filter(|t| !t.is_empty()) {
                                        Some(t) => format!("{name} {n}: {t}"),
                                        None => format!("{name} {n}"),
                                    };
                                    out.push(GotoTarget { p: p.id, o: pos, end_p: p.id, end_o: pos + 1, label });
                                }
                            }
                            pos += it.len16();
                        }
                    });
                });
            }
            "section" => {
                let mut n = 0;
                let mut starts_here = true;
                walk_paragraphs(body, &mut |p| {
                    if starts_here {
                        n += 1;
                        push_point(&mut out, p.id, 0, format!("Section {n}"));
                        starts_here = false;
                    }
                    if p.ppr.as_ref().is_some_and(|pp| pp.child("w:sectPr").is_some()) {
                        starts_here = true;
                    }
                });
            }
            other => return Err(EditError::Invalid(format!("unknown kind to go to: {other}"))),
        }
        Ok(out)
    }
}
