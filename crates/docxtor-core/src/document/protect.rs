//! Document protection applied to a document: what is protected, the ranges anyone may edit, the ops that start
//! and stop protection, and the check that an edit left the protected content as it was.
//!
//! Enforcement works on the result of an op batch rather than on each op: the text and markup that protection
//! covers are written out before and after, and the batch is undone when they differ. So every op, from the
//! editor, a script or the AI, is held to the same rules.

use super::*;
use crate::controls;
use crate::forms;
use crate::protection::{self, Protection, ProtectionOp, ProtectionSettings};

/// One range Word's `permStart` and `permEnd` mark as editable.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EditRange {
    pub id: String,
    pub group: String,
    pub from: Pos,
    pub to: Pos,
}

/// Everything the editor needs to know about protection.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProtectionInfo {
    #[serde(flatten)]
    pub protection: Protection,
    pub marked_final: bool,
    /// The ids of styles marked locked (`w:locked`): while formatting is limited they can't be applied.
    pub locked_styles: Vec<String>,
    /// The ranges the current user may edit.
    pub ranges: Vec<EditRange>,
    /// Forms protection: paragraphs in sections that are not protected.
    pub open_paragraphs: Vec<NodeId>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum Mode {
    ReadOnly,
    Forms,
}

/// A permStart or permEnd, as the walks below see it.
fn perm(e: &Element) -> Option<(bool, String, String)> {
    let start = match e.name.as_str() {
        "w:permStart" => true,
        "w:permEnd" => false,
        _ => return None,
    };
    let group = e.attr("w:edGrp").map(String::from).or_else(|| e.attr("w:ed").map(|u| format!("user:{u}"))).unwrap_or_else(|| "everyone".into());
    Some((start, e.attr("w:id").unwrap_or("").to_string(), group))
}

impl Document {
    /// Who the user counts as: the groups whose editable ranges they may edit (`everyone` always).
    pub fn set_user_groups(&mut self, groups: Vec<String>) {
        self.user_groups = groups;
    }

    fn in_group(&self, g: &str) -> bool {
        g == "everyone" || self.user_groups.iter().any(|u| u == g)
    }

    pub fn protection(&self) -> Protection {
        protection::read(self.settings.as_ref())
    }

    pub fn is_marked_final(&self) -> bool {
        protection::is_marked_final(self.custom.as_ref())
    }

