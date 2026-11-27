//! Text, runs and paragraphs: collecting inlines, runs, links and fields, and ending paragraphs.

use crate::xml::{Element, Node};
use super::state::*;

pub(super) fn simple_run(e: &Element) -> bool {
    e.name == "w:r" && e.elements().all(|c| matches!(c.name.as_str(), "w:rPr" | "w:t" | "w:tab") || (c.name == "w:br" && c.attr("w:type").is_none()))
}

/// Append `new` to `last` when both are plain runs with the same formatting.
pub(super) fn try_merge(last: &mut Element, new: &Element) -> bool {
    if !simple_run(last) || !simple_run(new) || last.child("w:rPr") != new.child("w:rPr") {
        return false;
    }
    for c in new.elements().filter(|c| c.name != "w:rPr") {
        if c.name == "w:t" {
            if let Some(Node::Element(l)) = last.children.last_mut() {
                if l.name == "w:t" {
                    let add = c.text();
                    if let Some(Node::Text(t)) = l.children.last_mut() {
                        t.push_str(&add);
                        continue;
                    }
                }
            }
        }
        last.children.push(Node::Element(c.clone()));
    }
    true
}

pub(super) fn text_el(text: String) -> Element {
    Element::new("w:t").with_attr("xml:space", "preserve").with_text(text)
}

pub(super) fn is_safe_url(u: &str) -> bool {
    let t = u.trim();
    if t.is_empty() {
        return false;
    }
    match t.find(':') {
        Some(i) if t[..i].chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '.' | '-')) => {
            matches!(t[..i].to_ascii_lowercase().as_str(), "http" | "https" | "mailto" | "tel" | "ftp" | "ftps" | "news" | "sms")
        }
        _ => true,
    }
}

/// Split a field instruction into words, keeping quoted strings together.
pub(super) fn split_args(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quoted = false;
    let mut any = false;
    for c in s.chars() {
        match c {
            '"' => {
                quoted = !quoted;
                any = true;
            }
            c if c.is_whitespace() && !quoted => {
                if any || !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                    any = false;
                }
            }
            c => cur.push(c),
        }
    }
    if any || !cur.is_empty() {
        out.push(cur);
    }
    out
}

pub(super) fn parse_hyperlink(inst: &str) -> Option<LinkTarget> {
    let args = split_args(inst);
    if !args.first()?.eq_ignore_ascii_case("hyperlink") {
        return None;
    }
    let mut url: Option<String> = None;
    let mut anchor: Option<String> = None;
    let mut i = 1;
    while i < args.len() {
        let a = &args[i];
        if let Some(sw) = a.strip_prefix('\\') {
            match sw.to_ascii_lowercase().as_str() {
                "l" => {
                    anchor = args.get(i + 1).cloned();
                    i += 1;
                }
                "o" | "t" => i += 1,
                _ => {}
            }
        } else if url.is_none() {
            url = Some(a.clone());
        }
        i += 1;
    }
    match (url, anchor) {
        (Some(u), Some(a)) if is_safe_url(&u) => Some(LinkTarget { url: Some(format!("{u}#{a}")), anchor: None }),
        (Some(u), None) if is_safe_url(&u) => Some(LinkTarget { url: Some(u), anchor: None }),
        (None, Some(a)) if !a.is_empty() => Some(LinkTarget { url: None, anchor: Some(a) }),
        _ => None,
    }
}

impl Interp {
    pub(super) fn body_text(&mut self, s: &str) {
        if !self.rtf_seen {
            return;
        }
        if !self.any_content && self.story().cur.is_none() && s.trim().is_empty() {
            return;
        }
        let clean = |s: &str| s.bytes().all(|b| b >= 0x20) && !s.contains(['\u{FFFE}', '\u{FFFF}']);
        let tmp: String;
        let s = if clean(s) {
            s
        } else {
            tmp = s.chars().filter(|c| (*c as u32) >= 0x20 || *c == '\t').filter(|c| !matches!(*c, '\u{FFFE}' | '\u{FFFF}')).collect();
            &tmp
        };
        if s.is_empty() {
            return;
        }
        self.any_content = true;
        let mut first = true;
        for part in s.split('\t') {
            if !first {
                self.push_piece(Element::new("w:tab"));
            }
            first = false;
            if !part.is_empty() {
                self.ensure_run();
                if let Some(r) = self.story().run.as_mut() {
                    r.text.push_str(part);
                }
            }
        }
    }

    pub(super) fn ensure_cur(&mut self) {
        let st = self.story();
        if st.cur.is_none() {
            let mut c = CurPara::default();
            if let Some(t) = &st.link_active {
                c.link = Some(LinkAcc { target: t.clone(), children: Vec::new() });
            }
            st.cur = Some(c);
        }
    }

