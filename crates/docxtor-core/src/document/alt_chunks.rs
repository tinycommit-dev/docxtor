//! Imported chunks (`w:altChunk`) in a document: reading them on open, making them real, and what a save does with
//! the parts of chunks that are gone. The idea and the editing model are in `crate::alt_chunk`.

use super::*;
use crate::alt_chunk::{convert, AltChunk};
use crate::package::Relationship;
use crate::budget::Budget;
use std::sync::Arc;

/// What reading the chunks of one document keeps: the budget, and what each part converted to.
struct Chunks<'a> {
    budget: &'a Budget,
    memo: HashMap<String, Option<(convert::Kind, crate::fragment::Fragment)>>,
}

/// What chunk previews are drawn with: the styles, the lists and the theme of the document at that time.
#[derive(Clone)]
pub(super) struct PreviewStamp {
    styles: Option<crate::xml::XmlDoc>,
    numbering: Option<crate::xml::XmlDoc>,
    theme: Arc<super::design::ThemePart>,
}

/// The error text that tells `apply_batch` a batch changed a chunk and has to run again on real content.
const RETRY: &str = "an imported chunk was changed";

/// The keys of an op's JSON that hold block ids.
const ID_KEYS: &[&str] = &["p", "id", "ids", "at", "from", "to", "after", "before", "scope", "block"];

impl Document {
    /// Read every chunk in the body (table cells and content controls too) and put its converted blocks in a
    /// container where the `w:altChunk` stood. Chunks that can't be read stay as they are.
    ///
    /// Chunks spend from `budget`, the one of the file this document is part of: a chunk that is a .docx opens
    /// with the same budget, one level deeper, and past the limit stays as it is. Elements that name the same part
    /// are converted once.
    pub(super) fn load_alt_chunks(&mut self, budget: &Budget) {
        if !crate::alt_chunk::has_unloaded(&self.body) {
            return;
        }
        let mut body = std::mem::take(&mut self.body).into_vec();
        let owner = self.main_path.clone();
        let mut ctx = Chunks { budget, memo: HashMap::new() };
        self.convert_alt_blocks(&mut body, &owner, &mut ctx);
        self.body = Body::from(body);
        self.alt_stamp = Some(self.preview_stamp());
    }

    fn preview_stamp(&self) -> PreviewStamp {
        PreviewStamp { styles: self.styles.doc.clone(), numbering: self.numbering.doc.clone(), theme: self.theme.clone() }
    }

    /// The previews of chunks are drawn with the styles, lists and theme the document had when they were made. When
    /// those change (an edit to a style, a new theme, an undo of either), draw them again, keeping their ids, so the
    /// chunk's text follows the rest of the document and stays what it will be once it is made editable.
    pub(super) fn sync_alt_previews(&mut self) {
        if self.alt_chunks.is_empty() {
            return;
        }
        let now = self.preview_stamp();
        if self.alt_stamp.as_ref().is_some_and(|s| s.styles == now.styles && s.numbering == now.numbering && Arc::ptr_eq(&s.theme, &now.theme)) {
            return;
        }
        self.alt_stamp = Some(now);
        let ids: Vec<NodeId> = self.alt_chunks.keys().copied().collect();
        for id in ids {
            let Some(old) = self.alt_chunks.get(&id).cloned() else { continue };
            let mut scratch = self.alt_scratch();
            let els = if old.fragment.is_empty() { Vec::new() } else { scratch.fragment_elements(&old.fragment, true) };
            let mut blocks: Vec<Block> = els.iter().map(|e| parse_block(e, &mut self.ids)).filter(|b| matches!(b, Block::Para(_) | Block::Table(_))).collect();
            scratch.keep_media_apart(id);
            let Some((mut list, i)) = self.body.parent_mut(id) else { continue };
            let Block::Container(c) = &mut list[i] else { continue };
            keep_ids(&c.blocks, &mut blocks);
            c.blocks = blocks;
            self.alt_chunks.insert(id, Arc::new(AltChunk { kind: old.kind, doc: scratch, fragment: old.fragment.clone(), owner: old.owner.clone(), rid: old.rid.clone(), first: old.first, last: old.last }));
            self.touch(id);
        }
    }

