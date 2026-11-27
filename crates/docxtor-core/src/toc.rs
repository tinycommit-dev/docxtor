//! Tables of contents, written the way Word writes them: a `TOC` field whose cached result is one
//! paragraph per heading (styles `TOC1`…`TOC9`), each a link to a `_Toc…` bookmark on its heading
//! with a `PAGEREF` field for the page number. Word can update the field itself; Docxtor rebuilds the
//! result from the headings and the page numbers its own layout measured.

use crate::toc_spec::Spec;
use crate::model::{Block, Inline, Paragraph, RunItem};
use serde::{Deserialize, Serialize};
use crate::xml::Element;

/// One line of the table: the heading's paragraph, its level (1-based), text and bookmark.
#[derive(Debug, Clone)]
pub struct Entry {
    pub para: crate::model::NodeId,
    pub level: u8,
    pub text: String,
    /// The heading's list number ("1", "2.3."), written before the text and a tab as Word does. Bullets give none.
    pub label: Option<String>,
    pub bookmark: String,
    /// The entry shows no page number (a `TC` field with `\n`, or a level the table leaves out).
    pub no_page: bool,
}

/// The default field instruction: heading levels 1–3, entries as links, no page numbers on the web.
pub fn default_instr(levels: u8) -> String {
    format!(r#" TOC \o "1-{}" \h \z \u "#, levels.clamp(1, 9))
}

/// Heading levels a `TOC` instruction includes (`\o "1-3"`), defaulting to 1–3.
pub fn levels_of(instr: &str) -> (u8, u8) {
    let Some(i) = instr.find("\\o") else { return (1, 3) };
    let rest = instr[i + 2..].trim_start().trim_start_matches('"');
    let spec: String = rest.chars().take_while(|c| c.is_ascii_digit() || *c == '-').collect();
    let mut parts = spec.split('-').filter_map(|p| p.parse::<u8>().ok());
    match (parts.next(), parts.next()) {
        (Some(a), Some(b)) if a >= 1 && b >= a => (a, b.min(9)),
        (Some(a), None) if a >= 1 => (a, a),
        _ => (1, 3),
    }
}

fn run(rpr: Option<Element>, item: Element) -> Element {
    let mut r = Element::new("w:r");
    if let Some(rp) = rpr {
        r = r.with_child(rp);
    }
    r.with_child(item)
}

fn hidden_rpr() -> Element {
    Element::new("w:rPr").with_child(Element::new("w:noProof")).with_child(Element::new("w:webHidden"))
}

fn fld(kind: &str) -> Element {
    Element::new("w:fldChar").with_attr("w:fldCharType", kind)
}

fn instr(text: &str) -> Element {
    Element::new("w:instrText").with_attr("xml:space", "preserve").with_text(text)
}

/// The paragraphs of the field result. `instr_text` is the field's instruction; `tab_pos` is the
/// right edge of the text column (twips), where page numbers align.
pub fn paragraphs(entries: &[Entry], instr_text: &str, tab_pos: i64, page: &dyn Fn(&Entry) -> String) -> Vec<Element> {
    paragraphs_styled(entries, instr_text, tab_pos, page, &|e| format!("TOC{}", e.level), "No table of contents entries found.")
}

/// Like [`paragraphs`], for lists that aren't headings (a table of figures): `style` names each entry's
/// paragraph style and `empty` is what the result says when there are no entries.
pub fn paragraphs_styled(entries: &[Entry], instr_text: &str, tab_pos: i64, page: &dyn Fn(&Entry) -> String, style: &dyn Fn(&Entry) -> String, empty: &str) -> Vec<Element> {
    paragraphs_with(entries, instr_text, &Layout::classic(tab_pos), page, style, empty)
}

/// The tab leader between an entry and its page number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Leader {
    #[default]
    Dot,
    Hyphen,
    Underscore,
    None,
}

impl Leader {
    pub fn val(self) -> &'static str {
        match self {
            Leader::Dot => "dot",
            Leader::Hyphen => "hyphen",
            Leader::Underscore => "underscore",
            Leader::None => "none",
        }
    }

    pub fn parse(s: &str) -> Option<Leader> {
        match s {
            "dot" | "middleDot" => Some(Leader::Dot),
            "hyphen" | "heavy" => Some(Leader::Hyphen),
            "underscore" => Some(Leader::Underscore),
            "none" => Some(Leader::None),
            _ => None,
        }
    }
}

/// How the result paragraphs of a list field are laid out.
#[derive(Debug, Clone)]
pub struct Layout {
    /// The right edge of the text column in twips, where aligned page numbers go.
    pub tab_pos: i64,
    pub leader: Leader,
    /// Page numbers sit at the right margin; otherwise they follow the entry after `sep`.
    pub right_align: bool,
    /// Entries are links to their headings.
    pub links: bool,
    /// The tab and page number are hidden in web layout (`\z`).
    pub web_hide: bool,
    /// What separates an entry from its page number when they are not aligned.
    pub sep: String,
}

