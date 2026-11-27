//! Arranging objects on the page: the stacking order of floating objects, behind or in front of the text,
//! grouping shapes and taking a group apart, and freehand drawings. See `crate::shapes::{order, group}`.

use super::*;
use crate::shapes::order::{self, Order, PICK};
use crate::shapes::{build, group, story};
use std::collections::BTreeMap;

impl Document {
    pub(super) fn apply_arrange_op(&mut self, op: &Op) -> Result<OpResult, EditError> {
        match op {
            Op::SetObjectOrder { at, order } => self.op_object_order(*at, order),
            Op::SetBehindText { at, behind } => self.op_behind_text(*at, *behind),
            Op::GroupShapes { at, offsets } => self.op_group(at, offsets.as_deref()),
            Op::UngroupShape { at, offset } => self.op_ungroup(*at, *offset),
            Op::InsertInk { at, strokes, color, width_pt, alpha, cap } => self.op_insert_ink(*at, strokes, color.as_deref(), *width_pt, *alpha, cap.as_deref()),
            _ => unreachable!("not an arrange op"),
        }
    }

    /// A copy of the object at a position.
    fn object_copy(&mut self, at: Pos) -> Result<Element, EditError> {
        let p = self.para_ref(at.p.0)?;
        edit::object_at(&p.inlines, at.o).cloned().ok_or_else(|| EditError::Invalid("there is no object at that position".into()))
    }

    fn ensure_shape_ns(&mut self) {
        for (p, u) in build::ROOT_NS.iter().chain([("wpg", group::NS_WPG)].iter()) {
            self.ensure_ns(p, u);
        }
    }

    // -----------------------------------------------------------------------
    // Stacking order
    // -----------------------------------------------------------------------

    fn op_object_order(&mut self, at: Pos, how: &str) -> Result<OpResult, EditError> {
        let ord = Order::parse(how).map_err(EditError::Invalid)?;
        self.touch(at.p.0);
        {
            let p = self.para_mut(at.p.0)?;
            let el = edit::object_at_mut(&mut p.inlines, at.o).ok_or_else(|| EditError::Invalid("there is no object at that position".into()))?;
            order::require_floating(el).map_err(EditError::Invalid)?;
            el.set_attr(PICK, "1");
        }
        // Every floating object of the text (or header, footer or note) holding the target, in document order.
        let mut items: Vec<(bool, order::Stack)> = Vec::new();
        story::walk_objects(&self.body, &mut |_, e| {
            if let Some(st) = order::stack_of(e) {
                items.push((e.attr(PICK).is_some(), st));
            }
        });
        let changes: BTreeMap<usize, u64> = match items.iter().position(|i| i.0) {
            None => BTreeMap::new(),
            Some(t) => {
                // Behind the text and in front of it are two stacks.
                let behind = items[t].1.behind;
                let peers: Vec<usize> = (0..items.len()).filter(|&i| items[i].1.behind == behind).collect();
                let z: Vec<u64> = peers.iter().map(|&i| items[i].1.z).collect();
                let local = peers.iter().position(|&i| i == t).expect("target among its peers");
                order::reorder(&z, local, ord).into_iter().map(|(i, v)| (peers[i], v)).collect()
            }
        };
        let mut touched = Vec::new();
        let mut index = 0;
        story::walk_objects_edit_body(&mut self.body, &mut |pid, e| {
            let mut changed = e.remove_attr(PICK).is_some();
            if order::stack_of(e).is_some() {
                if let Some(v) = changes.get(&index) {
                    order::set_z(e, *v);
                    touched.push(pid);
                    changed = true;
                }
                index += 1;
            }
            changed
        });
        for pid in touched {
            self.touch(pid);
        }
        Ok(OpResult { caret: Some(at), ..Default::default() })
    }

    fn op_behind_text(&mut self, at: Pos, behind: bool) -> Result<OpResult, EditError> {
        let el = self.object_copy(at)?;
        let wrap = Some(if behind { "behind" } else { "front" }.to_string());
        if crate::shapes::is_shape(&el) {
            let patch = crate::shapes::patch::ShapePatch { wrap, ..Default::default() };
            return self.apply_shape_op(&Op::SetShape { at, patch });
        }
        self.apply_inner(&Op::SetImage { at, patch: crate::image::ImagePatch { wrap, ..Default::default() } })
    }

    /// The highest stacking value in front of the text, or Word's first value when there is none.
    fn top_z(&self) -> u64 {
        let mut top: Option<u64> = None;
        story::walk_objects(&self.body, &mut |_, e| {
            if let Some(s) = order::stack_of(e).filter(|s| !s.behind) {
                top = Some(top.map_or(s.z, |t| t.max(s.z)));
            }
        });
        top.map_or(order::Z_BASE, |t| t + order::Z_STEP)
    }

    // -----------------------------------------------------------------------
    // Groups
    // -----------------------------------------------------------------------