    fn convert_alt_blocks(&mut self, blocks: &mut [Block], owner: &str, ctx: &mut Chunks) {
        for b in blocks.iter_mut() {
            match b {
                Block::Opaque(o) if o.el.name == "w:altChunk" => {
                    let el = o.el.clone();
                    if let Some((container, chunk)) = self.make_alt_chunk(&el, owner, o.id, ctx) {
                        self.alt_chunks.insert(container.id, Arc::new(chunk));
                        *b = Block::Container(container);
                    }
                }
                Block::Table(t) => {
                    for r in t.rows_mut() {
                        for c in r.cells_mut() {
                            self.convert_alt_blocks(&mut c.blocks, owner, ctx);
                        }
                    }
                }
                Block::Container(c) => self.convert_alt_blocks(&mut c.blocks, owner, ctx),
                _ => {}
            }
        }
    }

    /// The preview document for chunks: this document's styles, lists and theme, and no text.
    fn alt_scratch(&self) -> Document {
        let mut d = Document::new_blank();
        d.styles = self.styles.clone();
        d.numbering = self.numbering.clone();
        if let Some(rel) = self.pkg.rels_for(&self.main_path).ok().and_then(|r| r.into_iter().find(|r| r.rel_type == package::REL_THEME)) {
            if let Some(bytes) = self.pkg.get(&package::resolve_target(&self.main_path, &rel.target)) {
                d.pkg.set("word/theme/theme1.xml", bytes.to_vec());
            }
        }
        d
    }

    fn make_alt_chunk(&mut self, el: &Element, owner: &str, id: NodeId, ctx: &mut Chunks) -> Option<(BlockContainer, AltChunk)> {
        let rid = el.attr("r:id")?.to_string();
        let rel = self.pkg.resolve_rel(owner, &rid)?;
        if rel.external || !rel.rel_type.ends_with("/aFChunk") {
            return None;
        }
        // Too many chunks: the rest stay as the "embedded content" placeholder and the file still opens.
        if !ctx.budget.spend_chunk() {
            return None;
        }
        let (kind, fragment) = match ctx.memo.get(&rel.target) {
            Some(done) => done.clone()?,
            None => {
                let done = self.convert_part(&rel.target, ctx.budget);
                ctx.memo.insert(rel.target.clone(), done.clone());
                done?
            }
        };
        let mut scratch = self.alt_scratch();
        let els = if fragment.is_empty() { Vec::new() } else { scratch.fragment_elements(&fragment, true) };
        let first = self.ids.0 + 1;
        let blocks: Vec<Block> = els.iter().map(|e| parse_block(e, &mut self.ids)).filter(|b| matches!(b, Block::Para(_) | Block::Table(_))).collect();
        let last = self.ids.0;
        scratch.keep_media_apart(id);
        let container = BlockContainer { id, shell: el.clone(), content_in: None, blocks };
        Some((container, AltChunk { kind, doc: scratch, fragment, owner: owner.to_string(), rid, first, last }))
    }

    /// What a chunk's part holds, converted: the kind and the fragment. Spends the part's bytes from the budget.
    fn convert_part(&self, target: &str, budget: &Budget) -> Option<(convert::Kind, crate::fragment::Fragment)> {
        let data = self.pkg.get(target)?;
        budget.spend_bytes(data.len() as u64).ok()?;
        let ct = convert::content_type_of(&self.pkg, target).unwrap_or_default();
        let kind = convert::detect(&ct, target, data)?;
        let fragment = convert::to_fragment(&self.pkg, target, kind, data, budget)?;
        Some((kind, fragment))
    }

    /// Give the pictures the preview holds names no other picture of the document has, so they can be told apart.
    fn keep_media_apart(&mut self, tag: NodeId) {
        let names: Vec<String> = self.pkg.parts.iter().filter(|p| p.name.starts_with("word/media/")).map(|p| p.name.clone()).collect();
        if names.is_empty() {
            return;
        }
        let rels_path = package::rels_path(&self.main_path);
        let Ok(mut rels) = self.pkg.xml(&rels_path) else { return };
        let main = self.main_path.clone();
        for name in names {
            let Some(data) = self.pkg.get(&name).map(|d| d.to_vec()) else { continue };
            let base = name.rsplit('/').next().unwrap_or(&name).to_string();
            let renamed = format!("word/media/chunk{tag}_{base}");
            self.pkg.remove(&name);
            self.pkg.set(&renamed, data);
            for e in rels.root.elements_mut() {
                if e.attr("TargetMode") != Some("External") && package::resolve_target(&main, e.attr("Target").unwrap_or_default()) == name {
                    e.set_attr("Target", format!("media/chunk{tag}_{base}"));
                }
            }
        }
        self.pkg.set(&rels_path, rels.to_bytes());
    }