impl Layout {
    /// Dot leaders, aligned numbers, links: what Docxtor has always written.
    pub fn classic(tab_pos: i64) -> Layout {
        Layout { tab_pos, leader: Leader::Dot, right_align: true, links: true, web_hide: true, sep: " ".into() }
    }

    /// The layout an instruction asks for. The leader isn't in the instruction (Word keeps it in the tab stops),
    /// so it is given: it is the one the existing result uses.
    pub fn of(spec: &Spec, tab_pos: i64, leader: Leader) -> Layout {
        let (right_align, sep) = match &spec.sep {
            Some(s) if !s.contains('\t') => (false, s.clone()),
            _ => (true, " ".into()),
        };
        Layout { tab_pos, leader, right_align, links: spec.links, web_hide: spec.web_hide, sep }
    }
}

/// The tab leader of an existing result paragraph (its right tab stop), if it has one.
pub fn leader_of(p: &Paragraph) -> Option<Leader> {
    let tabs = p.ppr.as_ref()?.child("w:tabs")?;
    let tab = tabs.children_named("w:tab").find(|t| t.attr("w:val") == Some("right"))?;
    Some(tab.attr("w:leader").and_then(Leader::parse).unwrap_or(Leader::None))
}

/// The paragraphs of a field result with a layout: tabs and leaders, links, page numbers on or off.
pub fn paragraphs_with(entries: &[Entry], instr_text: &str, lay: &Layout, page: &dyn Fn(&Entry) -> String, style: &dyn Fn(&Entry) -> String, empty: &str) -> Vec<Element> {
    let begin = || {
        vec![
            run(None, fld("begin")),
            run(None, instr(instr_text)),
            run(None, fld("separate")),
        ]
    };
    let end = || run(None, fld("end"));
    if entries.is_empty() {
        let mut p = Element::new("w:p");
        for r in begin() {
            p = p.with_child(r);
        }
        let text = crate::model::text_run_element(Some(Element::new("w:rPr").with_child(Element::new("w:noProof"))), empty);
        return vec![p.with_child(text).with_child(end())];
    }
    let hidden = || if lay.web_hide { hidden_rpr() } else { Element::new("w:rPr").with_child(Element::new("w:noProof")) };
    let last = entries.len() - 1;
    entries
        .iter()
        .enumerate()
        .map(|(i, e)| {
            let numbered = !e.no_page;
            let mut ppr = Element::new("w:pPr").with_child(Element::new("w:pStyle").with_attr("w:val", style(e)));
            let mut tabs = Element::new("w:tabs");
            if e.label.is_some() {
                // Word puts the text after a numbered heading's number at 440 twips a level.
                tabs = tabs.with_child(Element::new("w:tab").with_attr("w:val", "left").with_attr("w:pos", (440 * i64::from(e.level.max(1))).to_string()));
            }
            if numbered && lay.right_align {
                tabs = tabs.with_child(Element::new("w:tab").with_attr("w:val", "right").with_attr("w:leader", lay.leader.val()).with_attr("w:pos", lay.tab_pos.to_string()));
            }
            if tabs.children_named("w:tab").next().is_some() {
                ppr = ppr.with_child(tabs);
            }
            ppr = ppr.with_child(Element::new("w:rPr").with_child(Element::new("w:noProof")));
            let mut p = Element::new("w:p").with_child(ppr);
            if i == 0 {
                for r in begin() {
                    p = p.with_child(r);
                }
            }
            let no_proof = || Some(Element::new("w:rPr").with_child(Element::new("w:noProof")));
            let mut parts = Vec::new();
            if let Some(label) = &e.label {
                parts.push(crate::model::text_run_element(no_proof(), label));
                parts.push(run(no_proof(), Element::new("w:tab")));
            }
            parts.push(crate::model::text_run_element(no_proof(), &e.text));
            if numbered {
                parts.push(if lay.right_align {
                    run(Some(hidden()), Element::new("w:tab"))
                } else {
                    run(Some(hidden()), Element::new("w:t").with_attr("xml:space", "preserve").with_text(lay.sep.clone()))
                });
                parts.push(run(Some(hidden()), fld("begin")));
                parts.push(run(Some(hidden()), instr(&format!(" PAGEREF {} {}", e.bookmark, if lay.links { "\\h " } else { "" }))));
                parts.push(run(Some(hidden()), fld("separate")));
                parts.push(run(Some(hidden()), Element::new("w:t").with_text(page(e))));
                parts.push(run(Some(hidden()), fld("end")));
            }
            if lay.links {
                let mut link = Element::new("w:hyperlink").with_attr("w:anchor", e.bookmark.clone()).with_attr("w:history", "1");
                for r in parts {
                    link = link.with_child(r);
                }
                p = p.with_child(link);
            } else {
                for r in parts {
                    p = p.with_child(r);
                }
            }
            if i == last {
                p = p.with_child(end());
            }
            p
        })
        .collect()
}

