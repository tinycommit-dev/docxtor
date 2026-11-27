//! The editable document tree.
//!
//! The model is deliberately "XML-backed": property blocks (`w:pPr`, `w:rPr`,
//! `w:tblPr`, ...) stay as raw elements that typed accessors read and patch in
//! place, and every construct the editor doesn't model is carried along as an
//! opaque element in its original position. This is what makes import/export
//! lossless for features Docxtor cannot yet display.

use crate::xml::{AttrValue, Element, Name, Node};

pub use crate::body::Body;

pub type NodeId = u32;

/// Placeholder character used in flattened paragraph text for inline objects
/// (images, shapes, footnote references, ...).
pub const OBJ: char = '\u{FFFC}';

#[derive(Debug, Clone, PartialEq)]
pub enum Block {
    Para(Paragraph),
    Table(Table),
    /// `w:sdt` / `w:customXml` at block level: a wrapper whose content is regular blocks.
    Container(BlockContainer),
    /// Anything else (`w:sectPr`, `w:bookmarkStart`, `w:altChunk`, ...).
    Opaque(Opaque),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Paragraph {
    pub id: NodeId,
    pub attrs: Vec<(Name, AttrValue)>,
    pub ppr: Option<Element>,
    pub inlines: Vec<Inline>,
    /// Stands for a display equation (`m:oMathPara`) that Word put directly under `w:body` or a cell, with no
    /// paragraph around it. It is written back that way while it still holds only that equation and no
    /// paragraph properties; once it is anything else it is saved as an ordinary paragraph.
    pub bare_math: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Table {
    pub id: NodeId,
    pub attrs: Vec<(Name, AttrValue)>,
    pub children: Vec<TableChild>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TableChild {
    Row(Row),
    Other(Element),
    /// A range marker (a bookmark, a permission range, ...) that sits inside a content control next to the rows
    /// it wraps. It is written back inside the same control, between the same rows.
    Marker(Element, Vec<SdtWrap>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    pub attrs: Vec<(Name, AttrValue)>,
    pub children: Vec<RowChild>,
    /// The content controls (`w:sdt`) this row sits in, outermost first. Rows next to each other with the same
    /// control are written back inside one control again.
    pub sdts: Vec<SdtWrap>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RowChild {
    Cell(Cell),
    Other(Element),
    /// A range marker inside a content control that wraps cells (see `TableChild::Marker`).
    Marker(Element, Vec<SdtWrap>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Cell {
    pub attrs: Vec<(Name, AttrValue)>,
    pub tcpr: Option<Element>,
    pub blocks: Vec<Block>,
    /// The content controls (`w:sdt`) this cell sits in, outermost first (see `Row::sdts`).
    pub sdts: Vec<SdtWrap>,
}

/// A content control that wraps rows of a table or cells of a row. The model keeps the rows and cells themselves,
/// so they are shown and edited like any other; the wrapper is kept for saving.
#[derive(Debug, Clone, PartialEq)]
pub struct SdtWrap {
    pub id: NodeId,
    /// The `w:sdt` element with its content removed (`w:sdtContent` stays, empty).
    pub shell: Element,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BlockContainer {
    pub id: NodeId,
    /// The wrapper element with its content removed.
    pub shell: Element,
    /// Name of the child of `shell` that holds the content (`w:sdtContent`), or None if direct.
    pub content_in: Option<String>,
    pub blocks: Vec<Block>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Opaque {
    pub id: NodeId,
    pub el: Element,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Inline {
    Run(Run),
    Container(InlineContainer),
    /// Zero-width markers and unsupported inline constructs (bookmarks, proofing marks,
    /// tracked deletions, math, ...). Preserved verbatim.
    Opaque(Element),
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Run {
    pub attrs: Vec<(Name, AttrValue)>,
    /// Attributes of the run's original `w:t` elements (e.g. `xml:space`), reused on write.
    pub t_attrs: Option<Vec<(Name, AttrValue)>>,
    pub rpr: Option<Element>,
    pub items: Vec<RunItem>,
}

impl Run {
    /// A run holding `text` (tabs and line breaks as their own items, see [`text_items`]). Empty text gives one empty
    /// text item, as the places that built such a run by hand did.
    pub fn of_text(rpr: Option<Element>, text: &str) -> Run {
        let mut items = text_items(text);
        if items.is_empty() {
            items.push(RunItem::Text(String::new()));
        }
        Run { attrs: vec![], t_attrs: None, rpr, items }
    }
}

/// The items for some text: a tab becomes a tab item and a line break a break item (a raw tab or newline never
/// belongs inside a text item: the file writer would give it a `<w:tab/>`, but the render model would show the raw
/// character until the file was reopened). Every op that builds a run from text goes through here.
pub fn text_items(text: &str) -> Vec<RunItem> {
    let mut items = Vec::new();
    let mut buf = String::new();
    for ch in text.chars() {
        match ch {
            '\t' => {
                if !buf.is_empty() {
                    items.push(RunItem::Text(std::mem::take(&mut buf)));
                }
                items.push(RunItem::Tab);
            }
            '\n' | '\u{000B}' => {
                if !buf.is_empty() {
                    items.push(RunItem::Text(std::mem::take(&mut buf)));
                }
                items.push(RunItem::Break(Element::new("w:br")));
            }
            '\r' => {}
            c => buf.push(c),
        }
    }
    if !buf.is_empty() {
        items.push(RunItem::Text(buf));
    }
    items
}

#[derive(Debug, Clone, PartialEq)]
pub enum RunItem {
    Text(String),
    Tab,
    /// `w:br` / `w:cr`. The element is kept to preserve `w:type` / `w:clear`.
    Break(Element),
    /// Visible single-character object: drawing, picture, symbol, note reference...
    Object(Element),
    /// Invisible run content: field characters, instruction text, rendered page breaks.
    Hidden(Element),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InlineKind {
    Hyperlink,
    Insertion,
    Field,
    Sdt,
    Other,
}

#[derive(Debug, Clone, PartialEq)]
pub struct InlineContainer {
    pub kind: InlineKind,
    pub shell: Element,
    pub content_in: Option<String>,
    pub children: Vec<Inline>,
}

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct IdGen(pub NodeId);

impl IdGen {
    pub fn fresh(&mut self) -> NodeId {
        self.0 += 1;
        self.0
    }
}

/// How deep tables and content controls sit inside each other at the deepest place, in the weighted levels of
/// `budget::nest_cost` (0 for plain paragraphs). Ops that nest blocks are checked against `budget::NEST_LIMIT`, so a
/// document that was saved can be opened again.
pub fn block_depth<'a>(blocks: impl IntoIterator<Item = &'a Block>) -> usize {
    blocks
        .into_iter()
        .map(|b| match b {
            Block::Table(t) => crate::budget::nest_cost("w:tbl") + t.rows().flat_map(|r| r.cells()).map(|c| block_depth(&c.blocks)).max().unwrap_or(0),
            Block::Container(c) => crate::budget::nest_cost(&c.shell.name) + block_depth(&c.blocks),
            _ => 0,
        })
        .max()
        .unwrap_or(0)
}

pub fn parse_blocks(parent: &Element, ids: &mut IdGen) -> Vec<Block> {
    let mut out = Vec::new();
    for n in &parent.children {
        if let Node::Element(e) = n {
            out.push(parse_block(e, ids));
        }
    }
    out
}

/// `parse_blocks`, taking the children out of `parent` and dropping each one's XML as soon as it is a block, so the
/// tree and the model of a long document are not in memory together.
pub fn parse_blocks_taking(parent: &mut Element, ids: &mut IdGen) -> Vec<Block> {
    let mut out = Vec::new();
    for n in std::mem::take(&mut parent.children) {
        if let Node::Element(e) = n {
            out.push(parse_block(&e, ids));
        }
    }
    out
}

pub fn parse_block(e: &Element, ids: &mut IdGen) -> Block {
    match e.name.as_str() {
        "w:p" => Block::Para(parse_paragraph(e, ids)),
        "w:tbl" => Block::Table(parse_table(e, ids)),
        "w:sdt" if e.child("w:sdtContent").is_some() => {
            let content = e.child("w:sdtContent").unwrap();
            let blocks = parse_blocks(content, ids);
            let mut shell = e.clone();
            if let Some(c) = shell.child_mut("w:sdtContent") {
                c.children.clear();
            }
            Block::Container(BlockContainer { id: ids.fresh(), shell, content_in: Some("w:sdtContent".into()), blocks })
        }
        "w:customXml" => {
            let blocks = parse_blocks(e, ids);
            let mut shell = e.clone();
            shell.children.retain(|n| matches!(n, Node::Element(c) if c.name == "w:customXmlPr"));
            let blocks = blocks
                .into_iter()
                .filter(|b| !matches!(b, Block::Opaque(o) if o.el.name == "w:customXmlPr"))
                .collect();
            Block::Container(BlockContainer { id: ids.fresh(), shell, content_in: None, blocks })
        }
        // An equation on its own, outside any paragraph: given a paragraph to be addressed (and edited) like others.
        "m:oMathPara" => Block::Para(Paragraph { id: ids.fresh(), attrs: Vec::new(), ppr: None, inlines: vec![Inline::Opaque(e.clone())], bare_math: true }),
        _ => Block::Opaque(Opaque { id: ids.fresh(), el: e.clone() }),
    }
}

pub fn parse_paragraph(e: &Element, ids: &mut IdGen) -> Paragraph {
    let mut ppr = None;
    let mut inlines = Vec::new();
    for n in &e.children {
        if let Node::Element(c) = n {
            if c.name == "w:pPr" && ppr.is_none() {
                ppr = Some(c.clone());
            } else {
                inlines.push(parse_inline(c));
            }
        }
    }
    Paragraph { id: ids.fresh(), attrs: e.attrs.clone(), ppr, inlines, bare_math: false }
}

fn container_kind(name: &str) -> Option<InlineKind> {
    Some(match name {
        "w:hyperlink" => InlineKind::Hyperlink,
        "w:ins" | "w:moveTo" => InlineKind::Insertion,
        "w:fldSimple" => InlineKind::Field,
        "w:sdt" => InlineKind::Sdt,
        "w:smartTag" | "w:customXml" | "w:dir" | "w:bdo" => InlineKind::Other,
        _ => return None,
    })
}

pub fn parse_inline(e: &Element) -> Inline {
    if e.name == "w:r" {
        return Inline::Run(parse_run(e));
    }
    if let Some(kind) = container_kind(&e.name) {
        // Property children that must stay with the wrapper rather than become inlines.
        let keep = |n: &str| matches!(n, "w:sdtPr" | "w:sdtEndPr" | "w:smartTagPr" | "w:customXmlPr" | "w:fldData");
        if kind == InlineKind::Sdt {
            if let Some(content) = e.child("w:sdtContent") {
                let children = content.elements().map(parse_inline).collect();
                let mut shell = e.clone();
                if let Some(c) = shell.child_mut("w:sdtContent") {
                    c.children.clear();
                }
                return Inline::Container(InlineContainer { kind, shell, content_in: Some("w:sdtContent".into()), children });
            }
            return Inline::Opaque(e.clone());
        }
        let children = e.elements().filter(|c| !keep(&c.name)).map(parse_inline).collect();
        let mut shell = e.clone();
        shell.children.retain(|n| matches!(n, Node::Element(c) if keep(&c.name)));
        return Inline::Container(InlineContainer { kind, shell, content_in: None, children });
    }
    Inline::Opaque(e.clone())
}

pub fn parse_run(e: &Element) -> Run {
    let mut rpr = None;
    let mut items = Vec::new();
    let mut t_attrs = None;
    for n in &e.children {
        let Node::Element(c) = n else { continue };
        match c.name.as_str() {
            "w:rPr" if rpr.is_none() => rpr = Some(c.clone()),
            "w:t" => {
                // Keep every w:t as its own item (even empty ones) so re-serialization is faithful.
                t_attrs.get_or_insert_with(|| c.attrs.clone());
                let text = c.text();
                if text.contains(['\n', '\r', '\t', '\u{000B}']) {
                    // A raw line feed or tab inside `w:t` is a break or a tab, as the writer will write it (a raw
                    // character never stays inside a text item, see `text_items`).
                    let parts = text_items(&text);
                    if parts.is_empty() {
                        items.push(RunItem::Text(String::new()));
                    }
                    items.extend(parts);
                } else {
                    items.push(RunItem::Text(text));
                }
            }
            "w:tab" => items.push(RunItem::Tab),
            "w:br" | "w:cr" => items.push(RunItem::Break(c.clone())),
            "w:noBreakHyphen" => push_text(&mut items, "\u{2011}"),
            "w:softHyphen" => push_text(&mut items, "\u{00AD}"),
            // A comment's anchor is zero width: comments themselves are not shown yet, but the file keeps them.
            "w:fldChar" | "w:instrText" | "w:delInstrText" | "w:lastRenderedPageBreak" | "w:annotationRef" | "w:separator"
            | "w:continuationSeparator" | "w:delText" | "w:commentReference" => items.push(RunItem::Hidden(c.clone())),
            _ => items.push(RunItem::Object(c.clone())),
        }
    }
    Run { attrs: e.attrs.clone(), t_attrs, rpr, items }
}

fn push_text(items: &mut Vec<RunItem>, s: &str) {
    if let Some(RunItem::Text(prev)) = items.last_mut() {
        prev.push_str(s);
    } else {
        items.push(RunItem::Text(s.into()));
    }
}

/// Elements that only mark a range. A content control around rows or cells may hold them next to the rows or cells.
fn is_range_marker(name: &str) -> bool {
    matches!(
        name,
        "w:bookmarkStart"
            | "w:bookmarkEnd"
            | "w:permStart"
            | "w:permEnd"
            | "w:commentRangeStart"
            | "w:commentRangeEnd"
            | "w:proofErr"
            | "w:moveFromRangeStart"
            | "w:moveFromRangeEnd"
            | "w:moveToRangeStart"
            | "w:moveToRangeEnd"
            | "w:customXmlInsRangeStart"
            | "w:customXmlInsRangeEnd"
            | "w:customXmlDelRangeStart"
            | "w:customXmlDelRangeEnd"
            | "w:customXmlMoveFromRangeStart"
            | "w:customXmlMoveFromRangeEnd"
            | "w:customXmlMoveToRangeStart"
            | "w:customXmlMoveToRangeEnd"
    )
}

/// The `w:sdtContent` of a `w:sdt` when it holds at least one `name` element (or a control around such) and
/// nothing else but range markers. Anything else keeps the control opaque.
fn sdt_content_of<'a>(sdt: &'a Element, name: &str) -> Option<&'a Element> {
    let content = sdt.child("w:sdtContent")?;
    let mut any = false;
    for c in content.elements() {
        if c.name == name || (c.name == "w:sdt" && sdt_content_of(c, name).is_some()) {
            any = true;
        } else if !is_range_marker(&c.name) {
            return None;
        }
    }
    any.then_some(content)
}

fn sdt_shell(sdt: &Element) -> Element {
    let mut shell = sdt.clone();
    if let Some(c) = shell.child_mut("w:sdtContent") {
        c.children.clear();
    }
    shell
}

fn parse_cells(parent: &Element, ids: &mut IdGen, sdts: &[SdtWrap], children: &mut Vec<RowChild>) {
    for rc in parent.elements() {
        match rc.name.as_str() {
            "w:tc" => {
                let mut tcpr = None;
                let mut blocks = Vec::new();
                for cc in rc.elements() {
                    if cc.name == "w:tcPr" && tcpr.is_none() {
                        tcpr = Some(cc.clone());
                    } else {
                        blocks.push(parse_block(cc, ids));
                    }
                }
                children.push(RowChild::Cell(Cell { attrs: rc.attrs.clone(), tcpr, blocks, sdts: sdts.to_vec() }));
            }
            "w:sdt" if sdt_content_of(rc, "w:tc").is_some() => {
                let mut inner = sdts.to_vec();
                inner.push(SdtWrap { id: ids.fresh(), shell: sdt_shell(rc) });
                parse_cells(sdt_content_of(rc, "w:tc").unwrap(), ids, &inner, children);
            }
            n if !sdts.is_empty() && is_range_marker(n) => children.push(RowChild::Marker(rc.clone(), sdts.to_vec())),
            _ => children.push(RowChild::Other(rc.clone())),
        }
    }
}

fn parse_rows(parent: &Element, ids: &mut IdGen, sdts: &[SdtWrap], children: &mut Vec<TableChild>) {
    for c in parent.elements() {
        match c.name.as_str() {
            "w:tr" => {
                let mut row = Row { attrs: c.attrs.clone(), children: Vec::new(), sdts: sdts.to_vec() };
                parse_cells(c, ids, &[], &mut row.children);
                children.push(TableChild::Row(row));
            }
            "w:sdt" if sdt_content_of(c, "w:tr").is_some() => {
                let mut inner = sdts.to_vec();
                inner.push(SdtWrap { id: ids.fresh(), shell: sdt_shell(c) });
                parse_rows(sdt_content_of(c, "w:tr").unwrap(), ids, &inner, children);
            }
            n if !sdts.is_empty() && is_range_marker(n) => children.push(TableChild::Marker(c.clone(), sdts.to_vec())),
            _ => children.push(TableChild::Other(c.clone())),
        }
    }
}

pub fn parse_table(e: &Element, ids: &mut IdGen) -> Table {
    let id = ids.fresh();
    let mut children = Vec::new();
    parse_rows(e, ids, &[], &mut children);
    Table { id, attrs: e.attrs.clone(), children }
}

// ---------------------------------------------------------------------------
// Serialization
// ---------------------------------------------------------------------------

pub fn write_blocks<'a>(blocks: impl IntoIterator<Item = &'a Block>, out: &mut Vec<Node>) {
    for b in blocks {
        out.push(Node::Element(write_block(b)));
    }
}

pub fn write_block(b: &Block) -> Element {
    match b {
        Block::Para(p) => match bare_equation(p) {
            Some(e) => e.clone(),
            None => write_paragraph(p),
        },
        Block::Table(t) => write_table(t),
        // An imported chunk is saved as the element it came from; its blocks are only a view of the chunk's part.
        Block::Container(c) if is_alt_chunk(c) => c.shell.clone(),
        Block::Container(c) => {
            let mut el = c.shell.clone();
            let target = match &c.content_in {
                Some(name) => el.child_mut(name).expect("container content slot"),
                None => &mut el,
            };
            write_blocks(&c.blocks, &mut target.children);
            el
        }
        Block::Opaque(o) => o.el.clone(),
    }
}

/// The equation a paragraph stands for when it is written back as the bare `m:oMathPara` it was read from.
fn bare_equation(p: &Paragraph) -> Option<&Element> {
    match p.inlines.as_slice() {
        [Inline::Opaque(e)] if p.bare_math && p.ppr.is_none() && p.attrs.is_empty() && e.name == "m:oMathPara" => Some(e),
        _ => None,
    }
}

pub fn write_paragraph(p: &Paragraph) -> Element {
    let mut el = Element { name: "w:p".into(), attrs: p.attrs.clone(), children: Vec::new() };
    if let Some(ppr) = &p.ppr {
        el.children.push(Node::Element(ppr.clone()));
    }
    for i in &p.inlines {
        el.children.push(Node::Element(write_inline(i)));
    }
    el
}

pub fn write_inline(i: &Inline) -> Element {
    match i {
        Inline::Run(r) => write_run(r),
        Inline::Container(c) => {
            let mut el = c.shell.clone();
            let target = match &c.content_in {
                Some(name) => el.child_mut(name).expect("container content slot"),
                None => &mut el,
            };
            for ch in &c.children {
                target.children.push(Node::Element(write_inline(ch)));
            }
            el
        }
        Inline::Opaque(e) => e.clone(),
    }
}

/// A `w:r` element holding `text` (a tab as `<w:tab/>`, a line break as `<w:br/>`), for the code that builds generated
/// paragraphs as XML (the table of contents, the index, the table of authorities, a phonetic guide).
pub fn text_run_element(rpr: Option<Element>, text: &str) -> Element {
    write_run(&Run { t_attrs: Some(vec![("xml:space".into(), "preserve".into())]), ..Run::of_text(rpr, text) })
}

pub fn write_run(r: &Run) -> Element {
    let mut el = Element { name: "w:r".into(), attrs: r.attrs.clone(), children: Vec::new() };
    if let Some(rpr) = &r.rpr {
        el.children.push(Node::Element(rpr.clone()));
    }
    for item in &r.items {
        match item {
            RunItem::Text(t) => write_text(t, r.t_attrs.as_deref(), &mut el),
            RunItem::Tab => el.children.push(Node::Element(Element::new("w:tab"))),
            RunItem::Break(e) | RunItem::Hidden(e) => el.children.push(Node::Element(e.clone())),
            // A shape carries an in-memory marker for its text boxes; it is never written.
            RunItem::Object(e) if e.attr(crate::shapes::story::MARK).is_some() => el.children.push(Node::Element(crate::shapes::story::written(e))),
            RunItem::Object(e) => el.children.push(Node::Element(e.clone())),
        }
    }
    el
}

fn write_text(t: &str, t_attrs: Option<&[(Name, AttrValue)]>, run: &mut Element) {
    // Characters that have dedicated elements in WordprocessingML.
    let mut buf = String::new();
    let flush = |buf: &mut String, run: &mut Element, force: bool| {
        if buf.is_empty() && !force {
            return;
        }
        let mut te = Element { name: "w:t".into(), attrs: t_attrs.map(|a| a.to_vec()).unwrap_or_default(), children: Vec::new() };
        if buf.starts_with(char::is_whitespace) || buf.ends_with(char::is_whitespace) || buf.contains("  ") {
            te.set_attr("xml:space", "preserve");
        }
        if !buf.is_empty() {
            te.children.push(Node::Text(std::mem::take(buf)));
        }
        run.children.push(Node::Element(te));
    };
    for ch in t.chars() {
        match ch {
            '\u{2011}' => {
                flush(&mut buf, run, false);
                run.children.push(Node::Element(Element::new("w:noBreakHyphen")));
            }
            '\u{00AD}' => {
                flush(&mut buf, run, false);
                run.children.push(Node::Element(Element::new("w:softHyphen")));
            }
            '\t' => {
                flush(&mut buf, run, false);
                run.children.push(Node::Element(Element::new("w:tab")));
            }
            '\n' => {
                flush(&mut buf, run, false);
                run.children.push(Node::Element(Element::new("w:br")));
            }
            c => buf.push(c),
        }
    }
    // An originally empty <w:t/> is kept as such.
    let force = t.is_empty();
    flush(&mut buf, run, force);
}

/// Write `items` (rows or cells) into `out`, putting runs of items that share a content control back inside it.
fn write_wrapped<T>(items: &[T], sdts_of: &dyn Fn(&T) -> &[SdtWrap], depth: usize, write_one: &mut dyn FnMut(&T, &mut Vec<Node>), out: &mut Vec<Node>) {
    let mut i = 0;
    while i < items.len() {
        match sdts_of(&items[i]).get(depth) {
            None => {
                write_one(&items[i], out);
                i += 1;
            }
            Some(w) => {
                let id = w.id;
                let mut j = i;
                while j < items.len() && sdts_of(&items[j]).get(depth).is_some_and(|x| x.id == id) {
                    j += 1;
                }
                let mut el = w.shell.clone();
                if let Some(content) = el.child_mut("w:sdtContent") {
                    write_wrapped(&items[i..j], sdts_of, depth + 1, write_one, &mut content.children);
                }
                out.push(Node::Element(el));
                i = j;
            }
        }
    }
}

/// Whether a cell's content ends with a paragraph, as a cell must: its last block is one, or is a content control (or
/// another container) that ends with one, as Word writes a control around a cell's text. The writer adds an empty
/// paragraph to a cell that does not, and edits give such a cell its paragraph (`cells_end_in_paragraph`): one rule
/// for both, so what the engine shows is what a save writes.
pub fn ends_in_paragraph(blocks: &[Block]) -> bool {
    match blocks.last() {
        Some(Block::Para(_)) => true,
        Some(Block::Container(c)) => ends_in_paragraph(&c.blocks),
        _ => false,
    }
}

fn write_cell(cell: &Cell, out: &mut Vec<Node>) {
    let mut tc = Element { name: "w:tc".into(), attrs: cell.attrs.clone(), children: Vec::new() };
    if let Some(p) = &cell.tcpr {
        tc.children.push(Node::Element(p.clone()));
    }
    write_blocks(&cell.blocks, &mut tc.children);
    // A cell must end with a paragraph.
    if !ends_in_paragraph(&cell.blocks) {
        tc.children.push(Node::Element(Element::new("w:p")));
    }
    out.push(Node::Element(tc));
}

fn row_child_sdts(c: &RowChild) -> &[SdtWrap] {
    match c {
        RowChild::Cell(c) => &c.sdts,
        RowChild::Marker(_, w) => w,
        RowChild::Other(_) => &[],
    }
}

fn table_child_sdts(c: &TableChild) -> &[SdtWrap] {
    match c {
        TableChild::Row(r) => &r.sdts,
        TableChild::Marker(_, w) => w,
        TableChild::Other(_) => &[],
    }
}

fn write_row(r: &Row, out: &mut Vec<Node>) {
    let mut tr = Element { name: "w:tr".into(), attrs: r.attrs.clone(), children: Vec::new() };
    // Runs of cells (and the range markers among them) in one control go back inside it; other children keep their place.
    let mut at = 0;
    while at < r.children.len() {
        match &r.children[at] {
            RowChild::Other(e) => {
                tr.children.push(Node::Element(e.clone()));
                at += 1;
            }
            RowChild::Cell(_) | RowChild::Marker(..) => {
                let start = at;
                while at < r.children.len() && !matches!(r.children[at], RowChild::Other(_)) {
                    at += 1;
                }
                write_wrapped(
                    &r.children[start..at],
                    &row_child_sdts,
                    0,
                    &mut |c: &RowChild, out| match c {
                        RowChild::Cell(c) => write_cell(c, out),
                        RowChild::Marker(e, _) | RowChild::Other(e) => out.push(Node::Element(e.clone())),
                    },
                    &mut tr.children,
                );
            }
        }
    }
    out.push(Node::Element(tr));
}

pub fn write_table(t: &Table) -> Element {
    let mut el = Element { name: "w:tbl".into(), attrs: t.attrs.clone(), children: Vec::new() };
    let mut at = 0;
    while at < t.children.len() {
        match &t.children[at] {
            TableChild::Other(e) => {
                el.children.push(Node::Element(e.clone()));
                at += 1;
            }
            TableChild::Row(_) | TableChild::Marker(..) => {
                let start = at;
                while at < t.children.len() && !matches!(t.children[at], TableChild::Other(_)) {
                    at += 1;
                }
                write_wrapped(
                    &t.children[start..at],
                    &table_child_sdts,
                    0,
                    &mut |c: &TableChild, out| match c {
                        TableChild::Row(r) => write_row(r, out),
                        TableChild::Marker(e, _) | TableChild::Other(e) => out.push(Node::Element(e.clone())),
                    },
                    &mut el.children,
                );
            }
        }
    }
    el
}

// ---------------------------------------------------------------------------
// Queries
// ---------------------------------------------------------------------------

impl Block {
    pub fn id(&self) -> NodeId {
        match self {
            Block::Para(p) => p.id,
            Block::Table(t) => t.id,
            Block::Container(c) => c.id,
            Block::Opaque(o) => o.id,
        }
    }
}

impl Table {
    pub fn rows(&self) -> impl Iterator<Item = &Row> {
        self.children.iter().filter_map(|c| match c {
            TableChild::Row(r) => Some(r),
            _ => None,
        })
    }
    pub fn rows_mut(&mut self) -> impl Iterator<Item = &mut Row> {
        self.children.iter_mut().filter_map(|c| match c {
            TableChild::Row(r) => Some(r),
            _ => None,
        })
    }
    pub fn tblpr(&self) -> Option<&Element> {
        self.children.iter().find_map(|c| match c {
            TableChild::Other(e) if e.name == "w:tblPr" => Some(e),
            _ => None,
        })
    }
    pub fn grid(&self) -> Vec<i64> {
        self.children
            .iter()
            .find_map(|c| match c {
                TableChild::Other(e) if e.name == "w:tblGrid" => Some(e),
                _ => None,
            })
            .map(|g| g.children_named("w:gridCol").map(|c| c.attr("w:w").and_then(|v| v.parse().ok()).unwrap_or(0)).collect())
            .unwrap_or_default()
    }
}

impl Row {
    pub fn cells(&self) -> impl Iterator<Item = &Cell> {
        self.children.iter().filter_map(|c| match c {
            RowChild::Cell(c) => Some(c),
            _ => None,
        })
    }
    pub fn cells_mut(&mut self) -> impl Iterator<Item = &mut Cell> {
        self.children.iter_mut().filter_map(|c| match c {
            RowChild::Cell(c) => Some(c),
            _ => None,
        })
    }
}

// -- how much memory a copy of the blocks takes -----------------------------------------------------------------
//
// An estimate, near enough to hold the undo history to a budget (see `Document::apply_batch`): the text and names
// they own, plus a fixed amount for each node.

const NODE: usize = 64;

fn attrs_bytes(attrs: &[(Name, AttrValue)]) -> usize {
    attrs.iter().map(|(k, v)| 48 + k.len() + v.len()).sum()
}

fn element_bytes(e: &Element) -> usize {
    NODE + e.name.len() + attrs_bytes(&e.attrs) + e.children.iter().map(node_bytes).sum::<usize>()
}

fn node_bytes(n: &Node) -> usize {
    match n {
        Node::Element(e) => element_bytes(e),
        Node::Text(s) | Node::CData(s) | Node::Comment(s) | Node::PI(s) => NODE + s.len(),
    }
}

/// About how many bytes of memory a parsed XML part takes.
pub fn xml_doc_bytes(d: &crate::xml::XmlDoc) -> usize {
    NODE + d.prolog.iter().map(node_bytes).sum::<usize>() + element_bytes(&d.root)
}

fn opt_bytes(e: &Option<Element>) -> usize {
    e.as_ref().map_or(0, element_bytes)
}

fn inline_bytes(i: &Inline) -> usize {
    match i {
        Inline::Run(r) => {
            let items: usize = r
                .items
                .iter()
                .map(|it| match it {
                    RunItem::Text(s) => NODE + s.len(),
                    RunItem::Tab => NODE,
                    RunItem::Break(e) | RunItem::Object(e) | RunItem::Hidden(e) => NODE + element_bytes(e),
                })
                .sum();
            NODE + attrs_bytes(&r.attrs) + r.t_attrs.as_deref().map_or(0, attrs_bytes) + opt_bytes(&r.rpr) + items
        }
        Inline::Container(c) => NODE + element_bytes(&c.shell) + c.children.iter().map(inline_bytes).sum::<usize>(),
        Inline::Opaque(e) => element_bytes(e),
    }
}

/// About how many bytes of memory these blocks (and everything inside them) take.
pub fn blocks_bytes<'a>(blocks: impl IntoIterator<Item = &'a Block>) -> usize {
    blocks
        .into_iter()
        .map(|b| match b {
            Block::Para(p) => NODE + attrs_bytes(&p.attrs) + opt_bytes(&p.ppr) + p.inlines.iter().map(inline_bytes).sum::<usize>(),
            Block::Table(t) => {
                NODE + attrs_bytes(&t.attrs)
                    + t.children
                        .iter()
                        .map(|c| match c {
                            TableChild::Other(e) | TableChild::Marker(e, _) => element_bytes(e),
                            TableChild::Row(r) => {
                                NODE + attrs_bytes(&r.attrs)
                                    + r.sdts.iter().map(|s| element_bytes(&s.shell)).sum::<usize>()
                                    + r.children
                                        .iter()
                                        .map(|c| match c {
                                            RowChild::Other(e) | RowChild::Marker(e, _) => element_bytes(e),
                                            RowChild::Cell(c) => NODE + attrs_bytes(&c.attrs) + opt_bytes(&c.tcpr) + c.sdts.iter().map(|s| element_bytes(&s.shell)).sum::<usize>() + blocks_bytes(&c.blocks),
                                        })
                                        .sum::<usize>()
                            }
                        })
                        .sum::<usize>()
            }
            Block::Container(c) => NODE + element_bytes(&c.shell) + blocks_bytes(&c.blocks),
            Block::Opaque(o) => NODE + element_bytes(&o.el),
        })
        .sum()
}

/// Visit every paragraph in document order (including inside tables and containers).
pub fn walk_paragraphs<'a>(blocks: impl IntoIterator<Item = &'a Block>, f: &mut dyn FnMut(&'a Paragraph)) {
    for b in blocks {
        match b {
            Block::Para(p) => {
                crate::perf::paragraph_walked();
                f(p)
            }
            Block::Table(t) => {
                for r in t.rows() {
                    for c in r.cells() {
                        walk_paragraphs(&c.blocks, f);
                    }
                }
            }
            Block::Container(c) => walk_paragraphs(&c.blocks, f),
            Block::Opaque(_) => {}
        }
    }
}

/// Whether a container stands for a `w:altChunk` (see `alt_chunk`): foreign content shown in place, saved as it came.
pub fn is_alt_chunk(c: &BlockContainer) -> bool {
    c.shell.name == "w:altChunk"
}

/// Like `walk_paragraphs`, but leaves out imported chunks: their paragraphs use the chunk's own styles and lists.
pub fn walk_paragraphs_local<'a>(blocks: impl IntoIterator<Item = &'a Block>, f: &mut dyn FnMut(&'a Paragraph)) {
    for b in blocks {
        match b {
            Block::Para(p) => {
                crate::perf::paragraph_walked();
                f(p)
            }
            Block::Table(t) => {
                for r in t.rows() {
                    for c in r.cells() {
                        walk_paragraphs_local(&c.blocks, f);
                    }
                }
            }
            Block::Container(c) if is_alt_chunk(c) => {}
            Block::Container(c) => walk_paragraphs_local(&c.blocks, f),
            Block::Opaque(_) => {}
        }
    }
}

pub fn walk_paragraphs_mut(blocks: &mut [Block], f: &mut dyn FnMut(&mut Paragraph)) {
    for b in blocks {
        match b {
            Block::Para(p) => {
                crate::perf::paragraph_walked();
                f(p)
            }
            Block::Table(t) => {
                for r in t.rows_mut() {
                    for c in r.cells_mut() {
                        walk_paragraphs_mut(&mut c.blocks, f);
                    }
                }
            }
            Block::Container(c) => walk_paragraphs_mut(&mut c.blocks, f),
            Block::Opaque(_) => {}
        }
    }
}

/// Visit every XML element the blocks keep (paragraph and run properties, drawings, field characters, wrappers,
/// markers...) and each element's descendants, so something that has to look at all of them (the relationship ids,
/// say) cannot miss a place the model keeps XML in.
pub fn walk_elements_mut(blocks: &mut [Block], f: &mut dyn FnMut(&mut Element)) {
    fn tree(e: &mut Element, f: &mut dyn FnMut(&mut Element)) {
        f(e);
        for n in e.children.iter_mut() {
            if let Node::Element(c) = n {
                tree(c, f);
            }
        }
    }
    fn inlines(list: &mut [Inline], f: &mut dyn FnMut(&mut Element)) {
        for i in list {
            match i {
                Inline::Run(r) => {
                    if let Some(e) = r.rpr.as_mut() {
                        tree(e, f);
                    }
                    for it in r.items.iter_mut() {
                        match it {
                            RunItem::Break(e) | RunItem::Object(e) | RunItem::Hidden(e) => tree(e, f),
                            RunItem::Text(_) | RunItem::Tab => {}
                        }
                    }
                }
                Inline::Container(c) => {
                    tree(&mut c.shell, f);
                    inlines(&mut c.children, f);
                }
                Inline::Opaque(e) => tree(e, f),
            }
        }
    }
    fn wraps(list: &mut [SdtWrap], f: &mut dyn FnMut(&mut Element)) {
        for w in list {
            tree(&mut w.shell, f);
        }
    }
    for b in blocks {
        match b {
            Block::Para(p) => {
                if let Some(e) = p.ppr.as_mut() {
                    tree(e, f);
                }
                inlines(&mut p.inlines, f);
            }
            Block::Table(t) => {
                for c in t.children.iter_mut() {
                    match c {
                        TableChild::Other(e) => tree(e, f),
                        TableChild::Marker(e, w) => {
                            tree(e, f);
                            wraps(w, f);
                        }
                        TableChild::Row(r) => {
                            wraps(&mut r.sdts, f);
                            for rc in r.children.iter_mut() {
                                match rc {
                                    RowChild::Other(e) => tree(e, f),
                                    RowChild::Marker(e, w) => {
                                        tree(e, f);
                                        wraps(w, f);
                                    }
                                    RowChild::Cell(c) => {
                                        wraps(&mut c.sdts, f);
                                        if let Some(e) = c.tcpr.as_mut() {
                                            tree(e, f);
                                        }
                                        walk_elements_mut(&mut c.blocks, f);
                                    }
                                }
                            }
                        }
                    }
                }
            }
            Block::Container(c) => {
                tree(&mut c.shell, f);
                walk_elements_mut(&mut c.blocks, f);
            }
            Block::Opaque(o) => tree(&mut o.el, f),
        }
    }
}

/// Like `walk_elements_mut`, for reading.
///
/// Visit every XML element the blocks keep (paragraph and run properties, drawings, field characters, wrappers,
/// markers...) and each element's descendants, so something that has to look at all of them (the relationship ids,
/// say) cannot miss a place the model keeps XML in.
pub fn walk_elements<'a>(blocks: impl IntoIterator<Item = &'a Block>, f: &mut dyn FnMut(&Element)) {
    fn tree(e: &Element, f: &mut dyn FnMut(&Element)) {
        f(e);
        for n in e.children.iter() {
            if let Node::Element(c) = n {
                tree(c, f);
            }
        }
    }
    fn inlines(list: &[Inline], f: &mut dyn FnMut(&Element)) {
        for i in list {
            match i {
                Inline::Run(r) => {
                    if let Some(e) = r.rpr.as_ref() {
                        tree(e, f);
                    }
                    for it in r.items.iter() {
                        match it {
                            RunItem::Break(e) | RunItem::Object(e) | RunItem::Hidden(e) => tree(e, f),
                            RunItem::Text(_) | RunItem::Tab => {}
                        }
                    }
                }
                Inline::Container(c) => {
                    tree(&c.shell, f);
                    inlines(&c.children, f);
                }
                Inline::Opaque(e) => tree(e, f),
            }
        }
    }
    fn wraps(list: &[SdtWrap], f: &mut dyn FnMut(&Element)) {
        for w in list {
            tree(&w.shell, f);
        }
    }
    for b in blocks {
        match b {
            Block::Para(p) => {
                if let Some(e) = p.ppr.as_ref() {
                    tree(e, f);
                }
                inlines(&p.inlines, f);
            }
            Block::Table(t) => {
                for c in t.children.iter() {
                    match c {
                        TableChild::Other(e) => tree(e, f),
                        TableChild::Marker(e, w) => {
                            tree(e, f);
                            wraps(w, f);
                        }
                        TableChild::Row(r) => {
                            wraps(&r.sdts, f);
                            for rc in r.children.iter() {
                                match rc {
                                    RowChild::Other(e) => tree(e, f),
                                    RowChild::Marker(e, w) => {
                                        tree(e, f);
                                        wraps(w, f);
                                    }
                                    RowChild::Cell(c) => {
                                        wraps(&c.sdts, f);
                                        if let Some(e) = c.tcpr.as_ref() {
                                            tree(e, f);
                                        }
                                        walk_elements(&c.blocks, f);
                                    }
                                }
                            }
                        }
                    }
                }
            }
            Block::Container(c) => {
                tree(&c.shell, f);
                walk_elements(&c.blocks, f);
            }
            Block::Opaque(o) => tree(&o.el, f),
        }
    }
}

/// Locate the block list containing paragraph/block `id`, returning (list, index).
pub fn find_parent_mut(blocks: &mut Vec<Block>, id: NodeId) -> Option<(&mut Vec<Block>, usize)> {
    if let Some(i) = blocks.iter().position(|b| b.id() == id) {
        return Some((blocks, i));
    }
    for b in blocks.iter_mut() {
        let found = parent_in_block(b, id);
        if found.is_some() {
            return found;
        }
    }
    None
}

/// Locate the block list holding `id` strictly inside block `b` (in a cell or container).
fn parent_in_block(b: &mut Block, id: NodeId) -> Option<(&mut Vec<Block>, usize)> {
    match b {
        Block::Table(t) => {
            for r in t.rows_mut() {
                for c in r.cells_mut() {
                    if let Some(x) = find_parent_mut(&mut c.blocks, id) {
                        return Some(x);
                    }
                }
            }
            None
        }
        Block::Container(c) => find_parent_mut(&mut c.blocks, id),
        _ => None,
    }
}

fn parent_in_block_ref(b: &Block, id: NodeId) -> Option<(&[Block], usize)> {
    fn in_list(blocks: &[Block], id: NodeId) -> Option<(&[Block], usize)> {
        if let Some(i) = blocks.iter().position(|b| b.id() == id) {
            return Some((blocks, i));
        }
        blocks.iter().find_map(|b| parent_in_block_ref(b, id))
    }
    match b {
        Block::Table(t) => t.rows().flat_map(|r| r.cells()).find_map(|c| in_list(&c.blocks, id)),
        Block::Container(c) => in_list(&c.blocks, id),
        _ => None,
    }
}

/// A list of blocks found for changing: the top level of the body, or the blocks inside a cell or container.
pub enum ListMut<'a> {
    Top(&'a mut Body),
    Inner(&'a mut Vec<Block>),
}

impl<'a> ListMut<'a> {
    /// The block `i` for changing, for as long as the list is borrowed (the list itself is used up).
    pub fn into_get_mut(self, i: usize) -> Option<&'a mut Block> {
        match self {
            ListMut::Top(b) => b.get_mut(i),
            ListMut::Inner(v) => v.get_mut(i),
        }
    }
    pub fn len(&self) -> usize {
        match self {
            ListMut::Top(b) => b.len(),
            ListMut::Inner(v) => v.len(),
        }
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    pub fn get(&self, i: usize) -> Option<&Block> {
        match self {
            ListMut::Top(b) => b.get(i),
            ListMut::Inner(v) => v.get(i),
        }
    }
    pub fn get_mut(&mut self, i: usize) -> Option<&mut Block> {
        match self {
            ListMut::Top(b) => b.get_mut(i),
            ListMut::Inner(v) => v.get_mut(i),
        }
    }
    pub fn iter(&self) -> Box<dyn DoubleEndedIterator<Item = &Block> + '_> {
        match self {
            ListMut::Top(b) => Box::new(b.iter()),
            ListMut::Inner(v) => Box::new(v.iter()),
        }
    }
    pub fn insert(&mut self, i: usize, blk: Block) {
        match self {
            ListMut::Top(b) => b.insert(i, blk),
            ListMut::Inner(v) => v.insert(i, blk),
        }
    }
    pub fn push(&mut self, blk: Block) {
        match self {
            ListMut::Top(b) => b.push(blk),
            ListMut::Inner(v) => v.push(blk),
        }
    }
    pub fn remove(&mut self, i: usize) -> Block {
        match self {
            ListMut::Top(b) => b.remove(i),
            ListMut::Inner(v) => v.remove(i),
        }
    }
    pub fn splice(&mut self, range: impl std::ops::RangeBounds<usize>, with: impl IntoIterator<Item = Block>) -> Vec<Block> {
        match self {
            ListMut::Top(b) => b.splice(range, with),
            ListMut::Inner(v) => v.splice(range, with).collect(),
        }
    }
    pub fn drain(&mut self, range: impl std::ops::RangeBounds<usize>) -> Vec<Block> {
        match self {
            ListMut::Top(b) => b.drain(range),
            ListMut::Inner(v) => v.drain(range).collect(),
        }
    }
    pub fn retain(&mut self, f: impl FnMut(&Block) -> bool) {
        match self {
            ListMut::Top(b) => b.retain(f),
            ListMut::Inner(v) => {
                let mut f = f;
                v.retain(|b| f(b))
            }
        }
    }
    pub fn truncate(&mut self, n: usize) {
        match self {
            ListMut::Top(b) => b.truncate(n),
            ListMut::Inner(v) => v.truncate(n),
        }
    }
    pub fn swap(&mut self, a: usize, b: usize) {
        match self {
            ListMut::Top(x) => x.swap(a, b),
            ListMut::Inner(v) => v.swap(a, b),
        }
    }
    pub fn extend(&mut self, with: impl IntoIterator<Item = Block>) {
        match self {
            ListMut::Top(b) => b.extend(with),
            ListMut::Inner(v) => v.extend(with),
        }
    }
}

impl std::ops::Index<usize> for ListMut<'_> {
    type Output = Block;
    fn index(&self, i: usize) -> &Block {
        match self {
            ListMut::Top(b) => &b[i],
            ListMut::Inner(v) => &v[i],
        }
    }
}

impl std::ops::IndexMut<usize> for ListMut<'_> {
    fn index_mut(&mut self, i: usize) -> &mut Block {
        match self {
            ListMut::Top(b) => &mut b[i],
            ListMut::Inner(v) => &mut v[i],
        }
    }
}

impl Body {
    /// `find_parent_mut` for the body: only the top-level block holding `id` is copied from an undo step that shares it.
    pub fn parent_mut(&mut self, id: NodeId) -> Option<(ListMut<'_>, usize)> {
        if let Some(i) = self.position(|b| b.id() == id) {
            return Some((ListMut::Top(self), i));
        }
        let t = self.iter().position(|b| find_block(std::slice::from_ref(b), id).is_some())?;
        let (v, i) = parent_in_block(self.get_mut(t)?, id)?;
        Some((ListMut::Inner(v), i))
    }

    /// Where `id` sits, for reading: the list holding it and its place in the list (`parent_mut` without the copy).
    pub fn parent_of(&self, id: NodeId) -> Option<(crate::body::BlocksRef<'_>, usize)> {
        if let Some(i) = self.position(|b| b.id() == id) {
            return Some((crate::body::BlocksRef::Top(self), i));
        }
        let b = self.iter().find(|b| find_block(std::slice::from_ref(*b), id).is_some())?;
        let (v, i) = parent_in_block_ref(b, id)?;
        Some((crate::body::BlocksRef::List(v), i))
    }

    /// `find_para_mut` for the body.
    pub fn para_mut(&mut self, id: NodeId) -> Option<&mut Paragraph> {
        let t = self.iter().position(|b| find_block(std::slice::from_ref(b), id).is_some())?;
        let blk = self.get_mut(t)?;
        if blk.id() == id {
            return match blk {
                Block::Para(p) => Some(p),
                _ => None,
            };
        }
        find_para_mut_in(blk, id)
    }
}

fn find_para_mut_in(b: &mut Block, id: NodeId) -> Option<&mut Paragraph> {
    let (list, i) = parent_in_block(b, id)?;
    match &mut list[i] {
        Block::Para(p) => Some(p),
        _ => None,
    }
}

pub fn find_block<'a>(blocks: impl IntoIterator<Item = &'a Block>, id: NodeId) -> Option<&'a Block> {
    for b in blocks {
        if b.id() == id {
            return Some(b);
        }
        let found = match b {
            Block::Table(t) => t.rows().flat_map(|r| r.cells()).find_map(|c| find_block(&c.blocks, id)),
            Block::Container(c) => find_block(&c.blocks, id),
            _ => None,
        };
        if found.is_some() {
            return found;
        }
    }
    None
}

pub fn find_para<'a>(blocks: impl IntoIterator<Item = &'a Block>, id: NodeId) -> Option<&'a Paragraph> {
    match find_block(blocks, id)? {
        Block::Para(p) => Some(p),
        _ => None,
    }
}

