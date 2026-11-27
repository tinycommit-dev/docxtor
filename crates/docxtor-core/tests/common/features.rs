//! A saved file for every feature of the engine, for the Open XML validator and LibreOffice (`scripts/validate.mjs`,
//! `scripts/lo-check.mjs`). Included with `#[path = "common/features.rs"] mod features;` (it needs `mod common;` beside it).
//!
//! The source is the op shapes recorded from the whole test suite (`tests/data/op_shapes.jsonl`): each is applied, with
//! ids and positions swapped for live ones (`swap_live`, as the random harness does), to a start document, and the
//! result is saved. Out come:
//!
//!   `kinds/<op>.docx`        the first shape of each op kind that applied and changed the saved file
//!   `shapes/<op>-<n>.docx`   every other shape that applied and changed it (n is the shape's line in op_shapes.jsonl)
//!   `built/<name>.docx`      documents made as a person or an AI would: `docxtor new`, Markdown, a table, a picture...
//!   `index.json`             what each file is, which kinds are covered and which are not, with the reason
//!
//! `DOCXTOR_FEATURES_OUT=<dir>` keeps the files there; without it the files go to a temporary folder and are removed.

#![allow(dead_code)]

use crate::common::*;
use docxtor_core::{ai, Document};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// A 4 by 4 red PNG, so a picture op has a real picture.
pub const PNG: &[u8] = &[
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52, 0x00, 0x00, 0x00, 0x04, 0x00, 0x00, 0x00, 0x04, 0x08, 0x06, 0x00, 0x00, 0x00, 0xa9, 0xf1, 0x9e, 0x7e, 0x00, 0x00, 0x00, 0x12, 0x49, 0x44, 0x41, 0x54,
    0x78, 0x9c, 0x63, 0x38, 0x61, 0xa3, 0xf1, 0x1f, 0x19, 0x33, 0x90, 0x2e, 0x00, 0x00, 0x97, 0x58, 0x22, 0xb1, 0x40, 0x31, 0xb1, 0xb1, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
];

/// Op kinds no recorded shape could apply to a start document in a way that changes the file, and why.
pub const NOT_COVERED: &[(&str, &str)] = &[("expand_alt_chunk", "needs a file that already has an altChunk part (Word's Insert, Text from File makes one; no op does): the op turns that part into paragraphs. tests/all/alt_chunks.rs builds such a file by hand and tests the op; no recorded op shape or built document here makes one")];

/// The op kinds the engine has: the variants of `enum Op` in `src/ops.rs`, in the names the JSON uses.
pub fn op_kinds() -> Vec<String> {
    let src = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/ops.rs")).unwrap();
    let start = src.find("pub enum Op {").expect("enum Op");
    let body = &src[start..];
    let end = body.find("\n}\n").expect("end of enum Op");
    let mut out = vec![];
    for line in body[..end].lines().skip(1) {
        let b = line.as_bytes();
        // A variant sits at four spaces of indent and starts with a capital.
        if line.len() > 4 && line.starts_with("    ") && b[4].is_ascii_uppercase() {
            let name: String = line[4..].chars().take_while(|c| c.is_alphanumeric()).collect();
            let mut snake = String::new();
            for (i, c) in name.chars().enumerate() {
                if c.is_uppercase() {
                    if i > 0 {
                        snake.push('_');
                    }
                    snake.extend(c.to_lowercase());
                } else {
                    snake.push(c);
                }
            }
            out.push(snake);
        }
    }
    out
}

fn with_media(v: &Value, name: &str) -> Value {
    match v {
        Value::Object(m) => Value::Object(m.iter().map(|(k, x)| (k.clone(), if k == "media" && x.is_string() { json!(name) } else { with_media(x, name) })).collect()),
        Value::Array(a) => Value::Array(a.iter().map(|x| with_media(x, name)).collect()),
        o => o.clone(),
    }
}

fn hash(s: &str, n: u64) -> u64 {
    let mut h = 1469598103934665603u64 ^ n.wrapping_mul(0x9E3779B97F4A7C15);
    for b in s.bytes() {
        h = (h ^ b as u64).wrapping_mul(1099511628211);
    }
    h
}