/// Field characters and instructions of a paragraph, in order (looking inside links and other containers).
fn field_items(inlines: &[Inline], out: &mut Vec<(String, String)>) {
    for i in inlines {
        match i {
            Inline::Run(r) => {
                for item in &r.items {
                    if let RunItem::Hidden(e) = item {
                        match e.name.as_str() {
                            "w:fldChar" => out.push(("fld".into(), e.attr("w:fldCharType").unwrap_or_default().to_string())),
                            "w:instrText" => out.push(("instr".into(), e.text())),
                            _ => {}
                        }
                    } else if matches!(item, RunItem::Tab) && out.last().is_some_and(|(k, _)| k == "instr") {
                        // Word writes the tab of `INDEX \e "<tab>"` as a tab run between two instruction runs.
                        out.push(("instr".into(), "\t".into()));
                    }
                }
            }
            Inline::Container(c) => field_items(&c.children, out),
            Inline::Opaque(e) if e.name == "w:fldSimple" => {
                out.push(("fld".into(), "begin".into()));
                out.push(("instr".into(), e.attr("w:instr").unwrap_or_default().to_string()));
                out.push(("fld".into(), "end".into()));
            }
            Inline::Opaque(_) => {}
        }
    }
}

/// A table of contents found in a list of blocks: indices of its first and last paragraphs and its instruction.
pub struct Found {
    pub start: usize,
    pub end: usize,
    pub instr: String,
}

/// Find the `TOC` fields that start and end in this list of blocks (not inside tables).
pub fn find(blocks: &[Block]) -> Vec<Found> {
    find_where(blocks, &|ins| ins.trim_start().to_uppercase().starts_with("TOC"))
}

/// Find the fields that start and end in this list of blocks and whose instruction `want` accepts
/// (the table of contents, a table of figures, an index).
pub fn find_where<S: crate::body::Seq + ?Sized>(blocks: &S, want: &dyn Fn(&str) -> bool) -> Vec<Found> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < blocks.seq_len() {
        let Block::Para(p) = blocks.seq_at(i) else {
            i += 1;
            continue;
        };
        let mut items = Vec::new();
        field_items(&p.inlines, &mut items);
        // A TOC field begins here if a begin is followed by an instruction starting with TOC.
        let mut found = None;
        for (k, (kind, v)) in items.iter().enumerate() {
            if kind == "fld" && v == "begin" {
                let ins: String = items[k + 1..].iter().take_while(|(kk, _)| kk == "instr").map(|(_, v)| v.as_str()).collect();
                if want(&ins) {
                    found = Some((k, ins));
                    break;
                }
            }
        }
        let Some((k, ins)) = found else {
            i += 1;
            continue;
        };
        // Follow field depth from the begin to its end, possibly across paragraphs.
        let mut depth = 0i32;
        let mut end = None;
        let mut j = i;
        let mut from = k;
        'outer: while j < blocks.seq_len() {
            let Block::Para(q) = blocks.seq_at(j) else { break };
            let mut qi = Vec::new();
            field_items(&q.inlines, &mut qi);
            for (kind, v) in qi.iter().skip(from) {
                if kind != "fld" {
                    continue;
                }
                match v.as_str() {
                    "begin" => depth += 1,
                    "end" => {
                        depth -= 1;
                        if depth == 0 {
                            end = Some(j);
                            break 'outer;
                        }
                    }
                    _ => {}
                }
            }
            from = 0;
            j += 1;
        }
        match end {
            Some(e) => {
                out.push(Found { start: i, end: e, instr: ins });
                i = e + 1;
            }
            None => i += 1,
        }
    }
    out
}

/// The name of a `_Toc` bookmark already on this paragraph, if any.
pub fn toc_bookmark(p: &Paragraph) -> Option<String> {
    p.inlines.iter().find_map(|i| match i {
        Inline::Opaque(e) if e.name == "w:bookmarkStart" => e.attr("w:name").filter(|n| n.starts_with("_Toc")).map(String::from),
        _ => None,
    })
}

/// Every bookmark id and name in these blocks (to pick new, unique ones).
pub fn bookmarks(blocks: &[Block], ids: &mut Vec<i64>, names: &mut std::collections::HashSet<String>) {
    fn inl(inlines: &[Inline], ids: &mut Vec<i64>, names: &mut std::collections::HashSet<String>) {
        for i in inlines {
            match i {
                Inline::Opaque(e) if e.name == "w:bookmarkStart" => {
                    if let Some(id) = e.attr("w:id").and_then(|v| v.parse().ok()) {
                        ids.push(id);
                    }
                    if let Some(n) = e.attr("w:name") {
                        names.insert(n.to_string());
                    }
                }
                Inline::Container(c) => inl(&c.children, ids, names),
                _ => {}
            }
        }
    }
    crate::model::walk_paragraphs(blocks, &mut |p| inl(&p.inlines, ids, names));
}
