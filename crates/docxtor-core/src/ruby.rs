//! Phonetic guides (`w:ruby`): small text above the base text, such as the reading of a kanji. Word keeps one in a
//! run as `w:ruby`, with the guide in `w:rt`, the base in `w:rubyBase` and the settings in `w:rubyPr`. In the
//! editor a guide is one position in the paragraph (like a picture or an equation) that shows its base with the guide
//! above it and is edited in the Phonetic guide box. The XML stays as Word wrote it.

use crate::xml::{Element, Node};
use serde::{Deserialize, Serialize};

/// The alignments of `w:rubyAlign`.
pub const ALIGNS: &[&str] = &["center", "distributeLetter", "distributeSpace", "left", "right", "rightVertical"];

/// A phonetic guide as the editor needs it to draw it and to open the Phonetic guide box.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RRuby {
    /// The text the guide is over.
    pub base: String,
    /// The guide itself.
    pub text: String,
    /// `center`, `distributeLetter`, `distributeSpace`, `left`, `right` or `rightVertical`.
    pub align: String,
    /// The size of the guide in points (`w:hps`); absent when the file gives none (then 0.5 of the base).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<f64>,
    /// The size of the base text the guide was made for, in points (`w:hpsBaseText`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_size: Option<f64>,
    /// How far the guide sits above the base text's baseline, in points (`w:hpsRaise`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raise: Option<f64>,
    /// The language of the guide (`w:lid`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lang: Option<String>,
    /// The guide's font, when its run names one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font: Option<String>,
    /// How the guide is formatted (its font, colour, size), once the document's styles are applied.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rt_f: Option<crate::props::RunFmt>,
}

fn half_points(e: &Element, name: &str) -> Option<f64> {
    e.child(name).and_then(|c| c.attr("w:val")).and_then(|v| v.parse::<f64>().ok()).map(|v| v / 2.0)
}

/// All the text under an element (`w:t`, with `w:tab` as a tab).
pub fn text_of(e: &Element) -> String {
    fn go(e: &Element, out: &mut String) {
        for n in &e.children {
            if let Node::Element(c) = n {
                match c.name.as_str() {
                    "w:t" => out.push_str(&c.text()),
                    "w:tab" => out.push('\t'),
                    "w:rPr" | "w:rubyPr" => {}
                    _ => go(c, out),
                }
            }
        }
    }
    let mut s = String::new();
    go(e, &mut s);
    s
}

/// The first run's `w:rPr` under an element.
pub fn first_rpr(e: &Element) -> Option<&Element> {
    for c in e.elements() {
        if c.name == "w:r" {
            return c.child("w:rPr");
        }
        if let Some(r) = first_rpr(c) {
            return Some(r);
        }
    }
    None
}

pub fn is_ruby(e: &Element) -> bool {
    e.name == "w:ruby"
}

/// Read a `w:ruby` element.
pub fn read(e: &Element) -> RRuby {
    let pr = e.child("w:rubyPr");
    let rt = e.child("w:rt");
    let rpr = rt.and_then(first_rpr);
    let font = rpr.and_then(|r| r.child("w:rFonts")).and_then(|f| f.attr("w:eastAsia").or(f.attr("w:ascii")).or(f.attr("w:hAnsi"))).map(String::from);
    RRuby {
        base: e.child("w:rubyBase").map(text_of).unwrap_or_default(),
        text: rt.map(text_of).unwrap_or_default(),
        align: pr.and_then(|p| p.child("w:rubyAlign")).and_then(|a| a.attr("w:val")).filter(|a| ALIGNS.contains(a)).unwrap_or("center").to_string(),
        size: pr.and_then(|p| half_points(p, "w:hps")).or_else(|| rpr.and_then(|r| half_points(r, "w:sz"))),
        base_size: pr.and_then(|p| half_points(p, "w:hpsBaseText")),
        raise: pr.and_then(|p| half_points(p, "w:hpsRaise")),
        lang: pr.and_then(|p| p.child("w:lid")).and_then(|l| l.attr("w:val")).map(String::from),
        font,
        rt_f: None,
    }
}

/// What the Phonetic guide box sets. `None` leaves a setting as it is; for the sizes, 0 removes it.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RubyProps {
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub align: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_size: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raise: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lang: Option<String>,
    /// The guide's font; an empty string removes it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font: Option<String>,
}

impl RubyProps {
    pub fn validate(&self) -> Result<(), String> {
        if let Some(a) = &self.align {
            if !ALIGNS.contains(&a.as_str()) {
                return Err(format!("A phonetic guide is aligned {}, not '{a}'.", ALIGNS.join(", ")));
            }
        }
        for (name, v) in [("size", self.size), ("base size", self.base_size), ("raise", self.raise)] {
            if v.is_some_and(|v| !(0.0..=1638.0).contains(&v)) {
                return Err(format!("The phonetic guide {name} is between 0 and 1638 points."));
            }
        }
        if self.text.is_empty() {
            return Err("A phonetic guide needs some text. Use clear_ruby to remove one.".into());
        }
        Ok(())
    }
}

fn run_with(rpr: Option<&Element>, text: &str) -> Element {
    crate::model::text_run_element(rpr.cloned(), text)
}

