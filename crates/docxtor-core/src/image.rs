//! Image layout: size, alt text and wrapping for DrawingML pictures, including
//! conversion between inline (`wp:inline`) and floating (`wp:anchor`) placement.

use crate::xml::{Element, Node};
use serde::{Deserialize, Serialize};

const EMU_PER_PT: f64 = 12700.0;

/// How an image sits relative to the text.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageLayout {
    /// `inline`, `square`, `tight`, `topBottom`, `behind` or `front`.
    pub wrap: String,
    /// `left`, `center` or `right` for floating images positioned by alignment.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub align: Option<String>,
    /// Offsets in points for images positioned by offset.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub x: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub y: Option<f64>,
    /// What the horizontal position is relative to (`column`, `page`, `margin`, `character`, `leftMargin`,
    /// `rightMargin`, `insideMargin`, `outsideMargin`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rel_x: Option<String>,
    /// What the vertical position is relative to (`paragraph`, `page`, `margin`, `line`, `topMargin`,
    /// `bottomMargin`, `insideMargin`, `outsideMargin`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rel_y: Option<String>,
    /// `top`, `center` or `bottom` for floating objects positioned vertically by alignment.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub align_y: Option<String>,
    /// Stacking order of a floating object (`relativeHeight`): the higher, the nearer the viewer. Objects behind
    /// the text and objects in front of it are two stacks, each ordered by this.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub z: Option<u64>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ImagePatch {
    /// Size in points.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wrap: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub align: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alt: Option<String>,
    // Picture formatting (see `picture`). Fractions are of the whole picture, angles are degrees.
    /// Crop as left, top, right and bottom fractions; negative values pad. The size follows unless `width` or `height` is given.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crop: Option<[f64; 4]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rot: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flip_h: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flip_v: Option<bool>,
    /// The picture's shape: `rect`, `roundRect`, `ellipse`, `triangle`, ...
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub geometry: Option<String>,
    /// The corner of a `roundRect`, as a fraction of the shorter side (0 to 0.5).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub corner: Option<f64>,
    /// `"none"`, or `{color, width_pt, dash}`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub border: Option<crate::picture::Switch<crate::picture::PicLine>>,
    /// 0 (invisible) to 1.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opacity: Option<f64>,
    /// -1 to 1.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub brightness: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contrast: Option<f64>,
    /// `"none"`, or `{kind: grayscale | blackWhite | sepia | duotone, threshold?, color1?, color2?}`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recolor: Option<crate::picture::Switch<crate::picture::Recolor>>,
    /// `"none"`, or `{from, to?}`: one colour made transparent (no `to`) or turned into another.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color_change: Option<crate::picture::Switch<crate::picture::ColorChange>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shadow: Option<crate::picture::Switch<crate::picture::Shadow>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub glow: Option<crate::picture::Switch<crate::picture::Glow>>,
    /// Soft edge radius in points; 0 removes it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub soft_edge: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reflection: Option<crate::picture::Switch<crate::picture::Reflection>>,
    /// Remove all adjustments, effects, the outline, the shape and the crop (Word's Reset Picture).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reset: Option<bool>,
    // Placement of a floating picture (see `place_extras`).
    /// Offsets in points from what `rel_x` / `rel_y` count from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub x: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub y: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rel_x: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rel_y: Option<String>,
    /// `top`, `center`, `bottom`, `inside` or `outside`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub align_y: Option<String>,
    /// Distance of the text from the picture: top, bottom, left, right, in points.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dist: Option<[f64; 4]>,
    /// Which sides text goes on for square, tight and through wrapping: `bothSides`, `left`, `right` or `largest`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wrap_text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lock_anchor: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allow_overlap: Option<bool>,
}

