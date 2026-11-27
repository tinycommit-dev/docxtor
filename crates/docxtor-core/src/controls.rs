//! Content controls (`w:sdt`): reading what a control is, building the markup Word writes for each kind, and the
//! small text and date helpers the ops need. `document/controls.rs` applies it to a document.
//!
//! A control stays the `w:sdt` element Word wrote: the model keeps its `w:sdtPr` in the shell and its content as
//! ordinary runs or blocks, so editing the content is editing text. Only what Docxtor changes is rewritten.

use crate::model::*;
use crate::ops::{Id, Pos};
use crate::package::{self, Package};
use crate::xml::{self, Element, Node};
use serde::{Deserialize, Serialize};

pub const NS_W14: &str = "http://schemas.microsoft.com/office/word/2010/wordml";
pub const NS_W15: &str = "http://schemas.microsoft.com/office/word/2012/wordml";
pub const REL_GLOSSARY: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/glossaryDocument";
pub const CT_GLOSSARY: &str = "application/vnd.openxmlformats-officedocument.wordprocessingml.document.glossary+xml";

/// The order of the children of `w:sdtPr`. The extension elements of Word 2010 and 2013 sit where Word puts them.
pub const SDTPR_ORDER: &[&str] = &[
    "w:rPr", "w:alias", "w:tag", "w:id", "w:lock", "w:placeholder", "w:temporary", "w:showingPlcHdr", "w15:appearance",
    "w15:color", "w:dataBinding", "w:label", "w:tabIndex", "w:equation", "w:comboBox", "w:date", "w:docPartObj",
    "w:docPartList", "w:dropDownList", "w:picture", "w:richText", "w:text", "w:citation", "w:group", "w:bibliography",
    "w14:checkbox", "w15:repeatingSection", "w15:repeatingSectionItem",
];

/// Word's names for the placeholder text of a new control, and the English text it shows.
pub const PLACEHOLDER_TEXT: (&str, &str) = ("DefaultPlaceholder_-1854013440", "Click or tap here to enter text.");
pub const PLACEHOLDER_DATE: (&str, &str) = ("DefaultPlaceholder_-1854013437", "Click or tap to enter a date.");
pub const PLACEHOLDER_CHOICE: (&str, &str) = ("DefaultPlaceholder_-1854013438", "Choose an item.");
pub const PLACEHOLDER_PICTURE: (&str, &str) = ("DefaultPlaceholder_-1854013436", "Click or tap to add a picture.");

/// One entry of a drop-down list or combo box.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ListItem {
    pub display: String,
    pub value: String,
}

/// The symbol a check box shows: a character code in hex and the font that has it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Symbol {
    pub val: String,
    pub font: String,
}

impl Symbol {
    pub fn ch(&self) -> char {
        u32::from_str_radix(&self.val, 16).ok().and_then(char::from_u32).unwrap_or('\u{2610}')
    }
}

/// What a content control is, as the editor needs it. `n` is the control's number among the content controls of
/// its paragraph (document order, outer before inner); `start` and `end` are the UTF-16 offsets of its content.
/// A block-level control has no `n`: it is addressed by `ctl`, the id of its block.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RControl {
    pub n: usize,
    /// `text`, `richText`, `picture`, `date`, `dropDown`, `comboBox`, `checkbox`, `repeating`, `repeatingItem`,
    /// `group`, `citation`, `bibliography`, `docPart`, `equation` or `other`.
    pub kind: String,
    pub start: usize,
    pub end: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ctl: Option<NodeId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alias: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    /// `boundingBox` (the default), `tags` or `hidden`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub appearance: Option<String>,
    /// `sdtLocked`, `contentLocked` or `sdtContentLocked`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lock: Option<String>,
    /// The control shows its placeholder text (`w:showingPlcHdr`).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub placeholder: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub multiline: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub items: Vec<ListItem>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date_format: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub calendar: Option<String>,
    /// The chosen date, `2026-10-14`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub full_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checked: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checked_symbol: Option<Symbol>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unchecked_symbol: Option<Symbol>,
    /// The control's value is bound to a custom XML part.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub bound: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub temporary: bool,
    /// A repeating section: the title of its items, and whether items may be added and removed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub section_title: Option<String>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub fixed_items: bool,
    /// The building block gallery of a document part control (`Bibliographies`, `Table of Contents`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gallery: Option<String>,
}

fn local(e: &Element) -> &str {
    e.local()
}

fn val(e: &Element) -> Option<&str> {
    // `w:val`, and the `w14:val` and `w15:val` of the extension elements.
    e.attrs.iter().find(|(k, _)| k.ends_with(":val")).map(|(_, v)| v.as_str())
}

fn on(e: &Element) -> bool {
    !matches!(val(e), Some("0" | "false" | "off"))
}

fn attr_local<'a>(e: &'a Element, name: &str) -> Option<&'a str> {
    e.attrs.iter().find(|(k, _)| xml::local_name(k) == name).map(|(_, v)| v.as_str())
}

