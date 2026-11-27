//! Connectors: inserting and changing them, and keeping them attached while ops move, resize, turn, mirror,
//! regroup or delete the shapes they are glued to. See `crate::shapes::connector`.
//!
//! Keeping attached works on the whole batch of ops: before the first op the sites that connectors hold are
//! noted, after the last op they are looked at again, and a connector whose shape moved is routed again in the
//! same undo step. A shape's place is only compared with itself, so shapes and connectors anchored to different
//! paragraphs keep working: the connector's end moves by as much as the site did.

use super::*;
use crate::shapes::connector::{self, Attach, Dir, EndSpec, Geo, Kind, Route, Site, Where};
use crate::shapes::connector::{drawing_mut, find_mut};
use crate::shapes::{build, member, read, story, EMU_PER_PT};
use std::collections::{BTreeMap, HashSet};

/// What a position is counted from. Positions in the same frame can be compared.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Frame(String, String, u32);

/// A shape's box on the page, in the frame of the outermost thing it sits in (points).
#[derive(Debug, Clone)]
pub(super) struct World {
    frame: Frame,
    geo: Geo,
    /// How the shape's own container maps to the page: `page = origin + scale * inside`.
    origin: (f64, f64),
    scale: (f64, f64),
}

#[derive(Debug, Clone)]
pub(super) struct Cxn {
    start: Option<Attach>,
    end: Option<Attach>,
    prst: String,
    adj: BTreeMap<String, f64>,
}

/// A shape or connector found in the text.
#[derive(Debug, Clone)]
pub(super) struct Found {
    id: u32,
    /// On its own in the text (not a member of a group or canvas).
    top: bool,
    world: Option<World>,
    /// Connection sites in the shape's own box (points).
    sites: Vec<Site>,
    conn: Option<Cxn>,
}

impl Found {
    /// A site as it is on the page: the frame, the place and the way a connector leaves it.
    fn site(&self, idx: usize) -> Option<(Frame, (f64, f64), Dir)> {
        let w = self.world.as_ref()?;
        let s = self.sites.get(idx)?;
        let (p, d) = w.geo.site(s);
        Some((w.frame.clone(), p, d))
    }
}

fn num(e: Option<&Element>, a: &str) -> f64 {
    e.and_then(|e| e.attr(a)).and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0)
}

fn pos_offset(anchor: &Element, name: &str) -> Option<f64> {
    anchor.child(name)?.child("wp:posOffset")?.text().trim().parse::<f64>().ok().map(|v| v / EMU_PER_PT)
}

fn truthy(v: Option<&str>) -> bool {
    matches!(v, Some("1" | "true" | "on"))
}

fn conn_of(wsp: &Element) -> Option<Cxn> {
    if !connector::is_connector(wsp) {
        return None;
    }
    let (start, end) = connector::attachments(wsp);
    let prst = wsp.child("wps:spPr").and_then(|s| s.child("a:prstGeom")).and_then(|g| g.attr("prst")).unwrap_or("").to_string();
    Some(Cxn { start, end, prst, adj: connector::adjusts(wsp) })
}

/// Every shape and connector in an object of the text, with its place on the page.
fn describe(pid: NodeId, obj: &Element, out: &mut Vec<Found>) {
    let from = out.len();
    describe_in(obj, out);
    for f in &mut out[from..] {
        if let Some(w) = &mut f.world {
            // Positions counted from a paragraph only compare within that paragraph.
            if matches!(w.frame.1.as_str(), "" | "paragraph" | "line") || w.frame.0 == "character" {
                w.frame.2 = pid;
            }
        }
    }
}

