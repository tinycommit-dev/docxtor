//! Opening files: whatever the file is, it comes out as a Docxtor `Document` and a report of what was done.
//!
//! - A Word file (.docx, .docm, .dotx) opens as it is. A damaged one is repaired first (`repair`): a zip
//!   with a broken or missing index, parts cut short, malformed XML, missing relationships or content types.
//!   Strict Open XML is mapped to the standard (Transitional) kind (`strict`).
//! - Other formats become a new .docx: OpenDocument text (`odt`), RTF and HTML (through the readers the
//!   clipboard uses), Word 2003 XML (`word2003`), Flat OPC (`flat_opc`) and the old binary Word format
//!   (`doc_binary`). Their content is built as a `Fragment` and pasted into a blank document (`text`).
//! - Files that are not Word documents, or that can't be read, give an `OpenError` whose `code` the page
//!   turns into plain language.
//!
//! The report holds codes, not sentences: `Note.code` says what happened and the page words it in the
//! person's language.

pub mod doc_binary;
pub mod flat_opc;
pub mod odt;
pub mod repair;
pub mod strict;
pub mod text;
pub mod word2003;
pub mod xmlfix;
pub mod zipscan;

use crate::budget::{Budget, OverBudget};
use crate::document::Document;
use crate::package::Package;
use serde::Serialize;

/// The kinds of file Docxtor reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Format {
    /// .docx, .docm and .dotx, as Word writes them (Strict Open XML is a note, not a format).
    Docx,
    /// One XML file that holds a whole package (Word's "Flat OPC").
    FlatOpc,
    /// Word 2003 XML (WordprocessingML before Word 2007).
    Word2003,
    /// OpenDocument text (.odt, and the single-file .fodt).
    Odt,
    Rtf,
    Html,
    /// The binary format of Word 97 to 2003 (.doc).
    Doc,
}

impl Format {
    /// Whether opening it makes a new .docx from another kind of file.
    pub fn converted(self) -> bool {
        !matches!(self, Format::Docx)
    }
}

/// One thing that was done or lost while opening.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Note {
    pub code: String,
    /// Names or a short fact the sentence needs (part names, a count's subject).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    pub count: usize,
}

impl Note {
    pub fn new(code: &str) -> Note {
        Note { code: code.into(), detail: None, count: 1 }
    }

    pub fn count(code: &str, count: usize) -> Note {
        Note { code: code.into(), detail: None, count }
    }

    pub fn detail(code: &str, detail: impl Into<String>) -> Note {
        Note { code: code.into(), detail: Some(detail.into()), count: 1 }
    }

    pub fn with_detail(mut self, detail: impl Into<String>) -> Note {
        self.detail = Some(detail.into());
        self
    }
}

/// What opening a file did to it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Report {
    pub format: Format,
    /// A new .docx was made from another kind of file.
    pub converted: bool,
    pub notes: Vec<Note>,
}

impl Report {
    pub fn new(format: Format) -> Report {
        Report { format, converted: format.converted(), notes: Vec::new() }
    }

    pub fn has(&self, code: &str) -> bool {
        self.notes.iter().any(|n| n.code == code)
    }
}

/// A file opened.
pub struct Opened {
    pub doc: Document,
    pub report: Report,
}

/// Why a file could not be opened. `code` is stable; `message` is English for logs and tests.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[error("{message}")]
pub struct OpenError {
    pub code: &'static str,
    /// What the file is, when the code is `not-word` (`spreadsheet`, `presentation`, `pdf`...).
    pub detail: String,
    pub message: String,
}

impl OpenError {
    pub fn new(code: &'static str, message: impl Into<String>) -> OpenError {
        OpenError { code, detail: String::new(), message: message.into() }
    }

    /// The file ran the input budget out: too large, too many parts, or nested too deeply.
    pub fn too_big() -> OpenError {
        OpenError::new("too-big", crate::budget::MESSAGE)
    }

    fn foreign(what: &str) -> OpenError {
        let message = match what {
            "spreadsheet" => "This is a spreadsheet, not a Word document.",
            "presentation" => "This is a presentation, not a Word document.",
            "pdf" => "This is a PDF, and Docxtor can't open PDFs.",
            "epub" => "This is an e-book (EPUB), and Docxtor can't open those.",
            "image" => "This is a picture, not a document.",
            "archive" => "This is an archive, not a document.",
            _ => "This isn't a kind of file Docxtor can open.",
        };
        OpenError { code: "not-word", detail: what.into(), message: message.into() }
    }
}

