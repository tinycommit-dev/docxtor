//! Fields that show a value: dates, document properties, page numbers, `IF`, `SET`, formulas and the like.
//! This is the document side of `field_values`: it gathers what the fields read (properties, bookmarks, tables,
//! pages), updates the results in the text and in headers, footers and notes, and holds the ops that insert a
//! field, change its code and set its result.

use super::*;
use crate::field_fmt::Dt;
use crate::field_values::{self, Env, FieldInput, Grid, Place};
use crate::fields::{self, Instr, Replace};
use crate::formula::{self, Cell};
use std::collections::HashSet;

/// Which fields an update looks at.
pub(super) struct ValueOpts<'a> {
    pub scope: Option<&'a HashSet<NodeId>>,
    pub pages: Option<&'a HashMap<String, u32>>,
    pub input: &'a FieldInput,
    /// As when the file opens: only `DATE`, `TIME` and fields marked dirty.
    pub open: bool,
    /// Only fields with these codes.
    pub only: Option<&'a [String]>,
}

/// A field found in the document, for the field list and for field types that extensions evaluate.
#[derive(Debug, Clone, Serialize)]
pub struct FieldInfo {
    pub p: NodeId,
    pub o: usize,
    pub instr: String,
    pub result: String,
    pub code: String,
    pub locked: bool,
    /// The result is only text, so it can be replaced.
    pub plain: bool,
    /// The field is in a header, footer or note.
    pub story: bool,
}

fn texts_of<'a>(blocks: impl IntoIterator<Item = &'a Block>) -> String {
    let mut out = String::new();
    walk_paragraphs(blocks, &mut |p| {
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(&p.plain_text());
    });
    out
}

fn cell_of(text: &str) -> Cell {
    let t = text.trim();
    if t.is_empty() {
        Cell::Empty
    } else {
        match crate::field_fmt::parse_number(t) {
            Some(n) => Cell::Number(n),
            None => Cell::Text(t.to_string()),
        }
    }
}

/// Where a paragraph sits in the tables of a document: (table, (row, column)).
type CellAt = (usize, (usize, usize));
type CellGrid = Vec<Vec<Cell>>;

impl Document {
    // -----------------------------------------------------------------------
    // What fields read
    // -----------------------------------------------------------------------

    /// Everything fields read from the document, with what the page passed in.
    pub(super) fn field_env(&self, input: Option<&FieldInput>) -> Env {
        let mut env = Env { input: input.cloned().unwrap_or_default(), lang: self.language(), props: self.doc_props(), ..Default::default() };
        if let Some(core) = &self.core {
            for e in core.root.elements() {
                env.core.insert(e.local().to_lowercase(), e.text().trim().to_string());
            }
        }
        // The properties as they are now, not as the file was last saved.
        if let Some(app) = &self.app {
            for e in app.root.elements() {
                env.app
                    .insert(e.local().to_lowercase(), e.text().trim().to_string());
            }
        }
        if let Some(doc) = &self.custom {
            for e in doc.root.elements() {
                if let (Some(name), Some(v)) = (e.attr("name"), e.elements().next()) {
                    env.custom.insert(name.to_lowercase(), v.text().trim().to_string());
                }
            }
        }
        if let Some(vars) = self.settings.as_ref().and_then(|s| s.root.child("w:docVars")) {
            for v in vars.children_named("w:docVar") {
                if let (Some(n), Some(val)) = (v.attr("w:name"), v.attr("w:val")) {
                    env.vars.insert(n.to_lowercase(), val.to_string());
                }
            }
        }
        let text = texts_of(self.main_body());
        env.words = text.split_whitespace().count();
        env.chars = text.chars().filter(|c| !c.is_whitespace()).count();
        env.bookmarks = self.bookmark_infos().into_iter().map(|(k, v)| (k, v.text)).collect();
        env
    }