fn describe_in(obj: &Element, out: &mut Vec<Found>) {
    let Some(read::Src::Drawing(d)) = read::primary(obj) else { return };
    let Some(c) = d.child("wp:anchor").or_else(|| d.child("wp:inline")) else { return };
    let anchored = c.name == "wp:anchor";
    let id: u32 = c.child("wp:docPr").and_then(|p| p.attr("id")).and_then(|v| v.parse().ok()).unwrap_or(0);
    let Some(content) = c.find("a:graphicData").and_then(|g| g.elements().next()) else { return };
    let ext = c.child("wp:extent");
    let (ew, eh) = (num(ext, "cx") / EMU_PER_PT, num(ext, "cy") / EMU_PER_PT);
    let rel = |n: &str| c.child(n).and_then(|p| p.attr("relativeFrom")).unwrap_or("").to_string();
    let place = if anchored { pos_offset(c, "wp:positionH").zip(pos_offset(c, "wp:positionV")) } else { None };
    let (rh, rv) = (rel("wp:positionH"), rel("wp:positionV"));
    let frame = Frame(rh, rv, 0);
    match content.name.as_str() {
        "wps:wsp" => {
            let x = content.child("wps:spPr").and_then(|s| s.child("a:xfrm"));
            let world = place.map(|(px, py)| World {
                frame,
                geo: Geo { x: px, y: py, w: ew, h: eh, rot: num(x, "rot") / 60000.0, flip_h: truthy(x.and_then(|x| x.attr("flipH"))), flip_v: truthy(x.and_then(|x| x.attr("flipV"))) },
                origin: (0.0, 0.0),
                scale: (1.0, 1.0),
            });
            out.push(Found { id, top: true, world, sites: connector::wsp_sites(content, ew, eh), conn: conn_of(content) });
        }
        "wpg:wgp" | "wpc:wpc" => {
            let x = content.child("wpg:grpSpPr").and_then(|s| s.child("a:xfrm"));
            let (rot, fh, fv) = (num(x, "rot"), truthy(x.and_then(|x| x.attr("flipH"))), truthy(x.and_then(|x| x.attr("flipV"))));
            // A group or canvas in line with the text has no place of its own: its members are compared with each other.
            let valid = rot == 0.0 && !fh && !fv;
            let (px, py) = place.unwrap_or((0.0, 0.0));
            let (mut origin, mut scale) = ((px, py), (1.0, 1.0));
            if content.name == "wpg:wgp" {
                let (off, cext) = (x.and_then(|x| x.child("a:chOff")), x.and_then(|x| x.child("a:chExt")));
                let (cw, ch) = (num(cext, "cx"), num(cext, "cy"));
                scale = (if cw > 0.0 { ew * EMU_PER_PT / cw } else { 1.0 }, if ch > 0.0 { eh * EMU_PER_PT / ch } else { 1.0 });
                origin = (px - num(off, "x") / EMU_PER_PT * scale.0, py - num(off, "y") / EMU_PER_PT * scale.1);
            }
            members(content, origin, scale, &frame, valid, 0, out);
        }
        _ => {}
    }
}

fn members(container: &Element, origin: (f64, f64), scale: (f64, f64), frame: &Frame, valid: bool, depth: usize, out: &mut Vec<Found>) {
    if depth > 6 {
        return;
    }
    for m in container.elements().filter(|m| member::is_member(m)) {
        match m.name.as_str() {
            "wps:wsp" => {
                let x = m.child("wps:spPr").and_then(|s| s.child("a:xfrm"));
                let (off, ext) = (x.and_then(|x| x.child("a:off")), x.and_then(|x| x.child("a:ext")));
                let (gx, gy, gw, gh) = (num(off, "x") / EMU_PER_PT, num(off, "y") / EMU_PER_PT, num(ext, "cx") / EMU_PER_PT, num(ext, "cy") / EMU_PER_PT);
                let id: u32 = m.child("wps:cNvPr").and_then(|p| p.attr("id")).and_then(|v| v.parse().ok()).unwrap_or(0);
                let (w, h) = (gw * scale.0, gh * scale.1);
                let world = valid.then(|| World {
                    frame: frame.clone(),
                    geo: Geo {
                        x: origin.0 + scale.0 * gx,
                        y: origin.1 + scale.1 * gy,
                        w,
                        h,
                        rot: num(x, "rot") / 60000.0,
                        flip_h: truthy(x.and_then(|x| x.attr("flipH"))),
                        flip_v: truthy(x.and_then(|x| x.attr("flipV"))),
                    },
                    origin,
                    scale,
                });
                out.push(Found { id, top: false, world, sites: connector::wsp_sites(m, w, h), conn: conn_of(m) });
            }
            "wpg:grpSp" | "wpg:wgp" => {
                let x = m.child("wpg:grpSpPr").and_then(|s| s.child("a:xfrm"));
                let (off, ext, ch_off, ch_ext) = (x.and_then(|x| x.child("a:off")), x.and_then(|x| x.child("a:ext")), x.and_then(|x| x.child("a:chOff")), x.and_then(|x| x.child("a:chExt")));
                let (gw, gh) = (num(ext, "cx"), num(ext, "cy"));
                let (cw, ch) = (num(ch_ext, "cx"), num(ch_ext, "cy"));
                let k = (if cw > 0.0 { gw / cw } else { 1.0 }, if ch > 0.0 { gh / ch } else { 1.0 });
                let still = num(x, "rot") == 0.0 && !truthy(x.and_then(|x| x.attr("flipH"))) && !truthy(x.and_then(|x| x.attr("flipV")));
                // The group's own place in its container, then what its members count from.
                let (gx, gy) = (num(off, "x") / EMU_PER_PT, num(off, "y") / EMU_PER_PT);
                let (cox, coy) = (num(ch_off, "x") / EMU_PER_PT, num(ch_off, "y") / EMU_PER_PT);
                let o = (origin.0 + scale.0 * (gx - cox * k.0), origin.1 + scale.1 * (gy - coy * k.1));
                members(m, o, (scale.0 * k.0, scale.1 * k.1), frame, valid && still, depth + 1, out);
            }
            _ => {}
        }
    }
}