/// What a file's bytes say it is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Kind {
    /// A zip with a Word package in it (it may be damaged).
    Word,
    Odt,
    FlatOpc,
    Word2003,
    Rtf,
    Html,
    Doc,
    /// A password-protected Word file (a compound file holding an encrypted package).
    Encrypted,
    /// A file that is not a Word document: `spreadsheet`, `presentation`, `pdf`, `epub`, `image`, `archive`, `other-opendocument`.
    Foreign(&'static str),
    Unknown,
}

/// The first few thousand characters of a text file, decoded, for telling XML and HTML apart.
fn head_text(bytes: &[u8]) -> String {
    let n = bytes.len().min(4096);
    let mut end = n;
    // Don't cut a UTF-8 character in half.
    while end > 0 && end < bytes.len() && (bytes[end] & 0xC0) == 0x80 {
        end -= 1;
    }
    xmlfix::decode(&bytes[..end])
}

/// Tell what a file is from its bytes. A zip is opened to look at its parts, so this reads the whole archive.
pub fn sniff(bytes: &[u8]) -> Kind {
    sniff_with(bytes, &Budget::file())
}

/// `sniff`, spending from `budget`.
pub fn sniff_with(bytes: &[u8], budget: &Budget) -> Kind {
    if bytes.is_empty() {
        return Kind::Unknown;
    }
    if crate::cfb::is_cfb(bytes) {
        return sniff_cfb(bytes);
    }
    if zipscan::looks_like_zip(bytes) {
        return match zipscan::read(bytes, budget) {
            Some(z) => classify_zip(&z.entries),
            None => Kind::Unknown,
        };
    }
    if bytes.starts_with(b"%PDF") {
        return Kind::Foreign("pdf");
    }
    if bytes.starts_with(&[0x89, b'P', b'N', b'G']) || bytes.starts_with(&[0xFF, 0xD8, 0xFF]) || bytes.starts_with(b"GIF8") {
        return Kind::Foreign("image");
    }
    let head = head_text(bytes);
    let t = head.trim_start_matches(|c: char| c == '\u{feff}' || c.is_whitespace());
    if t.starts_with("{\\rtf") {
        return Kind::Rtf;
    }
    if t.starts_with('<') {
        let lower = t.to_ascii_lowercase();
        let root = xml_root_name(&lower);
        if root == "pkg:package" {
            return Kind::FlatOpc;
        }
        if root == "w:worddocument" {
            return Kind::Word2003;
        }
        if root == "office:document" && lower.contains("opendocument.text") {
            return Kind::Odt;
        }
        if root == "html" || root == "!doctype" || root == "head" || root == "body" || lower.contains("<html") || lower.contains("<!doctype html") {
            return Kind::Html;
        }
        // Loose HTML fragments: a few known tags near the start.
        if ["<p", "<div", "<h1", "<h2", "<table", "<span", "<ul", "<ol", "<b>", "<i>", "<br"].iter().any(|tag| lower.starts_with(tag)) {
            return Kind::Html;
        }
    }
    Kind::Unknown
}

/// The name of the first element in some XML text (after any declaration, comments and doctype is `!doctype` for HTML's).
fn xml_root_name(lower: &str) -> String {
    let mut rest = lower;
    loop {
        let Some(i) = rest.find('<') else { return String::new() };
        let after = &rest[i + 1..];
        if after.starts_with('?') || after.starts_with("!--") {
            let end = if after.starts_with('?') { "?>" } else { "-->" };
            match after.find(end) {
                Some(e) => {
                    rest = &after[e + end.len()..];
                    continue;
                }
                None => return String::new(),
            }
        }
        if after.starts_with("!doctype") {
            return "!doctype".into();
        }
        let name: String = after.chars().take_while(|c| !c.is_whitespace() && *c != '>' && *c != '/').collect();
        return name;
    }
}

fn sniff_cfb(bytes: &[u8]) -> Kind {
    if crate::office_crypt::is_encrypted(bytes) {
        return Kind::Encrypted;
    }
    let Ok(r) = crate::cfb::CfbReader::new(bytes) else { return Kind::Foreign("other") };
    let paths = r.stream_paths();
    let has = |n: &str| paths.iter().any(|p| p == n);
    if has("WordDocument") {
        Kind::Doc
    } else if has("Workbook") || has("Book") {
        Kind::Foreign("spreadsheet")
    } else if has("PowerPoint Document") {
        Kind::Foreign("presentation")
    } else {
        Kind::Foreign("other")
    }
}

