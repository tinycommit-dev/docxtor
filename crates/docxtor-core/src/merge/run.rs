//! Merging a document with recipients. `Merger::unit` is a pure function of the template's blocks, the list and the
//! record it starts at: it copies the blocks, works out every merge field and rule in them in document order, and
//! gives back the copy with the field markup gone and the results left as plain text. A letter is one record per
//! unit; a sheet of labels or a directory uses several (the `NEXT` fields move on to the next record inside it).

use super::address::{self, Recipient};
use super::instr::{self, field_name};
use super::MailMerge;
use crate::field_fmt::{self as fmt, Dt};
use crate::field_values::{self, Env, Place};
use crate::fields::{self, Instr, Replace, Seen};
use crate::model::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// What the person answered when `ASK` and `FILLIN` fields asked.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Answers {
    /// Answers that apply to every record, by prompt key (see `Prompt::key`).
    #[serde(default)]
    pub once: HashMap<String, String>,
    /// Answers for one record (its position in the list, from 0), by prompt key.
    #[serde(default)]
    pub records: HashMap<usize, HashMap<String, String>>,
}

/// A question the merge will ask: an `ASK` or `FILLIN` field.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Prompt {
    /// `ask:<bookmark>` or `fill:<prompt>`: the key of the answer.
    pub key: String,
    pub kind: String,
    pub prompt: String,
    pub default: String,
    /// Asked once for the whole merge (`\o`).
    pub once: bool,
}

/// The questions in some blocks, in order, each once.
pub fn prompts<'a>(blocks: impl IntoIterator<Item = &'a Block>) -> Vec<Prompt> {
    let mut out: Vec<Prompt> = Vec::new();
    fields::scan_blocks(blocks, &mut |_, seen| {
        let i = Instr::parse(seen.instr);
        let words: Vec<String> = i.toks.iter().filter(|t| !t.is_switch()).map(|t| fields::unescape(&t.text)).collect();
        let p = match i.code.as_str() {
            "ASK" if words.len() >= 2 => Prompt { key: format!("ask:{}", words[0].to_lowercase()), kind: "ask".into(), prompt: words[1].clone(), default: i.value("d").unwrap_or_default(), once: i.has("o") },
            "FILLIN" if !words.is_empty() => Prompt { key: format!("fill:{}", words[0]), kind: "fillin".into(), prompt: words[0].clone(), default: i.value("d").unwrap_or_default(), once: i.has("o") },
            _ => return,
        };
        if !out.iter().any(|q| q.key == p.key) {
            out.push(p);
        }
    });
    out
}

/// One merged copy of the template.
#[derive(Debug, Clone)]
pub struct Unit {
    pub blocks: Vec<Block>,
    /// The position in the list of the record the copy started at.
    pub first: usize,
    /// How many records it used (more than one when `NEXT` fields moved on).
    pub used: usize,
    /// A `SKIPIF` field held: the copy is not part of the result.
    pub skipped: bool,
}

pub struct Merger<'a> {
    pub mm: &'a MailMerge,
    pub answers: &'a Answers,
    /// The records in merge order, as indexes into the table.
    pub order: Vec<usize>,
    pub lang: Option<&'a str>,
    /// The end of the range being merged (a position in `order`, exclusive): `NEXT` fields stop here, and a field
    /// past it shows nothing instead of the next recipient's details.
    pub limit: Option<usize>,
}