    /// The tables of the main text as grids of cells, and where each paragraph in one sits (its table, row and
    /// column). A paragraph in a nested table belongs to the inner table.
    fn table_grids(&self) -> (Vec<Grid>, HashMap<NodeId, CellAt>) {
        fn walk<'a>(blocks: impl IntoIterator<Item = &'a Block>, grids: &mut Vec<Grid>, at: &mut HashMap<NodeId, CellAt>) {
            for b in blocks {
                match b {
                    Block::Table(t) => {
                        let gi = grids.len();
                        let mut cells = Vec::new();
                        let mut places: Vec<(NodeId, (usize, usize))> = Vec::new();
                        for (r, row) in t.rows().enumerate() {
                            let mut line = Vec::new();
                            for (c, cell) in row.cells().enumerate() {
                                line.push(cell_of(&texts_of(&cell.blocks)));
                                walk_paragraphs(&cell.blocks, &mut |p| places.push((p.id, (r, c))));
                            }
                            cells.push(line);
                        }
                        grids.push(Grid { cells });
                        for (id, rc) in places {
                            at.insert(id, (gi, rc));
                        }
                        // Inner tables claim their own paragraphs.
                        for row in t.rows() {
                            for cell in row.cells() {
                                walk(&cell.blocks, grids, at);
                            }
                        }
                    }
                    Block::Container(c) => walk(&c.blocks, grids, at),
                    _ => {}
                }
            }
        }
        let mut grids = Vec::new();
        let mut at = HashMap::new();
        walk(self.main_body(), &mut grids, &mut at);
        (grids, at)
    }

    /// The section each paragraph of the main text is in, counted from 1.
    fn section_numbers(&self) -> HashMap<NodeId, usize> {
        let mut out = HashMap::new();
        let mut n = 1;
        walk_paragraphs(self.main_body(), &mut |p| {
            out.insert(p.id, n);
            if p.ppr.as_ref().is_some_and(|pp| pp.child("w:sectPr").is_some()) {
                n += 1;
            }
        });
        out
    }

    // -----------------------------------------------------------------------
    // Updating
    // -----------------------------------------------------------------------

    /// Recompute the fields that show a value. Returns the paragraphs that changed.
    pub(super) fn update_value_fields(&mut self, o: &ValueOpts) -> Vec<NodeId> {
        let mut env = self.field_env(Some(o.input));
        let (grids, cells) = self.table_grids();
        let sections = self.section_numbers();
        let labels = self.page_labels();
        let page_of = o.pages.map(|_| self.page_map(o.pages));
        let mut changed: Vec<NodeId> = Vec::new();
        for _ in 0..3 {
            // `SET` variables, in the order they are set.
            let mut sets = HashMap::new();
            fields::scan_blocks(self.main_body(), &mut |_, s| {
                let i = Instr::parse(s.instr);
                if i.code == "SET" {
                    let w: Vec<&fields::Tok> = i.toks.iter().filter(|t| !t.is_switch()).collect();
                    if w.len() >= 2 {
                        sets.insert(w[0].text.to_lowercase(), fields::unescape(&w[1].text));
                    }
                }
            });
            env.sets = sets;
            let wanted = |code: &str, dirty: bool| -> bool {
                if o.open && !dirty && !matches!(code, "DATE" | "TIME") {
                    return false;
                }
                o.only.is_none_or(|only| only.iter().any(|n| n.eq_ignore_ascii_case(code)))
            };
            let mut body_compute = |pid: NodeId, seen: &fields::Seen| -> Option<Replace> {
                if o.scope.is_some_and(|s| !s.contains(&pid)) {
                    return None;
                }
                let code = Instr::parse(seen.instr).code;
                if !wanted(&code, seen.dirty) {
                    return None;
                }
                let at = cells.get(&pid);
                let place = Place {
                    page: page_of.as_ref().and_then(|m| m.get(&pid)).map(|n| labels.label(pid, *n)),
                    section: sections.get(&pid).copied(),
                    table: at.map(|(g, _)| &grids[*g]),
                    cell: at.map(|(_, rc)| *rc),
                };
                field_values::evaluate(seen.instr, &env, &place)
            };
            let ch = fields::update_blocks_body(&mut self.body, &mut body_compute);
            let mut pass = ch.clone();
            for id in &ch {
                self.touch(*id);
            }
            // Headers, footers and notes: their page numbers come from the layout, so those stay.
            let mut story_compute = |pid: NodeId, seen: &fields::Seen| -> Option<Replace> {
                if o.scope.is_some_and(|s| !s.contains(&pid)) {
                    return None;
                }
                let code = Instr::parse(seen.instr).code;
                if fields::is_page_dependent(&code) || code == "SECTION" || !wanted(&code, seen.dirty) {
                    return None;
                }
                field_values::evaluate(seen.instr, &env, &Place::default())
            };
            for si in 0..self.stories.len() {
                let ch = fields::update_blocks(&mut self.stories[si].blocks, &mut story_compute);
                if !ch.is_empty() {
                    // The steps after the batch run on the blocks of the story that changed (a field result inside a
                    // control that showed its placeholder ends it, as in the body).
                    let tops: std::collections::BTreeSet<NodeId> = ch.iter().filter_map(|id| crate::model::top_level_of(&self.stories[si].blocks, *id)).collect();
                    self.story_edited(si, tops);
                    self.story_gen += 1;
                    self.stories[si].gen = self.story_gen;
                    self.dirty_parts.insert(self.stories[si].part.clone());
                    self.stories_touched = true;
                    pass.extend(ch);
                }
            }
            if pass.is_empty() {
                break;
            }
            changed.extend(pass);
        }
        changed.sort_unstable();
        changed.dedup();
        // Results in the text are written on save; stories were marked above as they changed.
        self.body_dirty |= changed.iter().any(|id| find_block(&self.body, *id).is_some());
        changed
    }

    /// Whether the file asks for its fields to be updated when it opens (`w:updateFields`).
    pub(super) fn updates_fields_on_open(&self) -> bool {
        self.settings.as_ref().and_then(|s| s.root.child("w:updateFields")).is_some_and(|u| !matches!(u.attr("w:val"), Some("0" | "false" | "off")))
    }

    pub(super) fn clear_update_fields_setting(&mut self) {
        self.settings_edit(false, &mut |root| {
            root.remove_children("w:updateFields");
        });
    }

    // -----------------------------------------------------------------------
    // Looking at fields
    // -----------------------------------------------------------------------

    /// Every field in the main text and the stories, in order.
    pub fn list_fields(&self) -> Vec<FieldInfo> {
        let mut out = Vec::new();
        fn add<'a>(out: &mut Vec<FieldInfo>, blocks: impl IntoIterator<Item = &'a Block>, story: bool) {
            fields::scan_blocks(blocks, &mut |p, s| {
                out.push(FieldInfo { p, o: s.offset, instr: s.instr.to_string(), result: s.result.to_string(), code: Instr::parse(s.instr).code, locked: s.locked, plain: s.plain, story });
            });
        }
        add(&mut out, self.main_body(), false);
        for st in &self.stories {
            add(&mut out, &st.blocks, true);
        }
        out
    }

    /// The result a field would have now, for the Insert field dialog. None when it can't be worked out.
    pub fn field_preview(&self, instr: &str, input: Option<&FieldInput>) -> Option<String> {
        let mut env = self.field_env(input);
        fields::scan_blocks(self.main_body(), &mut |_, s| {
            let i = Instr::parse(s.instr);
            if i.code == "SET" {
                let w: Vec<&fields::Tok> = i.toks.iter().filter(|t| !t.is_switch()).collect();
                if w.len() >= 2 {
                    env.sets.insert(w[0].text.to_lowercase(), fields::unescape(&w[1].text));
                }
            }
        });
        field_values::preview(instr, &env, &Place::default())
    }

    // -----------------------------------------------------------------------
    // Ops
    // -----------------------------------------------------------------------

    pub(super) fn op_insert_field(&mut self, at: Pos, field: &str, result: Option<&str>, input: Option<&FieldInput>) -> Result<OpResult, EditError> {
        let trimmed = field.trim();
        let (code, rest) = if let Some(formula) = trimmed.strip_prefix('=') {
            ("=".to_string(), formula.trim().to_string())
        } else {
            match trimmed.split_once(char::is_whitespace) {
                Some((c, r)) => (c.to_ascii_uppercase(), r.trim().to_string()),
                None => (trimmed.to_ascii_uppercase(), String::new()),
            }
        };
        if code.is_empty() || !(code == "=" || code.chars().all(|c| c.is_ascii_alphanumeric())) {
            return Err(EditError::Invalid(format!("invalid field '{field}'")));
        }
        if code == "TOC" {
            return Err(EditError::Invalid("a table of contents is a block of its own: use insert_toc".into()));
        }
        let instr = match (code.as_str(), rest.is_empty()) {
            ("DATE", true) => r#" DATE \@ "MMMM d, yyyy" "#.to_string(),
            ("TIME", true) => r#" TIME \@ "h:mm am/pm" "#.to_string(),
            ("=", _) => format!(" = {rest} "),
            (_, true) => format!(" {code} "),
            _ => format!(" {code} {rest} "),
        };
        let mut evaluated = None;
        let shown = match result {
            Some(r) => r.to_string(),
            None => {
                let mut env = self.field_env(input);
                env.sets.clear();
                // A formula is worked out against the table its cell is in.
                let (grids, cells) = if code == "=" { self.table_grids() } else { Default::default() };
                let in_cell = cells.get(&at.p.0);
                let place = Place {
                    page: Some("1".into()).filter(|_| code == "PAGE"),
                    section: Some(1).filter(|_| code == "SECTION"),
                    table: in_cell.map(|(g, _)| &grids[*g]),
                    cell: in_cell.map(|(_, rc)| *rc),
                };
                evaluated = field_values::evaluate(&instr, &env, &place);
                match &evaluated {
                    Some(r) => r.text.clone(),
                    None if matches!(code.as_str(), "PAGE" | "NUMPAGES" | "SECTIONPAGES" | "SECTION") => "1".into(),
                    None => code.clone(),
                }
            }
        };
        // An equation is drawn only from a complex field, and its result is empty: the drawing is the result.
        let shown = if code == "EQ" && result.is_none() { String::new() } else { shown };
        self.touch(at.p.0);
        let p = self.para_mut(at.p.0)?;
        let off = at.o.min(p.len16());
        let rpr = edit::rpr_at(&p.inlines, off).map(strip_link_style).filter(|r| !r.children.is_empty());
        if code == "EQ" {
            fields::insert_inlines_at(&mut p.inlines, off, fields::complex(&instr, &shown, rpr, false));
            let len = shown.encode_utf16().count();
            return Ok(OpResult { caret: Some(Pos::new(at.p.0, off + len)), ..Default::default() });
        }
        let run = Run::of_text(rpr, &shown);
        let fld = InlineContainer {
            kind: InlineKind::Field,
            shell: Element::new("w:fldSimple").with_attr("w:instr", instr),
            content_in: None,
            children: if shown.is_empty() { vec![] } else { vec![Inline::Run(run)] },
        };
        edit::insert_inline_at(&mut p.inlines, off, Inline::Container(fld));
        let mut len = shown.encode_utf16().count();
        // A field the engine couldn't work out on the spot (a cross-reference, a sequence number) is worked out now,
        // with everything else in its paragraph.
        if evaluated.is_none() && result.is_none() {
            self.refresh_fields_in(&[Id(at.p.0)], input, None);
            if let Ok(p) = self.para_ref(at.p.0) {
                fields::scan_inlines(&p.inlines, 0, &mut |s| {
                    if s.offset == off {
                        len = s.result.encode_utf16().count();
                    }
                });
            }
        }
        Ok(OpResult { caret: Some(Pos::new(at.p.0, off + len)), ..Default::default() })
    }

    /// Change the instruction of the field that starts at `at` and work out its result again.
    pub(super) fn op_set_field_code(&mut self, at: Pos, instr: &str, input: Option<&FieldInput>, pages: Option<&HashMap<String, u32>>) -> Result<OpResult, EditError> {
        let instr = instr.trim();
        if instr.is_empty() {
            return Err(EditError::Invalid("a field needs a code".into()));
        }
        self.touch(at.p.0);
        let found = {
            let p = self.para_mut(at.p.0)?;
            let new_instr = format!(" {instr} ");
            // A table of contents or an index spans paragraphs: its instruction is in the first one.
            fields::set_instr(&mut p.inlines, at.o, 0, &new_instr) || (at.o == 0 && matches!(Instr::parse(&new_instr).code.as_str(), "TOC" | "INDEX") && super::toc_ops::set_list_instr(p, &new_instr))
        };
        if !found {
            return Err(EditError::Invalid("there is no field at that position".into()));
        }
        self.refresh_fields_in(&[Id(at.p.0)], input, pages);
        Ok(OpResult::default())
    }

    /// Update everything in some paragraphs: numbers, references and values.
    pub(super) fn refresh_fields_in(&mut self, scope: &[Id], input: Option<&FieldInput>, pages: Option<&HashMap<String, u32>>) {
        let extra = UpdateExtra { input: input.cloned(), ..Default::default() };
        let _ = self.op_update_fields(pages, Some(scope), &extra);
    }

    /// Replace the plain-text result of the field that starts at `at`.
    pub(super) fn op_set_field_result(&mut self, at: Pos, text: &str) -> Result<OpResult, EditError> {
        self.touch(at.p.0);
        let p = self.para_mut(at.p.0)?;
        let mut found = false;
        fields::update_inlines(&mut p.inlines, 0, &mut |s| {
            if s.offset == at.o && !found {
                found = true;
                Some(Replace::from(text.to_string()))
            } else {
                None
            }
        });
        if !found {
            return Err(EditError::Invalid("there is no field at that position".into()));
        }
        Ok(OpResult::default())
    }

    /// A formula field's own table context, for the table formula feature: the cells of the table holding a
    /// paragraph, and which cell the paragraph is in.
    pub fn formula_context(&self, pid: NodeId) -> Option<(CellGrid, (usize, usize))> {
        let (grids, at) = self.table_grids();
        let (g, rc) = at.get(&pid)?;
        Some((grids[*g].cells.clone(), *rc))
    }

    /// Evaluate a formula body against the table that holds a paragraph (or none), with the document's
    /// bookmarks as names. The result is a plain number or an error message.
    pub fn evaluate_formula(&self, expr: &str, pid: Option<NodeId>) -> Result<f64, String> {
        struct Cx {
            cells: Vec<Vec<Cell>>,
            at: Option<(usize, usize)>,
            names: HashMap<String, String>,
        }
        impl formula::Context for Cx {
            fn cell(&self, r: usize, c: usize) -> Option<Cell> {
                self.cells.get(r)?.get(c).cloned()
            }
            fn position(&self) -> Option<(usize, usize)> {
                self.at
            }
            fn size(&self) -> (usize, usize) {
                (self.cells.len(), self.cells.iter().map(Vec::len).max().unwrap_or(0))
            }
            fn name(&self, name: &str) -> Option<f64> {
                crate::field_fmt::parse_number(self.names.get(&name.to_lowercase())?)
            }
        }
        let (cells, at) = pid.and_then(|p| self.formula_context(p)).map(|(c, a)| (c, Some(a))).unwrap_or_default();
        let names = self.bookmark_infos().into_iter().map(|(k, v)| (k, v.text)).collect();
        formula::evaluate(expr, &Cx { cells, at, names })
    }

    /// The date and time as pictures Word offers for the document's language, with today's text for each.
    pub fn date_format_samples(&self, now: &str) -> Vec<(String, String)> {
        let lang = self.language();
        let dt = Dt::parse_iso(now).unwrap_or(Dt { year: 2000, month: 1, day: 1, hour: 0, minute: 0, second: 0 });
        crate::field_fmt::date_formats(lang.as_deref()).into_iter().map(|p| (p.to_string(), crate::field_fmt::date_picture(&dt, p, lang.as_deref()))).collect()
    }
}

/// What an update gets besides the scope and the page numbers.
#[derive(Default, Clone)]
pub(super) struct UpdateExtra {
    pub open: bool,
    pub only: Option<Vec<String>>,
    pub input: Option<FieldInput>,
}
