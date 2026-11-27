//! Comparing two versions of a document: word-level changes for display (`compare_view`) and a Word
//! redline (`redline`): the newer document with tracked changes that show how it differs from the older one,
//! like Word's Review > Compare.
//!
//! Paragraphs of the two versions are lined up with the same LCS as `diff`, and removed and added runs of
//! paragraphs are paired up by how many words they share. Inside a paired paragraph the changes are found
//! per word, space and punctuation mark. Paragraphs whose text is the same but whose character or paragraph
//! formatting differs are noticed too: the view names what changed ("bold", "color", "align") and the redline
//! carries them as Word's tracked formatting changes (`w:rPrChange`, `w:pPrChange`).
//!
//! `combine` is the redline with the revised document as the base: tracked changes it already has stay as they are,
//! with their authors, and only the differences nobody tracked are marked.

use crate::diff::lcs_steps;
use crate::document::{DocError, Document};
use crate::edit;
use crate::model::*;
use crate::props::{put_ppr, ParaFmt, RunFmt};
use crate::xml::{Element, Node};
use serde::Serialize;
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Lining the paragraphs up
// ---------------------------------------------------------------------------

/// A paragraph of a document in reading order, with the table around it (the outermost one) if any.
struct Flat<'a> {
    p: &'a Paragraph,
    text: String,
    table: Option<NodeId>,
}

fn flatten<'a>(blocks: impl IntoIterator<Item = &'a Block>, table: Option<NodeId>, out: &mut Vec<Flat<'a>>) {
    flatten_as(blocks, table, out, false)
}

/// Like `flatten`. With `rejected`, a paragraph's text is what it was before its tracked changes (insertions left
/// out, deletions put back), and paragraphs that a tracked change added are left out.
fn flatten_as<'a>(blocks: impl IntoIterator<Item = &'a Block>, table: Option<NodeId>, out: &mut Vec<Flat<'a>>, rejected: bool) {
    for b in blocks {
        match b {
            Block::Para(p) if rejected => {
                if !mark_inserted(p) {
                    out.push(Flat { p, text: rejected_text(p), table });
                }
            }
            Block::Para(p) => out.push(Flat { p, text: p.plain_text(), table }),
            Block::Table(t) => {
                for c in t.rows().flat_map(|r| r.cells()) {
                    flatten_as(&c.blocks, Some(table.unwrap_or(t.id)), out, rejected);
                }
            }
            Block::Container(c) => flatten_as(&c.blocks, table, out, rejected),
            Block::Opaque(_) => {}
        }
    }
}

/// Whether a tracked change added the paragraph mark (`w:ins` in the mark's run properties).
fn mark_inserted(p: &Paragraph) -> bool {
    p.ppr.as_ref().and_then(|pp| pp.child("w:rPr")).is_some_and(|r| r.child("w:ins").is_some())
}

/// The text of every `w:delText` and `w:t` under an element.
fn deleted_text(e: &Element, out: &mut String) {
    for c in e.elements() {
        match c.name.as_str() {
            "w:delText" | "w:t" => out.push_str(&c.text()),
            "w:tab" => out.push('\t'),
            _ => deleted_text(c, out),
        }
    }
}

/// A paragraph's text as it was before its tracked changes: insertions are left out and deletions are back.
fn rejected_text(p: &Paragraph) -> String {
    fn go(inlines: &[Inline], out: &mut String) {
        for i in inlines {
            match i {
                Inline::Run(r) => out.push_str(&r.text().replace(OBJ, "")),
                Inline::Container(c) if c.kind == InlineKind::Insertion => {}
                Inline::Container(c) => go(&c.children, out),
                Inline::Opaque(e) if matches!(e.name.as_str(), "w:del" | "w:moveFrom") => deleted_text(e, out),
                Inline::Opaque(_) => {}
            }
        }
    }
    let mut out = String::new();
    go(&p.inlines, &mut out);
    out
}

/// Whether a paragraph holds tracked insertions or deletions.
fn has_tracked(p: &Paragraph) -> bool {
    fn go(inlines: &[Inline]) -> bool {
        inlines.iter().any(|i| match i {
            Inline::Run(_) => false,
            Inline::Container(c) => c.kind == InlineKind::Insertion || go(&c.children),
            Inline::Opaque(e) => matches!(e.name.as_str(), "w:del" | "w:ins" | "w:moveFrom" | "w:moveTo"),
        })
    }
    go(&p.inlines)
}

/// Indices into the old and new paragraph lists.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Item {
    Same(usize, usize),
    Changed(usize, usize),
    Added(usize),
    Removed(usize),
}

fn bag(s: &str) -> HashMap<String, usize> {
    let mut m = HashMap::new();
    for w in s.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()) {
        *m.entry(w.to_lowercase()).or_insert(0) += 1;
    }
    m
}

/// How alike two paragraphs are, 0 to 1 (the Dice coefficient over their words).
fn similarity(a: &HashMap<String, usize>, b: &HashMap<String, usize>) -> f64 {
    let (na, nb): (usize, usize) = (a.values().sum(), b.values().sum());
    if na + nb == 0 {
        return 0.0;
    }
    let common: usize = a.iter().map(|(w, n)| (*n).min(b.get(w).copied().unwrap_or(0))).sum();
    2.0 * common as f64 / (na + nb) as f64
}

/// Paragraphs that must be at least this alike to count as one paragraph that changed.
const PAIR_AT: f64 = 0.35;

