//! One control as the editor sees it (`Ctl`), and resetting a control to its placeholder.

use super::*;

/// A control found for changing: inline (a container in a paragraph) or block-level.
pub(super) enum Ctl<'a> {
    Inline(&'a mut InlineContainer),
    Block(&'a mut BlockContainer),
}

/// The bookmark starts and ends in these inlines (and the containers in them), to put back after the content is replaced.
pub(super) fn bookmark_markers(inlines: &[Inline], out: &mut Vec<Inline>) {
    for i in inlines {
        match i {
            Inline::Opaque(e) if matches!(e.name.as_str(), "w:bookmarkStart" | "w:bookmarkEnd") => out.push(i.clone()),
            Inline::Container(c) => bookmark_markers(&c.children, out),
            _ => {}
        }
    }
}

impl Ctl<'_> {
    pub(super) fn shell(&mut self) -> &mut Element {
        match self {
            Ctl::Inline(c) => &mut c.shell,
            Ctl::Block(c) => &mut c.shell,
        }
    }

    pub(super) fn pr(&mut self) -> Option<&mut Element> {
        self.shell().child_mut("w:sdtPr")
    }

    pub(super) fn text(&self) -> String {
        match self {
            Ctl::Inline(c) => controls::inline_text(&c.children),
            Ctl::Block(c) => controls::blocks_text(&c.blocks),
        }
    }

    pub(super) fn view(&self) -> RControl {
        let shell = match self {
            Ctl::Inline(c) => &c.shell,
            Ctl::Block(c) => &c.shell,
        };
        shell.child("w:sdtPr").map(controls::read_props).unwrap_or_default()
    }

    /// Replace the content by one run of text. The text looks like what the control held (without the
    /// placeholder style); `font` gives a check box its symbol font.
    pub(super) fn set_text(&mut self, text: &str, placeholder: bool, font: Option<&str>) {
        let pr = match self {
            Ctl::Inline(c) => c.shell.child("w:sdtPr").cloned(),
            Ctl::Block(c) => c.shell.child("w:sdtPr").cloned(),
        };
        match self {
            Ctl::Inline(c) => {
                let mut rpr = controls::content_rpr(&c.children, pr.as_ref());
                if let Some(f) = font {
                    rpr = Some(controls::with_font(rpr, f));
                }
                // A bookmark's start or end inside the content stays, so its other end is not left alone.
                let mut kept = vec![controls::text_run(text, rpr, placeholder)];
                bookmark_markers(&c.children, &mut kept);
                c.children = kept;
            }
            Ctl::Block(c) => {
                let rpr = {
                    let mut found = None;
                    walk_paragraphs(&c.blocks, &mut |p| {
                        if found.is_none() {
                            found = controls::content_rpr(&p.inlines, pr.as_ref());
                        }
                    });
                    found
                };
                let rpr = match font {
                    Some(f) => Some(controls::with_font(rpr, f)),
                    None => rpr,
                };
                // The first paragraph stays (its properties and id); the rest of the content goes.
                let first = c.blocks.iter().position(|b| matches!(b, Block::Para(_)));
                if let Some(i) = first {
                    let mut markers = Vec::new();
                    walk_paragraphs(&c.blocks, &mut |p| bookmark_markers(&p.inlines, &mut markers));
                    let Block::Para(p) = &mut c.blocks[i] else { unreachable!() };
                    p.inlines = vec![controls::text_run(text, rpr, placeholder)];
                    p.inlines.extend(markers);
                    let keep = c.blocks.remove(i);
                    c.blocks.retain(|b| matches!(b, Block::Opaque(_)));
                    c.blocks.insert(0, keep);
                }
            }
        }
    }

    /// Delete `start` UTF-16 units at the start of the content and `end` at its end, leaving the rest as it is. Content
    /// over more than one paragraph is left alone (the caller then sets the text).
    pub(super) fn cut_ends(&mut self, start: usize, end: usize) {
        let inlines = match self {
            Ctl::Inline(c) => &mut c.children,
            Ctl::Block(c) => {
                let mut paras = c.blocks.iter_mut().filter(|b| !matches!(b, Block::Opaque(_)));
                match (paras.next(), paras.next()) {
                    (Some(Block::Para(p)), None) => &mut p.inlines,
                    _ => return,
                }
            }
        };
        let total: usize = inlines.iter().map(|i| i.len16()).sum();
        if start + end > total {
            return;
        }
        crate::edit::delete_range(inlines, total - end, total);
        crate::edit::delete_range(inlines, 0, start);
    }

    pub(super) fn clear_placeholder_style(&mut self) {
        match self {
            Ctl::Inline(c) => controls::clear_placeholder_style(&mut c.children),
            Ctl::Block(c) => controls::blocks_clear_placeholder_style(&mut c.blocks),
        }
    }
}

