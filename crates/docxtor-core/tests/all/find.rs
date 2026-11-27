//! Find and replace: special characters, wildcards, regular expressions, whole words, keeping the case,
//! paragraph marks, headers, footers, notes, text boxes, formatting to find and formatting to put on, the AI tools.

use docxtor_core::ai::call_tool;
use docxtor_core::document::{Direction, FindFormat, FindOptions};
use docxtor_core::model::{walk_paragraphs, Paragraph};
use docxtor_core::stories::StoryKind;
use docxtor_core::{package::Package, Document};
use serde_json::{json, Value};

fn assert_valid(bytes: &[u8]) {
    let pkg = Package::open(bytes).unwrap();
    for part in &pkg.parts {
        if part.name.ends_with(".xml") || part.name.ends_with(".rels") {
            let doc = docxtor_core::xml::parse(&part.data).unwrap_or_else(|e| panic!("{}: {e}", part.name));
            docxtor_core::xml::check_namespaces(&doc).unwrap_or_else(|e| panic!("{}: {e}", part.name));
        }
    }
}

fn ops(doc: &mut Document, json: Value) -> Vec<docxtor_core::OpResult> {
    doc.apply_json(&json.to_string(), false).unwrap_or_else(|e| panic!("{e}: {json}"))
}

fn doc_with(md: &str) -> Document {
    let mut doc = Document::new_blank();
    let first = doc.render_all().order[0];
    ops(&mut doc, json!({"op":"insert_markdown","before":first,"markdown":md}));
    ops(&mut doc, json!({"op":"delete_blocks","ids":[first]}));
    doc
}

fn texts(doc: &Document) -> Vec<String> {
    let mut v = Vec::new();
    walk_paragraphs(&doc.body, &mut |p: &Paragraph| v.push(p.plain_text()));
    v
}

fn story_texts(doc: &Document, kind: StoryKind) -> Vec<String> {
    let mut v = Vec::new();
    for s in doc.stories.iter().filter(|s| s.kind == kind) {
        walk_paragraphs(&s.blocks, &mut |p: &Paragraph| v.push(p.plain_text()));
    }
    v
}

fn find(doc: &Document, q: &str, opts: Value) -> Vec<String> {
    let o: FindOptions = serde_json::from_value(opts).unwrap();
    doc.find(q, &o).unwrap().hits.iter().map(|h| h.text.clone()).collect()
}

fn replace(doc: &mut Document, find: &str, rep: &str, opts: Value) -> usize {
    let mut v = json!({"op":"replace_text","find":find,"replace":rep,"all":true});
    for (k, x) in opts.as_object().unwrap() {
        v[k] = x.clone();
    }
    ops(doc, v)[0].count.unwrap()
}

fn saved(doc: &mut Document) -> Vec<u8> {
    let b = doc.save().unwrap();
    assert_valid(&b);
    b
}

#[test]
fn plain_text_whole_words_prefix_suffix() {
    let doc = doc_with("The cat sat. Concatenate the cats and cat.");
    assert_eq!(find(&doc, "cat", json!({})).len(), 4);
    assert_eq!(find(&doc, "cat", json!({"whole_word": true})).len(), 2);
    assert_eq!(find(&doc, "cat", json!({"prefix": true})).len(), 3);
    assert_eq!(find(&doc, "cat", json!({"suffix": true})).len(), 2);
    assert_eq!(find(&doc, "THE", json!({})).len(), 0);
    assert_eq!(find(&doc, "THE", json!({"ignore_case": true})).len(), 2);
}

#[test]
fn special_characters_and_their_replacements() {
    let mut doc = doc_with("one\ttwo\n\nthree 4 five 67");
    assert_eq!(find(&doc, "^t", json!({})).len(), 1);
    assert_eq!(find(&doc, "^#", json!({})), vec!["4", "6", "7"]);
    assert_eq!(find(&doc, "^#^#", json!({})), vec!["67"]);
    assert_eq!(find(&doc, "e^$", json!({})).len(), 1);
    assert_eq!(find(&doc, "^p", json!({})).len(), 1);
    // A tab becomes a space, and the found text comes back with ^&.
    assert_eq!(replace(&mut doc, "^t", " ", json!({})), 1);
    assert_eq!(replace(&mut doc, "five", "^&^+^&", json!({})), 1);
    assert_eq!(texts(&doc)[0], "one two");
    assert_eq!(texts(&doc)[1], "three 4 five\u{2014}five 67");
}