fn resolve_gap(old: &[Flat], new: &[Flat], dels: &[usize], adds: &[usize], out: &mut Vec<Item>) {
    let pairable = |i: usize, j: usize| old[i].table.is_some() == new[j].table.is_some();
    let (n, m) = (dels.len(), adds.len());
    let mut pairs: Vec<(usize, usize)> = Vec::new();
    if n == 1 && m == 1 && pairable(dels[0], adds[0]) {
        pairs.push((0, 0));
    } else if n > 0 && m > 0 {
        let bo: Vec<_> = dels.iter().map(|&i| bag(&old[i].text)).collect();
        let bn: Vec<_> = adds.iter().map(|&j| bag(&new[j].text)).collect();
        // Cells of tables that sit in the same place line up even when their text is unrelated.
        let score = |a: usize, b: usize| {
            if !pairable(dels[a], adds[b]) {
                0.0
            } else if old[dels[a]].table.is_some() {
                0.5 + similarity(&bo[a], &bn[b])
            } else {
                similarity(&bo[a], &bn[b])
            }
        };
        if n * m <= 40_000 {
            // Best order-preserving matching: f[i][j] over the first i removed and j added paragraphs.
            let at = |i: usize, j: usize| i * (m + 1) + j;
            let mut f = vec![0.0f64; (n + 1) * (m + 1)];
            let mut how = vec![0u8; (n + 1) * (m + 1)];
            for i in 1..=n {
                for j in 1..=m {
                    let s = score(i - 1, j - 1);
                    let (mut best, mut h) = (f[at(i - 1, j)], 1u8);
                    if f[at(i, j - 1)] > best {
                        best = f[at(i, j - 1)];
                        h = 2;
                    }
                    if s >= PAIR_AT && f[at(i - 1, j - 1)] + s > best {
                        best = f[at(i - 1, j - 1)] + s;
                        h = 3;
                    }
                    f[at(i, j)] = best;
                    how[at(i, j)] = h;
                }
            }
            let (mut i, mut j) = (n, m);
            while i > 0 && j > 0 {
                match how[at(i, j)] {
                    3 => {
                        pairs.push((i - 1, j - 1));
                        i -= 1;
                        j -= 1;
                    }
                    2 => j -= 1,
                    _ => i -= 1,
                }
            }
            pairs.reverse();
        } else {
            for k in 0..n.min(m) {
                if score(k, k) >= PAIR_AT {
                    pairs.push((k, k));
                }
            }
        }
    }
    // Merge: removed paragraphs first, then added ones, before each pair and after the last.
    let (mut pi, mut pj) = (0, 0);
    for (a, b) in pairs.iter().copied().chain(std::iter::once((n, m))) {
        out.extend(dels[pi..a].iter().map(|&i| Item::Removed(i)));
        out.extend(adds[pj..b].iter().map(|&j| Item::Added(j)));
        if a < n && b < m {
            out.push(Item::Changed(dels[a], adds[b]));
        }
        pi = a + 1;
        pj = b + 1;
    }
}

fn align(old: &[Flat], new: &[Flat]) -> Vec<Item> {
    let a: Vec<String> = old.iter().map(|f| f.text.clone()).collect();
    let b: Vec<String> = new.iter().map(|f| f.text.clone()).collect();
    let steps = lcs_steps(&a, &b);
    let mut out = Vec::new();
    let mut k = 0;
    while k < steps.len() {
        if steps[k].0 == '=' {
            out.push(Item::Same(steps[k].1, steps[k].2));
            k += 1;
            continue;
        }
        let (mut dels, mut adds) = (Vec::new(), Vec::new());
        while k < steps.len() && steps[k].0 != '=' {
            match steps[k] {
                ('-', i, _) => dels.push(i),
                ('+', _, j) => adds.push(j),
                _ => {}
            }
            k += 1;
        }
        resolve_gap(old, new, &dels, &adds, &mut out);
    }
    out
}

// ---------------------------------------------------------------------------
// Words
// ---------------------------------------------------------------------------

/// One stretch of a paragraph that is kept, removed or added.
#[derive(Debug, Clone, PartialEq)]
pub enum Edit {
    Eq(String),
    Del(String),
    Ins(String),
}

/// Split text into words, runs of spaces and single punctuation marks.
fn tokens(s: &str) -> Vec<&str> {
    #[derive(PartialEq, Clone, Copy)]
    enum K {
        Word,
        Space,
        Other,
    }
    let kind = |c: char| if c.is_alphanumeric() || c == '_' { K::Word } else if c.is_whitespace() { K::Space } else { K::Other };
    let chars: Vec<(usize, char)> = s.char_indices().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let start = chars[i].0;
        let k = kind(chars[i].1);
        let mut j = i + 1;
        if k != K::Other {
            while j < chars.len() {
                let c = chars[j].1;
                let joins = kind(c) == k
                    // An apostrophe between letters stays inside the word (don't, it's).
                    || (k == K::Word && (c == '\'' || c == '\u{2019}') && chars.get(j + 1).is_some_and(|n| kind(n.1) == K::Word));
                if !joins {
                    break;
                }
                j += 1;
            }
        }
        let end = chars.get(j).map(|c| c.0).unwrap_or(s.len());
        out.push(&s[start..end]);
        i = j;
    }
    out
}

fn push_edit(out: &mut Vec<Edit>, e: Edit) {
    match (out.last_mut(), &e) {
        (Some(Edit::Eq(a)), Edit::Eq(b)) | (Some(Edit::Del(a)), Edit::Del(b)) | (Some(Edit::Ins(a)), Edit::Ins(b)) => a.push_str(b),
        _ => out.push(e),
    }
}