/// The documents a shape is tried on, as saved files (opening bytes is quicker than building `rich` from ops each time).
fn starts() -> Vec<(String, Vec<u8>)> {
    let mut out = vec![];
    for s in [Start::Built("rich"), Start::File("rich.docx".into()), Start::File("complex.docx".into())] {
        out.push((s.label(), s.open().save().unwrap()));
    }
    out
}

/// The built documents as start documents: they have what the first three lack (sources, a chart, shapes, index entries).
fn built_starts() -> Vec<(String, Vec<u8>)> {
    built_documents().into_iter().filter(|(name, _, _)| !name.starts_with("op_")).map(|(name, mut d, _)| (format!("built:{name}"), d.save().unwrap())).collect()
}

/// Applies one shape to a start document with live ids; the saved file when it applied and changed it, else why not.
fn try_shape(shape: &Value, op: &str, line: usize, start: &[u8], base: &[u8], seed: u64) -> Result<Vec<u8>, String> {
    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> Result<Vec<u8>, String> {
        let mut doc = Document::open(start).map_err(|e| e.to_string())?;
        let media = doc.add_media("png", PNG.to_vec());
        let mut rng = Rng::new(hash(op, line as u64 * 31 + seed));
        let live = Live::of(&doc, &mut vec![]);
        let swapped = swap_live(&with_media(shape, &media), &live, &mut rng, op);
        doc.apply_json(&Value::Array(vec![swapped]).to_string(), false).map_err(|e| e.to_string())?;
        let bytes = doc.save().map_err(|e| e.to_string())?;
        if bytes == base {
            Err("changed nothing".into())
        } else {
            Ok(bytes)
        }
    }));
    r.unwrap_or_else(|_| Err("panicked".into()))
}

fn write(dir: &Path, rel: &str, bytes: &[u8]) {
    let p = dir.join(rel);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, bytes).unwrap();
}

// ---------------------------------------------------------------------------
// Documents built the way a person or an AI builds them
// ---------------------------------------------------------------------------

fn paragraphs(doc: &Document) -> Vec<u32> {
    docxtor_core::model::paragraph_order(&doc.body)
}

/// A blank document with a heading and three paragraphs of text, for the built documents to start from.
fn start_text() -> Document {
    let mut doc = Document::new_blank();
    let first = paragraphs(&doc)[0];
    let run = |doc: &mut Document, v: Value| {
        doc.apply_json(&v.to_string(), false).unwrap_or_else(|e| panic!("building a feature file: {e}: {v}"));
    };
    run(&mut doc, json!({"op":"insert_text","at":{"p":first,"o":0},"text":"Introduction to the document"}));
    run(&mut doc, json!({"op":"set_style","ids":[first],"style":"Heading1"}));
    let mut last = first;
    for t in ["The first paragraph of the body, with a few words to point at.", "A second paragraph, for lists and notes.", "A third paragraph, for a table after it."] {
        run(&mut doc, json!({"op":"insert_paragraph","after":last,"text":t}));
        last = *paragraphs(&doc).last().unwrap();
    }
    doc
}

/// `"$3"` is the id of the fourth paragraph, `"$last"` the last one (a number in an op, "p<id>" in an AI tool's arguments); `"$media"` a picture.
fn fill(v: &Value, ps: &[u32], media: &str, tool: bool) -> Value {
    match v {
        Value::String(s) if s == "$media" => json!(media),
        Value::String(s) if s == "$last" => {
            let id = *ps.last().unwrap();
            if tool {
                json!(format!("p{id}"))
            } else {
                json!(id)
            }
        }
        Value::String(s) if s.starts_with('$') && s[1..].parse::<usize>().is_ok() => {
            let id = ps[s[1..].parse::<usize>().unwrap()];
            if tool {
                json!(format!("p{id}"))
            } else {
                json!(id)
            }
        }
        Value::Object(m) => Value::Object(m.iter().map(|(k, x)| (k.clone(), fill(x, ps, media, tool))).collect()),
        Value::Array(a) => Value::Array(a.iter().map(|x| fill(x, ps, media, tool)).collect()),
        o => o.clone(),
    }
}

