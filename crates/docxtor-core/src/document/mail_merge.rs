//! Mail merge, envelopes and labels on the document: the ops that start a merge, set its recipients, insert merge
//! fields, make a sheet of labels or an envelope; previewing a record in place; and merging into a new document.
//! The merging itself is `merge::run`, a pure function of the template and a record; this is what holds the state.

use super::*;
use crate::fields;
use crate::merge::address::{self, Recipient, STANDARD_FIELDS};
use crate::merge::envelope::{self, EnvelopeLayout};
use crate::merge::instr::{self, MergeFieldSpec};
use crate::merge::labels::{self, LabelFill, LabelSpec, TextLook};
use crate::merge::run::{self, Answers, Merger, Prompt};
use crate::merge::state::{self, SettingsInfo};
use crate::merge::{FilterRule, MailMerge, MailMergeDef, SortKey, SourceInfo, Table, MAIN_TYPES};
use serde::Serialize;
use std::collections::BTreeMap;
use std::sync::Arc;

/// The template, parked while a merged record is shown in its place.
#[derive(Clone)]
pub(super) struct Previewing {
    /// The template's blocks.
    pub body: Body,
    /// Which record (its position in the list) is shown.
    pub position: usize,
}

/// What the interface needs to know about the merge.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MergeInfo {
    /// The document is a mail merge main document (it has `w:mailMerge`).
    pub active: bool,
    /// `formLetters`, `mailingLabels`, `envelopes`, `catalog`, `email` or `fax`.
    pub main_type: String,
    /// The recipients' file, as the settings or the document's own copy name it.
    pub source_name: Option<String>,
    /// Docxtor holds the recipients (not only a link to a file Word wrote).
    pub has_recipients: bool,
    pub source: Option<SourceInfo>,
    pub columns: Vec<String>,
    /// Records in the list.
    pub records: usize,
    /// Records in the merge: ticked and passing the filter.
    pub included: usize,
    /// The position of the record shown in place of the template, when one is.
    pub previewing: Option<usize>,
    pub mapping: BTreeMap<String, String>,
    /// Word shows merged results when the file opens (`w:viewMergedData`).
    pub view_merged: bool,
}

/// The recipients, for the list editor.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MergeData {
    pub source: SourceInfo,
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
    /// A tick for each record.
    pub active: Vec<bool>,
    pub filter: Vec<FilterRule>,
    pub sort: Vec<SortKey>,
    pub mapping: BTreeMap<String, String>,
    /// The records in the merge, in order, as indexes into `rows`.
    pub order: Vec<usize>,
    /// For each of Word's address fields, the column that stands for it now (the mapping or a guess).
    pub matched: Vec<(String, Option<String>)>,
}

/// What a preview or a merge found.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewInfo {
    pub position: usize,
    pub count: usize,
    /// How many records the copy used (more than one for labels).
    pub used: usize,
    /// A rule leaves this record out of the merge.
    pub skipped: bool,
}

/// What a merge into a new document did.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MergeResult {
    pub records: usize,
    pub copies: usize,
    pub skipped: usize,
}

/// Which records to merge: all of them or a range (positions in the list, from 0, to is exclusive).
#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct MergeRange {
    #[serde(default)]
    pub from: Option<usize>,
    #[serde(default)]
    pub to: Option<usize>,
}

/// Content and closing section properties of a body.
fn split_tail(body: &Body) -> (Vec<Block>, Vec<Block>) {
    let n = body.iter().rposition(|b| !matches!(b, Block::Opaque(o) if o.el.name == "w:sectPr")).map(|i| i + 1).unwrap_or(0);
    (body.iter().take(n).cloned().collect(), body.iter().skip(n).cloned().collect())
}

fn text_look_rpr(rpr: Option<Element>) -> Option<Element> {
    let mut r = rpr.unwrap_or_else(|| Element::new("w:rPr"));
    crate::props::put_rpr(&mut r, Element::new("w:noProof"));
    Some(r)
}

impl Document {
    // -----------------------------------------------------------------------
    // Looking at the merge
    // -----------------------------------------------------------------------

    /// The template's blocks: the document's own, or the ones parked while a record is previewed.
    fn template_body(&self) -> &Body {
        self.merge_preview.as_ref().map(|p| &p.body).unwrap_or(&self.body)
    }

    fn merge_settings(&self) -> Option<(SettingsInfo, Element)> {
        let el = self.settings.as_ref()?.root.child("w:mailMerge")?.clone();
        let rels = self.pkg.rels_for(&self.settings_path).unwrap_or_default();
        let info = state::read_settings(&el, &|id| rels.iter().find(|r| r.id == id).map(|r| r.target.clone()));
        Some((info, el))
    }

