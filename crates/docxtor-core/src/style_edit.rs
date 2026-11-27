//! Editing styles: the definition the Modify style dialog writes, the details and looks the style gallery and the
//! Styles panel read, and the helpers that keep a `w:style` element in Word's order.

use crate::props::{put_ordered, ParaFmt, RunFmt};
use crate::xml::{Element, Node};
use crate::{props::ParaPatch, props::RunPatch};
use serde::{Deserialize, Serialize};

/// The order of a `w:style`'s children (ECMA-376 CT_Style).
pub const STYLE_ORDER: &[&str] = &[
    "w:name", "w:aliases", "w:basedOn", "w:next", "w:link", "w:autoRedefine", "w:hidden", "w:uiPriority", "w:semiHidden", "w:unhideWhenUsed",
    "w:qFormat", "w:locked", "w:personal", "w:personalCompose", "w:personalReply", "w:rsid", "w:pPr", "w:rPr", "w:tblPr", "w:trPr", "w:tcPr", "w:tblStylePr",
];

/// A style's definition as the Modify style dialog edits it. A field left out stays as it is (a new style gets
/// Word's defaults). Formatting is given as patches, like the formatting of text: only what is set is written.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StyleDefinition {
    /// The name people see. Only custom styles can be renamed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// For a new style: `paragraph`, `character`, or `linked` (a paragraph style with a character style of the same look).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    /// The style it builds on (id or name); an empty string builds on none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub based_on: Option<String>,
    /// The style of the paragraph after this one (id or name); an empty string, or the style itself, writes none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next: Option<String>,
    /// Show in the style gallery (`w:qFormat`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quick: Option<bool>,
    /// Update the style when text with it is formatted by hand (`w:autoRedefine`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_update: Option<bool>,
    /// Hide the style from the gallery and the lists (`w:semiHidden`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hidden: Option<bool>,
    /// Where the style sorts in the recommended order (`w:uiPriority`); lower comes first.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub priority: Option<i64>,
    /// Paragraph formatting (`w:pPr`), as for text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ppr: Option<ParaPatch>,
    /// Character formatting (`w:rPr`), as for text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rpr: Option<RunPatch>,
    /// The language of the style's text (BCP 47, `en-US`); an empty string removes it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lang: Option<String>,
}

/// Everything the Modify style dialog and the Styles panel show about one style.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StyleDetails {
    pub id: String,
    pub name: String,
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub based_on: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next: Option<String>,
    /// The linked style of a linked pair.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub link: Option<String>,
    pub quick: bool,
    pub auto_update: bool,
    pub hidden: bool,
    pub priority: i64,
    /// Made by a person (not one of Word's built-in styles).
    pub custom: bool,
    /// Word's built-in style that this document does not define yet; changing it writes Word's definition first.
    pub latent: bool,
    /// The formatting the style sets itself.
    pub own_para: ParaFmt,
    pub own_run: RunFmt,
    /// What its text looks like: document defaults, the styles it builds on and the style itself, with fonts and
    /// colours turned into names and hex.
    pub para: ParaFmt,
    pub run: RunFmt,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lang: Option<String>,
    /// How many paragraphs (or runs, for a character style) use it in the main text.
    pub uses: usize,
}

/// One tile of the style gallery or one row of the Styles panel: a style and how it looks.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StyleLook {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub quick: bool,
    pub hidden: bool,
    pub priority: i64,
    pub custom: bool,
    /// Not in the file yet; choosing it adds Word's definition.
    pub latent: bool,
    /// The style it is linked to: a paragraph style and its character style apply as one in Word.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub link: Option<String>,
    pub uses: usize,
    pub run: RunFmt,
    pub para: ParaFmt,
}

/// A style set drawn in a document's theme: the looks of its title, headings and body text.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StyleSetPreview {
    pub id: String,
    pub name: String,
    pub description: String,
    pub samples: Vec<StyleLook>,
}

/// A place a style is used: a paragraph and the part of its text, as UTF-16 offsets.
#[derive(Debug, Clone, Serialize)]
pub struct StyleUse {
    pub p: u32,
    pub from: usize,
    pub to: usize,
}