pub fn find_para_mut(blocks: &mut Vec<Block>, id: NodeId) -> Option<&mut Paragraph> {
    let (list, i) = find_parent_mut(blocks, id)?;
    match &mut list[i] {
        Block::Para(p) => Some(p),
        _ => None,
    }
}

/// Top-level block that contains `id` (used to tell the UI what to re-render).
pub fn top_level_of<'a>(blocks: impl IntoIterator<Item = &'a Block>, id: NodeId) -> Option<NodeId> {
    blocks.into_iter().find(|b| b.id() == id || find_block(std::slice::from_ref(*b), id).is_some()).map(|b| b.id())
}

/// Ids of paragraphs in document order.
pub fn paragraph_order<'a>(blocks: impl IntoIterator<Item = &'a Block>) -> Vec<NodeId> {
    let mut v = Vec::new();
    walk_paragraphs(blocks, &mut |p| v.push(p.id));
    v
}

// ---------------------------------------------------------------------------
// Flattened text
// ---------------------------------------------------------------------------

impl RunItem {
    /// Length in UTF-16 code units in the flattened paragraph text. A watermark shape in a header is a
    /// zero-width marker, like a bookmark: it is neither shown as an object nor takes a position.
    pub fn len16(&self) -> usize {
        match self {
            RunItem::Text(t) => t.encode_utf16().count(),
            RunItem::Object(e) if crate::marks::is_watermark_pict(e) => 0,
            RunItem::Tab | RunItem::Break(_) | RunItem::Object(_) => 1,
            RunItem::Hidden(_) => 0,
        }
    }