#[test]
fn reusing_found_text_that_holds_an_object_is_refused() {
    let mut doc = doc_with("before after");
    let id = doc.render_all().order[0];
    ops(&mut doc, json!({"op":"insert_footnote","at":{"p":id,"o":6},"text":"a note"}));
    let before = texts(&doc);
    // Plain text with ^&, a wildcard search with \0, a regular expression with $&.
    let cases = [("before^f after", "[^&]", json!({})), ("before^f after", "[\\0]", json!({"wildcards": true})), ("before. after", "[$&]", json!({"regex": true}))];
    for (find, rep, opts) in cases {
        let mut v = json!({"op":"replace_text","find":find,"replace":rep,"all":true});
        for (k, x) in opts.as_object().unwrap() {
            v[k] = x.clone();
        }
        let err = doc.apply_json(&v.to_string(), false).unwrap_err().to_string();
        assert!(err.contains("picture, note mark or equation"), "{rep}: {err}");
    }
    assert_eq!(texts(&doc), before);
    // The same replacement over text without an object still works.
    assert_eq!(replace(&mut doc, "before", "[^&]", json!({})), 1);
}

#[test]
fn a_caret_that_is_not_a_code_is_a_caret() {
    let mut doc = doc_with("E=mc^2 and a^^b");
    assert_eq!(find(&doc, "mc^2", json!({})), vec!["mc^2"]);
    assert_eq!(find(&doc, "a^^b", json!({})).len(), 0);
    assert_eq!(replace(&mut doc, "mc^2", "mc^^2", json!({})), 1);
    assert_eq!(texts(&doc)[0], "E=mc^2 and a^^b");
}

#[test]
fn paragraph_marks_join_and_split() {
    let mut doc = doc_with("alpha\n\nbeta\n\ngamma");
    assert_eq!(texts(&doc).len(), 3);
    // ^p in the text to find joins paragraphs.
    assert_eq!(replace(&mut doc, "alpha^pbeta", "alpha beta", json!({})), 1);
    assert_eq!(texts(&doc), vec!["alpha beta", "gamma"]);
    // ^p in the replacement splits them.
    assert_eq!(replace(&mut doc, " ", "^p", json!({})), 1);
    assert_eq!(texts(&doc), vec!["alpha", "beta", "gamma"]);
    assert!(doc.undo());
    assert_eq!(texts(&doc), vec!["alpha beta", "gamma"]);
    assert!(doc.undo());
    assert_eq!(texts(&doc), vec!["alpha", "beta", "gamma"]);
    // Empty paragraphs between: ^p^p becomes one mark.
    let mut doc = doc_with("a\n\nb");
    let first = doc.render_all().order[0];
    ops(&mut doc, json!({"op":"insert_paragraph","after":first,"text":""}));
    assert_eq!(texts(&doc), vec!["a", "", "b"]);
    assert_eq!(replace(&mut doc, "a^p^p", "a^p", json!({})), 1);
    assert_eq!(texts(&doc), vec!["a", "b"]);
    saved(&mut doc);
}

#[test]
fn wildcards() {
    let mut doc = doc_with("sat set sit seat Smith, John Smith and Jane Doe");
    assert_eq!(find(&doc, "<s?t>", json!({"wildcards": true})), vec!["sat", "set", "sit"]);
    assert_eq!(find(&doc, "[A-Z][a-z]{3,}", json!({"wildcards": true})), vec!["Smith", "John", "Smith", "Jane"]);
    assert_eq!(find(&doc, "[!a-z ,]*", json!({"wildcards": true}))[0], "S");
    // Groups in the replacement.
    assert_eq!(replace(&mut doc, "(<[A-Z][a-z]@>) (<[A-Z][a-z]@>)", "\\2, \\1", json!({"wildcards": true})), 2);
    assert_eq!(texts(&doc)[0], "sat set sit seat Smith, John Smith and Jane Doe".replace("John Smith", "Smith, John").replace("Jane Doe", "Doe, Jane"));
    assert!(call_tool(&mut doc, "search", &json!({"query": "[abc", "wildcards": true})).is_err());
}

