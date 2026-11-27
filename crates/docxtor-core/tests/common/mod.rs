//! Shared pieces of the random op harness (`tests/random_ops.rs`): a small seeded random number generator, the
//! string pool, the live view of a document (which ids, offsets, bookmarks, tabs and controls exist right now), the
//! op generator (hand-written kinds plus the replay of recorded shapes), the documents the harness starts from, and
//! the render-model comparison used by the save-and-reopen checks.

#![allow(dead_code)]

pub mod known;

use docxtor_core::model::{walk_paragraphs, Block};
use docxtor_core::Document;
use serde_json::{json, Map, Value};
use std::collections::HashMap;
use std::path::PathBuf;

// ---------------------------------------------------------------------------
// Random numbers
// ---------------------------------------------------------------------------

/// splitmix64: tiny, fast, good enough, and the same on every machine.
#[derive(Clone)]
pub struct Rng(pub u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        let mut r = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0xD1B5_4A32_D192_ED03);
        r.next();
        r
    }
    pub fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    /// 0..n (0 when n is 0).
    pub fn below(&mut self, n: usize) -> usize {
        if n == 0 {
            0
        } else {
            (self.next() % n as u64) as usize
        }
    }
    pub fn range(&mut self, a: usize, b: usize) -> usize {
        a + self.below(b.saturating_sub(a) + 1)
    }
    /// True one time in `n`.
    pub fn one_in(&mut self, n: usize) -> bool {
        self.below(n) == 0
    }
    pub fn pick<'a, T>(&mut self, s: &'a [T]) -> &'a T {
        &s[self.below(s.len())]
    }
}

// ---------------------------------------------------------------------------
// Strings
// ---------------------------------------------------------------------------

const POOL: &[&str] = &[
    "hello",
    "Hello, world",
    "a",
    " ",
    "two words",
    "\u{201c}curly\u{201d} and \u{2018}single\u{2019} quotes \u{2014} dash\u{2026}",
    "emoji \u{1F600}\u{1F44D}\u{1F3FD} done",
    "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}",
    "\u{05E9}\u{05DC}\u{05D5}\u{05DD} \u{05E2}\u{05D5}\u{05DC}\u{05DD}",
    "\u{0645}\u{0631}\u{062D}\u{0628}\u{0627} \u{0628}\u{0627}\u{0644}\u{0639}\u{0627}\u{0644}\u{0645} 123",
    "{{name}}",
    "{{",
    "}}{{",
    "{ DATE } { PAGE }",
    "{ MERGEFIELD x \\* upper }",
    "back\\slash \\\\ \\n \\t",
    "tab\there",
    "line\nbreak",
    "<b>not bold</b> & <i>nor</i> ]]> <?x?>",
    "100% \"quoted\" 'apos' $1 \\1 ^p ^t",
    "e\u{0301}\u{0301} combining",
    "\u{200B}zero\u{200F}width\u{202E}",
    "#anchor",
    "Heading one",
    "Supercalifragilisticexpialidocious_Pneumonoultramicroscopicsilicovolcanoconiosis_Floccinaucinihilipilification",
    "x",
    "",
];

pub fn text(rng: &mut Rng) -> String {
    match rng.below(40) {
        0 => "w".repeat(2000 + rng.below(2000)),
        1 => format!("{} ", "lorem ipsum ").repeat(40),
        2 => "\u{1F600}".repeat(1 + rng.below(30)),
        _ => rng.pick(POOL).to_string(),
    }
}

/// Short text that is a valid bookmark, style or tab name most of the time.
pub fn name(rng: &mut Rng) -> String {
    const N: &[&str] = &["Alpha", "Beta", "Target", "Intro", "Section_2", "x", "Name with space", "\u{00e9}t\u{00e9}", "top", "_hidden", "A\u{1F600}", "1abc", ""];
    rng.pick(N).to_string()
}

pub fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

// ---------------------------------------------------------------------------
// The live document
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct ParaRef {
    pub id: u32,
    pub len: usize,
    /// In a header, footer, note or text box (not in the main text).
    pub story: bool,
    pub in_table: bool,
}

#[derive(Default, Debug)]
pub struct Live {
    pub paras: Vec<ParaRef>,
    /// Top-level blocks of the main text, in order (any kind).
    pub blocks: Vec<u32>,
    /// A paragraph in each table (the first cell's first paragraph).
    pub tables: Vec<u32>,
    pub bookmarks: Vec<String>,
    pub tabs: Vec<String>,
    /// (paragraph, n) of every content control.
    pub controls: Vec<(u32, usize)>,
    /// Paragraphs that end a section.
    pub section_ends: Vec<u32>,
    /// Every id that exists, and every id seen before (for deleted ones).
    pub all_ids: Vec<u32>,
    pub styles: Vec<String>,
}

impl Live {
    pub fn of(doc: &Document, graveyard: &mut Vec<u32>) -> Live {
        let mut l = Live::default();
        let mut table_paras: Vec<u32> = Vec::new();
        for b in &doc.body {
            l.blocks.push(b.id());
            if let Block::Table(_) = b {
                let mut first = None;
                walk_paragraphs(std::slice::from_ref(b), &mut |p| {
                    first.get_or_insert(p.id);
                    table_paras.push(p.id);
                });
                if let Some(f) = first {
                    l.tables.push(f);
                }
            }
        }
        walk_paragraphs(&doc.body, &mut |p| {
            l.paras.push(ParaRef { id: p.id, len: p.len16(), story: false, in_table: table_paras.contains(&p.id) });
        });
        for s in &doc.stories {
            walk_paragraphs(&s.blocks, &mut |p| {
                l.paras.push(ParaRef { id: p.id, len: p.len16(), story: true, in_table: false });
            });
        }
        l.all_ids = l.paras.iter().map(|p| p.id).chain(l.blocks.iter().copied()).collect();
        let v = serde_json::to_value(doc.render_snapshot()).unwrap_or(Value::Null);
        scan_render(&v, &mut l);
        l.tabs = doc.tabs_info().tabs.iter().map(|t| t.id.clone()).collect();
        l.styles = doc.styles.ids().to_vec();
        for id in &l.all_ids {
            if !graveyard.contains(id) {
                graveyard.push(*id);
            }
        }
        l
    }
    pub fn body_paras(&self) -> Vec<&ParaRef> {
        self.paras.iter().filter(|p| !p.story).collect()
    }
}

