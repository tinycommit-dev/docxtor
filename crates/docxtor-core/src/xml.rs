//! A small, lossless XML DOM.
//!
//! Docxtor's fidelity guarantee rests on this module: every element, attribute,
//! comment and whitespace node that we read is kept and written back, even when
//! the editor has no idea what it means. Names are stored exactly as they appear
//! in the source (`w:p`, `w14:paraId`, ...).

use quick_xml::events::Event;
use quick_xml::Reader;
use std::borrow::Borrow;
use std::collections::HashSet;
use std::fmt::Write as _;
use std::sync::{Mutex, OnceLock};

/// An element or attribute name. A real file has a few hundred distinct names and tens of thousands of uses, so the names the
/// parser reads are shared: each distinct one is kept once for the whole process and every use points at it, where a `String`
/// per use was one allocation each. Names written in code (`"w:p"`) are `&'static str` already and cost nothing.
/// Equal text is equal whichever way the name is held.
#[derive(Clone)]
pub struct Name(Repr);

#[derive(Clone)]
enum Repr {
    Static(&'static str),
    Owned(Box<str>),
}

/// Most distinct names and most bytes the interner keeps. A damaged or made-up file can carry any number of different names, and
/// what is kept is never freed, so past these a name is an ordinary owned one (no leak) and costs what a `String` did.
const INTERN_MAX_NAMES: usize = 32_768;
const INTERN_MAX_BYTES: usize = 1 << 20;

struct Interner {
    names: HashSet<&'static str>,
    bytes: usize,
}

impl Name {
    /// The shared name with this text; the first time a text is seen it is kept for good (up to the limits above).
    pub fn intern(s: &str) -> Name {
        static INTERNER: OnceLock<Mutex<Interner>> = OnceLock::new();
        let m = INTERNER.get_or_init(|| Mutex::new(Interner { names: HashSet::new(), bytes: 0 }));
        let mut g = m.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(k) = g.names.get(s) {
            return Name(Repr::Static(k));
        }
        if g.names.len() >= INTERN_MAX_NAMES || g.bytes + s.len() > INTERN_MAX_BYTES {
            return Name(Repr::Owned(s.into()));
        }
        let k: &'static str = Box::leak(s.to_string().into_boxed_str());
        g.names.insert(k);
        g.bytes += k.len();
        Name(Repr::Static(k))
    }

    pub fn as_str(&self) -> &str {
        match &self.0 {
            Repr::Static(s) => s,
            Repr::Owned(s) => s,
        }
    }
}

impl From<&'static str> for Name {
    fn from(s: &'static str) -> Name {
        Name(Repr::Static(s))
    }
}
impl From<&Name> for Name {
    fn from(n: &Name) -> Name {
        n.clone()
    }
}
impl From<String> for Name {
    fn from(s: String) -> Name {
        Name::intern(&s)
    }
}
impl From<&String> for Name {
    fn from(s: &String) -> Name {
        Name::intern(s)
    }
}
impl From<Name> for String {
    fn from(n: Name) -> String {
        n.as_str().to_string()
    }
}
impl Default for Name {
    fn default() -> Name {
        Name(Repr::Static(""))
    }
}
impl std::ops::Deref for Name {
    type Target = str;
    fn deref(&self) -> &str {
        self.as_str()
    }
}
impl AsRef<str> for Name {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}
impl Borrow<str> for Name {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}
impl PartialEq for Name {
    fn eq(&self, o: &Name) -> bool {
        self.as_str() == o.as_str()
    }
}
impl Eq for Name {}
impl std::hash::Hash for Name {
    fn hash<H: std::hash::Hasher>(&self, h: &mut H) {
        self.as_str().hash(h)
    }
}
impl PartialOrd for Name {
    fn partial_cmp(&self, o: &Name) -> Option<std::cmp::Ordering> {
        Some(self.cmp(o))
    }
}
impl Ord for Name {
    fn cmp(&self, o: &Name) -> std::cmp::Ordering {
        self.as_str().cmp(o.as_str())
    }
}
impl PartialEq<str> for Name {
    fn eq(&self, o: &str) -> bool {
        self.as_str() == o
    }
}
impl PartialEq<&str> for Name {
    fn eq(&self, o: &&str) -> bool {
        self.as_str() == *o
    }
}
impl PartialEq<String> for Name {
    fn eq(&self, o: &String) -> bool {
        self.as_str() == o.as_str()
    }
}
impl PartialEq<Name> for str {
    fn eq(&self, o: &Name) -> bool {
        self == o.as_str()
    }
}
impl PartialEq<Name> for &str {
    fn eq(&self, o: &Name) -> bool {
        *self == o.as_str()
    }
}
impl PartialEq<Name> for String {
    fn eq(&self, o: &Name) -> bool {
        self.as_str() == o.as_str()
    }
}
impl std::fmt::Display for Name {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(self.as_str(), f)
    }
}
impl std::fmt::Debug for Name {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(self.as_str(), f)
    }
}

