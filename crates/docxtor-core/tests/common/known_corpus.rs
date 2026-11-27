//! Failures the real-world corpus run (`tests/corpus.rs`) has found that are real bugs in the product, so `cargo test`
//! stays green while this list says what is broken. Each entry is (check, a pattern for the message, why). In a
//! pattern `*` stands for any text (numbers, names, ids); the check must be the same too. The checks are `panic`,
//! `stack`, `memory`, `hang`, `abort`, `budget` (opening took longer than the limit), `text` (an internal error
//! message), `E1` to `E6` and the other rules the random steps report. Never allow a whole check: name the message.
//! Fix the bug, then delete the entry.

/// (check, message pattern, reason)
pub const KNOWN: &[(&str, &str, &str)] = &[
    // Empty: every file of a run of all of them (`DOCXTOR_CORPUS=all`) passes every check. An E6 pattern starts
    // with `*`, since the message begins "open(save(doc)) renders differently".
];

pub fn is_known(check: &str, message: &str) -> bool {
    KNOWN.iter().any(|(c, pattern, _)| *c == check && crate::common::known::glob(pattern, message))
}
