//! The list of cases the damaged-file harness (`tests/mutate_open.rs`) draws from. Each family is a group of
//! entry points with the starting files and mutations that make sense for it. A case marked `fixed` is part of the
//! small set `cargo test` runs; the long run draws from everything, with new seeds and sizes.

use crate::mutations::*;
use docxtor_core::package::Package;
use serde_json::{json, Value};

pub struct Cand {
    pub case: Value,
    pub fixed: bool,
}

pub struct Family {
    pub name: &'static str,
    pub weight: usize,
    pub cands: Vec<Cand>,
}

fn mk(target: &str, fixture: &str, part: &str, m: Value, seed: u64, fixed: bool) -> Cand {
    Cand { case: json!({"target": target, "fixture": fixture, "part": part, "mutation": m, "seed": seed}), fixed }
}

/// Counts are 100,000; sizes are 4 MB.
const BIG: u64 = 100_000;

const DOCX_WRAPS: &[(&str, &str, &str, &str)] = &[
    ("cell", "<w:tbl><w:tr><w:tc>", "</w:tc></w:tr></w:tbl>", "w:p"),
    ("sdt-block", "<w:sdt><w:sdtContent>", "</w:sdtContent></w:sdt>", "w:p"),
    ("sdt-run", "<w:sdt><w:sdtContent>", "</w:sdtContent></w:sdt>", "w:r"),
    ("hyperlink", "<w:hyperlink w:anchor=\"a\">", "</w:hyperlink>", "w:r"),
    ("ins", "<w:ins w:id=\"1\" w:author=\"a\">", "</w:ins>", "w:r"),
    ("smart-tag", "<w:smartTag w:uri=\"u\" w:element=\"e\">", "</w:smartTag>", "w:r"),
    ("field", "<w:fldSimple w:instr=\"PAGE\">", "</w:fldSimple>", "w:r"),
    ("textbox", "<w:p><w:r><w:pict><v:shape xmlns:v=\"urn:schemas-microsoft-com:vml\"><v:textbox><w:txbxContent>", "</w:txbxContent></v:textbox></v:shape></w:pict></w:r></w:p>", "w:p"),
    ("plain", "<w:zz>", "</w:zz>", "w:p"),
    ("para-in-para", "<w:p>", "</w:p>", "w:r"),
    ("alt-content", "<mc:AlternateContent><mc:Choice Requires=\"w14\">", "</mc:Choice></mc:AlternateContent>", "w:p"),
    ("body-in-body", "<w:body>", "</w:body>", "w:p"),
];

const ODT_WRAPS: &[(&str, &str, &str, &str)] = &[
    ("list", "<text:list><text:list-item>", "</text:list-item></text:list>", "text:p"),
    ("table", "<table:table><table:table-row><table:table-cell>", "</table:table-cell></table:table-row></table:table>", "text:p"),
    ("section", "<text:section>", "</text:section>", "text:p"),
    ("span", "<text:span>", "</text:span>", "text:span"),
    ("frame", "<draw:frame><draw:text-box>", "</draw:text-box></draw:frame>", "text:p"),
    ("plain", "<office:zz>", "</office:zz>", "text:p"),
];

/// Attributes that hold a size or a count. A huge number here can mean a huge table, page or picture.
const NUMBERS: &[(&str, &str)] = &[
    ("w:gridSpan", "w:val"),
    ("w:pgSz", "w:w"),
    ("w:pgSz", "w:h"),
    ("w:cols", "w:num"),
    ("w:cols", "w:space"),
    ("w:gridCol", "w:w"),
    ("w:tblW", "w:w"),
    ("w:tcW", "w:w"),
    ("w:ind", "w:left"),
    ("w:spacing", "w:line"),
    ("w:spacing", "w:before"),
    ("w:sz", "w:val"),
    ("w:numId", "w:val"),
    ("w:ilvl", "w:val"),
    ("w:start", "w:val"),
    ("w:trHeight", "w:val"),
    ("w:pgMar", "w:top"),
    ("wp:extent", "cx"),
    ("wp:extent", "cy"),
    ("a:ext", "cx"),
    ("w:tab", "w:pos"),
    ("w:lnNumType", "w:countBy"),
    ("w:startOverride", "w:val"),
    ("w:abstractNumId", "w:val"),
    ("w:bookmarkStart", "w:id"),
    ("w:footnoteReference", "w:id"),
    ("w:pgNumType", "w:start"),
    ("w:docGrid", "w:linePitch"),
    ("w:defaultTabStop", "w:val"),
    ("w:vMerge", "w:val"),
    ("table:table-cell", "table:number-columns-repeated"),
    ("table:table-cell", "table:number-columns-spanned"),
    ("table:table-row", "table:number-rows-repeated"),
    ("text:s", "text:c"),
];

const REPEATS: &[&str] = &[
    "w:p", "w:r", "w:t", "w:tr", "w:tc", "w:tbl", "w:sdt", "w:style", "w:abstractNum", "w:lvl", "w:num", "w:footnote", "w:bookmarkStart", "w:hyperlink", "w:tab", "w:br", "Relationship", "Override", "text:p", "text:span", "table:table-row", "table:table-cell", "style:style", "c", "row", "si", "sheet",
];

