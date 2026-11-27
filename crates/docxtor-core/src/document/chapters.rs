//! Chapter numbers in captions ("Figure 2-1"): a `STYLEREF 1 \s` field gives the number of the heading above,
//! and `SEQ Figure \s 1` starts counting again under each such heading. Numbered items for cross-references
//! ("Paragraph number" of a numbered paragraph, with or without its context) live here too.

use super::*;
use crate::fields::{self, Instr, Replace, SeqCounter};
use std::collections::HashSet;

/// Where the headings are, by level, in document order: for any paragraph, which heading of a level it is under
/// (a count that changes with each such heading) and that heading's number.
pub(super) struct ChapterIndex {
    order: HashMap<NodeId, usize>,
    /// For each level 1..=9: (position of the heading, how many headings of the level came up to it, its number).
    heads: Vec<Vec<(usize, usize, Option<String>)>>,
}

impl ChapterIndex {
    fn before(&self, pid: NodeId, level: u8) -> Option<&(usize, usize, Option<String>)> {
        let at = *self.order.get(&pid)?;
        let list = self.heads.get(level.checked_sub(1)? as usize)?;
        let n = list.partition_point(|h| h.0 <= at);
        n.checked_sub(1).and_then(|i| list.get(i))
    }

    /// A number that changes whenever a heading of this level is passed (0 before the first).
    pub fn epoch(&self, pid: NodeId, level: u8) -> usize {
        self.before(pid, level).map_or(0, |h| h.1)
    }

    /// The number of the heading of this level a paragraph is under (`1`, `2.3`), if it has one.
    pub fn number(&self, pid: NodeId, level: u8) -> Option<String> {
        self.before(pid, level).and_then(|h| h.2.clone())
    }
}

/// The level in `STYLEREF 1 \s` or `STYLEREF "Heading 2" \s`.
fn styleref_level(i: &Instr) -> Option<u8> {
    let arg: String = fields::unescape(i.first_arg()?).split_whitespace().collect::<Vec<_>>().join("").to_lowercase();
    let n: u8 = arg.strip_prefix("heading").unwrap_or(&arg).parse().ok()?;
    (1..=9).contains(&n).then_some(n)
}

impl Document {
    pub(super) fn chapter_index(&self) -> ChapterIndex {
        let labels = self.numbering.labels(&self.body, &self.styles);
        let mut order = HashMap::new();
        let mut heads: Vec<Vec<(usize, usize, Option<String>)>> = vec![Vec::new(); 9];
        let mut at = 0;
        walk_paragraphs(&self.body, &mut |p| {
            order.insert(p.id, at);
            if let Some(level) = self.heading_style_level(p).or_else(|| self.heading_level(p)) {
                let list = &mut heads[level as usize - 1];
                let n = list.len() + 1;
                list.push((at, n, Self::para_number(&labels, p.id)));
            }
            at += 1;
        });
        ChapterIndex { order, heads }
    }

    /// Number every `SEQ` field in document order (restarting under headings for `\s`) and work out `STYLEREF n \s`
    /// results. With `scope`, only fields in those paragraphs change. Returns the paragraphs that changed.
    pub(super) fn renumber_seq(&mut self, scope: Option<&HashSet<NodeId>>) -> Vec<NodeId> {
        let chapters = self.chapter_index();
        let mut counter = SeqCounter::default();
        let changed = fields::update_blocks_body(&mut self.body, &mut |pid, seen| {
            let i = Instr::parse(seen.instr);
            let wanted = scope.is_none_or(|s| s.contains(&pid));
            if i.code == "STYLEREF" && i.has("s") {
                let level = styleref_level(&i)?;
                return if wanted { chapters.number(pid, level).map(Replace::from) } else { None };
            }
            let next = counter.next_in(seen.instr, &|level| chapters.epoch(pid, level))?;
            wanted.then(|| Replace::from(next))
        });
        for id in &changed {
            self.touch(*id);
        }
        changed
    }

    // -----------------------------------------------------------------------
    // Numbered items
    // -----------------------------------------------------------------------