    pub fn merge_info(&self) -> MergeInfo {
        let settings = self.merge_settings();
        let cfg = self.merge.cfg.clone();
        let mapping = cfg.as_ref().map(|c| c.mapping.clone()).or_else(|| settings.as_ref().map(|(s, _)| s.mapping.clone())).unwrap_or_default();
        MergeInfo {
            active: settings.is_some(),
            main_type: settings.as_ref().map(|(s, _)| s.main_type.clone()).unwrap_or_else(|| "formLetters".into()),
            source_name: cfg.as_ref().map(|c| c.source.name.clone()).filter(|n| !n.is_empty()).or_else(|| settings.as_ref().and_then(|(s, _)| s.source_name.clone())),
            has_recipients: cfg.is_some(),
            source: cfg.as_ref().map(|c| c.source.clone()),
            columns: cfg.as_ref().map(|c| c.table.columns.clone()).unwrap_or_default(),
            records: cfg.as_ref().map(|c| c.table.rows.len()).unwrap_or(0),
            included: cfg.as_ref().map(|c| c.order().len()).unwrap_or(0),
            previewing: self.merge_preview.as_ref().map(|p| p.position),
            mapping,
            view_merged: settings.as_ref().is_some_and(|(s, _)| s.view_merged),
        }
    }

    /// The recipients, with the ticks, filter and sort.
    pub fn merge_data(&self) -> Option<MergeData> {
        let cfg = self.merge.cfg.as_ref()?;
        let matched = STANDARD_FIELDS.iter().map(|(n, _)| (n.to_string(), address::standard_column(cfg, n).and_then(|c| cfg.table.columns.get(c).cloned()))).collect();
        Some(MergeData {
            source: cfg.source.clone(),
            columns: cfg.table.columns.clone(),
            rows: cfg.table.rows.clone(),
            active: (0..cfg.table.rows.len()).map(|i| cfg.is_active(i)).collect(),
            filter: cfg.filter.clone(),
            sort: cfg.sort.clone(),
            mapping: cfg.mapping.clone(),
            order: cfg.order(),
            matched,
        })
    }

    /// The records that pass a filter, in the order a sort puts them, whatever the ticks say: what the recipient list shows while
    /// a filter or a sort is being edited. Indexes into the table.
    pub fn merge_order(&self, filter: &[FilterRule], sort: &[SortKey]) -> Vec<usize> {
        let Some(cfg) = self.merge.cfg.as_ref() else { return Vec::new() };
        let mut c = (**cfg).clone();
        c.active.clear();
        c.filter = filter.to_vec();
        c.sort = sort.to_vec();
        c.order()
    }

    /// The questions the merge will ask.
    pub fn merge_prompts(&self) -> Vec<Prompt> {
        run::prompts(self.template_body())
    }

    /// Positions in the list of the records that have `query` in a column (or in `field`'s).
    pub fn merge_find(&self, query: &str, field: Option<&str>) -> Vec<usize> {
        let Some(cfg) = self.merge.cfg.as_ref() else { return Vec::new() };
        let q = query.trim().to_lowercase();
        if q.is_empty() {
            return Vec::new();
        }
        let only = field.and_then(|f| cfg.column(f));
        cfg.order()
            .iter()
            .enumerate()
            .filter(|(_, &row)| {
                let r = &cfg.table.rows[row];
                match only {
                    Some(c) => r[c].to_lowercase().contains(&q),
                    None => r.iter().any(|v| v.to_lowercase().contains(&q)),
                }
            })
            .map(|(pos, _)| pos)
            .collect()
    }

    /// What a field would show for a record, for the address block and greeting line dialogs: the lines of the address,
    /// the greeting, or a column's value. Empty when it can't be worked out.
    pub fn merge_field_preview(&self, instr_text: &str, position: usize) -> String {
        let Some(cfg) = self.merge.cfg.as_ref() else { return String::new() };
        let order = cfg.order();
        let r = Recipient { mm: cfg, row: order.get(position).copied() };
        let i = crate::fields::Instr::parse(instr_text);
        match i.code.as_str() {
            "ADDRESSBLOCK" => address::address_block(&r, &i),
            "GREETINGLINE" => address::greeting_line(&r, &i),
            "MERGEFIELD" => instr::field_name(&i).and_then(|n| r.column(&n).map(String::from).or_else(|| Some(r.field(&n)))).unwrap_or_default(),
            _ => String::new(),
        }
    }