const CORE_REPEATS: &[&str] = &["w:p", "w:r", "w:tr", "w:tc", "w:style", "Relationship", "Override", "text:p", "table:table-cell", "row", "c"];

fn looks_like_xml(d: &[u8]) -> bool {
    let s = d.iter().find(|b| !(b.is_ascii_whitespace() || **b == 0xEF || **b == 0xBB || **b == 0xBF));
    s == Some(&b'<')
}

/// What to do to a blob of any kind: the cut-short, emptied, flipped and wrong-type versions.
fn generic_menu(core: bool) -> Vec<(Value, u64, bool)> {
    let mut v = vec![(json!({"kind": "empty"}), 0, core)];
    for p in [100, 500, 900] {
        v.push((json!({"kind": "truncate", "permille": p}), 0, core && p == 500));
    }
    for (count, seed) in [(1, 1), (8, 2), (8, 3), (64, 4)] {
        v.push((json!({"kind": "flip", "count": count, "head": seed % 2 == 0}), seed, core && seed == 2));
    }
    for with in ["png", "zip", "random", "text", "xml", "json", "cfb", "bom16"] {
        v.push((json!({"kind": "wrong", "with": with}), 5, core && matches!(with, "png" | "random")));
    }
    v
}

/// What to do to an XML text, whatever its vocabulary.
fn xml_menu(data: &[u8], core: bool, big_docs: bool) -> Vec<(Value, u64, bool)> {
    let t = String::from_utf8_lossy(data);
    let mut v = Vec::new();
    let present: Vec<String> = tags_of(&t).into_iter().filter(|g| find_element(&t, g).is_some()).collect();
    for tag in REPEATS {
        if present.iter().any(|p| p == tag) {
            v.push((json!({"kind": "repeat", "tag": tag, "n": BIG}), 0, core && CORE_REPEATS.contains(tag)));
        }
    }
    if let Some(last) = present.last().filter(|l| !REPEATS.contains(&l.as_str())) {
        v.push((json!({"kind": "repeat", "tag": last, "n": BIG}), 0, false));
    }
    v.push((json!({"kind": "nest", "mode": "root", "label": "plain", "open": "<w:zz>", "close": "</w:zz>", "n": BIG}), 0, core));
    let wraps: &[(&str, &str, &str, &str)] = if t.contains("<w:p") {
        DOCX_WRAPS
    } else if t.contains("<text:p") {
        ODT_WRAPS
    } else {
        &[]
    };
    for (label, open, close, tag) in wraps {
        if find_element(&t, tag).is_some() {
            v.push((json!({"kind": "nest", "mode": "around", "tag": tag, "label": label, "open": open, "close": close, "n": BIG}), 0, core && big_docs && matches!(*label, "cell" | "sdt-block" | "plain" | "hyperlink" | "textbox")));
        }
    }
    for (elem, attr) in NUMBERS {
        if t.contains(&format!("<{elem} ")) || t.contains(&format!("<{elem}>")) {
            let hot = matches!((*elem, *attr), ("w:gridSpan", _) | ("w:pgSz", _) | ("w:cols", "w:num") | ("wp:extent", _) | ("w:numId", _) | ("w:ilvl", _) | ("w:sz", _) | ("table:table-cell", _) | ("table:table-row", _));
            for (value, keep) in [("2147483647", true), ("99999999999999999999", false), ("-2147483648", false), ("4294967296", false)] {
                v.push((json!({"kind": "number", "elem": elem, "attr": attr, "value": value}), 0, core && hot && keep));
            }
        }
    }
    v.push((json!({"kind": "number", "value": "2147483647"}), 0, core));
    v.push((json!({"kind": "number", "value": "99999999999999999999"}), 0, false));
    v.push((json!({"kind": "attr", "size": 1 << 22, "ch": "9"}), 0, core));
    v.push((json!({"kind": "attr", "size": 1 << 22, "ch": "A"}), 0, false));
    v
}

/// What to do to a part of a Word package or an OpenDocument file.
fn part_menu(part: &str, data: &[u8], core: bool, big_docs: bool) -> Vec<(Value, u64, bool)> {
    let mut v = generic_menu(core);
    if looks_like_xml(data) {
        v.extend(xml_menu(data, core, big_docs));
    }
    // A part written as another part of the same file.
    v.push((json!({"kind": "wrong", "with": "other:word/styles.xml"}), 0, false));
    v.push((json!({"kind": "wrong", "with": "other:word/document.xml"}), 0, false));
    let _ = part;
    v
}

fn zip_offsets(bytes: &[u8]) -> Vec<(usize, &'static str)> {
    let mut out = Vec::new();
    let mut i = 0;
    while i + 4 <= bytes.len() {
        match &bytes[i..i + 4] {
            [0x50, 0x4B, 3, 4] => {
                out.push((i + 18, "compressed size"));
                out.push((i + 22, "unpacked size"));
                out.push((i + 26, "name length"));
                i += 30;
            }
            [0x50, 0x4B, 1, 2] => {
                out.push((i + 20, "central compressed size"));
                out.push((i + 24, "central unpacked size"));
                out.push((i + 28, "central name length"));
                out.push((i + 42, "central offset"));
                i += 46;
            }
            [0x50, 0x4B, 5, 6] => {
                out.push((i + 10, "entry count"));
                out.push((i + 12, "directory size"));
                out.push((i + 16, "directory offset"));
                i += 22;
            }
            _ => i += 1,
        }
    }
    out
}