    /// A picture of a chunk's preview, by the name the rendered paragraph gives it.
    pub(super) fn alt_media(&self, path: &str) -> Option<&[u8]> {
        if !path.starts_with("word/media/chunk") {
            return None;
        }
        self.alt_chunks.values().find_map(|a| a.doc.pkg.get(path))
    }

    /// The chunk container that holds block `id` (a paragraph, a table, or the container itself), if any.
    pub fn alt_chunk_of(&self, id: NodeId) -> Option<NodeId> {
        let cid = self.alt_chunks.iter().find(|(cid, a)| **cid == id || a.holds(id)).map(|(cid, _)| *cid)?;
        matches!(find_block(&self.body, cid), Some(Block::Container(c)) if is_alt_chunk(c)).then_some(cid)
    }

    /// What reading the chunks left out, as open notes: (code, count). `chunk-objects` is a chart or embedded object a
    /// chunk's file has that the preview can't show; making that chunk editable drops it from the file for good.
    pub fn alt_chunk_notes(&self) -> Vec<(&'static str, usize)> {
        let mut out: Vec<(&'static str, usize)> = Vec::new();
        let mut add = |code: &'static str, n: usize| match out.iter_mut().find(|(c, _)| *c == code) {
            Some(e) => e.1 += n,
            None => out.push((code, n)),
        };
        for a in self.alt_chunks.values() {
            for w in &a.fragment.warnings {
                match w.kind.as_str() {
                    "object-dropped" => add("chunk-objects", w.count),
                    "picture-unreadable" => add("picture-unreadable", w.count),
                    "picture-unsupported" => add("picture-unsupported", w.count),
                    _ => {}
                }
            }
        }
        out
    }