    // -----------------------------------------------------------------------
    // Ops
    // -----------------------------------------------------------------------

    pub(super) fn apply_merge_op(&mut self, op: &Op) -> Result<OpResult, EditError> {
        match op {
            Op::SetMailMerge { merge } => self.op_set_mail_merge(merge.as_ref()),
            Op::SetMergeRecipients { active, filter, sort, mapping } => self.op_set_merge_recipients(active.as_deref(), filter.as_deref(), sort.as_deref(), mapping.as_ref()),
            Op::InsertMergeField { at, field } => self.op_insert_merge_field(*at, field),
            Op::UpdateLabels { at } => self.op_update_labels(at.map(|i| i.0)),
            Op::MakeLabels { spec, fill, look } => self.op_make_labels(spec, fill, look),
            Op::InsertEnvelope { size, width, height, layout, replace, address_block } => {
                let size = envelope::size_of(size, *width, *height).map_err(EditError::Invalid)?;
                self.op_insert_envelope(&size, layout, *replace, *address_block)
            }
            _ => Err(EditError::Invalid("not a mail merge op".into())),
        }
    }

    fn set_settings_merge(&mut self, main_type: Option<&str>, cfg: Option<&MailMerge>) {
        let path = self.settings_path.clone();
        let Some(main_type) = main_type else {
            self.settings_edit(false, &mut |root| {
                root.remove_children("w:mailMerge");
            });
            return;
        };
        let rid = cfg.filter(|c| matches!(c.source.kind.as_str(), "csv" | "xlsx") && !c.source.name.is_empty()).map(|c| self.pkg.add_rel(&path, state::REL_MAIL_MERGE_SOURCE, &state::file_target(&c.source.name), true));
        let rid_kept = rid.clone();
        self.settings_edit(true, &mut |root| {
            if root.attr("xmlns:r").is_none() {
                root.set_attr("xmlns:r", NS_R);
            }
            // Changing only the document type keeps the data source the file already has.
            let existing = root.child("w:mailMerge").cloned();
            let el = match (&existing, cfg) {
                (Some(e), None) if e.child("w:dataSource").is_some() => {
                    let mut e = e.clone();
                    crate::props::put_ordered(&mut e, Element::new("w:mainDocumentType").with_attr("w:val", main_type), &["w:mainDocumentType"]);
                    e
                }
                _ => state::mail_merge_element(existing.as_ref(), main_type, cfg, rid_kept.as_deref()),
            };
            crate::props::put_ordered(root, el, SETTINGS_ORDER);
        });
    }

    pub(super) fn op_set_mail_merge(&mut self, def: Option<&MailMergeDef>) -> Result<OpResult, EditError> {
        let Some(def) = def else {
            self.set_settings_merge(None, None);
            self.merge.set(None);
            return Ok(OpResult::default());
        };
        if !MAIN_TYPES.contains(&def.main_type.as_str()) {
            return Err(EditError::Invalid(format!("unknown mail merge type '{}'; use one of {}", def.main_type, MAIN_TYPES.join(", "))));
        }
        let keep = def.source.is_none() && def.columns.is_empty();
        let cfg: Option<MailMerge> = if keep {
            self.merge.cfg.as_ref().map(|c| (**c).clone())
        } else {
            if def.columns.is_empty() {
                return Err(EditError::Invalid("The recipients need at least one column.".into()));
            }
            if def.rows.len() > 200_000 {
                return Err(EditError::Invalid("The list has more than 200,000 recipients.".into()));
            }
            let table = Table::from_rows(def.columns.clone(), def.rows.clone());
            let active = match &def.active {
                Some(a) if a.len() == table.rows.len() && a.iter().any(|x| !x) => a.clone(),
                _ => Vec::new(),
            };
            Some(MailMerge { source: def.source.clone().unwrap_or_default(), table: Arc::new(table), active, filter: def.filter.clone(), sort: def.sort.clone(), mapping: def.mapping.clone() })
        };
        self.set_settings_merge(Some(&def.main_type), cfg.as_ref());
        if !keep {
            self.merge.set(cfg);
        }
        Ok(OpResult::default())
    }

