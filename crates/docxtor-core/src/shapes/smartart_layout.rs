//! SmartArt layouts drawn without Word's pre-rendered drawing.
//!
//! Word saves a SmartArt graphic with a drawing part (`dsp:drawing`) that holds the shapes it laid out; Docxtor
//! draws that. When the part is missing (a file from another producer, or one that was stripped) the shapes are
//! laid out here from the data model: the nodes, their order and their depth. A handful of Word's layouts are
//! drawn natively (basic block list, basic process, basic cycle, vertical bullet list and a hierarchy); every
//! other layout falls back to the basic block list.
//!
//! Everything here is geometry in points: no XML, no theme. `smartart` turns the boxes into shapes.

use std::f64::consts::{PI, TAU};

/// A node of the data model with the nodes below it.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Node {
    /// The text, one line per paragraph.
    pub text: String,
    pub children: Vec<Node>,
}

impl Node {
    pub fn new(text: &str) -> Node {
        Node { text: text.to_string(), children: Vec::new() }
    }

    pub fn with(mut self, children: Vec<Node>) -> Node {
        self.children = children;
        self
    }
}

/// The layouts drawn natively.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Basic block list (`default`): a grid of equal boxes. Also the fallback for every other layout.
    BlockList,
    /// Basic process (`process1`): boxes in a row with an arrow between each pair.
    Process,
    /// Basic cycle (`cycle2`, and `cycle1`): ellipses on a circle with an arrow between each pair.
    Cycle,
    /// Vertical bullet list (`vList2`, `list1`): a bar for each node with its sub-nodes under it.
    VerticalList,
    /// Hierarchy and organization chart (`hierarchy1`, `orgChart1`): a tree with elbow connectors.
    Hierarchy,
}

/// The layout for a layout's `uniqueId` (`urn:microsoft.com/office/officeart/2005/8/layout/process1`).
/// `None` for a layout this module does not draw; callers then use `Kind::BlockList`.
pub fn native_kind(layout_id: &str) -> Option<Kind> {
    let name = layout_id.rsplit('/').next().unwrap_or(layout_id).trim();
    Some(match name {
        "default" => Kind::BlockList,
        "process1" => Kind::Process,
        "cycle2" | "cycle1" => Kind::Cycle,
        "vList2" | "list1" => Kind::VerticalList,
        "hierarchy1" | "orgChart1" => Kind::Hierarchy,
        _ => return None,
    })
}

/// What a box is for, which decides its colours.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// A node of the diagram; `index` counts the top-level nodes.
    Node,
    /// The lighter box that holds the sub-nodes of a node in a vertical list.
    Sub,
    /// An arrow between two nodes.
    Arrow,
    /// A line of a connector.
    Line,
}

/// One shape of a laid out diagram: a preset geometry in a box, in points from the frame's top-left corner.
#[derive(Debug, Clone, PartialEq)]
pub struct Piece {
    pub geometry: &'static str,
    /// Adjust values by name (`adj`), in DrawingML units.
    pub adj: Vec<(&'static str, f64)>,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    /// Degrees clockwise.
    pub rot: f64,
    /// The text of the shape, one paragraph per line. Empty for arrows and lines.
    pub text: Vec<Line>,
    pub role: Role,
    /// The top-level node this belongs to (decides the colour).
    pub index: usize,
    /// The text size that fits, in points.
    pub size: f64,
}

/// A paragraph of a shape's text.
#[derive(Debug, Clone, PartialEq)]
pub struct Line {
    pub text: String,
    /// A sub-node: smaller, with a bullet.
    pub sub: bool,
}

impl Piece {
    fn new(geometry: &'static str, x: f64, y: f64, w: f64, h: f64, role: Role, index: usize) -> Piece {
        Piece { geometry, adj: Vec::new(), x, y, w, h, rot: 0.0, text: Vec::new(), role, index, size: 0.0 }
    }