    /// What kind of content the chunk holds (`html`, `mht`, `text`, `rtf` or `docx`).
    pub fn alt_chunk_kind(&self, container: NodeId) -> Option<&'static str> {
        self.alt_chunks.get(&container).map(|a| a.kind.name())
    }

    /// The chunks in the body, in order, as (container id, kind).
    pub fn alt_chunks_listed(&self) -> Vec<(NodeId, &'static str)> {
        fn go<'a>(blocks: impl IntoIterator<Item = &'a Block>, d: &Document, out: &mut Vec<(NodeId, &'static str)>) {
            for b in blocks {
                match b {
                    Block::Container(c) if is_alt_chunk(c) => {
                        if let Some(a) = d.alt_chunks.get(&c.id) {
                            out.push((c.id, a.kind.name()));
                        }
                    }
                    Block::Container(c) => go(&c.blocks, d, out),
                    Block::Table(t) => t.rows().for_each(|r| r.cells().for_each(|c| go(&c.blocks, d, out))),
                    _ => {}
                }
            }
        }
        let mut out = Vec::new();
        go(&self.body, self, &mut out);
        out
    }

    /// Make a chunk ordinary content: the same conversion into this document (styles and lists merged as a paste does,
    /// pictures and links related to the part), in place of the container, with the ids the preview blocks had.
    /// The relationship and the part stay in the package until a save finds nothing uses them.
    pub(super) fn op_expand_alt_chunk(&mut self, id: NodeId) -> Result<OpResult, EditError> {
        // Any block of the chunk names it.
        let id = self.alt_chunk_of(id).ok_or(EditError::NotFound(id))?;
        let chunk = self.alt_chunks.get(&id).cloned().ok_or(EditError::NotFound(id))?;
        if !matches!(find_block(&self.body, id), Some(Block::Container(c)) if is_alt_chunk(c)) {
            return Err(EditError::Invalid("not an imported chunk".into()));
        }
        let els = if chunk.fragment.is_empty() { Vec::new() } else { self.fragment_elements(&chunk.fragment, false) };
        let mut fresh: Vec<Block> = els.iter().map(|e| parse_block(e, &mut self.ids)).filter(|b| matches!(b, Block::Para(_) | Block::Table(_))).collect();
        let (mut list, i) = self.body.parent_mut(id).ok_or(EditError::NotFound(id))?;
        let Block::Container(old) = list.remove(i) else { return Err(EditError::Invalid("not an imported chunk".into())) };
        keep_ids(&old.blocks, &mut fresh);
        let ids: Vec<NodeId> = fresh.iter().map(|b| b.id()).collect();
        for (k, b) in fresh.into_iter().enumerate() {
            list.insert(i + k, b);
        }
        for b in &ids {
            self.changed.insert(*b);
        }
        // The container no longer exists; drawing it again is the blocks' job.
        self.changed.remove(&id);
        self.body_dirty = true;
        Ok(OpResult { created: ids, ..Default::default() })
    }

    // -----------------------------------------------------------------------
    // Edits inside a chunk
    // -----------------------------------------------------------------------

    /// Apply ops atomically; ops that change what is inside an imported chunk make the chunk real first, in the same step.
    pub fn apply_batch(&mut self, ops: &[Op], merge: bool) -> Result<Vec<OpResult>, EditError> {
        // Before anything is expanded or noted: a view-only document is not touched.
        self.check_access(ops)?;
        record_ops(ops);
        // The invariant check (see `invariants`) notes what is broken before the batch and compares after it.
        self.inv_begin();
        let r = self.apply_batch_chunks(ops, merge);
        if r.is_ok() {
            self.sync_alt_previews();
            self.inv_end("apply_batch", Some(ops));
        }
        r
    }

    fn apply_batch_chunks(&mut self, ops: &[Op], merge: bool) -> Result<Vec<OpResult>, EditError> {
        if self.alt_chunks.is_empty() {
            return self.apply_batch_core(ops, merge);
        }
        let named = self.chunks_named_by(ops);
        if !named.is_empty() {
            return self.apply_with_expanded(&named, ops);
        }
        match self.apply_batch_core(ops, merge) {
            Err(EditError::Invalid(msg)) if msg.ends_with(RETRY) => {
                let dirty = std::mem::take(&mut self.alt_dirty);
                if dirty.is_empty() {
                    return Err(EditError::Invalid(msg));
                }
                self.apply_with_expanded(&dirty, ops)
            }
            other => other,
        }
    }

    /// Run `ops` with `chunks` made real first. An op that names one chunk can reach others (a range from one chunk
    /// over the next): each time the batch changes a chunk that was not expanded, that chunk joins the list and the
    /// batch runs again, at most once per chunk.
    fn apply_with_expanded(&mut self, chunks: &[NodeId], ops: &[Op]) -> Result<Vec<OpResult>, EditError> {
        let mut chunks = chunks.to_vec();
        loop {
            let mut all: Vec<Op> = chunks.iter().map(|c| Op::ExpandAltChunk { id: Id(*c) }).collect();
            all.extend_from_slice(ops);
            match self.apply_batch_core(&all, false) {
                Ok(mut results) => {
                    results.drain(..chunks.len());
                    return Ok(results);
                }
                Err(EditError::Invalid(msg)) if msg.ends_with(RETRY) => {
                    let more: Vec<NodeId> = std::mem::take(&mut self.alt_dirty).into_iter().filter(|c| !chunks.contains(c)).collect();
                    if more.is_empty() {
                        // Nothing new to expand: the batch cannot be made to work, and the caller never sees the internal text.
                        return Err(EditError::Invalid("that edit reaches imported content that can't be changed here".into()));
                    }
                    chunks.extend(more);
                }
                Err(e) => return Err(e),
            }
        }
    }

    /// The chunks whose blocks the ops name (by a block id in a position, a list of ids, and so on).
    fn chunks_named_by(&self, ops: &[Op]) -> Vec<NodeId> {
        let mut ids: Vec<u64> = Vec::new();
        for op in ops {
            if matches!(op, Op::ExpandAltChunk { .. }) {
                continue;
            }
            if let Ok(v) = serde_json::to_value(op) {
                collect_ids(&v, None, &mut ids);
            }
        }
        let mut out: Vec<NodeId> = Vec::new();
        for id in ids {
            if let Some(c) = u32::try_from(id).ok().and_then(|i| self.alt_chunk_of(i)) {
                if !out.contains(&c) {
                    out.push(c);
                }
            }
        }
        out
    }

    /// After a batch has run: the chunks whose blocks differ from `before`. The batch is then undone and run again
    /// with the chunks made real (see `apply_batch`).
    pub(super) fn alt_changed_since(&mut self, before: &Snapshot) -> bool {
        let mut dirty = Vec::new();
        // The change marks name top-level blocks: a chunk in a table cell or a content control is marked as its table or
        // control (Replace all, Replace font), so every chunk inside a block the batch touched is compared.
        let mut chunks: Vec<NodeId> = self.alt_chunks.keys().copied().filter(|cid| self.changed.contains(cid) || top_level_of(&self.body, *cid).is_some_and(|t| self.changed.contains(&t))).collect();
        chunks.sort_unstable();
        for cid in chunks {
            let (Some(Block::Container(now)), Some(Block::Container(was))) = (find_block(&self.body, cid), before.find_block(cid)) else { continue };
            if is_alt_chunk(now) && format!("{:?}", now.blocks) != format!("{:?}", was.blocks) {
                dirty.push(cid);
            }
        }
        self.alt_dirty = dirty;
        !self.alt_dirty.is_empty()
    }

    /// The text `apply_batch` fails with when a batch changed a chunk (see `alt_changed_since`).
    pub(super) fn alt_retry_error() -> EditError {
        EditError::Invalid(RETRY.into())
    }

    // -----------------------------------------------------------------------
    // Saving
    // -----------------------------------------------------------------------

    /// Add to `skip` and `replace` (what `Package::to_bytes_with` leaves out and rewrites) for chunks the body no
    /// longer has: the relationship, the part, its own relationships and what only it uses. A chunk the file had
    /// that nothing refers to from the start is not touched: that is the file's own content.
    pub(super) fn leave_out_gone_chunks(&self, pkg: &Package, skip: &mut BTreeSet<String>, replace: &mut HashMap<String, Vec<u8>>) {
        if self.alt_chunks.is_empty() {
            return;
        }
        let mut used: BTreeSet<(String, String)> = BTreeSet::new();
        fn scan<'a>(blocks: impl IntoIterator<Item = &'a Block>, owner: &str, used: &mut BTreeSet<(String, String)>) {
            for b in blocks {
                match b {
                    Block::Container(c) if is_alt_chunk(c) => {
                        if let Some(r) = c.shell.attr("r:id") {
                            used.insert((owner.to_string(), r.to_string()));
                        }
                    }
                    Block::Container(c) => scan(&c.blocks, owner, used),
                    Block::Table(t) => t.rows().for_each(|r| r.cells().for_each(|c| scan(&c.blocks, owner, used))),
                    _ => {}
                }
            }
        }
        scan(&self.body, &self.main_path, &mut used);
        let gone: BTreeSet<(String, String)> = self.alt_chunks.values().map(|a| (a.owner.clone(), a.rid.clone())).filter(|k| !used.contains(k)).collect();
        if gone.is_empty() {
            return;
        }
        let data_of = |name: &str, skip: &BTreeSet<String>, replace: &HashMap<String, Vec<u8>>| -> Option<Vec<u8>> {
            if skip.contains(name) {
                return None;
            }
            replace.get(name).cloned().or_else(|| pkg.get(name).map(|d| d.to_vec()))
        };
        for (owner, rid) in gone {
            let rels_path = package::rels_path(&owner);
            let Some(mut rels) = data_of(&rels_path, skip, replace).and_then(|d| xml::parse(&d).ok()) else { continue };
            let Some(rel) = rels.root.elements().find(|e| e.attr("Id") == Some(rid.as_str())).map(|e| Relationship {
                id: rid.clone(),
                rel_type: e.attr("Type").unwrap_or_default().to_string(),
                target: e.attr("Target").unwrap_or_default().to_string(),
                external: e.attr("TargetMode") == Some("External"),
            }) else { continue };
            if rel.external || !rel.rel_type.ends_with("/aFChunk") {
                continue;
            }
            let target = package::resolve_target(&owner, &rel.target);
            rels.root.children.retain(|n| !matches!(n, Node::Element(e) if e.attr("Id") == Some(rid.as_str())));
            replace.insert(rels_path, rels.to_bytes());
            // Another relationship to the same part keeps it.
            let shared = pkg.parts.iter().filter(|p| p.name.ends_with(".rels") && !skip.contains(&p.name)).any(|p| {
                let source = super::picture_watermark::rels_source(&p.name);
                let Some(d) = data_of(&p.name, skip, replace).and_then(|d| xml::parse(&d).ok()) else { return false };
                let hit = d.root.elements().any(|e| e.attr("TargetMode") != Some("External") && package::resolve_target(&source, e.attr("Target").unwrap_or_default()) == target);
                hit
            });
            if shared {
                continue;
            }
            // What only the chunk's own relationships use goes with it (the pictures of a web page).
            let own_rels = package::rels_path(&target);
            let mut children: Vec<String> = Vec::new();
            if let Some(d) = data_of(&own_rels, skip, replace).and_then(|d| xml::parse(&d).ok()) {
                for e in d.root.elements().filter(|e| e.attr("TargetMode") != Some("External")) {
                    children.push(package::resolve_target(&target, e.attr("Target").unwrap_or_default()));
                }
            }
            for child in children {
                let used_elsewhere = pkg.parts.iter().filter(|p| p.name.ends_with(".rels") && p.name != own_rels && !skip.contains(&p.name)).any(|p| {
                    let source = super::picture_watermark::rels_source(&p.name);
                    let Some(d) = data_of(&p.name, skip, replace).and_then(|d| xml::parse(&d).ok()) else { return true };
                    let hit = d.root.elements().any(|e| e.attr("TargetMode") != Some("External") && package::resolve_target(&source, e.attr("Target").unwrap_or_default()) == child);
                    hit
                });
                if !used_elsewhere {
                    skip.insert(child);
                }
            }
            skip.insert(own_rels);
            skip.insert(target.clone());
            if let Some(ct) = data_of("[Content_Types].xml", skip, replace).and_then(|d| xml::parse(&d).ok()) {
                let mut ct = ct;
                let want = format!("/{target}");
                let before = ct.root.children.len();
                ct.root.children.retain(|n| !matches!(n, Node::Element(e) if e.local() == "Override" && e.attr("PartName") == Some(want.as_str())));
                if ct.root.children.len() != before {
                    replace.insert("[Content_Types].xml".into(), ct.to_bytes());
                }
            }
        }
    }
}