/// What a control is, read from its `w:sdtPr`.
pub fn read_props(pr: &Element) -> RControl {
    let mut c = RControl { kind: "richText".into(), ..Default::default() };
    for e in pr.elements() {
        match local(e) {
            "alias" => c.alias = val(e).map(String::from),
            "tag" => c.tag = val(e).map(String::from),
            "id" => c.id = val(e).and_then(|v| v.parse().ok()),
            "lock" => c.lock = val(e).filter(|v| *v != "unlocked").map(String::from),
            "showingPlcHdr" => c.placeholder = on(e),
            "temporary" => c.temporary = on(e),
            "dataBinding" => c.bound = true,
            "color" => c.color = val(e).and_then(crate::colour::hex_hash),
            "appearance" => c.appearance = val(e).map(String::from),
            "text" => {
                c.kind = "text".into();
                c.multiline = attr_local(e, "multiLine").is_some_and(|v| matches!(v, "1" | "true" | "on"));
            }
            "richText" => c.kind = "richText".into(),
            "picture" => c.kind = "picture".into(),
            "group" => c.kind = "group".into(),
            "citation" => c.kind = "citation".into(),
            "bibliography" => c.kind = "bibliography".into(),
            "equation" => c.kind = "equation".into(),
            "docPartList" => c.kind = "docPart".into(),
            "docPartObj" => {
                c.kind = "docPart".into();
                c.gallery = e.child("w:docPartGallery").and_then(val).map(String::from);
            }
            "date" => {
                c.kind = "date".into();
                c.full_date = attr_local(e, "fullDate").map(|d| d.chars().take(10).collect());
                for d in e.elements() {
                    match local(d) {
                        "dateFormat" => c.date_format = val(d).map(String::from),
                        "lid" => c.lid = val(d).map(String::from),
                        "calendar" => c.calendar = val(d).map(String::from),
                        _ => {}
                    }
                }
            }
            "dropDownList" | "comboBox" => {
                c.kind = if local(e) == "dropDownList" { "dropDown" } else { "comboBox" }.into();
                for li in e.elements().filter(|x| local(x) == "listItem") {
                    let value = attr_local(li, "value").unwrap_or("").to_string();
                    let display = attr_local(li, "displayText").map(String::from).unwrap_or_else(|| value.clone());
                    c.items.push(ListItem { display, value });
                }
            }
            "checkbox" => {
                c.kind = "checkbox".into();
                let mut checked = false;
                for d in e.elements() {
                    let sym = || Some(Symbol { val: val(d).unwrap_or("").to_string(), font: attr_local(d, "font").unwrap_or("MS Gothic").to_string() });
                    match local(d) {
                        "checked" => checked = matches!(val(d), Some("1" | "true" | "on")),
                        "checkedState" => c.checked_symbol = sym(),
                        "uncheckedState" => c.unchecked_symbol = sym(),
                        _ => {}
                    }
                }
                c.checked = Some(checked);
            }
            "repeatingSection" => {
                c.kind = "repeating".into();
                c.section_title = e.elements().find(|x| local(x) == "sectionTitle").and_then(val).map(String::from);
                c.fixed_items = e.elements().any(|x| local(x) == "doNotAllowInsertDeleteSection" && on(x));
            }
            "repeatingSectionItem" => c.kind = "repeatingItem".into(),
            _ => {}
        }
    }
    c
}

/// The data binding of a control: the XPath into a custom XML part, the part's id and the prefix mappings.
pub fn binding_of(pr: &Element) -> Option<(String, String, String)> {
    let b = pr.elements().find(|e| local(e) == "dataBinding")?;
    Some((
        attr_local(b, "xpath")?.to_string(),
        attr_local(b, "storeItemID").unwrap_or("").to_string(),
        attr_local(b, "prefixMappings").unwrap_or("").to_string(),
    ))
}

/// The name of the placeholder building block (`w:placeholder`).
pub fn placeholder_name(pr: &Element) -> Option<String> {
    pr.elements().find(|e| local(e) == "placeholder").and_then(|p| p.elements().find(|d| local(d) == "docPart")).and_then(val).map(String::from)
}

/// Whether the content of the control is part of what a form fills in: kinds whose content is edited like text.
pub fn is_textual(kind: &str) -> bool {
    matches!(kind, "text" | "richText" | "date" | "dropDown" | "comboBox")
}

/// `contentLocked` and `sdtContentLocked`: the content can't be edited.
pub fn locks_content(lock: Option<&str>) -> bool {
    matches!(lock, Some("contentLocked" | "sdtContentLocked"))
}

/// `sdtLocked` and `sdtContentLocked`: the control can't be deleted.
pub fn locks_control(lock: Option<&str>) -> bool {
    matches!(lock, Some("sdtLocked" | "sdtContentLocked"))
}

// ---------------------------------------------------------------------------
// Finding controls in a paragraph
// ---------------------------------------------------------------------------

/// The content controls of a paragraph, outer before inner: the control, where its content starts and ends.
pub fn sdts_in(inlines: &[Inline]) -> Vec<(&InlineContainer, usize, usize)> {
    fn walk<'a>(inlines: &'a [Inline], at: &mut usize, out: &mut Vec<(&'a InlineContainer, usize, usize)>) {
        for i in inlines {
            match i {
                Inline::Container(c) => {
                    let slot = (c.kind == InlineKind::Sdt).then(|| {
                        out.push((c, *at, *at));
                        out.len() - 1
                    });
                    walk(&c.children, at, out);
                    if let Some(s) = slot {
                        out[s].2 = *at;
                    }
                }
                other => *at += other.len16(),
            }
        }
    }
    let mut out = Vec::new();
    walk(inlines, &mut 0, &mut out);
    out
}

/// The `n`th content control of a paragraph, for changing.
pub fn sdt_mut(inlines: &mut [Inline], n: usize) -> Option<&mut InlineContainer> {
    fn walk<'a>(inlines: &'a mut [Inline], n: usize, seen: &mut usize) -> Option<&'a mut InlineContainer> {
        for i in inlines.iter_mut() {
            if let Inline::Container(c) = i {
                if c.kind == InlineKind::Sdt {
                    if *seen == n {
                        return Some(c);
                    }
                    *seen += 1;
                }
                if let Some(found) = walk(&mut c.children, n, seen) {
                    return Some(found);
                }
            }
        }
        None
    }
    walk(inlines, n, &mut 0)
}