/// The glossary text of a control's placeholder, else Word's default for its kind.
pub(super) fn placeholder_for(pkg: &crate::package::Package, main: &str, pr: &Element) -> String {
    let kind = controls::read_props(pr).kind;
    controls::placeholder_name(pr)
        .and_then(|n| controls::glossary_text(pkg, main, &n))
        .filter(|t| !t.is_empty())
        .unwrap_or_else(|| default_placeholder(&kind).1.to_string())
}

pub(super) fn default_placeholder(kind: &str) -> (&'static str, &'static str) {
    match kind {
        "date" => controls::PLACEHOLDER_DATE,
        "dropDown" | "comboBox" => controls::PLACEHOLDER_CHOICE,
        "picture" => controls::PLACEHOLDER_PICTURE,
        _ => controls::PLACEHOLDER_TEXT,
    }
}

/// Give every block (and the rows' content controls) new ids, for a copy.
pub(super) fn reid_blocks(blocks: &mut [Block], ids: &mut IdGen) {
    for b in blocks {
        match b {
            Block::Para(p) => p.id = ids.fresh(),
            Block::Container(c) => {
                c.id = ids.fresh();
                reid_blocks(&mut c.blocks, ids);
            }
            Block::Table(t) => {
                t.id = ids.fresh();
                for r in t.rows_mut() {
                    for w in r.sdts.iter_mut() {
                        w.id = ids.fresh();
                    }
                    for c in r.cells_mut() {
                        for w in c.sdts.iter_mut() {
                            w.id = ids.fresh();
                        }
                        reid_blocks(&mut c.blocks, ids);
                    }
                }
            }
            Block::Opaque(o) => o.id = ids.fresh(),
        }
    }
}

/// Put every control in the blocks back to what it shows when empty: its placeholder, an unticked box.
pub(super) fn reset_controls(blocks: &mut [Block], pkg: &crate::package::Package, main: &str) {
    fn inl(inlines: &mut [Inline], pkg: &crate::package::Package, main: &str) {
        for i in inlines.iter_mut() {
            if let Inline::Container(c) = i {
                if c.kind == InlineKind::Sdt {
                    let mut ctl = Ctl::Inline(c);
                    reset_one(&mut ctl, pkg, main);
                    continue;
                }
                inl(&mut c.children, pkg, main);
            }
        }
    }
    for b in blocks.iter_mut() {
        match b {
            Block::Para(p) => inl(&mut p.inlines, pkg, main),
            Block::Container(c) => {
                reset_controls(&mut c.blocks, pkg, main);
                let kind = c.shell.child("w:sdtPr").map(controls::read_props).unwrap_or_default().kind;
                if controls::is_textual(&kind) {
                    reset_one(&mut Ctl::Block(c), pkg, main);
                }
            }
            Block::Table(t) => {
                for r in t.rows_mut() {
                    for c in r.cells_mut() {
                        reset_controls(&mut c.blocks, pkg, main);
                    }
                }
            }
            Block::Opaque(_) => {}
        }
    }
}

pub(super) fn reset_one(ctl: &mut Ctl, pkg: &crate::package::Package, main: &str) {
    let v = ctl.view();
    if v.kind == "checkbox" {
        if let Some(pr) = ctl.pr() {
            controls::set_checked(pr, false);
        }
        let sym = v.unchecked_symbol.clone().unwrap_or(controls::Symbol { val: "2610".into(), font: "MS Gothic".into() });
        ctl.set_text(&sym.ch().to_string(), false, Some(&sym.font));
        return;
    }
    if !controls::is_textual(&v.kind) {
        return;
    }
    let Some(pr) = ctl.pr().map(|p| p.clone()) else { return };
    let text = placeholder_for(pkg, main, &pr);
    if let Some(pr) = ctl.pr() {
        controls::set_showing(pr, true);
        if v.kind == "date" {
            if let Some(d) = pr.child_mut("w:date") {
                d.remove_attr("w:fullDate");
            }
        }
    }
    ctl.set_text(&text, true, None);
}
