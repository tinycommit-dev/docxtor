//! Text boxes as stories.
//!
//! Each `w:txbxContent` of a shape becomes a `Story` of kind `TextBox`, so its paragraphs have ids in the
//! document's id space and every op that targets one is routed into it, exactly as for a header or a footnote.
//! The story's `part` is the part holding the shape (the main document, or a header), and its `note_id` is
//! `tb<key>`.
//!
//! The shape's own element (the `RunItem::Object`) keeps the markup as it was read. To find the shape a story
//! belongs to, the element carries an in-memory attribute (`data-docxtor-tb`) listing the keys of its text boxes
//! in document order; the writer drops it (see `model::write_run`), so saved XML never contains it. When a
//! story's content changes, `Document` writes the new content back into the `w:txbxContent` of the shape's
//! `mc:Choice` and into the copy in its `mc:Fallback`, and nothing else.

use crate::model::*;
use crate::stories::{Story, StoryKind};
use crate::xml::{Element, Node};
use std::collections::HashSet;

/// In-memory marker on a shape's object element: the keys of its text-box stories, separated by spaces.
pub const MARK: &str = "data-docxtor-tb";

pub fn note_id(key: &str) -> String {
    format!("tb{key}")
}

/// The key of a text-box story.
pub fn key_of(story: &Story) -> Option<&str> {
    if story.kind != StoryKind::TextBox {
        return None;
    }
    story.note_id.as_deref()?.strip_prefix("tb")
}

/// The keys an object element carries, in document order.
pub fn keys_of(e: &Element) -> Vec<String> {
    e.attr(MARK).map(|v| v.split_whitespace().map(String::from).collect()).unwrap_or_default()
}

/// The element as it is written: without the marker.
pub fn written(e: &Element) -> Element {
    let mut c = e.clone();
    c.remove_attr(MARK);
    c
}

/// The element holding the shape's primary markup (inside the `mc:Choice` that has it) and the one holding
/// its fallback copy, if any.
pub fn roots(e: &Element) -> Option<(&Element, Option<&Element>)> {
    super::read::primary(e)?;
    if e.name == "mc:AlternateContent" {
        let choice = e.children_named("mc:Choice").find(|c| super::read::primary_in_choice(c).is_some())?;
        return Some((choice, e.child("mc:Fallback")));
    }
    Some((e, None))
}

fn collect<'a>(e: &'a Element, out: &mut Vec<&'a Element>) {
    for c in e.elements() {
        if c.name == "w:txbxContent" {
            out.push(c);
        } else {
            collect(c, out);
        }
    }
}

fn collect_mut<'a>(e: &'a mut Element, out: &mut Vec<&'a mut Element>) {
    for c in e.elements_mut() {
        if c.name == "w:txbxContent" {
            out.push(c);
        } else {
            collect_mut(c, out);
        }
    }
}

/// The `w:txbxContent` elements of a shape's primary markup, in document order.
pub fn contents(e: &Element) -> Vec<&Element> {
    let mut out = Vec::new();
    if let Some((p, _)) = roots(e) {
        collect(p, &mut out);
    }
    out
}

/// Write `content` into text box number `index` of the shape: the primary markup and, when the fallback has
/// as many text boxes, its copy. Returns whether anything changed.
pub fn set_content(e: &mut Element, index: usize, content: &[Node]) -> bool {
    if roots(e).is_none() {
        return false;
    }
    let mut changed = false;
    let is_ac = e.name == "mc:AlternateContent";
    let mut apply = |root: &mut Element, require: Option<usize>| {
        let mut list = Vec::new();
        collect_mut(root, &mut list);
        if require.is_some_and(|n| n != list.len()) {
            return 0;
        }
        let n = list.len();
        if let Some(t) = list.into_iter().nth(index) {
            if t.children != content {
                t.children = content.to_vec();
                changed = true;
            }
        }
        n
    };
    if is_ac {
        let (mut pi, mut fi) = (None, None);
        for (i, n) in e.children.iter().enumerate() {
            if let Node::Element(c) = n {
                if c.name == "mc:Choice" && pi.is_none() && super::read::primary_in_choice(c).is_some() {
                    pi = Some(i);
                } else if c.name == "mc:Fallback" && fi.is_none() {
                    fi = Some(i);
                }
            }
        }
        let mut count = 0;
        if let Some(Node::Element(c)) = pi.map(|i| &mut e.children[i]) {
            count = apply(c, None);
        }
        if let Some(Node::Element(f)) = fi.map(|i| &mut e.children[i]) {
            apply(f, Some(count));
            // WordArt: the fallback is VML that keeps the text in `v:textpath`, so it says the same.
            if index == 0 {
                changed |= set_textpath(f, content);
            }
        }
    } else {
        apply(e, None);
    }
    changed
}