const POKES: &[u64] = &[0, 1, 0xFFFF_FFFF, 0x7FFF_FFFF, 0x8000_0000, 0x0100_0000, 0xFFFF_FFFE];

fn zip_level(target: &str, fixture: &str, bytes: &[u8], core: bool) -> Vec<Cand> {
    let mut v = Vec::new();
    for (count, seed) in [(1u64, 11u64), (16, 12), (16, 13), (200, 14)] {
        v.push(mk(target, fixture, "(zip)", json!({"kind": "flip", "count": count, "head": seed % 2 == 0}), seed, core && seed == 12));
    }
    for p in [50, 400, 800, 990] {
        v.push(mk(target, fixture, "(zip)", json!({"kind": "truncate", "permille": p}), 0, core && p == 800));
    }
    let offs = zip_offsets(bytes);
    // The size fields of the first entries and the directory: a size much larger than the data is a memory test.
    for (k, (off, name)) in offs.iter().enumerate().filter(|(k, _)| *k < 24 || k % 7 == 0) {
        let _ = (k, name);
        for (j, value) in POKES.iter().enumerate() {
            v.push(mk(target, fixture, "(zip)", json!({"kind": "poke", "offset": off, "width": 4, "value": value, "what": name}), 0, core && *off < 100 && j == 2));
        }
    }
    v
}

/// Everything that can be done to a Word package (or an OpenDocument one): every part, the relationships, the parts
/// as a set, and the zip itself.
fn zip_family(fixtures: &[&str], target: &str, core_files: &[&str]) -> Vec<Cand> {
    let mut v = Vec::new();
    for f in fixtures {
        let base = fixture(f);
        let pkg = Package::open(&base).expect(f);
        let core = core_files.contains(f);
        let rich = *f == "rich.docx";
        let main = pkg.main_document_path();
        for p in &pkg.parts {
            let is_main = p.name == main || p.name == "content.xml";
            let part_core = core && (is_main || (rich && matches!(p.name.as_str(), "word/styles.xml" | "word/numbering.xml" | "word/header1.xml" | "word/settings.xml")) || (!rich && matches!(p.name.as_str(), "word/styles.xml" | "[Content_Types].xml")));
            let menu = part_menu(&p.name, &p.data, part_core, core && is_main);
            for (m, seed, fixed) in menu {
                // The fixed set takes the full menu for the main part and a few for the others.
                let keep = fixed && (is_main || !matches!(m["kind"].as_str(), Some("number" | "attr")) || p.name.ends_with(".rels"));
                v.push(mk(target, f, &p.name, m, seed, keep));
            }
            if p.name.ends_with(".rels") {
                for how in ["self", "missing", "main", "rels", "root", "traversal", "empty", "styles", "content-types"] {
                    v.push(mk(target, f, &p.name, json!({"kind": "rel", "how": how}), 0, core && matches!(how, "self" | "missing" | "main" | "rels") && (p.name == "_rels/.rels" || p.name.ends_with("document.xml.rels"))));
                }
                for ty in ["header", "footer", "footnotes", "endnotes", "comments", "numbering", "styles", "settings", "theme", "image", "aFChunk", "subDocument", "attachedTemplate", "glossaryDocument", "officeDocument"] {
                    for how in ["self", "missing", "main"] {
                        v.push(mk(target, f, &p.name, json!({"kind": "rel-add", "how": how, "type": ty}), 0, core && p.name.ends_with("document.xml.rels") && how == "self" && matches!(ty, "header" | "aFChunk" | "officeDocument")));
                    }
                }
            }
        }
        if pkg.has(&main) && main.starts_with("word/") {
            for (depth, fan, with) in [(1, 1, "simple"), (1, 1, "self"), (6, 1, "simple"), (40, 1, "simple"), (30, 2, "simple"), (12, 2, "rich"), (200, 1, "simple")] {
                v.push(mk(target, f, &main, json!({"kind": "chunk", "depth": depth, "fan": fan, "with": with}), 0, core && matches!((depth, fan, with), (1, 1, "self") | (30, 2, "simple") | (200, 1, "simple"))));
            }
            for how in ["main", "rels"] {
                v.push(mk(target, f, &main, json!({"kind": "chunk-self", "how": how}), 0, core && how == "main"));
            }
            v.push(mk(target, f, &main, json!({"kind": "dup-part"}), 0, core));
            v.push(mk(target, f, &main, json!({"kind": "odd-name"}), 1, false));
            v.push(mk(target, f, &main, json!({"kind": "odd-name"}), 2, false));
            v.push(mk(target, f, &main, json!({"kind": "odd-name"}), 3, core));
            v.push(mk(target, f, "(package)", json!({"kind": "many-parts", "n": BIG}), 0, core));
            v.push(mk(target, f, &main, json!({"kind": "bomb", "size": 1u64 << 30}), 0, core));
            v.push(mk(target, f, "word/styles.xml", json!({"kind": "bomb", "size": 3u64 << 29}), 0, false));
        } else if f.ends_with(".odt") {
            v.push(mk(target, f, "(package)", json!({"kind": "many-parts", "n": BIG}), 0, core));
            v.push(mk(target, f, "content.xml", json!({"kind": "bomb", "size": 1u64 << 30}), 0, core));
        }
        v.extend(zip_level(target, f, &base, core));
    }
    v
}

