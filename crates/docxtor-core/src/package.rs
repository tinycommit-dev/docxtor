//! The OPC (zip) container. Parts are kept as raw bytes in their original order
//! so that anything Docxtor doesn't touch is written back byte-for-byte.

use crate::xml::{self, Element, XmlDoc};
use std::io::{Cursor, Read, Write};

pub const REL_OFFICE_DOCUMENT: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument";
pub const REL_STYLES: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles";
pub const REL_NUMBERING: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/numbering";
pub const REL_THEME: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme";
pub const REL_IMAGE: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/image";
pub const REL_HYPERLINK: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink";
pub const REL_HEADER: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/header";
pub const REL_FOOTER: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/footer";
pub const REL_COMMENTS: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/comments";
pub const REL_FOOTNOTES: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/footnotes";
pub const REL_ENDNOTES: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/endnotes";
pub const REL_SETTINGS: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/settings";
pub const REL_CORE: &str = "http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties";

pub const CT_NUMBERING: &str = "application/vnd.openxmlformats-officedocument.wordprocessingml.numbering+xml";
pub const CT_STYLES: &str = "application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml";
pub const CT_HEADER: &str = "application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml";
pub const CT_FOOTER: &str = "application/vnd.openxmlformats-officedocument.wordprocessingml.footer+xml";
pub const CT_SETTINGS: &str = "application/vnd.openxmlformats-officedocument.wordprocessingml.settings+xml";
pub const CT_CORE: &str = "application/vnd.openxmlformats-package.core-properties+xml";
pub const CT_ENDNOTES: &str = "application/vnd.openxmlformats-officedocument.wordprocessingml.endnotes+xml";
pub const CT_FOOTNOTES: &str = "application/vnd.openxmlformats-officedocument.wordprocessingml.footnotes+xml";

#[derive(Debug, thiserror::Error)]
pub enum PackageError {
    #[error("not a valid .docx (zip) file: {0}")]
    Zip(String),
    #[error("missing part: {0}")]
    MissingPart(String),
    #[error(transparent)]
    Xml(#[from] xml::XmlError),
    #[error("io: {0}")]
    Io(String),
}

#[derive(Debug, Clone)]
pub struct Part {
    pub name: String,
    /// Empty for the part kept as pieces (the main text after a save, `Package::set_pieces`): read a part of a
    /// package through `Package::get`, `part_data` or `bytes_of`, not here.
    pub data: Vec<u8>,
    pub stored: bool,
}

/// One piece of a part's text, shared (never copied) between the package, the body cache and the parts a save hands on.
pub type Piece = std::sync::Arc<Vec<u8>>;
/// A part's text as pieces, in order.
pub type PieceList = Vec<Piece>;

/// A part's bytes as a save sees them: one slice, or pieces that are never joined (see `Package::set_pieces`).
#[derive(Clone, Copy)]
pub enum Data<'a> {
    One(&'a [u8]),
    Many(&'a [Piece]),
}

impl<'a> Data<'a> {
    pub fn len(&self) -> usize {
        match self {
            Data::One(d) => d.len(),
            Data::Many(p) => p.iter().map(|x| x.len()).sum(),
        }
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    /// The bytes in order, as slices.
    pub fn chunks(&self) -> impl Iterator<Item = &'a [u8]> {
        let (one, many): (Option<&'a [u8]>, &'a [Piece]) = match *self {
            Data::One(d) => (Some(d), &[]),
            Data::Many(p) => (None, p),
        };
        one.into_iter().chain(many.iter().map(|p| p.as_slice()))
    }
    /// The one slice, when the data is not in pieces.
    pub fn as_one(&self) -> Option<&'a [u8]> {
        match *self {
            Data::One(d) => Some(d),
            Data::Many(_) => None,
        }
    }
    /// The whole data as one slice: joined (a copy) when it is in pieces. For rare paths only.
    pub fn to_cow(&self) -> std::borrow::Cow<'a, [u8]> {
        match *self {
            Data::One(d) => std::borrow::Cow::Borrowed(d),
            Data::Many(p) => {
                crate::perf::part_joined();
                std::borrow::Cow::Owned(join_pieces(p))
            }
        }
    }
}