#[test]
fn regular_expressions() {
    let mut doc = doc_with("Born 1990-05-17 and 2001-12-03. Call 555-1234.");
    assert_eq!(find(&doc, r"\d{4}-\d{2}-\d{2}", json!({"regex": true})), vec!["1990-05-17", "2001-12-03"]);
    assert_eq!(replace(&mut doc, r"(\d{4})-(\d{2})-(\d{2})", "$3/$2/$1", json!({"regex": true})), 2);
    assert_eq!(texts(&doc)[0], "Born 17/05/1990 and 03/12/2001. Call 555-1234.");
    assert_eq!(replace(&mut doc, r"(?<area>\d{3})-(?<num>\d{4})", "$<num> ($<area>) $$", json!({"regex": true})), 1);
    assert_eq!(texts(&doc)[0], "Born 17/05/1990 and 03/12/2001. Call 1234 (555) $.");
    assert!(doc.find("(unclosed", &serde_json::from_value(json!({"regex": true})).unwrap()).is_err());
    saved(&mut doc);
}

#[test]
fn replacing_keeps_the_case_when_match_case_is_off() {
    let mut doc = doc_with("Apple and APPLE and apple and Big Apple.");
    assert_eq!(replace(&mut doc, "apple", "pear", json!({"ignore_case": true, "keep_case": true})), 4);
    assert_eq!(texts(&doc)[0], "Pear and PEAR and pear and Big Pear.");
    // Without keep_case the replacement is as typed.
    let mut doc = doc_with("Apple APPLE");
    replace(&mut doc, "apple", "pear", json!({"ignore_case": true}));
    assert_eq!(texts(&doc)[0], "pear pear");
}

#[test]
fn ignoring_punctuation_white_space_and_marks() {
    let doc = doc_with("A well-known cafe, the café; a e i o u!");
    assert_eq!(find(&doc, "wellknown", json!({"ignore_punct": true})), vec!["well-known"]);
    assert_eq!(find(&doc, "aei", json!({"ignore_space": true})), vec!["a e i"]);
    assert_eq!(find(&doc, "cafe", json!({"ignore_marks": true})), vec!["cafe", "café"]);
    assert_eq!(find(&doc, "cafe", json!({})), vec!["cafe"]);
    assert_eq!(find(&doc, "welnone", json!({"sounds_like": true})).len(), 0);
}

#[test]
fn sounds_like() {
    let doc = doc_with("Smith met Smythe and Jones.");
    assert_eq!(find(&doc, "Smith", json!({"sounds_like": true})), vec!["Smith", "Smythe"]);
}