    fn op_group(&mut self, at: &[Pos], offsets: Option<&[[f64; 2]]>) -> Result<OpResult, EditError> {
        let invalid = EditError::Invalid;
        if at.len() < 2 {
            return Err(invalid("select at least two shapes to group them".into()));
        }
        if offsets.is_some_and(|o| o.len() != at.len()) {
            return Err(invalid("give one offset for each shape".into()));
        }
        for (i, a) in at.iter().enumerate() {
            if at[..i].contains(a) {
                return Err(invalid("a shape is listed twice".into()));
            }
        }
        let mut parts = Vec::new();
        let mut first_anchor: Option<Element> = None;
        let mut frame: Option<(String, String)> = None;
        for (i, pos) in at.iter().enumerate() {
            let el = self.object_copy(*pos)?;
            let anchor = el.find("wp:anchor").cloned();
            let rel = |a: &Element, n: &str| a.child(n).and_then(|p| p.attr("relativeFrom")).unwrap_or("").to_string();
            let part = group::take_part(&el, offsets.map(|o| (o[i][0], o[i][1])), frame.as_ref().filter(|_| offsets.is_none()).map(|(h, v)| (h.as_str(), v.as_str()))).map_err(invalid)?;
            if let (None, Some(a)) = (&first_anchor, &anchor) {
                frame = Some((rel(a, "wp:positionH"), rel(a, "wp:positionV")));
                first_anchor = Some(a.clone());
            }
            parts.push(part);
        }
        let first_anchor = first_anchor.ok_or_else(|| invalid("a shape in line with the text can't be grouped; wrap it first".into()))?;
        self.ensure_shape_ns();
        self.drawing_id += 1;
        let id = self.drawing_id;
        let z = parts.iter().map(|p| p.z).max().unwrap_or(order::Z_BASE);
        let el = group::build_group(&parts, &first_anchor, id, &format!("Group {id}"), z);
        // The shapes go, the group takes the place of the first one.
        let mut gone = at.to_vec();
        gone.sort_by_key(|p| std::cmp::Reverse((p.p.0, p.o)));
        let before = at[0].o - at.iter().filter(|p| p.p == at[0].p && p.o < at[0].o).count();
        for pos in gone {
            self.apply_inner(&Op::Delete { from: pos, to: Pos::new(pos.p.0, pos.o + 1) })?;
        }
        let off = self.insert_object(at[0].p.0, before, el)?;
        Ok(OpResult { caret: Some(Pos::new(at[0].p.0, off + 1)), ..Default::default() })
    }

    /// Put an object in the text; returns the offset it landed at.
    fn insert_object(&mut self, para: NodeId, off: usize, el: Element) -> Result<usize, EditError> {
        self.touch(para);
        let p = self.para_mut(para)?;
        let off = off.min(p.len16());
        let run = Run { t_attrs: None, attrs: vec![], rpr: None, items: vec![RunItem::Object(el)] };
        edit::insert_inline_at(&mut p.inlines, off, Inline::Run(run));
        Ok(off)
    }

    fn op_ungroup(&mut self, at: Pos, offset: Option<[f64; 2]>) -> Result<OpResult, EditError> {
        let el = self.object_copy(at)?;
        self.ensure_shape_ns();
        let parts = group::ungroup(&el, offset.map(|o| (o[0], o[1])), ("column", "paragraph")).map_err(EditError::Invalid)?;
        self.apply_inner(&Op::Delete { from: at, to: Pos::new(at.p.0, at.o + 1) })?;
        let mut off = at.o;
        for part in parts {
            off = self.insert_object(at.p.0, off, part)? + 1;
        }
        Ok(OpResult { caret: Some(Pos::new(at.p.0, off)), ..Default::default() })
    }

    // -----------------------------------------------------------------------
    // Freehand drawing
    // -----------------------------------------------------------------------

    fn op_insert_ink(&mut self, at: Pos, strokes: &[Vec<[f64; 2]>], color: Option<&str>, width_pt: Option<f64>, alpha: Option<f64>, cap: Option<&str>) -> Result<OpResult, EditError> {
        let invalid = EditError::Invalid;
        let color = match color {
            Some(c) => crate::shapes::hex6(c).ok_or_else(|| invalid(format!("'{c}' is not a colour; use six hex digits like 000000")))?,
            None => "000000".into(),
        };
        let width_pt = width_pt.unwrap_or(2.25);
        if !(0.25..=72.0).contains(&width_pt) {
            return Err(invalid("the pen width must be between 0.25 and 72 points".into()));
        }
        if alpha.is_some_and(|a| !(0.0..=1.0).contains(&a)) {
            return Err(invalid("alpha must be between 0 and 1".into()));
        }
        let cap = match cap {
            None | Some("round" | "rnd") => group::InkCap::Round,
            Some("square" | "sq") => group::InkCap::Square,
            Some("flat") => group::InkCap::Flat,
            Some(other) => return Err(invalid(format!("unknown cap '{other}'; use round, square or flat"))),
        };
        if strokes.iter().map(|s| s.len()).sum::<usize>() > 200_000 {
            return Err(invalid("the drawing has too many points".into()));
        }
        if strokes.iter().flatten().any(|p| !p[0].is_finite() || !p[1].is_finite() || p[0].abs() > 1e5 || p[1].abs() > 1e5) {
            return Err(invalid("the drawing has a point off the page".into()));
        }
        self.ensure_shape_ns();
        self.drawing_id += 1;
        let ink = group::Ink { strokes: strokes.iter().map(|s| s.iter().map(|p| (p[0], p[1])).collect()).collect(), color, width_pt, alpha: alpha.filter(|a| *a < 1.0), cap, id: self.drawing_id, z: self.top_z() };
        let (el, ..) = group::build_ink(&ink).map_err(invalid)?;
        let off = self.insert_object(at.p.0, at.o, el)?;
        Ok(OpResult { caret: Some(Pos::new(at.p.0, off + 1)), ..Default::default() })
    }
}