impl ImagePatch {
    /// The patch changes how the picture itself looks (not its size, wrapping or place).
    pub fn touches_picture(&self) -> bool {
        self.crop.is_some()
            || self.rot.is_some()
            || self.flip_h.is_some()
            || self.flip_v.is_some()
            || self.geometry.is_some()
            || self.corner.is_some()
            || self.border.is_some()
            || self.opacity.is_some()
            || self.brightness.is_some()
            || self.contrast.is_some()
            || self.recolor.is_some()
            || self.color_change.is_some()
            || self.shadow.is_some()
            || self.glow.is_some()
            || self.soft_edge.is_some()
            || self.reflection.is_some()
            || self.reset == Some(true)
    }

    /// The patch changes where or how a floating picture sits, beyond the wrap and alignment.
    fn touches_place(&self) -> bool {
        self.x.is_some() || self.y.is_some() || self.rel_x.is_some() || self.rel_y.is_some() || self.align_y.is_some() || self.dist.is_some() || self.wrap_text.is_some() || self.lock_anchor.is_some() || self.allow_overlap.is_some()
    }
}

fn emu_pt(v: Option<&str>) -> Option<f64> {
    v.and_then(|s| s.parse::<f64>().ok()).map(|e| e / EMU_PER_PT)
}

/// The `wp:inline` or `wp:anchor` inside a drawing (looking into mc:AlternateContent).
fn container(drawing: &Element) -> Option<&Element> {
    drawing.find("wp:inline").or_else(|| drawing.find("wp:anchor"))
}

pub fn layout(drawing: &Element) -> ImageLayout {
    let Some(c) = container(drawing) else { return ImageLayout { wrap: "inline".into(), ..Default::default() } };
    if c.name == "wp:inline" {
        return ImageLayout { wrap: "inline".into(), ..Default::default() };
    }
    let wrap = if c.child("wp:wrapSquare").is_some() {
        "square"
    } else if c.child("wp:wrapTight").is_some() || c.child("wp:wrapThrough").is_some() {
        "tight"
    } else if c.child("wp:wrapTopAndBottom").is_some() {
        "topBottom"
    } else if matches!(c.attr("behindDoc"), Some("1") | Some("true")) {
        "behind"
    } else {
        "front"
    };
    let ph = c.child("wp:positionH");
    let pv = c.child("wp:positionV");
    let align = ph.and_then(|p| p.child("wp:align")).map(|a| a.text()).map(|t| match t.trim() {
        "inside" => "left".to_string(),
        "outside" => "right".to_string(),
        other => other.to_string(),
    });
    ImageLayout {
        wrap: wrap.into(),
        x: ph.and_then(|p| p.child("wp:posOffset")).and_then(|o| emu_pt(Some(o.text().trim()))),
        y: pv.and_then(|p| p.child("wp:posOffset")).and_then(|o| emu_pt(Some(o.text().trim()))),
        rel_x: ph.and_then(|p| p.attr("relativeFrom")).map(String::from),
        rel_y: pv.and_then(|p| p.attr("relativeFrom")).map(String::from),
        align_y: pv.and_then(|p| p.child("wp:align")).map(|a| a.text().trim().to_string()),
        z: c.attr("relativeHeight").and_then(|v| v.trim().parse::<u64>().ok()),
        align,
    }
}

fn set_ext(el: &mut Element, cx: i64, cy: i64) {
    el.set_attr("cx", cx.to_string());
    el.set_attr("cy", cy.to_string());
}

fn find_mut<'a>(el: &'a mut Element, name: &str) -> Option<&'a mut Element> {
    for c in el.elements_mut() {
        if c.name == name {
            return Some(c);
        }
        if let Some(f) = find_mut(c, name) {
            return Some(f);
        }
    }
    None
}

const ANCHOR_ORDER: &[&str] = &[
    "wp:simplePos", "wp:positionH", "wp:positionV", "wp:extent", "wp:effectExtent", "wp:wrapNone", "wp:wrapSquare", "wp:wrapTight",
    "wp:wrapThrough", "wp:wrapTopAndBottom", "wp:docPr", "wp:cNvGraphicFramePr", "a:graphic",
];