/// Every shape in the blocks. A shape anchored to a paragraph counts that paragraph in its frame.
fn scan<'a>(blocks: impl IntoIterator<Item = &'a Block>) -> Vec<Found> {
    let mut out = Vec::new();
    story::walk_objects(blocks, &mut |pid, e| describe(pid, e, &mut out));
    out
}

type SiteAt = (Frame, (f64, f64), Dir);

/// One end of a connector: what it is glued to, where it is and which way the site faces.
type ResolvedEnd = (Option<Attach>, Option<(f64, f64)>, Option<Dir>);

/// What was noted before a batch of ops.
pub(super) struct ConnSnapshot {
    sites: HashMap<(u32, usize), SiteAt>,
    /// The two ends of each connector.
    conns: HashMap<u32, ConnPre>,
}

/// A connector before the ops: its frame, its two ends and what they were glued to.
struct ConnPre {
    frame: Frame,
    start: (f64, f64),
    end: (f64, f64),
    glued: [Option<Attach>; 2],
}

fn dist(a: (f64, f64), b: (f64, f64)) -> f64 {
    (a.0 - b.0).hypot(a.1 - b.1)
}

/// What to do to a connector after a batch.
struct Action {
    id: u32,
    top: bool,
    route: Option<Route>,
    start: Option<Option<Attach>>,
    end: Option<Option<Attach>>,
}

/// Note where the sites that connectors hold are in a list of blocks (the body or a story), before ops run. None when no
/// connector is glued to anything.
pub(super) fn connectors_before<'a>(blocks: impl IntoIterator<Item = &'a Block>) -> Option<ConnSnapshot> {
    let found = scan(blocks);
    let mut needed: HashSet<(u32, usize)> = HashSet::new();
    for f in &found {
        if let Some(c) = &f.conn {
            for a in [c.start, c.end].into_iter().flatten() {
                needed.insert((a.id, a.idx));
            }
        }
    }
    if needed.is_empty() {
        return None;
    }
    let mut sites = HashMap::new();
    for f in &found {
        for idx in 0..f.sites.len() {
            if needed.contains(&(f.id, idx)) {
                if let Some(s) = f.site(idx) {
                    sites.entry((f.id, idx)).or_insert(s);
                }
            }
        }
    }
    let mut conns = HashMap::new();
    for f in &found {
        if f.conn.as_ref().is_some_and(|c| c.start.is_some() || c.end.is_some()) {
            if let Some(w) = &f.world {
                let (s, e) = connector::ends(&w.geo);
                conns.entry(f.id).or_insert(ConnPre { frame: w.frame.clone(), start: s, end: e, glued: [f.conn.as_ref().and_then(|c| c.start), f.conn.as_ref().and_then(|c| c.end)] });
            }
        }
    }
    Some(ConnSnapshot { sites, conns })
}

/// After ops ran: route again the connectors whose shapes moved, and let go of shapes that are gone. Returns the
/// paragraphs that changed.
pub(super) fn connectors_after(blocks: &mut [Block], before: Option<ConnSnapshot>) -> Vec<NodeId> {
    let actions = plan(&*blocks, before);
    apply(blocks, &actions)
}