    /// Make sure a pending run exists with the current character formatting.
    pub(super) fn ensure_run(&mut self) {
        self.ensure_cur();
        let (fmt, ps) = {
            let g = &self.stack[self.stack.len() - 1];
            (g.ch.clone(), g.pf.style)
        };
        let link = self.story().link_active.is_some();
        let same = matches!(&self.story().run, Some(r) if r.fmt == fmt && r.link == link);
        if !same {
            self.flush_run();
            self.story().run = Some(PendRun { fmt, pstyle: ps, link, pieces: Vec::new(), text: String::new() });
        }
    }

    pub(super) fn push_piece(&mut self, el: Element) {
        if !self.rtf_seen {
            return;
        }
        self.any_content = true;
        self.ensure_run();
        if let Some(r) = self.story().run.as_mut() {
            if !r.text.is_empty() {
                let t = std::mem::take(&mut r.text);
                r.pieces.push(text_el(t));
            }
            r.pieces.push(el);
        }
    }

    pub(super) fn flush_run(&mut self) {
        let Some(mut run) = self.story().run.take() else { return };
        if !run.text.is_empty() {
            let t = std::mem::take(&mut run.text);
            run.pieces.push(text_el(t));
        }
        if run.pieces.is_empty() {
            return;
        }
        let rstyle = if run.link { self.builtin("Hyperlink") } else { run.fmt.cstyle.and_then(|n| self.ensure_style(true, n)) };
        let base = self.run_base(run.pstyle, run.fmt.cstyle);
        let rpr = self.rpr(&run.fmt, &base, rstyle.as_deref());
        let mut r = Element::new("w:r");
        if !rpr.children.is_empty() {
            r.children.push(Node::Element(rpr));
        }
        r.children.extend(run.pieces.into_iter().map(Node::Element));
        self.push_inline(r);
    }

    pub(super) fn push_inline(&mut self, el: Element) {
        self.ensure_cur();
        let Some(cur) = self.story().cur.as_mut() else { return };
        let sink = match cur.link.as_mut() {
            Some(l) => &mut l.children,
            None => &mut cur.inlines,
        };
        if let Some(last) = sink.last_mut() {
            if try_merge(last, &el) {
                return;
            }
        }
        sink.push(el);
    }

    pub(super) fn close_link(&mut self) {
        let l = match self.story().cur.as_mut().and_then(|c| c.link.take()) {
            Some(l) => l,
            None => return,
        };
        if l.children.is_empty() {
            return;
        }
        let mut h = Element::new("w:hyperlink");
        if let Some(u) = &l.target.url {
            let key = self.b.link(u);
            h.set_attr("r:id", key);
        } else if let Some(a) = &l.target.anchor {
            h.set_attr("w:anchor", a.as_str());
        }
        h.children = l.children.into_iter().map(Node::Element).collect();
        if let Some(cur) = self.story().cur.as_mut() {
            cur.inlines.push(h);
        }
    }

    /// Start a field result. Returns whether it is a link (the group then closes it).
    pub(super) fn open_fldrslt(&mut self) -> bool {
        let inst = self.fields.last().map(|f| f.inst.clone()).unwrap_or_default();
        let Some(t) = parse_hyperlink(&inst) else { return false };
        if self.story().link_active.is_some() {
            return false;
        }
        self.flush_run();
        let st = self.story();
        st.link_active = Some(t.clone());
        if let Some(cur) = st.cur.as_mut() {
            if cur.link.is_none() {
                cur.link = Some(LinkAcc { target: t, children: Vec::new() });
            }
        }
        true
    }

    // -----------------------------------------------------------------------
    // Paragraphs and tables
    // -----------------------------------------------------------------------

    pub(super) fn end_para(&mut self, kind: EndKind) {
        self.flush_run();
        self.close_link();
        let cur = self.story().cur.take();
        if cur.is_none() && kind == EndKind::Soft {
            return;
        }
        let cur = cur.unwrap_or_default();
        let pf = self.g().pf.clone();
        let el = self.build_para(cur, &pf);
        self.add_block(el, pf.intbl, kind == EndKind::Cell);
        self.any_content = true;
    }

    pub(super) fn add_block(&mut self, el: Element, intbl: bool, force_cell: bool) {
        let in_table = force_cell || intbl || self.story().tbl.row_open;
        if in_table {
            self.story().tbl.cell_blocks.push(el);
        } else {
            self.flush_table();
            self.story().blocks.push(el);
        }
    }
}