/// Apply a patch to a drawing element (`w:drawing`, or an `mc:AlternateContent`
/// holding one, which is collapsed to the DrawingML choice).
pub fn apply(drawing: &mut Element, patch: &ImagePatch) -> Result<(), String> {
    // A new-style chart keeps its mc:AlternateContent, so Word still finds the chart part and the picture.
    if drawing.name == "mc:AlternateContent" && crate::chart::chartex::is_chartex_drawing(drawing) {
        return crate::chart::chartex::apply_patch(drawing, patch);
    }
    if drawing.name == "mc:AlternateContent" {
        let inner = drawing.find("w:drawing").cloned().ok_or("not a picture")?;
        *drawing = inner;
    }
    if drawing.find("wp:inline").is_none() && drawing.find("wp:anchor").is_none() {
        return Err("not a picture".into());
    }
    // A picture is placed by offsets or distances only when it floats.
    let mut patch = patch.clone();
    if patch.touches_place() && drawing.find("wp:inline").is_some() && patch.wrap.as_deref().is_none_or(|w| w == "inline") {
        patch.wrap = Some("square".into());
    }
    let patch = &patch;
    crate::picture::apply(drawing, patch)?;
    apply_layout(drawing, patch)?;
    // Room for a turn, outline or effect is measured from the size the picture ends up with, so this runs after the resize.
    if patch.width.is_some() || patch.height.is_some() {
        crate::picture::refresh_effect_extent(drawing);
    }
    Ok(())
}