fn classify_zip(entries: &[zipscan::RawEntry]) -> Kind {
    let get = |n: &str| entries.iter().find(|e| e.name == n).map(|e| e.data.as_slice());
    if let Some(m) = get("mimetype") {
        let m = String::from_utf8_lossy(m);
        let m = m.trim();
        if m == "application/epub+zip" {
            return Kind::Foreign("epub");
        }
        if let Some(rest) = m.strip_prefix("application/vnd.oasis.opendocument.") {
            return if rest.starts_with("text") && !rest.starts_with("text-web") { Kind::Odt } else if rest.starts_with("spreadsheet") { Kind::Foreign("spreadsheet") } else if rest.starts_with("presentation") { Kind::Foreign("presentation") } else { Kind::Foreign("other-opendocument") };
        }
    }
    let has_prefix = |p: &str| entries.iter().any(|e| e.name.starts_with(p));
    if let Some(ct) = get("[Content_Types].xml") {
        let ct = String::from_utf8_lossy(ct);
        if ct.contains("wordprocessingml") {
            return Kind::Word;
        }
        if ct.contains("spreadsheetml") {
            return Kind::Foreign("spreadsheet");
        }
        if ct.contains("presentationml") {
            return Kind::Foreign("presentation");
        }
    }
    if has_prefix("word/") {
        return Kind::Word;
    }
    if has_prefix("xl/") {
        return Kind::Foreign("spreadsheet");
    }
    if has_prefix("ppt/") {
        return Kind::Foreign("presentation");
    }
    // An OpenDocument file that lost its `mimetype`.
    if get("content.xml").is_some_and(|c| String::from_utf8_lossy(&c[..c.len().min(2000)]).contains("office:document-content")) {
        return Kind::Odt;
    }
    Kind::Foreign("archive")
}

/// Open any file Docxtor can read. `name` is only used when the bytes alone don't settle it.
pub fn open(bytes: &[u8], name: &str) -> Result<Opened, OpenError> {
    open_with(bytes, name, &Budget::file())
}

/// `open`, spending from `budget`: the one budget of this file, handed to every reader. When it runs out the
/// result is the `too-big` error, whatever reader ran it out.
pub fn open_with(bytes: &[u8], name: &str, budget: &Budget) -> Result<Opened, OpenError> {
    let _ = name;
    if bytes.is_empty() {
        return Err(OpenError::new("empty", "The file is empty."));
    }
    // A file's bytes count as work the first time they are looked at, so a reader can't be handed more than the limits allow.
    budget.spend_work(bytes.len() as u64 / 8).map_err(|_| OpenError::too_big())?;
    let opened = open_kind(bytes, budget);
    // Whatever the reader said, a budget that ran out is the answer: a half-read file is not "damaged".
    if budget.is_out() {
        return Err(OpenError::too_big());
    }
    opened
}

fn open_kind(bytes: &[u8], budget: &Budget) -> Result<Opened, OpenError> {
    match sniff_with_zip(bytes, budget) {
        (Kind::Word, Some(z)) => open_word(z, budget),
        (Kind::Odt, Some(z)) => text::finish(Format::Odt, odt::read_zip(&z.entries)?),
        (Kind::Odt, None) => text::finish(Format::Odt, odt::read_flat(bytes)?),
        (Kind::FlatOpc, _) => {
            let pkg = flat_opc::read(bytes)?;
            let z = zipscan::ZipRead { entries: pkg.parts.into_iter().map(|p| zipscan::RawEntry { name: p.name, data: p.data, stored: p.stored, damaged: false }).collect(), scanned: false };
            let mut o = open_word(z, budget)?;
            o.report.format = Format::FlatOpc;
            o.report.converted = true;
            Ok(o)
        }
        (Kind::Word2003, _) => text::finish(Format::Word2003, word2003::read(bytes)?),
        (Kind::Rtf, _) => text::finish(Format::Rtf, text::from_rtf(bytes)),
        (Kind::Html, _) => text::finish(Format::Html, text::from_html(bytes)),
        (Kind::Doc, _) => text::finish(Format::Doc, doc_binary::read(bytes)?),
        (Kind::Encrypted, _) => Err(OpenError::new("encrypted", "This document is protected with a password.")),
        // Starts like a zip, but nothing at all could be read from it.
        (Kind::Foreign("damaged-zip"), _) => Err(OpenError::new("damaged", "Nothing could be read from the file.")),
        (Kind::Foreign(what), _) => Err(OpenError::foreign(what)),
        _ => Err(OpenError::new("unknown", "Docxtor doesn't recognise this kind of file.")),
    }
}

