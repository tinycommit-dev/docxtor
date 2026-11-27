//! Document tabs: what a Google Docs document has that Word doesn't.
//!
//! The body of a tabbed document stays one linear body, so Word shows every tab one after the other. Each tab after
//! the first starts a new section (next page), and its first block holds a hidden point bookmark called
//! `_DocxtorTab_<id>`. The list of tabs (id, name, emoji icon, parent, order and the tab that is open) lives in a custom
//! XML part of Docxtor's own, found by its namespace like the mail merge recipients, so Word ignores it and keeps it.
//! A document without that part has no tabs: one implicit tab, and nothing is added to the file.
//!
//! This module holds the pure parts: the list, the part and the ops. The ops are applied in `document/tabs.rs`.

use crate::citations::{relative_to, CT_CUSTOM_XML_PROPS, REL_CUSTOM_XML, REL_CUSTOM_XML_PROPS};
use crate::model::NodeId;
use crate::package::{self, Package};
use crate::xml::{self, Element, Node, XmlDoc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

pub const NS_TABS: &str = "https://docxtor.tinycommit.dev/ns/tabs/1";
const NS_DS: &str = "http://schemas.openxmlformats.org/officeDocument/2006/customXml";

/// The start of the hidden bookmark that marks the first block of a tab.
pub const MARKER_PREFIX: &str = "_DocxtorTab_";

/// Docs nests tabs up to three levels.
pub const MAX_DEPTH: usize = 3;

/// The longest name a tab can have.
pub const MAX_NAME: usize = 100;

/// The longest icon (an emoji, possibly several code points) read from a file.
const MAX_ICON: usize = 16;

/// The bookmark that marks the start of a tab, and the name a link to the tab points at.
pub fn marker_name(id: &str) -> String {
    format!("{MARKER_PREFIX}{id}")
}

/// The tab a bookmark name marks the start of.
pub fn marker_id(name: &str) -> Option<&str> {
    name.strip_prefix(MARKER_PREFIX).filter(|id| !id.is_empty())
}

#[derive(Debug, Clone, PartialEq)]
pub struct Tab {
    pub id: String,
    pub name: String,
    /// An emoji, or none.
    pub icon: Option<String>,
    /// The tab this one is nested in.
    pub parent: Option<String>,
}

/// The tabs in document order: a parent comes before its children, and a tab's whole subtree follows it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TabList {
    pub tabs: Vec<Tab>,
    /// The number the next new tab's id takes (ids are never used twice).
    pub next: u32,
}

impl TabList {
    pub fn index(&self, id: &str) -> Option<usize> {
        self.tabs.iter().position(|t| t.id == id)
    }

    /// The nesting level of every tab: 0 for the top level.
    ///
    /// One pass with a map from id to depth, so a long list is not searched once for each tab's parent (a tabs
    /// part from outside can list thousands of tabs). A parent that is listed more than once, or after its
    /// child, is the first one listed before it, as `index` finds it.
    pub fn depths(&self) -> Vec<usize> {
        let mut out: Vec<usize> = Vec::with_capacity(self.tabs.len());
        let mut at: HashMap<&str, usize> = HashMap::with_capacity(self.tabs.len());
        for (i, t) in self.tabs.iter().enumerate() {
            let d = t.parent.as_deref().and_then(|p| at.get(p).copied()).map_or(0, |j| out[j] + 1);
            out.push(d);
            at.entry(t.id.as_str()).or_insert(i);
        }
        out
    }

    /// The index just past a tab and everything nested in it.
    pub fn subtree_end(&self, i: usize) -> usize {
        let depths = self.depths();
        let mut k = i + 1;
        while k < self.tabs.len() && depths[k] > depths[i] {
            k += 1;
        }
        k
    }

    /// How many levels sit below a tab (0 for a tab with no children).
    pub fn height(&self, i: usize) -> usize {
        let depths = self.depths();
        (i + 1..self.subtree_end(i)).map(|k| depths[k] - depths[i]).max().unwrap_or(0)
    }