/// The op kinds the steps of the built documents use, by document.
pub fn ops_of_built() -> BTreeMap<String, Vec<String>> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/feature_builds.json");
    let list: Vec<Value> = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let mut out: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for b in list {
        for step in b["steps"].as_array().unwrap() {
            for op in step["ops"].as_array().into_iter().flatten() {
                if let Some(k) = op["op"].as_str() {
                    let e = out.entry(k.to_string()).or_default();
                    let name = b["name"].as_str().unwrap().to_string();
                    if !e.contains(&name) {
                        e.push(name);
                    }
                }
            }
        }
    }
    out
}

/// (name, the document, the text its PDF must show): what the CLI and the AI tools produce, one thing at a time. The
/// steps are in `tests/data/feature_builds.json`: each is `{"ops": [...]}` or `{"tool": name, "args": {...}}`.
pub fn built_documents() -> Vec<(String, Document, Vec<String>)> {
    built_documents_with_markup().into_iter().map(|(n, d, e, _)| (n, d, e)).collect()
}

/// The same with the markup each must have written somewhere in its package (the riskiest markup: the legacy
/// password hash, a gradient page background, a 3-D page border, GREETINGLINE, the CJK number
/// formats, the phonetic guide settings, tight wrapping).
pub fn built_documents_with_markup() -> Vec<(String, Document, Vec<String>, Vec<String>)> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/feature_builds.json");
    let list: Vec<Value> = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let mut out = vec![];
    for b in list {
        let name = b["name"].as_str().unwrap().to_string();
        let mut doc = if b["start"] == "blank" { Document::new_blank() } else { start_text() };
        let media = doc.add_media("png", PNG.to_vec());
        for step in b["steps"].as_array().unwrap() {
            let ps = paragraphs(&doc);
            if let Some(t) = step["tool"].as_str() {
                let args = fill(&step["args"], &ps, &media, true);
                ai::call_tool(&mut doc, t, &args).unwrap_or_else(|e| panic!("feature file {name}: tool {t}: {e}: {args}"));
            } else {
                let ops = fill(&step["ops"], &ps, &media, false);
                doc.apply_json(&ops.to_string(), false).unwrap_or_else(|e| panic!("feature file {name}: {e}: {ops}"));
            }
        }
        let expect = b["expect"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().to_string()).collect();
        let markup = b["markup"].as_array().map(|a| a.iter().map(|v| v.as_str().unwrap().to_string()).collect()).unwrap_or_default();
        out.push((name, doc, expect, markup));
    }
    out
}

// ---------------------------------------------------------------------------
// The run
// ---------------------------------------------------------------------------

pub struct Report {
    pub dir: PathBuf,
    pub covered: Vec<String>,
    pub uncovered: Vec<String>,
    pub only_built: Vec<String>,
    pub last_refusal: BTreeMap<String, String>,
    pub kinds: usize,
    pub shapes: usize,
    pub built: usize,
}