fn scan_render(v: &Value, l: &mut Live) {
    match v {
        Value::Object(m) => {
            if m.get("t").and_then(Value::as_str) == Some("p") {
                let id = m.get("id").and_then(Value::as_u64).unwrap_or(0) as u32;
                if let Some(Value::Array(b)) = m.get("bookmarks") {
                    for n in b.iter().filter_map(Value::as_str) {
                        if !l.bookmarks.iter().any(|x| x == n) {
                            l.bookmarks.push(n.to_string());
                        }
                    }
                }
                if let Some(Value::Array(c)) = m.get("controls") {
                    for k in c.iter().filter_map(|c| c.get("n").and_then(Value::as_u64)) {
                        l.controls.push((id, k as usize));
                    }
                }
                if m.get("sectBreak").is_some() {
                    l.section_ends.push(id);
                }
            }
            m.values().for_each(|x| scan_render(x, l));
        }
        Value::Array(a) => a.iter().for_each(|x| scan_render(x, l)),
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// Generating ops
// ---------------------------------------------------------------------------

/// Every kind the hand-written generator knows. `replay` (a recorded shape with live ids) is added by the caller.
pub const KINDS: &[&str] = &[
    "insert_text",
    "insert_text",
    "insert_text",
    "delete",
    "delete",
    "split_paragraph",
    "split_paragraph",
    "format_text",
    "format_paragraphs",
    "set_style",
    "toggle_list",
    "set_list_style",
    "indent",
    "restart_numbering",
    "insert_paragraph",
    "delete_blocks",
    "move_blocks",
    "insert_table",
    "table_edit",
    "table_edit",
    "table_cells",
    "paste_html",
    "insert_fragment",
    "paste_markdown",
    "insert_markdown",
    "set_paragraph_text",
    "replace_text",
    "replace_font",
    "add_header_footer",
    "add_header_footer",
    "remove_header_footer",
    "link_header_footer",
    "set_title_page",
    "insert_footnote",
    "insert_endnote",
    "insert_bookmark",
    "delete_bookmark",
    "rename_bookmark",
    "set_link",
    "control_insert",
    "control_edit",
    "tab_add",
    "tab_delete",
    "tab_move",
    "tab_duplicate",
    "tab_rename",
    "update_fields",
    "set_page_setup",
    "set_columns",
    "set_section",
    "insert_break",
    "insert_cover_page",
    "remove_cover_page",
    "protection",
    "insert_toc",
    "update_toc",
    "set_watermark",
    "insert_field",
    "change_case",
    "sort_paragraphs",
    "convert_to_table",
    "insert_page_part",
    "set_page_color",
    "set_line_numbers",
    "set_language",
    "insert_horizontal_line",
    "set_doc_props",
    "insert_text_box",
    "insert_caption",
    "set_style_definition",
];

fn pos(p: &ParaRef, rng: &mut Rng) -> Value {
    json!({"p": p.id, "o": rng.below(p.len + 1)})
}

/// A paragraph: mostly main text, sometimes a story or a table cell.
fn any_para<'a>(l: &'a Live, rng: &mut Rng) -> Option<&'a ParaRef> {
    if l.paras.is_empty() {
        return None;
    }
    if rng.one_in(5) {
        let s: Vec<&ParaRef> = l.paras.iter().filter(|p| p.story).collect();
        if !s.is_empty() {
            return Some(s[rng.below(s.len())]);
        }
    }
    let b = l.body_paras();
    if b.is_empty() {
        None
    } else {
        Some(b[rng.below(b.len())])
    }
}

fn body_para<'a>(l: &'a Live, rng: &mut Rng) -> Option<&'a ParaRef> {
    let b = l.body_paras();
    if b.is_empty() {
        None
    } else {
        Some(b[rng.below(b.len())])
    }
}

fn range(l: &Live, rng: &mut Rng) -> Option<(Value, Value)> {
    let a = any_para(l, rng)?;
    let x = rng.below(a.len + 1);
    // One time in four across paragraphs (body only).
    if !a.story && rng.one_in(4) {
        let b = l.body_paras();
        let i = b.iter().position(|p| p.id == a.id)?;
        let j = (i + 1 + rng.below(3)).min(b.len() - 1);
        let z = b[j];
        let y = rng.below(z.len + 1);
        return Some((json!({"p": a.id, "o": x}), json!({"p": z.id, "o": y})));
    }
    let y = rng.range(x, a.len);
    Some((json!({"p": a.id, "o": x}), json!({"p": a.id, "o": y})))
}

fn some_ids(l: &Live, rng: &mut Rng, top_level: bool) -> Vec<u32> {
    let pool: Vec<u32> = if top_level { l.blocks.clone() } else { l.body_paras().iter().map(|p| p.id).collect() };
    if pool.is_empty() {
        return vec![];
    }
    let n = 1 + rng.below(3);
    let i = rng.below(pool.len());
    (0..n).filter_map(|k| pool.get(i + k).copied()).collect()
}