/// The control view of an inline control: `n`, offsets and everything its properties say.
pub fn inline_view(c: &InlineContainer, n: usize, start: usize, end: usize) -> RControl {
    let mut v = c.shell.child("w:sdtPr").map(read_props).unwrap_or_default();
    v.n = n;
    v.start = start;
    v.end = end;
    v
}

/// The view of a block-level control.
pub fn block_view(c: &BlockContainer) -> Option<RControl> {
    let pr = c.shell.child("w:sdtPr")?;
    let mut v = read_props(pr);
    if v.kind == "citation" {
        return None;
    }
    v.ctl = Some(c.id);
    Some(v)
}

pub fn blocks_text(blocks: &[Block]) -> String {
    let mut parts = Vec::new();
    walk_paragraphs(blocks, &mut |p| parts.push(p.text()));
    parts.join("\n")
}

pub fn inline_text(children: &[Inline]) -> String {
    children.iter().map(|i| i.text()).collect()
}

// ---------------------------------------------------------------------------
// Building and changing a control's properties
// ---------------------------------------------------------------------------

/// What the Properties dialog edits. In a patch, a field left out stays as it is, and an empty string removes
/// an alias, tag or colour.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ControlProps {
    pub alias: Option<String>,
    pub tag: Option<String>,
    pub color: Option<String>,
    pub appearance: Option<String>,
    /// `unlocked`, `sdtLocked`, `contentLocked` or `sdtContentLocked`.
    pub lock: Option<String>,
    pub multiline: Option<bool>,
    pub items: Option<Vec<ListItem>>,
    pub date_format: Option<String>,
    pub lid: Option<String>,
    pub calendar: Option<String>,
    pub checked_symbol: Option<Symbol>,
    pub unchecked_symbol: Option<Symbol>,
    /// The placeholder text (shown while the control is empty).
    pub placeholder: Option<String>,
    pub temporary: Option<bool>,
    pub section_title: Option<String>,
    pub fixed_items: Option<bool>,
}

fn declared(e: Element, prefix: &str, uri: &str, local_ns: bool) -> Element {
    if local_ns {
        e.with_attr(format!("xmlns:{prefix}"), uri)
    } else {
        e
    }
}

/// Set or remove a one-attribute child of `w:sdtPr` (`w:alias`, `w:tag`, `w:lock`).
fn put_val(pr: &mut Element, name: &'static str, v: Option<&str>) {
    match v {
        Some(v) if !v.is_empty() => crate::props::put_ordered(pr, Element::new(name).with_attr("w:val", v), SDTPR_ORDER),
        _ => {
            pr.remove_children(name);
        }
    }
}

fn put_flag(pr: &mut Element, name: &'static str, on: bool) {
    if on {
        if pr.child(name).is_none() {
            crate::props::put_ordered(pr, Element::new(name), SDTPR_ORDER);
        }
    } else {
        pr.remove_children(name);
    }
}

/// Find a child of `w:sdtPr` by its local name (the file's own prefix may differ), returning its full name.
fn find_name(pr: &Element, local_name: &str) -> Option<String> {
    pr.elements().find(|e| local(e) == local_name).map(|e| e.name.to_string())
}

fn remove_local(pr: &mut Element, local_name: &str) {
    pr.children.retain(|n| !matches!(n, Node::Element(e) if local(e) == local_name));
}

