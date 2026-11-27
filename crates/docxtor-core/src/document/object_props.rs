//! Ops every drawing object shares: naming it, hiding it, placing it on the page and moving it in the stacking
//! order, and the list of all objects the Selection pane shows. See `crate::shapes::object_props`.

use super::*;
use crate::shapes::object_props::{self as props, AxisPos, ObjectInfo};
use crate::shapes::order::{self, PICK};
use crate::shapes::{stack_move, story};
use std::collections::BTreeMap;

fn collect(inlines: &[Inline], start: &mut usize, pid: u32, out: &mut Vec<ObjectInfo>) {
    for inl in inlines {
        match inl {
            Inline::Run(r) => {
                for item in &r.items {
                    let l = item.len16();
                    if let (RunItem::Object(e), 1) = (item, l) {
                        if let Some(i) = props::info(e, pid, *start) {
                            out.push(i);
                        }
                    }
                    *start += l;
                }
            }
            Inline::Container(c) => collect(&c.children, start, pid, out),
            Inline::Opaque(_) => *start += inl.len16(),
        }
    }
}

impl Document {
    /// Every drawing object of the main text in document order: shapes, text boxes, pictures, charts and groups,
    /// with the name, hidden flag and stacking the Selection pane shows.
    pub fn objects(&self) -> Vec<ObjectInfo> {
        let mut out = Vec::new();
        walk_paragraphs(&self.body, &mut |p| {
            let mut start = 0;
            collect(&p.inlines, &mut start, p.id, &mut out);
        });
        out
    }

    pub(super) fn apply_object_op(&mut self, op: &Op) -> Result<OpResult, EditError> {
        match op {
            Op::SetObjectProps { at, name, hidden } => self.op_object_props(*at, name.as_deref(), *hidden),
            Op::SetObjectPosition { at, rel_x, align, x, rel_y, align_y, y } => {
                let h = AxisPos { rel: rel_x.clone(), align: align.clone(), offset: *x };
                let v = AxisPos { rel: rel_y.clone(), align: align_y.clone(), offset: *y };
                self.op_object_position(*at, &h, &v)
            }
            Op::MoveObject { at, above, below } => self.op_move_object(*at, *above, *below),
            _ => unreachable!("not an object op"),
        }
    }

    /// Change the object at a position with `f`.
    fn with_object<R>(&mut self, at: Pos, f: impl FnOnce(&mut Element) -> Result<R, String>) -> Result<R, EditError> {
        self.touch(at.p.0);
        let p = self.para_mut(at.p.0)?;
        let el = edit::object_at_mut(&mut p.inlines, at.o).ok_or_else(|| EditError::Invalid("there is no object at that position".into()))?;
        if props::kind_of(el).is_none() {
            return Err(EditError::Invalid("that object is not a drawing".into()));
        }
        f(el).map_err(EditError::Invalid)
    }

    fn op_object_props(&mut self, at: Pos, name: Option<&str>, hidden: Option<bool>) -> Result<OpResult, EditError> {
        if let Some(n) = name {
            if n.trim().is_empty() {
                return Err(EditError::Invalid("an object's name can't be empty".into()));
            }
            if n.chars().count() > 255 {
                return Err(EditError::Invalid("an object's name can be 255 characters at most".into()));
            }
        }
        self.with_object(at, |el| {
            if let Some(n) = name {
                props::set_name(el, n.trim());
            }
            if let Some(h) = hidden {
                props::set_hidden(el, h);
            }
            Ok(())
        })?;
        Ok(OpResult { caret: Some(at), ..Default::default() })
    }

    fn op_object_position(&mut self, at: Pos, x: &AxisPos, y: &AxisPos) -> Result<OpResult, EditError> {
        self.with_object(at, |el| props::set_position(el, x, y))?;
        Ok(OpResult { caret: Some(at), ..Default::default() })
    }

    fn op_move_object(&mut self, at: Pos, above: Option<Pos>, below: Option<Pos>) -> Result<OpResult, EditError> {
        let invalid = |m: &str| EditError::Invalid(m.into());
        let (anchor, is_above) = match (above, below) {
            (Some(a), None) => (a, true),
            (None, Some(b)) => (b, false),
            _ => return Err(invalid("give either `above` or `below`, one object to stack against")),
        };
        if anchor == at {
            return Ok(OpResult { caret: Some(at), ..Default::default() });
        }
        // Both must float, in the same stack. Checked before anything is marked.
        let stack_at = |this: &mut Document, pos: Pos| -> Result<order::Stack, EditError> {
            let p = this.para_mut(pos.p.0)?;
            let el = edit::object_at_mut(&mut p.inlines, pos.o).ok_or_else(|| invalid("there is no object at that position"))?;
            order::require_floating(el).map_err(EditError::Invalid)
        };
        let (st, sa) = (stack_at(self, at)?, stack_at(self, anchor)?);
        if st.behind != sa.behind {
            return Err(invalid("the two objects are not in the same stack; one is behind the text and the other in front of it"));
        }
        for (pos, mark) in [(at, "1"), (anchor, "2")] {
            self.touch(pos.p.0);
            let p = self.para_mut(pos.p.0)?;
            if let Some(el) = edit::object_at_mut(&mut p.inlines, pos.o) {
                el.set_attr(PICK, mark);
            }
        }
        let mut items: Vec<(Option<String>, order::Stack)> = Vec::new();
        story::walk_objects(&self.body, &mut |_, e| {
            if let Some(s) = order::stack_of(e) {
                items.push((e.attr(PICK).map(String::from), s));
            }
        });
        let find = |m: &str| items.iter().position(|i| i.0.as_deref() == Some(m));
        let changes: BTreeMap<usize, u64> = match (find("1"), find("2")) {
            (Some(t), Some(a)) => {
                let peers: Vec<usize> = (0..items.len()).filter(|&i| items[i].1.behind == st.behind).collect();
                let z: Vec<u64> = peers.iter().map(|&i| items[i].1.z).collect();
                let lt = peers.iter().position(|&i| i == t).expect("target among its peers");
                let la = peers.iter().position(|&i| i == a).expect("anchor among its peers");
                stack_move::place(&z, lt, la, is_above).into_iter().map(|(i, v)| (peers[i], v)).collect()
            }
            _ => BTreeMap::new(),
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
}