#[test]
fn headers_footers_notes_and_main_only() {
    let mut doc = doc_with("Body mentions Acme here.\n\nAnother Acme line.");
    let h = ops(&mut doc, json!({"op":"add_header_footer","kind":"header"}))[0].caret.unwrap();
    ops(&mut doc, json!({"op":"insert_text","at":{"p":h.p,"o":0},"text":"Acme header"}));
    let f = ops(&mut doc, json!({"op":"add_header_footer","kind":"footer"}))[0].caret.unwrap();
    ops(&mut doc, json!({"op":"insert_text","at":{"p":f.p,"o":0},"text":"Footer by Acme"}));
    let p = doc.render_all().order[0];
    ops(&mut doc, json!({"op":"insert_footnote","at":{"p":p,"o":4},"text":"Note about Acme."}));
    let all = doc.find("Acme", &Default::default()).unwrap();
    assert_eq!(all.hits.len(), 5);
    assert_eq!(all.hits.iter().filter(|h| h.story.is_some()).count(), 3);
    let main = doc.find("Acme", &FindOptions { main_only: true, ..Default::default() }).unwrap();
    assert_eq!(main.hits.len(), 2);
    // Replace all reaches every story.
    assert_eq!(replace(&mut doc, "Acme", "Globex", json!({})), 5);
    assert_eq!(story_texts(&doc, StoryKind::Header), vec!["Globex header"]);
    assert_eq!(story_texts(&doc, StoryKind::Footer), vec!["Footer by Globex"]);
    assert_eq!(story_texts(&doc, StoryKind::Footnote), vec![" Note about Globex."]);
    // main_only leaves them.
    assert_eq!(replace(&mut doc, "Globex", "Initech", json!({"main_only": true})), 2);
    assert_eq!(story_texts(&doc, StoryKind::Header), vec!["Globex header"]);
    // One undo undoes a whole replace across the stories.
    assert!(doc.undo());
    assert_eq!(texts(&doc)[0], "Body mentions Globex here.");
    assert!(doc.undo());
    assert_eq!(story_texts(&doc, StoryKind::Header), vec!["Acme header"]);
    assert_eq!(texts(&doc)[0], "Body mentions Acme here.");
    saved(&mut doc);
}

#[test]
fn text_boxes_are_searched() {
    let mut doc = doc_with("Before the box.");
    let p = doc.render_all().order[0];
    ops(&mut doc, json!({"op":"insert_text_box","at":{"p":p,"o":6},"w":144,"h":60,"text":"Inside the box","floating":false}));
    let hits = doc.find("Inside", &Default::default()).unwrap().hits;
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].story.as_ref().map(|s| s.kind), Some(StoryKind::TextBox));
    assert_eq!(replace(&mut doc, "Inside", "Within", json!({})), 1);
    assert_eq!(story_texts(&doc, StoryKind::TextBox), vec!["Within the box"]);
    let bytes = saved(&mut doc);
    let again = Document::open(&bytes).unwrap();
    assert_eq!(story_texts(&again, StoryKind::TextBox), vec!["Within the box"]);
}

#[test]
fn down_and_up_from_a_position() {
    let mut doc = doc_with("one two\n\none two\n\none two");
    let ids = doc.render_all().order;
    let from = docxtor_core::Pos::new(ids[1], 0);
    let down = FindOptions { direction: Direction::Down, from: Some(from), ..Default::default() };
    assert_eq!(doc.find("one", &down).unwrap().hits.len(), 2);
    let up = FindOptions { direction: Direction::Up, from: Some(from), ..Default::default() };
    assert_eq!(doc.find("one", &up).unwrap().hits.len(), 1);
    // Replace the first match after the position only.
    let r = ops(&mut doc, json!({"op":"replace_text","find":"one","replace":"1","from":{"p":from.p,"o":0},"direction":"down"}));
    assert_eq!(r[0].count, Some(1));
    assert_eq!(texts(&doc), vec!["one two", "1 two", "one two"]);
}

#[test]
fn formatting_to_find_and_to_put_on() {
    let mut doc = doc_with("Plain words, **bold words** and *italic words* then **more bold**.");
    let o = FindOptions { find_format: Some(FindFormat { bold: Some(true), ..Default::default() }), ..Default::default() };
    let hits: Vec<String> = doc.find("", &o).unwrap().hits.iter().map(|h| h.text.clone()).collect();
    assert_eq!(hits, vec!["bold words", "more bold"]);
    let o = FindOptions { find_format: Some(FindFormat { bold: Some(true), ..Default::default() }), ..Default::default() };
    assert_eq!(doc.find("words", &o).unwrap().hits.len(), 1);
    // Plain text with no bold in it.
    let o = FindOptions { find_format: Some(FindFormat { bold: Some(false), italic: Some(false), ..Default::default() }), ..Default::default() };
    assert!(doc.find("Plain", &o).unwrap().hits.len() == 1);
    // Replace the bold text's look with the Emphasis character style, leaving the words alone.
    let n = ops(&mut doc, json!({"op":"replace_text","find":"","all":true,"find_format":{"bold":true},"format":{"style":"Emphasis","bold":false}}));
    assert_eq!(n[0].count, Some(2));
    assert_eq!(texts(&doc)[0], "Plain words, bold words and italic words then more bold.");
    let after = doc.find("", &FindOptions { find_format: Some(FindFormat { style: Some("Emphasis".into()), ..Default::default() }), ..Default::default() }).unwrap();
    assert_eq!(after.hits.iter().map(|h| h.text.as_str()).collect::<Vec<_>>(), vec!["bold words", "more bold"]);
    let none = doc.find("", &FindOptions { find_format: Some(FindFormat { bold: Some(true), ..Default::default() }), ..Default::default() });
    assert_eq!(none.unwrap().hits.len(), 0);
    assert!(doc.undo());
    saved(&mut doc);
}