fn collect_ids(v: &serde_json::Value, key: Option<&str>, out: &mut Vec<u64>) {
    use serde_json::Value;
    match v {
        Value::Number(n) => {
            if key.is_some_and(|k| ID_KEYS.contains(&k)) {
                out.extend(n.as_u64());
            }
        }
        Value::Array(a) => a.iter().for_each(|x| collect_ids(x, key, out)),
        Value::Object(o) => o.iter().for_each(|(k, x)| collect_ids(x, Some(k), out)),
        _ => {}
    }
}

/// Give blocks the ids of the blocks they were made from, walking the two lists side by side while their shapes agree.
fn keep_ids(old: &[Block], new: &mut [Block]) {
    for (o, n) in old.iter().zip(new.iter_mut()) {
        match (o, n) {
            (Block::Para(a), Block::Para(b)) => b.id = a.id,
            (Block::Table(a), Block::Table(b)) => {
                b.id = a.id;
                let (ra, rb): (Vec<_>, Vec<_>) = (a.rows().collect(), b.rows_mut().collect());
                for (ra, rb) in ra.into_iter().zip(rb) {
                    for (ca, cb) in ra.cells().zip(rb.cells_mut()) {
                        keep_ids(&ca.blocks, &mut cb.blocks);
                    }
                }
            }
            _ => {}
        }
    }
}

/// Test aid: when `DOCXTOR_RECORD_OPS` names a file, every batch is appended to it as one JSON line (the random op
/// harness replays the shapes the hand-written tests use; see `tests/random_ops.rs`).
fn record_ops(ops: &[Op]) {
    if let Some(path) = std::env::var_os("DOCXTOR_RECORD_OPS") {
        use std::io::Write;
        if let (Ok(mut line), Ok(mut f)) = (serde_json::to_string(ops), std::fs::OpenOptions::new().create(true).append(true).open(path)) {
            line.push('\n');
            let _ = f.write_all(line.as_bytes());
        }
    }
}