/// Text and binary files that are not zips: the whole file is the part.
fn file_family(target: &str, fixture_name: &str, extra: Vec<(Value, u64, bool)>, core: bool) -> Vec<Cand> {
    let base = fixture(fixture_name);
    let mut v = Vec::new();
    for (m, seed, fixed) in generic_menu(core) {
        v.push(mk(target, fixture_name, "(file)", m, seed, fixed));
    }
    if looks_like_xml(&base) {
        for (m, seed, fixed) in xml_menu(&base, core, true) {
            v.push(mk(target, fixture_name, "(file)", m, seed, fixed));
        }
    }
    for (m, seed, fixed) in extra {
        v.push(mk(target, fixture_name, "(file)", m, seed, fixed && core));
    }
    v
}

fn rtf_extras() -> Vec<(Value, u64, bool)> {
    let nest = |label: &str, open: &str, close: &str, lit: &str| json!({"kind": "nest", "mode": "inner", "label": label, "open": open, "close": close, "lit": lit, "n": BIG});
    vec![
        (nest("group", "{", "}", "\\rtf1"), 0, true),
        (nest("group-in-text", "{\\b ", "}", "Hello"), 0, false),
        (json!({"kind": "repeat-lit", "lit": "{\\b x}", "at": "before-last:}", "n": BIG}), 0, true),
        (json!({"kind": "repeat-lit", "lit": "\\par ", "at": "before-last:}", "n": BIG}), 0, false),
        (json!({"kind": "repeat-lit", "lit": "\\trowd\\cellx1000\\intbl x\\cell\\row ", "at": "before-last:}", "n": BIG}), 0, true),
        (json!({"kind": "repeat-lit", "lit": "\\cellx1000", "at": "after:\\trowd", "n": BIG}), 0, false),
        (json!({"kind": "repeat-lit", "lit": "\\trowd\\cellx1000\\intbl\\itap9 x\\cell\\row ", "at": "before-last:}", "n": 10_000}), 0, false),
        (json!({"kind": "repeat-lit", "lit": "{\\field{\\*\\fldinst PAGE}{\\fldrslt 1}}", "at": "before-last:}", "n": BIG}), 0, false),
        (json!({"kind": "repeat-lit", "lit": "\\fs99999999999999999999 \\li2147483647 \\ri-2147483648 \\sb4294967296 ", "at": "before-last:}", "n": 1}), 0, true),
        (json!({"kind": "repeat-lit", "lit": "\\u99999999999 \\u-99999 \\uc999999 \\'zz \\bin999999999 ", "at": "before-last:}", "n": 1}), 0, false),
        (json!({"kind": "repeat-lit", "lit": "{\\pict\\emfblip\\picw99999999\\pich99999999 0102030405}", "at": "before-last:}", "n": 100}), 0, false),
        (json!({"kind": "repeat-lit", "lit": "{\\*\\shppict{\\pict\\wmetafile8\\picw99999\\pich99999 01000900000300}}", "at": "before-last:}", "n": 100}), 0, false),
        (json!({"kind": "repeat-lit", "lit": "\\ls1\\ilvl8 \\listid99999999 ", "at": "before-last:}", "n": BIG}), 0, false),
        (json!({"kind": "fill", "after": "\\rtf1", "ch": "9", "size": 1 << 22}), 0, true),
        (json!({"kind": "fill", "after": "Hello", "ch": "a", "size": 1 << 24}), 0, false),
        (json!({"kind": "repeat-lit", "lit": "{\\colortbl;\\red255\\green0\\blue0;}", "at": "before-last:}", "n": BIG}), 0, false),
        (json!({"kind": "repeat-lit", "lit": "{\\fonttbl{\\f0 Arial;}}", "at": "before-last:}", "n": BIG}), 0, false),
    ]
}

fn html_extras() -> Vec<(Value, u64, bool)> {
    let nest = |label: &str, open: &str, close: &str| json!({"kind": "nest", "mode": "inner", "label": label, "open": open, "close": close, "lit": "<body>", "n": BIG});
    let at_body = |lit: &str, n: u64| json!({"kind": "repeat-lit", "lit": lit, "at": "after:<body>", "n": n});
    vec![
        (nest("div", "<div>", "</div>"), 0, true),
        (nest("blockquote", "<blockquote>", "</blockquote>"), 0, false),
        (nest("list", "<ul><li>", "</li></ul>"), 0, true),
        (nest("table", "<table><tr><td>", "</td></tr></table>"), 0, true),
        (nest("span", "<span>", "</span>"), 0, false),
        (at_body("<b>", BIG), 0, true),
        (at_body("<div>", BIG), 0, false),
        (at_body("<td>", BIG), 0, false),
        (at_body("<table><tr><td>x</td></tr></table>", BIG), 0, false),
        (at_body("<p>x</p>", BIG), 0, true),
        (at_body("<li>x", BIG), 0, false),
        (at_body("<br>", BIG), 0, false),
        (at_body("<span style=\"font-size:1px\">", BIG), 0, false),
        (at_body("<table><tr><td colspan=\"4294967295\" rowspan=\"65535\">x</td></tr></table>", 1), 0, true),
        (at_body("<table><tr><td colspan=\"99999999999999999999\">x</td></tr></table>", 1), 0, false),
        (at_body("<ol start=\"4294967295\"><li>x</li></ol>", 1), 0, false),
        (at_body("<img src=\"data:image/png;base64,AAAA\" width=\"99999999\" height=\"99999999\">", 1), 0, false),
        (at_body("<font size=\"99999999\" color=\"#zz\">x</font>", 1), 0, false),
        (at_body("<p style=\"margin-left:99999999999px;text-indent:-99999999999px;width:1e308px\">x</p>", 1), 0, false),
        (at_body("<table><tr>", 1), 0, false),
        (at_body("<table width=\"99999999999\"><colgroup><col span=\"4294967295\"></colgroup><tr><td>x</td></tr></table>", 1), 0, true),
        (json!({"kind": "fill", "after": "<body>", "ch": "a", "size": 1 << 24}), 0, false),
        (json!({"kind": "fill", "after": "<body><p>", "ch": "&", "size": 1 << 22}), 0, false),
        (json!({"kind": "repeat-lit", "lit": "&#99999999999;&#xFFFFFFFF;&bogus;", "at": "after:<body>", "n": BIG}), 0, false),
    ]
}