/// How `new` differs from `old`, word by word: kept, removed and added stretches in order, with the removed
/// text of a change before the added text. Changes separated only by spaces are joined into one.
pub fn word_edits(old: &str, new: &str) -> Vec<Edit> {
    let (a, b) = (tokens(old), tokens(new));
    let mut pre = 0;
    while pre < a.len() && pre < b.len() && a[pre] == b[pre] {
        pre += 1;
    }
    let mut suf = 0;
    while suf < a.len() - pre && suf < b.len() - pre && a[a.len() - 1 - suf] == b[b.len() - 1 - suf] {
        suf += 1;
    }
    let (am, bm) = (&a[pre..a.len() - suf], &b[pre..b.len() - suf]);
    // Token-level script for the middle.
    let mut script: Vec<(char, &str)> = Vec::new();
    if am.len().saturating_mul(bm.len()) <= 4_000_000 {
        let (n, m) = (am.len(), bm.len());
        let mut dp = vec![0u32; (n + 1) * (m + 1)];
        let at = |i: usize, j: usize| i * (m + 1) + j;
        for i in (0..n).rev() {
            for j in (0..m).rev() {
                dp[at(i, j)] = if am[i] == bm[j] { dp[at(i + 1, j + 1)] + 1 } else { dp[at(i + 1, j)].max(dp[at(i, j + 1)]) };
            }
        }
        let (mut i, mut j) = (0, 0);
        while i < n || j < m {
            if i < n && j < m && am[i] == bm[j] {
                script.push(('=', am[i]));
                i += 1;
                j += 1;
            } else if i < n && (j >= m || dp[at(i + 1, j)] >= dp[at(i, j + 1)]) {
                script.push(('-', am[i]));
                i += 1;
            } else {
                script.push(('+', bm[j]));
                j += 1;
            }
        }
    } else {
        script.extend(am.iter().map(|t| ('-', *t)));
        script.extend(bm.iter().map(|t| ('+', *t)));
    }
    // Segments: kept text, and changes (removed text, added text).
    enum Seg {
        Eq(String),
        Change(String, String),
    }
    let mut segs: Vec<Seg> = Vec::new();
    if pre > 0 {
        segs.push(Seg::Eq(a[..pre].concat()));
    }
    for (c, t) in script {
        match (c, segs.last_mut()) {
            ('=', Some(Seg::Eq(s))) => s.push_str(t),
            ('=', _) => segs.push(Seg::Eq(t.to_string())),
            ('-', Some(Seg::Change(d, _))) => d.push_str(t),
            ('+', Some(Seg::Change(_, i))) => i.push_str(t),
            ('-', _) => segs.push(Seg::Change(t.to_string(), String::new())),
            _ => segs.push(Seg::Change(String::new(), t.to_string())),
        }
    }
    if suf > 0 {
        match segs.last_mut() {
            Some(Seg::Eq(s)) => s.push_str(&a[a.len() - suf..].concat()),
            _ => segs.push(Seg::Eq(a[a.len() - suf..].concat())),
        }
    }
    // Join changes that only a space apart.
    let mut joined: Vec<Seg> = Vec::new();
    let mut k = 0;
    while k < segs.len() {
        match &segs[k] {
            Seg::Change(d, i) => {
                let (mut d, mut i) = (d.clone(), i.clone());
                while let (Some(Seg::Eq(sp)), Some(Seg::Change(d2, i2))) = (segs.get(k + 1), segs.get(k + 2)) {
                    if !sp.trim().is_empty() {
                        break;
                    }
                    d.push_str(sp);
                    d.push_str(d2);
                    i.push_str(sp);
                    i.push_str(i2);
                    k += 2;
                }
                joined.push(Seg::Change(d, i));
            }
            Seg::Eq(s) => joined.push(Seg::Eq(s.clone())),
        }
        k += 1;
    }
    let mut out = Vec::new();
    for seg in joined {
        match seg {
            Seg::Eq(s) => push_edit(&mut out, Edit::Eq(s)),
            Seg::Change(d, i) => {
                if !d.is_empty() {
                    push_edit(&mut out, Edit::Del(d));
                }
                if !i.is_empty() {
                    push_edit(&mut out, Edit::Ins(i));
                }
            }
        }
    }
    out
}

fn u16len(s: &str) -> usize {
    s.encode_utf16().count()
}

// ---------------------------------------------------------------------------
// Formatting
// ---------------------------------------------------------------------------

/// Whether a paragraph holds only text (and links around text): no objects, fields or existing tracked changes.
fn is_simple(inlines: &[Inline]) -> bool {
    inlines.iter().all(|i| match i {
        Inline::Run(r) => r.items.iter().all(|it| match it {
            RunItem::Text(_) | RunItem::Tab | RunItem::Break(_) => true,
            RunItem::Hidden(e) => matches!(e.name.as_str(), "w:lastRenderedPageBreak" | "w:softHyphen"),
            RunItem::Object(_) => false,
        }),
        Inline::Container(c) => matches!(c.kind, InlineKind::Hyperlink | InlineKind::Sdt | InlineKind::Other) && is_simple(&c.children),
        Inline::Opaque(e) => !matches!(e.name.as_str(), "w:del" | "w:ins" | "w:moveFrom" | "w:moveTo" | "m:oMath" | "m:oMathPara"),
    })
}

/// The properties (by their JSON names, such as `bold` or `spaceBefore`) in which two values differ.
fn differing<T: Serialize>(a: &T, b: &T) -> Vec<String> {
    let (Ok(serde_json::Value::Object(x)), Ok(serde_json::Value::Object(y))) = (serde_json::to_value(a), serde_json::to_value(b)) else {
        return Vec::new();
    };
    let mut keys: Vec<&String> = x.keys().chain(y.keys()).collect();
    keys.sort();
    keys.dedup();
    keys.into_iter().filter(|k| x.get(*k) != y.get(*k)).cloned().collect()
}

/// What differs between the direct formatting of two runs, by name. Empty when it is the same as far as Docxtor
/// models it.
fn run_fmt_names(old: Option<&Element>, new: Option<&Element>) -> Vec<String> {
    let a = old.map(RunFmt::from_rpr).unwrap_or_default();
    let b = new.map(RunFmt::from_rpr).unwrap_or_default();
    if a == b {
        return Vec::new();
    }
    named(differing(&a, &b))
}

/// The names to show for properties that differ: the complex-script variants count as the property itself,
/// and a difference Docxtor has no name for is "other".
fn named(names: Vec<String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for n in names {
        let n = match n.as_str() {
            "boldCs" => "bold".to_string(),
            "italicCs" => "italic".to_string(),
            "sizeCs" => "size".to_string(),
            "fontCs" | "fontCsTheme" | "fontTheme" => "font".to_string(),
            _ => n,
        };
        if !out.contains(&n) {
            out.push(n);
        }
    }
    if out.is_empty() {
        out.push("other".into());
    }
    out
}

