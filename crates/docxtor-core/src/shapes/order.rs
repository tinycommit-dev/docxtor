//! The stacking order of floating objects (shapes, text boxes, charts and pictures).
//!
//! Word keeps it in `wp:anchor relativeHeight` (a whole number: the higher, the nearer the viewer) and
//! `behindDoc` (1 draws the object behind the text). Objects behind the text and objects in front of it are two
//! stacks, each ordered by `relativeHeight`; objects with the same value are drawn in document order. VML keeps
//! the same number in `z-index`, negative behind the text, so the fallback is kept in step.

use crate::xml::Element;

/// Word's first value, and the distance between the ones it hands out.
pub const Z_BASE: u64 = 251_658_240;
pub const Z_STEP: u64 = 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Order {
    Front,
    Back,
    Forward,
    Backward,
}

impl Order {
    pub fn parse(s: &str) -> Result<Order, String> {
        match s {
            "front" | "bring_to_front" | "top" => Ok(Order::Front),
            "back" | "send_to_back" | "bottom" => Ok(Order::Back),
            "forward" | "bring_forward" => Ok(Order::Forward),
            "backward" | "send_backward" => Ok(Order::Backward),
            other => Err(format!("unknown order '{other}'; use front, back, forward or backward")),
        }
    }
}

/// Where a floating object sits in the stacking order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stack {
    pub z: u64,
    pub behind: bool,
}

const VML_SHAPES: &[&str] = &["v:rect", "v:roundrect", "v:oval", "v:line", "v:shape", "v:group"];

fn truthy(v: Option<&str>) -> bool {
    matches!(v, Some("1" | "true" | "on"))
}

fn z_of_style(style: &str) -> Option<i64> {
    style.split(';').filter_map(|kv| kv.split_once(':')).find(|(k, _)| k.trim() == "z-index").and_then(|(_, v)| v.trim().parse().ok())
}

/// The stacking of an object, or None when it is in line with the text (or is not a drawing).
pub fn stack_of(e: &Element) -> Option<Stack> {
    if let Some(a) = e.find("wp:anchor") {
        return Some(Stack { z: a.attr("relativeHeight").and_then(|v| v.trim().parse().ok()).unwrap_or(0), behind: truthy(a.attr("behindDoc")) });
    }
    if e.find("wp:inline").is_some() {
        return None;
    }
    // VML only.
    let pict = if e.name == "w:pict" { e } else { e.child("w:pict")? };
    let shape = pict.elements().find(|c| VML_SHAPES.contains(&c.name.as_str()))?;
    let style = shape.attr("style")?;
    if !style.split(';').any(|kv| kv.trim() == "position:absolute") {
        return None;
    }
    let z = z_of_style(style).unwrap_or(1);
    Some(Stack { z: z.unsigned_abs(), behind: z < 0 })
}

fn put_z(style: &str, z: i64) -> String {
    let mut pairs: Vec<(String, String)> = style.split(';').filter_map(|kv| kv.split_once(':')).map(|(k, v)| (k.trim().to_string(), v.trim().to_string())).collect();
    match pairs.iter_mut().find(|(k, _)| k == "z-index") {
        Some(slot) => slot.1 = z.to_string(),
        None => pairs.push(("z-index".into(), z.to_string())),
    }
    pairs.iter().map(|(k, v)| format!("{k}:{v}")).collect::<Vec<_>>().join(";")
}

fn set_vml_z(pict: &mut Element, z: i64) {
    for sh in pict.elements_mut().filter(|c| VML_SHAPES.contains(&c.name.as_str())) {
        if let Some(style) = sh.attr("style").map(String::from) {
            if style.split(';').any(|kv| kv.trim() == "position:absolute") {
                sh.set_attr("style", put_z(&style, z));
            }
        }
    }
}

fn anchor_mut(e: &mut Element) -> Option<&mut Element> {
    for c in e.elements_mut() {
        if c.name == "wp:anchor" {
            return Some(c);
        }
        if let Some(f) = anchor_mut(c) {
            return Some(f);
        }
    }
    None
}

/// Give the object a stacking value, in the DrawingML and in the VML fallback.
pub fn set_z(e: &mut Element, z: u64) {
    let behind = stack_of(e).is_some_and(|s| s.behind);
    let signed = if behind { -(z as i64) } else { z as i64 };
    if let Some(a) = anchor_mut(e) {
        a.set_attr("relativeHeight", z.to_string());
        if let Some(fb) = e.child_mut("mc:Fallback") {
            for pict in fb.elements_mut().filter(|c| c.name == "w:pict") {
                set_vml_z(pict, signed);
            }
        }
    } else if e.name == "w:pict" {
        set_vml_z(e, signed);
    } else if let Some(p) = e.child_mut("w:pict") {
        set_vml_z(p, signed);
    }
}