    /// What is protected and where the editable ranges are.
    pub fn protection_info(&self) -> ProtectionInfo {
        let protection = self.protection();
        let mut ranges = Vec::new();
        let mut open: Vec<(String, String, Pos)> = Vec::new();
        let mut pending_start: Vec<(String, String)> = Vec::new();
        let mut last: Option<Pos> = None;
        fn walk_inlines(inlines: &[Inline], at: &mut usize, pid: NodeId, open: &mut Vec<(String, String, Pos)>, ranges: &mut Vec<(String, String, Pos, Pos)>) {
            for i in inlines {
                match i {
                    Inline::Run(r) => *at += r.len16(),
                    Inline::Container(c) => walk_inlines(&c.children, at, pid, open, ranges),
                    Inline::Opaque(e) => {
                        if let Some((start, id, group)) = perm(e) {
                            let here = Pos::new(pid, *at);
                            if start {
                                open.push((id, group, here));
                            } else if let Some(k) = open.iter().position(|(oid, _, _)| *oid == id) {
                                let (oid, g, from) = open.remove(k);
                                ranges.push((oid, g, from, here));
                            }
                        }
                        *at += i.len16();
                    }
                }
            }
        }
        let mut raw: Vec<(String, String, Pos, Pos)> = Vec::new();
        fn walk_blocks<'a>(bs: impl IntoIterator<Item = &'a Block>, open: &mut Vec<(String, String, Pos)>, pending: &mut Vec<(String, String)>, last: &mut Option<Pos>, raw: &mut Vec<(String, String, Pos, Pos)>) {
            for b in bs {
                match b {
                    Block::Para(p) => {
                        for (id, g) in pending.drain(..) {
                            open.push((id, g, Pos::new(p.id, 0)));
                        }
                        let mut at = 0;
                        walk_inlines(&p.inlines, &mut at, p.id, open, raw);
                        *last = Some(Pos::new(p.id, at));
                    }
                    Block::Opaque(o) => {
                        if let Some((start, id, group)) = perm(&o.el) {
                            if start {
                                pending.push((id, group));
                            } else if let Some(end) = *last {
                                if let Some(k) = open.iter().position(|(oid, _, _)| *oid == id) {
                                    let (oid, g, from) = open.remove(k);
                                    raw.push((oid, g, from, end));
                                }
                            }
                        }
                    }
                    Block::Container(c) => walk_blocks(&c.blocks, open, pending, last, raw),
                    Block::Table(t) => {
                        for r in t.rows() {
                            for c in r.cells() {
                                walk_blocks(&c.blocks, open, pending, last, raw);
                            }
                        }
                    }
                }
            }
        }
        walk_blocks(&self.body, &mut open, &mut pending_start, &mut last, &mut raw);
        for (id, group, from, to) in raw {
            if self.in_group(&group) {
                ranges.push(EditRange { id, group, from, to });
            }
        }
        let locked_styles = self
            .styles
            .doc
            .as_ref()
            .map(|d| d.root.children_named("w:style").filter(|s| s.child("w:locked").is_some_and(|l| !matches!(l.attr("w:val"), Some("0" | "false" | "off")))).filter_map(|s| s.attr("w:styleId")).map(String::from).collect())
            .unwrap_or_default();
        let open_paragraphs = if protection.edit == "forms" { self.unprotected_paragraphs() } else { Vec::new() };
        ProtectionInfo { protection, marked_final: self.is_marked_final(), locked_styles, ranges, open_paragraphs }
    }

    /// Whether each section is protected by forms protection (`w:formProt` is on unless it says otherwise).
    fn section_protected(&self) -> Vec<bool> {
        let mut flags = Vec::new();
        let on = |s: &Element| s.child("w:formProt").is_none_or(|f| !matches!(f.attr("w:val"), Some("0" | "false" | "off")));
        walk_paragraphs(&self.body, &mut |p| {
            if let Some(s) = p.ppr.as_ref().and_then(|pp| pp.child("w:sectPr")) {
                flags.push(on(s));
            }
        });
        let last = self.body.iter().rev().find_map(|b| match b {
            Block::Opaque(o) if o.el.name == "w:sectPr" => Some(on(&o.el)),
            _ => None,
        });
        flags.push(last.unwrap_or(true));
        flags
    }

    fn unprotected_paragraphs(&self) -> Vec<NodeId> {
        let flags = self.section_protected();
        if flags.iter().all(|f| *f) {
            return Vec::new();
        }
        let mut out = Vec::new();
        let mut s = 0usize;
        walk_paragraphs(&self.body, &mut |p| {
            if !flags.get(s).copied().unwrap_or(true) {
                out.push(p.id);
            }
            if p.ppr.as_ref().and_then(|pp| pp.child("w:sectPr")).is_some() {
                s += 1;
            }
        });
        out
    }

    // -----------------------------------------------------------------------
    // Enforcement
    // -----------------------------------------------------------------------

    /// How the document restricts editing right now, if it does.
    pub(super) fn editing_mode(&self) -> Option<Mode> {
        let p = self.protection();
        if !p.restricts_editing() {
            return None;
        }
        Some(if p.edit == "forms" { Mode::Forms } else { Mode::ReadOnly })
    }

    /// What the protection covers, written out: everything outside the places that may be edited.
    pub(super) fn protected_signature(&self, mode: Mode) -> String {
        let flags = if mode == Mode::Forms { self.section_protected() } else { Vec::new() };
        let mut s = Sig { out: String::new(), open: Vec::new(), mode, last_rpr: String::new(), flags, sect: 0, groups: self.user_groups.clone() };
        s.blocks(&self.body, false);
        s.out
    }

    /// The error for an edit that touched protected content.
    pub(super) fn protection_error(&self) -> EditError {
        EditError::Invalid(if self.protection().edit == "forms" {
            "this document is protected: only form fields and content controls can be filled in".into()
        } else {
            "this document is protected: only the editable ranges can be changed".into()
        })
    }

    /// Reject ops that restyle the document or change its pages while editing is restricted (read only, comments,
    /// forms): the signature only covers the body, so styles, the theme, defaults and page setup are checked here.
    pub(super) fn check_layout_allowed(&self, ops: &[Op]) -> Result<(), EditError> {
        if self.editing_mode().is_some() && ops.iter().any(|o| o.edits_styles() || o.changes_page_layout()) {
            return Err(EditError::Invalid("this document is protected: styles, themes and page layout can't be changed".into()));
        }
        Ok(())
    }

    /// Reject formatting ops while formatting is limited to some styles.
    pub(super) fn check_formatting_allowed(&self, ops: &[Op]) -> Result<(), EditError> {
        let p = self.protection();
        if !(p.enforced && p.formatting) {
            return Ok(());
        }
        let locked: Vec<String> = self.protection_info().locked_styles;
        for op in ops {
            if op.edits_styles() {
                return Err(EditError::Invalid("formatting is limited in this document: styles and themes can't be changed".into()));
            }
            if op.class().limited_formatting {
                return Err(EditError::Invalid("formatting is limited in this document: apply one of the allowed styles instead".into()));
            }
            if let Op::SetStyle { style, .. } = op {
                let id = self.styles.lookup(style).unwrap_or_else(|| style.clone());
                if locked.contains(&id) {
                    return Err(EditError::Invalid(format!("the style \"{style}\" isn't allowed in this document")));
                }
            }
        }
        Ok(())
    }

    // -----------------------------------------------------------------------
    // The ops
    // -----------------------------------------------------------------------

    pub(super) fn op_protection(&mut self, op: &ProtectionOp) -> Result<OpResult, EditError> {
        match op {
            ProtectionOp::Set { settings } => {
                if self.protection().enforced {
                    return Err(EditError::Invalid("protection is on: stop it before changing it".into()));
                }
                let mut err = None;
                self.settings_edit(true, &mut |r| {
                    if let Err(e) = protection::apply(r, settings) {
                        err = Some(e);
                    }
                });
                if let Some(e) = err {
                    return Err(EditError::Invalid(e));
                }
                if let Some(allowed) = &settings.allowed_styles {
                    self.lock_styles_except(allowed);
                }
                Ok(OpResult::default())
            }
            ProtectionOp::Stop { password } => {
                let Some(e) = self.settings.as_ref().and_then(|d| d.root.child("w:documentProtection")).cloned() else {
                    return Ok(OpResult::default());
                };
                if !protection::check_password(&e, password.as_deref().unwrap_or("")).map_err(EditError::Invalid)? {
                    return Err(EditError::Invalid("wrong password".into()));
                }
                self.settings_edit(false, &mut |r| {
                    let _ = protection::apply(r, &ProtectionSettings { enforce: Some(false), ..Default::default() });
                });
                Ok(OpResult::default())
            }
            ProtectionOp::MarkFinal { on } => {
                self.ensure_custom_part();
                if let Some(c) = self.custom.as_mut() {
                    let before = c.clone();
                    protection::set_marked_final(c, *on);
                    if *c != before {
                        self.custom_dirty = true;
                    }
                }
                Ok(OpResult::default())
            }
            ProtectionOp::EditableRange { from, to, group, remove } => self.op_editable_range(*from, *to, group.as_deref(), *remove),
        }
    }

    pub(crate) fn ensure_custom_part(&mut self) {
        if self.custom.is_none() {
            self.custom = Some(protection::new_custom_doc());
            self.custom_dirty = true;
        }
    }

    /// Mark every style except `allowed` as locked (and unlock the allowed ones).
    fn lock_styles_except(&mut self, allowed: &[String]) {
        let Some(doc) = self.styles.doc.as_mut() else { return };
        for s in doc.root.elements_mut().filter(|e| e.name == "w:style") {
            let id = s.attr("w:styleId").unwrap_or("").to_string();
            let is_default = s.attr("w:default") == Some("1");
            // Paragraph and character styles are what people choose from; table and list styles are left alone.
            if !matches!(s.attr("w:type"), Some("paragraph" | "character")) {
                continue;
            }
            s.remove_children("w:locked");
            if !allowed.contains(&id) && !is_default {
                let order = ["w:name", "w:aliases", "w:basedOn", "w:next", "w:link", "w:autoRedefine", "w:hidden", "w:uiPriority", "w:semiHidden", "w:unhideWhenUsed", "w:qFormat", "w:locked", "w:personal", "w:personalCompose", "w:personalReply", "w:rsid", "w:pPr", "w:rPr", "w:tblPr", "w:trPr", "w:tcPr", "w:tblStylePr"];
                crate::props::put_ordered(s, Element::new("w:locked"), &order);
            }
        }
        let root = doc.root.clone();
        self.styles.load(&root);
        self.styles.dirty = true;
    }

    fn op_editable_range(&mut self, from: Pos, to: Pos, group: Option<&str>, remove: bool) -> Result<OpResult, EditError> {
        let (from, to) = self.ordered(from, to)?;
        self.touch(from.p.0);
        self.touch(to.p.0);
        if remove {
            let info = self.protection_info();
            let hit: Vec<String> = info
                .ranges
                .iter()
                .filter(|r| {
                    let order = paragraph_order(&self.body);
                    let ix = |p: NodeId| order.iter().position(|x| *x == p).unwrap_or(0);
                    (ix(r.from.p.0), r.from.o) <= (ix(from.p.0), from.o) && (ix(from.p.0), from.o) <= (ix(r.to.p.0), r.to.o)
                })
                .map(|r| r.id.clone())
                .collect();
            if hit.is_empty() {
                return Err(EditError::Invalid("there is no editable range there".into()));
            }
            fn strip(blocks: &mut [Block], ids: &[String]) -> bool {
                fn inl(inlines: &mut Vec<Inline>, ids: &[String]) -> bool {
                    let n = inlines.len();
                    inlines.retain(|i| !matches!(i, Inline::Opaque(e) if perm(e).is_some_and(|(_, id, _)| ids.contains(&id))));
                    let mut any = inlines.len() != n;
                    for i in inlines.iter_mut() {
                        if let Inline::Container(c) = i {
                            any |= inl(&mut c.children, ids);
                        }
                    }
                    any
                }
                let mut any = false;
                for b in blocks.iter_mut() {
                    match b {
                        Block::Para(p) => any |= inl(&mut p.inlines, ids),
                        Block::Container(c) => any |= strip(&mut c.blocks, ids),
                        Block::Table(t) => {
                            for r in t.rows_mut() {
                                for c in r.cells_mut() {
                                    any |= strip(&mut c.blocks, ids);
                                }
                            }
                        }
                        Block::Opaque(_) => {}
                    }
                }
                any
            }
            for id in self.body.edit_each(|s| strip(s, &hit)) {
                self.touch(id);
            }
            self.body.retain(|b| !matches!(b, Block::Opaque(o) if perm(&o.el).is_some_and(|(_, id, _)| hit.contains(&id))));
            return Ok(OpResult::default());
        }
        // A fresh id: larger than any permStart in the document.
        let mut next = 0i64;
        walk_paragraphs(&self.body, &mut |p| {
            fn inl(inlines: &[Inline], next: &mut i64) {
                for i in inlines {
                    match i {
                        Inline::Opaque(e) => {
                            if let Some((_, id, _)) = perm(e) {
                                *next = (*next).max(id.parse().unwrap_or(0));
                            }
                        }
                        Inline::Container(c) => inl(&c.children, next),
                        _ => {}
                    }
                }
            }
            inl(&p.inlines, &mut next);
        });
        let id = (next + 1).max(1_000).to_string();
        let group = group.unwrap_or("everyone");
        let mk = |tag: &'static str| {
            let mut e = Element::new(tag).with_attr("w:id", &id);
            if tag == "w:permStart" {
                e.set_attr("w:edGrp", group);
            }
            e
        };
        // The end goes in first so the start's position is not moved by it.
        {
            let p = self.para_mut(to.p.0)?;
            let off = to.o.min(p.len16());
            edit::insert_inline_at(&mut p.inlines, off, Inline::Opaque(mk("w:permEnd")));
        }
        let p = self.para_mut(from.p.0)?;
        let off = from.o.min(p.len16());
        edit::insert_inline_at(&mut p.inlines, off, Inline::Opaque(mk("w:permStart")));
        Ok(OpResult::default())
    }
}