/// What differs between the paragraph properties of two paragraphs, by name.
fn para_fmt_names(old: &Paragraph, new: &Paragraph) -> Vec<String> {
    let a = old.ppr.as_ref().map(ParaFmt::from_ppr).unwrap_or_default();
    let b = new.ppr.as_ref().map(ParaFmt::from_ppr).unwrap_or_default();
    if a == b {
        return Vec::new();
    }
    named(differing(&a, &b))
}

/// A stretch of text that kept its words and changed its formatting.
#[derive(Debug, Clone)]
struct FmtChange {
    /// Where the stretch starts in the newer paragraph (UTF-16) and how long it is.
    new: usize,
    len: usize,
    names: Vec<String>,
    /// The run properties the text had before.
    old_rpr: Option<Element>,
}

/// (start, length, direct run properties) of each run with text.
fn run_spans(p: &Paragraph) -> Vec<(usize, usize, Option<&Element>)> {
    let mut out = Vec::new();
    walk_runs(&p.inlines, &mut 0, &mut |r, start, _| {
        let len = r.len16();
        if len > 0 {
            out.push((start, len, r.rpr.as_ref()));
        }
    });
    out
}

fn span_at<'a>(spans: &[(usize, usize, Option<&'a Element>)], at: usize) -> Option<&'a Element> {
    spans.iter().find(|(s, l, _)| at >= *s && at < s + l).and_then(|(_, _, r)| *r)
}

fn rpr_xml(r: Option<&Element>) -> String {
    r.map(|e| e.elements().filter(|c| c.name != "w:rPrChange").map(|c| c.to_xml()).collect::<Vec<_>>().join("")).unwrap_or_default()
}

/// Formatting changes in the `len` characters that start at `o0` in `old` and at `n0` in `new`, which have the
/// same text. Only for paragraphs that are simple.
fn fmt_changes(old: &Paragraph, new: &Paragraph, o0: usize, n0: usize, len: usize) -> Vec<FmtChange> {
    if len == 0 || !is_simple(&old.inlines) || !is_simple(&new.inlines) {
        return Vec::new();
    }
    let (so, sn) = (run_spans(old), run_spans(new));
    let mut cuts = vec![0, len];
    cuts.extend(so.iter().map(|(s, _, _)| s.saturating_sub(o0)).filter(|c| *c > 0 && *c < len));
    cuts.extend(sn.iter().map(|(s, _, _)| s.saturating_sub(n0)).filter(|c| *c > 0 && *c < len));
    cuts.sort_unstable();
    cuts.dedup();
    let mut out: Vec<FmtChange> = Vec::new();
    for w in cuts.windows(2) {
        let (a, b) = (w[0], w[1]);
        let (ro, rn) = (span_at(&so, o0 + a), span_at(&sn, n0 + a));
        let names = run_fmt_names(ro, rn);
        if names.is_empty() {
            continue;
        }
        let old_rpr = ro.cloned();
        if let Some(last) = out.last_mut() {
            if last.new + last.len == n0 + a && last.names == names && rpr_xml(last.old_rpr.as_ref()) == rpr_xml(old_rpr.as_ref()) {
                last.len += b - a;
                continue;
            }
        }
        out.push(FmtChange { new: n0 + a, len: b - a, names, old_rpr });
    }
    out
}

// ---------------------------------------------------------------------------
// Comparison for display
// ---------------------------------------------------------------------------

/// A stretch of a paragraph: `same`, `ins` (only in the newer version), `del` (only in the older one) or `fmt`
/// (the same words with different character formatting; `fmt` names what changed).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ComparePart {
    pub t: &'static str,
    pub text: String,
    /// For a `fmt` part: the properties that changed (`bold`, `italic`, `underline`, `color`, `size`, `font`...).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub fmt: Vec<String>,
}

/// One paragraph of the comparison, in reading order. Removed paragraphs sit where they were removed.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompareBlock {
    /// `same`, `added`, `removed`, `changed` (the words changed) or `formatted` (the words are the same, the
    /// formatting is not).
    pub kind: &'static str,
    /// The text: one part for same, added and removed paragraphs (none when the paragraph is empty); the
    /// word-level changes for a changed one.
    pub parts: Vec<ComparePart>,
    /// The paragraph in the newer document; absent for a removed one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub p: Option<NodeId>,
    /// Position in the newer document's paragraphs (for a removed paragraph, where it would have been).
    pub index: usize,
    /// The paragraph is in a table.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub table: bool,
    /// The paragraph properties that changed (`style`, `align`, `indLeft`, `spaceBefore`...), for a paragraph found in both.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub para_fmt: Vec<String>,
}

fn part(t: &'static str, text: &str) -> Vec<ComparePart> {
    if text.is_empty() {
        Vec::new()
    } else {
        vec![ComparePart { t, text: text.to_string(), fmt: Vec::new() }]
    }
}

/// The text of a stretch of a paragraph as parts, the changed formatting marked. `at` is where the stretch
/// starts in the newer paragraph (UTF-16) and `changes` the formatting changes of that paragraph.
fn text_parts(text: &str, at: usize, changes: &[FmtChange], out: &mut Vec<ComparePart>) {
    let end = at + u16len(text);
    let mut pos = at;
    let slice = |from: usize, to: usize| &text[edit::utf16_to_byte(text, from - at)..edit::utf16_to_byte(text, to - at)];
    for c in changes.iter().filter(|c| c.new >= at && c.new + c.len <= end) {
        if c.new > pos {
            out.extend(part("same", slice(pos, c.new)));
        }
        out.push(ComparePart { t: "fmt", text: slice(c.new, c.new + c.len).to_string(), fmt: c.names.clone() });
        pos = c.new + c.len;
    }
    if pos < end {
        out.extend(part("same", slice(pos, end)));
    }
}