impl<'a> Merger<'a> {
    pub fn new(mm: &'a MailMerge, answers: &'a Answers) -> Merger<'a> {
        Merger { mm, answers, order: mm.order(), lang: None, limit: None }
    }

    pub fn len(&self) -> usize {
        self.order.len()
    }

    pub fn is_empty(&self) -> bool {
        self.order.is_empty()
    }

    /// The copy that starts at position `start` of the list. `merged` is how many records were merged before it
    /// (for `MERGESEQ`). `content` is the template without its closing section properties.
    pub fn unit(&self, content: &[Block], start: usize, merged: usize, ids: &mut IdGen) -> Unit {
        let mut blocks: Vec<Block> = content.to_vec();
        renumber(&mut blocks, ids);
        let mut st = State { m: self, cur: start, seq: merged + 1, sets: HashMap::new(), skip: false };
        walk_paragraphs_mut(&mut blocks, &mut |p| st.paragraph(p));
        let end = self.limit.unwrap_or(self.len()).min(self.len());
        let used = (st.cur.min(end.saturating_sub(1)) + 1).saturating_sub(start).max(1);
        let sets = st.sets.keys().cloned().collect::<Vec<_>>();
        walk_paragraphs_mut(&mut blocks, &mut |p| unwrap_paragraph(p, &sets));
        Unit { blocks, first: start, used, skipped: st.skip }
    }

    /// Every copy for the whole list, in order. Skipped copies are left out.
    pub fn all(&self, content: &[Block], ids: &mut IdGen) -> Vec<Unit> {
        let mut out = Vec::new();
        let mut at = 0;
        let mut merged = 0;
        while at < self.len() {
            let u = self.unit(content, at, merged, ids);
            at += u.used;
            if !u.skipped {
                merged += u.used;
                out.push(u);
            }
        }
        out
    }
}

/// Give every block in the list a new id.
pub fn renumber(blocks: &mut [Block], ids: &mut IdGen) {
    for b in blocks {
        match b {
            Block::Para(p) => p.id = ids.fresh(),
            Block::Table(t) => {
                t.id = ids.fresh();
                for r in t.rows_mut() {
                    for c in r.cells_mut() {
                        renumber(&mut c.blocks, ids);
                    }
                }
            }
            Block::Container(c) => {
                c.id = ids.fresh();
                renumber(&mut c.blocks, ids);
            }
            Block::Opaque(o) => o.id = ids.fresh(),
        }
    }
}

/// Take the bookmarks out of a copy, so a second copy doesn't repeat a name.
pub fn strip_bookmarks(blocks: &mut [Block]) {
    walk_paragraphs_mut(blocks, &mut |p| {
        p.inlines.retain(|i| !matches!(i, Inline::Opaque(e) if matches!(e.name.as_str(), "w:bookmarkStart" | "w:bookmarkEnd")));
    });
}

struct State<'a> {
    m: &'a Merger<'a>,
    /// The position in the list of the record the next field reads.
    cur: usize,
    seq: usize,
    /// What `SET` and `ASK` gave, by lower-case name.
    sets: HashMap<String, String>,
    skip: bool,
}

/// The words of a field after its name, as one string (`NEXTIF "a" = "b"` gives `"a" = "b"`).
fn after_code(instr: &str) -> &str {
    let t = instr.trim_start();
    match t.find(char::is_whitespace) {
        Some(i) => t[i..].trim(),
        None => "",
    }
}

fn plain(s: impl Into<String>) -> Option<Replace> {
    Some(Replace { text: s.into(), ..Default::default() })
}

/// A date as a list writes it: `2026-10-13`, or `10/13/2026` with an optional time.
fn loose_date(s: &str) -> Option<Dt> {
    if let Some(d) = Dt::parse_iso(s) {
        return Some(d);
    }
    let s = s.trim();
    let (date, time) = match s.split_once(' ') {
        Some((d, t)) => (d, Some(t.trim())),
        None => (s, None),
    };
    let mut p = date.split('/');
    let (m, d, y) = (p.next()?.parse::<u32>().ok()?, p.next()?.parse::<u32>().ok()?, p.next()?.parse::<i32>().ok()?);
    let (mut hour, mut minute) = (0, 0);
    if let Some(t) = time {
        let pm = t.to_ascii_lowercase().ends_with("pm");
        let am = t.to_ascii_lowercase().ends_with("am");
        let t = t.trim_end_matches(|c: char| c.is_ascii_alphabetic()).trim();
        let mut tp = t.split(':');
        hour = tp.next()?.parse::<u32>().ok()?;
        minute = tp.next().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0);
        if pm && hour < 12 {
            hour += 12;
        }
        if am && hour == 12 {
            hour = 0;
        }
    }
    Dt::parse_iso(&format!("{y:04}-{m:02}-{d:02}T{hour:02}:{minute:02}:00"))
}