fn run_patch(rng: &mut Rng) -> Value {
    let mut m = Map::new();
    let n = 1 + rng.below(3);
    for _ in 0..n {
        match rng.below(10) {
            0 => m.insert("bold".into(), json!(rng.one_in(2))),
            1 => m.insert("italic".into(), json!(rng.one_in(2))),
            2 => m.insert("underline".into(), json!(*rng.pick(&["single", "double", "none", "wave"]))),
            3 => m.insert("strike".into(), json!(rng.one_in(2))),
            4 => m.insert("font".into(), json!(*rng.pick(&["Arial", "Lora", "Times New Roman", "", "Papyrus"]))),
            5 => m.insert("size".into(), json!(*rng.pick(&[8.0, 11.0, 12.0, 24.0, 72.0]))),
            6 => m.insert("color".into(), json!(*rng.pick(&["FF0000", "auto", "00ff00", "#336699"]))),
            7 => m.insert("highlight".into(), json!(*rng.pick(&["yellow", "none", "cyan"]))),
            8 => m.insert("vertAlign".into(), json!(*rng.pick(&["superscript", "subscript", "baseline"]))),
            _ => m.insert("clear".into(), json!(true)),
        };
    }
    Value::Object(m)
}

fn para_patch(rng: &mut Rng) -> Value {
    let mut m = Map::new();
    let n = 1 + rng.below(3);
    for _ in 0..n {
        match rng.below(8) {
            0 => m.insert("align".into(), json!(*rng.pick(&["left", "center", "right", "both"]))),
            1 => m.insert("spaceBefore".into(), json!(rng.below(600) as i64)),
            2 => m.insert("spaceAfter".into(), json!(rng.below(600) as i64)),
            3 => m.insert("lineMultiple".into(), json!(*rng.pick(&[1.0, 1.15, 1.5, 2.0]))),
            4 => m.insert("indLeft".into(), json!(rng.below(2000) as i64)),
            5 => m.insert("indFirst".into(), json!(rng.below(1440) as i64 - 720)),
            6 => m.insert("keepNext".into(), json!(rng.one_in(2))),
            _ => m.insert("pageBreakBefore".into(), json!(rng.one_in(2))),
        };
    }
    Value::Object(m)
}