/// Apply the properties to a control's `w:sdtPr`. `local_ns` declares the namespaces of the elements it writes on
/// the elements themselves (for a part whose root does not declare them).
pub fn patch_props(pr: &mut Element, p: &ControlProps, local_ns: bool) -> Result<(), String> {
    if let Some(a) = &p.alias {
        put_val(pr, "w:alias", Some(a));
    }
    if let Some(t) = &p.tag {
        put_val(pr, "w:tag", Some(t));
    }
    if let Some(l) = &p.lock {
        put_val(pr, "w:lock", (l != "unlocked").then_some(l.as_str()));
    }
    if let Some(t) = p.temporary {
        put_flag(pr, "w:temporary", t);
    }
    if let Some(c) = &p.color {
        remove_local(pr, "color");
        if !c.is_empty() {
            let e = declared(Element::new("w15:color").with_attr("w:val", c.trim_start_matches('#').to_uppercase()), "w15", NS_W15, local_ns);
            crate::props::put_ordered(pr, e, SDTPR_ORDER);
        }
    }
    if let Some(a) = &p.appearance {
        remove_local(pr, "appearance");
        if a != "boundingBox" && !a.is_empty() {
            if !matches!(a.as_str(), "tags" | "hidden") {
                return Err(format!("unknown appearance '{a}'"));
            }
            let e = declared(Element::new("w15:appearance").with_attr("w15:val", a), "w15", NS_W15, local_ns);
            crate::props::put_ordered(pr, e, SDTPR_ORDER);
        }
    }
    if let Some(m) = p.multiline {
        if let Some(t) = pr.child_mut("w:text") {
            if m {
                t.set_attr("w:multiLine", "1");
            } else {
                t.remove_attr("w:multiLine");
            }
        }
    }
    if let Some(items) = &p.items {
        let name = if pr.child("w:comboBox").is_some() {
            "w:comboBox"
        } else if pr.child("w:dropDownList").is_some() {
            "w:dropDownList"
        } else {
            return Err("only a drop-down list or combo box has list items".into());
        };
        let list = pr.child_mut(name).expect("checked");
        list.children.retain(|n| !matches!(n, Node::Element(e) if e.name == "w:listItem"));
        for it in items {
            let mut li = Element::new("w:listItem");
            if it.display != it.value {
                li.set_attr("w:displayText", &it.display);
            }
            li.set_attr("w:value", &it.value);
            list.children.push(Node::Element(li));
        }
    }
    if p.date_format.is_some() || p.lid.is_some() || p.calendar.is_some() {
        let Some(d) = pr.child_mut("w:date") else { return Err("only a date picker has a date format".into()) };
        let set = |d: &mut Element, name: &'static str, v: &Option<String>| {
            if let Some(v) = v {
                if let Some(e) = d.child_mut(name) {
                    e.set_attr("w:val", v);
                } else {
                    let order = ["w:dateFormat", "w:lid", "w:storeMappedDataAs", "w:calendar"];
                    crate::props::put_ordered(d, Element::new(name).with_attr("w:val", v), &order);
                }
            }
        };
        set(d, "w:dateFormat", &p.date_format);
        set(d, "w:lid", &p.lid);
        set(d, "w:calendar", &p.calendar);
    }
    if p.checked_symbol.is_some() || p.unchecked_symbol.is_some() {
        let name = find_name(pr, "checkbox").ok_or("only a check box has symbols")?;
        let cb = pr.child_mut(&name).expect("checked");
        for (tag, sym) in [("w14:checkedState", &p.checked_symbol), ("w14:uncheckedState", &p.unchecked_symbol)] {
            if let Some(s) = sym {
                let order = ["w14:checked", "w14:checkedState", "w14:uncheckedState"];
                let e = Element::new(tag).with_attr("w14:val", s.val.to_uppercase()).with_attr("w14:font", &s.font);
                crate::props::put_ordered(cb, e, &order);
            }
        }
    }
    if p.section_title.is_some() || p.fixed_items.is_some() {
        let name = find_name(pr, "repeatingSection").ok_or("only a repeating section has a title")?;
        let rs = pr.child_mut(&name).expect("checked");
        if let Some(t) = &p.section_title {
            rs.children.retain(|n| !matches!(n, Node::Element(e) if local(e) == "sectionTitle"));
            if !t.is_empty() {
                rs.children.insert(0, Node::Element(Element::new("w15:sectionTitle").with_attr("w15:val", t)));
            }
        }
        if let Some(f) = p.fixed_items {
            rs.children.retain(|n| !matches!(n, Node::Element(e) if local(e) == "doNotAllowInsertDeleteSection"));
            if f {
                rs.children.push(Node::Element(Element::new("w15:doNotAllowInsertDeleteSection").with_attr("w15:val", "1")));
            }
        }
    }
    Ok(())
}

/// The kinds Insert, Form controls offers.
pub const KINDS: &[&str] = &["text", "richText", "picture", "date", "dropDown", "comboBox", "checkbox", "repeating", "group"];

/// A new `w:sdt` of a kind, with its content slot empty. `id` is the control's `w:id`.
pub fn new_shell(kind: &str, id: i64, p: &ControlProps, placeholder: Option<&str>, local_ns: bool) -> Result<Element, String> {
    let mut pr = Element::new("w:sdtPr");
    let mut seed = ControlProps { alias: p.alias.clone(), tag: p.tag.clone(), lock: p.lock.clone(), color: p.color.clone(), appearance: p.appearance.clone(), temporary: p.temporary, ..Default::default() };
    seed.items = None;
    pr.children.push(Node::Element(Element::new("w:id").with_attr("w:val", id.to_string())));
    let kind_el = match kind {
        "text" => {
            let mut t = Element::new("w:text");
            if p.multiline == Some(true) {
                t.set_attr("w:multiLine", "1");
            }
            t
        }
        "richText" => Element::new("w:richText"),
        "picture" => Element::new("w:picture"),
        "group" => Element::new("w:group"),
        "date" => {
            let mut d = Element::new("w:date");
            d.children.push(Node::Element(Element::new("w:dateFormat").with_attr("w:val", p.date_format.as_deref().unwrap_or("M/d/yyyy"))));
            d.children.push(Node::Element(Element::new("w:lid").with_attr("w:val", p.lid.as_deref().unwrap_or("en-US"))));
            d.children.push(Node::Element(Element::new("w:storeMappedDataAs").with_attr("w:val", "dateTime")));
            d.children.push(Node::Element(Element::new("w:calendar").with_attr("w:val", p.calendar.as_deref().unwrap_or("gregorian"))));
            d
        }
        "dropDown" | "comboBox" => {
            let mut l = Element::new(if kind == "dropDown" { "w:dropDownList" } else { "w:comboBox" });
            let first = placeholder.unwrap_or(PLACEHOLDER_CHOICE.1);
            // Word's first entry is the prompt, with no display text of its own.
            l.children.push(Node::Element(Element::new("w:listItem").with_attr("w:value", first)));
            for it in p.items.iter().flatten() {
                let mut li = Element::new("w:listItem");
                if it.display != it.value {
                    li.set_attr("w:displayText", &it.display);
                }
                li.set_attr("w:value", &it.value);
                l.children.push(Node::Element(li));
            }
            l
        }
        "checkbox" => {
            let on = p.checked_symbol.clone().unwrap_or(Symbol { val: "2612".into(), font: "MS Gothic".into() });
            let off = p.unchecked_symbol.clone().unwrap_or(Symbol { val: "2610".into(), font: "MS Gothic".into() });
            declared(
                Element::new("w14:checkbox")
                    .with_child(Element::new("w14:checked").with_attr("w14:val", "0"))
                    .with_child(Element::new("w14:checkedState").with_attr("w14:val", on.val.to_uppercase()).with_attr("w14:font", on.font))
                    .with_child(Element::new("w14:uncheckedState").with_attr("w14:val", off.val.to_uppercase()).with_attr("w14:font", off.font)),
                "w14",
                NS_W14,
                local_ns,
            )
        }
        "repeating" => {
            let mut r = Element::new("w15:repeatingSection");
            if let Some(t) = p.section_title.as_deref().filter(|t| !t.is_empty()) {
                r.children.push(Node::Element(Element::new("w15:sectionTitle").with_attr("w15:val", t)));
            }
            if p.fixed_items == Some(true) {
                r.children.push(Node::Element(Element::new("w15:doNotAllowInsertDeleteSection").with_attr("w15:val", "1")));
            }
            declared(r, "w15", NS_W15, local_ns)
        }
        other => return Err(format!("unknown content control kind '{other}'")),
    };
    crate::props::put_ordered(&mut pr, kind_el, SDTPR_ORDER);
    patch_props(&mut pr, &seed, local_ns)?;
    Ok(Element::new("w:sdt").with_child(pr).with_child(Element::new("w:sdtContent")))
}