/// `connectors_after` for the body: only the blocks an action changes are copied from an undo step that shares them.
pub(super) fn connectors_after_body(body: &mut Body, before: Option<ConnSnapshot>) -> Vec<NodeId> {
    let actions = plan(&*body, before);
    if actions.is_empty() {
        return Vec::new();
    }
    let mut touched = Vec::new();
    body.edit_each(|s| {
        let t = apply(s, &actions);
        let any = !t.is_empty();
        touched.extend(t);
        any
    });
    touched
}

/// What the connectors need after the ops: the actions to apply.
fn plan<'a>(blocks: impl IntoIterator<Item = &'a Block>, before: Option<ConnSnapshot>) -> Vec<Action> {
    let Some(before) = before else { return Vec::new() };
    let found = scan(blocks);
    let mut by_id: HashMap<u32, &Found> = HashMap::new();
    let mut uses: HashMap<u32, usize> = HashMap::new();
    for f in &found {
        by_id.entry(f.id).or_insert(f);
        *uses.entry(f.id).or_default() += 1;
    }
    let mut actions: Vec<Action> = Vec::new();
    for c in &found {
        let Some(conn) = &c.conn else { continue };
        if conn.start.is_none() && conn.end.is_none() {
            continue;
        }
        // Files don't guarantee unique drawing ids. Two connectors with the same id can't be told apart here (what was
        // noted, and the action that follows, go by id), so they are left alone rather than guessed at.
        if uses.get(&c.id).copied().unwrap_or(0) > 1 {
            continue;
        }
        let Some(w) = &c.world else {
            // A connector the engine can't place: it only lets go of shapes that are gone.
            let gone = |a: Option<Attach>| a.is_some_and(|a| !by_id.contains_key(&a.id));
            if gone(conn.start) || gone(conn.end) {
                actions.push(Action { id: c.id, top: c.top, route: None, start: gone(conn.start).then_some(None), end: gone(conn.end).then_some(None) });
            }
            continue;
        };
        let now = connector::ends(&w.geo);
        let pre_c = before.conns.get(&c.id).filter(|p| p.frame == w.frame);
        let pre = pre_c.map(|p| (p.start, p.end)).unwrap_or(now);
        let mut reroute = false;
        let mut detach = [false, false];
        let mut point = [now.0, now.1];
        let mut dir: [Option<Dir>; 2] = [None, None];
        for (i, a) in [conn.start, conn.end].into_iter().enumerate() {
            let (was, is) = if i == 0 { (pre.0, now.0) } else { (pre.1, now.1) };
            let Some(a) = a else { continue };
            // A shape whose id another shape shares: which one the end is glued to is not known.
            if uses.get(&a.id).copied().unwrap_or(0) > 1 {
                continue;
            }
            let Some(shape) = by_id.get(&a.id) else {
                detach[i] = true;
                continue;
            };
            let Some(site) = shape.site(a.idx) else { continue };
            // Moved by an op, while still glued to what it was glued to: it was dragged away from the shape.
            // An end given a new site by the op is not that.
            let moved = dist(was, is) > 0.01 && pre_c.is_some_and(|p| p.glued[i] == Some(a));
            let changed = before.sites.get(&(a.id, a.idx)).filter(|p| p.0 == site.0).map(|p| dist(p.1, site.1) > 0.01 || p.2 != site.2).unwrap_or(false);
            if changed {
                // The end goes where it was, plus as far as the site moved.
                let pre_site = before.sites[&(a.id, a.idx)].1;
                point[i] = (was.0 + site.1 .0 - pre_site.0, was.1 + site.1 .1 - pre_site.1);
                dir[i] = Some(site.2);
                reroute = true;
            } else if moved {
                // The connector itself was moved away from the shape.
                detach[i] = true;
            } else {
                dir[i] = Some(site.2);
            }
        }
        let start = detach[0].then_some(None);
        let end = detach[1].then_some(None);
        let route = if reroute && (w.scale.0 - w.scale.1).abs() < 1e-9 && w.scale.0 > 0.0 {
            let (ds0, de0) = connector::directions(&conn.prst, &conn.adj, &w.geo);
            let kind = Kind::of(&conn.prst).unwrap_or(Kind::Straight);
            let straight = if conn.prst.is_empty() { "straightConnector1" } else { conn.prst.as_str() };
            let mut r = connector::route(kind, straight, point[0], Some(dir[0].unwrap_or(ds0)), point[1], Some(dir[1].unwrap_or(de0)));
            // Back into the connector's own container.
            let k = w.scale.0;
            r.geo.x = (r.geo.x - w.origin.0) / k;
            r.geo.y = (r.geo.y - w.origin.1) / k;
            r.geo.w /= k;
            r.geo.h /= k;
            Some(r)
        } else {
            None
        };
        if route.is_some() || start.is_some() || end.is_some() {
            actions.push(Action { id: c.id, top: c.top, route, start, end });
        }
    }
    actions
}