fn apply_layout(drawing: &mut Element, patch: &ImagePatch) -> Result<(), String> {
    let is_inline = drawing.find("wp:inline").is_some();
    let c = if is_inline { find_mut(drawing, "wp:inline") } else { find_mut(drawing, "wp:anchor") }.ok_or("not a picture")?;

    if patch.width.is_some() || patch.height.is_some() {
        let ext = c.child("wp:extent").cloned().unwrap_or_else(|| Element::new("wp:extent"));
        let cur_w = ext.attr("cx").and_then(|v| v.parse::<f64>().ok()).unwrap_or(914400.0);
        let cur_h = ext.attr("cy").and_then(|v| v.parse::<f64>().ok()).unwrap_or(914400.0);
        let cx = patch.width.map(|w| w * EMU_PER_PT).unwrap_or(cur_w).max(EMU_PER_PT).round() as i64;
        let cy = patch.height.map(|h| h * EMU_PER_PT).unwrap_or(cur_h).max(EMU_PER_PT).round() as i64;
        if let Some(e) = c.child_mut("wp:extent") {
            set_ext(e, cx, cy);
        }
        // The picture's own transform carries the same size.
        if let Some(xfrm) = find_mut(c, "a:xfrm") {
            if let Some(e) = xfrm.child_mut("a:ext") {
                set_ext(e, cx, cy);
            }
        }
    }
    if let Some(alt) = &patch.alt {
        if let Some(d) = c.child_mut("wp:docPr") {
            d.set_attr("descr", alt);
        }
    }
    if patch.wrap.is_none() && patch.align.is_none() {
        if patch.touches_place() {
            place_extras(c, patch)?;
        }
        return Ok(());
    }
    let cur = layout(&Element { name: "w:drawing".into(), attrs: vec![], children: vec![Node::Element(c.clone())] });
    let was_through = c.child("wp:wrapThrough").is_some();
    let wrap = patch.wrap.clone().unwrap_or(if was_through { "through".into() } else { cur.wrap.clone() });
    let align = patch.align.clone().or(cur.align.clone()).unwrap_or_else(|| "left".into());
    if !matches!(wrap.as_str(), "inline" | "square" | "tight" | "through" | "topBottom" | "behind" | "front") {
        return Err(format!("unknown wrap '{wrap}'; use inline, square, tight, through, topBottom, behind or front"));
    }

    // Keep the parts both forms share.
    let take = |name: &str| c.child(name).cloned();
    let extent = take("wp:extent").unwrap_or_else(|| Element::new("wp:extent").with_attr("cx", "914400").with_attr("cy", "914400"));
    let effect = take("wp:effectExtent").unwrap_or_else(|| Element::new("wp:effectExtent").with_attr("l", "0").with_attr("t", "0").with_attr("r", "0").with_attr("b", "0"));
    let doc_pr = take("wp:docPr");
    let frame = take("wp:cNvGraphicFramePr");
    let graphic = take("a:graphic");
    let old_wrap_text = ["wp:wrapSquare", "wp:wrapTight", "wp:wrapThrough"].iter().find_map(|n| c.child(n)).and_then(|w| w.attr("wrapText")).map(String::from);
    let old_poly = ["wp:wrapTight", "wp:wrapThrough"].iter().find_map(|n| c.child(n)).and_then(|w| w.child("wp:wrapPolygon")).cloned();
    let extras: Vec<Node> = c
        .children
        .iter()
        .filter(|n| matches!(n, Node::Element(e) if !ANCHOR_ORDER.contains(&e.name.as_str())))
        .cloned()
        .collect();

    let mut out = if wrap == "inline" {
        let mut e = Element::new("wp:inline").with_attr("distT", "0").with_attr("distB", "0").with_attr("distL", "0").with_attr("distR", "0");
        for part in [Some(extent), Some(effect), doc_pr, frame, graphic].into_iter().flatten() {
            e.children.push(Node::Element(part));
        }
        e
    } else {
        let behind = wrap == "behind";
        let mut e = if c.name == "wp:anchor" {
            let mut keep = c.clone();
            keep.children.clear();
            keep
        } else {
            Element::new("wp:anchor")
                .with_attr("distT", "0")
                .with_attr("distB", "0")
                .with_attr("distL", "114300")
                .with_attr("distR", "114300")
                .with_attr("simplePos", "0")
                .with_attr("relativeHeight", "251659264")
                .with_attr("behindDoc", "0")
                .with_attr("locked", "0")
                .with_attr("layoutInCell", "1")
                .with_attr("allowOverlap", "1")
        };
        e.set_attr("behindDoc", if behind { "1" } else { "0" });
        let simple = c.child("wp:simplePos").cloned().unwrap_or_else(|| Element::new("wp:simplePos").with_attr("x", "0").with_attr("y", "0"));
        // Changing only how text wraps keeps where an anchored object is.
        let kept_h = if c.name == "wp:anchor" && patch.align.is_none() { c.child("wp:positionH").cloned() } else { None };
        let pos_h = kept_h.unwrap_or_else(|| {
            Element::new("wp:positionH")
                .with_attr("relativeFrom", "column")
                .with_child(Element::new("wp:align").with_text(match align.as_str() {
                    "center" => "center",
                    "right" => "right",
                    "inside" => "inside",
                    "outside" => "outside",
                    _ => "left",
                }))
        });
        let pos_v = c
            .child("wp:positionV")
            .cloned()
            .unwrap_or_else(|| Element::new("wp:positionV").with_attr("relativeFrom", "paragraph").with_child(Element::new("wp:posOffset").with_text("0")));
        let sides = patch.wrap_text.clone().or(old_wrap_text).unwrap_or_else(|| "bothSides".into());
        let polygon = |kind: &'static str| Element::new(kind).with_attr("wrapText", sides.clone()).with_child(old_poly.clone().unwrap_or_else(default_polygon));
        let wrap_el = match wrap.as_str() {
            "square" => Element::new("wp:wrapSquare").with_attr("wrapText", sides.clone()),
            "tight" => polygon("wp:wrapTight"),
            "through" => polygon("wp:wrapThrough"),
            "topBottom" => Element::new("wp:wrapTopAndBottom"),
            _ => Element::new("wp:wrapNone"),
        };
        for part in [Some(simple), Some(pos_h), Some(pos_v), Some(extent), Some(effect), Some(wrap_el), doc_pr, frame, graphic].into_iter().flatten() {
            e.children.push(Node::Element(part));
        }
        e
    };
    out.children.extend(extras);
    // Namespace declarations often live on the container itself (python-docx puts
    // xmlns:pic there); carry them over or descendants become unbound.
    for (k, v) in &c.attrs {
        if (k.starts_with("xmlns:") || k == "xmlns") && out.attr(k).is_none() {
            out.attrs.push((k.clone(), v.clone()));
        }
    }
    *c = out;
    if patch.touches_place() && c.name == "wp:anchor" {
        place_extras(c, patch)?;
    }
    // Anchored drawings need the wp prefix declared somewhere; documents that use
    // pictures always do, and new ones come from Docxtor's own templates.
    Ok(())
}