    /// For every numbered paragraph, the paragraphs whose numbers make up its full number, outermost first and
    /// ending with itself (`1.`, `a)`, `ii.` give the three for the item `ii`).
    pub(super) fn number_chains(&self, labels: &HashMap<NodeId, ListLabel>) -> HashMap<NodeId, Vec<NodeId>> {
        let mut out = HashMap::new();
        // Per list: the latest paragraph at each level.
        let mut stacks: HashMap<i64, Vec<(i64, NodeId)>> = HashMap::new();
        walk_paragraphs(&self.body, &mut |p| {
            let Some(l) = labels.get(&p.id).filter(|l| !l.bullet) else { return };
            let stack = stacks.entry(l.num_id).or_default();
            while stack.last().is_some_and(|(lv, _)| *lv >= l.level) {
                stack.pop();
            }
            stack.push((l.level, p.id));
            out.insert(p.id, stack.iter().map(|(_, id)| *id).collect());
        });
        out
    }

    /// The number of a paragraph as a cross-reference shows it. `mode` is the field's switch: `n` the paragraph's
    /// own number, `w` the number with every level above it, `r` the number relative to the paragraph the field is
    /// in (the levels they share are left out).
    pub(super) fn numbered_text(labels: &HashMap<NodeId, ListLabel>, chains: &HashMap<NodeId, Vec<NodeId>>, target: NodeId, host: Option<NodeId>, mode: char) -> Option<String> {
        let clean = |id: &NodeId| labels.get(id).map(|l| l.text.trim().to_string()).filter(|t| !t.is_empty());
        let own = Self::para_number(labels, target)?;
        let chain = chains.get(&target)?;
        let skip = match mode {
            'n' => chain.len() - 1,
            'w' => 0,
            _ => host.and_then(|h| chains.get(&h)).map_or(0, |hc| chain.iter().zip(hc).take_while(|(a, b)| a == b).count().min(chain.len() - 1)),
        };
        let parts: Vec<String> = chain[skip..].iter().filter_map(clean).collect();
        if parts.len() <= 1 {
            return Some(own);
        }
        // A level that already spells out the levels above ("1.2.3") stands alone.
        let last = parts.last()?;
        if last.starts_with(parts[parts.len() - 2].trim_end_matches('.')) && last.contains('.') {
            return Some(own);
        }
        // "1." "a." "ii." make 1.a.ii; a part that ends in ")" takes no dot after it.
        let mut s = String::new();
        let mut prev_dot = false;
        for (k, part) in parts.iter().enumerate() {
            let t = part.trim_end_matches('.');
            if k > 0 && prev_dot && !t.starts_with(['(', '[']) {
                s.push('.');
            }
            s.push_str(t);
            prev_dot = part.ends_with('.');
        }
        Some(s)
    }

    /// The paragraphs a cross-reference to a "numbered item" can point at, in order: their number, the number with
    /// its context, and the start of the text.
    pub fn numbered_items(&self) -> Vec<NumberedItem> {
        let labels = self.numbering.labels(&self.body, &self.styles);
        let chains = self.number_chains(&labels);
        let mut out = Vec::new();
        walk_paragraphs(self.main_body(), &mut |p| {
            let Some(number) = Self::para_number(&labels, p.id) else { return };
            let style = p.ppr.as_ref().and_then(|pp| pp.child("w:pStyle")).and_then(|s| s.attr("w:val")).unwrap_or_default();
            if style.starts_with("TOC") || style == "TableofFigures" {
                return;
            }
            let mut text: String = p.plain_text().split_whitespace().collect::<Vec<_>>().join(" ");
            if text.chars().count() > 80 {
                text = text.chars().take(80).collect::<String>().trim_end().to_string() + "\u{2026}";
            }
            let full = Self::numbered_text(&labels, &chains, p.id, None, 'w').unwrap_or_else(|| number.clone());
            let level = labels.get(&p.id).map_or(0, |l| l.level);
            out.push(NumberedItem { p: p.id, number, full, text, level });
        });
        out
    }
}

/// A numbered paragraph a cross-reference can point at.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NumberedItem {
    pub p: NodeId,
    /// Its own number (`ii`).
    pub number: String,
    /// The number with the levels above it (`1.a.ii`).
    pub full: String,
    pub text: String,
    /// The list level, 0 for the outermost.
    pub level: i64,
}
