//! The command line never writes over a document in place: a crash or a full disk part way through must not leave a short file.

use std::path::PathBuf;
use std::process::{Command, Output};

fn docxtor(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_docxtor")).args(args).env_remove("DOCXTOR_PASSWORD").output().expect("run docxtor")
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("docxtor-cli-safe-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn files(dir: &std::path::Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    v.sort();
    v
}

/// A write that goes through a temporary file and a rename gives the target a new file; one that truncates and rewrites the target
/// changes the file every hard link shares. The link is the proof the old file was never touched.
#[cfg(unix)]
#[test]
fn a_write_replaces_the_file_and_never_rewrites_it_in_place() {
    let dir = scratch("inplace");
    let target = dir.join("doc.docx");
    let out = docxtor(&["new", target.to_str().unwrap(), "--markdown", "# First"]);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let first = std::fs::read(&target).unwrap();
    let link = dir.join("old-copy.docx");
    std::fs::hard_link(&target, &link).unwrap();

    let out = docxtor(&["new", target.to_str().unwrap(), "--markdown", "# Second"]);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert_ne!(std::fs::read(&target).unwrap(), first, "the document was written");
    assert_eq!(std::fs::read(&link).unwrap(), first, "the old file was not rewritten in place");
    assert_eq!(files(&dir), vec!["doc.docx", "old-copy.docx"], "no temporary file is left");
}

/// What the command line wrote can be opened again, and a password-protected result is still protected.
#[test]
fn the_written_file_opens_again() {
    let dir = scratch("reopen");
    let target = dir.join("doc.docx");
    let out = docxtor(&["new", target.to_str().unwrap(), "--markdown", "# Title\n\nBody."]);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let out = docxtor(&["check", target.to_str().unwrap()]);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));

    let protected = dir.join("secret.docx");
    let out = docxtor(&["encrypt", target.to_str().unwrap(), protected.to_str().unwrap(), "--password", "pw one"]);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert!(docxtor_core::office_crypt::is_encrypted(&std::fs::read(&protected).unwrap()));
    assert_eq!(files(&dir), vec!["doc.docx", "secret.docx"]);
}

/// A folder that can't be written to gives a plain error that names the file, and the original is left as it was.
#[cfg(unix)]
#[test]
fn a_folder_that_cannot_be_written_leaves_the_original() {
    use std::os::unix::fs::PermissionsExt;
    let dir = scratch("readonly");
    let target = dir.join("precious.docx");
    assert!(docxtor(&["new", target.to_str().unwrap(), "--markdown", "# Keep me"]).status.success());
    let original = std::fs::read(&target).unwrap();
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o555)).unwrap();
    let blocked = std::fs::File::create(dir.join("probe")).is_err();
    let out = docxtor(&["new", target.to_str().unwrap(), "--markdown", "# Overwritten"]);
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
    if !blocked {
        return; // a process that ignores folder permissions (root) can't show this
    }
    assert!(!out.status.success());
    let err = String::from_utf8_lossy(&out.stderr).to_string();
    assert!(err.contains("precious.docx") && err.contains("can't write in") && err.contains("the original is unchanged"), "{err}");
    assert_eq!(std::fs::read(&target).unwrap(), original);
    assert_eq!(files(&dir), vec!["precious.docx"]);
}