/// An attribute value. Nearly every value in a Word file is short (a revision id, `22`, `en-US`, `auto`), so a value of up to
/// `INLINE` bytes is kept inside the pair with no allocation at all; a longer one is a `Box<str>` of exactly its length.
/// No wider than a `String` on any target, so a pair is never larger than it was.
#[derive(Clone)]
pub struct AttrValue(ValRepr);

/// An attribute: its name and its value.
pub type Attr = (Name, AttrValue);

/// Bytes kept inline: the room a `String` takes, less the tag and the length.
const INLINE: usize = std::mem::size_of::<String>() - 2;

#[derive(Clone)]
enum ValRepr {
    Inline { len: u8, bytes: [u8; INLINE] },
    Heap(Box<str>),
}

const _: () = assert!(std::mem::size_of::<AttrValue>() <= std::mem::size_of::<String>());

impl AttrValue {
    // This and `From<&str>` stay out of line: inlined at the hundreds of places that read or make a value, they made the
    // engine 29 KB larger (brotli); out of line, 8.6 KB.
    #[inline(never)]
    pub fn as_str(&self) -> &str {
        match &self.0 {
            ValRepr::Inline { len, bytes } => std::str::from_utf8(&bytes[..*len as usize]).unwrap_or_default(),
            ValRepr::Heap(s) => s,
        }
    }
}
impl From<&str> for AttrValue {
    #[inline(never)]
    fn from(s: &str) -> AttrValue {
        if s.len() <= INLINE {
            let mut bytes = [0u8; INLINE];
            bytes[..s.len()].copy_from_slice(s.as_bytes());
            AttrValue(ValRepr::Inline { len: s.len() as u8, bytes })
        } else {
            AttrValue(ValRepr::Heap(s.into()))
        }
    }
}
impl From<String> for AttrValue {
    fn from(s: String) -> AttrValue {
        if s.len() <= INLINE {
            AttrValue::from(s.as_str())
        } else {
            AttrValue(ValRepr::Heap(s.into_boxed_str()))
        }
    }
}
impl From<&String> for AttrValue {
    fn from(s: &String) -> AttrValue {
        AttrValue::from(s.as_str())
    }
}
impl From<&AttrValue> for AttrValue {
    fn from(v: &AttrValue) -> AttrValue {
        v.clone()
    }
}
impl From<AttrValue> for String {
    fn from(v: AttrValue) -> String {
        match v.0 {
            ValRepr::Heap(s) => s.into_string(),
            ValRepr::Inline { .. } => v.as_str().to_string(),
        }
    }
}
impl Default for AttrValue {
    fn default() -> AttrValue {
        AttrValue(ValRepr::Inline { len: 0, bytes: [0; INLINE] })
    }
}
impl std::ops::Deref for AttrValue {
    type Target = str;
    fn deref(&self) -> &str {
        self.as_str()
    }
}
impl AsRef<str> for AttrValue {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}
impl Borrow<str> for AttrValue {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}
impl PartialEq for AttrValue {
    fn eq(&self, o: &AttrValue) -> bool {
        self.as_str() == o.as_str()
    }
}
impl Eq for AttrValue {}
impl std::hash::Hash for AttrValue {
    fn hash<H: std::hash::Hasher>(&self, h: &mut H) {
        self.as_str().hash(h)
    }
}
impl PartialOrd for AttrValue {
    fn partial_cmp(&self, o: &AttrValue) -> Option<std::cmp::Ordering> {
        Some(self.cmp(o))
    }
}
impl Ord for AttrValue {
    fn cmp(&self, o: &AttrValue) -> std::cmp::Ordering {
        self.as_str().cmp(o.as_str())
    }
}
impl PartialEq<str> for AttrValue {
    fn eq(&self, o: &str) -> bool {
        self.as_str() == o
    }
}
impl PartialEq<&str> for AttrValue {
    fn eq(&self, o: &&str) -> bool {
        self.as_str() == *o
    }
}
impl PartialEq<String> for AttrValue {
    fn eq(&self, o: &String) -> bool {
        self.as_str() == o.as_str()
    }
}
impl PartialEq<AttrValue> for str {
    fn eq(&self, o: &AttrValue) -> bool {
        self == o.as_str()
    }
}
impl PartialEq<AttrValue> for &str {
    fn eq(&self, o: &AttrValue) -> bool {
        *self == o.as_str()
    }
}
impl PartialEq<AttrValue> for String {
    fn eq(&self, o: &AttrValue) -> bool {
        self.as_str() == o.as_str()
    }
}
impl std::fmt::Display for AttrValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(self.as_str(), f)
    }
}
impl std::fmt::Debug for AttrValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(self.as_str(), f)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Node {
    Element(Element),
    Text(String),
    CData(String),
    Comment(String),
    PI(String),
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Element {
    pub name: Name,
    pub attrs: Vec<Attr>,
    pub children: Vec<Node>,
}

/// A parsed XML part: the prolog (declaration, doctype, leading comments) and the root.
#[derive(Debug, Clone, PartialEq)]
pub struct XmlDoc {
    pub decl: Option<String>,
    /// What came right after the declaration (`\n` in files made by many tools, `\r\n` in Word's): written back as it was, so a part
    /// written again with nothing changed is the part that came. A new part has none and gets `\r\n`.
    pub decl_end: Option<String>,
    pub prolog: Vec<Node>,
    pub root: Element,
}

#[derive(Debug, thiserror::Error)]
pub enum XmlError {
    #[error("xml parse error: {0}")]
    Parse(String),
    #[error("xml document has no root element")]
    NoRoot,
    /// Elements, or blocks, nest deeper than `budget::XML_DEPTH` / `budget::BLOCK_DEPTH` allow.
    #[error("This file is too large or too deeply nested to open.")]
    TooDeep,
}

impl Element {
    pub fn new(name: impl Into<Name>) -> Self {
        Element { name: name.into(), attrs: Vec::new(), children: Vec::new() }
    }