    pub fn fresh_id(&mut self) -> String {
        loop {
            self.next += 1;
            let id = format!("t{}", self.next);
            if self.index(&id).is_none() {
                return id;
            }
        }
    }

    /// Parents come first and nest at most `MAX_DEPTH` levels; ids are unique.
    pub fn is_valid(&self) -> bool {
        let mut seen: Vec<&str> = Vec::new();
        for t in &self.tabs {
            if seen.contains(&t.id.as_str()) {
                return false;
            }
            if let Some(p) = &t.parent {
                if !seen.contains(&p.as_str()) {
                    return false;
                }
            }
            seen.push(&t.id);
        }
        self.depths().iter().all(|d| *d < MAX_DEPTH)
    }
}

// ---------------------------------------------------------------------------
// The part
// ---------------------------------------------------------------------------

fn el(name: &str) -> Element {
    Element::new(format!("ft:{name}"))
}

pub fn to_xml(list: &TabList, active: Option<&str>) -> XmlDoc {
    let mut root = el("tabs").with_attr("xmlns:ft", NS_TABS).with_attr("version", "1").with_attr("next", list.next.to_string());
    if let Some(a) = active {
        root.set_attr("active", a);
    }
    for t in &list.tabs {
        let mut e = el("tab").with_attr("id", t.id.clone()).with_attr("name", t.name.clone());
        if let Some(i) = &t.icon {
            e.set_attr("icon", i.clone());
        }
        if let Some(p) = &t.parent {
            e.set_attr("parent", p.clone());
        }
        root.children.push(Node::Element(e));
    }
    XmlDoc::new(root)
}

/// The tabs a part holds and the tab that was open, or none when the part isn't Docxtor's or lists no tabs.
pub fn from_xml(doc: &XmlDoc) -> Option<(TabList, Option<String>)> {
    from_xml_with(doc, &crate::budget::Budget::default())
}

/// `from_xml`, reading no more tabs than the budget allows (`Limits::tabs`) and cutting names and icons to the
/// length ops allow. Linear in the number of tabs: depth is tracked in a map as the tabs are read.
pub fn from_xml_with(doc: &XmlDoc, budget: &crate::budget::Budget) -> Option<(TabList, Option<String>)> {
    let root = &doc.root;
    if root.local() != "tabs" || !root.attrs.iter().any(|(_, v)| v == NS_TABS) {
        return None;
    }
    let mut list = TabList { next: root.attr("next").and_then(|n| n.parse().ok()).unwrap_or(0), ..Default::default() };
    let mut depth_of: HashMap<String, usize> = HashMap::new();
    for e in root.elements().filter(|e| e.local() == "tab") {
        if list.tabs.len() >= budget.limits().tabs {
            break;
        }
        let Some(id) = e.attr("id").filter(|i| !i.is_empty() && !depth_of.contains_key(*i)) else { continue };
        // A parent that isn't listed before it, or would nest too deep, makes the tab a top-level one.
        let mut parent = e.attr("parent").filter(|p| depth_of.contains_key(*p)).map(String::from);
        let mut depth = parent.as_deref().map_or(0, |p| depth_of[p] + 1);
        if depth >= MAX_DEPTH {
            parent = None;
            depth = 0;
        }
        let name: String = e.attr("name").unwrap_or_default().chars().take(MAX_NAME).collect();
        let icon = e.attr("icon").filter(|i| !i.is_empty()).map(|i| i.chars().take(MAX_ICON).collect::<String>());
        let tab = Tab { id: id.into(), name, icon, parent };
        depth_of.insert(tab.id.clone(), depth);
        list.tabs.push(tab);
        if let Some(n) = id.strip_prefix('t').and_then(|n| n.parse::<u32>().ok()) {
            list.next = list.next.max(n);
        }
    }
    if list.tabs.is_empty() {
        return None;
    }
    Some((list, root.attr("active").map(String::from)))
}