fn apply(blocks: &mut [Block], actions: &[Action]) -> Vec<NodeId> {
    if actions.is_empty() {
        return Vec::new();
    }
    let mut touched = Vec::new();
    story::walk_objects_mut(blocks, &mut |pid, obj| {
        for act in actions {
            if apply_action(obj, act) {
                touched.push(pid);
            }
        }
    });
    touched
}

/// The `wps:wsp` with this drawing id inside an object of the text (a shape on its own, or a member).
fn find_wsp_mut(obj: &mut Element, id: u32, top: bool) -> Option<&mut Element> {
    let drawing = drawing_mut(obj)?;
    if top {
        let own = drawing.find("wp:docPr").and_then(|p| p.attr("id")).and_then(|v| v.parse::<u32>().ok());
        if own != Some(id) {
            return None;
        }
        return find_mut(drawing, "wps:wsp");
    }
    fn go(e: &mut Element, id: u32) -> Option<&mut Element> {
        for c in e.elements_mut() {
            if c.name == "wps:wsp" && c.child("wps:cNvPr").and_then(|p| p.attr("id")).and_then(|v| v.parse::<u32>().ok()) == Some(id) {
                return Some(c);
            }
            if let Some(f) = go(c, id) {
                return Some(f);
            }
        }
        None
    }
    go(drawing, id)
}

fn apply_action(obj: &mut Element, act: &Action) -> bool {
    if act.top {
        // The position lives in the object's anchor: check it is this object before writing.
        let is_it = drawing_mut(obj).and_then(|d| d.find("wp:docPr").and_then(|p| p.attr("id")).and_then(|v| v.parse::<u32>().ok())) == Some(act.id);
        if !is_it {
            return false;
        }
        if let Some(r) = &act.route {
            if connector::write(obj, r, Where::Top).is_err() {
                return false;
            }
        }
        if let Some(wsp) = find_wsp_mut(obj, act.id, true) {
            connector::set_attachments(wsp, act.start, act.end);
        }
        return true;
    }
    let Some(wsp) = find_wsp_mut(obj, act.id, false) else { return false };
    if let Some(r) = &act.route {
        let _ = connector::write(wsp, r, Where::Member);
    }
    connector::set_attachments(wsp, act.start, act.end);
    true
}

// ---------------------------------------------------------------------------
// Ops
// ---------------------------------------------------------------------------

/// The line a new connector gets when the op says nothing: black, 1.5 points.
fn default_line() -> Option<build::LineLook> {
    Some(build::LineLook { color: "000000".into(), width_pt: 1.5, ..Default::default() })
}

impl Document {
    pub(super) fn apply_connector_op(&mut self, op: &Op) -> Result<OpResult, EditError> {
        match op {
            Op::InsertConnector { at, kind, start, end, from, to, line } => self.op_insert_connector(*at, kind, *start, *end, *from, *to, line.as_ref()),
            Op::SetConnector { at, kind, start, end, from, to } => self.op_set_connector(*at, kind.as_deref(), *start, *end, *from, *to),
            _ => unreachable!("not a connector op"),
        }
    }