/// The `w:rubyPr` for a set of properties, with its children in schema order.
fn ruby_pr(p: &RubyProps, old: Option<&Element>) -> Element {
    let mut pr = Element::new("w:rubyPr");
    let half = |v: f64| ((v * 2.0).round() as i64).to_string();
    let align = p.align.clone().or_else(|| old.and_then(|o| o.child("w:rubyAlign")).and_then(|a| a.attr("w:val")).map(String::from)).unwrap_or_else(|| "center".into());
    pr.children.push(Node::Element(Element::new("w:rubyAlign").with_attr("w:val", align)));
    let keep = |name: &str| old.and_then(|o| o.child(name)).and_then(|c| c.attr("w:val")).map(String::from);
    // The schema requires all three (found by the Open XML validator). When nothing says otherwise they get what Word writes
    // for its default guide over 10.5 pt text: 5 pt, raised 9 pt.
    for (name, v, default) in [("w:hps", p.size, "10"), ("w:hpsRaise", p.raise, "18"), ("w:hpsBaseText", p.base_size, "21")] {
        let val = match v {
            Some(v) if v > 0.0 => half(v),
            // 0 asks for automatic: Word's default, not the size the guide had before.
            Some(_) => default.to_string(),
            None => keep(name).unwrap_or_else(|| default.to_string()),
        };
        pr.children.push(Node::Element(Element::new(name).with_attr("w:val", val)));
    }
    let lid = p.lang.clone().or_else(|| keep("w:lid")).unwrap_or_else(|| "ja-JP".into());
    pr.children.push(Node::Element(Element::new("w:lid").with_attr("w:val", lid)));
    pr
}

/// The `w:rPr` of the guide's run: the base's formatting at the guide's size, with the guide's font.
fn rt_rpr(base_rpr: Option<&Element>, p: &RubyProps, old_rt: Option<&Element>) -> Option<Element> {
    let mut rpr = old_rt.and_then(first_rpr).cloned().or_else(|| base_rpr.cloned()).unwrap_or_else(|| Element::new("w:rPr"));
    if let Some(font) = &p.font {
        if !font.is_empty() {
            rpr.remove_children("w:rFonts");
            crate::props::put_rpr(&mut rpr, Element::new("w:rFonts").with_attr("w:eastAsia", font).with_attr("w:ascii", font).with_attr("w:hAnsi", font).with_attr("w:hint", "eastAsia"));
        } else if old_rt.is_some() {
            // An empty font takes the guide's own font away; a new guide keeps the font of its text.
            rpr.remove_children("w:rFonts");
        }
    }
    if let Some(sz) = p.size.filter(|s| *s > 0.0) {
        let hp = ((sz * 2.0).round() as i64).to_string();
        rpr.remove_children("w:sz");
        rpr.remove_children("w:szCs");
        crate::props::put_rpr(&mut rpr, Element::new("w:sz").with_attr("w:val", &hp));
        crate::props::put_rpr(&mut rpr, Element::new("w:szCs").with_attr("w:val", hp));
    }
    (!rpr.children.is_empty()).then_some(rpr)
}

/// A new `w:ruby` over `base`, whose run gets `base_rpr`.
pub fn build(base: &str, base_rpr: Option<&Element>, p: &RubyProps) -> Element {
    let mut e = Element::new("w:ruby");
    e.children.push(Node::Element(ruby_pr(p, None)));
    let rt = Element::new("w:rt").with_child(run_with(rt_rpr(base_rpr, p, None).as_ref(), &p.text));
    e.children.push(Node::Element(rt));
    e.children.push(Node::Element(Element::new("w:rubyBase").with_child(run_with(base_rpr, base))));
    e
}

/// Change a `w:ruby`: the guide's text and settings. The base text stays as it is.
pub fn update(e: &mut Element, p: &RubyProps) {
    let old_pr = e.child("w:rubyPr").cloned();
    let old_rt = e.child("w:rt").cloned();
    let base_rpr = e.child("w:rubyBase").and_then(first_rpr).cloned();
    let pr = ruby_pr(p, old_pr.as_ref());
    let rt = Element::new("w:rt").with_child(run_with(rt_rpr(base_rpr.as_ref(), p, old_rt.as_ref()).as_ref(), &p.text));
    e.remove_children("w:rubyPr");
    e.remove_children("w:rt");
    e.children.insert(0, Node::Element(rt));
    e.children.insert(0, Node::Element(pr));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_and_read() {
        let e = build("漢字", None, &RubyProps { text: "かんじ".into(), align: Some("distributeSpace".into()), size: Some(5.0), raise: Some(9.0), base_size: Some(10.5), ..Default::default() });
        let names: Vec<_> = e.elements().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["w:rubyPr", "w:rt", "w:rubyBase"]);
        let r = read(&e);
        assert_eq!((r.base.as_str(), r.text.as_str(), r.align.as_str()), ("漢字", "かんじ", "distributeSpace"));
        assert_eq!((r.size, r.raise, r.base_size), (Some(5.0), Some(9.0), Some(10.5)));
    }

    #[test]
    fn update_keeps_the_base() {
        let mut e = build("漢字", None, &RubyProps { text: "かんじ".into(), size: Some(5.0), ..Default::default() });
        update(&mut e, &RubyProps { text: "カンジ".into(), align: Some("left".into()), ..Default::default() });
        let r = read(&e);
        assert_eq!((r.base.as_str(), r.text.as_str(), r.align.as_str(), r.size), ("漢字", "カンジ", "left", Some(5.0)));
    }
}