fn join_pieces(p: &[Piece]) -> Vec<u8> {
    let mut v = Vec::with_capacity(p.iter().map(|x| x.len()).sum());
    for x in p {
        v.extend_from_slice(x);
    }
    v
}

/// The joined copy of a part kept as pieces, made when something asks for the part as one slice (`Package::get`). Not
/// copied with the package.
#[derive(Default)]
struct JoinCache(std::sync::OnceLock<Vec<u8>>);

impl Clone for JoinCache {
    fn clone(&self) -> Self {
        JoinCache::default()
    }
}

impl std::fmt::Debug for JoinCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "JoinCache")
    }
}

#[derive(Debug, Clone, Default)]
pub struct Package {
    pub parts: Vec<Part>,
    /// The one part (the main text, once a save has written it) whose text is kept as pieces: its entry in `parts`
    /// keeps its place and its `stored` flag and has no data. Saves write the pieces without joining them; `get` joins.
    pieces: Option<(String, std::sync::Arc<PieceList>)>,
    joined: JoinCache,
    /// While an op batch runs: how each relationship part, `[Content_Types].xml` and each newly added part was
    /// before its first change, so a refused batch or an undo can put the package back (see `begin_journal`).
    journal: Option<Vec<PartDelta>>,
    /// For each picture part (`word/media/...`), a number that changes whenever its bytes are set, replaced or taken
    /// out, so a cache of what a picture looks like can tell a part name that now holds a different picture (see
    /// `Package::media_gen`).
    media_gens: std::collections::HashMap<String, u32>,
    media_counter: u32,
}

/// One part as it was: `None` for a part that did not exist.
pub type PartDelta = (String, Option<Part>);

#[derive(Debug, Clone, PartialEq)]
pub struct Relationship {
    pub id: String,
    pub rel_type: String,
    pub target: String,
    pub external: bool,
}

impl Package {
    pub fn open(bytes: &[u8]) -> Result<Self, PackageError> {
        Self::open_with(bytes, &crate::budget::Budget::default())
    }

