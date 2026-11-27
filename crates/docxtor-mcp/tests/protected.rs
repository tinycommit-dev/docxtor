//! `docxtor-mcp` with a password: a new file made with one is protected from its first write and on every save.

use docxtor_core::office_crypt;
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};

fn call(stdin: &mut impl Write, out: &mut impl BufRead, id: u32, name: &str, args: Value) -> Value {
    let req = json!({"jsonrpc": "2.0", "id": id, "method": "tools/call", "params": {"name": name, "arguments": args}});
    writeln!(stdin, "{req}").unwrap();
    stdin.flush().unwrap();
    let mut line = String::new();
    out.read_line(&mut line).unwrap();
    serde_json::from_str(&line).unwrap()
}

#[test]
fn a_new_file_made_with_a_password_is_protected() {
    let dir = std::env::temp_dir().join(format!("docxtor-mcp-protected-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("new.docx");
    let _ = std::fs::remove_file(&path);
    let p = path.to_str().unwrap();

    let mut child = Command::new(env!("CARGO_BIN_EXE_docxtor-mcp")).env_remove("DOCXTOR_PASSWORD").stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let mut out = BufReader::new(child.stdout.take().unwrap());

    let r = call(&mut stdin, &mut out, 1, "open_document", json!({"path": p, "create": true, "password": "pw one"}));
    assert_eq!(r["result"]["isError"], Value::Null, "{r}");
    let first = std::fs::read(&path).unwrap();
    assert!(office_crypt::is_encrypted(&first), "the file made with a password is plain");
    assert!(office_crypt::decrypt(&first, "pw one").is_ok());

    // Saving keeps it protected, with new key material each time.
    let r = call(&mut stdin, &mut out, 2, "save_document", json!({}));
    assert_eq!(r["result"]["isError"], Value::Null, "{r}");
    let second = std::fs::read(&path).unwrap();
    assert!(office_crypt::is_encrypted(&second));
    assert_ne!(first, second);
    assert!(office_crypt::decrypt(&second, "pw one").is_ok());

    drop(stdin);
    let _ = child.wait();
    let _ = std::fs::remove_dir_all(&dir);
}
