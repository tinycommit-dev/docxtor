//! Equations: Word's math markup (OMML, `m:oMath` and `m:oMathPara`), shown through MathML and edited as LaTeX.
//!
//! * [`omml_to_mathml`] draws an equation (Chrome renders MathML itself).
//! * [`latex_to_omml`] reads the LaTeX people type into Word's markup.
//! * [`omml_to_latex`] gives an existing equation back as LaTeX so it can be edited.
//!
//! An equation is held as the original XML element in the paragraph (so an untouched one is written back byte for
//! byte); in the paragraph's text it counts as one character, like an image.

mod ai;
mod latex;
mod mathml;
mod symbols;
mod tolatex;

pub use ai::{call as call_tool, definitions as tool_definitions};
pub use latex::latex_to_omml;
pub use mathml::omml_to_mathml;
pub use tolatex::omml_to_latex;

use crate::xml::{Element, Node};
use serde::Serialize;

pub const NS_M: &str = "http://schemas.openxmlformats.org/officeDocument/2006/math";

/// Whether an inline element is an equation.
pub fn is_math(e: &Element) -> bool {
    matches!(e.name.as_str(), "m:oMath" | "m:oMathPara")
}

/// What a renderer needs to show and edit an equation.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct MathInfo {
    /// MathML Core markup, one `<math>` element.
    pub mathml: String,
    /// The equation as LaTeX (best effort for markup Word made that LaTeX cannot say).
    pub latex: String,
    /// A display equation (`m:oMathPara`) sits on its own line.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub display: bool,
    /// Justification of a display equation: `center` (Word's default), `left`, `right` or `centerGroup`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub jc: Option<String>,
}

/// LaTeX straight to MathML, for a live preview while it is typed. Errors say what is wrong with the LaTeX.
pub fn latex_to_mathml(latex: &str, display: bool) -> Result<String, String> {
    let omath = latex_to_omml(latex)?;
    Ok(omml_to_mathml(&if display { display_wrap(omath) } else { omath }))
}

pub fn info(e: &Element) -> MathInfo {
    let display = e.name == "m:oMathPara";
    let jc = if display { e.child("m:oMathParaPr").and_then(|p| p.child("m:jc")).and_then(|j| j.attr("m:val")).map(String::from) } else { None };
    MathInfo { mathml: omml_to_mathml(e), latex: omml_to_latex(e), display, jc }
}

/// The text of the equation, for search and plain-text views.
pub fn plain_text(e: &Element) -> String {
    let mut ts = Vec::new();
    e.find_all("m:t", &mut ts);
    ts.iter().map(|t| t.text()).collect()
}

/// A display equation (`m:oMathPara`) around an inline one. Word centres it.
pub fn display_wrap(omath: Element) -> Element {
    Element::new("m:oMathPara").with_child(Element::new("m:oMathParaPr").with_child(val("m:jc", "center"))).with_child(omath)
}

/// The equation occupying `[off, off + 1)` of some inline content.
pub fn at(inlines: &[crate::model::Inline], off: usize) -> Option<&Element> {
    use crate::model::Inline;
    let mut start = 0;
    for inl in inlines {
        let len = inl.len16();
        match inl {
            Inline::Opaque(e) if is_math(e) && start == off => return Some(e),
            Inline::Container(c) if off >= start && off < start + len => return at(&c.children, off - start),
            _ => {}
        }
        start += len;
    }
    None
}

pub fn at_mut(inlines: &mut [crate::model::Inline], off: usize) -> Option<&mut Element> {
    use crate::model::Inline;
    let mut start = 0;
    for inl in inlines.iter_mut() {
        let len = inl.len16();
        match inl {
            Inline::Opaque(e) if is_math(e) && start == off => return Some(e),
            Inline::Container(c) if off >= start && off < start + len => return at_mut(&mut c.children, off - start),
            _ => {}
        }
        start += len;
    }
    None
}

/// The `m:oMath` inside an equation of either kind.
pub fn inner(e: &Element) -> Option<&Element> {
    if e.name == "m:oMath" {
        Some(e)
    } else {
        e.child("m:oMath")
    }
}

/// Replace the formula inside an equation, keeping `m:oMathParaPr` and everything else around it.
pub fn replace_inner(e: &mut Element, new: Element) {
    if e.name == "m:oMath" {
        *e = new;
        return;
    }
    let mut new = Some(new);
    let mut out = Vec::new();
    for n in e.children.drain(..) {
        match n {
            Node::Element(c) if c.name == "m:oMath" => {
                if let Some(x) = new.take() {
                    out.push(Node::Element(x));
                }
            }
            other => out.push(other),
        }
    }
    if let Some(x) = new {
        out.push(Node::Element(x));
    }
    e.children = out;
}

// ---------------------------------------------------------------------------
// Builders shared by the readers
// ---------------------------------------------------------------------------

pub(crate) const MATH_FONT: &str = "Cambria Math";

/// `<w:rPr><w:rFonts w:ascii="Cambria Math" w:hAnsi="Cambria Math"/></w:rPr>`: what Word puts in every math run.
pub(crate) fn font_rpr() -> Element {
    Element::new("w:rPr").with_child(Element::new("w:rFonts").with_attr("w:ascii", MATH_FONT).with_attr("w:hAnsi", MATH_FONT))
}

/// `<m:ctrlPr>` with the math font and italics, which Word writes inside every structure's properties.
pub(crate) fn ctrl_pr() -> Element {
    Element::new("m:ctrlPr").with_child(Element::new("w:rPr").with_child(Element::new("w:rFonts").with_attr("w:ascii", MATH_FONT).with_attr("w:hAnsi", MATH_FONT)).with_child(Element::new("w:i")))
}

pub(crate) fn val(name: &'static str, v: &str) -> Element {
    Element::new(name).with_attr("m:val", v)
}

/// A structure's properties element (`m:fPr`) holding the given children and the control properties.
pub(crate) fn pr(name: &'static str, children: Vec<Element>) -> Element {
    let mut e = Element::new(name);
    for c in children {
        e.children.push(Node::Element(c));
    }
    e.children.push(Node::Element(ctrl_pr()));
    e
}

/// An argument element (`m:e`, `m:num`, `m:sub`, ...) around some content.
pub(crate) fn arg(name: &'static str, content: Vec<Element>) -> Element {
    let mut e = Element::new(name);
    for c in content {
        e.children.push(Node::Element(c));
    }
    e
}

/// Whether an `m:val`-style flag is on: absent value means on, `0`, `false` and `off` mean off.
pub(crate) fn flag(e: &Element) -> bool {
    !matches!(e.attr("m:val"), Some("0" | "false" | "off"))
}

#[cfg(test)]
mod tests;
