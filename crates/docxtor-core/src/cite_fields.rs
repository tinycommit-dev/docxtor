//! Complex fields in the document model, as far as citations need them: finding `CITATION` and
//! `BIBLIOGRAPHY` fields, swapping their cached results, and building the content controls Word wraps
//! them in. Pure functions over the model; `document/cite.rs` applies them to a document.

use crate::cite_styles::Rich;
use crate::model::*;
use crate::props::put_rpr;
use crate::xml::{Element, Node};

/// Where a field character sits: which paragraph of a list (the "unit"), which inline, which run item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Loc {
    pub unit: usize,
    pub inl: usize,
    pub item: usize,
}

/// A complex field at the top level of a list of paragraphs (or of one paragraph's inlines).
#[derive(Debug, Clone)]
pub struct Span {
    pub instr: String,
    pub begin: Loc,
    pub sep: Option<Loc>,
    pub end: Loc,
}

impl Span {
    /// The field's name (`CITATION`, `BIBLIOGRAPHY`, ...), upper case.
    pub fn name(&self) -> String {
        self.instr.split_whitespace().next().unwrap_or("").to_uppercase()
    }
}

/// The top-level complex fields in these units (each the inlines of one paragraph). Fields inside
/// containers (hyperlinks, content controls) are not seen: look into those lists on their own.
pub fn spans(units: &[&[Inline]]) -> Vec<Span> {
    struct Open {
        span: Span,
        depth: usize,
        in_instr: bool,
    }
    let mut stack: Vec<Open> = Vec::new();
    let mut out = Vec::new();
    for (u, inlines) in units.iter().enumerate() {
        for (ii, inl) in inlines.iter().enumerate() {
            let Inline::Run(r) = inl else { continue };
            for (k, item) in r.items.iter().enumerate() {
                let RunItem::Hidden(e) = item else { continue };
                let here = Loc { unit: u, inl: ii, item: k };
                match e.name.as_str() {
                    "w:fldChar" => match e.attr("w:fldCharType") {
                        Some("begin") => {
                            let depth = stack.len();
                            stack.push(Open { span: Span { instr: String::new(), begin: here, sep: None, end: here }, depth, in_instr: true });
                        }
                        Some("separate") => {
                            if let Some(o) = stack.last_mut() {
                                o.span.sep = Some(here);
                                o.in_instr = false;
                            }
                        }
                        Some("end") => {
                            if let Some(mut o) = stack.pop() {
                                o.span.end = here;
                                if o.depth == 0 {
                                    out.push(o.span);
                                }
                            }
                        }
                        _ => {}
                    },
                    "w:instrText" => {
                        if let Some(o) = stack.last_mut() {
                            if o.in_instr {
                                o.span.instr.push_str(&e.text());
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    out
}

/// The instruction text of the first field in a list of inlines (used on a content control's children).
pub fn first_instr(inlines: &[Inline]) -> Option<String> {
    spans(&[inlines]).into_iter().next().map(|s| s.instr)
}

/// A paragraph run holding only a field character.
fn fld_run(kind: &str, rpr: Option<Element>) -> Inline {
    Inline::Run(Run { attrs: vec![], t_attrs: None, rpr, items: vec![RunItem::Hidden(Element::new("w:fldChar").with_attr("w:fldCharType", kind))] })
}

fn instr_run(instr: &str) -> Inline {
    Inline::Run(Run {
        attrs: vec![],
        t_attrs: None,
        rpr: None,
        items: vec![RunItem::Hidden(Element::new("w:instrText").with_attr("xml:space", "preserve").with_text(instr))],
    })
}

fn no_proof() -> Element {
    Element::new("w:rPr").with_child(Element::new("w:noProof"))
}

/// Run properties for one piece of a field result: the base, with the look of the piece set as asked.
fn piece_rpr(base: Option<&Element>, piece: &crate::cite_styles::Span) -> Option<Element> {
    let mut rpr = base.cloned().unwrap_or_else(no_proof);
    for tag in ["w:i", "w:iCs", "w:b", "w:bCs", "w:smallCaps", "w:vertAlign"] {
        rpr.remove_children(tag);
    }
    if piece.italic {
        put_rpr(&mut rpr, Element::new("w:i"));
        put_rpr(&mut rpr, Element::new("w:iCs"));
    }
    if piece.bold {
        put_rpr(&mut rpr, Element::new("w:b"));
        put_rpr(&mut rpr, Element::new("w:bCs"));
    }
    if piece.small_caps {
        put_rpr(&mut rpr, Element::new("w:smallCaps"));
    }
    if piece.sup {
        put_rpr(&mut rpr, Element::new("w:vertAlign").with_attr("w:val", "superscript"));
    }
    if rpr.children.is_empty() {
        None
    } else {
        Some(rpr)
    }
}

/// The runs of a formatted result.
pub fn result_runs(rich: &Rich, base: Option<&Element>) -> Vec<Inline> {
    rich.0
        .iter()
        .filter(|s| !s.text.is_empty())
        .map(|s| Inline::Run(Run::of_text(piece_rpr(base, s), &s.text)))
        .collect()
}

/// A citation as Word writes it: a content control (`w:citation`) around a `CITATION` field.
pub fn citation_sdt(instr: &str, result: &Rich, id: i64, base: Option<&Element>) -> Inline {
    let shell = Element::new("w:sdt")
        .with_child(Element::new("w:sdtPr").with_child(Element::new("w:id").with_attr("w:val", id.to_string())).with_child(Element::new("w:citation")))
        .with_child(Element::new("w:sdtContent"));
    let mut children = vec![fld_run("begin", None), instr_run(instr), fld_run("separate", None)];
    children.extend(result_runs(result, base));
    children.push(fld_run("end", None));
    Inline::Container(InlineContainer { kind: InlineKind::Sdt, shell, content_in: Some("w:sdtContent".into()), children })
}

/// Whether a container is a citation content control.
pub fn is_citation_sdt(c: &InlineContainer) -> bool {
    c.kind == InlineKind::Sdt && c.shell.child("w:sdtPr").is_some_and(|p| p.child("w:citation").is_some())
}

/// The run properties of the first piece of text after the field's separator, to give a new result the same look.
pub fn result_rpr(units: &[&[Inline]], sp: &Span) -> Option<Element> {
    let sep = sp.sep?;
    for (u, list) in units.iter().enumerate().take(sp.end.unit + 1).skip(sep.unit) {
        let from = if u == sep.unit { sep.inl } else { 0 };
        let to = if u == sp.end.unit { sp.end.inl } else { list.len().saturating_sub(1) };
        for (i, inl) in list.iter().enumerate().take(to + 1).skip(from) {
            if let Inline::Run(r) = inl {
                let after_sep = if u == sep.unit && i == sep.inl { sep.item + 1 } else { 0 };
                let has_text = r.items.iter().skip(after_sep).any(|it| matches!(it, RunItem::Text(t) if !t.is_empty()));
                if has_text {
                    return r.rpr.clone();
                }
            }
        }
    }
    None
}

/// Replace the cached result of a field that sits in one list of inlines (`sp.begin.unit == 0`).
pub fn replace_result(inlines: &mut Vec<Inline>, sp: &Span, new: Vec<Inline>) -> bool {
    let Some(sep) = sp.sep else { return false };
    if sep.unit != 0 || sp.end.unit != 0 || sep.inl > sp.end.inl {
        return false;
    }
    let tail = match &inlines[sp.end.inl] {
        Inline::Run(r) => Run { items: r.items[sp.end.item..].to_vec(), ..r.clone() },
        _ => return false,
    };
    match &mut inlines[sep.inl] {
        Inline::Run(r) => r.items.truncate(sep.item + 1),
        _ => return false,
    }
    inlines.splice(sep.inl + 1..=sp.end.inl, new.into_iter().chain(std::iter::once(Inline::Run(tail))));
    true
}

/// A citation's new result, with its new instruction when that changed too.
pub type Rendered = (Option<String>, Rich);

/// The `CITATION` fields of a list of inlines that have a cached result, at the top level.
fn citation_spans(inlines: &[Inline]) -> Vec<Span> {
    spans(&[inlines]).into_iter().filter(|s| s.name() == "CITATION" && s.sep.is_some()).collect()
}

/// How many citation fields these inlines hold, inside containers too.
fn count_citations(inlines: &[Inline]) -> usize {
    let nested: usize = inlines.iter().map(|i| if let Inline::Container(c) = i { count_citations(&c.children) } else { 0 }).sum();
    nested + citation_spans(inlines).len()
}

/// Something that matters for the order of citations in the text: a citation, or a footnote reference
/// (which stands for the note, and the citations in it).
#[derive(Debug, Clone, PartialEq)]
pub enum Mark {
    Cite(String),
    Note(String),
}

/// The citations and footnote references of some inlines in reading order. The citations come in the order
/// `rewrite_citations` numbers them.
pub fn marks_in_order(inlines: &[Inline], out: &mut Vec<Mark>) {
    let mut found: Vec<(usize, Vec<Mark>)> = Vec::new();
    for (i, inl) in inlines.iter().enumerate() {
        match inl {
            Inline::Run(r) => {
                for it in &r.items {
                    if let RunItem::Object(e) = it {
                        if e.name == "w:footnoteReference" {
                            if let Some(id) = e.attr("w:id") {
                                found.push((i, vec![Mark::Note(id.to_string())]));
                            }
                        }
                    }
                }
            }
            Inline::Container(c) => {
                let mut inner = Vec::new();
                marks_in_order(&c.children, &mut inner);
                if !inner.is_empty() {
                    found.push((i, inner));
                }
            }
            Inline::Opaque(_) => {}
        }
    }
    for sp in citation_spans(inlines) {
        found.push((sp.begin.inl, vec![Mark::Cite(sp.instr)]));
    }
    found.sort_by_key(|(i, _)| *i);
    out.extend(found.into_iter().flat_map(|(_, m)| m));
}

/// Rewrite the result of every `CITATION` field in these inlines (and in containers inside them).
/// `render` gets the field's number (counting from `*next`, in reading order) and its instruction, and
/// returns the new result, or None to leave the field as it is. Returns how many fields were rewritten.
pub fn rewrite_citations(inlines: &mut Vec<Inline>, next: &mut usize, render: &mut dyn FnMut(usize, &str) -> Option<Rendered>) -> usize {
    enum Item {
        Container(usize),
        Field(Span),
    }
    let mut items: Vec<(usize, Item)> = Vec::new();
    for (i, inl) in inlines.iter().enumerate() {
        if let Inline::Container(_) = inl {
            items.push((i, Item::Container(i)));
        }
    }
    for sp in citation_spans(inlines) {
        items.push((sp.begin.inl, Item::Field(sp)));
    }
    items.sort_by_key(|(i, _)| *i);
    // Number them in reading order first.
    let mut starts = Vec::with_capacity(items.len());
    for (_, it) in &items {
        starts.push(*next);
        *next += match it {
            Item::Container(i) => match &inlines[*i] {
                Inline::Container(c) => count_citations(&c.children),
                _ => 0,
            },
            Item::Field(_) => 1,
        };
    }
    // Then rewrite from the end, so what is spliced in does not move what comes before it.
    let mut n = 0;
    for ((_, it), start) in items.into_iter().zip(starts).rev() {
        match it {
            Item::Container(i) => {
                let mut inner = start;
                if let Inline::Container(c) = &mut inlines[i] {
                    n += rewrite_citations(&mut c.children, &mut inner, render);
                }
            }
            Item::Field(sp) => {
                let Some((new_instr, rich)) = render(start, &sp.instr) else { continue };
                let base = result_rpr(&[inlines.as_slice()], &sp);
                let runs = result_runs(&rich, base.as_ref().or(Some(&no_proof())));
                if replace_result(inlines, &sp, runs) {
                    n += 1;
                }
                if let Some(text) = new_instr {
                    set_instr(inlines, &sp, &text);
                }
            }
        }
    }
    n
}

/// Put new text in the instruction of a field found by `spans` (before its result was replaced, the locations
/// of the instruction are still valid, since only what follows the separator moved).
fn set_instr(inlines: &mut [Inline], sp: &Span, text: &str) {
    let mut done = false;
    let last = sp.sep.map_or(sp.end.inl, |s| s.inl);
    for inl in inlines.iter_mut().take(last + 1).skip(sp.begin.inl) {
        if let Inline::Run(r) = inl {
            for it in r.items.iter_mut() {
                if let RunItem::Hidden(e) = it {
                    if e.name == "w:instrText" {
                        if done {
                            // Instructions split over several elements: the first holds the whole text now.
                            *e = Element::new("w:instrText").with_attr("xml:space", "preserve");
                        } else {
                            *e = Element::new("w:instrText").with_attr("xml:space", "preserve").with_text(text);
                            done = true;
                        }
                    }
                }
            }
        }
    }
}

/// Replace the instruction of the first `CITATION` field in a content control's children.
pub fn set_first_instr(children: &mut [Inline], text: &str) -> bool {
    let sp = spans(&[&*children]).into_iter().next();
    match sp {
        Some(sp) => {
            set_instr(children, &sp, text);
            true
        }
        None => false,
    }
}

/// The inlines of a field result as plain runs (for unwrapping a citation whose source is gone).
pub fn result_inlines(children: &[Inline]) -> Vec<Inline> {
    let Some(sp) = spans(&[children]).into_iter().next() else { return Vec::new() };
    let Some(sep) = sp.sep else { return Vec::new() };
    let mut out = Vec::new();
    for (i, inl) in children.iter().enumerate().take(sp.end.inl + 1).skip(sep.inl) {
        if let Inline::Run(r) = inl {
            let from = if i == sep.inl { sep.item + 1 } else { 0 };
            let to = if i == sp.end.inl { sp.end.item } else { r.items.len() };
            let items: Vec<RunItem> = r.items[from.min(to)..to].iter().filter(|it| !matches!(it, RunItem::Hidden(_))).cloned().collect();
            if !items.is_empty() {
                out.push(Inline::Run(Run { items, ..r.clone() }));
            }
        } else {
            out.push(inl.clone());
        }
    }
    out
}

// ---------------------------------------------------------------------------
// The bibliography
// ---------------------------------------------------------------------------

/// A bibliography field found among the blocks of a list: indices of its first and last paragraph.
pub struct BibField {
    pub first: usize,
    pub last: usize,
    pub span: Span,
}

fn para_units<'a>(blocks: impl IntoIterator<Item = &'a Block>) -> (Vec<usize>, Vec<&'a [Inline]>) {
    let mut idx = Vec::new();
    let mut units = Vec::new();
    for (i, b) in blocks.into_iter().enumerate() {
        if let Block::Para(p) = b {
            idx.push(i);
            units.push(p.inlines.as_slice());
        }
    }
    (idx, units)
}

/// The `BIBLIOGRAPHY` fields that start and end in this list (not inside tables), whose paragraphs are
/// next to each other.
pub fn find_bibliographies<'a>(blocks: impl IntoIterator<Item = &'a Block>) -> Vec<BibField> {
    let (idx, units) = para_units(blocks);
    spans(&units)
        .into_iter()
        .filter(|s| s.name() == "BIBLIOGRAPHY" && s.sep.is_some())
        .filter_map(|s| {
            let (first, last) = (idx[s.begin.unit], idx[s.end.unit]);
            // Only a field whose paragraphs are consecutive blocks can be rewritten in place.
            (last - first == s.end.unit - s.begin.unit).then_some(BibField { first, last, span: s })
        })
        .collect()
}

/// Whether any list (or container inside it) holds a bibliography field.
pub fn has_bibliography<'a, I: IntoIterator<Item = &'a Block> + Copy>(blocks: I) -> bool {
    !find_bibliographies(blocks).is_empty()
        || blocks.into_iter().any(|b| match b {
            Block::Container(c) => has_bibliography(&c.blocks),
            _ => false,
        })
}

/// The paragraph properties of the entries: the style, a hanging indent, the paragraph mark's `noProof`.
pub fn entry_ppr(indent: Option<(i64, i64)>, style: &str) -> Element {
    let mut ppr = Element::new("w:pPr").with_child(Element::new("w:pStyle").with_attr("w:val", style));
    if let Some((left, hanging)) = indent {
        ppr = ppr.with_child(Element::new("w:ind").with_attr("w:left", left.to_string()).with_attr("w:hanging", hanging.to_string()));
    }
    ppr.with_child(no_proof())
}

/// Paragraph properties with the entries' hanging indent set.
fn with_indent(ppr: Option<Element>, indent: (i64, i64)) -> Option<Element> {
    let mut ppr = ppr.unwrap_or_else(|| Element::new("w:pPr"));
    let ind = Element::new("w:ind").with_attr("w:left", indent.0.to_string()).with_attr("w:hanging", indent.1.to_string());
    crate::props::put_ppr(&mut ppr, ind);
    Some(ppr)
}

/// Replace the entries of every bibliography field in `blocks` (and in containers inside it). The first
/// entry shares the paragraph of the field's start, the others get paragraphs of their own, and the end
/// of the field stays in a last paragraph, as in Word. Returns how many fields were rewritten.
pub fn rewrite_bibliographies(blocks: &mut Vec<Block>, entries: &[Rich], indent: (i64, i64), ids: &mut IdGen) -> usize {
    let mut done = 0;
    for b in blocks.iter_mut() {
        if let Block::Container(c) = b {
            done += rewrite_bibliographies(&mut c.blocks, entries, indent, ids);
        }
    }
    for f in find_bibliographies(&*blocks).into_iter().rev() {
        let (units_idx, units) = para_units(&*blocks);
        let _ = units_idx;
        let base = result_rpr(&units, &f.span);
        let sep = f.span.sep.expect("filtered on a separator");
        let end = f.span.end;
        let (Block::Para(first), Block::Para(last)) = (&blocks[f.first], &blocks[f.last]) else { continue };
        // The start paragraph keeps everything up to the separator; the last one keeps the end of the field and what follows.
        let ppr = with_indent(first.ppr.clone(), indent);
        let mut head = first.clone();
        head.ppr = ppr.clone();
        head.inlines.truncate(sep.inl + 1);
        if let Some(Inline::Run(r)) = head.inlines.get_mut(sep.inl) {
            r.items.truncate(sep.item + 1);
        }
        let tail_inlines: Vec<Inline> = match &last.inlines[end.inl] {
            Inline::Run(r) => {
                let mut v = vec![Inline::Run(Run { items: r.items[end.item..].to_vec(), ..r.clone() })];
                v.extend(last.inlines[end.inl + 1..].iter().cloned());
                v
            }
            _ => return done,
        };
        let mut tail = if f.first == f.last { Paragraph { id: ids.fresh(), attrs: vec![], ppr: ppr.clone(), inlines: vec![], bare_math: false } } else { last.clone() };
        tail.inlines = tail_inlines;
        let mut new_blocks: Vec<Block> = Vec::new();
        for (i, e) in entries.iter().enumerate() {
            let runs = result_runs(e, base.as_ref().or(Some(&no_proof())));
            if i == 0 {
                head.inlines.extend(runs);
            } else {
                let id = ids.fresh();
                new_blocks.push(Block::Para(Paragraph { id, attrs: vec![], ppr: ppr.clone(), inlines: runs, bare_math: false }));
            }
        }
        let mut replacement = vec![Block::Para(head)];
        replacement.extend(new_blocks);
        replacement.push(Block::Para(tail));
        blocks.splice(f.first..=f.last, replacement);
        done += 1;
    }
    done
}

/// A new bibliography content control with Word's structure and no entries yet:
/// an optional heading, a paragraph with the start of the field and a paragraph that ends it.
pub fn new_bibliography(id: i64, heading: Option<&str>, entry_ppr: &Element) -> Element {
    let mut content = Element::new("w:sdtContent");
    if let Some(h) = heading.filter(|h| !h.trim().is_empty()) {
        content = content.with_child(
            Element::new("w:p")
                .with_child(Element::new("w:pPr").with_child(Element::new("w:pStyle").with_attr("w:val", "Heading1")))
                .with_child(crate::model::text_run_element(None, h)),
        );
    }
    let hidden = |kind: &str| Element::new("w:r").with_child(Element::new("w:fldChar").with_attr("w:fldCharType", kind));
    let begin = Element::new("w:p")
        .with_child(entry_ppr.clone())
        .with_child(hidden("begin"))
        .with_child(Element::new("w:r").with_child(Element::new("w:instrText").with_attr("xml:space", "preserve").with_text(" BIBLIOGRAPHY ")))
        .with_child(hidden("separate"));
    let end = Element::new("w:p").with_child(entry_ppr.clone()).with_child(hidden("end"));
    content.children.push(Node::Element(begin));
    content.children.push(Node::Element(end));
    Element::new("w:sdt")
        .with_child(
            Element::new("w:sdtPr").with_child(Element::new("w:id").with_attr("w:val", id.to_string())).with_child(
                Element::new("w:docPartObj").with_child(Element::new("w:docPartGallery").with_attr("w:val", "Bibliographies")).with_child(Element::new("w:docPartUnique")),
            ),
        )
        .with_child(content)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text_run(t: &str) -> Inline {
        Inline::Run(Run { items: vec![RunItem::Text(t.into())], ..Default::default() })
    }

    #[test]
    fn finds_and_replaces_a_result() {
        let mut inlines = vec![text_run("See "), citation_sdt(" CITATION Smi20 \\l 1033 ", &Rich::plain_of("(old)"), 7, None), text_run(".")];
        let mut seen = Vec::new();
        let n = rewrite_citations(&mut inlines, &mut 0, &mut |_, instr| {
            seen.push(instr.to_string());
            let mut r = Rich::new();
            r.t("(").i("new").t(")");
            Some((None, r))
        });
        assert_eq!(n, 1);
        assert_eq!(seen, [" CITATION Smi20 \\l 1033 "]);
        let Inline::Container(c) = &inlines[1] else { panic!() };
        assert!(is_citation_sdt(c));
        let xml = write_inline(&inlines[1]).to_xml();
        assert!(xml.contains("<w:t>(</w:t>") && xml.contains("<w:i/>") && xml.contains("<w:t>new</w:t>"), "{xml}");
        assert!(xml.contains("fldCharType=\"end\""), "{xml}");
        assert!(!xml.contains("old"), "{xml}");
        assert_eq!(inlines[1].text(), "(new)");
    }

    #[test]
    fn nested_fields_are_part_of_the_outer_one() {
        let hidden = |e: Element| Inline::Run(Run { items: vec![RunItem::Hidden(e)], ..Default::default() });
        let f = |k: &str| hidden(Element::new("w:fldChar").with_attr("w:fldCharType", k));
        let list = vec![f("begin"), hidden(Element::new("w:instrText").with_text(" IF ")), f("begin"), hidden(Element::new("w:instrText").with_text(" PAGE ")), f("separate"), text_run("1"), f("end"), f("separate"), text_run("x"), f("end")];
        let s = spans(&[&list]);
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].name(), "IF");
        assert_eq!(s[0].end.inl, 9);
    }

    #[test]
    fn bibliography_rewrite_keeps_the_field_around_the_entries() {
        let mut ids = IdGen(100);
        let ppr = entry_ppr(Some((720, 720)), "Bibliography");
        let el = new_bibliography(5, Some("References"), &ppr);
        let mut blocks = vec![parse_block(&el, &mut ids)];
        let mut one = Rich::new();
        one.t("Smith, J. (2020). ").i("A book").t(".");
        let two = Rich::plain_of("Zed, Z. (2021). Another.");
        assert_eq!(rewrite_bibliographies(&mut blocks, &[one.clone(), two.clone()], (720, 720), &mut ids), 1);
        let Block::Container(c) = &blocks[0] else { panic!() };
        let texts: Vec<String> = c.blocks.iter().map(|b| if let Block::Para(p) = b { p.plain_text() } else { String::new() }).collect();
        assert_eq!(texts, ["References", "Smith, J. (2020). A book.", "Zed, Z. (2021). Another.", ""]);
        // A second rewrite replaces the entries.
        rewrite_bibliographies(&mut blocks, &[two], (720, 720), &mut ids);
        let Block::Container(c) = &blocks[0] else { panic!() };
        let texts: Vec<String> = c.blocks.iter().map(|b| if let Block::Para(p) = b { p.plain_text() } else { String::new() }).collect();
        assert_eq!(texts, ["References", "Zed, Z. (2021). Another.", ""]);
        let xml = write_block(&blocks[0]).to_xml();
        assert!(xml.contains("BIBLIOGRAPHY") && xml.contains("docPartGallery"), "{xml}");
        // The field is still one field: begin once, end once.
        assert_eq!(xml.matches("fldCharType=\"begin\"").count(), 1);
        assert_eq!(xml.matches("fldCharType=\"end\"").count(), 1);
        // No entries at all leaves the field empty but intact.
        rewrite_bibliographies(&mut blocks, &[], (720, 720), &mut ids);
        let Block::Container(c) = &blocks[0] else { panic!() };
        assert_eq!(c.blocks.len(), 3);
    }
}
