//! Violations of the invariant check (see `invariants`) that are known bugs, so `cargo test` stays green while the
//! list says what is broken. Each entry is (rule, a part of the message, why). Fix the bug, then delete the entry.
//! Only a debug build (`cargo test`) looks at this list; the browser build with the switch on reports everything.
//! Never allow a whole rule: name the message. Every entry is a bug traced in the code, not a guess.

use crate::invariants::Violation;

const KNOWN: &[(&str, &str, &str)] = &[
    (
        "E1",
        "undeclared namespace prefix",
        "damaged_input_never_panics: the lenient XML reader keeps an attribute whose prefix was damaged (`8:val` or `:val` for `w:val`, from a \
         flipped byte) and the repaired document is saved with it, so the saved part does not pass the namespace check and Word \
         would refuse it (E1, E20).",
    ),
];

pub fn is_known(v: &Violation) -> bool {
    KNOWN.iter().any(|(rule, part, _)| v.rule == *rule && v.message.contains(part))
}
