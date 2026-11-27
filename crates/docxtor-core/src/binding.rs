//! Data binding for content controls: a control's `w:dataBinding` names a custom XML part (by the id in its
//! properties part) and an XPath into it. This reads and writes the node's text.
//!
//! The XPath is the simple form Word writes: `/ns0:root[1]/ns0:child[2]`, optionally ending in `/@attr` or
//! `/text()`. The prefixes are the ones in `w:prefixMappings` (`xmlns:ns0='uri'`), which need not be the
//! prefixes the part itself uses.

use crate::package::{self, Package};
use crate::xml::{self, Element, Node, XmlDoc};

const REL_CUSTOM_XML_PROPS: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/customXmlProps";

/// `xmlns:ns0='uri' xmlns:ns1='uri'` as (prefix, uri) pairs.
fn mappings(s: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut rest = s;
    while let Some(i) = rest.find("xmlns:") {
        rest = &rest[i + 6..];
        let Some(eq) = rest.find('=') else { break };
        let prefix = rest[..eq].trim().to_string();
        let after = &rest[eq + 1..];
        let Some(q) = after.chars().next().filter(|c| *c == '\'' || *c == '"') else { break };
        let body = &after[q.len_utf8()..];
        let Some(end) = body.find(q) else { break };
        out.push((prefix, body[..end].to_string()));
        rest = &body[end + 1..];
    }
    out
}

/// The custom XML part whose properties part carries this item id.
pub fn find_part(pkg: &Package, store: &str) -> Option<String> {
    if store.is_empty() {
        return None;
    }
    let want = store.trim().to_uppercase();
    for p in &pkg.parts {
        let n = &p.name;
        if !n.starts_with("customXml/") || !n.ends_with(".xml") || n.contains("/_rels/") || n.contains("itemProps") {
            continue;
        }
        for rel in pkg.rels_for(n).unwrap_or_default() {
            if rel.rel_type != REL_CUSTOM_XML_PROPS {
                continue;
            }
            let props = package::resolve_target(n, &rel.target);
            let Ok(doc) = pkg.xml(&props) else { continue };
            let id = doc.root.attrs.iter().find(|(k, _)| xml::local_name(k) == "itemID").map(|(_, v)| v.trim().to_uppercase());
            if id.as_deref() == Some(want.as_str()) {
                return Some(n.clone());
            }
        }
    }
    None
}

struct Step {
    prefix: Option<String>,
    name: String,
    index: usize,
    attr: bool,
    text: bool,
}

fn steps(xpath: &str) -> Vec<Step> {
    xpath
        .split('/')
        .filter(|s| !s.is_empty())
        .map(|s| {
            let (name, index) = match s.find('[') {
                Some(i) => (&s[..i], s[i + 1..].trim_end_matches(']').parse::<usize>().unwrap_or(1)),
                None => (s, 1),
            };
            let attr = name.starts_with('@');
            let name = name.trim_start_matches('@');
            let (prefix, name) = match name.split_once(':') {
                Some((p, n)) => (Some(p.to_string()), n.to_string()),
                None => (None, name.to_string()),
            };
            Step { prefix, text: name == "text()", name, index, attr }
        })
        .collect()
}

/// The namespace an element's name is in, given the declarations in scope.
fn ns_of(name: &str, scope: &[(String, String)]) -> Option<String> {
    let prefix = name.split_once(':').map(|(p, _)| p).unwrap_or("");
    let key = if prefix.is_empty() { "xmlns".to_string() } else { format!("xmlns:{prefix}") };
    scope.iter().rev().find(|(k, _)| *k == key).map(|(_, v)| v.clone())
}

fn scope_with(parent: &[(String, String)], e: &Element) -> Vec<(String, String)> {
    let mut s = parent.to_vec();
    for (k, v) in &e.attrs {
        if k == "xmlns" || k.starts_with("xmlns:") {
            s.push((k.to_string(), v.to_string()));
        }
    }
    s
}

fn matches(e: &Element, step: &Step, maps: &[(String, String)], scope: &[(String, String)]) -> bool {
    if xml::local_name(&e.name) != step.name {
        return false;
    }
    if let Some(p) = &step.prefix {
        if let Some((_, uri)) = maps.iter().find(|(mp, _)| mp == p) {
            if let Some(have) = ns_of(&e.name, scope) {
                return &have == uri;
            }
        }
    }
    true
}

/// Find the element a path of element steps leads to, as indices through the tree.
fn locate(root: &Element, st: &[Step], maps: &[(String, String)]) -> Option<Vec<usize>> {
    let elems: Vec<&Step> = st.iter().filter(|s| !s.attr && !s.text).collect();
    let first = elems.first()?;
    let scope = scope_with(&[], root);
    if !matches(root, first, maps, &scope) {
        return None;
    }
    let mut path = Vec::new();
    let mut cur = root;
    let mut scope = scope;
    for step in elems.iter().skip(1) {
        let mut seen = 0;
        let mut found = None;
        for (i, n) in cur.children.iter().enumerate() {
            if let Node::Element(c) = n {
                let cs = scope_with(&scope, c);
                if matches(c, step, maps, &cs) {
                    seen += 1;
                    if seen == step.index {
                        found = Some((i, c, cs));
                        break;
                    }
                }
            }
        }
        let (i, c, cs) = found?;
        path.push(i);
        cur = c;
        scope = cs;
    }
    Some(path)
}