    pub fn flat(&self) -> String {
        match self {
            RunItem::Text(t) => t.clone(),
            RunItem::Tab => "\t".into(),
            RunItem::Break(_) => "\n".into(),
            RunItem::Object(e) if crate::marks::is_watermark_pict(e) => String::new(),
            RunItem::Object(_) => OBJ.to_string(),
            RunItem::Hidden(_) => String::new(),
        }
    }
}

impl Run {
    pub fn len16(&self) -> usize {
        self.items.iter().map(|i| i.len16()).sum()
    }
    pub fn text(&self) -> String {
        self.items.iter().map(|i| i.flat()).collect()
    }
}

impl Inline {
    pub fn len16(&self) -> usize {
        match self {
            Inline::Run(r) => r.len16(),
            Inline::Container(c) => c.children.iter().map(|i| i.len16()).sum(),
            // An equation is one character, like an image; other opaque inlines are zero-width markers.
            Inline::Opaque(e) => usize::from(crate::math::is_math(e)),
        }
    }
    pub fn text(&self) -> String {
        match self {
            Inline::Run(r) => r.text(),
            Inline::Container(c) => c.children.iter().map(|i| i.text()).collect(),
            Inline::Opaque(e) if crate::math::is_math(e) => OBJ.to_string(),
            Inline::Opaque(_) => String::new(),
        }
    }
}