    pub fn with_attr(mut self, k: impl Into<Name>, v: impl Into<AttrValue>) -> Self {
        self.set_attr(k, v);
        self
    }

    pub fn with_child(mut self, child: Element) -> Self {
        self.children.push(Node::Element(child));
        self
    }

    pub fn with_text(mut self, text: impl Into<String>) -> Self {
        self.children.push(Node::Text(text.into()));
        self
    }

    /// Local part of the name (`p` for `w:p`).
    pub fn local(&self) -> &str {
        local_name(&self.name)
    }

    pub fn is(&self, name: &str) -> bool {
        self.name == name
    }

    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attrs.iter().find(|(k, _)| k == name).map(|(_, v)| v.as_str())
    }

    pub fn set_attr(&mut self, k: impl Into<Name>, v: impl Into<AttrValue>) {
        let k = k.into();
        let v = v.into();
        if let Some(slot) = self.attrs.iter_mut().find(|(ek, _)| *ek == k) {
            slot.1 = v;
        } else {
            self.attrs.push((k, v));
        }
    }

    pub fn remove_attr(&mut self, k: &str) -> Option<String> {
        let idx = self.attrs.iter().position(|(ek, _)| ek == k)?;
        Some(String::from(self.attrs.remove(idx).1))
    }

    pub fn elements(&self) -> impl Iterator<Item = &Element> {
        self.children.iter().filter_map(|n| match n {
            Node::Element(e) => Some(e),
            _ => None,
        })
    }

