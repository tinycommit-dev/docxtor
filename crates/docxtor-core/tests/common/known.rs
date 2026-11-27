//! Failures the random op harness has found that are real bugs in the product, so `cargo test` stays green while this
//! list says what is broken. Each entry is (check, a pattern for the message, why). In a pattern `*` stands for any
//! text (names, ids and offsets vary); the check must be the same too. Fix the bug, then delete the entry.
//! Never allow a whole check: name the message. A mistake of the harness is fixed in the harness, not listed here.
//! The reason holds the smallest repro found (a start document and the ops) and where the cause is. "Cause not
//! traced" says the repro is real but the line is not known yet. (Violations the engine's own check reports are filtered by
//! `invariants_known.rs`, which the harness also honours.)

/// (check, message pattern, reason)
pub const KNOWN: &[(&str, &str, &str)] = &[
    // ---- E14 -------------------------------------------------------------------------------------------------------
];

/// A pattern where `*` stands for any text: every piece between the stars must appear, in order.
pub fn glob(pattern: &str, text: &str) -> bool {
    let mut rest = text;
    let pieces: Vec<&str> = pattern.split('*').collect();
    for (i, piece) in pieces.iter().enumerate() {
        if piece.is_empty() {
            continue;
        }
        match rest.find(piece) {
            Some(at) if i > 0 || pattern.starts_with('*') || at == 0 => rest = &rest[at + piece.len()..],
            _ => return false,
        }
    }
    true
}

pub fn is_known(check: &str, message: &str) -> bool {
    KNOWN.iter().any(|(c, pattern, _)| *c == check && glob(pattern, message))
}