fn doc_pokes(bytes: &[u8]) -> Vec<(Value, u64, bool)> {
    let mut v = Vec::new();
    let u32_at = |o: usize| bytes.get(o..o + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]])).unwrap_or(0);
    // The header's fields: sector sizes, the FAT, the directory, the mini FAT, the DIFAT.
    for (i, off) in [0x1Eusize, 0x20, 0x28, 0x2C, 0x30, 0x38, 0x3C, 0x40, 0x44, 0x48, 0x4C, 0x50].iter().enumerate() {
        for (j, value) in [0u64, 1, 0xFFFF_FFFF, 0xFFFF_FFFE, 0x7FFF_FFFF, 0x1000_0000].iter().enumerate() {
            let width = if matches!(*off, 0x1E | 0x20) { 2 } else { 4 };
            v.push((json!({"kind": "poke", "offset": off, "width": width, "value": value, "what": "cfb header"}), 0, i % 3 == 0 && j == 2));
        }
    }
    // The directory entries: left, right and child ids, start sector, size.
    let first = u32_at(0x30) as usize;
    if first < 4096 {
        let dir = (first + 1) * 512;
        for k in 0..6usize {
            for (i, rel) in [0x44usize, 0x48, 0x4C, 0x74, 0x78].iter().enumerate() {
                for (j, value) in [0u64, 1, 0xFFFF_FFFE, 0x7FFF_FFFF, 0xFFFF_FFFF].iter().enumerate() {
                    v.push((json!({"kind": "poke", "offset": dir + k * 128 + rel, "width": 4, "value": value, "what": "cfb directory"}), 0, k < 2 && i % 2 == 0 && j == 3));
                }
            }
        }
    }
    v
}

fn emf_pokes(bytes: &[u8]) -> Vec<(Value, u64, bool)> {
    let mut v = Vec::new();
    let u32_at = |o: usize| bytes.get(o..o + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]])).unwrap_or(0) as usize;
    let len = bytes.len() as u64;
    let mut o = 0;
    let mut k = 0;
    while o + 8 <= bytes.len() && k < 40 {
        let size = u32_at(o + 4).max(8);
        // Clamped lengths stay inside the file; unclamped ones run past it.
        let rest = len - o as u64;
        for (j, value) in [0u64, 1, 7, 8, rest + 1, rest + 4096, 0x7FFF_FFFF, 0xFFFF_FFFF, 0xFFFF_FFFC, 12, size as u64 + 4, size as u64 + 1].iter().enumerate() {
            v.push((json!({"kind": "poke", "offset": o + 4, "width": 4, "value": value, "what": "emf record length"}), 0, k < 3 && j % 3 == 0));
        }
        for value in [0u64, 0xFFFF_FFFF, 0x7FFF_FFFF, 0x8000_0000] {
            v.push((json!({"kind": "poke", "offset": o + 8, "width": 4, "value": value, "what": "emf first field"}), 0, false));
        }
        o += size;
        k += 1;
    }
    // The header: bounds, frame, bytes, records, handles, palette, device sizes.
    for off in (8..88).step_by(4) {
        for value in [0u64, 0xFFFF_FFFF, 0x7FFF_FFFF, 0x8000_0000] {
            v.push((json!({"kind": "poke", "offset": off, "width": 4, "value": value, "what": "emf header"}), 0, off == 48 || off == 52));
        }
    }
    v
}

fn wmf_pokes(bytes: &[u8]) -> Vec<(Value, u64, bool)> {
    let mut v = Vec::new();
    let len = bytes.len() as u64;
    let mut o = 40usize;
    let mut k = 0;
    while o + 6 <= bytes.len() && k < 40 {
        let words = u32::from_le_bytes([bytes[o], bytes[o + 1], bytes[o + 2], bytes[o + 3]]).max(3) as usize;
        let rest = (len - o as u64) / 2;
        for (j, value) in [0u64, 1, 2, rest + 1, rest + 4096, 0x7FFF_FFFF, 0xFFFF_FFFF, words as u64 + 1].iter().enumerate() {
            v.push((json!({"kind": "poke", "offset": o, "width": 4, "value": value, "what": "wmf record length"}), 0, k < 3 && j % 3 == 0));
        }
        o += words * 2;
        k += 1;
    }
    for off in (0..40).step_by(2) {
        for value in [0u64, 0xFFFF, 0x7FFF, 0x8000] {
            v.push((json!({"kind": "poke", "offset": off, "width": 2, "value": value, "what": "wmf header"}), 0, off == 28));
        }
    }
    v
}