/// Writes every feature file under `dir` and `index.json`.
pub fn write_all(dir: &Path, mode: &str) -> Report {
    let all = op_kinds();
    let shapes = load_shapes_with_lines();
    let mut starts = starts();
    let n_plain = starts.len();
    starts.extend(built_starts());
    // What each start saves when nothing is applied: a shape that leaves this unchanged did nothing.
    let bases: Vec<Vec<u8>> = starts
        .iter()
        .map(|(_, bytes)| {
            let mut d = Document::open(bytes).unwrap();
            d.add_media("png", PNG.to_vec());
            d.save().unwrap()
        })
        .collect();
    let mut index: BTreeMap<String, Value> = BTreeMap::new();
    let mut covered: BTreeMap<String, usize> = BTreeMap::new();
    let mut why: BTreeMap<String, String> = BTreeMap::new();
    let (mut n_kinds, mut n_shapes) = (0, 0);
    // mode "kinds": one file per op kind only (the quick run in CI); "none": only the built documents; "": every shape.
    for (line, shape) in &shapes {
        let Some(op) = shape.get("op").and_then(Value::as_str) else { continue };
        if mode == "none" || (mode == "kinds" && covered.contains_key(op)) {
            continue;
        }
        'found: for (si, (label, start)) in starts.iter().enumerate() {
            // The built documents are tried only for an op the first three starts could not do (and with fewer seeds).
            if si >= n_plain && covered.contains_key(op) {
                break;
            }
            for seed in 0..(if si < n_plain { 4u64 } else { 2 }) {
                let bytes = match try_shape(shape, op, *line, start, &bases[si], seed) {
                    Ok(b) => b,
                    Err(e) => {
                        why.insert(op.to_string(), format!("{e} (shape {line}, start {label})"));
                        continue;
                    }
                };
                let first = !covered.contains_key(op);
                let rel = if first { format!("kinds/{op}.docx") } else { format!("shapes/{op}-{line}.docx") };
                write(dir, &rel, &bytes);
                *covered.entry(op.to_string()).or_default() += 1;
                if first {
                    n_kinds += 1;
                } else {
                    n_shapes += 1;
                }
                index.insert(rel, json!({"op": op, "shape": line, "start": label, "seed": seed, "ops": shape}));
                break 'found;
            }
        }
    }
    let built = built_documents_with_markup();
    let n_built = built.len();
    for (name, mut doc, expect, markup) in built {
        let bytes = doc.save().unwrap();
        // The markup the document is there to write must be in the package.
        let pkg = docxtor_core::package::Package::open(&bytes).unwrap();
        let all: String = pkg.parts.iter().filter(|p| p.name.ends_with(".xml") || p.name.ends_with(".rels") || p.name.ends_with(".vml")).map(|p| String::from_utf8_lossy(&p.data).into_owned()).collect();
        for m in &markup {
            assert!(all.contains(m.as_str()), "feature file built/{name}.docx was to have {m} and has not");
        }
        let rel = format!("built/{name}.docx");
        write(dir, &rel, &bytes);
        index.insert(rel, json!({"built": name, "expect": expect}));
    }
    // An op kind a built document uses counts as covered: the document is saved and checked like the others.
    let by_built = ops_of_built();
    // Files in the other formats Docxtor opens (.doc, .odt, .rtf, .html, Word 2003 XML), converted as `docxtor convert` does.
    let mut formats: Vec<PathBuf> = std::fs::read_dir(repo_path("fixtures/formats")).map(|rd| rd.flatten().map(|e| e.path()).collect()).unwrap_or_default();
    formats.sort();
    let mut n_formats = 0;
    for path in formats {
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        let Ok((docx, _)) = docxtor_core::open::to_docx(&std::fs::read(&path).unwrap(), &name) else { continue };
        let rel = format!("built/format_{}.docx", name.replace('.', "_"));
        write(dir, &rel, &docx);
        index.insert(rel, json!({"built": name, "expect": []}));
        n_formats += 1;
    }
    let n_built = n_built + n_formats;
    let uncovered: Vec<String> = all.iter().filter(|k| !covered.contains_key(*k) && !by_built.contains_key(*k)).cloned().collect();
    let only_built: Vec<String> = all.iter().filter(|k| !covered.contains_key(*k) && by_built.contains_key(*k)).cloned().collect();
    let last_refusal: BTreeMap<&String, &String> = why.iter().filter(|(k, _)| uncovered.contains(k)).collect();
    let idx = json!({"files": index, "covered": covered, "covered_by_built_documents": only_built, "uncovered": uncovered, "last_refusal": last_refusal, "op_kinds": all.len()});
    write(dir, "index.json", serde_json::to_string_pretty(&idx).unwrap().as_bytes());
    let last_refusal = why.iter().filter(|(k, _)| uncovered.contains(k)).map(|(k, v)| (k.clone(), v.clone())).collect();
    Report { dir: dir.to_path_buf(), covered: covered.keys().cloned().collect(), uncovered, only_built, last_refusal, kinds: n_kinds, shapes: n_shapes, built: n_built }
}

/// The recorded shapes with their line numbers (1-based) in op_shapes.jsonl.
pub fn load_shapes_with_lines() -> Vec<(usize, Value)> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/op_shapes.jsonl");
    std::fs::read_to_string(path).unwrap_or_default().lines().enumerate().filter_map(|(i, l)| serde_json::from_str(l).ok().map(|v| (i + 1, v))).collect()
}
