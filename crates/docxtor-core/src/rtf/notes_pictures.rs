//! Footnotes and endnotes, and pictures.

use crate::fragment::{inline_drawing, sniff_image, FragNote};
use crate::xml::{Element, Node};
use super::state::*;

impl Interp {
    pub(super) fn note_run(style: &str, mark: Element) -> Element {
        Element::new("w:r").with_child(Element::new("w:rPr").with_child(Element::new("w:rStyle").with_attr("w:val", style))).with_child(mark)
    }

    pub(super) fn open_footnote(&mut self) -> bool {
        self.flush_run();
        self.note_count += 1;
        let key = format!("n{}", self.note_count);
        let _ = self.builtin("Footnote Reference");
        let _ = self.builtin("Footnote Text");
        let r = Self::note_run("FootnoteReference", Element::new("w:footnoteReference").with_attr("w:id", key.as_str()));
        self.push_inline(r);
        self.stories.push(Story { note: Some(NoteInfo { key, endnote: false }), ..Default::default() });
        let g = self.g();
        g.dest = Dest::Footnote;
        g.own = Own::Footnote;
        g.ch = CharFmt::default();
        g.pf = ParaFmt::default();
        true
    }

    pub(super) fn close_footnote(&mut self) {
        self.end_para(EndKind::Soft);
        if self.story().tbl.row_open || !self.story().tbl.cells.is_empty() || !self.story().tbl.cell_blocks.is_empty() {
            self.end_row();
        }
        self.flush_table();
        if self.stories.len() < 2 {
            return;
        }
        let Some(st) = self.stories.pop() else { return };
        let Some(info) = st.note else { return };
        let tag = if info.endnote { "w:endnote" } else { "w:footnote" };
        let mut el = Element::new(tag).with_attr("w:id", info.key.as_str());
        if st.blocks.is_empty() {
            el.children.push(Node::Element(Element::new("w:p")));
        }
        el.children.extend(st.blocks.into_iter().map(Node::Element));
        self.notes.insert(info.key, FragNote { kind: if info.endnote { "endnote".into() } else { "footnote".into() }, xml: el.to_xml() });
    }

    /// `\chftn` inside a note: the note's own number mark.
    pub(super) fn note_mark(&mut self) {
        let st = self.story();
        let Some(info) = &st.note else { return };
        if st.chftn_done {
            return;
        }
        let endnote = info.endnote;
        st.chftn_done = true;
        self.flush_run();
        let r = if endnote {
            Self::note_run("EndnoteReference", Element::new("w:endnoteRef"))
        } else {
            Self::note_run("FootnoteReference", Element::new("w:footnoteRef"))
        };
        self.push_inline(r);
    }

    /// `\ftnalt`: the note is an endnote, so the reference made when the group opened changes kind.
    pub(super) fn mark_endnote(&mut self) {
        let n = self.stories.len();
        if n < 2 {
            return;
        }
        match self.stories[n - 1].note.as_mut() {
            Some(info) => info.endnote = true,
            None => return,
        }
        let _ = self.builtin("Endnote Reference");
        let _ = self.builtin("Endnote Text");
        let outer = &mut self.stories[n - 2];
        let Some(cur) = outer.cur.as_mut() else { return };
        let sink = match cur.link.as_mut() {
            Some(l) => &mut l.children,
            None => &mut cur.inlines,
        };
        if let Some(last) = sink.last_mut() {
            for node in last.children.iter_mut() {
                if let Node::Element(c) = node {
                    if c.name == "w:footnoteReference" {
                        c.name = "w:endnoteReference".into();
                    } else if c.name == "w:rPr" {
                        for r in c.elements_mut() {
                            if r.name == "w:rStyle" {
                                r.set_attr("w:val", "EndnoteReference");
                            }
                        }
                    }
                }
            }
        }
    }

    pub(super) fn finish_pict(&mut self) {
        let pict = std::mem::take(&mut self.pict);
        if pict.kind == PKind::Unsupported {
            self.b.warn("picture-unsupported");
            return;
        }
        if pict.data.is_empty() {
            self.b.warn("picture-unreadable");
            return;
        }
        let (pw, ph) = sniff_image(&pict.data).map(|(_, w, h)| (w as f64, h as f64)).unwrap_or((0.0, 0.0));
        let Some(key) = self.b.media(&pict.data) else {
            self.b.warn(if pict.kind == PKind::Raster { "picture-unreadable" } else { "picture-unsupported" });
            return;
        };
        let sx = if pict.sx > 0 { pict.sx as f64 / 100.0 } else { 1.0 };
        let sy = if pict.sy > 0 { pict.sy as f64 / 100.0 } else { 1.0 };
        let size = |goal: i64, px: f64, raw: i64, scale: f64| -> f64 {
            let v = if goal > 0 {
                goal as f64 / 20.0 * scale
            } else if px > 0.0 {
                px * 0.75
            } else if raw > 0 {
                raw as f64 * 0.75
            } else {
                100.0
            };
            v.clamp(1.0, 4000.0)
        };
        let w = size(pict.wgoal, pw, pict.picw, sx);
        let h = size(pict.hgoal, ph, pict.pich, sy);
        self.flush_run();
        let id = self.b.drawing_id();
        let r = Element::new("w:r").with_child(inline_drawing(&key, w, h, id, ""));
        self.any_content = true;
        self.push_inline(r);
    }
}
