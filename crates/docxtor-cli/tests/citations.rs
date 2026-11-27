//! `docxtor sources` and `docxtor cite-import` run end to end.

use std::process::{Command, Output};

fn docxtor(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_docxtor")).args(args).output().expect("run docxtor")
}

fn scratch(name: &str) -> String {
    let dir = std::env::temp_dir().join(format!("docxtor-cli-cite-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir.join(name).display().to_string()
}

fn text(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).to_string()
}

fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).to_string()
}

const BIB: &str = r#"@article{smith2020,
  author = {Smith, John and Lee, Karen},
  title = {A study of things},
  journal = {Journal of Stuff},
  volume = {12}, number = {3}, pages = {45--67}, year = {2020},
  doi = {10.1000/xyz123}
}
@book{kuhn2012, author = {Kuhn, Thomas S.}, title = {The structure of scientific revolutions}, publisher = {University of Chicago Press}, address = {Chicago}, year = {2012}}
"#;

const CSL: &str = r#"[{"type":"article-journal","title":"Deep learning","author":[{"given":"Yann","family":"LeCun"}],"container-title":"Nature","issued":{"date-parts":[[2015]]},"DOI":"10.1038/nature14539"}]"#;

fn new_doc(name: &str) -> String {
    let path = scratch(name);
    let out = docxtor(&["new", &path, "--markdown", "Hello there."]);
    assert!(out.status.success(), "{}", stderr(&out));
    path
}

#[test]
fn cite_import_adds_bibtex_sources_and_sources_lists_them() {
    let doc = new_doc("a.docx");
    let bib = scratch("refs.bib");
    std::fs::write(&bib, BIB).unwrap();
    let out_path = scratch("a-out.docx");
    let out = docxtor(&["cite-import", &doc, &bib, "-o", &out_path]);
    assert!(out.status.success(), "{}", stderr(&out));
    let t = text(&out);
    assert!(t.contains("added Smi20 (A study of things)") && t.contains("added Kuh12"), "{t}");
    assert!(t.contains("2 source(s) added, 0 already in the document"), "{t}");
    // The input is untouched without -o semantics: -o wrote a new file.
    assert!(docxtor(&["sources", &doc]).status.success());
    assert!(text(&docxtor(&["sources", &doc])).contains("no sources yet"));

    let list = docxtor(&["sources", &out_path]);
    assert!(list.status.success(), "{}", stderr(&list));
    let t = text(&list);
    assert!(t.contains("Citation style: apa. 2 source(s):") && t.contains("Smi20 [article] Smith, John; Lee, Karen (2020). A study of things — cited 0 times"), "{t}");

    let json = docxtor(&["sources", &out_path, "--json"]);
    let v: serde_json::Value = serde_json::from_str(&text(&json)).unwrap();
    assert_eq!(v["style"], "apa");
    assert_eq!(v["sources"].as_array().unwrap().len(), 2);
    assert_eq!(v["sources"][0]["containerTitle"], "Journal of Stuff");
    assert_eq!(v["sources"][0]["pages"], "45-67");
    assert_eq!(v["citations"], serde_json::json!([]));

    let bibtex = docxtor(&["sources", &out_path, "--bibtex"]);
    let t = text(&bibtex);
    assert!(t.contains("@article{Smi20,") && t.contains("pages = {45--67}") && t.contains("@book{Kuh12,"), "{t}");
}

#[test]
fn importing_again_skips_what_is_there_and_json_files_work_too() {
    let doc = new_doc("b.docx");
    let bib = scratch("refs2.bib");
    std::fs::write(&bib, BIB).unwrap();
    assert!(docxtor(&["cite-import", &doc, &bib]).status.success());
    // Same file again: nothing new, and the document is left alone.
    let out = docxtor(&["cite-import", &doc, &bib]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(text(&out).contains("0 source(s) added, 2 already in the document"), "{}", text(&out));
    // CSL-JSON, found by its extension.
    let csl = scratch("refs.json");
    std::fs::write(&csl, CSL).unwrap();
    let out = docxtor(&["cite-import", &doc, &csl]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(text(&out).contains("added LeC15 (Deep learning)"), "{}", text(&out));
    assert!(text(&docxtor(&["sources", &doc])).contains("3 source(s):"));
}

#[test]
fn problems_are_plain() {
    let doc = new_doc("c.docx");
    let empty = scratch("empty.bib");
    std::fs::write(&empty, "nothing here").unwrap();
    let out = docxtor(&["cite-import", &doc, &empty]);
    assert!(!out.status.success());
    assert!(stderr(&out).contains("No BibTeX entries found"), "{}", stderr(&out));
    let out = docxtor(&["cite-import", &doc, "/nonexistent.bib"]);
    assert!(!out.status.success());
    let bad = scratch("bad.json");
    std::fs::write(&bad, "{not json").unwrap();
    let out = docxtor(&["cite-import", &doc, &bad]);
    assert!(!out.status.success() && stderr(&out).contains("not valid JSON"), "{}", stderr(&out));
    assert!(!docxtor(&["sources", "/nonexistent.docx"]).status.success());
}

#[test]
fn a_cited_document_shows_counts_and_tools_work_from_the_command_line() {
    let doc = new_doc("d.docx");
    let bib = scratch("refs3.bib");
    std::fs::write(&bib, BIB).unwrap();
    assert!(docxtor(&["cite-import", &doc, &bib]).status.success());
    let read = text(&docxtor(&["read", &doc]));
    let id = read.split("[p").nth(1).unwrap().split(|c: char| !c.is_ascii_digit()).next().unwrap().to_string();
    let out = docxtor(&["tool", &doc, "cite", &format!(r#"{{"id":"p{id}","after_text":"Hello","tags":["Smi20"],"page":"4"}}"#)]);
    assert!(out.status.success(), "{}", stderr(&out));
    let out = docxtor(&["tool", &doc, "insert_bibliography", "{}"]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(text(&out).contains("Smith, J., & Lee, K. (2020)"), "{}", text(&out));
    let t = text(&docxtor(&["sources", &doc]));
    assert!(t.contains("Smi20") && t.contains("cited 1 time\n"), "{t}");
    let json: serde_json::Value = serde_json::from_str(&text(&docxtor(&["sources", &doc, "--json"]))).unwrap();
    assert_eq!(json["citations"][0]["tags"], serde_json::json!(["Smi20"]));
    assert_eq!(json["citations"][0]["page"], "4");
    // The plain-text export of the document includes the formatted citation.
    let md = text(&docxtor(&["text", &doc]));
    assert!(md.contains("Hello (Smith & Lee, 2020, p. 4) there."), "{md}");
}