/// Every paragraph of `new` against `old`, with word-level changes for the paragraphs that changed and the
/// formatting changes of paragraphs found in both.
pub fn compare_view(old: &Document, new: &Document) -> Vec<CompareBlock> {
    let (mut fo, mut fn_) = (Vec::new(), Vec::new());
    flatten(&old.body, None, &mut fo);
    flatten(&new.body, None, &mut fn_);
    let mut out = Vec::new();
    let mut next_new = 0;
    for item in align(&fo, &fn_) {
        match item {
            Item::Same(i, j) => {
                let para_fmt = para_fmt_names(fo[i].p, fn_[j].p);
                let changes = fmt_changes(fo[i].p, fn_[j].p, 0, 0, u16len(&fn_[j].text));
                let mut block = CompareBlock { kind: "same", parts: part("same", &fn_[j].text), p: Some(fn_[j].p.id), index: j, table: fn_[j].table.is_some(), para_fmt };
                if !changes.is_empty() || !block.para_fmt.is_empty() {
                    block.kind = "formatted";
                    block.parts = Vec::new();
                    text_parts(&fn_[j].text, 0, &changes, &mut block.parts);
                }
                out.push(block);
                next_new = j + 1;
            }
            Item::Added(j) => {
                out.push(CompareBlock { kind: "added", parts: part("ins", &fn_[j].text), p: Some(fn_[j].p.id), index: j, table: fn_[j].table.is_some(), para_fmt: Vec::new() });
                next_new = j + 1;
            }
            Item::Removed(i) => {
                out.push(CompareBlock { kind: "removed", parts: part("del", &fo[i].text), p: None, index: next_new, table: fo[i].table.is_some(), para_fmt: Vec::new() });
            }
            Item::Changed(i, j) => {
                let mut parts = Vec::new();
                let mut pos = (0usize, 0usize);
                for e in word_edits(&fo[i].text, &fn_[j].text) {
                    match e {
                        Edit::Eq(t) => {
                            let n = u16len(&t);
                            let changes = fmt_changes(fo[i].p, fn_[j].p, pos.0, pos.1, n);
                            text_parts(&t, pos.1, &changes, &mut parts);
                            pos = (pos.0 + n, pos.1 + n);
                        }
                        Edit::Del(t) => {
                            pos.0 += u16len(&t);
                            parts.push(ComparePart { t: "del", text: t, fmt: Vec::new() });
                        }
                        Edit::Ins(t) => {
                            pos.1 += u16len(&t);
                            parts.push(ComparePart { t: "ins", text: t, fmt: Vec::new() });
                        }
                    }
                }
                out.push(CompareBlock { kind: "changed", parts, p: Some(fn_[j].p.id), index: j, table: fn_[j].table.is_some(), para_fmt: para_fmt_names(fo[i].p, fn_[j].p) });
                next_new = j + 1;
            }
        }
    }
    out
}

impl Document {
    /// Paragraph-level changes from `old` to this document.
    pub fn compare_summary(&self, old: &Document) -> crate::diff::DiffSummary {
        crate::diff::diff_documents(old, self)
    }

    /// Word-level comparison of this (newer) document against `old`; see [`compare_view`].
    pub fn compare_view(&self, old: &Document) -> Vec<CompareBlock> {
        compare_view(old, self)
    }
}

// ---------------------------------------------------------------------------
// The redline
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    /// Wrap every run.
    All,
    /// Wrap only runs of plain text, leaving objects, fields and anything else as the document has it.
    TextOnly,
}

#[derive(Clone, Copy)]
enum Action {
    Changed(usize),
    /// The words are the same as in the old paragraph (the index) but the formatting is not.
    Formatted(usize),
    Added,
}

struct Redliner<'a> {
    new: &'a Document,
    author: &'a str,
    date: &'a str,
    next_rev: i64,
    ids: IdGen,
    old_flat: Vec<Flat<'a>>,
    action: HashMap<NodeId, Action>,
    /// Leave paragraphs that already carry tracked changes as they are (see `combine`).
    keep_tracked: bool,
    /// Removed body paragraphs (indices into `old_flat`) to put before the block with this id.
    before: HashMap<NodeId, Vec<usize>>,
}

/// The largest `w:id` in some blocks.
fn max_w_id<'a>(blocks: impl IntoIterator<Item = &'a Block>) -> i64 {
    let mut max = 0;
    for b in blocks {
        let xml = write_block(b).to_xml();
        for part in xml.split(" w:id=\"").skip(1) {
            let digits: String = part.chars().take_while(|c| c.is_ascii_digit()).collect();
            max = max.max(digits.parse::<i64>().unwrap_or(0));
        }
    }
    max
}

fn wrappable(r: &Run, mode: Mode) -> bool {
    mode == Mode::All || (!r.items.is_empty() && r.items.iter().all(|it| matches!(it, RunItem::Text(_) | RunItem::Tab | RunItem::Break(_))))
}

impl<'a> Redliner<'a> {
    fn rev(&mut self, name: &'static str) -> Element {
        self.next_rev += 1;
        Element::new(name).with_attr("w:id", self.next_rev.to_string()).with_attr("w:author", self.author).with_attr("w:date", self.date)
    }