/// The name Word's lists show for a style: its built-in names are written in lower case in the file (`heading 1`,
/// `footnote text`, `toc 1`) and shown with capitals (`Heading 1`, `Footnote Text`, `TOC 1`). A name a person gave stays.
pub fn display_name(name: &str, custom: bool) -> String {
    if custom {
        return name.to_string();
    }
    name.split(' ')
        .map(|w| match w {
            "toc" => "TOC".to_string(),
            "" => String::new(),
            _ => {
                let mut c = w.chars();
                c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// A style id made from a name the way Word does: letters and digits only, a number after it if it is taken.
pub fn style_id_for(name: &str, taken: &dyn Fn(&str) -> bool) -> String {
    let mut base: String = name.chars().filter(|c| c.is_alphanumeric()).collect();
    if base.is_empty() {
        base = "Style".into();
    }
    if base.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        base.insert(0, 'S');
    }
    let mut id = base.clone();
    let mut n = 1;
    while taken(&id) {
        n += 1;
        id = format!("{base}{n}");
    }
    id
}

/// Put a flag element (`w:qFormat`) in a style, or take it out.
pub fn set_flag(style: &mut Element, name: &'static str, on: bool) {
    if on {
        if style.child(name).is_none() {
            put_ordered(style, Element::new(name), STYLE_ORDER);
        }
    } else {
        style.remove_children(name);
    }
}

/// Set an element that carries a `w:val` (`w:basedOn`), or take it out with `None`.
pub fn set_val(style: &mut Element, name: &'static str, val: Option<&str>) {
    match val {
        Some(v) => put_ordered(style, Element::new(name).with_attr("w:val", v), STYLE_ORDER),
        None => {
            style.remove_children(name);
        }
    }
}

/// The `w:pPr` or `w:rPr` of a style, created in its place when missing.
pub fn props_mut<'a>(style: &'a mut Element, name: &'static str) -> &'a mut Element {
    if style.child(name).is_none() {
        put_ordered(style, Element::new(name), STYLE_ORDER);
    }
    style.child_mut(name).expect("just added")
}

/// Take an empty `w:pPr` or `w:rPr` out of a style.
pub fn drop_if_empty(style: &mut Element, name: &str) {
    if style.child(name).is_some_and(|e| e.children.is_empty()) {
        style.remove_children(name);
    }
}

/// The attribute `w:val` of a child element.
pub fn val<'a>(style: &'a Element, name: &str) -> Option<&'a str> {
    style.child(name).and_then(|e| e.attr("w:val"))
}

/// Whether two property elements say the same (name, attributes and children).
pub fn same_prop(a: &Element, b: &Element) -> bool {
    a == b
}

/// The property elements of a `w:pPr` or `w:rPr` that a style may carry, left out of what a selection gives a new style:
/// the style reference, section properties, tracked changes and the mark's own run properties.
pub fn is_structural(e: &Element) -> bool {
    matches!(e.name.as_str(), "w:pStyle" | "w:rStyle" | "w:sectPr" | "w:pPrChange" | "w:rPrChange" | "w:rPr" | "w:numPr" | "w:framePr")
}

/// The children of `props` that are formatting, in order.
pub fn formatting_of(props: &Element) -> Vec<Element> {
    props.children.iter().filter_map(|n| if let Node::Element(e) = n { Some(e) } else { None }).filter(|e| !is_structural(e)).cloned().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_follow_names_and_avoid_clashes() {
        let taken = |id: &str| id == "MyStyle";
        assert_eq!(style_id_for("My style", &|_| false), "Mystyle");
        assert_eq!(style_id_for("MyStyle", &taken), "MyStyle2");
        assert_eq!(style_id_for("1st level", &|_| false), "S1stlevel");
        assert_eq!(style_id_for("---", &|_| false), "Style");
    }

    #[test]
    fn built_in_names_are_shown_with_capitals() {
        assert_eq!(display_name("heading 1", false), "Heading 1");
        assert_eq!(display_name("footnote text", false), "Footnote Text");
        assert_eq!(display_name("toc 2", false), "TOC 2");
        assert_eq!(display_name("Title", false), "Title");
        assert_eq!(display_name("my style", true), "my style");
    }

    #[test]
    fn style_children_go_in_schema_order() {
        let mut s = Element::new("w:style").with_child(Element::new("w:name").with_attr("w:val", "X")).with_child(Element::new("w:rPr"));
        set_val(&mut s, "w:basedOn", Some("Normal"));
        set_flag(&mut s, "w:qFormat", true);
        props_mut(&mut s, "w:pPr");
        let names: Vec<_> = s.elements().map(|e| e.name.as_str()).collect();
        assert_eq!(names, ["w:name", "w:basedOn", "w:qFormat", "w:pPr", "w:rPr"]);
        set_flag(&mut s, "w:qFormat", false);
        drop_if_empty(&mut s, "w:pPr");
        assert_eq!(s.elements().count(), 3);
    }
}