    pub fn elements_mut(&mut self) -> impl Iterator<Item = &mut Element> {
        self.children.iter_mut().filter_map(|n| match n {
            Node::Element(e) => Some(e),
            _ => None,
        })
    }

    pub fn child(&self, name: &str) -> Option<&Element> {
        self.elements().find(|e| e.name == name)
    }

    pub fn child_mut(&mut self, name: &str) -> Option<&mut Element> {
        self.elements_mut().find(|e| e.name == name)
    }

    pub fn children_named<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a Element> + 'a {
        self.elements().filter(move |e| e.name == name)
    }

    /// Follow a path of child names, e.g. `["w:body", "w:sectPr"]`.
    pub fn path(&self, path: &[&str]) -> Option<&Element> {
        let mut cur = self;
        for p in path {
            cur = cur.child(p)?;
        }
        Some(cur)
    }

    /// Remove every child element with this name; returns how many were removed.
    pub fn remove_children(&mut self, name: &str) -> usize {
        let before = self.children.len();
        self.children.retain(|n| !matches!(n, Node::Element(e) if e.name == name));
        before - self.children.len()
    }

    /// Concatenated text content of this element and all descendants.
    pub fn text(&self) -> String {
        let mut out = String::new();
        self.collect_text(&mut out);
        out
    }

    fn collect_text(&self, out: &mut String) {
        for c in &self.children {
            match c {
                Node::Text(t) | Node::CData(t) => out.push_str(t),
                Node::Element(e) => e.collect_text(out),
                _ => {}
            }
        }
    }

    /// Depth-first search for the first descendant with the given name.
    pub fn find(&self, name: &str) -> Option<&Element> {
        for e in self.elements() {
            if e.name == name {
                return Some(e);
            }
            if let Some(f) = e.find(name) {
                return Some(f);
            }
        }
        None
    }

    pub fn find_all<'a>(&'a self, name: &str, out: &mut Vec<&'a Element>) {
        for e in self.elements() {
            if e.name == name {
                out.push(e);
            }
            e.find_all(name, out);
        }
    }

    /// Serialize this element (no XML declaration).
    pub fn to_xml(&self) -> String {
        let mut out = String::new();
        write_element(self, &mut out);
        out
    }
}

pub fn local_name(name: &str) -> &str {
    match name.rfind(':') {
        Some(i) => &name[i + 1..],
        None => name,
    }
}

/// Parse a complete XML part.
pub fn parse(bytes: &[u8]) -> Result<XmlDoc, XmlError> {
    // The two work vectors are kept from one parse to the next (small parts are read often: an undo reads one), unless a big
    // part made them big.
    type Scratch = (Vec<Node>, Vec<Attr>);
    thread_local! {
        static SCRATCH: std::cell::RefCell<Scratch> = const { std::cell::RefCell::new((Vec::new(), Vec::new())) };
    }
    let (mut kids, mut attr_scratch) = SCRATCH.with(|s| std::mem::take(&mut *s.borrow_mut()));
    let out = parse_with(bytes, &mut kids, &mut attr_scratch);
    kids.clear();
    attr_scratch.clear();
    if kids.capacity() <= 4096 && attr_scratch.capacity() <= 256 {
        SCRATCH.with(|s| *s.borrow_mut() = (kids, attr_scratch));
    }
    out
}