/// The outline Word writes for a rectangle that wraps tightly or through: its four corners in a 21600 grid.
fn default_polygon() -> Element {
    let pt = |name: &'static str, x: i32, y: i32| Element::new(name).with_attr("x", x.to_string()).with_attr("y", y.to_string());
    Element::new("wp:wrapPolygon")
        .with_attr("edited", "0")
        .with_child(pt("wp:start", 0, 0))
        .with_child(pt("wp:lineTo", 0, 21600))
        .with_child(pt("wp:lineTo", 21600, 21600))
        .with_child(pt("wp:lineTo", 21600, 0))
        .with_child(pt("wp:lineTo", 0, 0))
}

const REL_H: &[&str] = &["character", "column", "insideMargin", "leftMargin", "margin", "outsideMargin", "page", "rightMargin"];
const REL_V: &[&str] = &["bottomMargin", "insideMargin", "line", "margin", "outsideMargin", "page", "paragraph", "topMargin"];

/// Where a floating picture sits, in numbers: the offsets and what they count from, the distance of the text
/// and the anchor's behaviour (Word's Position and Layout dialog).
fn place_extras(c: &mut Element, patch: &ImagePatch) -> Result<(), String> {
    if let Some(r) = &patch.rel_x {
        if !REL_H.contains(&r.as_str()) {
            return Err(format!("unknown horizontal reference '{r}'; use {}", REL_H.join(", ")));
        }
    }
    if let Some(r) = &patch.rel_y {
        if !REL_V.contains(&r.as_str()) {
            return Err(format!("unknown vertical reference '{r}'; use {}", REL_V.join(", ")));
        }
    }
    if let Some(a) = &patch.align_y {
        if !matches!(a.as_str(), "top" | "center" | "bottom" | "inside" | "outside") {
            return Err(format!("unknown vertical alignment '{a}'; use top, center, bottom, inside or outside"));
        }
    }
    if let Some(w) = &patch.wrap_text {
        if !matches!(w.as_str(), "bothSides" | "left" | "right" | "largest") {
            return Err(format!("unknown text side '{w}'; use bothSides, left, right or largest"));
        }
    }
    let emu = |pt: f64| (pt * EMU_PER_PT).round().max(0.0) as i64;
    for (axis, rel, off, align) in [("wp:positionH", &patch.rel_x, patch.x, None::<&String>), ("wp:positionV", &patch.rel_y, patch.y, patch.align_y.as_ref())] {
        if rel.is_none() && off.is_none() && align.is_none() {
            continue;
        }
        let el = c.child_mut(axis).ok_or("that picture has no position")?;
        if let Some(r) = rel {
            el.set_attr("relativeFrom", r.clone());
        }
        if let Some(a) = align {
            el.children = vec![Node::Element(Element::new("wp:align").with_text(a.clone()))];
        } else if let Some(v) = off {
            el.children = vec![Node::Element(Element::new("wp:posOffset").with_text(((v * EMU_PER_PT).round() as i64).to_string()))];
        }
    }
    if let Some([t, b, l, r]) = patch.dist {
        for (k, v) in [("distT", t), ("distB", b), ("distL", l), ("distR", r)] {
            c.set_attr(k, emu(v).to_string());
            // A wrap element can carry its own distances; they would win over the anchor's.
            for w in ["wp:wrapSquare", "wp:wrapTight", "wp:wrapThrough", "wp:wrapTopAndBottom"] {
                if let Some(we) = c.child_mut(w) {
                    if we.attr(k).is_some() {
                        we.set_attr(k, emu(v).to_string());
                    }
                }
            }
        }
    }
    if let Some(w) = &patch.wrap_text {
        for n in ["wp:wrapSquare", "wp:wrapTight", "wp:wrapThrough"] {
            if let Some(we) = c.child_mut(n) {
                we.set_attr("wrapText", w.clone());
            }
        }
    }
    if let Some(v) = patch.lock_anchor {
        c.set_attr("locked", if v { "1" } else { "0" });
    }
    if let Some(v) = patch.allow_overlap {
        c.set_attr("allowOverlap", if v { "1" } else { "0" });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::xml;

    fn drawing() -> Element {
        xml::parse(
            br#"<w:drawing xmlns:w="w" xmlns:wp="wp" xmlns:a="a"><wp:inline distT="0" xmlns:pic="pic"><wp:extent cx="1270000" cy="635000"/><wp:effectExtent l="0" t="0" r="0" b="0"/><wp:docPr id="1" name="P"/><a:graphic><a:graphicData><pic:pic><pic:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="1270000" cy="635000"/></a:xfrm></pic:spPr></pic:pic></a:graphicData></a:graphic></wp:inline></w:drawing>"#,
        )
        .unwrap()
        .root
    }

    #[test]
    fn turning_and_resizing_together_leaves_room_for_the_new_size() {
        let mut d = drawing();
        let patch: ImagePatch = serde_json::from_str(r#"{"rot":90,"width":200,"height":100}"#).unwrap();
        apply(&mut d, &patch).unwrap();
        // 200 x 100 turned a quarter is 100 x 200: 50 pt more above and below (635000 EMU), none at the sides.
        let ee = d.find("wp:effectExtent").unwrap();
        assert_eq!((ee.attr("l"), ee.attr("t"), ee.attr("r"), ee.attr("b")), (Some("0"), Some("635000"), Some("0"), Some("635000")));
    }

    #[test]
    fn resize_and_wrap_roundtrip() {
        let mut d = drawing();
        apply(&mut d, &ImagePatch { width: Some(50.0), height: Some(25.0), alt: Some("chart".into()), ..Default::default() }).unwrap();
        assert_eq!(d.find("wp:extent").unwrap().attr("cx"), Some("635000"));
        assert_eq!(d.find("a:ext").unwrap().attr("cy"), Some("317500"));
        assert_eq!(d.find("wp:docPr").unwrap().attr("descr"), Some("chart"));

        apply(&mut d, &ImagePatch { wrap: Some("square".into()), align: Some("right".into()), ..Default::default() }).unwrap();
        let l = layout(&d);
        assert_eq!((l.wrap.as_str(), l.align.as_deref()), ("square", Some("right")));
        let anchor = d.find("wp:anchor").unwrap();
        let names: Vec<_> = anchor.elements().map(|e| e.name.as_str()).collect();
        assert_eq!(names, ["wp:simplePos", "wp:positionH", "wp:positionV", "wp:extent", "wp:effectExtent", "wp:wrapSquare", "wp:docPr", "a:graphic"]);

        apply(&mut d, &ImagePatch { wrap: Some("behind".into()), ..Default::default() }).unwrap();
        assert_eq!(layout(&d).wrap, "behind");
        apply(&mut d, &ImagePatch { wrap: Some("inline".into()), ..Default::default() }).unwrap();
        assert_eq!(layout(&d).wrap, "inline");
        assert_eq!(d.find("wp:inline").unwrap().attr("xmlns:pic"), Some("pic"), "namespace declarations survive re-wrapping");
        assert_eq!(d.find("wp:extent").unwrap().attr("cx"), Some("635000"));
    }
}