impl Paragraph {
    pub fn new(id: NodeId) -> Self {
        Paragraph { id, attrs: Vec::new(), ppr: None, inlines: Vec::new(), bare_math: false }
    }
    /// Flattened text: tabs as `\t`, breaks as `\n`, objects as U+FFFC.
    pub fn text(&self) -> String {
        self.inlines.iter().map(|i| i.text()).collect()
    }
    pub fn len16(&self) -> usize {
        self.inlines.iter().map(|i| i.len16()).sum()
    }
    /// Plain text with objects removed (for search/AI).
    pub fn plain_text(&self) -> String {
        self.text().replace(OBJ, "")
    }
}

/// Visit every run in an inline list with its starting UTF-16 offset.
pub fn walk_runs<'a>(inlines: &'a [Inline], start: &mut usize, f: &mut dyn FnMut(&'a Run, usize, &[&'a InlineContainer])) {
    fn go<'a>(
        inlines: &'a [Inline],
        start: &mut usize,
        stack: &mut Vec<&'a InlineContainer>,
        f: &mut dyn FnMut(&'a Run, usize, &[&'a InlineContainer]),
    ) {
        for i in inlines {
            match i {
                Inline::Run(r) => {
                    f(r, *start, stack);
                    *start += r.len16();
                }
                Inline::Container(c) => {
                    stack.push(c);
                    go(&c.children, start, stack, f);
                    stack.pop();
                }
                Inline::Opaque(_) => *start += i.len16(),
            }
        }
    }
    let mut stack = Vec::new();
    go(inlines, start, &mut stack, f);
}