/// The tabs of a document, with the part they live in. Like the recipients of a mail merge it keeps a snapshot for
/// undo and writes the part when the file is saved.
///
/// Which tab is open is a way of looking, not part of the document (review 1.2): each pane of the editor holds its own,
/// and no op changes it. The state keeps only what the file said (`active`, so a reopened file shows the tab it was
/// saved on) and what the editor last told it to write (`Document::set_open_tab`, called when saving). Neither counts
/// as a change to the file, and undo leaves it alone.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TabsState {
    /// Part name once it exists in the package.
    pub path: Option<String>,
    pub list: Option<Arc<TabList>>,
    active: Option<String>,
    pub dirty: bool,
    active_dirty: bool,
}

impl TabsState {
    /// Find Docxtor's part by what it holds (a `tabs` root in Docxtor's namespace), not by its name.
    pub fn load(pkg: &Package, main_path: &str, budget: &crate::budget::Budget) -> TabsState {
        let mut candidates: Vec<String> = pkg
            .rels_for(main_path)
            .unwrap_or_default()
            .into_iter()
            .filter(|r| r.rel_type == REL_CUSTOM_XML && !r.external)
            .map(|r| package::resolve_target(main_path, &r.target))
            .collect();
        for p in &pkg.parts {
            if p.name.starts_with("customXml/") && p.name.ends_with(".xml") && !p.name.contains("/_rels/") && !candidates.contains(&p.name) {
                candidates.push(p.name.clone());
            }
        }
        for name in candidates {
            let Some(data) = pkg.get(&name) else { continue };
            if !data.windows(NS_TABS.len()).any(|w| w == NS_TABS.as_bytes()) {
                continue;
            }
            if let Some((list, active)) = xml::parse(data).ok().and_then(|d| from_xml_with(&d, budget)) {
                // The tab the file names, as it names it: which tab that falls back to when it is not there is `tabs_info`'s to say.
                return TabsState { path: Some(name), list: Some(Arc::new(list)), active, dirty: false, active_dirty: false };
            }
        }
        TabsState::default()
    }

    /// The tab the file was saved on, or the last one `set_open` named, as it was named (it may not be there).
    pub fn asked(&self) -> Option<&str> {
        self.active.as_deref()
    }

    /// The open tab: the one asked for, else the first.
    pub fn open_id(&self) -> Option<&str> {
        let list = self.list.as_ref()?;
        self.active.as_deref().filter(|a| list.index(a).is_some()).or_else(|| list.tabs.first().map(|t| t.id.as_str()))
    }

    /// Set the tab to write into the file at the next save (the editor says which one it shows).
    pub fn set_open(&mut self, id: &str) {
        if self.open_id() != Some(id) {
            self.active_dirty = true;
        }
        self.active = Some(id.into());
    }

    pub fn set(&mut self, list: Option<TabList>) {
        self.list = list.map(Arc::new);
        self.dirty = true;
        // The tab asked for is kept while there are no tabs: an undo that brings them back opens it again.
    }

    pub fn restore(&mut self, snap: TabsState) {
        if snap.list != self.list {
            self.dirty = true;
        } else {
            self.dirty |= snap.dirty;
        }
        self.list = snap.list;
        if self.path.is_none() {
            self.path = snap.path;
        }
        // The open tab stays (a way of looking, not part of the document).
    }

    pub fn needs_flush(&self) -> bool {
        self.dirty || self.active_dirty
    }