/// Put the text of a WordArt text box (`content`: its paragraphs) in the `string` of the fallback's `v:textpath`.
fn set_textpath(fallback: &mut Element, content: &[Node]) -> bool {
    let Some(tp) = super::patch::find_mut(fallback, "v:textpath") else { return false };
    if tp.attr("string").is_none() {
        return false;
    }
    let lines: Vec<String> = content
        .iter()
        .filter_map(|n| match n {
            Node::Element(p) if p.name == "w:p" => {
                let mut ts = Vec::new();
                p.find_all("w:t", &mut ts);
                Some(ts.iter().map(|t| t.text()).collect::<String>())
            }
            _ => None,
        })
        .collect();
    let text = lines.join("\n");
    if tp.attr("string") == Some(text.as_str()) {
        return false;
    }
    tp.set_attr("string", text);
    true
}

// ---------------------------------------------------------------------------
// Walking objects
// ---------------------------------------------------------------------------

fn objects_in<'a>(inlines: &'a [Inline], f: &mut dyn FnMut(&'a Element)) {
    for i in inlines {
        match i {
            Inline::Run(r) => {
                for it in &r.items {
                    if let RunItem::Object(e) = it {
                        f(e);
                    }
                }
            }
            Inline::Container(c) => objects_in(&c.children, f),
            Inline::Opaque(_) => {}
        }
    }
}

fn objects_in_mut(inlines: &mut [Inline], f: &mut dyn FnMut(&mut Element)) {
    for i in inlines {
        match i {
            Inline::Run(r) => {
                for it in &mut r.items {
                    if let RunItem::Object(e) = it {
                        f(e);
                    }
                }
            }
            Inline::Container(c) => objects_in_mut(&mut c.children, f),
            Inline::Opaque(_) => {}
        }
    }
}

/// Every object element in the blocks (tables and containers included) with the id of its paragraph.
pub fn walk_objects<'a>(blocks: impl IntoIterator<Item = &'a Block>, f: &mut dyn FnMut(NodeId, &'a Element)) {
    walk_paragraphs(blocks, &mut |p| objects_in(&p.inlines, &mut |e| f(p.id, e)));
}

pub fn walk_objects_mut(blocks: &mut [Block], f: &mut dyn FnMut(NodeId, &mut Element)) {
    walk_paragraphs_mut(blocks, &mut |p| {
        let id = p.id;
        objects_in_mut(&mut p.inlines, &mut |e| f(id, e));
    });
}

/// `walk_objects_mut` for the body: `f` says whether it changed the object. A block an undo step shares is walked on a
/// scratch copy that goes in only when something in it changed. Returns the ids of the top-level blocks that changed.
pub fn walk_objects_edit_body(body: &mut crate::model::Body, f: &mut dyn FnMut(NodeId, &mut Element) -> bool) -> Vec<NodeId> {
    body.edit_each(|s| {
        let mut any = false;
        walk_objects_mut(s, &mut |pid, e| any |= f(pid, e));
        any
    })
}

/// Whether any object in the blocks is a shape.
pub fn has_shapes(blocks: &[Block]) -> bool {
    let mut any = false;
    walk_objects(blocks, &mut |_, e| any |= super::is_shape(e));
    any
}

// ---------------------------------------------------------------------------
// Loading and finding stories
// ---------------------------------------------------------------------------

/// Give every text box in the body and in the stories a story of its own (`TextBox`), and mark its shape.
/// Text boxes inside text boxes get theirs too.
pub fn load(main_path: &str, body: &mut [Block], stories: &mut Vec<Story>, ids: &mut IdGen) {
    load_in(main_path, Some(body), stories, ids);
}

/// `load`, leaving the body alone (`None`) when it is known to hold no shape (see `Document::body_has_shapes`).
pub fn load_in(main_path: &str, body: Option<&mut [Block]>, stories: &mut Vec<Story>, ids: &mut IdGen) {
    let mut found: Vec<Story> = Vec::new();
    if let Some(body) = body {
        register(body, main_path, ids, &mut found);
    }
    load_rest(stories, ids, found);
}