#[test]
fn replacing_with_formatting_and_a_paragraph_style() {
    let mut doc = doc_with("Intro\n\nKey point here\n\nOutro");
    ops(&mut doc, json!({"op":"replace_text","find":"Key point","replace":"Highlight","format":{"bold":true,"color":"FF0000"},"para_format":{"align":"center"}}));
    assert_eq!(texts(&doc)[1], "Highlight here");
    let hits = doc.find("", &FindOptions { find_format: Some(FindFormat { bold: Some(true), color: Some("FF0000".into()), ..Default::default() }), ..Default::default() }).unwrap().hits;
    assert_eq!(hits.iter().map(|h| h.text.as_str()).collect::<Vec<_>>(), vec!["Highlight"]);
    let centred = doc.find("", &FindOptions { find_format: Some(FindFormat { align: Some("center".into()), ..Default::default() }), ..Default::default() }).unwrap().hits;
    assert_eq!(centred.len(), 1);
    // A paragraph style on the paragraph of the match.
    ops(&mut doc, json!({"op":"replace_text","find":"Intro","para_format":{"style":"Heading1"}}));
    let heads = doc.find("", &FindOptions { find_format: Some(FindFormat { para_style: Some("Heading1".into()), ..Default::default() }), ..Default::default() }).unwrap().hits;
    assert_eq!(heads.iter().map(|h| h.text.as_str()).collect::<Vec<_>>(), vec!["Intro"]);
    // A language on the match.
    ops(&mut doc, json!({"op":"replace_text","find":"Outro","language":"fr-FR"}));
    let fr = doc.find("", &FindOptions { find_format: Some(FindFormat { lang: Some("fr-FR".into()), ..Default::default() }), ..Default::default() }).unwrap().hits;
    assert_eq!(fr.iter().map(|h| h.text.as_str()).collect::<Vec<_>>(), vec!["Outro"]);
    saved(&mut doc);
}

#[test]
fn format_at_reads_what_a_search_finds() {
    let mut doc = doc_with("Plain **bold** end");
    let id = doc.render_all().order[0];
    let f = doc.find_format_at(id, 7).unwrap();
    assert_eq!(f.bold, Some(true));
    let hits = doc.find("", &FindOptions { find_format: Some(f), ..Default::default() }).unwrap().hits;
    assert_eq!(hits.iter().map(|h| h.text.as_str()).collect::<Vec<_>>(), vec!["bold"]);
}

#[test]
fn the_tools_take_the_options() {
    let mut doc = doc_with("Alpha beta. Gamma delta.");
    let r = call_tool(&mut doc, "search", &json!({"query": "^[A-Z]", "regex": true})).unwrap();
    assert!(r.contains("matching paragraph"), "{r}");
    let r = call_tool(&mut doc, "replace_text", &json!({"find": "(\\w+) (\\w+)\\.", "replace": "$2 $1.", "regex": true, "all": true})).unwrap();
    assert!(r.contains("Replaced 2"), "{r}");
    assert_eq!(texts(&doc)[0], "beta Alpha. delta Gamma.");
    assert!(call_tool(&mut doc, "replace_text", &json!({"find": "nope", "replace": "x"})).is_err());
    let r = call_tool(&mut doc, "replace_text", &json!({"find": "", "replace": "", "find_format": {"bold": true}, "format": {"italic": true}}));
    assert!(r.is_err());
    // Plain-text search still finds text as before.
    let r = call_tool(&mut doc, "search", &json!({"query": "GAMMA", "ignore_case": true})).unwrap();
    assert!(r.contains("1 matching"), "{r}");
}

