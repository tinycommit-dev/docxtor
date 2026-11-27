//! `docxtor-mcp` saves after every edit by default. A save goes through a temporary file and a rename, and a save that can't be
//! made leaves the document on disk as it was.

use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

struct Server {
    child: Child,
    stdin: ChildStdin,
    out: BufReader<ChildStdout>,
    id: u32,
}

impl Server {
    fn start() -> Server {
        let mut child = Command::new(env!("CARGO_BIN_EXE_docxtor-mcp")).env_remove("DOCXTOR_PASSWORD").env_remove("DOCXTOR_MCP_AUTOSAVE").stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().unwrap();
        let stdin = child.stdin.take().unwrap();
        let out = BufReader::new(child.stdout.take().unwrap());
        Server { child, stdin, out, id: 0 }
    }

    fn call(&mut self, name: &str, args: Value) -> Value {
        self.id += 1;
        let req = json!({"jsonrpc": "2.0", "id": self.id, "method": "tools/call", "params": {"name": name, "arguments": args}});
        writeln!(self.stdin, "{req}").unwrap();
        self.stdin.flush().unwrap();
        let mut line = String::new();
        self.out.read_line(&mut line).unwrap();
        serde_json::from_str(&line).unwrap()
    }

    fn stop(mut self) {
        drop(self.stdin);
        let _ = self.child.wait();
    }
}

fn scratch(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("docxtor-mcp-safe-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn files(dir: &std::path::Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    v.sort();
    v
}

/// The autosave after an edit replaces the file; it does not rewrite it in place (a hard link to the old file still holds the old bytes).
#[cfg(unix)]
#[test]
fn an_autosave_replaces_the_file_and_leaves_no_temporary_file() {
    let dir = scratch("autosave");
    let path = dir.join("notes.docx");
    let p = path.to_str().unwrap();
    let mut s = Server::start();
    let r = s.call("open_document", json!({"path": p, "create": true}));
    assert_eq!(r["result"]["isError"], Value::Null, "{r}");
    let first = std::fs::read(&path).unwrap();
    let link = dir.join("old-copy.docx");
    std::fs::hard_link(&path, &link).unwrap();

    let r = s.call("insert_content", json!({"markdown": "A new paragraph."}));
    assert_eq!(r["result"]["isError"], Value::Null, "{r}");
    let second = std::fs::read(&path).unwrap();
    assert_ne!(first, second, "the edit was saved");
    assert_eq!(std::fs::read(&link).unwrap(), first, "the old file was not rewritten in place");
    assert!(docxtor_core::Document::open(&second).is_ok(), "what was saved opens again");
    assert_eq!(files(&dir), vec!["notes.docx", "old-copy.docx"]);
    s.stop();
}

/// An autosave into a folder that can't be written to is an error the agent sees, and the file on disk stays as it was.
#[cfg(unix)]
#[test]
fn a_save_that_cannot_be_made_is_an_error_and_the_file_is_untouched() {
    use std::os::unix::fs::PermissionsExt;
    let dir = scratch("readonly");
    let path = dir.join("keep.docx");
    let p = path.to_str().unwrap();
    let mut s = Server::start();
    assert_eq!(s.call("open_document", json!({"path": p, "create": true}))["result"]["isError"], Value::Null);
    let original = std::fs::read(&path).unwrap();
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o555)).unwrap();
    let blocked = std::fs::File::create(dir.join("probe")).is_err();
    let r = s.call("insert_content", json!({"markdown": "Lost?"}));
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
    s.stop();
    if !blocked {
        return; // a process that ignores folder permissions (root) can't show this
    }
    assert_eq!(r["result"]["isError"], json!(true), "{r}");
    let text = r["result"]["content"][0]["text"].as_str().unwrap();
    assert!(text.contains("keep.docx") && text.contains("can't write in") && text.contains("the original is unchanged"), "{text}");
    assert_eq!(std::fs::read(&path).unwrap(), original);
    assert_eq!(files(&dir), vec!["keep.docx"]);
}