/// A random op (or a short batch) of a random kind, as JSON, from what the document has now. None when the kind
/// needs something the document lacks (then the caller tries another).
pub fn gen_kind(kind: &str, l: &Live, rng: &mut Rng) -> Option<Vec<Value>> {
    if l.blocks.is_empty() || l.paras.is_empty() {
        return None;
    }
    let op = match kind {
        "insert_text" => {
            let p = any_para(l, rng)?;
            let mut o = json!({"op":"insert_text","at":pos(p,rng),"text":text(rng)});
            if rng.one_in(4) {
                o["format"] = run_patch(rng);
            }
            o
        }
        "delete" => {
            let (a, b) = range(l, rng)?;
            json!({"op":"delete","from":a,"to":b})
        }
        "split_paragraph" => json!({"op":"split_paragraph","at":pos(any_para(l,rng)?,rng)}),
        "format_text" => {
            let (a, b) = range(l, rng)?;
            json!({"op":"format_text","from":a,"to":b,"format":run_patch(rng)})
        }
        "format_paragraphs" => json!({"op":"format_paragraphs","ids":some_ids(l,rng,false),"format":para_patch(rng)}),
        "set_style" => {
            let s = if rng.one_in(3) && !l.styles.is_empty() { rng.pick(&l.styles).clone() } else { rng.pick(&["Heading1", "Heading2", "Title", "Normal", "ListParagraph", "Quote", "Caption", "NoSuchStyle", "Heading 3"]).to_string() };
            json!({"op":"set_style","ids":some_ids(l,rng,false),"style":s})
        }
        "toggle_list" => json!({"op":"toggle_list","ids":some_ids(l,rng,false),"kind":*rng.pick(&["bullet","decimal"])}),
        "set_list_style" => json!({"op":"set_list_style","ids":some_ids(l,rng,false),"style":*rng.pick(&["disc","diamond","decimal","paren","outline","upper","roman","zero","checklist","none"])}),
        "indent" => json!({"op":"indent","ids":some_ids(l,rng,false),"delta":*rng.pick(&[1,-1,2,-3])}),
        "restart_numbering" => json!({"op": *rng.pick(&["restart_numbering","continue_numbering"]),"id":body_para(l,rng)?.id}),
        "insert_paragraph" => {
            let id = *rng.pick(&l.blocks);
            let mut o = json!({"op":"insert_paragraph","text":text(rng)});
            o[if rng.one_in(2) { "after" } else { "before" }] = json!(id);
            if rng.one_in(3) {
                o["style"] = json!(*rng.pick(&["Heading1", "Quote", "Title"]));
            }
            o
        }
        "delete_blocks" => json!({"op":"delete_blocks","ids":some_ids(l,rng,true)}),
        "move_blocks" => {
            let mut o = json!({"op":"move_blocks","ids":some_ids(l,rng,true)});
            o[if rng.one_in(2) { "after" } else { "before" }] = json!(*rng.pick(&l.blocks));
            o
        }
        "insert_table" => {
            let (r, c) = (1 + rng.below(4), 1 + rng.below(4));
            let mut o = json!({"op":"insert_table","after":*rng.pick(&l.blocks),"rows":r,"cols":c});
            if rng.one_in(2) {
                o["data"] = json!((0..r).map(|_| (0..c).map(|_| text(rng)).collect::<Vec<_>>()).collect::<Vec<_>>());
            }
            o
        }
        "table_edit" => json!({"op":"table_edit","at":*l.tables.get(rng.below(l.tables.len().max(1)))?,"action":*rng.pick(&["row_above","row_below","col_left","col_right","delete_row","delete_col","delete_table"])}),
        "table_cells" => {
            let t = *l.tables.get(rng.below(l.tables.len().max(1)))?;
            let b = l.paras.iter().filter(|p| p.in_table).map(|p| p.id).nth(rng.below(8)).unwrap_or(t);
            let a = match rng.below(3) {
                0 => json!({"type":"merge"}),
                1 => json!({"type":"shade","color":"FFFF00"}),
                _ => json!({"type":"format","textDirection":"sideways"}),
            };
            json!({"op":"table_cells","from":t,"to":b,"action":a})
        }
        "paste_html" => {
            const H: &[&str] = &[
                "<p>One <b>bold</b> <i>it</i> <a href=\"https://example.com\">link</a></p>",
                "<ul><li>a</li><li>b<ul><li>c</li></ul></li></ul>",
                "<table><tr><td>1</td><td>2</td></tr><tr><td>3</td><td>4</td></tr></table>",
                "<h1>Title</h1><p>text</p><ol><li>x</li></ol>",
                "<p><a href=\"#top\">to top</a> <a name=\"top\"></a></p>",
                "<span style=\"color:red;font-size:20px\">styled</span>",
                "plain text no tags",
                "<p>\u{05E9}\u{05DC}\u{05D5}\u{05DD}</p><p>{{x}} \u{1F600}</p>",
                "<img src=\"data:image/gif;base64,R0lGODlhAQABAAAAACw=\"><br><hr>",
                "<div><p>nested <div>divs</div></p></div><blockquote>q</blockquote><pre>code\n  x</pre>",
            ];
            let mut o = json!({"op":"paste_html","at":pos(any_para(l,rng)?,rng),"html":*rng.pick(H)});
            if rng.one_in(3) {
                o["mode"] = json!(*rng.pick(&["keep", "merge"]));
            }
            o
        }
        "insert_fragment" => {
            // Tabs and line breaks in a fragment are elements, as the clipboard writes them (a raw character in w:t is not).
            let t = xml_escape(&text(rng)).replace('\t', "</w:t><w:tab/><w:t xml:space=\"preserve\">").replace('\n', "</w:t><w:br/><w:t xml:space=\"preserve\">");
            let body = match rng.below(3) {
                0 => format!("<w:p><w:r><w:t xml:space=\"preserve\">{t}</w:t></w:r></w:p>"),
                1 => format!("<w:p><w:pPr><w:pStyle w:val=\"Heading1\"/></w:pPr><w:r><w:rPr><w:b/></w:rPr><w:t>{t}</w:t></w:r></w:p><w:p><w:r><w:t>second</w:t></w:r></w:p>"),
                _ => format!("<w:tbl><w:tblPr/><w:tblGrid><w:gridCol w:w=\"3000\"/></w:tblGrid><w:tr><w:tc><w:p><w:r><w:t>{t}</w:t></w:r></w:p></w:tc></w:tr></w:tbl><w:p/>"),
            };
            let frag = json!({"version":1,"ns":[["w","http://schemas.openxmlformats.org/wordprocessingml/2006/main"]],"body":body,"block":rng.one_in(2)});
            json!({"op":"insert_fragment","at":pos(any_para(l,rng)?,rng),"fragment":frag})
        }
        "paste_markdown" => json!({"op":"paste_markdown","at":pos(any_para(l,rng)?,rng),"markdown":*rng.pick(&["**bold** and _it_","# Head\n\n- a\n- b","| a | b |\n|---|---|\n| 1 | 2 |","`code` [link](https://x.org) {{x}}","plain"])}),
        "insert_markdown" => {
            let mut o = json!({"op":"insert_markdown","markdown":*rng.pick(&["# H1\n\ntext\n\n1. one\n2. two","> quote\n\n```\ncode\n```","| a | b |\n|---|---|\n| 1 | 2 |\n\n![x](y)"])});
            o[if rng.one_in(2) { "after" } else { "before" }] = json!(*rng.pick(&l.blocks));
            o
        }
        "set_paragraph_text" => json!({"op":"set_paragraph_text","id":body_para(l,rng)?.id,"markdown":*rng.pick(&["**x** y","plain","","# no","[a](#b) \u{1F600}"])}),
        "replace_text" => {
            let find = if rng.one_in(2) { rng.pick(&["hello", "o", "the", "a", "{{", "\u{1F600}", "^p", "e"]).to_string() } else { text(rng) };
            let mut o = json!({"op":"replace_text","find":find,"replace":text(rng),"all":rng.one_in(2)});
            if rng.one_in(4) {
                o["main_only"] = json!(true);
            }
            if rng.one_in(5) {
                o["regex"] = json!(true);
            }
            if rng.one_in(5) {
                o["format"] = run_patch(rng);
            }
            o
        }
        "replace_font" => json!({"op":"replace_font","from":*rng.pick(&["Arial","Calibri","Times New Roman","Lora","Papyrus"]),"to":*rng.pick(&["Lora","Arial","Georgia"])}),
        "add_header_footer" => {
            let mut o = json!({"op":"add_header_footer","kind":*rng.pick(&["header","footer"]),"variant":*rng.pick(&["default","first","even"])});
            if rng.one_in(2) && !l.section_ends.is_empty() {
                o["section"] = json!(*rng.pick(&l.section_ends));
            }
            o
        }
        "remove_header_footer" => {
            let mut o = json!({"op":"remove_header_footer","kind":*rng.pick(&["header","footer"])});
            if rng.one_in(2) {
                o["variant"] = json!(*rng.pick(&["default", "first", "even"]));
            }
            if rng.one_in(2) && !l.section_ends.is_empty() {
                o["section"] = json!(*rng.pick(&l.section_ends));
            }
            o
        }
        "link_header_footer" => json!({"op":"link_header_footer","kind":*rng.pick(&["header","footer"]),"variant":*rng.pick(&["default","first"]),"section":*l.section_ends.get(rng.below(l.section_ends.len().max(1)))?,"linked":rng.one_in(2)}),
        "set_title_page" => {
            let mut o = json!({"op":"set_title_page","on":rng.one_in(2)});
            if rng.one_in(2) && !l.section_ends.is_empty() {
                o["section"] = json!(*rng.pick(&l.section_ends));
            }
            o
        }
        "insert_footnote" | "insert_endnote" => json!({"op":kind,"at":pos(body_para(l,rng)?,rng),"text":text(rng)}),
        "insert_bookmark" => {
            let (a, b) = range(l, rng)?;
            let to = if rng.one_in(2) { b } else { a.clone() };
            json!({"op":"insert_bookmark","from":a,"to":to,"name":name(rng)})
        }
        "delete_bookmark" => json!({"op":"delete_bookmark","name":if l.bookmarks.is_empty() || rng.one_in(5) { name(rng) } else { rng.pick(&l.bookmarks).clone() }}),
        "rename_bookmark" => json!({"op":"rename_bookmark","from":if l.bookmarks.is_empty() { name(rng) } else { rng.pick(&l.bookmarks).clone() },"to":name(rng)}),
        "set_link" => {
            let (a, b) = range(l, rng)?;
            let url = if rng.one_in(3) { json!(null) } else if !l.bookmarks.is_empty() && rng.one_in(2) { json!(format!("#{}", rng.pick(&l.bookmarks))) } else { json!(*rng.pick(&["https://example.com/a?b=1&c=2", "mailto:a@b.c", "#nowhere", "javascript:alert(1)", "https://\u{00e9}.org/\u{1F600}"])) };
            json!({"op":"set_link","from":a,"to":b,"url":url})
        }
        "control_insert" => {
            json!({"op":"control","action":"insert","at":pos(body_para(l,rng)?,rng),"kind":*rng.pick(&["text", "richText", "checkbox", "date", "dropDown", "comboBox"])})
        }
        "control_edit" => {
            let (p, n) = *l.controls.get(rng.below(l.controls.len().max(1)))?;
            let target = json!({"p":p,"n":n});
            match rng.below(5) {
                0 => json!({"op":"control","action":"remove","target":target}),
                1 => json!({"op":"control","action":"set","target":target,"text":text(rng)}),
                2 => json!({"op":"control","action":"set","target":target,"checked":rng.one_in(2)}),
                3 => json!({"op":"control","action":"props","target":target,"props":{"alias":text(rng),"placeholder":text(rng),"lock":*rng.pick(&["unlocked","contentLocked","sdtLocked"])}}),
                _ => json!({"op":"control","action":"set","target":target,"index":rng.below(4)}),
            }
        }
        "tab_add" => {
            let mut o = json!({"op":"tab","action":"add","name":name(rng)});
            if rng.one_in(2) {
                o["first_name"] = json!(name(rng));
            }
            if rng.one_in(2) {
                o["open"] = json!(true);
            }
            o
        }
        "tab_delete" | "tab_duplicate" | "tab_rename" | "tab_move" => {
            if l.tabs.is_empty() {
                return None;
            }
            let id = rng.pick(&l.tabs).clone();
            match kind {
                "tab_delete" => json!({"op":"tab","action":"delete","id":id}),
                "tab_duplicate" => json!({"op":"tab","action":"duplicate","id":id}),
                "tab_rename" => json!({"op":"tab","action":"rename","id":id,"name":name(rng)}),
                _ => {
                    let mut o = json!({"op":"tab","action":"move","id":id});
                    if rng.one_in(2) {
                        o["parent"] = json!(rng.pick(&l.tabs).clone());
                    }
                    if rng.one_in(2) {
                        o["index"] = json!(rng.below(4));
                    }
                    o
                }
            }
        }
        "update_fields" => json!({"op":"update_fields"}),
        "set_page_setup" => {
            let mut o = json!({"op":"set_page_setup"});
            if rng.one_in(2) {
                o["width"] = json!(*rng.pick(&[12240, 15840, 8000, 100, 31000]));
                o["height"] = json!(*rng.pick(&[15840, 12240, 20000, 0]));
            }
            if rng.one_in(2) {
                o["margins"] = json!([rng.below(3000), rng.below(3000), rng.below(3000), rng.below(3000)]);
            }
            if rng.one_in(3) && !l.section_ends.is_empty() {
                o["section"] = json!(*rng.pick(&l.section_ends));
            }
            o
        }
        "set_columns" => {
            let mut o = json!({"op":"set_columns","count":rng.range(0, 7)});
            if rng.one_in(2) {
                o["space"] = json!(rng.below(1000));
            }
            if rng.one_in(3) {
                o["sep"] = json!(true);
            }
            if rng.one_in(2) && !l.section_ends.is_empty() {
                o["section"] = json!(*rng.pick(&l.section_ends));
            }
            o
        }
        "set_section" => {
            let mut o = json!({"op":"set_section","v_align":*rng.pick(&["top","center","both","bottom"]),"gutter":rng.below(1000),"header":rng.below(1500),"footer":rng.below(1500)});
            if rng.one_in(2) && !l.section_ends.is_empty() {
                o["section"] = json!(*rng.pick(&l.section_ends));
            }
            o
        }
        "insert_break" => json!({"op":"insert_break","at":pos(body_para(l,rng)?,rng),"kind":*rng.pick(&["page","column","next_page","continuous","even_page","odd_page"])}),
        "insert_cover_page" => json!({"op":"insert_cover_page","design":*rng.pick(&["docxtor.simple","docxtor.centered","docxtor.nope"])}),
        "remove_cover_page" => json!({"op":"remove_cover_page"}),
        "protection" => match rng.below(5) {
            0 => json!({"op":"protection","action":"set","settings":{"edit":*rng.pick(&["readOnly","forms","comments","trackedChanges"]),"enforce":true}}),
            1 | 2 => json!({"op":"protection","action":"stop"}),
            3 => json!({"op":"protection","action":"mark_final","on":rng.one_in(2)}),
            _ => {
                let (a, b) = range(l, rng)?;
                json!({"op":"protection","action":"editable_range","from":a,"to":b})
            }
        },
        "insert_toc" => json!({"op":"insert_toc","before":*rng.pick(&l.blocks),"levels":1+rng.below(4),"title":"Contents"}),
        "update_toc" => json!({"op":"update_toc","pages_only":rng.one_in(2)}),
        "set_watermark" => json!({"op":"set_watermark","text":if rng.one_in(3) { json!(null) } else { json!(text(rng)) }}),
        "insert_field" => json!({"op":"insert_field","at":pos(any_para(l,rng)?,rng),"field":*rng.pick(&["PAGE","NUMPAGES","DATE \\@ \"yyyy-MM-dd\"","TITLE","EQ \\f(1,2)","SEQ Figure \\* ARABIC","REF Target \\h","{ x }","STYLEREF \"Heading 1\"","= 1+2"])}),
        "change_case" => {
            let (a, b) = range(l, rng)?;
            json!({"op":"change_case","from":a,"to":b,"mode":*rng.pick(&["upper","lower","title","sentence","toggle"])})
        }
        "sort_paragraphs" => json!({"op":"sort_paragraphs","ids":some_ids(l,rng,false),"keys":[],"case_sensitive":false,"header":rng.one_in(3),"separator":"tabs"}),
        "convert_to_table" => json!({"op":"convert_to_table","ids":some_ids(l,rng,false),"separator":*rng.pick(&["tabs","commas","paragraphs"])}),
        "insert_page_part" => json!({"op":"insert_page_part","kind":*rng.pick(&["header","footer"]),"design":*rng.pick(&["docxtor.blank","docxtor.three","docxtor.nope"])}),
        "set_page_color" => json!({"op":"set_page_color","color":if rng.one_in(3) { json!(null) } else { json!(*rng.pick(&["FFFF00", "#00ff00", "zzz"])) }}),
        "set_line_numbers" => json!({"op":"set_line_numbers","on":rng.one_in(2),"count_by":1+rng.below(5),"restart":*rng.pick(&["newPage","newSection","continuous"])}),
        "set_language" => json!({"op":"set_language","lang":*rng.pick(&["es-ES","ar-SA","he-IL","en-US","xx"]),"ids":some_ids(l,rng,false)}),
        "insert_horizontal_line" => json!({"op":"insert_horizontal_line","at":pos(body_para(l,rng)?,rng)}),
        "set_doc_props" => json!({"op":"set_doc_props","title":text(rng),"author":text(rng),"keywords":text(rng)}),
        "insert_text_box" => json!({"op":"insert_text_box","at":pos(body_para(l,rng)?,rng),"w":100.0,"h":40.0,"text":text(rng),"floating":rng.one_in(2),"x":10.0,"y":10.0}),
        "insert_caption" => json!({"op":"insert_caption","after":*rng.pick(&l.blocks),"label":*rng.pick(&["Figure","Table"]),"text":text(rng)}),
        "set_style_definition" => {
            if rng.one_in(2) {
                // A new style (only custom styles can be renamed; built-in ones keep their names).
                json!({"op":"set_style_definition","def":{"name":format!("Custom{}", rng.below(4)),"kind":*rng.pick(&["paragraph","character","linked"]),"basedOn":*rng.pick(&["Normal","Heading1",""]),"run":{"italic":true},"para":{"spaceBefore":rng.below(300)}}})
            } else {
                json!({"op":"set_style_definition","style":*rng.pick(&["Heading1","Normal","Quote","Title","Custom0","Custom1"]),"def":{"next":*rng.pick(&["Normal","Heading2"]),"quick":rng.one_in(2),"run":{"bold":rng.one_in(2),"size":rng.range(8,30) as f64},"para":{"spaceAfter":rng.below(400)}}})
            }
        }
        _ => return None,
    };
    Some(vec![op])
}