    pub(super) fn op_set_merge_recipients(&mut self, active: Option<&[bool]>, filter: Option<&[FilterRule]>, sort: Option<&[SortKey]>, mapping: Option<&BTreeMap<String, String>>) -> Result<OpResult, EditError> {
        let Some(cur) = self.merge.cfg.clone() else {
            return Err(EditError::Invalid("Choose the recipients first.".into()));
        };
        let mut cfg = (*cur).clone();
        if let Some(a) = active {
            if a.len() != cfg.table.rows.len() {
                return Err(EditError::Invalid(format!("There are {} recipients and {} ticks.", cfg.table.rows.len(), a.len())));
            }
            cfg.active = if a.iter().all(|x| *x) { Vec::new() } else { a.to_vec() };
        }
        if let Some(f) = filter {
            cfg.filter = f.to_vec();
        }
        if let Some(s) = sort {
            cfg.sort = s.to_vec();
        }
        if let Some(m) = mapping {
            for col in m.values().filter(|c| !c.is_empty()) {
                if cfg.column(col).is_none() {
                    return Err(EditError::Invalid(format!("There is no column called '{col}'.")));
                }
            }
            cfg.mapping = m.iter().filter(|(_, v)| !v.is_empty()).map(|(k, v)| (k.clone(), v.clone())).collect();
            // The matched address fields are part of the settings too.
            if let Some((info, _)) = self.merge_settings() {
                self.set_settings_merge(Some(&info.main_type), Some(&cfg));
            }
        }
        self.merge.set(Some(cfg));
        Ok(OpResult::default())
    }

    pub(super) fn op_insert_merge_field(&mut self, at: Pos, spec: &MergeFieldSpec) -> Result<OpResult, EditError> {
        self.touch(at.p.0);
        let p = self.para_mut(at.p.0)?;
        let off = at.o.min(p.len16());
        let rpr = edit::rpr_at(&p.inlines, off).map(strip_link_style).filter(|r| !r.children.is_empty());
        let inlines = instr::build(spec, text_look_rpr(rpr)).map_err(EditError::Invalid)?;
        let len: usize = inlines.iter().map(|i| i.len16()).sum();
        fields::insert_inlines_at(&mut p.inlines, off, inlines);
        Ok(OpResult { caret: Some(Pos::new(at.p.0, off + len)), ..Default::default() })
    }

    /// Copy the first label of a sheet into the others, each starting with Next Record (Word's Update Labels).
    pub(super) fn op_update_labels(&mut self, at: Option<NodeId>) -> Result<OpResult, EditError> {
        let target = match at {
            Some(a) => top_level_of(&self.body, a).ok_or(EditError::NotFound(a))?,
            None => self.body.iter().find(|b| matches!(b, Block::Table(_))).map(|b| b.id()).ok_or_else(|| EditError::Invalid("The document has no sheet of labels.".into()))?,
        };
        let idx = self.body.iter().position(|b| b.id() == target).ok_or(EditError::NotFound(target))?;
        let mut ids = self.ids.clone();
        let Block::Table(t) = &mut self.body[idx] else { return Err(EditError::Invalid("That isn't a sheet of labels.".into())) };
        let cells = labels::label_cells(t);
        if cells.len() < 2 {
            return Err(EditError::Invalid("That table has only one label.".into()));
        }
        let first: Vec<Block> = {
            let (r, c) = cells[0];
            t.rows().nth(r).and_then(|row| row.cells().nth(c)).map(|cell| cell.blocks.clone()).unwrap_or_default()
        };
        for &(r, c) in &cells[1..] {
            let mut blocks = first.clone();
            run::renumber(&mut blocks, &mut ids);
            if let Some(Block::Para(p)) = blocks.first_mut() {
                let next = fields::complex(" NEXT ", "«Next_Record»", Some(fields::no_proof()), false);
                fields::insert_inlines_at(&mut p.inlines, 0, next);
            }
            if let Some(cell) = t.rows_mut().nth(r).and_then(|row| row.cells_mut().nth(c)) {
                cell.blocks = blocks;
            }
        }
        self.ids = ids;
        self.touch_all();
        Ok(OpResult { count: Some(cells.len()), ..Default::default() })
    }

    /// Replace the document with a sheet of labels.
    pub(super) fn op_make_labels(&mut self, spec: &LabelSpec, fill: &LabelFill, look: &TextLook) -> Result<OpResult, EditError> {
        labels::check(spec).map_err(EditError::Invalid)?;
        if let LabelFill::Single { row, col, .. } = fill {
            if *row < 1 || *row > spec.down || *col < 1 || *col > spec.across {
                return Err(EditError::Invalid(format!("This sheet has {} rows and {} columns; there is no label at row {row}, column {col}.", spec.down, spec.across)));
            }
        }
        let table = labels::table(spec, fill, look);
        let blocks = vec![parse_block(&table, &mut self.ids), Block::Para(parse_paragraph(&labels::closing_paragraph(), &mut self.ids))];
        let created: Vec<NodeId> = blocks.iter().map(|b| b.id()).collect();
        let sect = self.ids.fresh();
        self.body = Body::from(blocks);
        self.body.push(Block::Opaque(Opaque { id: sect, el: labels::section(spec) }));
        self.touch_all();
        Ok(OpResult { created, ..Default::default() })
    }

