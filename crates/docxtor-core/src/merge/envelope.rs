//! Envelopes. Word adds an envelope to a document as a new first section with the envelope's page size, the return
//! address at the top left and the delivery address lower down and to the right. Word puts the addresses in text frames;
//! Docxtor doesn't lay out frames, so the addresses are ordinary paragraphs, positioned with an indent and the space
//! before the first one, which looks the same in Word and in Docxtor.

use super::labels::{text_paragraph, TextLook};
use crate::xml::{Element, Node};
use serde::{Deserialize, Serialize};

/// An envelope size: Word's list. Sizes are in twips, `w` the long side.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnvelopeSize {
    pub id: String,
    pub name: String,
    pub w: i64,
    pub h: i64,
}

fn inch(v: f64) -> i64 {
    (v * 1440.0).round() as i64
}

fn mm(v: f64) -> i64 {
    (v * 1440.0 / 25.4).round() as i64
}

/// Word's envelope sizes.
pub fn sizes() -> Vec<EnvelopeSize> {
    let s = |id: &str, name: &str, w: i64, h: i64| EnvelopeSize { id: id.into(), name: name.into(), w, h };
    vec![
        s("10", "Size 10 (4 1/8 x 9 1/2 in)", inch(9.5), inch(4.125)),
        s("9", "Size 9 (3 7/8 x 8 7/8 in)", inch(8.875), inch(3.875)),
        s("6-3-4", "Size 6 3/4 (3 5/8 x 6 1/2 in)", inch(6.5), inch(3.625)),
        s("monarch", "Monarch (3 7/8 x 7 1/2 in)", inch(7.5), inch(3.875)),
        s("11", "Size 11 (4 1/2 x 10 3/8 in)", inch(10.375), inch(4.5)),
        s("12", "Size 12 (4 3/4 x 11 in)", inch(11.0), inch(4.75)),
        s("dl", "DL (110 x 220 mm)", mm(220.0), mm(110.0)),
        s("c6", "C6 (114 x 162 mm)", mm(162.0), mm(114.0)),
        s("c65", "C65 (114 x 229 mm)", mm(229.0), mm(114.0)),
        s("c5", "C5 (162 x 229 mm)", mm(229.0), mm(162.0)),
        s("c4", "C4 (229 x 324 mm)", mm(324.0), mm(229.0)),
        s("b6", "B6 (125 x 176 mm)", mm(176.0), mm(125.0)),
        s("b5", "B5 (176 x 250 mm)", mm(250.0), mm(176.0)),
        s("b4", "B4 (250 x 353 mm)", mm(353.0), mm(250.0)),
    ]
}

/// The size with this id, or one made from a width and a height.
pub fn size_of(id: &str, w: Option<i64>, h: Option<i64>) -> Result<EnvelopeSize, String> {
    if let (Some(w), Some(h)) = (w, h) {
        if w < 1440 || h < 1440 || w > 31680 || h > 31680 {
            return Err("An envelope is between 1 and 22 inches on a side.".into());
        }
        let (w, h) = (w.max(h), w.min(h));
        return Ok(EnvelopeSize { id: "custom".into(), name: "Custom".into(), w, h });
    }
    sizes().into_iter().find(|s| s.id == id).ok_or_else(|| format!("unknown envelope size '{id}'"))
}

/// Where the text goes and what it looks like.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct EnvelopeLayout {
    /// The delivery address, lines separated by `\n`.
    pub delivery: String,
    /// The return address.
    #[serde(default, rename = "return")]
    pub return_address: String,
    #[serde(default)]
    pub delivery_look: TextLook,
    #[serde(default)]
    pub return_look: TextLook,
    /// Where the delivery address starts, in twips from the left and top edges of the envelope (default: where Word puts it).
    #[serde(default)]
    pub delivery_x: Option<i64>,
    #[serde(default)]
    pub delivery_y: Option<i64>,
    /// Where the return address starts (default: the corner, a quarter inch in).
    #[serde(default)]
    pub return_x: Option<i64>,
    #[serde(default)]
    pub return_y: Option<i64>,
}

/// The margin of an envelope page.
pub const MARGIN: i64 = 360;

/// The section properties of an envelope page.
pub fn section(size: &EnvelopeSize) -> Element {
    Element::new("w:sectPr")
        .with_child(Element::new("w:pgSz").with_attr("w:w", size.w.to_string()).with_attr("w:h", size.h.to_string()).with_attr("w:orient", "landscape"))
        .with_child(
            Element::new("w:pgMar")
                .with_attr("w:top", MARGIN.to_string())
                .with_attr("w:right", MARGIN.to_string())
                .with_attr("w:bottom", MARGIN.to_string())
                .with_attr("w:left", MARGIN.to_string())
                .with_attr("w:header", "0")
                .with_attr("w:footer", "0")
                .with_attr("w:gutter", "0"),
        )
        .with_child(Element::new("w:cols").with_attr("w:space", "720"))
}

/// Whether a section's page is one of the envelope sizes (so adding an envelope again replaces it).
pub fn is_envelope_page(w: i64, h: i64) -> bool {
    let (long, short) = (w.max(h), w.min(h));
    sizes().iter().any(|s| (s.w - long).abs() <= 20 && (s.h - short).abs() <= 20)
}