// ---------------------------------------------------------------------------
// Recorded shapes
// ---------------------------------------------------------------------------

/// Keys whose value is a block id (a number), and `p` inside a position.
const ID_KEYS: &[&str] = &["id", "after", "before", "at", "section", "from", "to", "end", "p", "block", "ctl", "target", "paragraph", "para"];

/// An op recorded from the hand-written tests, with every block id and position swapped for one from the live document.
pub fn swap_live(v: &Value, l: &Live, rng: &mut Rng, op_name: &str) -> Value {
    match v {
        Value::Object(m) => {
            let mut out = Map::new();
            // A position: {"p": id, "o": offset}.
            if m.contains_key("p") && m.contains_key("o") && m.len() == 2 {
                if let Some(p) = any_para(l, rng) {
                    return pos(p, rng);
                }
            }
            for (k, x) in m {
                let nv = match x {
                    Value::Number(n) if n.is_u64() && ID_KEYS.contains(&k.as_str()) && !l.all_ids.is_empty() => json!(*rng.pick(&l.all_ids)),
                    Value::Array(a) if k == "ids" && a.iter().all(|e| e.is_u64()) && !l.all_ids.is_empty() => json!(a.iter().map(|_| *rng.pick(&l.all_ids)).collect::<Vec<_>>()),
                    Value::String(s) if op_name == "tab" && (k == "id" || k == "parent") && !l.tabs.is_empty() => {
                        let _ = s;
                        json!(rng.pick(&l.tabs).clone())
                    }
                    Value::String(s) if (k == "name" || k == "from" || k == "to") && matches!(op_name, "delete_bookmark" | "rename_bookmark") && !l.bookmarks.is_empty() && rng.one_in(2) => {
                        let _ = s;
                        json!(rng.pick(&l.bookmarks).clone())
                    }
                    _ => swap_live(x, l, rng, op_name),
                };
                out.insert(k.clone(), nv);
            }
            Value::Object(out)
        }
        Value::Array(a) => Value::Array(a.iter().map(|x| swap_live(x, l, rng, op_name)).collect()),
        other => other.clone(),
    }
}

