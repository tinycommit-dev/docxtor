//! `docxtor` with password-protected files: open, edit, save protected again, encrypt and decrypt.

use std::path::PathBuf;
use std::process::{Command, Output};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures").join(name)
}

fn docxtor(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_docxtor")).args(args).env_remove("DOCXTOR_PASSWORD").output().unwrap()
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("docxtor-cli-encrypt-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir.join(name)
}

fn out(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).to_string()
}

fn err(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).to_string()
}

const PW: &str = "docxtor-test-pw";

#[test]
fn reads_protected_files_with_a_password() {
    for name in ["encrypted/encrypted-agile.docx", "encrypted/encrypted-standard.docx", "encrypted/encrypted-agile-aes128-sha1.docx"] {
        let f = fixture(name);
        let o = docxtor(&["text", f.to_str().unwrap(), "--password", PW]);
        assert!(o.status.success(), "{name}: {}", err(&o));
        assert!(out(&o).contains("marmalade"), "{name}");
    }
}

#[test]
fn tells_a_missing_or_wrong_password_apart() {
    let f = fixture("encrypted/encrypted-agile.docx");
    let none = docxtor(&["text", f.to_str().unwrap()]);
    assert!(!none.status.success());
    assert!(err(&none).contains("protected with a password"));
    let wrong = docxtor(&["text", f.to_str().unwrap(), "--password", "nope"]);
    assert!(!wrong.status.success());
    assert!(err(&wrong).contains("that password isn't right"));
}

#[test]
fn the_password_can_come_from_the_environment() {
    let f = fixture("encrypted/encrypted-agile.docx");
    let o = Command::new(env!("CARGO_BIN_EXE_docxtor")).args(["text", f.to_str().unwrap()]).env("DOCXTOR_PASSWORD", PW).output().unwrap();
    assert!(o.status.success());
    assert!(out(&o).contains("marmalade"));
}

#[test]
fn edits_are_saved_protected_again() {
    let copy = scratch("edit.docx");
    std::fs::copy(fixture("encrypted/encrypted-agile.docx"), &copy).unwrap();
    let p = copy.to_str().unwrap();
    let o = docxtor(&["tool", p, "replace_text", r#"{"find":"marmalade","replace":"quince","all":true}"#, "--password", PW]);
    assert!(o.status.success(), "{}", err(&o));
    let bytes = std::fs::read(&copy).unwrap();
    assert_eq!(bytes[..8], [0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1], "still a protected file");
    let after = docxtor(&["text", p, "--password", PW]);
    assert!(out(&after).contains("quince"), "{}", out(&after));
    assert!(!out(&after).contains("marmalade"));
    assert!(!docxtor(&["text", p, "--password", "nope"]).status.success());
}

#[test]
fn encrypt_and_decrypt_round_trip() {
    let plain = scratch("plain.docx");
    assert!(docxtor(&["new", plain.to_str().unwrap(), "--markdown", "# Hello\n\nA note."]).status.success());
    let locked = scratch("locked.docx");
    let o = docxtor(&["encrypt", plain.to_str().unwrap(), locked.to_str().unwrap(), "--password", "s3cret"]);
    assert!(o.status.success(), "{}", err(&o));
    assert_eq!(std::fs::read(&locked).unwrap()[..4], [0xD0, 0xCF, 0x11, 0xE0]);
    // Encrypting again is refused.
    let twice = scratch("twice.docx");
    assert!(!docxtor(&["encrypt", locked.to_str().unwrap(), twice.to_str().unwrap(), "--password", "s3cret"]).status.success());
    let open = scratch("open.docx");
    assert!(!docxtor(&["decrypt", locked.to_str().unwrap(), open.to_str().unwrap(), "--password", "wrong"]).status.success());
    let o = docxtor(&["decrypt", locked.to_str().unwrap(), open.to_str().unwrap(), "--password", "s3cret"]);
    assert!(o.status.success(), "{}", err(&o));
    assert_eq!(std::fs::read(&open).unwrap()[..2], *b"PK");
    assert!(out(&docxtor(&["text", open.to_str().unwrap()])).contains("A note."));
}

#[test]
fn new_with_a_password_makes_a_protected_file() {
    let f = scratch("new-locked.docx");
    assert!(docxtor(&["new", f.to_str().unwrap(), "--markdown", "Secret", "--password", "pw"]).status.success());
    assert_eq!(std::fs::read(&f).unwrap()[..4], [0xD0, 0xCF, 0x11, 0xE0]);
    assert!(out(&docxtor(&["text", f.to_str().unwrap(), "--password", "pw"])).contains("Secret"));
}

#[test]
fn the_corpus_gate_skips_protected_files_without_a_password() {
    let dir = scratch("corpus");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::copy(fixture("simple.docx"), dir.join("simple.docx")).unwrap();
    std::fs::copy(fixture("encrypted/encrypted-agile.docx"), dir.join("encrypted.docx")).unwrap();
    let o = docxtor(&["corpus", dir.to_str().unwrap(), "--min", "99"]);
    assert!(o.status.success(), "{}{}", out(&o), err(&o));
    assert!(err(&o).contains("skipped 1 password-protected"));
    let o = docxtor(&["corpus", dir.to_str().unwrap(), "--min", "99", "--password", PW]);
    assert!(o.status.success(), "{}{}", out(&o), err(&o));
    assert!(out(&o).contains("2 files"));
}