    /// Add an envelope as the first section, or replace the document with one.
    pub(super) fn op_insert_envelope(&mut self, size: &envelope::EnvelopeSize, layout: &EnvelopeLayout, replace: bool, address_block: bool) -> Result<OpResult, EditError> {
        let (els, delivery_at) = envelope::paragraphs(size, layout, address_block);
        let mut blocks: Vec<Block> = els.iter().map(|e| Block::Para(parse_paragraph(e, &mut self.ids))).collect();
        if address_block {
            if let Some(Block::Para(p)) = blocks.get_mut(delivery_at) {
                let inlines = instr::build(&MergeFieldSpec::AddressBlock { name: None, company: false, country: None, except: None, format: None }, None).map_err(EditError::Invalid)?;
                fields::insert_inlines_at(&mut p.inlines, 0, inlines);
            }
        }
        let created: Vec<NodeId> = blocks.iter().map(|b| b.id()).collect();
        if replace {
            let sect = self.ids.fresh();
            // The envelope is the whole document: its page is the document's closing section, not one inside it.
            if let Some(Block::Para(p)) = blocks.last_mut() {
                if let Some(ppr) = p.ppr.as_mut() {
                    ppr.remove_children("w:sectPr");
                }
            }
            self.body = Body::from(blocks);
            self.body.push(Block::Opaque(Opaque { id: sect, el: envelope::section(size) }));
        } else {
            // An envelope already at the front is changed, not added to: the first section, when it is one Docxtor
            // made (a B5 report section has an envelope's size but not its margins).
            let first_end = self.body.iter().position(|b| matches!(b, Block::Para(p) if p.ppr.as_ref().and_then(|pp| pp.child("w:sectPr")).is_some()));
            let old = first_end.filter(|&k| matches!(&self.body[k], Block::Para(p) if p.ppr.as_ref().and_then(|pp| pp.child("w:sectPr")).is_some_and(envelope::is_envelope_section)));
            if let Some(end) = old {
                self.body.drain(..=end);
            }
            for (k, b) in blocks.into_iter().enumerate() {
                self.body.insert(k, b);
            }
        }
        self.touch_all();
        Ok(OpResult { created, ..Default::default() })
    }

    // -----------------------------------------------------------------------
    // Previewing and merging
    // -----------------------------------------------------------------------

    fn merger<'a>(&'a self, answers: &'a Answers) -> Result<Merger<'a>, String> {
        let cfg = self.merge.cfg.as_ref().ok_or("Choose the recipients first.")?;
        let mut m = Merger::new(cfg, answers);
        m.lang = None;
        if m.is_empty() {
            return Err("No recipients are included in the merge.".into());
        }
        Ok(m)
    }

    /// Show a record in place of the template (`Some(position)`), or the template again (`None`). While a record is
    /// shown the document can't be edited and saves as the template.
    pub fn merge_preview(&mut self, position: Option<usize>, answers: &Answers) -> Result<Option<PreviewInfo>, String> {
        let Some(pos) = position else {
            if let Some(p) = self.merge_preview.take() {
                self.body = p.body;
                self.touch_all();
            }
            return Ok(None);
        };
        let (blocks, info) = {
            let merger = self.merger(answers)?;
            let count = merger.len();
            let pos = pos.min(count - 1);
            let (content, tail) = split_tail(self.template_body());
            let mut ids = self.ids.clone();
            let unit = merger.unit(&content, pos, pos, &mut ids);
            let mut blocks = unit.blocks;
            let mut tail: Vec<Block> = tail;
            run::renumber(&mut tail, &mut ids);
            blocks.extend(tail);
            self.ids = ids;
            (blocks, PreviewInfo { position: pos, count, used: unit.used, skipped: unit.skipped })
        };
        let template = match self.merge_preview.take() {
            Some(p) => p.body,
            None => std::mem::take(&mut self.body),
        };
        self.merge_preview = Some(Previewing { body: template, position: info.position });
        self.body = Body::from(blocks);
        self.touch_all();
        Ok(Some(info))
    }