impl State<'_> {
    fn recipient(&self) -> Recipient<'_> {
        Recipient { mm: self.m.mm, row: self.m.order.get(self.cur).copied().filter(|_| self.m.limit.is_none_or(|l| self.cur < l)) }
    }

    fn paragraph(&mut self, p: &mut Paragraph) {
        fields::update_inner_first(&mut p.inlines, &mut |seen| self.field(seen));
    }

    fn answer(&self, key: &str, default: &str) -> String {
        self.m.answers.records.get(&self.cur).and_then(|r| r.get(key)).or_else(|| self.m.answers.once.get(key)).cloned().unwrap_or_else(|| default.to_string())
    }

    fn compare(&self, rest: &str) -> bool {
        let env = Env { sets: self.sets.clone(), lang: self.m.lang.map(String::from), ..Default::default() };
        field_values::evaluate(&format!(" COMPARE {rest} "), &env, &Place::default()).is_some_and(|r| r.text == "1")
    }

    fn field(&mut self, seen: &Seen) -> Option<Replace> {
        let i = Instr::parse(seen.instr);
        match i.code.as_str() {
            "MERGEFIELD" => plain(self.merge_field(&i, seen.instr)),
            "ADDRESSBLOCK" => plain(address::address_block(&self.recipient(), &i)),
            "GREETINGLINE" => plain(address::greeting_line(&self.recipient(), &i)),
            "MERGEREC" => plain((self.cur + 1).to_string()),
            "MERGESEQ" => plain(self.seq.to_string()),
            "NEXT" => {
                self.cur += 1;
                self.seq += 1;
                plain("")
            }
            "NEXTIF" => {
                if self.compare(after_code(seen.instr)) {
                    self.cur += 1;
                    self.seq += 1;
                }
                plain("")
            }
            "SKIPIF" => {
                if self.compare(after_code(seen.instr)) {
                    self.skip = true;
                }
                plain("")
            }
            "SET" => {
                let w: Vec<String> = i.toks.iter().filter(|t| !t.is_switch()).map(|t| fields::unescape(&t.text)).collect();
                if w.len() >= 2 {
                    self.sets.insert(w[0].to_lowercase(), w[1].clone());
                }
                plain("")
            }
            "ASK" => {
                let w: Vec<String> = i.toks.iter().filter(|t| !t.is_switch()).map(|t| fields::unescape(&t.text)).collect();
                if w.len() >= 2 {
                    let a = self.answer(&format!("ask:{}", w[0].to_lowercase()), &i.value("d").unwrap_or_default());
                    self.sets.insert(w[0].to_lowercase(), a);
                }
                plain("")
            }
            "FILLIN" => {
                let prompt = i.first_arg().map(fields::unescape).unwrap_or_default();
                plain(self.answer(&format!("fill:{prompt}"), &i.value("d").unwrap_or_default()))
            }
            "REF" => {
                let name = i.first_arg().map(fields::unescape)?.to_lowercase();
                plain(self.sets.get(&name)?.clone())
            }
            "IF" | "COMPARE" => {
                let env = Env { sets: self.sets.clone(), lang: self.m.lang.map(String::from), ..Default::default() };
                field_values::evaluate(seen.instr, &env, &Place::default())
            }
            _ => None,
        }
    }

    /// The text of a `MERGEFIELD`: the column's value with the field's switches applied.
    fn merge_field(&self, i: &Instr, instr: &str) -> String {
        let Some(name) = field_name(i) else { return String::new() };
        let rec = self.recipient();
        let value = match rec.column(&name) {
            Some(v) => v.to_string(),
            // A template written with Word's own names works with a list that names its columns differently.
            None => rec.field(&name),
        };
        if value.trim().is_empty() {
            return String::new();
        }
        let mut text = value;
        if let Some(pic) = i.value("@") {
            if let Some(dt) = loose_date(&text) {
                text = fmt::date_picture(&dt, &pic, self.m.lang);
            }
        }
        text = field_values::apply_switches(instr, &text);
        let before = i.value("b").unwrap_or_default();
        let after = i.value("f").unwrap_or_default();
        format!("{before}{text}{after}")
    }
}