#[test]
fn replace_all_in_a_long_document_is_quick() {
    let paragraphs: Vec<String> = (0..6000).map(|i| format!("Paragraph {i} has the word foo and another foo, then bar.")).collect();
    let mut doc = doc_with(&paragraphs.join("\n\n"));
    let t = std::time::Instant::now();
    let found = doc.find("foo", &Default::default()).unwrap();
    assert_eq!(found.hits.len(), 12000);
    assert!(t.elapsed().as_secs() < 30, "find took {:?}", t.elapsed());
    let t = std::time::Instant::now();
    assert_eq!(replace(&mut doc, "foo", "baz", json!({})), 12000);
    assert!(t.elapsed().as_secs() < 90, "replace took {:?}", t.elapsed());
}

fn labels(doc: &Document, kind: &str) -> Vec<String> {
    doc.goto_targets(kind).unwrap().into_iter().map(|t| t.label).collect()
}

#[test]
fn go_to_lists_the_places_of_each_kind() {
    let mut doc = doc_with("# Intro\n\nBody text.\n\n## Methods\n\nMore text here.\n\n| A | B |\n| --- | --- |\n| 1 | 2 |\n\nLast paragraph.");
    assert_eq!(labels(&doc, "heading"), vec!["Heading 1: Intro", "Heading 2: Methods"]);
    assert_eq!(labels(&doc, "table"), vec!["Table 1"]);
    assert_eq!(labels(&doc, "section"), vec!["Section 1"]);
    let body = texts(&doc);
    assert_eq!(body[0], "Intro");
    let more = doc.render_all().order[3];
    ops(&mut doc, json!({"op":"insert_footnote","at":{"p":more,"o":3},"text":"First note."}));
    ops(&mut doc, json!({"op":"insert_footnote","at":{"p":more,"o":8},"text":"Second note."}));
    let notes = labels(&doc, "footnote");
    assert_eq!(notes.len(), 2);
    assert!(notes[0].starts_with("Footnote 1: ") && notes[1].starts_with("Footnote 2: "), "{notes:?}");
    assert!(labels(&doc, "endnote").is_empty());
    ops(&mut doc, json!({"op":"insert_equation","at":{"p":more,"o":0},"latex":"x^2"}));
    assert_eq!(labels(&doc, "equation"), vec!["Equation 1"]);
    assert!(doc.goto_targets("nonsense").is_err());
}

fn fixture(name: &str) -> Vec<u8> {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures").join(name);
    std::fs::read(&p).unwrap_or_else(|_| panic!("missing fixture {p:?}"))
}

#[test]
fn breaks_notes_pictures_and_fields_are_found_by_their_codes() {
    let mut doc = doc_with("First part\n\nSecond part\n\nThird part and a note");
    let ids = doc.render_all().order;
    ops(&mut doc, json!({"op":"insert_break","at":{"p":ids[0],"o":5},"kind":"page"}));
    ops(&mut doc, json!({"op":"insert_break","at":{"p":ids[1],"o":3},"kind":"column"}));
    ops(&mut doc, json!({"op":"insert_text","at":{"p":ids[1],"o":0},"text":"one\ntwo"}));
    ops(&mut doc, json!({"op":"insert_break","at":{"p":ids[1],"o":0},"kind":"next_page"}));
    let ids = doc.render_all().order;
    let third = *ids.last().unwrap();
    ops(&mut doc, json!({"op":"insert_footnote","at":{"p":third,"o":5},"text":"A note."}));
    assert_eq!(find(&doc, "^m", json!({})).len(), 1, "a page break");
    assert_eq!(find(&doc, "^n", json!({})).len(), 1, "a column break");
    assert_eq!(find(&doc, "^l", json!({})).len(), 1, "a line break");
    assert_eq!(find(&doc, "^b", json!({})).len(), 1, "a section break");
    assert_eq!(find(&doc, "^f", json!({})).len(), 1, "a footnote mark");
    assert_eq!(find(&doc, "^e", json!({})).len(), 0);
    // A page break can be turned into a paragraph mark and back.
    assert_eq!(replace(&mut doc, "^m", "^p", json!({})), 1);
    assert_eq!(find(&doc, "^m", json!({})).len(), 0);
    assert_eq!(replace(&mut doc, "^n", "^m", json!({})), 1);
    assert_eq!(find(&doc, "^m", json!({})).len(), 1);
    saved(&mut doc);
}