    /// Run `f` with the template in place of a previewed record.
    pub(super) fn with_template<R>(&mut self, f: impl FnOnce(&mut Document) -> R) -> R {
        let Some(p) = self.merge_preview.take() else { return f(self) };
        let shown = std::mem::replace(&mut self.body, p.body);
        let r = f(self);
        let template = std::mem::replace(&mut self.body, shown);
        self.merge_preview = Some(Previewing { body: template, position: p.position });
        r
    }

    /// Merge the document with the recipients into a new document: one copy for each record, a section for each. The
    /// new document has no merge fields and no recipients. `answers` is what `ASK` and `FILLIN` fields were answered.
    pub fn merge_to_docx(&mut self, answers: &Answers, range: &MergeRange) -> Result<(Vec<u8>, MergeResult), String> {
        self.with_template(|doc| doc.merge_to_docx_inner(answers, range))
    }

    fn merge_to_docx_inner(&mut self, answers: &Answers, range: &MergeRange) -> Result<(Vec<u8>, MergeResult), String> {
        let main_type = self.merge_settings().map(|(s, _)| s.main_type).unwrap_or_else(|| "formLetters".into());
        self.flush_parts();
        let mut out = Document::from_package(self.pkg.clone()).map_err(|e| e.to_string())?;
        // A new file: pictures and charts nothing in it uses are left out, even ones the template carried along unused.
        out.baseline_orphans = Some(BTreeSet::new());
        let (units, total) = {
            let mut merger = self.merger(answers)?;
            let total = merger.len();
            let (content, _) = split_tail(&out.body);
            // A range cuts the list; the copies are made from the records in it.
            let from = range.from.unwrap_or(0).min(total);
            let to = range.to.unwrap_or(total).clamp(from, total);
            if from == to {
                return Err("The range holds no records.".into());
            }
            // `NEXT` fields don't read past the range: a message for record 1 holds record 1's details only.
            merger.limit = Some(to);
            let mut ids = out.ids.clone();
            let mut units = Vec::new();
            let (mut at, mut merged) = (from, 0usize);
            while at < to {
                let u = merger.unit(&content, at, merged + from, &mut ids);
                at += u.used;
                if !u.skipped {
                    merged += u.used;
                    units.push(u);
                }
            }
            out.ids = ids;
            (units, to - from)
        };
        let copies = units.len();
        let used: usize = units.iter().map(|u| u.used).sum();
        if copies == 0 {
            return Err("Every record was left out by a rule.".into());
        }
        let tail: Vec<Block> = split_tail(&out.body).1;
        let final_sect = tail.iter().find_map(|b| match b {
            Block::Opaque(o) if o.el.name == "w:sectPr" => Some(o.el.clone()),
            _ => None,
        });
        let mut body: Vec<Block> = Vec::new();
        for (k, mut u) in units.into_iter().enumerate() {
            if k > 0 {
                run::strip_bookmarks(&mut u.blocks);
            }
            body.extend(u.blocks);
            // A directory flows on; anything else starts each record on a page of its own.
            if k + 1 < copies && main_type != "catalog" {
                let sect = final_sect.clone().unwrap_or_else(|| Element::new("w:sectPr"));
                match body.last_mut() {
                    Some(Block::Para(p)) if p.ppr.as_ref().is_none_or(|pp| pp.child("w:sectPr").is_none()) => {
                        put_ppr(p.ppr.get_or_insert_with(|| Element::new("w:pPr")), sect);
                    }
                    Some(Block::Para(_)) => {}
                    _ => {
                        let mut p = Paragraph::new(out.ids.fresh());
                        put_ppr(p.ppr.get_or_insert_with(|| Element::new("w:pPr")), sect);
                        body.push(Block::Para(p));
                    }
                }
            }
        }
        if body.is_empty() {
            body.push(Block::Para(Paragraph::new(out.ids.fresh())));
        }
        let mut tail = tail;
        run::renumber(&mut tail, &mut out.ids);
        body.extend(tail);
        out.body = Body::from(body);
        out.body_dirty = true;
        // The new document is an ordinary one: it keeps neither the settings of a merge nor the list of people.
        out.settings_edit(false, &mut |root| {
            root.remove_children("w:mailMerge");
        });
        out.merge.set(None);
        let bytes = out.save().map_err(|e| e.to_string())?;
        Ok((bytes, MergeResult { records: used, copies, skipped: total.saturating_sub(used) }))
    }
}