/// `sniff`, keeping the zip that was read so it isn't read twice.
fn sniff_with_zip(bytes: &[u8], budget: &Budget) -> (Kind, Option<zipscan::ZipRead>) {
    if zipscan::looks_like_zip(bytes) {
        return match zipscan::read(bytes, budget) {
            Some(z) => (classify_zip(&z.entries), Some(z)),
            None => (Kind::Foreign("damaged-zip"), None),
        };
    }
    // A zip whose first bytes are gone can still be scanned when there are local headers further on.
    if !crate::cfb::is_cfb(bytes) && bytes.windows(4).take(1 << 20).any(|w| w == [0x50, 0x4B, 0x03, 0x04]) && !bytes.starts_with(b"{\\rtf") && sniff(&bytes[..bytes.len().min(512)]) == Kind::Unknown {
        let z = zipscan::scan(bytes, budget);
        if !z.entries.is_empty() {
            return (classify_zip(&z.entries), Some(z));
        }
    }
    (sniff(bytes), None)
}

fn open_word(z: zipscan::ZipRead, budget: &Budget) -> Result<Opened, OpenError> {
    budget.check().map_err(|_| OpenError::too_big())?;
    let (pkg, notes) = repair::load_package(z)?;
    let mut report = Report::new(Format::Docx);
    report.notes = notes;
    let doc = Document::from_package_with(pkg, budget).map_err(|e| match e {
        crate::document::DocError::NoBody => OpenError::new("no-text", "The document has no text part Docxtor can read."),
        other => OpenError::new("damaged", other.to_string()),
    })?;
    // Charts and pictures of imported content that the page can't show: said once, not found out by editing.
    report.notes.extend(doc.alt_chunk_notes().into_iter().map(|(code, n)| Note::count(code, n)));
    Ok(Opened { doc, report })
}

/// Open any file Docxtor can read and write it as .docx bytes.
pub fn to_docx(bytes: &[u8], name: &str) -> Result<(Vec<u8>, Report), OpenError> {
    let Opened { mut doc, report } = open(bytes, name)?;
    let out = doc.save().map_err(|e| OpenError::new("damaged", e.to_string()))?;
    Ok((out, report))
}

/// The package of a Word file as `Document::open` wants it: repaired and in the standard form.
pub fn open_package(bytes: &[u8], budget: &Budget) -> Result<(Package, Vec<Note>), OpenError> {
    crate::perf::plant_open();
    let z = zipscan::read(bytes, budget).ok_or_else(|| OpenError::new("damaged-zip", "The file is not a readable zip."))?;
    budget.check().map_err(|_: OverBudget| OpenError::too_big())?;
    repair::load_package(z)
}

/// A note in plain English, for the command line and logs. The page words the same notes in the person's language.
pub fn describe(n: &Note) -> String {
    let d = n.detail.as_deref().unwrap_or("");
    let c = n.count;
    let parts = |c: usize| if c == 1 { "part" } else { "parts" };
    match n.code.as_str() {
        "zip-directory" => format!("The file's index was damaged. Docxtor rebuilt it and found {c} {}.", parts(c)),
        "zip-damaged" => format!("{c} {} were cut short or damaged and were read as far as possible: {d}.", parts(c)),
        "xml-recovered" => format!("{c} {} had damaged markup. Docxtor kept what could be read: {d}.", parts(c)),
        "xml-unreadable" => format!("{c} {} could not be read and were left as they are: {d}.", parts(c)),
        "text-cut" => "The text of the document ends early. What came after the damage is lost.".into(),
        "parts-missing" => format!("{c} pictures or other parts the document refers to are missing."),
        "rels-restored" => "Links between the document's parts were missing and have been put back.".into(),
        "types-restored" => "The file's list of part types was incomplete and has been completed.".into(),
        "strict" => "This file uses Strict Open XML. Docxtor read it as the standard kind and saves it as that.".into(),
        "picture-unreadable" => format!("{c} pictures could not be read and were left out."),
        "picture-unsupported" => format!("{c} pictures are in a format Docxtor can't show and were left out."),
        "pictures-linked" => format!("{c} pictures are linked to files outside this one and were left out."),
        "chunk-objects" => format!("{c} charts or embedded objects in imported content are not shown, and editing that content removes them."),
        "object-dropped" => format!("{c} drawings or embedded objects were left out."),
        "lost-drawings" => format!("{c} drawn shapes or floating pictures were left out."),
        "lost-text-boxes" => "Text boxes were turned into ordinary paragraphs, or left out.".into(),
        "lost-headers" => "Headers and footers were left out.".into(),
        "lost-comments" => "Comments were left out.".into(),
        "doc-word6" => "This is a Word 6 or Word 95 file. Only its text was read.".into(),
        "nothing-found" => "No text was found in the file.".into(),
        other => other.to_string(),
    }
}