fn tiff_pokes() -> Vec<(Value, u64, bool)> {
    let mut v = Vec::new();
    // The tags of the base file, at 10 + 12 * i: width, height, bits, compression, photometric, strip offset,
    // samples, rows per strip, strip byte count.
    let names = ["width", "height", "bits", "compression", "photometric", "strip offset", "samples", "rows per strip", "strip bytes"];
    for (i, name) in names.iter().enumerate() {
        let hot = matches!(i, 0 | 1 | 3 | 5 | 8);
        for value in [0u64, 1, 5, 7, 8, 32773, 0xFFFF, 0x10000, 0x7FFF_FFFF, 0xFFFF_FFFF, 100_000] {
            v.push((json!({"kind": "poke", "offset": 10 + 12 * i + 8, "width": 4, "value": value, "what": format!("tiff {name}")}), 0, hot && matches!(value, 0xFFFF | 0x7FFF_FFFF | 5 | 32773)));
        }
    }
    // The tag type and count, the next-IFD pointer (an IFD that points at itself), the first IFD offset.
    for i in 0..9 {
        for (field, off) in [("type", 10 + 12 * i + 2), ("count", 10 + 12 * i + 4)] {
            for value in [0u64, 0xFFFF_FFFF, 0x7FFF_FFFF] {
                v.push((json!({"kind": "poke", "offset": off, "width": if field == "type" { 2 } else { 4 }, "value": value, "what": format!("tiff tag {field}")}), 0, i == 5 && value == 0xFFFF_FFFF));
            }
        }
    }
    for (name, off, value) in [("next ifd points at itself", 10 + 12 * 9, 8u64), ("first ifd offset", 4, 0xFFFF_FFFF), ("first ifd at the header", 4, 0), ("entry count", 8, 0xFFFF), ("entry count zero", 8, 0)] {
        v.push((json!({"kind": "poke", "offset": off, "width": 4, "value": value, "what": format!("tiff {name}")}), 0, true));
    }
    v
}

fn crypt_extras() -> Vec<(Value, u64, bool)> {
    let mut v = Vec::new();
    for (i, token) in ["spinCount=\"", "saltSize=\"", "keyBits=\"", "blockSize=\"", "hashSize=\"", "<?xml"].iter().enumerate() {
        v.push((json!({"kind": "digits", "token": token}), 0, i == 1 || i == 2));
    }
    v.extend(doc_pokes(&fixture("encrypted/encrypted-agile.docx")).into_iter().map(|(m, s, _)| (m, s, false)));
    v
}

fn ops_cands() -> Vec<Cand> {
    let shapes = crate::common::load_shapes();
    let mut v = Vec::new();
    for (i, _) in shapes.iter().enumerate() {
        let hot = i % 40 == 0;
        for (kind, extra, fixed) in [
            ("huge-number", json!({}), hot),
            ("negative", json!({}), hot),
            ("float", json!({}), false),
            ("huge-string", json!({"size": 1 << 24, "ch": "a"}), hot),
            ("wrong-type", json!({}), hot),
            ("huge-array", json!({"n": BIG}), hot),
            ("odd-string", json!({}), false),
            ("deep-in-op", json!({"n": BIG}), false),
            ("huge-batch", json!({"n": 20_000}), false),
        ] {
            let mut m = json!({"kind": kind, "shape": i});
            for (k, x) in extra.as_object().unwrap() {
                m[k] = x.clone();
            }
            v.push(mk("ops-json", "built:rich", "(ops)", m, (i % 5) as u64 + 1, fixed));
        }
    }
    for (kind, fixed) in [("deep-array", true), ("deep-object", true)] {
        v.push(mk("ops-json", "built:rich", "(ops)", json!({"kind": kind, "shape": 0, "n": BIG}), 0, fixed));
    }
    v
}

fn csv_extras() -> Vec<(Value, u64, bool)> {
    vec![
        (json!({"kind": "repeat-lit", "lit": "Row,row@example.com,Town,1\n", "n": 200_000}), 0, true),
        (json!({"kind": "repeat-lit", "lit": ",x", "at": "before-first-newline", "n": BIG}), 0, true),
        (json!({"kind": "repeat-lit", "lit": ",x", "at": "end", "n": BIG}), 0, false),
        (json!({"kind": "fill", "after": "ada@example.com,", "ch": "a", "size": 1 << 24}), 0, true),
        (json!({"kind": "repeat-lit", "lit": "\"", "at": "end", "n": 1}), 0, false),
        (json!({"kind": "fill", "after": "Wilmslow,", "ch": "\"", "size": 1 << 20}), 0, false),
        (json!({"kind": "repeat-lit", "lit": "\u{0}", "at": "start", "n": 1000}), 0, false),
        (json!({"kind": "repeat-lit", "lit": "\"a\nb\",", "at": "start", "n": BIG}), 0, false),
        (json!({"kind": "repeat-lit", "lit": ";\t|,", "at": "start", "n": BIG}), 0, false),
    ]
}