/// Take the field markup off the merge fields of a paragraph and leave their results as text.
fn unwrap_paragraph(p: &mut Paragraph, sets: &[String]) {
    let pick = |s: &str| {
        let i = Instr::parse(s);
        instr::is_merge_code(&i.code) || matches!(i.code.as_str(), "IF" | "COMPARE") || (i.code == "REF" && i.first_arg().is_some_and(|n| sets.contains(&fields::unescape(n).to_lowercase())))
    };
    fields::unwrap_fields(&mut p.inlines, 0, usize::MAX / 2, &pick);
    unwrap_simple(&mut p.inlines, &pick);
}

/// A `w:fldSimple` merge field becomes its result runs.
fn unwrap_simple(inlines: &mut Vec<Inline>, pick: &dyn Fn(&str) -> bool) {
    let mut out = Vec::with_capacity(inlines.len());
    for inl in std::mem::take(inlines) {
        match inl {
            Inline::Container(mut c) if c.kind == InlineKind::Field && c.shell.attr("w:instr").is_some_and(pick) => {
                unwrap_simple(&mut c.children, pick);
                out.extend(c.children);
            }
            Inline::Container(mut c) => {
                unwrap_simple(&mut c.children, pick);
                out.push(Inline::Container(c));
            }
            other => out.push(other),
        }
    }
    *inlines = out;
}

#[cfg(test)]
mod tests {
    use super::super::instr::{build, MergeFieldSpec};
    use super::super::Table;
    use super::*;
    use crate::xml::Element;
    use std::sync::Arc;

    fn mm() -> MailMerge {
        let rows = [["Ada", "Lovelace", "CA", "1815-12-10", "1234.5"], ["Grace", "Hopper", "NY", "", "7"], ["Alan", "Turing", "CA", "1912-06-23", ""]];
        MailMerge {
            table: Arc::new(Table::from_rows(["First Name", "Last Name", "State", "Born", "Fee"].map(String::from).to_vec(), rows.iter().map(|r| r.map(String::from).to_vec()).collect())),
            ..Default::default()
        }
    }

    fn para(ids: &mut IdGen, parts: Vec<Vec<Inline>>, text: &str) -> Paragraph {
        let mut p = Paragraph::new(ids.fresh());
        for part in parts {
            p.inlines.extend(part);
        }
        if !text.is_empty() {
            p.inlines.push(Inline::Run(Run { items: vec![RunItem::Text(text.into())], ..Default::default() }));
        }
        p
    }

    fn field(spec: MergeFieldSpec) -> Vec<Inline> {
        build(&spec, None).unwrap()
    }

    fn mf(name: &str) -> MergeFieldSpec {
        MergeFieldSpec::Field { name: name.into(), before: None, after: None, case: None, picture: None }
    }

    fn text_of(blocks: &[Block]) -> Vec<String> {
        let mut out = Vec::new();
        walk_paragraphs(blocks, &mut |p| out.push(p.plain_text()));
        out
    }

    fn fields_left(blocks: &[Block]) -> usize {
        let mut n = 0;
        fields::scan_blocks(blocks, &mut |_, _| n += 1);
        n
    }

