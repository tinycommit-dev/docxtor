//! Paragraph-level comparison of two documents (version history, reviews, `docxtor diff`).

use crate::document::Document;
use crate::model::walk_paragraphs;
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffEntry {
    /// `added`, `removed` or `changed`.
    pub kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    /// Paragraph index in the newer document (or where the removal happened).
    pub index: usize,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffSummary {
    pub added: usize,
    pub removed: usize,
    pub changed: usize,
    pub entries: Vec<DiffEntry>,
}

pub fn paragraph_texts(doc: &Document) -> Vec<String> {
    let mut v = Vec::new();
    walk_paragraphs(&doc.body, &mut |p| v.push(p.plain_text()));
    v
}

pub fn diff_documents(old: &Document, new: &Document) -> DiffSummary {
    diff_texts(&paragraph_texts(old), &paragraph_texts(new))
}

/// LCS diff over paragraphs, with removed/added runs paired up as "changed".
pub fn diff_texts(a: &[String], b: &[String]) -> DiffSummary {
    // Trim the common prefix and suffix: edits are usually local.
    let mut pre = 0;
    while pre < a.len() && pre < b.len() && a[pre] == b[pre] {
        pre += 1;
    }
    let mut suf = 0;
    while suf < a.len() - pre && suf < b.len() - pre && a[a.len() - 1 - suf] == b[b.len() - 1 - suf] {
        suf += 1;
    }
    let am = &a[pre..a.len() - suf];
    let bm = &b[pre..b.len() - suf];

    let script = middle_script(am, bm);

    let mut out = DiffSummary::default();
    let mut k = 0;
    while k < script.len() {
        if script[k].0 == '=' {
            k += 1;
            continue;
        }
        // Collect a run of removals and additions, then pair them as changes.
        let mut dels = Vec::new();
        let mut adds = Vec::new();
        while k < script.len() && script[k].0 != '=' {
            match script[k] {
                ('-', i, _) => dels.push(i),
                ('+', _, j) => adds.push(j),
                _ => {}
            }
            k += 1;
        }
        let pairs = dels.len().min(adds.len());
        for p in 0..pairs {
            out.changed += 1;
            out.entries.push(DiffEntry { kind: "changed", before: Some(am[dels[p]].clone()), after: Some(bm[adds[p]].clone()), index: pre + adds[p] });
        }
        for &j in &adds[pairs..] {
            out.added += 1;
            out.entries.push(DiffEntry { kind: "added", before: None, after: Some(bm[j].clone()), index: pre + j });
        }
        for &i in &dels[pairs..] {
            out.removed += 1;
            let index = pre + adds.last().map(|j| j + 1).unwrap_or(i.min(bm.len()));
            out.entries.push(DiffEntry { kind: "removed", before: Some(am[i].clone()), after: None, index });
        }
    }
    out
}

/// LCS edit script over two paragraph lists: `('=', i, j)` keeps `a[i]` as `b[j]`, `('-', i, _)` removes
/// `a[i]` and `('+', _, j)` adds `b[j]`. Indices are into the slices given.
fn middle_script(am: &[String], bm: &[String]) -> Vec<(char, usize, usize)> {
    let mut script: Vec<(char, usize, usize)> = Vec::new();
    if am.len().saturating_mul(bm.len()) <= 9_000_000 {
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
                script.push(('=', i, j));
                i += 1;
                j += 1;
            } else if j < m && (i >= n || dp[at(i, j + 1)] >= dp[at(i + 1, j)]) {
                script.push(('+', i, j));
                j += 1;
            } else {
                script.push(('-', i, j));
                i += 1;
            }
        }
    } else {
        // Huge rewrite: treat as replace-all.
        script.extend((0..am.len()).map(|i| ('-', i, 0)));
        script.extend((0..bm.len()).map(|j| ('+', am.len(), j)));
    }
    script
}

/// The whole edit script between two paragraph lists (common prefix and suffix included as `'='`),
/// with indices into `a` and `b`. Used to line two versions up paragraph by paragraph.
pub fn lcs_steps(a: &[String], b: &[String]) -> Vec<(char, usize, usize)> {
    let mut pre = 0;
    while pre < a.len() && pre < b.len() && a[pre] == b[pre] {
        pre += 1;
    }
    let mut suf = 0;
    while suf < a.len() - pre && suf < b.len() - pre && a[a.len() - 1 - suf] == b[b.len() - 1 - suf] {
        suf += 1;
    }
    let mut out: Vec<(char, usize, usize)> = (0..pre).map(|k| ('=', k, k)).collect();
    for (c, i, j) in middle_script(&a[pre..a.len() - suf], &b[pre..b.len() - suf]) {
        out.push((c, i + pre, j + pre));
    }
    for k in 0..suf {
        out.push(('=', a.len() - suf + k, b.len() - suf + k));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(s: &[&str]) -> Vec<String> {
        s.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn diffs() {
        let d = diff_texts(&v(&["a", "b", "c", "d"]), &v(&["a", "B", "c", "d", "e"]));
        assert_eq!((d.added, d.removed, d.changed), (1, 0, 1));
        assert_eq!(d.entries[0].before.as_deref(), Some("b"));
        assert_eq!(d.entries[0].after.as_deref(), Some("B"));
        let d = diff_texts(&v(&["a", "b", "c"]), &v(&["a", "c"]));
        assert_eq!((d.added, d.removed, d.changed), (0, 1, 0));
        let d = diff_texts(&v(&["x"]), &v(&["x"]));
        assert!(d.entries.is_empty());
    }
}