fn bib_extras() -> Vec<(Value, u64, bool)> {
    vec![
        (json!({"kind": "repeat-lit", "lit": "@article{k, title = {t}, author = {A and B}, year = 2000}\n", "n": BIG}), 0, true),
        (json!({"kind": "nest", "mode": "inner", "label": "braces", "open": "{", "close": "}", "lit": "title = {", "n": BIG}), 0, true),
        (json!({"kind": "nest", "mode": "inner", "label": "braces-in-author", "open": "{", "close": "}", "lit": "author = {", "n": BIG}), 0, false),
        (json!({"kind": "nest", "mode": "inner", "label": "unbalanced-open", "open": "{", "close": "", "lit": "title = {", "n": BIG}), 0, false),
        (json!({"kind": "fill", "after": "title = {", "ch": "a", "size": 1 << 24}), 0, true),
        (json!({"kind": "repeat-lit", "lit": " and Smith, J", "at": "after:author = {Smith, John", "n": BIG}), 0, false),
        (json!({"kind": "repeat-lit", "lit": "@string{zz = zz # zz}\n@article{k, title = zz}\n", "at": "start", "n": 1}), 0, true),
        (json!({"kind": "repeat-lit", "lit": "@string{a = b # \"x\"}\n@string{b = a # \"y\"}\n@article{k, title = a}\n", "at": "start", "n": 1}), 0, false),
        (json!({"kind": "repeat-lit", "lit": "@comment{", "at": "end", "n": BIG}), 0, false),
        (json!({"kind": "repeat-lit", "lit": "@", "at": "end", "n": BIG}), 0, false),
        (json!({"kind": "repeat-lit", "lit": "\\\"{\\'{\\c{", "at": "after:title = {", "n": BIG}), 0, false),
        (json!({"kind": "repeat-lit", "lit": "\\textbf{", "at": "after:title = {", "n": BIG}), 0, false),
        (json!({"kind": "repeat-lit", "lit": "$", "at": "after:title = {", "n": BIG}), 0, false),
    ]
}

fn csljson_extras() -> Vec<(Value, u64, bool)> {
    let wrap = |label: &str, open: &str, close: &str, lit: &str| json!({"kind": "nest", "mode": "around", "label": label, "open": open, "close": close, "lit": lit, "n": BIG});
    vec![
        (wrap("array-around-string", "[", "]", "\"Smith\""), 0, true),
        (wrap("object-around-author", "{\"a\":", "}", "{\"family\":\"Smith\",\"given\":\"John\"}"), 0, true),
        (wrap("array-around-array", "[", "]", "[2020,3,1]"), 0, false),
        (json!({"kind": "repeat-lit", "lit": ",{\"id\":\"x\",\"type\":\"book\",\"title\":\"t\"}", "at": "before-last:]", "n": BIG}), 0, true),
        (json!({"kind": "repeat-lit", "lit": ",{\"family\":\"S\",\"given\":\"J\"}", "at": "before-last:]", "n": 1}), 0, false),
        (json!({"kind": "fill", "after": "\"title\":\"", "ch": "a", "size": 1 << 24}), 0, true),
        (json!({"kind": "repeat-lit", "lit": "[[99999999999999999999,1,1]]", "at": "after:\"date-parts\":", "n": 1}), 0, false),
        (json!({"kind": "repeat-lit", "lit": "\"x\":1,", "at": "after:{", "n": BIG}), 0, false),
        (json!({"kind": "repeat-lit", "lit": "[", "at": "start", "n": BIG}), 0, true),
        (json!({"kind": "repeat-lit", "lit": "{\"a\":", "at": "start", "n": BIG}), 0, false),
    ]
}

fn csl_extras() -> Vec<(Value, u64, bool)> {
    let around = |label: &str, open: &str, close: &str| json!({"kind": "nest", "mode": "around", "tag": "text", "label": label, "open": open, "close": close, "n": BIG});
    vec![
        (json!({"kind": "csl-loop", "how": "self"}), 0, true),
        (json!({"kind": "csl-loop", "how": "pair"}), 0, true),
        (around("group", "<group>", "</group>"), 0, true),
        (around("choose", "<choose><if variable=\"title\">", "</if></choose>"), 0, true),
        (around("names", "<names variable=\"author\"><substitute>", "</substitute></names>"), 0, false),
        (json!({"kind": "repeat", "tag": "text", "n": BIG}), 0, true),
        (json!({"kind": "repeat", "tag": "macro", "n": BIG}), 0, false),
        (json!({"kind": "repeat", "tag": "group", "n": BIG}), 0, false),
        (json!({"kind": "number", "elem": "name", "attr": "et-al-min", "value": "4294967295"}), 0, false),
        (json!({"kind": "number", "elem": "name", "attr": "et-al-use-first", "value": "4294967295"}), 0, false),
        (json!({"kind": "number", "elem": "bibliography", "attr": "second-field-align", "value": "99999999999"}), 0, false),
        (json!({"kind": "nest", "mode": "root", "label": "plain", "open": "<zz>", "close": "</zz>", "n": BIG}), 0, true),
        (json!({"kind": "fill", "after": "<text ", "ch": "x", "size": 1 << 22}), 0, false),
    ]
}