fn at<'a>(root: &'a Element, path: &[usize]) -> &'a Element {
    let mut cur = root;
    for i in path {
        if let Node::Element(c) = &cur.children[*i] {
            cur = c;
        }
    }
    cur
}

fn at_mut<'a>(root: &'a mut Element, path: &[usize]) -> &'a mut Element {
    let mut cur = root;
    for i in path {
        cur = match &mut cur.children[*i] {
            Node::Element(c) => c,
            _ => unreachable!("path points at an element"),
        };
    }
    cur
}

fn parse_part(pkg: &Package, store: &str) -> Option<(String, XmlDoc)> {
    let part = find_part(pkg, store)?;
    let doc = pkg.xml(&part).ok()?;
    Some((part, doc))
}

/// The text of the node a control is bound to.
pub fn get(pkg: &Package, store: &str, xpath: &str, prefixes: &str) -> Option<String> {
    let (_, doc) = parse_part(pkg, store)?;
    let st = steps(xpath);
    let maps = mappings(prefixes);
    let path = locate(&doc.root, &st, &maps)?;
    let e = at(&doc.root, &path);
    match st.last() {
        Some(s) if s.attr => e.attrs.iter().find(|(k, _)| xml::local_name(k) == s.name).map(|(_, v)| v.to_string()),
        _ => Some(e.text()),
    }
}

/// Set the text of the node a control is bound to. Returns whether the part changed.
pub fn set(pkg: &mut Package, store: &str, xpath: &str, prefixes: &str, value: &str) -> bool {
    let Some((part, mut doc)) = parse_part(pkg, store) else { return false };
    let st = steps(xpath);
    let maps = mappings(prefixes);
    let Some(path) = locate(&doc.root, &st, &maps) else { return false };
    let e = at_mut(&mut doc.root, &path);
    match st.last() {
        Some(s) if s.attr => {
            let key = e.attrs.iter().find(|(k, _)| xml::local_name(k) == s.name).map(|(k, _)| k.to_string()).unwrap_or_else(|| s.name.clone());
            e.set_attr(key, value);
        }
        _ => {
            e.children.retain(|n| matches!(n, Node::Element(_)));
            if !value.is_empty() {
                e.children.insert(0, Node::Text(value.to_string()));
            }
        }
    }
    pkg.set(&part, doc.to_bytes());
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn package() -> Package {
        let mut pkg = Package::default();
        pkg.set("[Content_Types].xml", br#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"/>"#.to_vec());
        pkg.set("customXml/item1.xml", br#"<root xmlns="urn:data"><a>one</a><list><b>x</b><b>y</b></list></root>"#.to_vec());
        pkg.set("customXml/itemProps1.xml", br#"<ds:datastoreItem xmlns:ds="http://schemas.openxmlformats.org/officeDocument/2006/customXml" ds:itemID="{ABC}"/>"#.to_vec());
        pkg.add_rel("customXml/item1.xml", REL_CUSTOM_XML_PROPS, "itemProps1.xml", false);
        pkg
    }

    #[test]
    fn reads_and_writes_a_bound_node() {
        let mut pkg = package();
        let maps = "xmlns:ns0='urn:data'";
        assert_eq!(get(&pkg, "{abc}", "/ns0:root[1]/ns0:a[1]", maps).as_deref(), Some("one"));
        assert_eq!(get(&pkg, "{ABC}", "/ns0:root[1]/ns0:list[1]/ns0:b[2]", maps).as_deref(), Some("y"));
        assert!(set(&mut pkg, "{ABC}", "/ns0:root[1]/ns0:a[1]", maps, "two"));
        assert_eq!(get(&pkg, "{ABC}", "/ns0:root[1]/ns0:a[1]", maps).as_deref(), Some("two"));
        // Another namespace is another node.
        assert_eq!(get(&pkg, "{ABC}", "/ns0:root[1]/ns0:a[1]", "xmlns:ns0='urn:other'"), None);
        assert_eq!(get(&pkg, "{NOPE}", "/ns0:root[1]", maps), None);
    }

    #[test]
    fn broken_prefix_mappings_do_not_panic() {
        let pkg = package();
        for maps in ["xmlns:ns0=", "xmlns:ns0=\u{201D}urn:data\u{201D}", "xmlns:ns0=urn:data", "xmlns:", "xmlns:ns0='urn:data"] {
            // Whatever it resolves to, reading must not panic.
            let _ = get(&pkg, "{ABC}", "/ns0:root[1]/ns0:a[1]", maps);
        }
        assert_eq!(get(&pkg, "{ABC}", "/ns0:root[1]/ns0:a[1]", "xmlns:ns0=\"urn:data\"").as_deref(), Some("one"));
    }
}
