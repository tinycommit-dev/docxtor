//! Curves, freeforms and scribbles, and giving a shape a new outline ("Edit points"). See `crate::shapes::freeform`.

use super::*;
use crate::shapes::build::{self, LineLook};
use crate::shapes::freeform::{self, CmdIn, FreeSpec, PathIn, Seg};
use crate::shapes::patch::ShapePatch;

impl Document {
    pub(super) fn apply_geometry_op(&mut self, op: &Op) -> Result<OpResult, EditError> {
        match op {
            Op::InsertFreeform { at, points, cmds, smooth, closed, fill, line, floating, x, y, label } => {
                self.op_insert_freeform(*at, points, cmds, *smooth, *closed, fill.as_deref(), line.as_ref(), *floating, *x, *y, label.as_deref())
            }
            Op::SetShapePath { at, paths, x, y } => self.op_set_shape_path(*at, paths, *x, *y),
            _ => unreachable!("not a geometry op"),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn op_insert_freeform(
        &mut self,
        at: Pos,
        points: &[[f64; 2]],
        cmds: &[CmdIn],
        smooth: bool,
        closed: bool,
        fill: Option<&str>,
        line: Option<&crate::shapes::LineSpec>,
        floating: bool,
        x: Option<f64>,
        y: Option<f64>,
        label: Option<&str>,
    ) -> Result<OpResult, EditError> {
        let invalid = EditError::Invalid;
        if self.active_part != self.main_path {
            return Err(invalid("that can only be inserted in the main text".into()));
        }
        let (segs, closed) = if !cmds.is_empty() {
            let segs = freeform::segs(cmds).map_err(invalid)?;
            let closed = segs.iter().any(|s| matches!(s, Seg::Z));
            (segs, closed)
        } else {
            if points.len() < 2 {
                return Err(invalid("give at least two points".into()));
            }
            if points.len() > 100_000 || points.iter().any(|p| !p[0].is_finite() || !p[1].is_finite() || p[0].abs() > 1e5 || p[1].abs() > 1e5) {
                return Err(invalid("the outline has a point off the page".into()));
            }
            let pts: Vec<(f64, f64)> = points.iter().map(|p| (p[0], p[1])).collect();
            (freeform::from_points(&pts, smooth, closed), closed)
        };
        let fill = match fill {
            Some(f) => build::fill_hex(f).map_err(invalid)?,
            None => closed.then(|| "DAE3F3".to_string()),
        };
        let default_line = Some(LineLook { color: "2F528F".into(), width_pt: if closed { 1.0 } else { 1.5 }, ..Default::default() });
        let line = build::line_look(line, default_line).map_err(EditError::Invalid)?;
        self.drawing_id += 1;
        let spec = FreeSpec {
            segs,
            closed,
            fill,
            line,
            floating,
            x: x.unwrap_or(0.0),
            y: y.unwrap_or(0.0),
            id: self.drawing_id,
            label: label.filter(|l| !l.is_empty()).unwrap_or("Freeform: Shape").to_string(),
        };
        let (el, w, h) = freeform::build_free(&spec).map_err(EditError::Invalid)?;
        if w > 5000.0 || h > 5000.0 {
            return Err(EditError::Invalid("the outline is much bigger than a page".into()));
        }
        build::ensure_root_ns(&mut self.doc_xml.root);
        self.touch(at.p.0);
        let p = self.para_mut(at.p.0)?;
        let off = at.o.min(p.len16());
        let run = Run { t_attrs: None, attrs: vec![], rpr: None, items: vec![RunItem::Object(el)] };
        edit::insert_inline_at(&mut p.inlines, off, Inline::Run(run));
        Ok(OpResult { caret: Some(Pos::new(at.p.0, off + 1)), ..Default::default() })
    }

    fn op_set_shape_path(&mut self, at: Pos, paths: &[PathIn], x: Option<f64>, y: Option<f64>) -> Result<OpResult, EditError> {
        let invalid = |m: &str| EditError::Invalid(m.into());
        let cur = self.shape_at(at).ok_or_else(|| invalid("there is no shape at that position"))?;
        if cur.kind != "shape" {
            return Err(invalid("only a single shape has points to edit, not a group"));
        }
        if cur.source != "drawingml" {
            return Err(invalid("this shape comes from an old file format; its points can't be edited"));
        }
        let plan = freeform::plan(paths, cur.w, cur.h).map_err(EditError::Invalid)?;
        let mut patch = ShapePatch { w: Some(plan.w.max(0.1)), h: Some(plan.h.max(0.1)), ..Default::default() };
        // The box follows the outline: a floating shape's corner moves to where the new bounds start.
        if cur.layout.wrap != "inline" {
            let corner = match (x, y) {
                (Some(x), Some(y)) => Some((x, y)),
                _ => match (cur.layout.x, cur.layout.y) {
                    (Some(cx), Some(cy)) => Some(freeform::new_corner((cx, cy, cur.w, cur.h), cur.rot, (cur.flip_h, cur.flip_v), &plan)),
                    _ => None,
                },
            };
            if let Some((px, py)) = corner {
                patch.x = Some((px * 100.0).round() / 100.0);
                patch.y = Some((py * 100.0).round() / 100.0);
            }
        }
        self.touch(at.p.0);
        let p = self.para_mut(at.p.0)?;
        let el = edit::object_at_mut(&mut p.inlines, at.o).ok_or_else(|| invalid("there is no shape at that position"))?;
        crate::shapes::patch::apply(el, &patch, &cur).map_err(EditError::Invalid)?;
        freeform::write(el, &plan).map_err(EditError::Invalid)?;
        Ok(OpResult { caret: Some(at), ..Default::default() })
    }
}