/// Set the `w:placeholder` building block and whether the control shows it.
pub fn set_placeholder(pr: &mut Element, name: &str, showing: bool) {
    crate::props::put_ordered(pr, Element::new("w:placeholder").with_child(Element::new("w:docPart").with_attr("w:val", name)), SDTPR_ORDER);
    put_flag(pr, "w:showingPlcHdr", showing);
}

pub fn set_showing(pr: &mut Element, showing: bool) {
    pr.children.retain(|n| !matches!(n, Node::Element(e) if local(e) == "showingPlcHdr"));
    if showing {
        crate::props::put_ordered(pr, Element::new("w:showingPlcHdr"), SDTPR_ORDER);
    }
}

pub fn is_showing(pr: &Element) -> bool {
    pr.elements().any(|e| local(e) == "showingPlcHdr" && on(e))
}

/// Set the check box's state in its properties.
pub fn set_checked(pr: &mut Element, checked: bool) {
    if let Some(name) = find_name(pr, "checkbox") {
        let cb = pr.child_mut(&name).expect("found");
        let prefix = name.split(':').next().unwrap_or("w14").to_string();
        let ck = format!("{prefix}:checked");
        let v = format!("{prefix}:val");
        let value = if checked { "1" } else { "0" };
        let existing = cb.elements().position(|e| local(e) == "checked");
        match existing {
            Some(i) => {
                let e = cb.elements_mut().nth(i).expect("found");
                e.attrs.clear();
                e.set_attr(&v, value);
            }
            None => cb.children.insert(0, Node::Element(Element::new(ck).with_attr(v, value))),
        }
    }
}

// ---------------------------------------------------------------------------
// Content
// ---------------------------------------------------------------------------

/// The formatting a control's new text takes: that of its first run, else the control's own `w:rPr`.
pub fn content_rpr(children: &[Inline], pr: Option<&Element>) -> Option<Element> {
    let mut found = None;
    fn first(inlines: &[Inline], out: &mut Option<Element>) {
        for i in inlines {
            if out.is_some() {
                return;
            }
            match i {
                Inline::Run(r) if r.items.iter().any(|it| matches!(it, RunItem::Text(_))) => *out = Some(r.rpr.clone().unwrap_or_else(|| Element::new("w:rPr"))),
                Inline::Container(c) => first(&c.children, out),
                _ => {}
            }
        }
    }
    first(children, &mut found);
    let mut rpr = found.or_else(|| pr.and_then(|p| p.child("w:rPr")).cloned());
    if let Some(r) = rpr.as_mut() {
        if r.child("w:rStyle").and_then(|s| s.attr("w:val")) == Some("PlaceholderText") {
            r.remove_children("w:rStyle");
        }
    }
    rpr.filter(|r| !r.children.is_empty())
}

/// A run of text with the formatting given; a placeholder carries the `PlaceholderText` style.
pub fn text_run(text: &str, rpr: Option<Element>, placeholder: bool) -> Inline {
    let mut rpr = rpr;
    if placeholder {
        let r = rpr.get_or_insert_with(|| Element::new("w:rPr"));
        crate::props::put_rpr(r, Element::new("w:rStyle").with_attr("w:val", "PlaceholderText"));
    }
    if rpr.as_ref().is_some_and(|r| r.children.is_empty()) {
        rpr = None;
    }
    Inline::Run(Run::of_text(rpr, text))
}

/// Take the placeholder style off every run of the content.
pub fn clear_placeholder_style(children: &mut [Inline]) {
    for i in children.iter_mut() {
        match i {
            Inline::Run(r) => {
                if let Some(rpr) = r.rpr.as_mut() {
                    if rpr.child("w:rStyle").and_then(|s| s.attr("w:val")) == Some("PlaceholderText") {
                        rpr.remove_children("w:rStyle");
                    }
                }
                if r.rpr.as_ref().is_some_and(|x| x.children.is_empty()) {
                    r.rpr = None;
                }
            }
            Inline::Container(c) => clear_placeholder_style(&mut c.children),
            Inline::Opaque(_) => {}
        }
    }
}

pub fn blocks_clear_placeholder_style(blocks: &mut [Block]) {
    walk_paragraphs_mut(blocks, &mut |p| clear_placeholder_style(&mut p.inlines));
}

/// Give `rpr` a font (a check box symbol's).
pub fn with_font(rpr: Option<Element>, font: &str) -> Element {
    let mut r = rpr.unwrap_or_else(|| Element::new("w:rPr"));
    let f = Element::new("w:rFonts").with_attr("w:ascii", font).with_attr("w:eastAsia", font).with_attr("w:hAnsi", font).with_attr("w:hint", "eastAsia");
    crate::props::put_rpr(&mut r, f);
    r
}

