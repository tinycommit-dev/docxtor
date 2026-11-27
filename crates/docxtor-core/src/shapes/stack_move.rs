//! Moving one object to a place in the stacking order (the Selection pane's drag): directly above or below
//! another object of the same stack. See `order` for the stacking values and the one-step moves.

use super::order::{Z_BASE, Z_STEP};

/// New stacking values so that `target` sits directly above (`above`) or below `anchor`. `z` has the value of each
/// object of one stack, in document order; objects with equal values are drawn in document order. Returns
/// `(index, new value)` pairs, as few as it can manage.
pub fn place(z: &[u64], target: usize, anchor: usize, above: bool) -> Vec<(usize, u64)> {
    let n = z.len();
    if target == anchor || target >= n || anchor >= n {
        return Vec::new();
    }
    let mut sorted: Vec<usize> = (0..n).collect();
    sorted.sort_by_key(|&i| (z[i], i));
    let mut next: Vec<usize> = sorted.iter().copied().filter(|&i| i != target).collect();
    let at = next.iter().position(|&i| i == anchor).expect("anchor in stack");
    next.insert(if above { at + 1 } else { at }, target);
    if next == sorted {
        return Vec::new();
    }
    // The cheap way: only the moved object changes, to a value between its new neighbours. It works when the
    // others are strictly ordered by value already (no ties to tell apart by document order).
    let others: Vec<usize> = next.iter().copied().filter(|&i| i != target).collect();
    let distinct = others.windows(2).all(|w| z[w[0]] < z[w[1]]);
    if distinct {
        let pos = next.iter().position(|&i| i == target).expect("target placed");
        let lo = pos.checked_sub(1).map(|i| z[next[i]]);
        let hi = next.get(pos + 1).map(|&i| z[i]);
        let v = match (lo, hi) {
            (Some(l), Some(h)) if h > l + 1 => Some(l + (h - l) / 2),
            (Some(l), None) if l + Z_STEP <= u32::MAX as u64 => Some(l + Z_STEP),
            (None, Some(h)) if h > Z_STEP => Some(h - Z_STEP),
            (None, None) => None,
            _ => None,
        };
        if let Some(v) = v {
            return vec![(target, v)];
        }
    }
    // Ties, or no room: number the stack again, in the new order.
    let base = if n as u64 * Z_STEP + Z_BASE < u32::MAX as u64 { Z_BASE } else { Z_STEP };
    next.iter().enumerate().filter_map(|(rank, &i)| {
        let v = base + Z_STEP * rank as u64;
        (z[i] != v).then_some((i, v))
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn order_of(z: &[u64]) -> Vec<usize> {
        let mut s: Vec<usize> = (0..z.len()).collect();
        s.sort_by_key(|&i| (z[i], i));
        s
    }

    fn run(z: &mut [u64], target: usize, anchor: usize, above: bool) {
        for (i, v) in place(z, target, anchor, above) {
            z[i] = v;
        }
    }

    #[test]
    fn lands_between_neighbours_changing_only_itself() {
        let mut z = vec![1000, 2000, 3000, 4000];
        let ch = place(&z, 0, 2, true);
        assert_eq!(ch.len(), 1);
        run(&mut z, 0, 2, true);
        assert_eq!(order_of(&z), vec![1, 2, 0, 3]);
        run(&mut z, 3, 1, false);
        assert_eq!(order_of(&z), vec![3, 1, 2, 0]);
    }

    #[test]
    fn to_the_ends() {
        let mut z = vec![5000, 6000, 7000];
        run(&mut z, 0, 2, true);
        assert_eq!(order_of(&z), vec![1, 2, 0]);
        run(&mut z, 0, 1, false);
        assert_eq!(order_of(&z), vec![0, 1, 2]);
    }

    #[test]
    fn no_room_or_ties_number_the_stack_again() {
        let mut z = vec![10, 11, 12];
        run(&mut z, 0, 2, true);
        assert_eq!(order_of(&z), vec![1, 2, 0]);
        let mut z = vec![7, 7, 7];
        run(&mut z, 2, 0, false);
        assert_eq!(order_of(&z), vec![2, 0, 1]);
    }

    #[test]
    fn staying_put_changes_nothing() {
        let z = vec![1000, 2000, 3000];
        assert!(place(&z, 1, 0, true).is_empty());
        assert!(place(&z, 1, 2, false).is_empty());
        assert!(place(&z, 1, 1, true).is_empty());
    }
}