    /// Wrap the runs inside `[from, to)` of some inline content in insertions.
    fn wrap(&mut self, inlines: &mut Vec<Inline>, from: usize, to: usize, base: usize, mode: Mode, zero_width: bool) {
        let old = std::mem::take(inlines);
        let mut group: Vec<Inline> = Vec::new();
        let mut start = base;
        let flush = |me: &mut Self, group: &mut Vec<Inline>, inlines: &mut Vec<Inline>| {
            if !group.is_empty() {
                let shell = me.rev("w:ins");
                inlines.push(Inline::Container(InlineContainer { kind: InlineKind::Insertion, shell, content_in: None, children: std::mem::take(group) }));
            }
        };
        for inl in old {
            let len = inl.len16();
            let inside = start >= from && start + len <= to && (len > 0 || zero_width);
            match inl {
                Inline::Run(r) if inside && wrappable(&r, mode) => group.push(Inline::Run(r)),
                Inline::Container(mut c) if len > 0 && start < to && start + len > from => {
                    flush(self, &mut group, inlines);
                    // A link or field can't sit inside an insertion; its runs can.
                    if !(c.kind == InlineKind::Insertion || (mode == Mode::TextOnly && c.kind == InlineKind::Field)) {
                        self.wrap(&mut c.children, from, to, start, mode, zero_width);
                    }
                    inlines.push(Inline::Container(c));
                }
                other => {
                    flush(self, &mut group, inlines);
                    inlines.push(other);
                }
            }
            start += len;
        }
        flush(self, &mut group, inlines);
    }

    fn wrap_all(&mut self, inlines: &mut Vec<Inline>, mode: Mode) {
        self.wrap(inlines, 0, usize::MAX, 0, mode, true);
    }

    fn wrap_range(&mut self, inlines: &mut Vec<Inline>, from: usize, to: usize) {
        edit::split_at(inlines, from);
        edit::split_at(inlines, to);
        self.wrap(inlines, from, to, 0, Mode::All, false);
    }

    /// An old run as a run of deleted text; None if it holds no text.
    fn deleted_run(&self, r: &Run) -> Option<Element> {
        let mut el = Element::new("w:r");
        if let Some(rpr) = &r.rpr {
            let mut rpr = rpr.clone();
            // A character style the newer file lacks would point nowhere.
            if rpr.child("w:rStyle").and_then(|s| s.attr("w:val")).is_some_and(|id| !self.new.styles.exists(id)) {
                rpr.remove_children("w:rStyle");
            }
            rpr.remove_children("w:rPrChange");
            if !rpr.children.is_empty() {
                el.children.push(Node::Element(rpr));
            }
        }
        let before = el.children.len();
        for item in &r.items {
            match item {
                RunItem::Text(t) => {
                    let mut buf = String::new();
                    let flush = |buf: &mut String, el: &mut Element| {
                        if !buf.is_empty() {
                            el.children.push(Node::Element(Element::new("w:delText").with_attr("xml:space", "preserve").with_text(std::mem::take(buf))));
                        }
                    };
                    for ch in t.chars() {
                        if ch == '\u{2011}' {
                            flush(&mut buf, &mut el);
                            el.children.push(Node::Element(Element::new("w:noBreakHyphen")));
                        } else {
                            buf.push(ch);
                        }
                    }
                    flush(&mut buf, &mut el);
                }
                RunItem::Tab => el.children.push(Node::Element(Element::new("w:tab"))),
                RunItem::Break(e) => el.children.push(Node::Element(e.clone())),
                RunItem::Object(_) | RunItem::Hidden(_) => {}
            }
        }
        (el.children.len() > before).then_some(el)
    }

    fn collect_deleted(&self, inlines: &[Inline], out: &mut Vec<Node>) {
        for i in inlines {
            match i {
                Inline::Run(r) => out.extend(self.deleted_run(r).map(Node::Element)),
                Inline::Container(c) => self.collect_deleted(&c.children, out),
                Inline::Opaque(_) => {}
            }
        }
    }

    /// The text of `[from, to)` of an old paragraph as a `w:del`, with the old formatting.
    fn deleted(&mut self, old: &Paragraph, from: usize, to: usize) -> Option<Inline> {
        let slice = edit::slice(&old.inlines, from, to);
        let mut runs = Vec::new();
        self.collect_deleted(&slice, &mut runs);
        if runs.is_empty() {
            return None;
        }
        let mut del = self.rev("w:del");
        del.children = runs;
        Some(Inline::Opaque(del))
    }

    /// Mark a paragraph's own mark as inserted or deleted.
    fn mark(&mut self, p: &mut Paragraph, name: &'static str) {
        let ppr = p.ppr.get_or_insert_with(|| Element::new("w:pPr"));
        if ppr.child("w:rPr").is_none() {
            put_ppr(ppr, Element::new("w:rPr"));
        }
        if ppr.child("w:rPr").is_some_and(|r| r.child(name).is_some()) {
            return;
        }
        let el = self.rev(name);
        if let Some(rpr) = ppr.child_mut("w:rPr") {
            // Tracked changes of the mark come first in its properties.
            rpr.children.insert(0, Node::Element(el));
        }
    }

    /// An old paragraph, removed: its text as deleted runs and a deleted paragraph mark.
    /// An old paragraph's properties without anything the newer document can't make sense of: the section
    /// properties, earlier tracked changes, a style or list it doesn't have.
    fn usable_ppr(&self, src: &Element) -> Element {
        let mut ppr = src.clone();
        ppr.remove_children("w:sectPr");
        ppr.remove_children("w:pPrChange");
        if ppr.child("w:pStyle").and_then(|s| s.attr("w:val")).is_some_and(|id| !self.new.styles.exists(id)) {
            ppr.remove_children("w:pStyle");
        }
        if let Some(np) = ppr.child("w:numPr") {
            let id = np.child("w:numId").and_then(|n| n.attr("w:val")).and_then(|v| v.parse::<i64>().ok());
            let lvl = np.child("w:ilvl").and_then(|n| n.attr("w:val")).and_then(|v| v.parse::<i64>().ok()).unwrap_or(0);
            if id.is_none_or(|id| id == 0 || self.new.numbering.level(id, lvl).is_none()) {
                ppr.remove_children("w:numPr");
            }
        }
        ppr
    }

    fn deleted_para(&mut self, old: &Paragraph) -> Paragraph {
        let mut p = Paragraph::new(self.ids.fresh());
        if let Some(src) = &old.ppr {
            let mut ppr = self.usable_ppr(src);
            if let Some(rpr) = ppr.child_mut("w:rPr") {
                for n in ["w:ins", "w:del", "w:moveFrom", "w:moveTo", "w:rPrChange"] {
                    rpr.remove_children(n);
                }
            }
            p.ppr = Some(ppr);
        }
        self.mark(&mut p, "w:del");
        if let Some(del) = self.deleted(old, 0, old.len16()) {
            p.inlines.push(del);
        }
        p
    }