    #[test]
    fn letters_one_record_each() {
        let mut ids = IdGen(0);
        let body = vec![
            Block::Para(para(&mut ids, vec![field(MergeFieldSpec::MergeRec), field(mf("First Name"))], " ")),
            Block::Para(para(&mut ids, vec![field(MergeFieldSpec::Field { name: "Last name".into(), before: Some("Dear ".into()), after: Some("!".into()), case: Some("upper".into()), picture: None })], "")),
            Block::Para(para(&mut ids, vec![field(MergeFieldSpec::Field { name: "Born".into(), before: None, after: None, case: None, picture: Some("MMMM d, yyyy".into()) })], "")),
            Block::Para(para(&mut ids, vec![field(MergeFieldSpec::Field { name: "Fee".into(), before: Some("$".into()), after: None, case: None, picture: Some("#,##0.00".into()) })], "")),
        ];
        let answers = Answers::default();
        let m = mm();
        let merger = Merger::new(&m, &answers);
        let units = merger.all(&body, &mut ids);
        assert_eq!(units.len(), 3);
        assert_eq!(text_of(&units[0].blocks), ["1Ada ", "Dear LOVELACE!", "December 10, 1815", "$1,234.50"]);
        // An empty value drops its before and after text; the next record is the second.
        assert_eq!(text_of(&units[1].blocks), ["2Grace ", "Dear HOPPER!", "", "$7.00"]);
        assert_eq!(text_of(&units[2].blocks), ["3Alan ", "Dear TURING!", "June 23, 1912", ""]);
        // The field markup is gone, and every copy has ids of its own.
        assert!(units.iter().all(|u| fields_left(&u.blocks) == 0));
        let all: Vec<u32> = units.iter().flat_map(|u| u.blocks.iter().map(|b| b.id())).collect();
        assert_eq!(all.len(), 12);
        assert!(all.iter().collect::<std::collections::HashSet<_>>().len() == 12);
    }

    #[test]
    fn the_template_is_not_touched() {
        let mut ids = IdGen(0);
        let body = vec![Block::Para(para(&mut ids, vec![field(mf("First Name"))], ""))];
        let before = text_of(&body);
        let m = mm();
        let a = Answers::default();
        let one = Merger::new(&m, &a).unit(&body, 1, 0, &mut ids);
        assert_eq!(text_of(&one.blocks), ["Grace"]);
        assert_eq!(text_of(&body), before);
        assert_eq!(before, ["«First_Name»"]);
        // The same record again gives the same text.
        let again = Merger::new(&m, &a).unit(&body, 1, 0, &mut ids);
        assert_eq!(text_of(&again.blocks), text_of(&one.blocks));
    }

    #[test]
    fn if_reads_the_field_inside_it() {
        let mut ids = IdGen(0);
        let body = vec![Block::Para(para(&mut ids, vec![field(MergeFieldSpec::If { field: "State".into(), op: "eq".into(), value: "CA".into(), then: "West coast".into(), otherwise: "Elsewhere".into() })], ""))];
        let m = mm();
        let a = Answers::default();
        let units = Merger::new(&m, &a).all(&body, &mut ids);
        assert_eq!(units.iter().map(|u| text_of(&u.blocks)[0].clone()).collect::<Vec<_>>(), ["West coast", "Elsewhere", "West coast"]);
        assert!(units.iter().all(|u| fields_left(&u.blocks) == 0));
        // Blank and not blank.
        let body = vec![Block::Para(para(&mut ids, vec![field(MergeFieldSpec::If { field: "Born".into(), op: "empty".into(), value: String::new(), then: "unknown".into(), otherwise: "known".into() })], ""))];
        let units = Merger::new(&m, &a).all(&body, &mut ids);
        assert_eq!(units.iter().map(|u| text_of(&u.blocks)[0].clone()).collect::<Vec<_>>(), ["known", "unknown", "known"]);
    }

    #[test]
    fn next_record_fills_a_sheet() {
        // Two cells' worth of text in one copy: the second starts with Next Record.
        let mut ids = IdGen(0);
        let body = vec![
            Block::Para(para(&mut ids, vec![field(mf("First Name"))], "")),
            Block::Para(para(&mut ids, vec![field(MergeFieldSpec::Next), field(mf("First Name")), field(MergeFieldSpec::MergeSeq)], "")),
        ];
        let m = mm();
        let a = Answers::default();
        let units = Merger::new(&m, &a).all(&body, &mut ids);
        // Three records, two to a copy: Ada and Grace, then Alan and nothing.
        assert_eq!(units.len(), 2);
        assert_eq!((units[0].first, units[0].used, units[1].first, units[1].used), (0, 2, 2, 1));
        assert_eq!(text_of(&units[0].blocks), ["Ada", "Grace2"]);
        assert_eq!(text_of(&units[1].blocks), ["Alan", "4"]);
    }