fn parse_with(bytes: &[u8], kids: &mut Vec<Node>, attr_scratch: &mut Vec<Attr>) -> Result<XmlDoc, XmlError> {
    let text = decode_bytes(bytes);
    let mut reader = Reader::from_str(&text);
    {
        let cfg = reader.config_mut();
        cfg.trim_text(false);
        cfg.expand_empty_elements = false;
        cfg.check_end_names = false;
    }
    let mut decl = None;
    let mut decl_end: Option<String> = None;
    let mut prolog = Vec::new();
    // Open elements, each with where its children start in `kids`: the children of every open element wait there and are
    // moved into a vector of exactly their length when the element closes (a vector grown by `push` holds up to twice what it
    // needs, and most elements have one or two children and attributes).
    let mut stack: Vec<(Element, usize)> = Vec::new();
    let mut root: Option<Element> = None;
    // Elements and blocks nest only so deep (see `crate::budget`): every part of every format passes here,
    // so the limit that keeps clone, drop, render and save inside a small stack lives here.
    let mut nesting = 0usize;

    loop {
        let ev = reader.read_event().map_err(|e| XmlError::Parse(format!("{e} at {}", reader.buffer_position())))?;
        match ev {
            Event::Decl(d) => {
                decl = Some(d.as_ref().trim().trim_start_matches("xml").trim().to_string());
            }
            Event::Start(s) => {
                // A declaration followed by nothing before the root: nothing is written between them either.
                if stack.is_empty() && root.is_none() && prolog.is_empty() && decl.is_some() && decl_end.is_none() {
                    decl_end = Some(String::new());
                }
                let el = start_to_element(&s, attr_scratch)?;
                nesting += crate::budget::nest_cost(&el.name);
                if stack.len() >= crate::budget::XML_DEPTH || nesting > crate::budget::NEST_LIMIT {
                    crate::budget::mark_xml_overflow();
                    return Err(XmlError::TooDeep);
                }
                stack.push((el, kids.len()));
            }
            Event::Empty(s) => {
                let el = start_to_element(&s, attr_scratch)?;
                push_node(&stack, kids, &mut root, &mut prolog, Node::Element(el));
            }
            Event::End(_) => {
                if let Some(el) = close_element(&mut stack, kids) {
                    nesting = nesting.saturating_sub(crate::budget::nest_cost(&el.name));
                    push_node(&stack, kids, &mut root, &mut prolog, Node::Element(el));
                }
            }
            Event::Text(t) => {
                let s = t.xml10_content();
                // The white space between the declaration and what follows it.
                // (As it is in the file: the text read here has its line ends made `\n`, and Word's are `\r\n`.)
                if stack.is_empty() && root.is_none() && prolog.is_empty() && decl.is_some() && decl_end.is_none() && s.trim().is_empty() {
                    decl_end = Some(t.into_inner().into_owned());
                }
                append_text(&stack, kids, &s);
            }
            Event::GeneralRef(r) => {
                let resolved: String = if let Ok(Some(ch)) = r.resolve_char_ref() {
                    ch.to_string()
                } else {
                    match r.as_ref() {
                        "amp" => "&".into(),
                        "lt" => "<".into(),
                        "gt" => ">".into(),
                        "quot" => "\"".into(),
                        "apos" => "'".into(),
                        other => format!("&{other};"),
                    }
                };
                append_text(&stack, kids, &resolved);
            }
            Event::CData(c) => {
                let s = c.into_inner().into_owned();
                push_child(&stack, kids, &mut prolog, Node::CData(s));
            }
            Event::Comment(c) => {
                let s = c.into_inner().into_owned();
                push_child(&stack, kids, &mut prolog, Node::Comment(s));
            }
            Event::PI(p) => {
                let s = p.as_ref().to_string();
                push_child(&stack, kids, &mut prolog, Node::PI(s));
            }
            Event::DocType(_) => {}
            Event::Eof => break,
        }
    }
    // Unclosed elements: close them gracefully rather than failing.
    while let Some(el) = close_element(&mut stack, kids) {
        push_node(&stack, kids, &mut root, &mut prolog, Node::Element(el));
    }
    let root = root.ok_or(XmlError::NoRoot)?;
    Ok(XmlDoc { decl, decl_end, prolog, root })
}