    /// A paragraph of the newer document that pairs with an old one with different text.
    fn changed_para(&mut self, p: &mut Paragraph, old: &Paragraph) {
        self.mark_para_format(p, old);
        // With tracked changes of its own, the paragraph is left as it is.
        if self.keep_tracked && has_tracked(p) {
            return;
        }
        if is_simple(&p.inlines) {
            let edits = word_edits(&old.text(), &p.text());
            // Formatting changes are found before the paragraph is changed, and written after.
            let mut formats = Vec::new();
            let mut pos = (0usize, 0usize);
            for e in &edits {
                match e {
                    Edit::Eq(t) => {
                        let n = u16len(t);
                        formats.extend(fmt_changes(old, p, pos.0, pos.1, n));
                        pos = (pos.0 + n, pos.1 + n);
                    }
                    Edit::Del(t) => pos.0 += u16len(t),
                    Edit::Ins(t) => pos.1 += u16len(t),
                }
            }
            let mut pos = (0usize, 0usize);
            for e in edits {
                match e {
                    Edit::Eq(t) => {
                        pos.0 += u16len(&t);
                        pos.1 += u16len(&t);
                    }
                    Edit::Del(t) => {
                        let n = u16len(&t);
                        if let Some(del) = self.deleted(old, pos.0, pos.0 + n) {
                            edit::insert_inline_at(&mut p.inlines, pos.1, del);
                        }
                        pos.0 += n;
                    }
                    Edit::Ins(t) => {
                        let n = u16len(&t);
                        self.wrap_range(&mut p.inlines, pos.1, pos.1 + n);
                        pos.1 += n;
                    }
                }
            }
            self.mark_run_formats(p, &formats);
            return;
        }
        // Objects or fields: replace the text wholesale and leave everything else as it is.
        self.wrap_all(&mut p.inlines, Mode::TextOnly);
        if let Some(del) = self.deleted(old, 0, old.len16()) {
            p.inlines.insert(0, del);
        }
    }

    /// Tracked formatting changes (`w:rPrChange`) on the stretches of a paragraph that changed their formatting.
    fn mark_run_formats(&mut self, p: &mut Paragraph, changes: &[FmtChange]) {
        for c in changes {
            let old_rpr = c.old_rpr.as_ref().map(|r| {
                let mut r = r.clone();
                r.remove_children("w:rPrChange");
                if r.child("w:rStyle").and_then(|s| s.attr("w:val")).is_some_and(|id| !self.new.styles.exists(id)) {
                    r.remove_children("w:rStyle");
                }
                r
            });
            // One revision per run, each with the properties the text had before.
            edit::format_range(&mut p.inlines, c.new, c.new + c.len, &mut |r| {
                let mut change = self.rev("w:rPrChange");
                change.children.push(Node::Element(old_rpr.clone().unwrap_or_else(|| Element::new("w:rPr"))));
                let mut rpr = r.rpr.take().unwrap_or_else(|| Element::new("w:rPr"));
                rpr.remove_children("w:rPrChange");
                rpr.children.push(Node::Element(change));
                r.rpr = Some(rpr);
            });
        }
    }

    /// A tracked change of the paragraph properties (`w:pPrChange`) when they differ from the old paragraph's.
    fn mark_para_format(&mut self, p: &mut Paragraph, old: &Paragraph) {
        if para_fmt_names(old, p).is_empty() {
            return;
        }
        let mut before = match &old.ppr {
            Some(src) => self.usable_ppr(src),
            None => Element::new("w:pPr"),
        };
        before.remove_children("w:rPr");
        let mut change = self.rev("w:pPrChange");
        change.children.push(Node::Element(before));
        let ppr = p.ppr.get_or_insert_with(|| Element::new("w:pPr"));
        ppr.remove_children("w:pPrChange");
        put_ppr(ppr, change);
    }

    fn para(&mut self, p: &mut Paragraph, mark: bool) {
        match self.action.get(&p.id).copied() {
            None => {}
            Some(Action::Added) => {
                self.wrap_all(&mut p.inlines, Mode::All);
                if mark {
                    self.mark(p, "w:ins");
                }
            }
            Some(Action::Changed(oi)) => {
                let old = self.old_flat[oi].p;
                self.changed_para(p, old);
            }
            Some(Action::Formatted(oi)) => {
                let old = self.old_flat[oi].p;
                self.mark_para_format(p, old);
                let formats = fmt_changes(old, p, 0, 0, p.len16());
                self.mark_run_formats(p, &formats);
            }
        }
    }

    fn blocks(&mut self, blocks: &mut Vec<Block>, top: bool, cell: bool) {
        let all = std::mem::take(blocks);
        let last = all.len().saturating_sub(1);
        for (n, b) in all.into_iter().enumerate() {
            if top {
                if let Some(list) = self.before.get(&b.id()).cloned() {
                    for i in list {
                        let old = self.old_flat[i].p;
                        let p = self.deleted_para(old);
                        blocks.push(Block::Para(p));
                    }
                }
            }
            match b {
                Block::Para(mut p) => {
                    // The mark ending a table cell can't be tracked on its own.
                    self.para(&mut p, !(cell && n == last));
                    blocks.push(Block::Para(p));
                }
                Block::Table(mut t) => {
                    self.table(&mut t);
                    blocks.push(Block::Table(t));
                }
                Block::Container(mut c) => {
                    self.blocks(&mut c.blocks, top, cell);
                    blocks.push(Block::Container(c));
                }
                other => blocks.push(other),
            }
        }
    }

