//! Docxtor MCP server (stdio). Exposes .docx files to any MCP-capable agent with the
//! same tools the in-app assistant uses.
//!
//! Add it to an MCP client, for example:  claude mcp add docxtor -- /path/to/docxtor-mcp
//! Set DOCXTOR_MCP_AUTOSAVE=0 to require explicit `save_document` calls.

use docxtor_core::office_crypt::{self, CryptError, CryptSession};
use docxtor_core::{ai, safe_write, Document};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{BufRead, Write};

struct Server {
    docs: HashMap<String, Document>,
    /// Documents opened with a password (or created with one), saved protected again. Kept in memory only.
    sessions: HashMap<String, CryptSession>,
    active: Option<String>,
    autosave: bool,
}

fn path_prop() -> Value {
    json!({"type": "string", "description": "Path of an opened .docx (defaults to the active document)"})
}

impl Server {
    fn tools(&self) -> Vec<Value> {
        let mut tools = vec![
            json!({
                "name": "open_document",
                "description": "Open a .docx file and make it the active document. Returns a summary and outline. Use `create: true` to start a new empty document at that path. A password-protected file needs `password` (or the DOCXTOR_PASSWORD environment variable); it is saved protected again.",
                "inputSchema": {"type": "object", "required": ["path"], "properties": {
                    "path": {"type": "string"}, "create": {"type": "boolean"},
                    "password": {"type": "string", "description": "Password of a protected document"}
                }}
            }),
            json!({
                "name": "save_document",
                "description": if self.autosave { "Save the document (edits are also auto-saved). Use save_as to write a copy." } else { "Save the document. Use save_as to write a copy." },
                "inputSchema": {"type": "object", "properties": {"path": path_prop(), "save_as": {"type": "string"}}}
            }),
            json!({
                "name": "undo",
                "description": "Undo the last edit to the document.",
                "inputSchema": {"type": "object", "properties": {"path": path_prop()}}
            }),
            json!({
                "name": "list_open_documents",
                "description": "List documents opened in this session.",
                "inputSchema": {"type": "object", "properties": {}}
            }),
        ];
        if let Value::Array(defs) = ai::tool_definitions() {
            for mut d in defs {
                let mut schema = d["input_schema"].take();
                if let Some(props) = schema.get_mut("properties").and_then(|p| p.as_object_mut()) {
                    props.insert("path".into(), path_prop());
                }
                tools.push(json!({"name": d["name"], "description": d["description"], "inputSchema": schema}));
            }
        }
        tools
    }

    fn resolve(&self, args: &Value) -> Result<String, String> {
        match args.get("path").and_then(|p| p.as_str()) {
            Some(p) => {
                let abs = absolute(p);
                if self.docs.contains_key(&abs) {
                    Ok(abs)
                } else {
                    Err(format!("{p} is not open; call open_document first"))
                }
            }
            None => self.active.clone().ok_or_else(|| "no document is open; call open_document first".to_string()),
        }
    }

    fn save(&mut self, path: &str, target: Option<&str>) -> Result<String, String> {
        let doc = self.docs.get_mut(path).ok_or("document not open")?;
        let bytes = doc.save_stamped(&docxtor_core::docprops::now_iso()).map_err(|e| e.to_string())?;
        let out = target.map(absolute).unwrap_or_else(|| path.to_string());
        // Written to a temporary file, read back and opened again, then renamed over the target: a crash or a full disk
        // never leaves a short file where a good document was (see docxtor_core::safe_write). Autosave goes through here.
        match self.sessions.get_mut(path) {
            Some(session) => {
                safe_write::check_docx(&bytes).map_err(|e| format!("{out}: the new file did not pass its check: {e}; the original is unchanged"))?;
                office_crypt::try_os_random(&mut [0u8; 1])?;
                let enc = session.encrypt(&bytes, &mut |b: &mut [u8]| office_crypt::os_random(b));
                safe_write::write_protected_docx(&out, &enc)?;
            }
            None => safe_write::write_docx(&out, &bytes)?,
        }
        Ok(out)
    }