    #[test]
    fn skip_and_next_if() {
        let mut ids = IdGen(0);
        let body = vec![Block::Para(para(&mut ids, vec![field(MergeFieldSpec::SkipIf { field: "State".into(), op: "eq".into(), value: "NY".into() }), field(mf("First Name")), field(MergeFieldSpec::MergeSeq)], ""))];
        let m = mm();
        let a = Answers::default();
        let units = Merger::new(&m, &a).all(&body, &mut ids);
        assert_eq!(units.iter().map(|u| text_of(&u.blocks)[0].clone()).collect::<Vec<_>>(), ["Ada1", "Alan2"]);
        // Next record if the state is CA: Ada is in CA, so her copy goes on to Grace's record; Alan is in CA but last.
        let body = vec![Block::Para(para(&mut ids, vec![field(mf("First Name")), field(MergeFieldSpec::NextIf { field: "State".into(), op: "eq".into(), value: "CA".into() }), field(mf("First Name"))], ""))];
        let units = Merger::new(&m, &a).all(&body, &mut ids);
        assert_eq!(units.iter().map(|u| (text_of(&u.blocks)[0].clone(), u.used)).collect::<Vec<_>>(), [("AdaGrace".to_string(), 2), ("Alan".to_string(), 1)]);
    }

    #[test]
    fn ask_set_and_fill_in() {
        let mut ids = IdGen(0);
        let ref_field = |name: &str| fields::complex(&format!(" REF {name} "), "?", None, false);
        let body = vec![
            Block::Para(para(&mut ids, vec![field(MergeFieldSpec::Ask { bookmark: "Due".into(), prompt: "Due date?".into(), default: "Friday".into(), once: true }), field(MergeFieldSpec::Set { bookmark: "Sig".into(), value: "Ada".into() })], "")),
            Block::Para(para(&mut ids, vec![ref_field("Due"), ref_field("Sig"), field(MergeFieldSpec::FillIn { prompt: "Room?".into(), default: "A1".into(), once: false })], "")),
        ];
        let found = prompts(&body);
        assert_eq!(found.iter().map(|p| (p.key.as_str(), p.once, p.default.as_str())).collect::<Vec<_>>(), [("ask:due", true, "Friday"), ("fill:Room?", false, "A1")]);
        let m = mm();
        let mut a = Answers::default();
        let plain = Merger::new(&m, &a).unit(&body, 0, 0, &mut ids);
        assert_eq!(text_of(&plain.blocks), ["", "FridayAdaA1"]);
        a.once.insert("ask:due".into(), "Monday".into());
        a.records.insert(1, [("fill:Room?".to_string(), "B2".to_string())].into());
        let units = Merger::new(&m, &a).all(&body, &mut ids);
        assert_eq!(text_of(&units[0].blocks)[1], "MondayAdaA1");
        assert_eq!(text_of(&units[1].blocks)[1], "MondayAdaB2");
        assert_eq!(fields_left(&units[1].blocks), 0);
    }

    #[test]
    fn other_fields_stay_and_simple_fields_merge() {
        let mut ids = IdGen(0);
        let mut p = Paragraph::new(ids.fresh());
        p.inlines.extend(fields::complex(" DATE ", "today", None, false));
        let simple = InlineContainer {
            kind: InlineKind::Field,
            shell: Element::new("w:fldSimple").with_attr("w:instr", " MERGEFIELD First_Name "),
            content_in: None,
            children: vec![Inline::Run(Run { items: vec![RunItem::Text("«First_Name»".into())], ..Default::default() })],
        };
        p.inlines.push(Inline::Container(simple));
        let m = mm();
        let a = Answers::default();
        let u = Merger::new(&m, &a).unit(&[Block::Para(p)], 0, 0, &mut ids);
        assert_eq!(text_of(&u.blocks), ["todayAda"]);
        // The date field is still a field; the merge field is not.
        assert_eq!(fields_left(&u.blocks), 1);
    }

    #[test]
    fn dates_in_a_list() {
        assert_eq!(loose_date("10/13/2026 6:30 PM").map(|d| d.iso()).as_deref(), Some("2026-10-13T18:30:00Z"));
        assert_eq!(loose_date("2026-10-13").map(|d| d.day), Some(13));
        assert!(loose_date("soon").is_none());
    }
}