    /// The shape at a position as connectors see it. A shape without a drawing id, or with one another drawing
    /// has too, gets a fresh one, as Word does when it connects to it.
    fn connectable(&mut self, pos: Pos) -> Result<(Attach, Found), EditError> {
        let invalid = |m: &str| EditError::Invalid(m.into());
        let el = {
            let p = self.para_ref(pos.p.0)?;
            edit::object_at(&p.inlines, pos.o).cloned().ok_or_else(|| invalid("there is no shape at that position"))?
        };
        let mut found = Vec::new();
        describe(pos.p.0, &el, &mut found);
        let f = found.into_iter().next().filter(|f| f.top).ok_or_else(|| invalid("only a shape on its own in the text can be connected, not a group or a member of one"))?;
        let shapes = scan(&self.body);
        let clash = f.id == 0 || shapes.iter().filter(|s| s.id == f.id).count() > 1;
        let mut f = f;
        if clash {
            self.drawing_id += 1;
            let id = self.drawing_id;
            self.touch(pos.p.0);
            let p = self.para_mut(pos.p.0)?;
            if let Some(obj) = edit::object_at_mut(&mut p.inlines, pos.o) {
                set_drawing_id(obj, id);
            }
            f.id = id;
        }
        Ok((Attach { id: f.id, idx: 0 }, f))
    }