    /// Write the part into the package: created with its properties part, relationship and content type the first
    /// time, as Word lays out custom XML; taken out again (with all of that) when the document no longer has tabs.
    pub fn flush(&mut self, pkg: &mut Package, main_path: &str) {
        if !self.needs_flush() {
            return;
        }
        self.dirty = false;
        self.active_dirty = false;
        let Some(list) = self.list.clone() else {
            if let Some(path) = self.path.take() {
                pkg.remove_rels_to(main_path, &path);
                let props = pkg.rels_for(&path).unwrap_or_default().into_iter().find(|r| r.rel_type == REL_CUSTOM_XML_PROPS).map(|r| package::resolve_target(&path, &r.target));
                if let Some(p) = props {
                    pkg.remove(&p);
                    pkg.remove_override(&p);
                }
                pkg.remove(&package::rels_path(&path));
                pkg.remove(&path);
            }
            return;
        };
        // The tab asked for, when the document has it; none otherwise (a reopened file then opens the first tab in the text, as this
        // one shows when nothing was asked).
        let doc = to_xml(&list, self.active.as_deref().filter(|a| list.index(a).is_some()));
        let path = match &self.path {
            Some(p) => p.clone(),
            None => {
                let mut n = 1;
                loop {
                    let name = format!("customXml/item{n}.xml");
                    if !pkg.has(&name) && !pkg.has(&format!("customXml/itemProps{n}.xml")) {
                        break name;
                    }
                    n += 1;
                }
            }
        };
        // What the package holds decides what is missing, not what was true when `path` was remembered: undo of a
        // deleted tab list brings `path` back from a snapshot while the save before it took the pieces out.
        let n: u32 = path.strip_prefix("customXml/item").and_then(|r| r.strip_suffix(".xml")).and_then(|d| d.parse().ok()).unwrap_or(1);
        let existing_props = pkg.rels_for(&path).unwrap_or_default().into_iter().find(|r| r.rel_type == REL_CUSTOM_XML_PROPS).map(|r| package::resolve_target(&path, &r.target));
        let props = existing_props.unwrap_or_else(|| format!("customXml/itemProps{n}.xml"));
        if !pkg.has(&props) {
            let ds = XmlDoc::new(
                Element::new("ds:datastoreItem")
                    .with_attr("ds:itemID", format!("{{F0120000-{n:04X}-5441-8C00-{:012X}}}", 0xF012u64))
                    .with_attr("xmlns:ds", NS_DS)
                    .with_child(Element::new("ds:schemaRefs").with_child(Element::new("ds:schemaRef").with_attr("ds:uri", NS_TABS))),
            );
            pkg.set(&props, ds.to_bytes());
        }
        let related = |pkg: &Package, from: &str, kind: &str, to: &str| pkg.rels_for(from).unwrap_or_default().iter().any(|r| r.rel_type == kind && !r.external && package::resolve_target(from, &r.target) == to);
        if !related(pkg, &path, REL_CUSTOM_XML_PROPS, &props) {
            pkg.add_rel(&path, REL_CUSTOM_XML_PROPS, &relative_to(&path, &props), false);
        }
        pkg.ensure_override(&props, CT_CUSTOM_XML_PROPS);
        pkg.ensure_default("xml", "application/xml");
        if !related(pkg, main_path, REL_CUSTOM_XML, &path) {
            pkg.add_rel(main_path, REL_CUSTOM_XML, &relative_to(main_path, &path), false);
        }
        self.path = Some(path.clone());
        pkg.set(&path, doc.to_bytes());
    }
}

// ---------------------------------------------------------------------------
// What the interface and the AI tools are told
// ---------------------------------------------------------------------------

/// One tab as the tab list shows it.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TabInfo {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    /// 0 for the top level.
    pub depth: usize,
    /// The first block of the tab (a top-level block id).
    pub start: NodeId,
    /// How many top-level blocks the tab holds in the body (bookmark ends and other invisible elements count).
    pub blocks: usize,
    /// The ids of the blocks the tab shows, in the order the page draws them (`RenderOut::order`): invisible
    /// elements are left out and an imported chunk is its own blocks. A pane hides every block that is not listed.
    pub ids: Vec<NodeId>,
}

/// All the tabs and the open one. `tabs` is empty for a document without tabs.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TabsInfo {
    pub tabs: Vec<TabInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active: Option<String>,
}

// ---------------------------------------------------------------------------
// Ops
// ---------------------------------------------------------------------------