    /// `open`, spending from `budget`: the parts and the bytes inflated. A zip that runs it out is an error.
    pub fn open_with(bytes: &[u8], budget: &crate::budget::Budget) -> Result<Self, PackageError> {
        let over = |_| PackageError::Zip(crate::budget::MESSAGE.to_string());
        let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).map_err(|e| PackageError::Zip(e.to_string()))?;
        let mut parts = Vec::with_capacity(zip.len().min(1024));
        for i in 0..zip.len() {
            budget.spend_part().map_err(over)?;
            let mut f = zip.by_index(i).map_err(|e| PackageError::Zip(e.to_string()))?;
            if f.is_dir() {
                continue;
            }
            let name = f.name().map_err(|e| PackageError::Zip(e.to_string()))?.trim_start_matches('/').to_string();
            let stored = f.compression() == zip::CompressionMethod::Stored;
            // What the entry says it holds can't be trusted: reserve and read no more than the budget has left.
            let left = budget.bytes_left();
            let mut data = Vec::with_capacity((f.size() as usize).min(1 << 24).min(left as usize));
            (&mut f).take(left.saturating_add(1)).read_to_end(&mut data).map_err(|e| PackageError::Io(e.to_string()))?;
            budget.spend_bytes(data.len() as u64).map_err(over)?;
            parts.push(Part { name, data, stored });
        }
        Ok(Package::from_parts(parts))
    }

    pub fn from_parts(parts: Vec<Part>) -> Self {
        Package { parts, pieces: None, joined: JoinCache::default(), journal: None, media_gens: Default::default(), media_counter: 0 }
    }

    /// Start recording the first state of every part an op changes that undo has to give back: the relationship
    /// parts, `[Content_Types].xml` and parts that are new. Parts that are rewritten from the document's own state
    /// (styles, settings, stories...) are not recorded; the snapshot restores those.
    pub fn begin_journal(&mut self) {
        self.journal = Some(Vec::new());
    }

    /// Stop recording and return what was recorded: the package as it was before the batch, for the parts it touched.
    pub fn end_journal(&mut self) -> Vec<PartDelta> {
        self.journal.take().unwrap_or_default()
    }

    /// A number for the picture part `name` that changes whenever the bytes it holds change, or it is added or taken
    /// out; 0 for a part never touched since the file opened. Part names are reused (undo gives one up, the next
    /// picture takes the free name), so a name alone does not say which picture it holds.
    pub fn media_gen(&self, name: &str) -> u32 {
        self.media_gens.get(name.trim_start_matches('/')).copied().unwrap_or(0)
    }

    fn bump_media(&mut self, name: &str) {
        if name.starts_with("word/media/") {
            self.media_counter = self.media_counter.wrapping_add(1).max(1);
            self.media_gens.insert(name.to_string(), self.media_counter);
        }
    }

    /// Put the parts in `delta` back as they were, and return what they were instead (to undo the undo).
    pub fn swap_delta(&mut self, delta: Vec<PartDelta>) -> Vec<PartDelta> {
        let mut now = Vec::with_capacity(delta.len());
        for (name, want) in delta {
            self.bump_media(&name);
            let at = self.parts.iter().position(|p| p.name == name);
            let cur = at.map(|i| self.part_with_bytes(i));
            self.forget_pieces_of(&name);
            match (at, want) {
                (Some(i), Some(p)) => self.parts[i] = p,
                (Some(i), None) => {
                    self.parts.remove(i);
                }
                (None, Some(p)) => self.parts.push(p),
                (None, None) => {}
            }
            now.push((name, cur));
        }
        now
    }

    /// A copy of part `i` with its bytes, joined when they are kept as pieces (a rare path: a journal or an undo met the main part).
    fn part_with_bytes(&self, i: usize) -> Part {
        let mut p = self.parts[i].clone();
        if let Some((n, list)) = &self.pieces {
            if *n == p.name {
                crate::perf::part_joined();
                p.data = join_pieces(list);
            }
        }
        p
    }

    /// The part `name` is about to be replaced or taken out: it is no longer kept as pieces.
    fn forget_pieces_of(&mut self, name: &str) {
        if self.pieces.as_ref().is_some_and(|(n, _)| n == name) {
            self.pieces = None;
        }
        self.joined = JoinCache::default();
    }

    /// Stop recording and undo everything recorded.
    pub fn rollback_journal(&mut self) {
        let delta = self.end_journal();
        self.swap_delta(delta);
    }

    /// Record `name` as it is now, before it changes, when a journal is running: every part an op writes or removes
    /// through the package (a rule, not a list of names), so a refused batch and undo give each back byte for byte.
    fn note(&mut self, name: &str) {
        let Some(j) = &self.journal else { return };
        if j.iter().any(|(n, _)| n == name) {
            return;
        }
        let cur = self.parts.iter().position(|p| p.name == name).map(|i| self.part_with_bytes(i));
        if let Some(j) = &mut self.journal {
            j.push((name.to_string(), cur));
        }
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>, PackageError> {
        self.to_bytes_with(&Default::default(), &Default::default())
    }

    /// Like `to_bytes`, without the parts named in `skip` and with the data in `replace` for the parts it
    /// names. The package itself is not changed.
    pub fn to_bytes_with(&self, skip: &std::collections::BTreeSet<String>, replace: &std::collections::HashMap<String, Vec<u8>>) -> Result<Vec<u8>, PackageError> {
        write_zip(self.entries(skip, replace))
    }

    /// The parts a save writes, in the order it writes them: `[Content_Types].xml` first for some consumers.
    fn entries<'a>(&'a self, skip: &std::collections::BTreeSet<String>, replace: &'a std::collections::HashMap<String, Vec<u8>>) -> Vec<(&'a str, Data<'a>, bool)> {
        let mut order: Vec<&Part> = self.parts.iter().filter(|p| !skip.contains(&p.name)).collect();
        order.sort_by_key(|p| if p.name == "[Content_Types].xml" { 0 } else { 1 });
        order
            .into_iter()
            .map(|p| {
                let data = match replace.get(&p.name) {
                    Some(d) => Data::One(d.as_slice()),
                    None => self.data_of(p),
                };
                (p.name.as_str(), data, p.stored)
            })
            .collect()
    }

    /// The data of a part of this package, in pieces when it is kept so.
    pub(crate) fn data_of<'a>(&'a self, p: &'a Part) -> Data<'a> {
        match &self.pieces {
            Some((n, list)) if *n == p.name => Data::Many(list.as_slice()),
            _ => Data::One(&p.data),
        }
    }

    /// The bytes of a part of this package as one slice: joined (a copy) when they are kept as pieces. For rare paths only.
    pub fn bytes_of<'a>(&'a self, p: &'a Part) -> std::borrow::Cow<'a, [u8]> {
        self.data_of(p).to_cow()
    }

    /// The part `name` as a save sees it: its pieces when it is kept as pieces.
    pub fn part_data(&self, name: &str) -> Option<Data<'_>> {
        let name = name.trim_start_matches('/');
        self.parts.iter().find(|p| p.name == name).map(|p| self.data_of(p))
    }

    /// The size of a part, without joining it.
    pub fn part_len(&self, name: &str) -> Option<usize> {
        self.part_data(name).map(|d| d.len())
    }

    /// The pieces the part `name` is kept as, if it is.
    pub fn pieces_of(&self, name: &str) -> Option<&std::sync::Arc<PieceList>> {
        let name = name.trim_start_matches('/');
        self.pieces.as_ref().filter(|(n, _)| n == name).map(|(_, l)| l)
    }

    /// Let go of the data of the part `name` (it stays in the package, empty) because it is about to be written again whole.
    pub fn release(&mut self, name: &str) {
        let name = name.trim_start_matches('/');
        self.note(name);
        self.forget_pieces_of(name);
        if let Some(p) = self.parts.iter_mut().find(|p| p.name == name) {
            p.data = Vec::new();
        }
    }

    /// Keep the text of the part `name` as `list` (its pieces, in order) instead of as one buffer: saves hand the pieces on
    /// without joining them, so a big part never needs one buffer of its own. `get` still gives the whole part (joined, a copy
    /// kept until the part changes), for the rare paths that need it. The part keeps its place and `stored` flag.
    pub fn set_pieces(&mut self, name: &str, list: std::sync::Arc<PieceList>) {
        let name = name.trim_start_matches('/');
        self.note(name);
        self.bump_media(name);
        // Only one part is kept so: another one is joined back.
        if let Some((other, old)) = self.pieces.take() {
            if other != name {
                if let Some(p) = self.parts.iter_mut().find(|p| p.name == other) {
                    p.data = join_pieces(&old);
                }
            }
        }
        self.joined = JoinCache::default();
        match self.parts.iter_mut().find(|p| p.name == name) {
            Some(p) => p.data = Vec::new(),
            None => self.parts.push(Part { name: name.to_string(), data: Vec::new(), stored: false }),
        }
        self.pieces = Some((name.to_string(), list));
    }

    /// What `to_bytes_with` would write, as parts that are not compressed yet (see `zip_packed`): for each part a byte that is
    /// 1 when the zip keeps it stored, its name's length and name, and its data's length and data (lengths are 4 bytes,
    /// little endian). Compressing is most of a save, and a copy of the parts can be compressed anywhere.
    pub fn pack_with(&self, skip: &std::collections::BTreeSet<String>, replace: &std::collections::HashMap<String, Vec<u8>>) -> Vec<u8> {
        let size: usize = self.entries(skip, replace).iter().map(|(n, d, _)| n.len() + d.len() + 9).sum();
        let mut out = Vec::with_capacity(size);
        self.pack_pieces(skip, replace, usize::MAX, &mut |p| out.extend_from_slice(p));
        out
    }

    /// `pack_with` handed on in pieces of at most `max` bytes (the small parts and the heads of parts gathered into one
    /// piece, a big part's data cut in pieces of its own), so the bytes never have to be in one buffer here: the browser
    /// binding gives each piece to the page as it comes (see `DocxtorDoc::savePackedPieces`).
    pub fn pack_pieces(&self, skip: &std::collections::BTreeSet<String>, replace: &std::collections::HashMap<String, Vec<u8>>, max: usize, piece: &mut dyn FnMut(&[u8])) {
        let mut buf: Vec<u8> = Vec::new();
        for (name, data, stored) in self.entries(skip, replace) {
            buf.push(stored as u8);
            buf.extend_from_slice(&(name.len() as u32).to_le_bytes());
            buf.extend_from_slice(name.as_bytes());
            buf.extend_from_slice(&(data.len() as u32).to_le_bytes());
            if buf.len() + data.len() <= max {
                for c in data.chunks() {
                    buf.extend_from_slice(c);
                }
                continue;
            }
            if !buf.is_empty() {
                piece(&buf);
                buf.clear();
            }
            for c in data.chunks() {
                c.chunks(max).for_each(&mut *piece);
            }
        }
        if !buf.is_empty() {
            piece(&buf);
        }
    }

    pub fn get(&self, name: &str) -> Option<&[u8]> {
        let name = name.trim_start_matches('/');
        let p = self.parts.iter().find(|p| p.name == name)?;
        match &self.pieces {
            Some((n, list)) if *n == p.name => Some(
                self.joined
                    .0
                    .get_or_init(|| {
                        crate::perf::part_joined();
                        join_pieces(list)
                    })
                    .as_slice(),
            ),
            _ => Some(p.data.as_slice()),
        }
    }

    pub fn has(&self, name: &str) -> bool {
        let name = name.trim_start_matches('/');
        self.parts.iter().any(|p| p.name == name)
    }

    pub fn set(&mut self, name: &str, data: Vec<u8>) {
        let name = name.trim_start_matches('/');
        self.note(name);
        self.bump_media(name);
        self.forget_pieces_of(name);
        if let Some(p) = self.parts.iter_mut().find(|p| p.name == name) {
            p.data = data;
        } else {
            let stored = is_precompressed(name);
            self.parts.push(Part { name: name.to_string(), data, stored });
        }
    }

    /// Drop a part from the package (nothing happens if it isn't there).
    pub fn remove(&mut self, name: &str) {
        let name = name.trim_start_matches('/');
        self.note(name);
        self.bump_media(name);
        self.forget_pieces_of(name);
        self.parts.retain(|p| p.name != name);
    }

    /// Drop the relationships of `part` that point at `target` (an absolute part name).
    pub fn remove_rels_to(&mut self, part: &str, target: &str) {
        let path = rels_path(part);
        let Some(mut doc) = self.get(&path).and_then(|d| xml::parse(d).ok()) else { return };
        let before = doc.root.children.len();
        doc.root.children.retain(|n| !matches!(n, xml::Node::Element(e) if e.local() == "Relationship" && e.attr("TargetMode") != Some("External") && resolve_target(part, e.attr("Target").unwrap_or_default()) == target));
        if doc.root.children.len() != before {
            self.set(&path, doc.to_bytes());
        }
    }

    /// Drop the `[Content_Types].xml` override of a part.
    pub fn remove_override(&mut self, part: &str) {
        let Ok(mut doc) = self.xml("[Content_Types].xml") else { return };
        let pn = format!("/{}", part.trim_start_matches('/'));
        let before = doc.root.children.len();
        doc.root.children.retain(|n| !matches!(n, xml::Node::Element(e) if e.local() == "Override" && e.attr("PartName") == Some(pn.as_str())));
        if doc.root.children.len() != before {
            self.set("[Content_Types].xml", doc.to_bytes());
        }
    }

    pub fn xml(&self, name: &str) -> Result<XmlDoc, PackageError> {
        let data = self.get(name).ok_or_else(|| PackageError::MissingPart(name.to_string()))?;
        Ok(xml::parse(data)?)
    }

    /// Path to the main document part (normally `word/document.xml`).
    pub fn main_document_path(&self) -> String {
        if let Ok(rels) = self.rels_for("") {
            if let Some(r) = rels.iter().find(|r| r.rel_type == REL_OFFICE_DOCUMENT) {
                return resolve_target("", &r.target);
            }
        }
        "word/document.xml".to_string()
    }

    /// Relationships of a part (`""` for the package root).
    pub fn rels_for(&self, part: &str) -> Result<Vec<Relationship>, PackageError> {
        let path = rels_path(part);
        let Some(data) = self.get(&path) else { return Ok(Vec::new()) };
        let doc = xml::parse(data)?;
        Ok(doc
            .root
            .elements()
            .filter(|e| e.local() == "Relationship")
            .map(|e| Relationship {
                id: e.attr("Id").unwrap_or_default().to_string(),
                rel_type: e.attr("Type").unwrap_or_default().to_string(),
                target: e.attr("Target").unwrap_or_default().to_string(),
                external: e.attr("TargetMode") == Some("External"),
            })
            .collect())
    }

    /// Add a relationship to `part`, returning the new id. Reuses an identical existing one.
    pub fn add_rel(&mut self, part: &str, rel_type: &str, target: &str, external: bool) -> String {
        let path = rels_path(part);
        let mut doc = match self.get(&path).and_then(|d| xml::parse(d).ok()) {
            Some(d) => d,
            None => XmlDoc::new(
                Element::new("Relationships")
                    .with_attr("xmlns", "http://schemas.openxmlformats.org/package/2006/relationships"),
            ),
        };
        for e in doc.root.elements() {
            if e.attr("Type") == Some(rel_type) && e.attr("Target") == Some(target) {
                return e.attr("Id").unwrap_or_default().to_string();
            }
        }
        let mut n = doc.root.elements().count() + 1;
        let existing: Vec<String> = doc.root.elements().filter_map(|e| e.attr("Id").map(String::from)).collect();
        let mut id = format!("rId{n}");
        while existing.contains(&id) {
            n += 1;
            id = format!("rId{n}");
        }
        let mut rel = Element::new("Relationship").with_attr("Id", &id).with_attr("Type", rel_type).with_attr("Target", target);
        if external {
            rel.set_attr("TargetMode", "External");
        }
        doc.root.children.push(xml::Node::Element(rel));
        self.set(&path, doc.to_bytes());
        id
    }

    /// Resolve a relationship id of `part` to an absolute part name (or external URL).
    pub fn resolve_rel(&self, part: &str, id: &str) -> Option<Relationship> {
        let rels = self.rels_for(part).ok()?;
        let mut r = rels.into_iter().find(|r| r.id == id)?;
        if !r.external {
            r.target = resolve_target(part, &r.target);
        }
        Some(r)
    }

    /// Ensure `[Content_Types].xml` declares `part` with the given content type.
    pub fn ensure_override(&mut self, part: &str, content_type: &str) {
        let Ok(mut doc) = self.xml("[Content_Types].xml") else { return };
        let pn = format!("/{}", part.trim_start_matches('/'));
        if doc.root.elements().any(|e| e.local() == "Override" && e.attr("PartName") == Some(pn.as_str())) {
            return;
        }
        doc.root
            .children
            .push(xml::Node::Element(Element::new("Override").with_attr("PartName", pn).with_attr("ContentType", content_type)));
        self.set("[Content_Types].xml", doc.to_bytes());
    }

    pub fn ensure_default(&mut self, ext: &str, content_type: &str) {
        let Ok(mut doc) = self.xml("[Content_Types].xml") else { return };
        let ext_l = ext.to_ascii_lowercase();
        if doc.root.elements().any(|e| e.local() == "Default" && e.attr("Extension").map(|x| x.to_ascii_lowercase()) == Some(ext_l.clone())) {
            return;
        }
        let el = Element::new("Default").with_attr("Extension", ext_l).with_attr("ContentType", content_type);
        // Defaults conventionally precede overrides.
        let pos = doc.root.children.iter().position(|n| matches!(n, xml::Node::Element(e) if e.local() == "Override")).unwrap_or(doc.root.children.len());
        doc.root.children.insert(pos, xml::Node::Element(el));
        self.set("[Content_Types].xml", doc.to_bytes());
    }
}

