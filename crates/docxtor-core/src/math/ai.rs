//! AI tools for equations. They live apart from `ai` and are merged into its tool list and executor.

use crate::ai::{find_in_para, id_of, script_for_paragraphs};
use crate::document::Document;
use crate::ops::{Op, Pos};
use serde_json::{json, Value};

const LATEX_HELP: &str = "LaTeX: \\frac{a}{b} (\\nicefrac{a}{b} for a/b, \\sfrac for a skewed one, \\genfrac{}{}{0pt}{}{a}{b} for no bar), x^2, x_i, \\sqrt{x}, \\sqrt[3]{x}, \\sum_{i=1}^{n}, \\prod, \\int_a^b, \\iint, \\oint, \\lim_{x \\to 0}, \\left( ... \\right), \\begin{pmatrix} a & b \\\\ c & d \\end{pmatrix} (also matrix, bmatrix, vmatrix, cases, aligned), Greek letters (\\alpha, \\Omega), \\cdot \\times \\div \\pm \\leq \\geq \\neq \\approx \\equiv \\infty \\partial \\nabla \\to \\Rightarrow \\in \\subset \\cup \\cap \\forall \\exists \\ldots \\cdots, \\sin \\cos \\tan \\log \\ln \\exp \\max \\min, \\hat \\bar \\vec \\dot \\tilde, \\text{words}, \\mathbf{v}. Write it without surrounding dollar signs.";

pub fn definitions() -> Vec<Value> {
    vec![
        json!({
            "name": "insert_equation",
            "description": format!("Insert an equation (a real Word equation, not text) into a paragraph right after some text, or at the end of the paragraph when `after_text` is omitted. With `display` it goes on its own centred line (the paragraph is split there); otherwise it sits in the text. It appears in read_document as $latex$ or $$latex$$. {LATEX_HELP}"),
            "input_schema": {"type": "object", "required": ["id", "latex"], "properties": {
                "id": {"type": "string"},
                "after_text": {"type": "string", "description": "Exact text the equation follows (first occurrence); default is the end of the paragraph"},
                "latex": {"type": "string", "description": "The equation in LaTeX, without dollar signs"},
                "display": {"type": "boolean", "description": "Put it on its own line (default false)"}
            }}
        }),
        json!({
            "name": "set_equation",
            "description": "Replace an equation that is already in a paragraph with new LaTeX (the Word equation is kept as an equation). `number` says which one when the paragraph has several (1 is the first). `display` switches between a line of its own and inline; leave it out to keep how it is shown.",
            "input_schema": {"type": "object", "required": ["id", "latex"], "properties": {
                "id": {"type": "string"},
                "latex": {"type": "string", "description": "The new equation in LaTeX, without dollar signs"},
                "number": {"type": "integer", "description": "Which equation in the paragraph, counting from 1 (default 1)"},
                "display": {"type": "boolean"}
            }}
        }),
    ]
}

pub fn call(doc: &mut Document, name: &str, args: &Value) -> Option<Result<String, String>> {
    Some(match name {
        "insert_equation" => insert_equation(doc, args),
        "set_equation" => set_equation(doc, args),
        _ => return None,
    })
}

fn s(args: &Value, k: &str) -> Option<String> {
    args.get(k).and_then(|v| v.as_str()).map(String::from)
}

fn insert_equation(doc: &mut Document, args: &Value) -> Result<String, String> {
    let id = id_of(args.get("id").ok_or("id is required")?)?;
    let latex = s(args, "latex").ok_or("latex is required")?;
    let at = match s(args, "after_text") {
        Some(t) if !t.is_empty() => find_in_para(doc, id, &t, false)?[0].1,
        _ => doc.find_any_para(id).ok_or(format!("paragraph p{id} not found"))?.len16(),
    };
    let display = args.get("display").and_then(|v| v.as_bool()).unwrap_or(false);
    let r = doc.apply_batch(&[Op::InsertEquation { at: Pos::new(id, at), latex, display }], false).map_err(|e| e.to_string())?;
    let mut ids = vec![id];
    ids.extend(r[0].created.iter().copied());
    Ok(format!("Equation inserted:\n{}", script_for_paragraphs(doc, &ids)))
}

fn set_equation(doc: &mut Document, args: &Value) -> Result<String, String> {
    let id = id_of(args.get("id").ok_or("id is required")?)?;
    let latex = s(args, "latex").ok_or("latex is required")?;
    let number = args.get("number").and_then(|v| v.as_u64()).unwrap_or(1).max(1) as usize;
    let at = {
        let p = doc.find_any_para(id).ok_or(format!("paragraph p{id} not found"))?;
        let offsets: Vec<usize> = p.text().encode_utf16().enumerate().filter(|(o, u)| *u == 0xFFFC && crate::math::at(&p.inlines, *o).is_some()).map(|(o, _)| o).collect();
        *offsets.get(number - 1).ok_or_else(|| format!("p{id} has {} equation(s); there is no number {number}", offsets.len()))?
    };
    let display = args.get("display").and_then(|v| v.as_bool());
    let r = doc.apply_batch(&[Op::SetEquation { at: Pos::new(id, at), latex, display }], false).map_err(|e| e.to_string())?;
    let mut ids = vec![id];
    ids.extend(r[0].created.iter().copied());
    Ok(format!("Equation changed:\n{}", script_for_paragraphs(doc, &ids)))
}