    fn call(&mut self, name: &str, args: &Value) -> Result<String, String> {
        match name {
            "open_document" => {
                let p = absolute(args.get("path").and_then(|v| v.as_str()).ok_or("path is required")?);
                let create = args.get("create").and_then(|v| v.as_bool()).unwrap_or(false);
                let given = args.get("password").and_then(|v| v.as_str()).map(String::from).or_else(|| std::env::var("DOCXTOR_PASSWORD").ok().filter(|v| !v.is_empty()));
                let mut session = None;
                let mut doc = if create && !std::path::Path::new(&p).exists() {
                    let mut d = Document::new_blank();
                    let bytes = d.save_stamped(&docxtor_core::docprops::now_iso()).map_err(|e| e.to_string())?;
                    // A new file made with a password is protected from its first write, and stays so on every save.
                    let bytes = match given.as_deref().map(CryptSession::new) {
                        Some(mut s) => {
                            office_crypt::try_os_random(&mut [0u8; 1])?;
                            let enc = s.encrypt(&bytes, &mut |b: &mut [u8]| office_crypt::os_random(b));
                            session = Some(s);
                            enc
                        }
                        None => bytes,
                    };
                    if session.is_some() {
                        safe_write::write_protected_docx(&p, &bytes)?;
                    } else {
                        safe_write::write_docx(&p, &bytes)?;
                    }
                    d
                } else {
                    let mut bytes = std::fs::read(&p).map_err(|e| format!("{p}: {e}"))?;
                    if office_crypt::is_encrypted(&bytes) {
                        let pw = given.as_deref().ok_or_else(|| format!("{p} is protected with a password; open it again with the `password` argument"))?;
                        let (plain, s) = office_crypt::open(&bytes, pw).map_err(|e| match e {
                            CryptError::WrongPassword => format!("{p}: that password isn't right"),
                            e => format!("{p}: {e}"),
                        })?;
                        bytes = plain;
                        session = Some(s);
                    }
                    Document::open(&bytes).map_err(|e| format!("{p}: {e}"))?
                };
                let msg = format!("Opened {p}\n{}\nOutline:\n{}", ai::summary(&mut doc), ai::outline(&mut doc));
                self.docs.insert(p.clone(), doc);
                match session {
                    Some(s) => self.sessions.insert(p.clone(), s),
                    None => self.sessions.remove(&p),
                };
                self.active = Some(p);
                Ok(msg)
            }
            "list_open_documents" => Ok(self
                .docs
                .keys()
                .map(|k| if Some(k) == self.active.as_ref() { format!("{k} (active)") } else { k.clone() })
                .collect::<Vec<_>>()
                .join("\n")),
            "save_document" => {
                let p = self.resolve(args)?;
                let out = self.save(&p, args.get("save_as").and_then(|v| v.as_str()))?;
                Ok(format!("Saved {out}"))
            }
            "undo" => {
                let p = self.resolve(args)?;
                let ok = self.docs.get_mut(&p).unwrap().undo();
                if ok && self.autosave {
                    self.save(&p, None)?;
                }
                Ok(if ok { "Undone.".into() } else { "Nothing to undo.".into() })
            }
            tool => {
                let p = self.resolve(args)?;
                let doc = self.docs.get_mut(&p).unwrap();
                let before = doc.checkpoint();
                let out = ai::call_tool(doc, tool, args)?;
                if self.autosave && doc.checkpoint() != before {
                    self.save(&p, None)?;
                }
                Ok(out)
            }
        }
    }
}

fn absolute(p: &str) -> String {
    let path = std::path::Path::new(p);
    if path.is_absolute() {
        p.to_string()
    } else {
        std::env::current_dir().map(|d| d.join(path).to_string_lossy().into_owned()).unwrap_or_else(|_| p.to_string())
    }
}

fn main() {
    let mut server = Server {
        docs: HashMap::new(),
        sessions: HashMap::new(),
        active: None,
        autosave: std::env::var("DOCXTOR_MCP_AUTOSAVE").map(|v| v != "0").unwrap_or(true),
    };
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let msg: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(e) => {
                let err = json!({"jsonrpc": "2.0", "id": null, "error": {"code": -32700, "message": e.to_string()}});
                let _ = writeln!(stdout, "{err}");
                let _ = stdout.flush();
                continue;
            }
        };
        let Some(id) = msg.get("id").cloned() else { continue }; // notifications need no reply
        let method = msg.get("method").and_then(|m| m.as_str()).unwrap_or("");
        let params = msg.get("params").cloned().unwrap_or(json!({}));
        let result: Result<Value, (i64, String)> = match method {
            "initialize" => Ok(json!({
                "protocolVersion": params.get("protocolVersion").cloned().unwrap_or(json!("2025-06-18")),
                "capabilities": {"tools": {"listChanged": false}},
                "serverInfo": {"name": "docxtor", "version": env!("CARGO_PKG_VERSION")},
                "instructions": ai::SYSTEM_GUIDE,
            })),
            "ping" => Ok(json!({})),
            "tools/list" => Ok(json!({"tools": server.tools()})),
            "tools/call" => {
                let name = params.get("name").and_then(|n| n.as_str()).unwrap_or("");
                let args = params.get("arguments").cloned().unwrap_or(json!({}));
                match server.call(name, &args) {
                    Ok(text) => Ok(json!({"content": [{"type": "text", "text": text}]})),
                    Err(e) => Ok(json!({"content": [{"type": "text", "text": e}], "isError": true})),
                }
            }
            other => Err((-32601, format!("method not found: {other}"))),
        };
        let resp = match result {
            Ok(r) => json!({"jsonrpc": "2.0", "id": id, "result": r}),
            Err((code, message)) => json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}}),
        };
        let _ = writeln!(stdout, "{resp}");
        let _ = stdout.flush();
    }
}