fn xlsx_sheet_cands(target: &str) -> Vec<Cand> {
    let fixture_name = "xlsx:base";
    let base = fixture(fixture_name);
    let pkg = Package::open(&base).expect("base workbook");
    let mut v = Vec::new();
    for p in &pkg.parts {
        let sheet = p.name.contains("sheet1");
        for (m, seed, fixed) in part_menu(&p.name, &p.data, true, false) {
            let keep = fixed && (sheet || p.name.contains("workbook.xml") || p.name.contains("sharedStrings")) && !matches!(m["kind"].as_str(), Some("number" | "attr"));
            v.push(mk(target, fixture_name, &p.name, m, seed, keep));
        }
        if p.name.contains("sheet") || p.name.contains("sharedStrings") {
            for value in ["4294967295", "2147483648", "99999999999"] {
                v.push(mk(target, fixture_name, &p.name, json!({"kind": "text-number", "elem": "v", "value": value}), 0, sheet && value == "4294967295"));
            }
            for (name, ch, size) in [("r", "Z", 20), ("r", "9", 20), ("ref", "9", 30), ("count", "9", 20), ("min", "9", 20), ("max", "9", 20), ("s", "9", 20), ("t", "x", 100)] {
                v.push(mk(target, fixture_name, &p.name, json!({"kind": "attr", "name": name, "ch": ch, "size": size}), 0, sheet && name == "r" && ch == "Z"));
            }
        }
    }
    v.extend(zip_level(target, fixture_name, &base, true).into_iter().map(|mut c| {
        c.fixed = c.fixed && c.case["mutation"]["kind"] == "flip";
        c
    }));
    v
}

pub fn families() -> Vec<Family> {
    let mut f = Vec::new();
    let docx = ["simple.docx", "rich.docx", "complex.docx", "long.docx", "built:rich", "built:plain", "textutil/apple-html.docx", "textutil/apple-rtf.docx"];
    f.push(Family { name: "docx", weight: 34, cands: zip_family(&docx, "open", &["simple.docx", "rich.docx"]) });
    let mut other = zip_family(&["formats/sample.odt"], "open", &["formats/sample.odt"]);
    other.extend(file_family("open", "formats/sample-word2003.xml", vec![], true));
    other.extend(file_family("open", "flat:rich.docx", vec![], false));
    other.extend(file_family("open", "formats/sample.rtf", rtf_extras(), true));
    other.extend(file_family("open", "formats/sample.html", html_extras(), true));
    other.extend(file_family("open", "formats/sample.doc", doc_pokes(&fixture("formats/sample.doc")), true));
    f.push(Family { name: "other formats", weight: 14, cands: other });
    let mut crypt = Vec::new();
    for name in ["encrypted/encrypted-agile.docx", "encrypted/encrypted-standard.docx", "encrypted/encrypted-agile-aes128-sha1.docx"] {
        crypt.extend(file_family("crypt", name, crypt_extras(), name.ends_with("agile.docx") || name.ends_with("standard.docx")).into_iter().map(|mut c| {
            if name.ends_with("standard.docx") {
                c.fixed = c.fixed && c.case["mutation"]["kind"] != "poke";
            }
            c
        }));
    }
    f.push(Family { name: "password-protected", weight: 6, cands: crypt });
    f.push(Family { name: "rtf paste", weight: 5, cands: file_family("rtf-paste", "formats/sample.rtf", rtf_extras(), true) });
    f.push(Family { name: "html paste", weight: 5, cands: file_family("html-paste", "formats/sample.html", html_extras(), true) });
    let mut pics = Vec::new();
    pics.extend(file_family("emf", "emf:base", emf_pokes(&fixture("emf:base")), true));
    pics.extend(file_family("wmf", "wmf:base", wmf_pokes(&fixture("wmf:base")), true));
    pics.extend(file_family("tiff", "tiff:base", tiff_pokes(), true));
    f.push(Family { name: "pictures", weight: 10, cands: pics });
    f.push(Family { name: "recipient list csv", weight: 4, cands: file_family("csv", "csv:base", csv_extras(), true) });
    let mut book = xlsx_sheet_cands("xlsx");
    book.extend(xlsx_sheet_cands("chart-workbook"));
    f.push(Family { name: "workbooks", weight: 6, cands: book });
    f.push(Family { name: "bibtex", weight: 4, cands: file_family("bibtex", "bib:base", bib_extras(), true) });
    f.push(Family { name: "csl-json", weight: 4, cands: file_family("csl-json", "csljson:base", csljson_extras(), true) });
    let mut csl = Vec::new();
    for s in ["elsevier-vancouver.csl", "chicago-notes-bibliography.csl"] {
        let name = format!("csl:{s}");
        csl.extend(file_family("csl", &name, csl_extras(), s.starts_with("elsevier")));
    }
    f.push(Family { name: "csl styles", weight: 4, cands: csl });
    f.push(Family { name: "ops json", weight: 6, cands: ops_cands() });
    let am = zip_family(&["rich.docx"], "automark", &[]);
    f.push(Family { name: "automark file", weight: 1, cands: am.into_iter().filter(|c| c.case["part"] == "word/document.xml").take(60).collect() });
    f
}
