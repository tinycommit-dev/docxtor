//! `docxtor check` and `docxtor compare` run end to end.

use std::path::PathBuf;
use std::process::{Command, Output};

fn docxtor(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_docxtor")).args(args).output().expect("run docxtor")
}

fn fixture(name: &str) -> String {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures").join(name).display().to_string()
}

fn scratch(name: &str) -> String {
    let dir = std::env::temp_dir().join(format!("docxtor-cli-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir.join(name).display().to_string()
}

fn text(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).to_string()
}

#[test]
fn check_prints_issues_and_exits_cleanly() {
    let out = docxtor(&["check", &fixture("rich.docx")]);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let t = text(&out);
    assert!(t.contains("accessibility issue(s)") && t.contains("missing_alt") && t.contains("table_header"), "{t}");

    let out = docxtor(&["check", &fixture("rich.docx"), "--json"]);
    assert!(out.status.success());
    let v: serde_json::Value = serde_json::from_str(&text(&out)).unwrap();
    assert!(v.as_array().unwrap().iter().any(|i| i["kind"] == "missing_alt" && i["severity"] == "error" && i["p"].is_number()));

    // A missing file is an error.
    assert!(!docxtor(&["check", "/nonexistent.docx"]).status.success());
}

#[test]
fn compare_writes_a_redline() {
    let new = scratch("new.docx");
    let red = scratch("red.docx");
    let edited = docxtor(&["tool", &fixture("rich.docx"), "replace_text", r#"{"find":"Quarterly","replace":"Annual","all":true}"#, "-o", &new]);
    assert!(edited.status.success(), "{}", String::from_utf8_lossy(&edited.stderr));
    let out = docxtor(&["compare", &fixture("rich.docx"), &new, "-o", &red, "--author", "Ada", "--date", "2026-10-12T09:30:00Z"]);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert!(text(&out).contains("1 changed"), "{}", text(&out));
    let doc = docxtor_core::package::Package::open(&std::fs::read(&red).unwrap()).unwrap();
    let xml = String::from_utf8(doc.get("word/document.xml").unwrap().to_vec()).unwrap();
    assert!(xml.contains(r#"<w:del w:id="1" w:author="Ada" w:date="2026-10-12T09:30:00Z">"#), "{xml}");
    assert!(xml.contains("<w:delText xml:space=\"preserve\">Quarterly</w:delText>") && xml.contains("<w:t>Annual</w:t>"));

    // The default date is the current time, and -o is required.
    let out = docxtor(&["compare", &fixture("rich.docx"), &new, "-o", &red]);
    assert!(out.status.success());
    let doc = docxtor_core::package::Package::open(&std::fs::read(&red).unwrap()).unwrap();
    let xml = String::from_utf8(doc.get("word/document.xml").unwrap().to_vec()).unwrap();
    assert!(xml.contains(r#"w:author="Docxtor" w:date="20"#), "{xml}");
    let out = docxtor(&["compare", &fixture("rich.docx"), &new]);
    assert!(!out.status.success() && String::from_utf8_lossy(&out.stderr).contains("-o"));
}

#[test]
fn replace_text_takes_the_find_options() {
    // A regular expression with a group, in the text and in the header (which says "Docxtor — confidential draft").
    let new = scratch("regex.docx");
    let out = docxtor(&["tool", &fixture("rich.docx"), "replace_text", r#"{"find":"(Docxtor|Quarterly) (\\S+)","replace":"$2 $1","regex":true,"all":true}"#, "-o", &new]);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let doc = docxtor_core::Document::open(&std::fs::read(&new).unwrap()).unwrap();
    let all = doc.find("Product Quarterly", &Default::default()).unwrap();
    assert_eq!(all.hits.len(), 1, "{all:?}");
    let header = doc.find("— Docxtor", &Default::default()).unwrap();
    assert_eq!(header.hits.len(), 1, "the header is searched too");
    // A search with a wildcard reports the paragraphs.
    let out = docxtor(&["tool", &fixture("rich.docx"), "search", r#"{"query":"<Q?>","wildcards":true}"#]);
    assert!(out.status.success());
    assert!(text(&out).contains("matching paragraph") || text(&out).contains("No paragraphs"), "{}", text(&out));
    // A pattern that is not valid is an error with a reason.
    let out = docxtor(&["tool", &fixture("rich.docx"), "search", r#"{"query":"(open","regex":true}"#]);
    assert!(!out.status.success());
}

#[test]
fn count_prints_paragraphs_words_and_characters() {
    let out = docxtor(&["count", &fixture("rich.docx")]);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert!(text(&out).contains("paragraphs, 98 words,"), "{}", text(&out));
    let out = docxtor(&["count", &fixture("rich.docx"), "--json", "--extras"]);
    let v: serde_json::Value = serde_json::from_str(&text(&out)).unwrap();
    assert_eq!(v["counted"]["words"], 98 + v["notes"]["words"].as_u64().unwrap() + v["boxes"]["words"].as_u64().unwrap());
    assert!(!docxtor(&["count", "/nonexistent.docx"]).status.success());
}

#[test]
fn inspect_lists_and_removes() {
    let out = docxtor(&["inspect", &fixture("complex.docx"), "--json"]);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let v: serde_json::Value = serde_json::from_str(&text(&out)).unwrap();
    let props = v.as_array().unwrap().iter().find(|f| f["id"] == "properties").unwrap();
    assert!(props["detail"].as_array().unwrap().iter().any(|d| d == "author: python-docx"), "{props}");
    assert!(props["detail"].as_array().unwrap().iter().any(|d| d == "thumbnail"), "{props}");
    let cleaned = scratch("cleaned.docx");
    let out = docxtor(&["inspect", &fixture("complex.docx"), "--remove", "all", "-o", &cleaned]);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert!(text(&out).contains("removed") && text(&out).contains("wrote"), "{}", text(&out));
    let pkg = docxtor_core::package::Package::open(&std::fs::read(&cleaned).unwrap()).unwrap();
    assert!(pkg.get("docProps/thumbnail.jpeg").is_none());
    assert!(!String::from_utf8_lossy(pkg.get("docProps/core.xml").unwrap()).contains("python-docx</dc:creator>"));
    // The original is untouched, and a kind that doesn't exist is an error.
    assert!(docxtor_core::package::Package::open(&std::fs::read(fixture("complex.docx")).unwrap()).unwrap().get("docProps/thumbnail.jpeg").is_some());
    assert!(!docxtor(&["inspect", &fixture("complex.docx"), "--remove", "bogus", "-o", &cleaned]).status.success());
}

#[test]
fn combine_writes_the_revised_document_with_the_differences_marked() {
    let revised = scratch("revised.docx");
    let combined = scratch("combined.docx");
    let edited = docxtor(&["tool", &fixture("rich.docx"), "replace_text", r#"{"find":"Quarterly","replace":"Annual","all":true}"#, "-o", &revised]);
    assert!(edited.status.success(), "{}", String::from_utf8_lossy(&edited.stderr));
    let out = docxtor(&["combine", &fixture("rich.docx"), &revised, "-o", &combined, "--author", "Ada", "--date", "2026-10-12T09:30:00Z"]);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let pkg = docxtor_core::package::Package::open(&std::fs::read(&combined).unwrap()).unwrap();
    let xml = String::from_utf8(pkg.get("word/document.xml").unwrap().to_vec()).unwrap();
    assert!(xml.contains(r#"w:author="Ada" w:date="2026-10-12T09:30:00Z""#), "{xml}");
    assert!(xml.contains("<w:delText xml:space=\"preserve\">Quarterly</w:delText>") && xml.contains("<w:t>Annual</w:t>"));
    assert!(!docxtor(&["combine", &fixture("rich.docx"), &revised]).status.success());
}

/// A damaged file Docxtor can repair does not open as a package, so the commands that compare a save with the original
/// used to panic on it (`roundtrip`, `info`, `fidelity`). They report the engine's answer instead of an internal error.
#[test]
fn commands_that_compare_with_the_original_do_not_panic_on_a_damaged_file() {
    let good = std::fs::read(fixture("rich.docx")).unwrap();
    // The end of the zip (its directory) is gone: Docxtor reads the parts one by one.
    let damaged = scratch("damaged.docx");
    std::fs::write(&damaged, &good[..good.len() * 3 / 4]).unwrap();
    let out = scratch("damaged-out.docx");
    for args in [vec!["roundtrip", &damaged, &out], vec!["info", &damaged], vec!["fidelity", &damaged, &damaged]] {
        let o = docxtor(&args);
        let err = String::from_utf8_lossy(&o.stderr).to_string();
        assert!(!err.contains("panicked"), "{args:?}: {err}");
        assert_ne!(o.status.code(), Some(101), "{args:?}: {err}");
    }
}
