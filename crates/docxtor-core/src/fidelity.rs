//! Round-trip fidelity measurement.
//!
//! Compares two packages (typically an original file and Docxtor's re-save of it)
//! and produces a score. This is how Docxtor turns "we preserve your document" from
//! a promise into a number that CI tracks on every commit.

use crate::package::Package;
use crate::xml::{self, Element, Node};
use serde::Serialize;
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PartDiff {
    pub name: String,
    /// 0..1 structural similarity (1 = semantically identical XML).
    pub similarity: f64,
    pub text_equal: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FidelityReport {
    pub parts_total: usize,
    pub parts_identical: usize,
    pub missing: Vec<String>,
    pub added: Vec<String>,
    pub changed: Vec<PartDiff>,
    /// Weighted score in percent. The main document counts for half.
    pub score: f64,
}

/// Attributes that change on every save in Word and carry no meaning.
fn is_noise_attr(k: &str) -> bool {
    k.starts_with("w:rsid") || k == "w14:paraId" || k == "w14:textId"
}

fn signatures(el: &Element, out: &mut HashMap<String, usize>) {
    let mut attrs: Vec<String> = el.attrs.iter().filter(|(k, _)| !is_noise_attr(k)).map(|(k, v)| format!("{k}={v}")).collect();
    attrs.sort();
    *out.entry(format!("{}[{}]", el.name, attrs.join(","))).or_default() += 1;
    for c in &el.children {
        match c {
            Node::Element(e) => signatures(e, out),
            Node::Text(t) if !t.trim().is_empty() => *out.entry(format!("#text:{t}")).or_default() += 1,
            _ => {}
        }
    }
}

fn xml_similarity(a: &Element, b: &Element) -> f64 {
    let mut sa = HashMap::new();
    let mut sb = HashMap::new();
    signatures(a, &mut sa);
    signatures(b, &mut sb);
    let mut inter = 0usize;
    let mut union = 0usize;
    for (k, &ca) in &sa {
        let cb = sb.get(k).copied().unwrap_or(0);
        inter += ca.min(cb);
        union += ca.max(cb);
    }
    for (k, &cb) in &sb {
        if !sa.contains_key(k) {
            union += cb;
        }
    }
    if union == 0 {
        1.0
    } else {
        inter as f64 / union as f64
    }
}

fn normalized_text(el: &Element) -> String {
    let mut ts = Vec::new();
    el.find_all("w:t", &mut ts);
    ts.iter().map(|t| t.text()).collect::<String>()
}

pub fn compare(original: &Package, saved: &Package) -> FidelityReport {
    let mut missing = Vec::new();
    let mut added = Vec::new();
    let mut changed = Vec::new();
    let mut identical = 0;
    let mut weighted = 0.0;
    let mut weights = 0.0;
    let main = original.main_document_path();
    for p in &original.parts {
        let w = if p.name == main { (original.parts.len().max(2) - 1) as f64 } else { 1.0 };
        weights += w;
        let Some(other) = saved.get(&p.name) else {
            missing.push(p.name.clone());
            continue;
        };
        if other == p.data.as_slice() {
            identical += 1;
            weighted += w;
            continue;
        }
        let is_xml = p.name.ends_with(".xml") || p.name.ends_with(".rels");
        let (sim, text_equal) = if is_xml {
            match (xml::parse(&p.data), xml::parse(other)) {
                (Ok(a), Ok(b)) => {
                    if a.root == b.root {
                        (1.0, true)
                    } else {
                        (xml_similarity(&a.root, &b.root), normalized_text(&a.root) == normalized_text(&b.root))
                    }
                }
                _ => (0.0, false),
            }
        } else {
            (0.0, false)
        };
        if sim >= 1.0 {
            identical += 1;
        } else {
            changed.push(PartDiff { name: p.name.clone(), similarity: sim, text_equal });
        }
        weighted += w * sim;
    }
    for p in &saved.parts {
        if original.get(&p.name).is_none() {
            added.push(p.name.clone());
        }
    }
    FidelityReport {
        parts_total: original.parts.len(),
        parts_identical: identical,
        missing,
        added,
        changed,
        score: if weights > 0.0 { (weighted / weights * 1000.0).round() / 10.0 } else { 100.0 },
    }
}