// ---------------------------------------------------------------------------
// Dates
// ---------------------------------------------------------------------------

/// Day of the week, 0 for Sunday.
pub fn weekday(y: i32, m: u32, d: u32) -> usize {
    const T: [i32; 12] = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
    let y = if m < 3 { y - 1 } else { y };
    ((y + y / 4 - y / 100 + y / 400 + T[(m - 1) as usize % 12] + d as i32) % 7) as usize
}

/// `2026-10-14` or `2026-10-14T00:00:00Z` as year, month, day.
pub fn parse_date(s: &str) -> Option<(i32, u32, u32)> {
    let s = s.get(..10)?;
    let mut it = s.split('-');
    let y: i32 = it.next()?.parse().ok()?;
    let m: u32 = it.next()?.parse().ok()?;
    let d: u32 = it.next()?.parse().ok()?;
    ((1..=12).contains(&m) && (1..=31).contains(&d)).then_some((y, m, d))
}

struct Names {
    months: [&'static str; 12],
    months_short: [&'static str; 12],
    days: [&'static str; 7],
    days_short: [&'static str; 7],
}

fn names(lid: &str) -> Names {
    match lid.split('-').next().unwrap_or("en").to_lowercase().as_str() {
        "es" => Names {
            months: ["enero", "febrero", "marzo", "abril", "mayo", "junio", "julio", "agosto", "septiembre", "octubre", "noviembre", "diciembre"],
            months_short: ["ene.", "feb.", "mar.", "abr.", "may.", "jun.", "jul.", "ago.", "sep.", "oct.", "nov.", "dic."],
            days: ["domingo", "lunes", "martes", "miércoles", "jueves", "viernes", "sábado"],
            days_short: ["dom.", "lun.", "mar.", "mié.", "jue.", "vie.", "sáb."],
        },
        "fr" => Names {
            months: ["janvier", "février", "mars", "avril", "mai", "juin", "juillet", "août", "septembre", "octobre", "novembre", "décembre"],
            months_short: ["janv.", "févr.", "mars", "avr.", "mai", "juin", "juil.", "août", "sept.", "oct.", "nov.", "déc."],
            days: ["dimanche", "lundi", "mardi", "mercredi", "jeudi", "vendredi", "samedi"],
            days_short: ["dim.", "lun.", "mar.", "mer.", "jeu.", "ven.", "sam."],
        },
        "de" => Names {
            months: ["Januar", "Februar", "März", "April", "Mai", "Juni", "Juli", "August", "September", "Oktober", "November", "Dezember"],
            months_short: ["Jan", "Feb", "Mrz", "Apr", "Mai", "Jun", "Jul", "Aug", "Sep", "Okt", "Nov", "Dez"],
            days: ["Sonntag", "Montag", "Dienstag", "Mittwoch", "Donnerstag", "Freitag", "Samstag"],
            days_short: ["So", "Mo", "Di", "Mi", "Do", "Fr", "Sa"],
        },
        "it" => Names {
            months: ["gennaio", "febbraio", "marzo", "aprile", "maggio", "giugno", "luglio", "agosto", "settembre", "ottobre", "novembre", "dicembre"],
            months_short: ["gen", "feb", "mar", "apr", "mag", "giu", "lug", "ago", "set", "ott", "nov", "dic"],
            days: ["domenica", "lunedì", "martedì", "mercoledì", "giovedì", "venerdì", "sabato"],
            days_short: ["dom", "lun", "mar", "mer", "gio", "ven", "sab"],
        },
        "pt" => Names {
            months: ["janeiro", "fevereiro", "março", "abril", "maio", "junho", "julho", "agosto", "setembro", "outubro", "novembro", "dezembro"],
            months_short: ["jan", "fev", "mar", "abr", "mai", "jun", "jul", "ago", "set", "out", "nov", "dez"],
            days: ["domingo", "segunda-feira", "terça-feira", "quarta-feira", "quinta-feira", "sexta-feira", "sábado"],
            days_short: ["dom", "seg", "ter", "qua", "qui", "sex", "sáb"],
        },
        _ => Names {
            months: ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"],
            months_short: ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"],
            days: ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"],
            days_short: ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"],
        },
    }
}

/// A date in a Word date format (`M/d/yyyy`, `dddd, MMMM d, yyyy`, `d MMM yyyy`). Text in single quotes is kept
/// as it is; names of months and days follow the language.
pub fn format_date(fmt: &str, lid: &str, y: i32, m: u32, d: u32) -> String {
    let n = names(lid);
    let chars: Vec<char> = fmt.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let mut run = 1;
        while i + run < chars.len() && chars[i + run] == c {
            run += 1;
        }
        match c {
            '\'' => {
                // A quoted stretch (`''` is a quote).
                i += 1;
                if chars.get(i) == Some(&'\'') {
                    out.push('\'');
                    i += 1;
                    continue;
                }
                while i < chars.len() && chars[i] != '\'' {
                    out.push(chars[i]);
                    i += 1;
                }
                i += 1;
                continue;
            }
            'd' => match run {
                1 => out.push_str(&d.to_string()),
                2 => out.push_str(&format!("{d:02}")),
                3 => out.push_str(n.days_short[weekday(y, m, d)]),
                _ => out.push_str(n.days[weekday(y, m, d)]),
            },
            'M' => match run {
                1 => out.push_str(&m.to_string()),
                2 => out.push_str(&format!("{m:02}")),
                3 => out.push_str(n.months_short[(m - 1) as usize]),
                _ => out.push_str(n.months[(m - 1) as usize]),
            },
            'y' => match run {
                1 | 2 => out.push_str(&format!("{:02}", y.rem_euclid(100))),
                _ => out.push_str(&format!("{y:04}")),
            },
            other => {
                for _ in 0..run {
                    out.push(other);
                }
            }
        }
        i += run;
    }
    out
}

// ---------------------------------------------------------------------------
// Placeholder text (the glossary)
// ---------------------------------------------------------------------------

fn glossary_path(pkg: &Package, main: &str) -> Option<String> {
    pkg.rels_for(main).ok()?.into_iter().find(|r| r.rel_type == REL_GLOSSARY).map(|r| package::resolve_target(main, &r.target))
}

fn doc_part_name(part: &Element) -> Option<&str> {
    part.child("w:docPartPr")?.child("w:name")?.attr("w:val")
}

/// The text of a placeholder building block, from the document's glossary.
pub fn glossary_text(pkg: &Package, main: &str, name: &str) -> Option<String> {
    let path = glossary_path(pkg, main)?;
    let doc = pkg.xml(&path).ok()?;
    let parts = doc.root.child("w:docParts")?;
    let part = parts.elements().find(|p| doc_part_name(p) == Some(name))?;
    let body = part.child("w:docPartBody")?;
    let mut paras = Vec::new();
    for p in body.elements().filter(|e| e.name == "w:p") {
        let mut ts = Vec::new();
        p.find_all("w:t", &mut ts);
        paras.push(ts.iter().map(|t| t.text()).collect::<String>());
    }
    Some(paras.join("\n"))
}

fn hex_guid(seed: &str) -> String {
    // Two 64-bit FNV passes give 128 bits that depend on the text; it only has to be unique per text.
    let mut a: u64 = 0xcbf29ce484222325;
    let mut b: u64 = 0x84222325cbf29ce4;
    for byte in seed.bytes() {
        a = (a ^ byte as u64).wrapping_mul(0x100000001b3);
        b = (b.rotate_left(5) ^ byte as u64).wrapping_mul(0x100000001b3);
    }
    let h = format!("{a:016X}{b:016X}");
    format!("{{{}-{}-{}-{}-{}}}", &h[0..8], &h[8..12], &h[12..16], &h[16..20], &h[20..32])
}

const GLOSSARY_STYLES: &str = r#"<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:style w:type="paragraph" w:default="1" w:styleId="Normal"><w:name w:val="Normal"/><w:qFormat/></w:style><w:style w:type="character" w:default="1" w:styleId="DefaultParagraphFont"><w:name w:val="Default Paragraph Font"/><w:uiPriority w:val="1"/><w:semiHidden/><w:unhideWhenUsed/></w:style><w:style w:type="character" w:styleId="PlaceholderText"><w:name w:val="Placeholder Text"/><w:basedOn w:val="DefaultParagraphFont"/><w:uiPriority w:val="99"/><w:semiHidden/><w:rPr><w:color w:val="808080"/></w:rPr></w:style></w:styles>"#;

/// An empty glossary document at `path`, with its styles next to it.
fn new_glossary(pkg: &mut Package, path: &str) {
    let doc = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:glossaryDocument xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:docParts/></w:glossaryDocument>"#;
    pkg.set(path, doc.as_bytes().to_vec());
    pkg.ensure_override(path, CT_GLOSSARY);
    let styles = package::resolve_target(path, "styles.xml");
    if pkg.get(&styles).is_none() {
        pkg.set(&styles, GLOSSARY_STYLES.as_bytes().to_vec());
        pkg.ensure_override(&styles, package::CT_STYLES);
    }
    if !pkg.rels_for(path).is_ok_and(|rels| rels.iter().any(|r| r.rel_type == package::REL_STYLES)) {
        pkg.add_rel(path, package::REL_STYLES, "styles.xml", false);
    }
}

/// Make sure the glossary has a placeholder building block with this text, and return its name. Word's own
/// names are used for its default texts.
pub fn ensure_placeholder(pkg: &mut Package, main: &str, text: &str, preferred: Option<&str>) -> String {
    let path = match glossary_path(pkg, main) {
        Some(p) if pkg.get(&p).is_some() => p,
        // The file points at a glossary it does not hold (a file that lost the part): it is made where it points, or
        // the placeholder's text would be looked up in nothing and the box would show other text than its placeholder.
        Some(p) => {
            new_glossary(pkg, &p);
            p
        }
        None => {
            let path = "word/glossary/document.xml".to_string();
            new_glossary(pkg, &path);
            pkg.add_rel(main, REL_GLOSSARY, "glossary/document.xml", false);
            path
        }
    };
    let Ok(mut doc) = pkg.xml(&path) else { return preferred.unwrap_or(PLACEHOLDER_TEXT.0).to_string() };
    if doc.root.child("w:docParts").is_none() {
        doc.root.children.push(Node::Element(Element::new("w:docParts")));
    }
    let parts = doc.root.child_mut("w:docParts").expect("added");
    // A building block with this text already exists.
    for p in parts.elements() {
        let Some(name) = doc_part_name(p) else { continue };
        let is_placeholder = p.path(&["w:docPartPr", "w:category", "w:gallery"]).and_then(|g| g.attr("w:val")) == Some("placeholder");
        if !is_placeholder {
            continue;
        }
        let mut ts = Vec::new();
        p.find_all("w:t", &mut ts);
        if ts.iter().map(|t| t.text()).collect::<String>() == text {
            return name.to_string();
        }
    }
    let mut name = preferred.map(String::from).unwrap_or_else(|| hex_guid(text).trim_matches(['{', '}']).replace('-', ""));
    if parts.elements().any(|p| doc_part_name(p) == Some(name.as_str())) {
        name = hex_guid(&format!("{text}{name}")).trim_matches(['{', '}']).replace('-', "");
    }
    let part = Element::new("w:docPart")
        .with_child(
            Element::new("w:docPartPr")
                .with_child(Element::new("w:name").with_attr("w:val", &name))
                .with_child(Element::new("w:category").with_child(Element::new("w:name").with_attr("w:val", "General")).with_child(Element::new("w:gallery").with_attr("w:val", "placeholder")))
                .with_child(Element::new("w:types").with_child(Element::new("w:type").with_attr("w:val", "bbPlcHdr")))
                .with_child(Element::new("w:behaviors").with_child(Element::new("w:behavior").with_attr("w:val", "content")))
                .with_child(Element::new("w:guid").with_attr("w:val", hex_guid(&name))),
        )
        .with_child(
            Element::new("w:docPartBody").with_child(Element::new("w:p").with_child(
                crate::model::text_run_element(Some(Element::new("w:rPr").with_child(Element::new("w:rStyle").with_attr("w:val", "PlaceholderText"))), text),
            )),
        );
    parts.children.push(Node::Element(part));
    pkg.set(&path, doc.to_bytes());
    name
}

// ---------------------------------------------------------------------------
// The ops
// ---------------------------------------------------------------------------

/// Which content control an op means: the `n`th control of paragraph `p`, the block-level control `block`, or
/// the row-level control `wrap`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ControlRef {
    pub p: Option<Id>,
    pub n: usize,
    pub block: Option<Id>,
    pub wrap: Option<Id>,
}

/// A picture for a picture control: media added with `add_media`, and its size in points.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PictureValue {
    pub media: String,
    pub width: f64,
    pub height: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alt: Option<String>,
}

/// Content control ops. The wire form is `{"op": "control", "action": "...", ...}`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum ControlOp {
    /// Insert a control of a kind at a position. With `to` (and a kind that holds text) the text between
    /// `at` and `to` becomes the content. `block` wraps the paragraph at `at` instead; a repeating section is
    /// always block-level.
    Insert {
        at: Pos,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        to: Option<Pos>,
        kind: String,
        #[serde(default)]
        props: ControlProps,
        #[serde(default)]
        block: bool,
    },
    /// Change a control's properties.
    Props { target: ControlRef, props: ControlProps },
    /// Give a control a value: text, a check box state, an entry of a list (by position), a date or a picture.
    Set {
        target: ControlRef,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        text: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        checked: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        index: Option<usize>,
        /// `2026-10-14`; an empty string clears the date.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        date: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        picture: Option<PictureValue>,
    },
    /// Take a control away. The content stays unless `delete_content`.
    Remove {
        target: ControlRef,
        #[serde(default)]
        delete_content: bool,
    },
    /// Add an item to a repeating section (a copy of the item `target` points at, or of the last one), or
    /// take the item `target` points at away.
    Repeat {
        target: ControlRef,
        #[serde(default)]
        remove: bool,
    },
}