// ---------------------------------------------------------------------------
// The signature
// ---------------------------------------------------------------------------

struct Sig {
    out: String,
    /// Ids of the editable ranges open at this point (ones the user may edit).
    open: Vec<String>,
    mode: Mode,
    last_rpr: String,
    flags: Vec<bool>,
    sect: usize,
    groups: Vec<String>,
}

impl Sig {
    fn editable_range(&self) -> bool {
        !self.open.is_empty()
    }

    fn blocks<'a>(&mut self, bs: impl IntoIterator<Item = &'a Block>, in_ctl: bool) {
        for b in bs {
            match b {
                Block::Para(p) => self.para(p, in_ctl),
                Block::Table(t) => {
                    self.out.push_str("{tbl");
                    for c in &t.children {
                        match c {
                            TableChild::Other(e) | TableChild::Marker(e, _) => self.out.push_str(&e.to_xml()),
                            TableChild::Row(r) => {
                                self.out.push_str("{r");
                                for rc in &r.children {
                                    match rc {
                                        RowChild::Other(e) | RowChild::Marker(e, _) => self.out.push_str(&e.to_xml()),
                                        RowChild::Cell(c) => {
                                            self.out.push_str("{c");
                                            if let Some(p) = &c.tcpr {
                                                self.out.push_str(&p.to_xml());
                                            }
                                            self.blocks(&c.blocks, in_ctl);
                                            self.out.push('}');
                                        }
                                    }
                                }
                                self.out.push('}');
                            }
                        }
                    }
                    self.out.push('}');
                }
                Block::Container(c) => {
                    let is_ctl = c.shell.child("w:sdtPr").is_some();
                    let editable_ctl = self.mode == Mode::Forms && is_ctl && c.shell.child("w:sdtPr").map(controls::read_props).is_some_and(|v| !matches!(v.kind.as_str(), "citation" | "bibliography" | "docPart"));
                    if !editable_ctl && !self.editable_range() {
                        self.out.push_str(&c.shell.to_xml());
                    }
                    self.blocks(&c.blocks, in_ctl || editable_ctl);
                }
                Block::Opaque(o) => {
                    if let Some((start, id, group)) = perm(&o.el) {
                        self.mark(start, id, group);
                    } else if o.el.name != "w:sectPr" || !self.editable_range() {
                        // Section properties of the whole document stay out: changing the page setup is not editing text.
                        if o.el.name != "w:sectPr" {
                            self.out.push_str(&o.el.to_xml());
                        }
                    }
                }
            }
        }
    }

    fn mark(&mut self, start: bool, id: String, group: String) {
        let mine = group == "everyone" || self.groups.contains(&group);
        if start && mine {
            self.open.push(id);
        } else if !start {
            self.open.retain(|o| *o != id);
        }
    }

    fn para(&mut self, p: &Paragraph, in_ctl: bool) {
        let section_open = self.mode == Mode::Forms && !self.flags.get(self.sect).copied().unwrap_or(true);
        let spans = if self.mode == Mode::Forms { forms::find(&p.inlines) } else { Vec::new() };
        let mut at = 0usize;
        self.inlines(&p.inlines, &mut at, in_ctl || section_open, &spans);
        if !self.editable_range() && !section_open && !in_ctl {
            self.out.push('\u{b6}');
            if let Some(pp) = &p.ppr {
                self.out.push_str(&pp.to_xml());
            }
            self.last_rpr.clear();
        }
        if p.ppr.as_ref().and_then(|pp| pp.child("w:sectPr")).is_some() {
            self.sect += 1;
        }
    }

    fn inlines(&mut self, inlines: &[Inline], at: &mut usize, free: bool, spans: &[forms::Found]) {
        for i in inlines {
            match i {
                Inline::Run(r) => {
                    let rpr = r.rpr.as_ref().map(|x| x.to_xml()).unwrap_or_default();
                    for item in &r.items {
                        let l = item.len16();
                        let in_form = spans.iter().any(|f| *at >= f.at && *at <= f.end);
                        let editable = free || self.editable_range() || in_form;
                        if !editable {
                            if rpr != self.last_rpr {
                                self.out.push_str(&rpr);
                                self.last_rpr = rpr.clone();
                            }
                            match item {
                                RunItem::Text(t) => self.out.push_str(t),
                                RunItem::Tab => self.out.push('\t'),
                                RunItem::Break(e) | RunItem::Hidden(e) => self.out.push_str(&e.to_xml()),
                                RunItem::Object(e) => self.out.push_str(&e.to_xml()),
                            }
                        }
                        *at += l;
                    }
                }
                Inline::Container(c) => {
                    let ctl = c.kind == InlineKind::Sdt && self.mode == Mode::Forms && c.shell.child("w:sdtPr").map(controls::read_props).is_some_and(|v| !matches!(v.kind.as_str(), "citation" | "bibliography"));
                    if !(free || ctl || self.editable_range()) {
                        self.out.push_str(&c.shell.to_xml());
                    }
                    self.inlines(&c.children, at, free || ctl, spans);
                }
                Inline::Opaque(e) => {
                    if let Some((start, id, group)) = perm(e) {
                        self.mark(start, id, group);
                    } else if !(free || self.editable_range() || matches!(e.name.as_str(), "w:bookmarkStart" | "w:bookmarkEnd" | "w:proofErr")) {
                        self.out.push_str(&e.to_xml());
                    }
                    *at += i.len16();
                }
            }
        }
    }
}