/// The stories' own text boxes, then the text boxes inside the ones found.
fn load_rest(stories: &mut Vec<Story>, ids: &mut IdGen, mut found: Vec<Story>) {
    for s in stories.iter_mut() {
        let part = s.part.clone();
        register(&mut s.blocks, &part, ids, &mut found);
    }
    // Text boxes inside the text boxes just found.
    let mut from = 0;
    while from < found.len() {
        let mut more = Vec::new();
        for s in found.iter_mut().skip(from) {
            let part = s.part.clone();
            register(&mut s.blocks, &part, ids, &mut more);
        }
        from = found.len();
        found.extend(more);
    }
    stories.extend(found);
}

/// `load_in` for the body. Only the blocks that hold a shape without a story are changed (and copied from an undo step).
pub fn load_in_body(main_path: &str, body: Option<&mut crate::model::Body>, stories: &mut Vec<Story>, ids: &mut IdGen) {
    let mut found: Vec<Story> = Vec::new();
    if let Some(body) = body {
        for i in 0..body.len() {
            let mut any = false;
            walk_objects(std::slice::from_ref(&body[i]), &mut |_, e| any |= needs_story(e));
            if any {
                register(std::slice::from_mut(&mut body[i]), main_path, ids, &mut found);
            }
        }
    }
    load_rest(stories, ids, found);
}

/// Whether `new_stories` would make stories for this object.
fn needs_story(e: &Element) -> bool {
    e.attr(MARK).is_none() && super::read::primary(e).is_some() && !contents(e).is_empty()
}

fn register(blocks: &mut [Block], part: &str, ids: &mut IdGen, out: &mut Vec<Story>) {
    walk_objects_mut(blocks, &mut |_, e| {
        if let Some(stories) = new_stories(e, part, ids) {
            out.extend(stories);
        }
    });
}

/// Make stories for the text boxes of a shape that has none yet, and mark the element.
pub fn new_stories(e: &mut Element, part: &str, ids: &mut IdGen) -> Option<Vec<Story>> {
    if e.attr(MARK).is_some() || super::read::primary(e).is_none() {
        return None;
    }
    let n = contents(e).len();
    if n == 0 {
        return None;
    }
    let parsed: Vec<Vec<Block>> = contents(e)
        .into_iter()
        .map(|t| {
            let mut blocks = parse_blocks(t, ids);
            if !blocks.iter().any(|b| matches!(b, Block::Para(_) | Block::Table(_) | Block::Container(_))) {
                blocks.insert(0, Block::Para(Paragraph::new(ids.fresh())));
            }
            blocks
        })
        .collect();
    let mut keys = Vec::new();
    let mut out = Vec::new();
    for blocks in parsed {
        let key = ids.fresh().to_string();
        out.push(Story { kind: StoryKind::TextBox, part: part.to_string(), note_id: Some(note_id(&key)), blocks, gen: 0 });
        keys.push(key);
    }
    e.set_attr(MARK, keys.join(" "));
    Some(out)
}

/// Keys of the text boxes that some shape in these blocks still holds.
pub fn live_keys<'a>(blocks: impl IntoIterator<Item = &'a Block>, out: &mut HashSet<String>) {
    walk_objects(blocks, &mut |_, e| {
        if e.attr(MARK).is_some() {
            out.extend(keys_of(e));
        }
    });
}

/// `write_back` for the body: only the blocks that hold a shape with the story are changed (and copied from an undo step).
pub fn write_back_body(body: &mut crate::model::Body, key: &str, content: &[Node]) -> (Vec<NodeId>, bool) {
    let (mut owners, mut changed) = (Vec::new(), false);
    for i in 0..body.len() {
        let mut holds = false;
        walk_objects(std::slice::from_ref(&body[i]), &mut |_, e| holds |= keys_of(e).iter().any(|k| k == key));
        if holds {
            let (o, c) = write_back(std::slice::from_mut(&mut body[i]), key, content);
            owners.extend(o);
            changed |= c;
        }
    }
    (owners, changed)
}

/// Write the text of story `key` into every shape holding it in these blocks. Returns the paragraphs that
/// hold such a shape, and whether any markup changed.
pub fn write_back(blocks: &mut [Block], key: &str, content: &[Node]) -> (Vec<NodeId>, bool) {
    let mut owners = Vec::new();
    let mut changed = false;
    walk_objects_mut(blocks, &mut |pid, e| {
        if let Some(i) = keys_of(e).iter().position(|k| k == key) {
            owners.push(pid);
            changed |= set_content(e, i, content);
        }
    });
    (owners, changed)
}