impl ControlOp {
    /// The flags of the op (see `OpClass`): every control op edits the text it is in.
    pub fn class(&self) -> crate::ops::OpClass {
        match self {
            ControlOp::Insert { .. } | ControlOp::Props { .. } | ControlOp::Set { .. } | ControlOp::Remove { .. } | ControlOp::Repeat { .. } => crate::ops::OpClass::BODY,
        }
    }

    /// The paragraph or block that locates the op, to route it into a header or note when it sits there.
    pub fn anchor(&self) -> Option<NodeId> {
        match self {
            ControlOp::Insert { at, .. } => Some(at.p.0),
            ControlOp::Props { target, .. } | ControlOp::Set { target, .. } | ControlOp::Remove { target, .. } | ControlOp::Repeat { target, .. } => {
                target.p.or(target.block).or(target.wrap).map(|i| i.0)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_follow_word_formats() {
        assert_eq!(format_date("M/d/yyyy", "en-US", 2026, 10, 14), "10/14/2026");
        assert_eq!(format_date("dddd, MMMM d, yyyy", "en-US", 2026, 10, 14), "Wednesday, October 14, 2026");
        assert_eq!(format_date("d 'de' MMMM 'de' yyyy", "es-ES", 2026, 1, 5), "5 de enero de 2026");
        assert_eq!(format_date("dd.MM.yy", "de-DE", 2026, 3, 9), "09.03.26");
        assert_eq!(weekday(2000, 1, 1), 6);
    }

    #[test]
    fn a_new_checkbox_has_its_symbols() {
        let s = new_shell("checkbox", 7, &ControlProps::default(), None, false).unwrap();
        let v = read_props(s.child("w:sdtPr").unwrap());
        assert_eq!(v.kind, "checkbox");
        assert_eq!(v.checked, Some(false));
        assert_eq!(v.checked_symbol.unwrap().ch(), '\u{2612}');
        assert_eq!(v.id, Some(7));
    }

    #[test]
    fn properties_are_written_in_order() {
        let mut s = new_shell("date", 9, &ControlProps { alias: Some("Due".into()), lock: Some("sdtLocked".into()), ..Default::default() }, None, false).unwrap();
        let pr = s.child_mut("w:sdtPr").unwrap();
        patch_props(pr, &ControlProps { color: Some("ff0000".into()), appearance: Some("tags".into()), tag: Some("due".into()), ..Default::default() }, true).unwrap();
        let names: Vec<&str> = pr.elements().map(|e| e.name.as_str()).collect();
        assert_eq!(names, ["w:alias", "w:tag", "w:id", "w:lock", "w15:appearance", "w15:color", "w:date"]);
        let v = read_props(pr);
        assert_eq!(v.color.as_deref(), Some("FF0000"));
        assert_eq!(v.appearance.as_deref(), Some("tags"));
    }
}