fn is_precompressed(name: &str) -> bool {
    let l = name.to_ascii_lowercase();
    [".png", ".jpg", ".jpeg", ".gif", ".zip", ".mp4", ".webp"].iter().any(|e| l.ends_with(e))
}

pub fn rels_path(part: &str) -> String {
    let part = part.trim_start_matches('/');
    if part.is_empty() {
        return "_rels/.rels".into();
    }
    match part.rfind('/') {
        Some(i) => format!("{}/_rels/{}.rels", &part[..i], &part[i + 1..]),
        None => format!("_rels/{part}.rels"),
    }
}

/// Resolve a relationship target relative to the source part's folder.
pub fn resolve_target(source_part: &str, target: &str) -> String {
    if let Some(abs) = target.strip_prefix('/') {
        return abs.to_string();
    }
    let base = match source_part.rfind('/') {
        Some(i) => &source_part[..i],
        None => "",
    };
    let mut segs: Vec<&str> = if base.is_empty() { Vec::new() } else { base.split('/').collect() };
    for s in target.split('/') {
        match s {
            "" | "." => {}
            ".." => {
                segs.pop();
            }
            s => segs.push(s),
        }
    }
    segs.join("/")
}

/// Write parts (name, data, stored) as a zip: the same bytes whether the parts come from a package or from `zip_packed`.
fn write_zip<'a>(entries: impl IntoIterator<Item = (&'a str, Data<'a>, bool)>) -> Result<Vec<u8>, PackageError> {
    let mut w = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, data, stored) in entries {
        let method = if stored { zip::CompressionMethod::Stored } else { zip::CompressionMethod::Deflated };
        let opts = zip::write::SimpleFileOptions::default().compression_method(method).last_modified_time(zip::DateTime::default());
        w.start_file(name, opts).map_err(|e| PackageError::Zip(e.to_string()))?;
        // One write per part: the compressor's output depends on how the input is cut into writes, so a part kept as pieces
        // is joined (one part at a time, for the length of its write) to give the bytes it always gave. Only this route,
        // never the browser's (`pack_pieces`, then `zip_packed` on one buffer on another thread).
        let whole = data.to_cow();
        w.write_all(&whole).map_err(|e| PackageError::Io(e.to_string()))?;
    }
    let cur = w.finish().map_err(|e| PackageError::Zip(e.to_string()))?;
    Ok(cur.into_inner())
}