#[test]
fn pictures_and_fields_in_a_real_document() {
    let mut doc = Document::open(&fixture("rich.docx")).unwrap();
    assert_eq!(find(&doc, "^g", json!({})).len(), 1, "the picture");
    // The page number field in the footer.
    let hits = doc.find("^d", &FindOptions::default()).unwrap().hits;
    assert!(hits.iter().any(|h| h.story.as_ref().is_some_and(|s| s.kind == StoryKind::Footer)), "{hits:?}");
    assert_eq!(doc.find("^d", &FindOptions { main_only: true, ..Default::default() }).unwrap().hits.len(), 0);
    // Replacing text around a picture leaves the picture.
    let before = doc.save().unwrap();
    assert_eq!(replace(&mut doc, "A chart placeholder image:", "A chart:", json!({})), 1);
    assert_eq!(find(&doc, "^g", json!({})).len(), 1);
    assert!(doc.save().unwrap().len() > 1000 && before.len() > 1000);
    saved(&mut doc);
}

#[test]
fn a_regular_expression_with_flags_and_a_cap_on_runaway_patterns() {
    let doc = doc_with(&"a".repeat(5000));
    let start = std::time::Instant::now();
    let r = doc.find("(a+)+b", &serde_json::from_value(json!({"regex": true})).unwrap()).unwrap();
    assert!(r.hits.is_empty());
    assert!(start.elapsed().as_secs() < 20, "took {:?}", start.elapsed());
}

#[test]
fn tables_are_searched_and_a_paragraph_mark_stays_in_its_cell() {
    let mut doc = doc_with("Before\n\n| Name | Qty |\n| --- | --- |\n| Apples | 3 |\n| Pears | 5 |\n\nAfter");
    assert_eq!(find(&doc, "Apples", json!({})).len(), 1);
    assert_eq!(find(&doc, "Qty^p", json!({})).len(), 0, "a cell's text ends where the cell does");
    assert_eq!(find(&doc, "Before^pAfter", json!({})).len(), 0, "the table is between them");
    assert_eq!(replace(&mut doc, "Pears", "Plums", json!({})), 1);
    assert!(texts(&doc).contains(&"Plums".to_string()));
    assert_eq!(replace(&mut doc, "[0-9]", "#", json!({"wildcards": true})), 2);
    saved(&mut doc);
}

#[test]
fn a_very_long_text_to_find_is_searched_and_a_very_long_pattern_is_refused() {
    let long = "lorem ipsum dolor sit amet ".repeat(800);
    let doc = doc_with(&long);
    // Plain text of any length is matched without nesting a call per character.
    let hits = doc.find(&long[..255], &FindOptions::default()).unwrap();
    assert!(!hits.hits.is_empty());
    assert!(doc.find(&long[..5000], &FindOptions::default()).is_ok());
    // A regex or wildcard pattern past Word's 255 characters is an error, not a stack overflow.
    let pattern = "(a)".repeat(100);
    for o in [FindOptions { regex: true, ..Default::default() }, FindOptions { wildcards: true, ..Default::default() }] {
        let err = doc.find(&pattern, &o).unwrap_err().to_string();
        assert!(err.contains("too long") && err.contains("255"), "{err}");
    }
    assert!(doc.find(&"(a)".repeat(80), &FindOptions { regex: true, ..Default::default() }).is_ok());
}