    fn node(geometry: &'static str, x: f64, y: f64, w: f64, h: f64, index: usize, n: &Node) -> Piece {
        let mut p = Piece::new(geometry, x, y, w, h, Role::Node, index);
        p.text = lines_of(n);
        p.size = fit_size(&p.text, w * 0.9, h * 0.85, 8.0, 28.0);
        p
    }
}

/// The paragraphs of a node and, below it, those of its sub-nodes.
fn lines_of(n: &Node) -> Vec<Line> {
    let mut out: Vec<Line> = n.text.split('\n').filter(|l| !l.is_empty()).map(|l| Line { text: l.to_string(), sub: false }).collect();
    for c in &n.children {
        for l in c.text.split('\n').filter(|l| !l.is_empty()) {
            out.push(Line { text: l.to_string(), sub: true });
        }
    }
    out
}

/// Lay out `nodes` in a frame of `w` by `h` points.
pub fn layout(kind: Kind, nodes: &[Node], w: f64, h: f64) -> Vec<Piece> {
    if nodes.is_empty() || w <= 0.0 || h <= 0.0 {
        return Vec::new();
    }
    match kind {
        Kind::BlockList => block_list(nodes, w, h),
        Kind::Process => process(nodes, w, h),
        Kind::Cycle => cycle(nodes, w, h),
        Kind::VerticalList => vertical_list(nodes, w, h),
        Kind::Hierarchy => hierarchy(nodes, w, h),
    }
}

// ---------------------------------------------------------------------------
// Text size
// ---------------------------------------------------------------------------

/// The largest size from `max` down to `min` at which the lines wrap to fit a box of `w` by `h` points. Letters
/// are taken as half the size wide, as Calibri roughly is.
pub fn fit_size(lines: &[Line], w: f64, h: f64, min: f64, max: f64) -> f64 {
    if lines.is_empty() {
        return max.min(18.0);
    }
    let mut size = max;
    while size > min {
        if text_height(lines, w, size) <= h {
            return size;
        }
        size -= 1.0;
    }
    min
}

/// How tall the lines are when wrapped to `w` points at `size`; sub-lines are 80% of the size.
fn text_height(lines: &[Line], w: f64, size: f64) -> f64 {
    let mut total = 0.0;
    for l in lines {
        let s = if l.sub { size * 0.8 } else { size };
        let per_line = (w / (s * 0.5)).floor().max(1.0) as usize;
        let mut count = 0usize;
        let mut used = 0usize;
        count += 1;
        for word in l.text.split_whitespace() {
            let n = word.chars().count();
            // A word longer than the line cannot be made to fit by wrapping.
            if n > per_line {
                return f64::MAX;
            }
            if used == 0 {
                used = n;
            } else if used + 1 + n <= per_line {
                used += 1 + n;
            } else {
                count += 1;
                used = n;
            }
        }
        total += count as f64 * s * 1.2;
    }
    total
}

// ---------------------------------------------------------------------------
// Basic block list
// ---------------------------------------------------------------------------

/// Columns and box width of Word's snake layout: boxes 0.6 as tall as wide, gaps a tenth of the width.
pub fn block_grid(n: usize, w: f64, h: f64) -> (usize, f64) {
    let mut best = (1, 0.0f64);
    for cols in 1..=n.max(1) {
        let rows = n.div_ceil(cols);
        let by_w = w / (cols as f64 + 0.1 * (cols as f64 - 1.0));
        let by_h = h / (0.6 * rows as f64 + 0.1 * (rows as f64 - 1.0));
        let bw = by_w.min(by_h);
        if bw > best.1 + 1e-9 {
            best = (cols, bw);
        }
    }
    best
}

fn block_list(nodes: &[Node], w: f64, h: f64) -> Vec<Piece> {
    let n = nodes.len();
    let (cols, bw) = block_grid(n, w, h);
    let (bh, gap) = (bw * 0.6, bw * 0.1);
    let rows = n.div_ceil(cols);
    let total_h = rows as f64 * bh + (rows as f64 - 1.0) * gap;
    let top = (h - total_h) / 2.0;
    let mut out = Vec::new();
    for (i, node) in nodes.iter().enumerate() {
        let (r, c) = (i / cols, i % cols);
        // The last row is centred when it is not full.
        let in_row = if r == rows - 1 { n - r * cols } else { cols };
        let row_w = in_row as f64 * bw + (in_row as f64 - 1.0) * gap;
        let left = (w - row_w) / 2.0;
        out.push(Piece::node("rect", left + c as f64 * (bw + gap), top + r as f64 * (bh + gap), bw, bh, i, node));
    }
    out
}

// ---------------------------------------------------------------------------
// Basic process
// ---------------------------------------------------------------------------

fn process(nodes: &[Node], w: f64, h: f64) -> Vec<Piece> {
    let n = nodes.len() as f64;
    // Boxes 0.6 as tall as wide, the space between two boxes 0.4 of the width.
    let a = (w / (n + (n - 1.0) * 0.4)).min(h / 0.6);
    let (bh, gap) = (a * 0.6, a * 0.4);
    let total = n * a + (n - 1.0) * gap;
    let (left, top) = ((w - total) / 2.0, (h - bh) / 2.0);
    let mut out = Vec::new();
    for (i, node) in nodes.iter().enumerate() {
        let x = left + i as f64 * (a + gap);
        let mut p = Piece::node("roundRect", x, top, a, bh, i, node);
        p.adj.push(("adj", 10000.0));
        out.push(p);
        if i + 1 < nodes.len() {
            let (aw, ah) = (gap * 0.53, gap * 0.6);
            let mut arrow = Piece::new("rightArrow", x + a + (gap - aw) / 2.0, top + (bh - ah) / 2.0, aw, ah, Role::Arrow, i);
            arrow.adj = vec![("adj1", 60000.0), ("adj2", 50000.0)];
            out.push(arrow);
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Basic cycle
// ---------------------------------------------------------------------------

/// The radius of the circle the node centres sit on and the diameter of a node, for `n` nodes in a square of
/// side `s`.
pub fn cycle_metrics(n: usize, s: f64) -> (f64, f64) {
    if n <= 1 {
        return (0.0, s * 0.6);
    }
    // A node is 0.6 of the distance between two neighbours; the nodes and the circle fit the square.
    let k = 0.6;
    let sin = (PI / n as f64).sin();
    let r = s / 2.0 / (1.0 + k * sin);
    (r, (k * 2.0 * r * sin).min(s * 0.45))
}

fn cycle(nodes: &[Node], w: f64, h: f64) -> Vec<Piece> {
    let n = nodes.len();
    let s = w.min(h);
    let (r, d) = cycle_metrics(n, s);
    let (cx, cy) = (w / 2.0, h / 2.0);
    // The first node is at the top; the others follow clockwise.
    let at = |i: usize| -PI / 2.0 + TAU * i as f64 / n as f64;
    let mut out = Vec::new();
    for (i, node) in nodes.iter().enumerate() {
        let a = at(i);
        let (x, y) = (cx + r * a.cos() - d / 2.0, cy + r * a.sin() - d / 2.0);
        out.push(Piece::node("ellipse", x, y, d, d, i, node));
    }
    if n > 1 {
        let chord = 2.0 * r * (PI / n as f64).sin();
        let len = ((chord - d) * 0.6).max(d * 0.25);
        for i in 0..n {
            let mid = at(i) + TAU / n as f64 / 2.0;
            let (mx, my) = (cx + r * mid.cos(), cy + r * mid.sin());
            let (aw, ah) = (len, len * 1.1);
            let mut arrow = Piece::new("rightArrow", mx - aw / 2.0, my - ah / 2.0, aw, ah, Role::Arrow, i);
            arrow.adj = vec![("adj1", 60000.0), ("adj2", 50000.0)];
            // Along the circle, clockwise.
            arrow.rot = (mid.to_degrees() + 90.0).rem_euclid(360.0);
            out.push(arrow);
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Vertical bullet list
// ---------------------------------------------------------------------------

fn vertical_list(nodes: &[Node], w: f64, h: f64) -> Vec<Piece> {
    // Weights: a bar is 1, the box under it a half and a third of a bar for each line.
    let sub_lines = |n: &Node| n.children.iter().map(|c| c.text.split('\n').filter(|l| !l.is_empty()).count()).sum::<usize>();
    let weight = |n: &Node| 1.0 + if n.children.is_empty() { 0.0 } else { 0.25 + 0.4 * sub_lines(n).max(1) as f64 };
    let gap_w = 0.12;
    let total: f64 = nodes.iter().map(weight).sum::<f64>() + gap_w * (nodes.len() as f64 - 1.0);
    let unit = (h / total).min(w * 0.22);
    let top = (h - unit * total) / 2.0;
    let mut y = top;
    let mut out = Vec::new();
    for (i, n) in nodes.iter().enumerate() {
        let mut bar = Piece::new("roundRect", 0.0, y, w, unit, Role::Node, i);
        bar.adj = vec![("adj", 16667.0)];
        bar.text = n.text.split('\n').filter(|l| !l.is_empty()).map(|l| Line { text: l.to_string(), sub: false }).collect();
        bar.size = fit_size(&bar.text, w * 0.9, unit * 0.8, 8.0, 28.0);
        out.push(bar);
        y += unit;
        if !n.children.is_empty() {
            let bh = unit * (weight(n) - 1.0);
            let mut sub = Piece::new("rect", w * 0.05, y, w * 0.9, bh, Role::Sub, i);
            sub.text = n.children.iter().flat_map(|c| c.text.split('\n').filter(|l| !l.is_empty()).map(|l| Line { text: l.to_string(), sub: true })).collect();
            sub.size = fit_size(&sub.text, w * 0.85, bh * 0.85, 8.0, 24.0);
            out.push(sub);
            y += bh;
        }
        y += unit * gap_w;
    }
    out
}

// ---------------------------------------------------------------------------
// Hierarchy
// ---------------------------------------------------------------------------

struct Slot {
    x: f64,
    depth: usize,
}

/// Slots (in box widths, 0 for the first leaf) of a tree: leaves one after another, a parent over its children.
fn place_tree(n: &Node, depth: usize, next: &mut f64, out: &mut Vec<Slot>) -> usize {
    let at = out.len();
    out.push(Slot { x: 0.0, depth });
    if n.children.is_empty() {
        out[at].x = *next;
        *next += 1.2;
    } else {
        let kids: Vec<usize> = n.children.iter().map(|c| place_tree(c, depth + 1, next, out)).collect();
        out[at].x = (out[kids[0]].x + out[*kids.last().unwrap()].x) / 2.0;
    }
    at
}

fn depth_of(n: &Node) -> usize {
    1 + n.children.iter().map(depth_of).max().unwrap_or(0)
}

fn hierarchy(nodes: &[Node], w: f64, h: f64) -> Vec<Piece> {
    // More than one top-level node: they are the children of an unseen root.
    let root = if nodes.len() == 1 { nodes[0].clone() } else { Node { text: String::new(), children: nodes.to_vec() } };
    let hidden_root = nodes.len() != 1;
    let mut slots = Vec::new();
    let mut next = 0.0;
    place_tree(&root, 0, &mut next, &mut slots);
    let levels = depth_of(&root) - hidden_root as usize;
    let span = next - 0.2;
    // A box is 0.6 as tall as wide, and the levels are 0.4 of the width apart.
    let a = (w / span).min(h / (0.6 * levels as f64 + 0.4 * (levels as f64 - 1.0))).max(1.0);
    let (bh, vgap) = (a * 0.6, a * 0.4);
    let left = (w - span * a) / 2.0;
    let top = (h - (levels as f64 * bh + (levels as f64 - 1.0) * vgap)) / 2.0;
    let mut out = Vec::new();
    let mut counter = 0usize;
    let frame = Frame { left, top, a, bh, vgap, skip: hidden_root as usize };
    emit_tree(&root, 0, hidden_root, &slots, &mut counter, &mut out, &frame);
    out
}

struct Frame {
    left: f64,
    top: f64,
    a: f64,
    bh: f64,
    vgap: f64,
    /// Levels above the first drawn one (1 when the root is not drawn).
    skip: usize,
}

/// Put the boxes and connector lines of a subtree in `out`; returns the slot index of its root.
fn emit_tree(n: &Node, depth: usize, hidden: bool, slots: &[Slot], counter: &mut usize, out: &mut Vec<Piece>, f: &Frame) -> usize {
    let at = *counter;
    *counter += 1;
    let s = &slots[at];
    let (x, y) = (f.left + s.x * f.a, f.top + (s.depth - f.skip.min(s.depth)) as f64 * (f.bh + f.vgap));
    // The root is not drawn when there are several top-level nodes.
    let drawn = depth > 0 || !hidden;
    if drawn {
        out.push(Piece::node("rect", x, y, f.a, f.bh, 0, &Node { text: n.text.clone(), children: Vec::new() }));
    }
    let kids: Vec<usize> = n.children.iter().map(|c| emit_tree(c, depth + 1, hidden, slots, counter, out, f)).collect();
    if drawn && !kids.is_empty() {
        let (px, py) = (x + f.a / 2.0, y + f.bh);
        let mid = py + f.vgap / 2.0;
        let child_x: Vec<f64> = kids.iter().map(|k| f.left + slots[*k].x * f.a + f.a / 2.0).collect();
        let line = |x: f64, y: f64, w: f64, h: f64| Piece::new("line", x, y, w, h, Role::Line, 0);
        out.push(line(px, py, 0.0, f.vgap / 2.0));
        let lo = child_x.iter().cloned().fold(px, f64::min);
        let hi = child_x.iter().cloned().fold(px, f64::max);
        if hi - lo > 0.01 {
            out.push(line(lo, mid, hi - lo, 0.0));
        }
        let child_top = f.top + (s.depth + 1 - f.skip) as f64 * (f.bh + f.vgap);
        for cx in child_x {
            out.push(line(cx, mid, 0.0, child_top - mid));
        }
    }
    at
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(n: &[&str]) -> Vec<Node> {
        n.iter().map(|t| Node::new(t)).collect()
    }

    #[test]
    fn layouts_are_known_by_their_ids() {
        assert_eq!(native_kind("urn:microsoft.com/office/officeart/2005/8/layout/default"), Some(Kind::BlockList));
        assert_eq!(native_kind("urn:microsoft.com/office/officeart/2005/8/layout/process1"), Some(Kind::Process));
        assert_eq!(native_kind("urn:microsoft.com/office/officeart/2005/8/layout/cycle2"), Some(Kind::Cycle));
        assert_eq!(native_kind("urn:microsoft.com/office/officeart/2005/8/layout/vList2"), Some(Kind::VerticalList));
        assert_eq!(native_kind("urn:microsoft.com/office/officeart/2005/8/layout/orgChart1"), Some(Kind::Hierarchy));
        assert_eq!(native_kind("urn:microsoft.com/office/officeart/2005/8/layout/pyramid1"), None);
    }

    #[test]
    fn block_grid_picks_the_biggest_boxes() {
        // Three boxes in a wide, short frame sit in one row.
        let (cols, bw) = block_grid(3, 432.0, 100.0);
        assert_eq!(cols, 3);
        assert!((bw - 432.0 / 3.2).abs() < 1e-6);
        // In Word's usual 6 by 3.5 inch frame, two over one is bigger.
        let (cols, _) = block_grid(3, 432.0, 252.0);
        assert_eq!(cols, 2);
        // In a tall frame they stack.
        let (cols, _) = block_grid(3, 100.0, 400.0);
        assert_eq!(cols, 1);
    }

    #[test]
    fn block_list_centres_the_last_row() {
        let p = layout(Kind::BlockList, &names(&["a", "b", "c", "d", "e"]), 432.0, 252.0);
        assert_eq!(p.len(), 5);
        let (cols, bw) = block_grid(5, 432.0, 252.0);
        assert_eq!(cols, 3);
        // The two boxes of the last row are centred as a pair.
        let row_w = 2.0 * bw + 0.1 * bw;
        assert!((p[3].x - (432.0 - row_w) / 2.0).abs() < 1e-6);
        assert!((p[3].y - p[4].y).abs() < 1e-9 && p[3].y > p[0].y);
        // All boxes are the same size and inside the frame.
        for b in &p {
            assert!((b.w - bw).abs() < 1e-9 && (b.h - bw * 0.6).abs() < 1e-9);
            assert!(b.x >= -1e-9 && b.x + b.w <= 432.0 + 1e-6 && b.y >= -1e-9 && b.y + b.h <= 252.0 + 1e-6);
        }
    }

    #[test]
    fn process_puts_an_arrow_between_boxes() {
        let p = layout(Kind::Process, &names(&["Plan", "Do", "Check"]), 432.0, 200.0);
        assert_eq!(p.iter().filter(|b| b.role == Role::Node).count(), 3);
        assert_eq!(p.iter().filter(|b| b.role == Role::Arrow).count(), 2);
        let boxes: Vec<&Piece> = p.iter().filter(|b| b.role == Role::Node).collect();
        // The row fills the frame's width.
        assert!((boxes[0].x).abs() < 1e-6);
        assert!((boxes[2].x + boxes[2].w - 432.0).abs() < 1e-6);
        // Each arrow sits in the gap between two boxes.
        let arrows: Vec<&Piece> = p.iter().filter(|b| b.role == Role::Arrow).collect();
        assert!(arrows[0].x > boxes[0].x + boxes[0].w && arrows[0].x + arrows[0].w < boxes[1].x);
    }

    #[test]
    fn cycle_puts_nodes_on_a_circle() {
        let p = layout(Kind::Cycle, &names(&["a", "b", "c", "d"]), 300.0, 300.0);
        let nodes: Vec<&Piece> = p.iter().filter(|b| b.role == Role::Node).collect();
        assert_eq!(nodes.len(), 4);
        // Every centre is the same distance from the middle; the first is at the top.
        let r: Vec<f64> = nodes.iter().map(|b| ((b.x + b.w / 2.0 - 150.0).powi(2) + (b.y + b.h / 2.0 - 150.0).powi(2)).sqrt()).collect();
        assert!(r.iter().all(|v| (v - r[0]).abs() < 1e-6));
        assert!((nodes[0].x + nodes[0].w / 2.0 - 150.0).abs() < 1e-6 && nodes[0].y + nodes[0].h / 2.0 < 150.0);
        // An arrow follows each node, and nodes stay inside the frame.
        assert_eq!(p.iter().filter(|b| b.role == Role::Arrow).count(), 4);
        for b in &nodes {
            assert!(b.x >= -1e-6 && b.y >= -1e-6 && b.x + b.w <= 300.0 + 1e-6 && b.y + b.h <= 300.0 + 1e-6);
        }
        // The arrow between the first two nodes points down and to the right: turned about 45 degrees.
        let arrow = p.iter().find(|b| b.role == Role::Arrow).unwrap();
        assert!((arrow.rot - 45.0).abs() < 1e-6);
    }

    #[test]
    fn vertical_list_has_a_bar_per_node_and_a_box_for_sub_nodes() {
        let nodes = vec![Node::new("One").with(vec![Node::new("a"), Node::new("b")]), Node::new("Two")];
        let p = layout(Kind::VerticalList, &nodes, 300.0, 300.0);
        assert_eq!(p.iter().filter(|b| b.role == Role::Node).count(), 2);
        let subs: Vec<&Piece> = p.iter().filter(|b| b.role == Role::Sub).collect();
        assert_eq!(subs.len(), 1);
        assert_eq!(subs[0].text.len(), 2);
        assert!(p.iter().all(|b| b.y >= -1e-6 && b.y + b.h <= 300.0 + 1e-6));
    }

    #[test]
    fn hierarchy_centres_a_parent_over_its_children() {
        let tree = vec![Node::new("Boss").with(vec![Node::new("A"), Node::new("B"), Node::new("C")])];
        let p = layout(Kind::Hierarchy, &tree, 400.0, 300.0);
        let boxes: Vec<&Piece> = p.iter().filter(|b| b.role == Role::Node).collect();
        assert_eq!(boxes.len(), 4);
        let boss = boxes.iter().find(|b| b.text[0].text == "Boss").unwrap();
        let b = boxes.iter().find(|b| b.text[0].text == "B").unwrap();
        assert!((boss.x + boss.w / 2.0 - (b.x + b.w / 2.0)).abs() < 1e-6);
        assert!(boss.y < b.y);
        assert!(p.iter().any(|l| l.role == Role::Line));
        for b in &boxes {
            assert!(b.x >= -1e-6 && b.x + b.w <= 400.0 + 1e-6);
        }
    }

    #[test]
    fn several_top_level_nodes_share_an_unseen_root() {
        let p = layout(Kind::Hierarchy, &names(&["A", "B"]), 300.0, 200.0);
        assert_eq!(p.iter().filter(|b| b.role == Role::Node).count(), 2);
    }

    #[test]
    fn text_gets_smaller_to_fit() {
        let short = vec![Line { text: "Hi".into(), sub: false }];
        let long = vec![Line { text: "A rather long sentence that needs several lines to fit".into(), sub: false }];
        let a = fit_size(&short, 100.0, 50.0, 8.0, 28.0);
        let b = fit_size(&long, 100.0, 50.0, 8.0, 28.0);
        assert!(a > b);
        assert!(b >= 8.0);
    }
}