    /// One end of a connector: what it is glued to, where it is (a point of the connector's frame) and which way its
    /// site faces.
    fn resolve_end(&mut self, spec: Option<EndSpec>, given: Option<[f64; 2]>, frame: &Frame) -> Result<ResolvedEnd, EditError> {
        let given = given.map(|p| (p[0], p[1]));
        match spec {
            Some(EndSpec::To { shape, idx }) => {
                let (att, f) = self.connectable(shape)?;
                let site = f.site(idx).ok_or_else(|| EditError::Invalid(format!("that shape has no connection site {idx}")))?;
                let pos = match given {
                    Some(p) => p,
                    None if site.0 == *frame => site.1,
                    None => return Err(EditError::Invalid("give from and to: the shape is placed from something other than the connector's paragraph".into())),
                };
                Ok((Some(Attach { id: att.id, idx }), Some(pos), Some(site.2)))
            }
            _ => Ok((None, given, None)),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn op_insert_connector(&mut self, at: Pos, kind: &str, start: Option<EndSpec>, end: Option<EndSpec>, from: Option<[f64; 2]>, to: Option<[f64; 2]>, line: Option<&crate::shapes::LineSpec>) -> Result<OpResult, EditError> {
        let invalid = EditError::Invalid;
        if self.active_part != self.main_path {
            return Err(invalid("that can only be inserted in the main text".into()));
        }
        let kind = Kind::parse(kind).map_err(invalid)?;
        let line = build::line_look(line, default_line()).map_err(EditError::Invalid)?;
        // A new connector hangs from its paragraph by the column and the paragraph's top.
        let frame = Frame("column".into(), "paragraph".into(), at.p.0);
        let (sa, s, ds) = self.resolve_end(start, from, &frame)?;
        let (ea, e, de) = self.resolve_end(end, to, &frame)?;
        let (s, e) = (s.ok_or_else(|| invalid("give where the connector starts (from) or a shape to start on".into()))?, e.ok_or_else(|| invalid("give where the connector ends (to) or a shape to end on".into()))?);
        if [s.0, s.1, e.0, e.1].iter().any(|v| !v.is_finite() || v.abs() > 1e5) {
            return Err(invalid("the connector has a point off the page".into()));
        }
        let route = connector::route(kind, "straightConnector1", s, ds, e, de);
        self.drawing_id += 1;
        let spec = connector::Spec { route, line, start: sa, end: ea, id: self.drawing_id };
        let el = connector::build(&spec);
        build::ensure_root_ns(&mut self.doc_xml.root);
        self.touch(at.p.0);
        let p = self.para_mut(at.p.0)?;
        let off = at.o.min(p.len16());
        let run = Run { t_attrs: None, attrs: vec![], rpr: None, items: vec![RunItem::Object(el)] };
        edit::insert_inline_at(&mut p.inlines, off, Inline::Run(run));
        Ok(OpResult { caret: Some(Pos::new(at.p.0, off + 1)), ..Default::default() })
    }

    fn op_set_connector(&mut self, at: Pos, kind: Option<&str>, start: Option<EndSpec>, end: Option<EndSpec>, from: Option<[f64; 2]>, to: Option<[f64; 2]>) -> Result<OpResult, EditError> {
        let invalid = |m: &str| EditError::Invalid(m.into());
        let el = {
            let p = self.para_ref(at.p.0)?;
            edit::object_at(&p.inlines, at.o).cloned().ok_or_else(|| invalid("there is no connector at that position"))?
        };
        let mut found = Vec::new();
        describe(at.p.0, &el, &mut found);
        let me = found.into_iter().next().filter(|f| f.top && f.conn.is_some()).ok_or_else(|| invalid("that is not a connector on its own in the text"))?;
        let conn = me.conn.clone().unwrap();
        let world = me.world.clone().ok_or_else(|| invalid("a connector placed by alignment can't be changed; move it first"))?;
        let kind = match kind {
            Some(k) => Kind::parse(k).map_err(EditError::Invalid)?,
            None => Kind::of(&conn.prst).unwrap_or(Kind::Straight),
        };
        let (cs, ce) = connector::ends(&world.geo);
        let (ds0, de0) = connector::directions(&conn.prst, &conn.adj, &world.geo);
        // Ends that stay glued keep following their site; the ends the op names are looked up.
        let mut ends = [(conn.start, cs, Some(ds0)), (conn.end, ce, Some(de0))];
        let specs = [(start, from), (end, to)];
        let shapes = scan(&self.body);
        for (i, (spec, given)) in specs.into_iter().enumerate() {
            match spec {
                Some(EndSpec::Free) => ends[i] = (None, given.map(|p| (p[0], p[1])).unwrap_or(ends[i].1), ends[i].2),
                Some(s @ EndSpec::To { .. }) => {
                    let (a, p, d) = self.resolve_end(Some(s), given, &world.frame)?;
                    ends[i] = (a, p.unwrap_or(ends[i].1), d);
                }
                None => {
                    if let Some(p) = given {
                        // Dragged to a free place: it lets go of its shape.
                        ends[i] = (None, (p[0], p[1]), ends[i].2);
                    } else if let Some(a) = ends[i].0 {
                        if let Some((_, _, d)) = shapes.iter().find(|s| s.id == a.id).and_then(|s| s.site(a.idx)) {
                            ends[i].2 = Some(d);
                        }
                    }
                }
            }
        }
        let straight = if conn.prst.is_empty() { "straightConnector1" } else { conn.prst.as_str() };
        let route = connector::route(kind, straight, ends[0].1, ends[0].2, ends[1].1, ends[1].2);
        self.touch(at.p.0);
        let p = self.para_mut(at.p.0)?;
        let obj = edit::object_at_mut(&mut p.inlines, at.o).ok_or_else(|| invalid("there is no connector at that position"))?;
        connector::write(obj, &route, Where::Top).map_err(EditError::Invalid)?;
        if let Some(wsp) = find_mut(drawing_mut(obj).ok_or_else(|| invalid("not a connector"))?, "wps:wsp") {
            connector::set_attachments(wsp, Some(ends[0].0), Some(ends[1].0));
        }
        Ok(OpResult { caret: Some(at), ..Default::default() })
    }

    /// The connection sites of the shape at a position, in points from the top-left of its box (not turned or mirrored).
    pub fn shape_sites(&self, at: Pos) -> Vec<Site> {
        let (blocks, _) = self.locate(at.p.0);
        let Some(p) = find_para(blocks, at.p.0) else { return vec![] };
        let Some(e) = shape_object(&p.inlines, at.o) else { return vec![] };
        let mut found = Vec::new();
        describe(at.p.0, e, &mut found);
        found.into_iter().next().filter(|f| f.top && f.conn.is_none()).map(|f| f.sites).unwrap_or_default()
    }
}

/// The inline object occupying `[off, off+1)`.
fn shape_object(inlines: &[Inline], off: usize) -> Option<&Element> {
    fn go<'a>(inlines: &'a [Inline], off: usize, start: &mut usize) -> Option<&'a Element> {
        for inl in inlines {
            let len = inl.len16();
            if off >= *start && off < *start + len {
                return match inl {
                    Inline::Run(r) => {
                        let mut pos = *start;
                        for item in &r.items {
                            let l = item.len16();
                            if pos == off && l == 1 {
                                if let RunItem::Object(e) = item {
                                    return Some(e);
                                }
                            }
                            pos += l;
                        }
                        None
                    }
                    Inline::Container(c) => go(&c.children, off, start),
                    Inline::Opaque(_) => None,
                };
            }
            *start += len;
        }
        None
    }
    go(inlines, off, &mut 0)
}

fn set_drawing_id(obj: &mut Element, id: u32) {
    if let Some(d) = drawing_mut(obj) {
        if let Some(p) = find_mut(d, "wp:docPr") {
            p.set_attr("id", id.to_string());
        }
    }
}