    fn table(&mut self, t: &mut Table) {
        for r in t.rows_mut() {
            // A row whose every paragraph is new is an inserted row.
            let mut ids = Vec::new();
            for c in r.cells() {
                walk_paragraphs(&c.blocks, &mut |p| ids.push(p.id));
            }
            if !ids.is_empty() && ids.iter().all(|id| matches!(self.action.get(id), Some(Action::Added))) {
                let at = r.children.iter().position(|c| matches!(c, RowChild::Other(e) if e.name == "w:trPr"));
                let at = match at {
                    Some(i) => i,
                    None => {
                        let i = r.children.iter().take_while(|c| matches!(c, RowChild::Other(e) if e.name == "w:tblPrEx")).count();
                        r.children.insert(i, RowChild::Other(Element::new("w:trPr")));
                        i
                    }
                };
                let ins = self.rev("w:ins");
                if let RowChild::Other(pr) = &mut r.children[at] {
                    pr.children.push(Node::Element(ins));
                }
            }
            for c in r.cells_mut() {
                self.blocks(&mut c.blocks, false, true);
            }
        }
    }
}

/// The newer document with tracked changes showing how it differs from the older one: word-level
/// insertions and deletions inside changed paragraphs, removed paragraphs put back as deleted ones and added
/// paragraphs as inserted ones. Everything else (formatting, headers and footers, tables that have no
/// counterpart, parts Docxtor does not edit) is the newer document's, as it is.
///
/// `author` and `date` (ISO 8601, like `2026-10-12T09:30:00Z`) go on every revision.
pub fn redline(old: &Document, new: &Document, author: &str, date: &str) -> Result<Vec<u8>, DocError> {
    redline_as(old, new, author, date, false)
}

/// The revised document with the differences from the original marked as tracked changes by `author`, like
/// Word's Review, Combine. Tracked changes the revised document already has stay as they are, with their own
/// authors and dates: paragraphs that carry any are compared by what they were before those changes, and left
/// alone. Tracked changes in the original document are not carried over.
pub fn combine(original: &Document, revised: &Document, author: &str, date: &str) -> Result<Vec<u8>, DocError> {
    redline_as(original, revised, author, date, true)
}

fn redline_as(old: &Document, new: &Document, author: &str, date: &str, keep_tracked: bool) -> Result<Vec<u8>, DocError> {
    let (mut fo, mut fnw) = (Vec::new(), Vec::new());
    flatten(&old.body, None, &mut fo);
    flatten_as(&new.body, None, &mut fnw, keep_tracked);
    let items = align(&fo, &fnw);

    let mut action = HashMap::new();
    let mut before: HashMap<NodeId, Vec<usize>> = HashMap::new();
    let mut tail: Vec<usize> = Vec::new();
    // Where a new paragraph sits as far as removed body paragraphs are concerned: itself, or its table.
    let key = |j: usize| fnw[j].table.unwrap_or(fnw[j].p.id);
    let mut next_key: Option<NodeId> = None;
    for item in items.iter().rev() {
        match *item {
            Item::Same(i, j) => {
                if !(keep_tracked && has_tracked(fnw[j].p)) {
                    let runs = !fmt_changes(fo[i].p, fnw[j].p, 0, 0, fnw[j].p.len16()).is_empty();
                    if runs || !para_fmt_names(fo[i].p, fnw[j].p).is_empty() {
                        action.insert(fnw[j].p.id, Action::Formatted(i));
                    }
                }
                next_key = Some(key(j));
            }
            Item::Added(j) => {
                action.insert(fnw[j].p.id, Action::Added);
                next_key = Some(key(j));
            }
            Item::Changed(i, j) => {
                action.insert(fnw[j].p.id, Action::Changed(i));
                next_key = Some(key(j));
            }
            // A paragraph that was in a table can't be put back anywhere sensible.
            Item::Removed(i) if fo[i].table.is_some() => {}
            Item::Removed(i) => match next_key {
                Some(k) => before.entry(k).or_default().push(i),
                None => tail.push(i),
            },
        }
    }
    for v in before.values_mut() {
        v.reverse();
    }
    tail.reverse();

    let mut max_id = max_w_id(&new.body);
    for s in &new.stories {
        max_id = max_id.max(max_w_id(&s.blocks));
    }
    let mut r = Redliner { new, author, date, next_rev: max_id, ids: IdGen(u32::MAX / 2), old_flat: fo, action, keep_tracked, before };
    crate::perf::body_cloned();
    let mut body = new.body.to_vec();
    r.blocks(&mut body, true, false);
    // Paragraphs removed after the last one go before the section properties that end the body.
    let at = body.iter().rposition(|b| matches!(b, Block::Opaque(o) if o.el.name == "w:sectPr")).unwrap_or(body.len());
    let tail_paras: Vec<Block> = tail
        .into_iter()
        .map(|i| {
            let old = r.old_flat[i].p;
            Block::Para(r.deleted_para(old))
        })
        .collect();
    body.splice(at..at, tail_paras);
    let mut out = new.fork_with_body(body);
    out.save()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn edits(a: &str, b: &str) -> Vec<(char, String)> {
        word_edits(a, b)
            .into_iter()
            .map(|e| match e {
                Edit::Eq(t) => ('=', t),
                Edit::Del(t) => ('-', t),
                Edit::Ins(t) => ('+', t),
            })
            .collect()
    }

    #[test]
    fn words_and_spaces() {
        assert_eq!(tokens("Hello, don't  stop"), ["Hello", ",", " ", "don't", "  ", "stop"]);
        let e = edits("The quick brown fox", "The slow brown fox");
        assert_eq!(e, [('=', "The ".into()), ('-', "quick".into()), ('+', "slow".into()), ('=', " brown fox".into())]);
        // Changes a space apart are one change.
        let e = edits("a b c", "x y c");
        assert_eq!(e, [('-', "a b".into()), ('+', "x y".into()), ('=', " c".into())]);
        assert_eq!(edits("same", "same"), [('=', "same".into())]);
        assert_eq!(edits("", "new"), [('+', "new".into())]);
    }
}
