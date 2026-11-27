//! The AI interface: DocScript (an ID-addressed text view of the document) and a
//! tool set modelled on coding-agent tools (read / search / str-replace / insert).
//!
//! The same tools power the in-app assistant (via WASM), the MCP server and the
//! CLI, so any model or agent sees the document the same way.

// The pieces: the prompt, DocScript, the tool definitions and the handlers that run a call.
mod docscript;
mod guide;
mod handlers;
mod tool_defs;

pub use docscript::{docscript, docscript_head, outline, script_for_paragraphs, selection_context, summary, ScriptOpts};
pub(crate) use docscript::{para_md_as, Flavor};
pub use guide::SYSTEM_GUIDE;
pub use handlers::call_tool;
pub(crate) use handlers::{find_in_para, id_of};
pub use tool_defs::tool_definitions;

#[cfg(test)]
use crate::document::Document;
#[cfg(test)]
use crate::model::*;
#[cfg(test)]
use serde_json::json;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn script_and_tools() {
        let mut doc = Document::new_blank();
        let pid = paragraph_order(&doc.body)[0];
        call_tool(&mut doc, "insert_content", &json!({"before": format!("p{pid}"), "markdown": "# Plan\n\nWe will **ship** it.\n\n- a\n- b"})).unwrap();
        let s = docscript(&mut doc, &ScriptOpts::default());
        assert!(s.contains("Heading1] Plan"), "{s}");
        assert!(s.contains("We will **ship** it."), "{s}");
        assert!(s.contains("list:\"•\""), "{s}");
        let r = call_tool(&mut doc, "replace_text", &json!({"find": "ship", "replace": "launch"})).unwrap();
        assert!(r.contains("**launch**"), "{r}");
        let hit = call_tool(&mut doc, "search", &json!({"query": "launch"})).unwrap();
        let id = hit.split("[p").nth(1).unwrap().split(']').next().unwrap();
        call_tool(&mut doc, "format_text", &json!({"id": format!("p{id}"), "text": "We", "format": {"italic": true}})).unwrap();
        let s = docscript(&mut doc, &ScriptOpts::default());
        assert!(s.contains("*We* will **launch** it."), "{s}");
        assert!(call_tool(&mut doc, "replace_text", &json!({"find": "nope", "replace": "x"})).is_err());
    }

    #[test]
    fn no_tool_description_repeats_a_block() {
        let defs = tool_definitions();
        for t in defs.as_array().unwrap() {
            let name = t["name"].as_str().unwrap_or("?");
            let d: Vec<char> = t["description"].as_str().unwrap_or("").chars().collect();
            let mut seen = std::collections::HashMap::new();
            for i in 0..d.len().saturating_sub(80) {
                let w: String = d[i..i + 80].iter().collect();
                if let Some(j) = seen.insert(w.clone(), i) {
                    panic!("the description of {name} repeats a block: {w:?} at {j} and {i}");
                }
            }
        }
    }
}