fn decode_bytes(bytes: &[u8]) -> String {
    // UTF-8 BOM
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        return String::from_utf8_lossy(&bytes[3..]).into_owned();
    }
    // UTF-16 LE/BE (rare in docx, but legal)
    if bytes.starts_with(&[0xFF, 0xFE]) || bytes.starts_with(&[0xFE, 0xFF]) {
        let le = bytes[0] == 0xFF;
        let units: Vec<u16> = bytes[2..]
            .chunks_exact(2)
            .map(|c| if le { u16::from_le_bytes([c[0], c[1]]) } else { u16::from_be_bytes([c[0], c[1]]) })
            .collect();
        let s = String::from_utf16_lossy(&units);
        // Our serializer always writes UTF-8, so drop the declared encoding.
        return s.replacen("encoding=\"UTF-16\"", "encoding=\"UTF-8\"", 1);
    }
    String::from_utf8_lossy(bytes).into_owned()
}

fn start_to_element(s: &quick_xml::events::BytesStart, scratch: &mut Vec<Attr>) -> Result<Element, XmlError> {
    let name = Name::intern(s.name().as_ref());
    for a in s.attributes().with_checks(false) {
        let a = a.map_err(|e| XmlError::Parse(e.to_string()))?;
        let key = Name::intern(a.key.as_ref());
        let val = match a.normalized_value(quick_xml::XmlVersion::Implicit1_0) {
            Ok(std::borrow::Cow::Borrowed(v)) => AttrValue::from(v),
            Ok(std::borrow::Cow::Owned(v)) => AttrValue::from(v),
            Err(_) => AttrValue::from(a.value.to_string()),
        };
        scratch.push((key, val));
    }
    // Exactly as long as it is (a vector grown by `push` would hold room for four to begin with); `mem::take` would keep the
    // scratch's room in the element and start the scratch over.
    #[allow(clippy::drain_collect)]
    let attrs: Vec<Attr> = scratch.drain(..).collect();
    Ok(Element { name, attrs, children: Vec::new() })
}

/// Close the innermost open element: its children leave `kids` for a vector of exactly their length.
fn close_element(stack: &mut Vec<(Element, usize)>, kids: &mut Vec<Node>) -> Option<Element> {
    let (mut el, start) = stack.pop()?;
    el.children = kids.drain(start..).collect();
    Some(el)
}

fn push_node(stack: &[(Element, usize)], kids: &mut Vec<Node>, root: &mut Option<Element>, prolog: &mut Vec<Node>, node: Node) {
    if !stack.is_empty() {
        kids.push(node);
    } else if let Node::Element(e) = node {
        if root.is_none() {
            *root = Some(e);
        }
    } else {
        prolog.push(node);
    }
}

fn push_child(stack: &[(Element, usize)], kids: &mut Vec<Node>, prolog: &mut Vec<Node>, node: Node) {
    if !stack.is_empty() {
        kids.push(node);
    } else {
        prolog.push(node);
    }
}

fn append_text(stack: &[(Element, usize)], kids: &mut Vec<Node>, s: &str) {
    let Some((_, start)) = stack.last() else { return };
    if kids.len() > *start {
        if let Some(Node::Text(prev)) = kids.last_mut() {
            prev.push_str(s);
            return;
        }
    }
    kids.push(Node::Text(s.to_string()));
}