/// Whether section properties are the ones `section` writes: an envelope size, landscape, with Docxtor's margins.
/// A B5 or B4 report section has the size but not the rest.
pub fn is_envelope_section(sect: &Element) -> bool {
    let Some(z) = sect.child("w:pgSz") else { return false };
    let num = |e: &Element, a: &str| e.attr(a).and_then(|v| v.parse::<i64>().ok());
    let size_ok = is_envelope_page(num(z, "w:w").unwrap_or(0), num(z, "w:h").unwrap_or(0));
    let margins_ok = sect.child("w:pgMar").is_some_and(|m| ["w:top", "w:right", "w:bottom", "w:left"].iter().all(|a| num(m, a) == Some(MARGIN)));
    size_ok && z.attr("w:orient") == Some("landscape") && margins_ok
}

fn size_pt(look: &TextLook, default: f64) -> f64 {
    look.size_pt.filter(|s| *s > 0.0).unwrap_or(default)
}

/// The paragraphs of an envelope page, the last one carrying the section break, and the index of the first delivery
/// paragraph. With `merge` the delivery address is one empty paragraph, for the caller to put an address block in.
pub fn paragraphs(size: &EnvelopeSize, l: &EnvelopeLayout, merge: bool) -> (Vec<Element>, usize) {
    let mut out = Vec::new();
    let rsize = size_pt(&l.return_look, 10.0);
    let dsize = size_pt(&l.delivery_look, 12.0);
    let rline = (rsize * 1.2 * 20.0).round() as i64;
    let dline = (dsize * 1.2 * 20.0).round() as i64;
    let rx = l.return_x.unwrap_or(MARGIN).max(MARGIN);
    let ry = l.return_y.unwrap_or(MARGIN).max(MARGIN);
    let ret_lines: Vec<&str> = l.return_address.lines().collect();
    let mut rl = l.return_look.clone();
    rl.size_pt = Some(rsize);
    for (k, line) in ret_lines.iter().enumerate() {
        out.push(text_paragraph(line, &rl, Some(rx - MARGIN), (k == 0).then_some(ry - MARGIN), Some(rline)));
    }
    let delivery_at = out.len();
    let dx = l.delivery_x.unwrap_or((size.w as f64 * 0.44).round() as i64).max(MARGIN);
    let dy = l.delivery_y.unwrap_or((size.h as f64 * 0.46).round() as i64).max(MARGIN);
    // The delivery address starts `dy` from the top: below the return address's lines and the space before them.
    let used = if ret_lines.is_empty() { MARGIN } else { ry + ret_lines.len() as i64 * rline };
    let gap = (dy - used).max(0);
    let mut dl = l.delivery_look.clone();
    dl.size_pt = Some(dsize);
    let lines: Vec<&str> = if merge { Vec::new() } else { l.delivery.lines().collect() };
    if lines.is_empty() {
        out.push(text_paragraph("", &dl, Some(dx - MARGIN), Some(gap), Some(dline)));
    }
    for (k, line) in lines.iter().enumerate() {
        out.push(text_paragraph(line, &dl, Some(dx - MARGIN), (k == 0).then_some(gap), Some(dline)));
    }
    // The last paragraph ends the section.
    if let Some(last) = out.last_mut() {
        if let Some(Node::Element(ppr)) = last.children.iter_mut().find(|n| matches!(n, Node::Element(e) if e.name == "w:pPr")) {
            ppr.children.push(Node::Element(section(size)));
        }
    }
    (out, delivery_at)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_are_words() {
        let ten = size_of("10", None, None).unwrap();
        assert_eq!((ten.w, ten.h), (13680, 5940));
        let dl = size_of("dl", None, None).unwrap();
        assert_eq!((dl.w, dl.h), (12472, 6236));
        assert!(size_of("zz", None, None).is_err());
        // Whatever order a custom size is given in, the long side is the width.
        let c = size_of("", Some(3000), Some(6000)).unwrap();
        assert_eq!((c.w, c.h), (6000, 3000));
        assert!(is_envelope_page(5940, 13680) && !is_envelope_page(12240, 15840));
        assert!(size_of("", Some(100), Some(6000)).is_err());
    }

    #[test]
    fn the_addresses_sit_where_word_puts_them() {
        let ten = size_of("10", None, None).unwrap();
        let (ps, at) = paragraphs(&ten, &EnvelopeLayout { delivery: "Ada Lovelace\n12 Crumb Rd\nLondon".into(), return_address: "Grace Hopper\n1 Navy Way".into(), ..Default::default() }, false);
        assert_eq!((ps.len(), at), (5, 2));
        // The last paragraph closes the envelope section with the envelope's page.
        let sect = ps[4].child("w:pPr").unwrap().child("w:sectPr").unwrap();
        assert_eq!(sect.child("w:pgSz").unwrap().attr("w:w"), Some("13680"));
        assert_eq!(sect.child("w:pgSz").unwrap().attr("w:orient"), Some("landscape"));
        assert!(ps[3].child("w:pPr").unwrap().child("w:sectPr").is_none());
        // The delivery address is indented to 44% of the width, less the margin, and starts 46% of the way down.
        let ind = ps[2].child("w:pPr").unwrap().child("w:ind").unwrap();
        assert_eq!(ind.attr("w:left"), Some((((13680.0f64 * 0.44).round() as i64) - 360).to_string().as_str()));
        let before: i64 = ps[2].child("w:pPr").unwrap().child("w:spacing").unwrap().attr("w:before").unwrap().parse().unwrap();
        let rline = (10.0f64 * 1.2 * 20.0).round() as i64;
        assert_eq!(360 + 2 * rline + before, (5940.0f64 * 0.46).round() as i64);
    }
}
