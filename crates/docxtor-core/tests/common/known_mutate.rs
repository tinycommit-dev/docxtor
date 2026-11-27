//! Failures the damaged-file harness (`tests/mutate_open.rs`) has found that are real bugs in the product, so
//! `cargo test` stays green while this list says what is broken. Each entry is (check, a pattern for the message,
//! why). In a pattern `*` stands for any text; the check must be the same too. The checks are `panic`, `stack`
//! (a stack overflow in a 1 MB stack, as WASM has), `memory` (more than 1 GB), `hang` (no answer inside the time
//! limit), `abort` (the process died some other way) and `text` (an error message that is an internal one or far
//! too long). A message reads: `<what happened> in <stage> (<entry point> <file> <part> <mutation>)`. Never allow a
//! whole check: name the message (what happened, the entry point and the kind of mutation). Fix the bug, then
//! delete the entry. The one `Budget` is the fix for most of them. A hang entry also covers a case that is only slow in a debug build.

/// (check, message pattern, reason)
pub const KNOWN: &[(&str, &str, &str)] = &[
    (
        "memory",
        "*memory past*in render (rtf-paste*repeat-lit*",
        "Pasting RTF with 100,000 table rows: the engine holds about 16 KB for each paragraph or row, so drawing it passes the 1 GB limit. A scale limit, not a damaged-file bug (a 100,000-paragraph .docx needs 1.6 GB to open too); left as engine memory work.",
    ),
    // ---- hang: a lot of work --------------------------------------------------------------------------------------
    (
        "hang",
        "*no answer*(open *repeat*",
        "An .odt or Word 2003 XML file with 100,000 paragraphs (25 MB). It used to take 16 s: converting it pasted the blocks and looked up each one's top-level block from the top (fixed in `insert_blocks`, now about 1 s alone in a release build, linear). Over the time limit only with four workers at once or in a debug build. Not a hang; see the memory entry for the cost of a very long document.",
    ),
    (
        "hang",
        "*no answer*(rtf-paste*",
        "Pasting RTF with 100,000 table rows: a slow render that runs out of the memory limit. Not a damaged-file bug: the engine holds about 16 KB for each paragraph or row, so a 100,000-paragraph document needs 1.6 to 3 GB. Left as engine memory work.",
    ),
    (
        "hang",
        "*no answer*(ops-json*",
        "A 16 MB string put in a shape by an op: drawing it is slow in a debug build only (fine in release).",
    ),
    (
        "stack",
        "*stack overflow*in paste (html-paste*nest label=*",
        "HTML with 100,000 nested tables: the paste reader's own depth limits hold in a release build; a debug build's bigger frames overflow 1 MB first.",
    ),
];

pub fn is_known(check: &str, message: &str) -> bool {
    KNOWN.iter().any(|(c, pattern, _)| *c == check && crate::common::known::glob(pattern, message))
}