pub fn load_shapes() -> Vec<Value> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/op_shapes.jsonl");
    std::fs::read_to_string(path).unwrap_or_default().lines().filter_map(|l| serde_json::from_str(l).ok()).collect()
}

// ---------------------------------------------------------------------------
// Starting documents
// ---------------------------------------------------------------------------

pub fn repo_path(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../").join(rel)
}

/// Where a run starts: a file of the repository's `fixtures/` or a document built in code.
#[derive(Clone, Debug, PartialEq)]
pub enum Start {
    File(String),
    /// Built from ops in code: `rich` has headers, footers, notes, tables, lists, controls, tabs, a cover page,
    /// bookmarks and links; `plain` is a short blank document.
    Built(&'static str),
    /// A document from outside the repository (the corpus): a label for messages and the file's bytes.
    Bytes(String, std::sync::Arc<Vec<u8>>),
}

impl Start {
    pub fn label(&self) -> String {
        match self {
            Start::File(f) => f.clone(),
            Start::Built(b) => format!("built:{b}"),
            Start::Bytes(l, _) => l.clone(),
        }
    }
    pub fn open(&self) -> Document {
        match self {
            // A name in fixtures/, or a whole path (a corpus file whose steps were written out to replay).
            Start::File(f) => {
                let path = if std::path::Path::new(f).is_absolute() { PathBuf::from(f) } else { repo_path(&format!("fixtures/{f}")) };
                Document::open(&std::fs::read(path).unwrap()).unwrap_or_else(|e| panic!("{f}: {e}"))
            }
            Start::Built(k) => build(k),
            Start::Bytes(l, b) => docxtor_core::open::open(b, "corpus.docx").map(|o| o.doc).unwrap_or_else(|e| panic!("{l}: {e}")),
        }
    }
}

pub fn fixture_files() -> Vec<String> {
    ["simple.docx", "rich.docx", "complex.docx", "long.docx"].iter().map(|s| s.to_string()).collect()
}

fn run(doc: &mut Document, v: Value) {
    if let Err(e) = doc.apply_json(&v.to_string(), false) {
        panic!("building the start document: {e}: {v}");
    }
}

fn paras(doc: &Document) -> Vec<u32> {
    let mut v = Vec::new();
    walk_paragraphs(&doc.body, &mut |p| v.push(p.id));
    v
}

/// A document in code with something for every kind of op to act on.
pub fn build(kind: &str) -> Document {
    let mut doc = Document::new_blank();
    let first = doc.render_all().order[0];
    run(&mut doc, json!({"op":"insert_text","at":{"p":first,"o":0},"text":"Introduction to the document"}));
    if kind == "plain" {
        run(&mut doc, json!({"op":"insert_paragraph","after":first,"text":"Second paragraph with some words in it."}));
        return doc;
    }
    run(&mut doc, json!({"op":"set_style","ids":[first],"style":"Heading1"}));
    let mut last = first;
    for t in ["First body paragraph with {{braces}}, \u{201c}curly quotes\u{201d} and an emoji \u{1F600}.", "List item one", "List item two", "Another paragraph about tables and notes.", "Closing paragraph \u{05E9}\u{05DC}\u{05D5}\u{05DD}."] {
        run(&mut doc, json!({"op":"insert_paragraph","after":last,"text":t}));
        last = *paras(&doc).last().unwrap();
        // insert_paragraph after the last block gives the new paragraph's id as the last in order only when appended
    }
    let ps = paras(&doc);
    run(&mut doc, json!({"op":"toggle_list","ids":[ps[2],ps[3]],"kind":"bullet"}));
    run(&mut doc, json!({"op":"insert_table","after":ps[4],"rows":2,"cols":3,"data":[["a","b","c"],["d","e","f"]]}));
    let ps = paras(&doc);
    run(&mut doc, json!({"op":"insert_bookmark","from":{"p":ps[1],"o":0},"to":{"p":ps[1],"o":5},"name":"Target"}));
    run(&mut doc, json!({"op":"set_link","from":{"p":ps[5],"o":0},"to":{"p":ps[5],"o":7},"url":"#Target"}));
    run(&mut doc, json!({"op":"set_link","from":{"p":ps[5],"o":8},"to":{"p":ps[5],"o":12},"url":"https://example.com"}));
    run(&mut doc, json!({"op":"insert_footnote","at":{"p":ps[1],"o":10},"text":"A footnote."}));
    run(&mut doc, json!({"op":"insert_endnote","at":{"p":ps[1],"o":20},"text":"An endnote."}));
    run(&mut doc, json!({"op":"add_header_footer","kind":"header","variant":"default"}));
    run(&mut doc, json!({"op":"add_header_footer","kind":"footer","variant":"default"}));
    run(&mut doc, json!({"op":"control","action":"insert","kind":"text","at":{"p":ps[1],"o":3}}));
    run(&mut doc, json!({"op":"insert_field","at":{"p":ps[5],"o":0},"field":"PAGE"}));
    run(&mut doc, json!({"op":"insert_cover_page","design":"docxtor.simple"}));
    let ps = paras(&doc);
    run(&mut doc, json!({"op":"insert_break","at":{"p":*ps.last().unwrap(),"o":3},"kind":"next_page"}));
    run(&mut doc, json!({"op":"tab","action":"add","name":"Second tab","first_name":"Overview"}));
    doc
}

// ---------------------------------------------------------------------------
// Render model comparison
// ---------------------------------------------------------------------------

/// The render model of a document as JSON: the body, the page setup (sections, headers, footers, notes) and the tabs.
/// E6: where the render model of a document and of its save, opened again, differ (ids and part names renumbered), or
/// None. Each model is kept as text once normalised, so only one is a JSON tree at a time.
pub fn reopened_diff(a: &Document, b: &Document) -> Option<String> {
    let text = |d: &Document| serde_json::to_string(&normalise(&render_model(d))).unwrap_or_default();
    let (x, y) = (text(a), text(b));
    if x == y {
        return None;
    }
    let (x, y): (Value, Value) = (serde_json::from_str(&x).ok()?, serde_json::from_str(&y).ok()?);
    first_diff(&x, &y)
}

pub fn render_model(doc: &Document) -> Value {
    json!({
        "body": serde_json::to_value(doc.render_snapshot()).unwrap_or(Value::Null),
        "page": serde_json::to_value(doc.page_setup()).unwrap_or(Value::Null),
        "tabs": serde_json::to_value(doc.tabs_info()).unwrap_or(Value::Null),
    })
}

/// The same model with ids replaced by the order they first appear in, and part names by the order the sections
/// first use them: opening a file renumbers both (E6 compares what the person sees, not how it is numbered).
/// Offsets (`start` and `end` of a control) are not ids; a section's `end` and a tab's `start` and `ids` are.
pub fn normalise(v: &Value) -> Value {
    // Part names in the order the sections use them, then any others.
    let mut parts: Vec<String> = Vec::new();
    if let Some(secs) = v["page"]["sections"].as_array() {
        for sec in secs {
            for k in ["headers", "footers"] {
                if let Some(Value::Object(m)) = sec.get(k) {
                    for x in m.values().filter_map(Value::as_str) {
                        if !parts.iter().any(|p| p == x) {
                            parts.push(x.to_string());
                        }
                    }
                }
            }
        }
    }
    if let Some(Value::Object(m)) = v["page"].get("parts") {
        for k in m.keys() {
            if !parts.iter().any(|p| p == k) {
                parts.push(k.clone());
            }
        }
    }
    struct Ctx {
        ids: HashMap<u64, usize>,
        parts: Vec<String>,
    }
    impl Ctx {
        fn rename(&self, s: &str) -> String {
            match self.parts.iter().position(|p| p == s) {
                Some(i) => format!("part#{i}"),
                None => s.to_string(),
            }
        }
    }
    fn go(v: &Value, c: &mut Ctx, key: &str, path: &mut Vec<String>) -> Value {
        match v {
            Value::Object(m) => {
                path.push(key.to_string());
                let out = Value::Object(m.iter().map(|(k, x)| (c.rename(k), go(x, c, k, path))).collect());
                path.pop();
                out
            }
            Value::Array(a) => Value::Array(a.iter().map(|x| go(x, c, key, path)).collect()),
            Value::Number(n) if n.is_u64() => {
                let in_ = |k: &str| path.iter().any(|p| p == k);
                // A drawing's own id (`wp:docPr`, kept from the file) is a number the file chose, not a paragraph id:
                // it must not take part in the numbering, or it collides with a paragraph id that opening hands out
                // (L5: shape id 5001 and the 5,001st paragraph after reopening). Text inside a shape is still blocks.
                let drawing_id = key == "id" && (path.last().map(String::as_str) == Some("shape") || (path.last().map(String::as_str) == Some("children") && in_("shape")));
                // So is a content control's `w:id` (its `ctl` is the block's id).
                let control_id = key == "id" && matches!(path.last().map(String::as_str), Some("control" | "controls"));
                let is_id = match key {
                    _ if drawing_id || control_id => false,
                    // A block that came from an embedded chunk names the chunk's id.
                    "id" | "ctl" | "order" | "p" | "chunk" => !in_("tabs") || key != "id",
                    "end" => in_("sections") && !in_("controls"),
                    "start" => in_("tabs") && !in_("controls"),
                    // The blocks each tab shows.
                    "ids" => in_("tabs"),
                    _ => false,
                };
                if is_id {
                    let next = c.ids.len();
                    json!(format!("#{}", c.ids.entry(n.as_u64().unwrap()).or_insert(next)))
                } else {
                    v.clone()
                }
            }
            Value::String(s) => Value::String(c.rename(s)),
            other => other.clone(),
        }
    }
    go(v, &mut Ctx { ids: HashMap::new(), parts }, "", &mut Vec::new())
}

/// The first difference between two JSON values, as a path and the two sides (short), or None when equal.
pub fn first_diff(a: &Value, b: &Value) -> Option<String> {
    fn go(a: &Value, b: &Value, path: &mut Vec<String>) -> Option<String> {
        let show = |v: &Value| {
            let s = v.to_string();
            if s.chars().count() > 160 {
                format!("{}...", s.chars().take(160).collect::<String>())
            } else {
                s
            }
        };
        match (a, b) {
            (Value::Object(x), Value::Object(y)) => {
                for (k, xv) in x {
                    path.push(k.clone());
                    match y.get(k) {
                        Some(yv) => {
                            if let Some(d) = go(xv, yv, path) {
                                return Some(d);
                            }
                        }
                        None => return Some(format!("{} :: {} vs (missing)", path.join("."), show(xv))),
                    }
                    path.pop();
                }
                if let Some(k) = y.keys().find(|k| !x.contains_key(*k)) {
                    path.push(k.clone());
                    return Some(format!("{} :: (missing) vs {}", path.join("."), show(&y[k])));
                }
                None
            }
            (Value::Array(x), Value::Array(y)) => {
                for (i, (xv, yv)) in x.iter().zip(y).enumerate() {
                    path.push(i.to_string());
                    if let Some(d) = go(xv, yv, path) {
                        return Some(d);
                    }
                    path.pop();
                }
                if x.len() != y.len() {
                    Some(format!("{} :: length {} vs {}", path.join("."), x.len(), y.len()))
                } else {
                    None
                }
            }
            _ if a == b => None,
            _ => Some(format!("{} :: {} vs {}", path.join("."), show(a), show(b))),
        }
    }
    go(a, b, &mut Vec::new())
}

/// Error texts that show the insides of the program: Rust's own words, a file in the source, a debug dump.
pub fn internal_text(s: &str) -> Option<&'static str> {
    const MARKERS: &[&str] = &[
        "called `Result", "called `Option", "unwrap()", "panicked", "RUST_BACKTRACE", "index out of bounds", "ParseIntError", "ParseFloatError", "invalid digit found", "Utf8Error", "FromUtf8Error", "invalid utf-8", "ErrorKind", "Custom {", "Os {", "kind:", "src/", ".rs:", "capacity overflow", "attempt to ", "TryFromIntError", "out of range integral type", "{:?}", "Some(", "Err(", "Error(", "Syntax(",
    ];
    MARKERS.iter().find(|m| s.contains(**m)).copied()
}