/// New values for the objects whose value has to change so that `target` moves as asked. `z` has the value of
/// each object of one stack (in document order). Returns `(index, new value)` pairs.
pub fn reorder(z: &[u64], target: usize, order: Order) -> Vec<(usize, u64)> {
    let n = z.len();
    let mut sorted: Vec<usize> = (0..n).collect();
    sorted.sort_by_key(|&i| (z[i], i));
    let pos = sorted.iter().position(|&i| i == target).expect("target in stack");
    let mut next = sorted.clone();
    match order {
        Order::Front if pos + 1 < n => {
            next.remove(pos);
            next.push(target);
        }
        Order::Back if pos > 0 => {
            next.remove(pos);
            next.insert(0, target);
        }
        Order::Forward if pos + 1 < n => next.swap(pos, pos + 1),
        Order::Backward if pos > 0 => next.swap(pos, pos - 1),
        _ => return Vec::new(),
    }
    let distinct = sorted.windows(2).all(|w| z[w[0]] < z[w[1]]);
    let max = z.iter().copied().max().unwrap_or(0);
    let min = z.iter().copied().min().unwrap_or(0);
    // The cheap ways first: only the moved object changes (to the top or the bottom), or two neighbours swap.
    match order {
        Order::Front if max + Z_STEP <= u32::MAX as u64 && (distinct || z[target] != max) => return vec![(target, max + Z_STEP)],
        Order::Back if min >= Z_STEP && (distinct || z[target] != min) => return vec![(target, min - Z_STEP)],
        Order::Forward | Order::Backward if distinct => {
            let other = if order == Order::Forward { sorted[pos + 1] } else { sorted[pos - 1] };
            return vec![(target, z[other]), (other, z[target])];
        }
        _ => {}
    }
    // Ties, or no room: number the stack again, in the new order.
    let base = if n as u64 * Z_STEP + Z_BASE < u32::MAX as u64 { Z_BASE } else { Z_STEP };
    let mut out = Vec::new();
    for (rank, &i) in next.iter().enumerate() {
        let v = base + Z_STEP * rank as u64;
        if z[i] != v {
            out.push((i, v));
        }
    }
    out
}

/// A temporary marker on the object an op works on, to find it again while walking the document.
pub const PICK: &str = "data-docxtor-pick";

/// An object that must float for an ordering op to mean anything.
pub fn require_floating(e: &Element) -> Result<Stack, String> {
    stack_of(e).ok_or_else(|| "only an object that floats over the text can be moved forward or back; give it a wrapping other than in line first".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn apply(z: &mut [u64], changes: Vec<(usize, u64)>) {
        for (i, v) in changes {
            z[i] = v;
        }
    }

    fn mv(z: &mut [u64], target: usize, order: Order) {
        let changes = reorder(z, target, order);
        apply(z, changes);
    }

    fn order_of(z: &[u64]) -> Vec<usize> {
        let mut s: Vec<usize> = (0..z.len()).collect();
        s.sort_by_key(|&i| (z[i], i));
        s
    }

    #[test]
    fn front_changes_only_the_moved_object() {
        let mut z = vec![10_000, 20_000, 30_000];
        let ch = reorder(&z, 0, Order::Front);
        assert_eq!(ch, vec![(0, 31_024)]);
        apply(&mut z, ch);
        assert_eq!(order_of(&z), vec![1, 2, 0]);
    }

    #[test]
    fn back_goes_below_the_lowest_or_renumbers() {
        let mut z = vec![10_000, 20_000, 30_000];
        mv(&mut z, 2, Order::Back);
        assert_eq!(order_of(&z), vec![2, 0, 1]);
        // No room under 0: everything is numbered again.
        let mut z = vec![0, 5, 9];
        mv(&mut z, 2, Order::Back);
        assert_eq!(order_of(&z), vec![2, 0, 1]);
    }

    #[test]
    fn forward_and_backward_swap_neighbours() {
        let mut z = vec![100, 200, 300];
        mv(&mut z, 0, Order::Forward);
        assert_eq!(order_of(&z), vec![1, 0, 2]);
        mv(&mut z, 2, Order::Backward);
        assert_eq!(order_of(&z), vec![1, 2, 0]);
        // Already at the end: nothing to do.
        assert!(reorder(&z, 0, Order::Front).is_empty());
        assert!(reorder(&z, 1, Order::Back).is_empty());
    }

    #[test]
    fn equal_values_are_told_apart() {
        // Three objects with the same value are drawn in document order; bringing the first forward puts it second.
        let mut z = vec![251_658_240, 251_658_240, 251_658_240];
        mv(&mut z, 0, Order::Forward);
        assert_eq!(order_of(&z), vec![1, 0, 2]);
        let mut z = vec![7, 7];
        mv(&mut z, 0, Order::Front);
        assert_eq!(order_of(&z), vec![1, 0]);
    }
}