impl XmlDoc {
    pub fn new(root: Element) -> Self {
        XmlDoc {
            decl: Some(r#"version="1.0" encoding="UTF-8" standalone="yes""#.into()),
            decl_end: None,
            prolog: Vec::new(),
            root,
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = String::with_capacity(4096);
        if let Some(d) = &self.decl {
            let _ = write!(out, "<?xml {d}?>{}", self.decl_end.as_deref().unwrap_or("\r\n"));
        }
        for n in &self.prolog {
            write_node(n, &mut out);
        }
        write_element(&self.root, &mut out);
        out.into_bytes()
    }
}

fn write_node(n: &Node, out: &mut String) {
    match n {
        Node::Element(e) => write_element(e, out),
        Node::Text(t) => escape_text(t, out),
        Node::CData(t) => {
            out.push_str("<![CDATA[");
            out.push_str(t);
            out.push_str("]]>");
        }
        Node::Comment(t) => {
            out.push_str("<!--");
            out.push_str(t);
            out.push_str("-->");
        }
        Node::PI(t) => {
            out.push_str("<?");
            out.push_str(t);
            out.push_str("?>");
        }
    }
}

/// The element as it is written inside a part (what `XmlDoc::to_bytes` writes for it).
pub fn write_element_into(e: &Element, out: &mut String) {
    write_element(e, out)
}

fn write_element(e: &Element, out: &mut String) {
    out.push('<');
    out.push_str(&e.name);
    for (k, v) in &e.attrs {
        out.push(' ');
        out.push_str(k);
        out.push_str("=\"");
        escape_attr(v, out);
        out.push('"');
    }
    if e.children.is_empty() {
        out.push_str("/>");
        return;
    }
    out.push('>');
    for c in &e.children {
        write_node(c, out);
    }
    out.push_str("</");
    out.push_str(&e.name);
    out.push('>');
}

fn escape_text(s: &str, out: &mut String) {
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            c => out.push(c),
        }
    }
}

fn escape_attr(s: &str, out: &mut String) {
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\n' => out.push_str("&#10;"),
            '\r' => out.push_str("&#13;"),
            '\t' => out.push_str("&#9;"),
            c => out.push(c),
        }
    }
}


/// Verify that every prefix used by an element or attribute is declared in scope.
/// Word (and any namespace-aware parser) rejects parts that fail this check.
pub fn check_namespaces(doc: &XmlDoc) -> Result<(), String> {
    fn go(e: &Element, scope: &mut Vec<String>) -> Result<(), String> {
        let before = scope.len();
        for (k, _) in &e.attrs {
            if let Some(p) = k.strip_prefix("xmlns:") {
                scope.push(p.to_string());
            }
        }
        let check = |name: &str, scope: &[String]| -> Result<(), String> {
            if let Some((p, _)) = name.split_once(':') {
                if p != "xml" && p != "xmlns" && !scope.iter().any(|s| s == p) {
                    return Err(format!("undeclared namespace prefix '{p}' on <{name}>"));
                }
            }
            Ok(())
        };
        check(&e.name, scope)?;
        for (k, _) in &e.attrs {
            check(k, scope).map_err(|m| format!("{m} (attribute of <{}>)", e.name))?;
        }
        for c in e.elements() {
            go(c, scope)?;
        }
        scope.truncate(before);
        Ok(())
    }
    go(&doc.root, &mut Vec::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_preserves_structure() {
        let src = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="urn:w" mc:Ignorable="w14"><!-- hi --><w:body><w:p w14:paraId="1A"><w:r><w:t xml:space="preserve"> a &amp; b &lt;c&gt; &#x263A; </w:t></w:r></w:p><w:unknown foo="x&quot;y"/></w:body></w:document>"#;
        let doc = parse(src.as_bytes()).unwrap();
        let out = doc.to_bytes();
        let again = parse(&out).unwrap();
        assert_eq!(doc, again);
        let t = doc.root.find("w:t").unwrap();
        assert_eq!(t.text(), " a & b <c> \u{263A} ");
        assert_eq!(doc.root.find("w:unknown").unwrap().attr("foo"), Some("x\"y"));
    }
}

#[cfg(test)]
mod decl_end_tests {
    use super::parse;

    #[test]
    fn what_follows_the_declaration_is_written_back_as_it_came() {
        for src in ["<?xml version='1.0' encoding='UTF-8' standalone='yes'?>\n<a><b/></a>", "<?xml version=\"1.0\"?>\r\n<a/>", "<?xml version=\"1.0\"?><a>x</a>"] {
            assert_eq!(String::from_utf8(parse(src.as_bytes()).unwrap().to_bytes()).unwrap(), src);
        }
        // A part made new gets Word's line end.
        assert!(super::XmlDoc::new(parse(b"<a/>").unwrap().root).to_bytes().starts_with(b"<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\r\n<a"));
    }
}