/// The zip of parts made by `Package::pack_with`: byte for byte what `to_bytes_with` writes. Needs no document, so it can run on
/// another thread with its own copy of the engine.
pub fn zip_packed(packed: &[u8]) -> Result<Vec<u8>, PackageError> {
    fn take<'a>(rest: &mut &'a [u8], n: usize) -> Result<&'a [u8], PackageError> {
        if rest.len() < n {
            return Err(PackageError::Io("the packed parts are cut short".into()));
        }
        let (head, tail) = rest.split_at(n);
        *rest = tail;
        Ok(head)
    }
    fn len(rest: &mut &[u8]) -> Result<usize, PackageError> {
        Ok(u32::from_le_bytes(take(rest, 4)?.try_into().unwrap()) as usize)
    }
    let mut entries = Vec::new();
    let mut rest = packed;
    while !rest.is_empty() {
        let stored = take(&mut rest, 1)?[0] == 1;
        let n = len(&mut rest)?;
        let name = std::str::from_utf8(take(&mut rest, n)?).map_err(|e| PackageError::Io(e.to_string()))?;
        let n = len(&mut rest)?;
        entries.push((name, Data::One(take(&mut rest, n)?), stored));
    }
    write_zip(entries)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn paths() {
        assert_eq!(rels_path("word/document.xml"), "word/_rels/document.xml.rels");
        assert_eq!(rels_path(""), "_rels/.rels");
        assert_eq!(resolve_target("word/document.xml", "media/image1.png"), "word/media/image1.png");
        assert_eq!(resolve_target("word/document.xml", "../customXml/item1.xml"), "customXml/item1.xml");
        assert_eq!(resolve_target("", "word/document.xml"), "word/document.xml");
    }
}