/// The ops on tabs. The wire form is `{"op": "tab", "action": "...", ...}`. Every one of them is one undo step. Showing
/// a tab is not an op: which tab is open belongs to the pane that shows it (the editor), not to the document.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum TabOp {
    /// Add a tab with one empty paragraph. It goes at the end of the top level; with `parent` it is the
    /// last tab nested in that tab (a subtab), and with `after` it follows that tab and its subtabs as a sibling. A
    /// document without tabs becomes one with two: its text is the first tab (called `first_name`).
    Add {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        name: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        icon: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        parent: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        after: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        first_name: Option<String>,
        /// Whether the pane that added the tab should show it (the default). The engine has no open tab of its own to
        /// change: the editor reads this, and a caller without a pane (the assistant, the CLI) can ignore it.
        #[serde(default = "yes")]
        open: bool,
    },
    Rename { id: String, name: String },
    /// An emoji for the tab; none takes it away.
    SetIcon {
        id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        icon: Option<String>,
    },
    /// Delete the tab, its subtabs and everything in them.
    Delete { id: String },
    /// Copy the tab (not its subtabs) to just after it and its subtabs. The pane that copied it shows the copy.
    Duplicate {
        id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        name: Option<String>,
    },
    /// Move the tab and its subtabs to be nested in `parent` (none for the top level), just before the tab `before`
    /// (none for the end). This reorders and nests.
    Move {
        id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        parent: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        before: Option<String>,
    },
}

fn yes() -> bool {
    true
}

impl TabOp {
    /// The flags of the op (see `OpClass`): renaming and an icon change the tab list only, not the text. Tab ops can't
    /// break a tab, so they stay within a paragraph.
    pub fn class(&self) -> crate::ops::OpClass {
        use crate::ops::OpClass as C;
        match self {
            TabOp::Rename { .. } | TabOp::SetIcon { .. } => C::BODY.within_paragraph().leaves_body(),
            TabOp::Add { .. } | TabOp::Delete { .. } | TabOp::Duplicate { .. } | TabOp::Move { .. } => C::BODY.within_paragraph(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tab(id: &str, parent: Option<&str>) -> Tab {
        Tab { id: id.into(), name: format!("Tab {id}"), icon: None, parent: parent.map(String::from) }
    }

    #[test]
    fn the_part_round_trips() {
        let list = TabList { tabs: vec![tab("t1", None), Tab { icon: Some("📄".into()), ..tab("t2", Some("t1")) }, tab("t3", None)], next: 3 };
        let doc = to_xml(&list, Some("t2"));
        let (back, active) = from_xml(&xml::parse(&doc.to_bytes()).unwrap()).unwrap();
        assert_eq!(back, list);
        assert_eq!(active.as_deref(), Some("t2"));
    }

    #[test]
    fn depths_and_subtrees_follow_the_parents() {
        let list = TabList { tabs: vec![tab("t1", None), tab("t2", Some("t1")), tab("t3", Some("t2")), tab("t4", None)], next: 4 };
        assert_eq!(list.depths(), vec![0, 1, 2, 0]);
        assert_eq!(list.subtree_end(0), 3);
        assert_eq!(list.subtree_end(3), 4);
        assert_eq!(list.height(0), 2);
        assert!(list.is_valid());
    }

    #[test]
    fn a_part_that_nests_too_deep_is_flattened() {
        let list = TabList { tabs: vec![tab("t1", None), tab("t2", Some("t1")), tab("t3", Some("t2")), tab("t4", Some("t3"))], next: 4 };
        let doc = to_xml(&list, None);
        let (back, _) = from_xml(&doc).unwrap();
        assert!(back.is_valid());
        assert_eq!(back.tabs[3].parent, None);
    }

    #[test]
    fn other_parts_are_not_tabs() {
        let doc = XmlDoc::new(Element::new("b:Sources").with_attr("xmlns:b", "x"));
        assert!(from_xml(&doc).is_none());
    }
}
