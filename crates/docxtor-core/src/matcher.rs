//! A backtracking pattern matcher for find and replace, with no outside crate. It reads three notations and
//! matches them against document text in one engine:
//!
//! * plain text with Word's `^` special characters (`^p`, `^t`, `^?` ...),
//! * Word's wildcards (`?`, `*`, `[a-z]`, `[!x]`, `{n,m}`, `@`, `<`, `>`, `(...)` and `\1`),
//! * regular expressions in JavaScript syntax (classes, groups, alternation, lazy and greedy quantifiers,
//!   backreferences, lookahead and lookbehind, `\b`, `\d`, `\w`, `\s`, `\p{L}` ...).
//!
//! The text it searches is a list of [`Hc`] (one per visible character), so a pattern can see what kind of thing
//! a character is (a paragraph mark, a footnote reference, a picture) and whether it lies in a field.

/// The kinds of character that are more than their code point.
pub const PMARK: u16 = 1;
pub const LINE: u16 = 2;
pub const PAGE: u16 = 4;
pub const COLUMN: u16 = 8;
pub const SECT: u16 = 16;
pub const FOOTNOTE: u16 = 32;
pub const ENDNOTE: u16 = 64;
pub const GRAPHIC: u16 = 128;
pub const EQUATION: u16 = 256;

/// One character of searchable text and where it came from.
#[derive(Debug, Clone, Copy)]
pub struct Hc {
    pub c: char,
    pub kind: u16,
    /// 0, or which field of its paragraph (1, 2, ...) the character is the result of.
    pub field: u16,
    /// Whether the character has the formatting being searched for (true when none is).
    pub ok: bool,
    /// Index of the paragraph in its stream, and the UTF-16 offset and length in that paragraph.
    pub para: u32,
    pub off: u32,
    pub len: u8,
}

impl Hc {
    pub fn plain(c: char) -> Hc {
        Hc { c, kind: 0, field: 0, ok: true, para: 0, off: 0, len: c.len_utf16() as u8 }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Class {
    Digit,
    NotDigit,
    Word,
    NotWord,
    Space,
    NotSpace,
    Letter,
    Upper,
    Lower,
    Number,
    Punct,
    NotLetter,
}

#[derive(Debug, Clone)]
pub enum SetItem {
    Ch(char),
    Range(char, char),
    Class(Class),
    Kind(u16),
}

#[derive(Debug, Clone)]
pub enum Node {
    Char(char),
    /// Any character but a paragraph mark or a line break (a regex `.`).
    Dot,
    /// Any single character (Word's `?` and `^?`).
    AnyChar,
    Set { items: Vec<SetItem>, neg: bool },
    Kind(u16),
    LineStart,
    LineEnd,
    WordBoundary,
    NotWordBoundary,
    WordStart,
    WordEnd,
    Group(Box<Node>, Option<usize>),
    Cat(Vec<Node>),
    Alt(Vec<Node>),
    Rep { node: Box<Node>, min: usize, max: usize, greedy: bool },
    Backref(usize),
    Look { node: Box<Node>, ahead: bool, neg: bool },
    /// A whole field's result.
    Field,
    /// A word that sounds like this Soundex code.
    Sound(String),
}

/// Matching options that apply to the whole pattern.
#[derive(Debug, Clone, Copy, Default)]
pub struct Flags {
    pub ignore_case: bool,
    /// Treat letters with marks (é, ü, ñ) as their plain letter.
    pub ignore_marks: bool,
    /// Every character a match uses must have `Hc::ok` set.
    pub need_fmt: bool,
}

#[derive(Debug, Clone)]
pub struct Matcher {
    pub root: Node,
    pub flags: Flags,
    pub groups: usize,
    pub names: Vec<(String, usize)>,
}

/// One match: `[start, end)` in the haystack, and each group's span.
#[derive(Debug, Clone)]
pub struct Found {
    pub start: usize,
    pub end: usize,
    pub caps: Vec<Option<(usize, usize)>>,
}

/// The most steps one attempt at one starting place may take. A pattern that runs past it (one that backtracks without
/// end) stops the whole search, which then reports what it found so far.
const STEP_LIMIT: usize = 3_000_000;

/// How many repetitions of a group may be nested in the backtracking at once. Each one costs several stack frames, and
/// the browser's stack is small, so a pattern like `(\w+ )+` over a long paragraph stops the search at this depth
/// (like the step limit) instead of overflowing the stack.
const REP_DEPTH_LIMIT: usize = 100;

/// The longest text to find that Word accepts, and the longest wildcard or regex pattern the engine compiles: past this
/// the matcher's recursion (a few frames for each group or alternative) could reach the end of the browser's stack.
pub const MAX_FIND_CHARS: usize = 255;

struct Ctx<'a> {
    h: &'a [Hc],
    caps: Vec<Option<(usize, usize)>>,
    steps: usize,
    /// Repetitions of groups currently being tried, one inside another.
    depth: usize,
}

pub fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// The plain letter of an accented one (Latin letters), or the character itself.
pub fn base_letter(c: char) -> char {
    if c.is_ascii() {
        return c;
    }
    const TABLE: &[(&str, char)] = &[
        ("àáâãäåāăąǎ", 'a'),
        ("ÀÁÂÃÄÅĀĂĄǍ", 'A'),
        ("çćĉċč", 'c'),
        ("ÇĆĈĊČ", 'C'),
        ("ďđ", 'd'),
        ("ĎĐ", 'D'),
        ("èéêëēĕėęě", 'e'),
        ("ÈÉÊËĒĔĖĘĚ", 'E'),
        ("ĝğġģ", 'g'),
        ("ĜĞĠĢ", 'G'),
        ("ĥħ", 'h'),
        ("ĤĦ", 'H'),
        ("ìíîïĩīĭįı", 'i'),
        ("ÌÍÎÏĨĪĬĮİ", 'I'),
        ("ĵ", 'j'),
        ("Ĵ", 'J'),
        ("ķ", 'k'),
        ("Ķ", 'K'),
        ("ĺļľŀł", 'l'),
        ("ĹĻĽĿŁ", 'L'),
        ("ñńņňŉ", 'n'),
        ("ÑŃŅŇ", 'N'),
        ("òóôõöøōŏőǒ", 'o'),
        ("ÒÓÔÕÖØŌŎŐǑ", 'O'),
        ("ŕŗř", 'r'),
        ("ŔŖŘ", 'R'),
        ("śŝşš", 's'),
        ("ŚŜŞŠ", 'S'),
        ("ţťŧ", 't'),
        ("ŢŤŦ", 'T'),
        ("ùúûüũūŭůűųǔ", 'u'),
        ("ÙÚÛÜŨŪŬŮŰŲǓ", 'U'),
        ("ŵ", 'w'),
        ("Ŵ", 'W'),
        ("ýÿŷ", 'y'),
        ("ÝŶŸ", 'Y'),
        ("źżž", 'z'),
        ("ŹŻŽ", 'Z'),
    ];
    for (set, base) in TABLE {
        if set.contains(c) {
            return *base;
        }
    }
    c
}

fn lower(c: char) -> char {
    let mut it = c.to_lowercase();
    match (it.next(), it.next()) {
        (Some(l), None) => l,
        _ => c,
    }
}

fn upper(c: char) -> char {
    let mut it = c.to_uppercase();
    match (it.next(), it.next()) {
        (Some(u), None) => u,
        _ => c,
    }
}

impl Flags {
    pub fn fold(&self, c: char) -> char {
        let c = if self.ignore_marks { base_letter(c) } else { c };
        if self.ignore_case {
            lower(c)
        } else {
            c
        }
    }
}

fn in_class(cl: Class, c: char) -> bool {
    match cl {
        Class::Digit => c.is_ascii_digit(),
        Class::NotDigit => !c.is_ascii_digit(),
        Class::Word => is_word(c),
        Class::NotWord => !is_word(c),
        Class::Space => c.is_whitespace() || c == '\u{feff}',
        Class::NotSpace => !(c.is_whitespace() || c == '\u{feff}'),
        Class::Letter => c.is_alphabetic(),
        Class::NotLetter => !c.is_alphabetic(),
        Class::Upper => c.is_uppercase(),
        Class::Lower => c.is_lowercase(),
        Class::Number => c.is_numeric(),
        Class::Punct => c.is_ascii_punctuation() || (!c.is_alphanumeric() && !c.is_whitespace() && !c.is_control() && !c.is_ascii() && is_punct_symbol(c)),
    }
}

fn is_punct_symbol(c: char) -> bool {
    matches!(c, '\u{2010}'..='\u{2027}' | '\u{2030}'..='\u{205e}' | '\u{00a1}' | '\u{00a7}' | '\u{00ab}' | '\u{00b6}' | '\u{00b7}' | '\u{00bb}' | '\u{00bf}' | '\u{3001}'..='\u{3003}' | '\u{300c}'..='\u{3011}')
}

/// Whether a character counts as punctuation for "Ignore punctuation characters".
pub fn is_punctuation(c: char) -> bool {
    in_class(Class::Punct, c)
}

/// Soundex code of a word (first letter plus three digits), the way Word's "Sounds like" groups words.
pub fn soundex(word: &str) -> String {
    let mut out = String::new();
    let mut last = '0';
    for c in word.chars().map(base_letter).filter(|c| c.is_ascii_alphabetic()) {
        let u = c.to_ascii_uppercase();
        let code = match u {
            'B' | 'F' | 'P' | 'V' => '1',
            'C' | 'G' | 'J' | 'K' | 'Q' | 'S' | 'X' | 'Z' => '2',
            'D' | 'T' => '3',
            'L' => '4',
            'M' | 'N' => '5',
            'R' => '6',
            _ => '0',
        };
        if out.is_empty() {
            out.push(u);
            last = code;
        } else {
            if code != '0' && code != last {
                out.push(code);
            }
            if u != 'H' && u != 'W' {
                last = code;
            }
        }
        if out.len() == 4 {
            break;
        }
    }
    while !out.is_empty() && out.len() < 4 {
        out.push('0');
    }
    out
}

impl Matcher {
    /// The shortest search that cannot match before a given character, for skipping ahead: the first literal.
    fn first_char(&self) -> Option<char> {
        fn first(n: &Node) -> Option<char> {
            match n {
                Node::Char(c) => Some(*c),
                Node::Cat(v) => v.first().and_then(first),
                Node::Group(n, _) => first(n),
                Node::Rep { node, min, .. } if *min > 0 => first(node),
                _ => None,
            }
        }
        first(&self.root)
    }

    /// The first match that starts at or after `from`. `None` also when the step limit is used up.
    pub fn find_at(&self, h: &[Hc], from: usize) -> Option<Found> {
        let mut ctx = Ctx { h, caps: vec![None; self.groups + 1], steps: 0, depth: 0 };
        let first = self.first_char().map(|c| self.flags.fold(c));
        let mut s = from;
        while s <= h.len() {
            if let Some(fc) = first {
                match h[s..].iter().position(|x| self.flags.fold(x.c) == fc) {
                    Some(k) => s += k,
                    None => return None,
                }
            }
            for c in ctx.caps.iter_mut() {
                *c = None;
            }
            ctx.steps = 0;
            let mut end = None;
            let mut done = |_: &mut Ctx, j: usize| {
                end = Some(j);
                true
            };
            let ran = self.run(&mut ctx, &self.root, s, &mut done);
            // A search stopped by a limit may have unwound into a shorter match; don't report that one.
            if ran && ctx.steps <= STEP_LIMIT {
                if let Some(e) = end {
                    return Some(Found { start: s, end: e, caps: ctx.caps.clone() });
                }
            }
            if ctx.steps > STEP_LIMIT {
                return None;
            }
            s += 1;
        }
        None
    }

    /// Every match from the start, none overlapping. An empty match is skipped. At most `limit` are returned.
    pub fn find_all(&self, h: &[Hc], limit: usize) -> Vec<Found> {
        let mut out = Vec::new();
        let mut at = 0;
        while at <= h.len() && out.len() < limit {
            let Some(f) = self.find_at(h, at) else { break };
            at = if f.end > f.start { f.end } else { f.start + 1 };
            if f.end > f.start {
                out.push(f);
            }
        }
        out
    }

    fn eat(&self, ctx: &Ctx, i: usize) -> bool {
        i < ctx.h.len() && (!self.flags.need_fmt || ctx.h[i].ok)
    }

    /// Whether the single-character node matches the character at `i`.
    fn one(&self, ctx: &Ctx, n: &Node, i: usize) -> bool {
        if !self.eat(ctx, i) {
            return false;
        }
        let x = ctx.h[i];
        match n {
            Node::Char(c) => self.flags.fold(x.c) == self.flags.fold(*c),
            Node::Dot => x.kind & (PMARK | LINE | PAGE | COLUMN) == 0,
            Node::AnyChar => true,
            Node::Set { items, neg } => self.in_set(items, &x) != *neg,
            Node::Kind(k) => x.kind & k != 0,
            _ => false,
        }
    }

    fn in_set(&self, items: &[SetItem], x: &Hc) -> bool {
        let f = &self.flags;
        let c = x.c;
        items.iter().any(|it| match it {
            SetItem::Ch(a) => f.fold(c) == f.fold(*a),
            SetItem::Range(a, b) => {
                let cands = if f.ignore_case { [c, lower(c), upper(c)] } else { [c, c, c] };
                cands.iter().any(|d| a <= d && d <= b)
            }
            SetItem::Class(cl) => in_class(*cl, c),
            SetItem::Kind(k) => x.kind & k != 0,
        })
    }

    fn simple(n: &Node) -> bool {
        matches!(n, Node::Char(_) | Node::Dot | Node::AnyChar | Node::Set { .. } | Node::Kind(_))
    }

    /// The body of a repeated group when it is a fixed run of single-character nodes, like `(.)`, `(?:ab)` or `([a-z]\d)`,
    /// with the group number it captures. Such a repetition can be matched with a loop instead of recursion.
    fn fixed_body(n: &Node) -> Option<(Vec<&Node>, Option<usize>)> {
        let mut cap = None;
        let mut n = n;
        while let Node::Group(inner, c) = n {
            if c.is_some() {
                if cap.is_some() {
                    return None;
                }
                cap = *c;
            }
            n = inner;
        }
        let body: Vec<&Node> = match n {
            Node::Cat(v) => v.iter().collect(),
            _ => vec![n],
        };
        if body.is_empty() || !body.iter().all(|x| Self::simple(x)) {
            return None;
        }
        Some((body, cap))
    }

    /// Repeat a fixed-width body with a loop: every pass uses the same number of characters, so the counts to try are
    /// known up front and no recursion is needed.
    #[allow(clippy::too_many_arguments)]
    fn rep_fixed(&self, ctx: &mut Ctx, body: &[&Node], cap: Option<usize>, min: usize, max: usize, greedy: bool, i: usize, k: &mut dyn FnMut(&mut Ctx, usize) -> bool) -> bool {
        let w = body.len();
        let mut most = 0;
        'count: while most < max {
            let at = i + most * w;
            for (t, b) in body.iter().enumerate() {
                if !self.one(ctx, b, at + t) {
                    break 'count;
                }
            }
            most += 1;
        }
        if most < min {
            return false;
        }
        let order: Box<dyn Iterator<Item = usize>> = if greedy { Box::new((min..=most).rev()) } else { Box::new(min..=most) };
        for c in order {
            ctx.steps += 1;
            if ctx.steps > STEP_LIMIT {
                return false;
            }
            let old = cap.map(|g| ctx.caps[g]);
            if let (Some(g), true) = (cap, c > 0) {
                ctx.caps[g] = Some((i + (c - 1) * w, i + c * w));
            }
            if k(ctx, i + c * w) {
                return true;
            }
            if let (Some(g), Some(old)) = (cap, old) {
                ctx.caps[g] = old;
            }
        }
        false
    }

    fn word_at(&self, ctx: &Ctx, i: usize) -> bool {
        i < ctx.h.len() && is_word(ctx.h[i].c)
    }

    fn run(&self, ctx: &mut Ctx, n: &Node, i: usize, k: &mut dyn FnMut(&mut Ctx, usize) -> bool) -> bool {
        ctx.steps += 1;
        if ctx.steps > STEP_LIMIT {
            return false;
        }
        match n {
            Node::Char(_) | Node::Dot | Node::AnyChar | Node::Set { .. } | Node::Kind(_) => self.one(ctx, n, i) && k(ctx, i + 1),
            Node::LineStart => (i == 0 || ctx.h[i - 1].kind & PMARK != 0) && k(ctx, i),
            Node::LineEnd => (i == ctx.h.len() || ctx.h[i].kind & PMARK != 0) && k(ctx, i),
            Node::WordBoundary | Node::NotWordBoundary => {
                let before = i > 0 && is_word(ctx.h[i - 1].c);
                let after = i < ctx.h.len() && is_word(ctx.h[i].c);
                ((before != after) == matches!(n, Node::WordBoundary)) && k(ctx, i)
            }
            Node::WordStart => (!(i > 0 && is_word(ctx.h[i - 1].c)) && self.word_at(ctx, i)) && k(ctx, i),
            Node::WordEnd => (i > 0 && is_word(ctx.h[i - 1].c) && !self.word_at(ctx, i)) && k(ctx, i),
            Node::Group(inner, cap) => match cap {
                None => self.run(ctx, inner, i, k),
                Some(c) => {
                    let c = *c;
                    self.run(ctx, inner, i, &mut |ctx, j| {
                        let old = ctx.caps[c];
                        ctx.caps[c] = Some((i, j));
                        if k(ctx, j) {
                            true
                        } else {
                            ctx.caps[c] = old;
                            false
                        }
                    })
                }
            },
            Node::Cat(v) => self.seq(ctx, v, i, k),
            Node::Alt(v) => v.iter().any(|a| self.run(ctx, a, i, k)),
            Node::Rep { node, min, max, greedy } => {
                if Self::simple(node) {
                    if *greedy {
                        let mut c = 0;
                        while c < *max && self.one(ctx, node, i + c) {
                            c += 1;
                        }
                        let mut c = c as isize;
                        while c >= *min as isize {
                            if k(ctx, i + c as usize) {
                                return true;
                            }
                            c -= 1;
                        }
                        false
                    } else {
                        let mut c = 0;
                        loop {
                            if c >= *min && k(ctx, i + c) {
                                return true;
                            }
                            if c >= *max || !self.one(ctx, node, i + c) {
                                return false;
                            }
                            c += 1;
                            if ctx.steps > STEP_LIMIT {
                                return false;
                            }
                            ctx.steps += 1;
                        }
                    }
                } else if let Some((body, cap)) = Self::fixed_body(node) {
                    self.rep_fixed(ctx, &body, cap, *min, *max, *greedy, i, k)
                } else {
                    self.rep(ctx, node, *min, *max, *greedy, 0, i, k)
                }
            }
            Node::Backref(g) => match ctx.caps.get(*g).copied().flatten() {
                None => k(ctx, i),
                Some((a, b)) => {
                    let len = b - a;
                    if i + len > ctx.h.len() {
                        return false;
                    }
                    for d in 0..len {
                        if self.flags.fold(ctx.h[a + d].c) != self.flags.fold(ctx.h[i + d].c) {
                            return false;
                        }
                    }
                    k(ctx, i + len)
                }
            },
            Node::Look { node, ahead, neg } => {
                let ok = if *ahead {
                    self.run(ctx, node, i, &mut |_, _| true)
                } else {
                    (0..=i).rev().any(|s| self.run(ctx, node, s, &mut |_, j| j == i))
                };
                ok != *neg && k(ctx, i)
            }
            Node::Field => {
                if i >= ctx.h.len() || ctx.h[i].field == 0 {
                    return false;
                }
                let (p, f) = (ctx.h[i].para, ctx.h[i].field);
                if i > 0 && ctx.h[i - 1].para == p && ctx.h[i - 1].field == f {
                    return false;
                }
                let mut j = i;
                while j < ctx.h.len() && ctx.h[j].para == p && ctx.h[j].field == f {
                    j += 1;
                }
                k(ctx, j)
            }
            Node::Sound(code) => {
                if i > 0 && is_word(ctx.h[i - 1].c) {
                    return false;
                }
                let mut j = i;
                let mut w = String::new();
                while j < ctx.h.len() && ctx.h[j].c.is_alphabetic() {
                    w.push(ctx.h[j].c);
                    j += 1;
                }
                j > i && soundex(&w) == *code && k(ctx, j)
            }
        }
    }

    fn seq(&self, ctx: &mut Ctx, v: &[Node], mut i: usize, k: &mut dyn FnMut(&mut Ctx, usize) -> bool) -> bool {
        // A run of single-character nodes (all of a plain search text) is matched in a loop: recursing once per
        // character would nest a few stack frames for each one, and the browser's stack is small.
        let mut v = v;
        while let Some((first, rest)) = v.split_first() {
            if !Self::simple(first) {
                break;
            }
            ctx.steps += 1;
            if ctx.steps > STEP_LIMIT || !self.one(ctx, first, i) {
                return false;
            }
            i += 1;
            v = rest;
        }
        match v.split_first() {
            None => k(ctx, i),
            Some((first, rest)) => self.run(ctx, first, i, &mut |ctx, j| self.seq(ctx, rest, j, k)),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn rep(&self, ctx: &mut Ctx, node: &Node, min: usize, max: usize, greedy: bool, count: usize, i: usize, k: &mut dyn FnMut(&mut Ctx, usize) -> bool) -> bool {
        if ctx.steps > STEP_LIMIT || count > 100_000 {
            return false;
        }
        if ctx.depth >= REP_DEPTH_LIMIT {
            // Too deep for the stack: stop the whole search, as the step limit does.
            ctx.steps = STEP_LIMIT + 1;
            return false;
        }
        ctx.depth += 1;
        let r = self.rep_step(ctx, node, min, max, greedy, count, i, k);
        ctx.depth -= 1;
        r
    }

    #[allow(clippy::too_many_arguments)]
    fn rep_step(&self, ctx: &mut Ctx, node: &Node, min: usize, max: usize, greedy: bool, count: usize, i: usize, k: &mut dyn FnMut(&mut Ctx, usize) -> bool) -> bool {
        let more = |this: &Self, ctx: &mut Ctx, k: &mut dyn FnMut(&mut Ctx, usize) -> bool| {
            count < max
                && this.run(ctx, node, i, &mut |ctx, j| {
                    // An empty step can't make progress.
                    if j == i && count >= min {
                        false
                    } else {
                        this.rep(ctx, node, min, max, greedy, count + 1, j, k)
                    }
                })
        };
        if greedy {
            more(self, ctx, k) || (count >= min && k(ctx, i))
        } else {
            (count >= min && k(ctx, i)) || more(self, ctx, k)
        }
    }
}

// ---------------------------------------------------------------------------
// Compiling the three notations
// ---------------------------------------------------------------------------

/// What each `^x` special character means in the text to find.
fn caret_node(c: char) -> Option<Node> {
    Some(match c {
        'p' => Node::Kind(PMARK),
        't' => Node::Char('\t'),
        'l' => Node::Kind(LINE),
        'm' => Node::Kind(PAGE),
        'n' => Node::Kind(COLUMN),
        'b' => Node::Kind(SECT),
        '?' => Node::AnyChar,
        '#' => Node::Set { items: vec![SetItem::Class(Class::Digit)], neg: false },
        '$' => Node::Set { items: vec![SetItem::Class(Class::Letter)], neg: false },
        '^' => Node::Char('^'),
        '+' => Node::Char('\u{2014}'),
        '=' => Node::Char('\u{2013}'),
        's' => Node::Char('\u{a0}'),
        '-' => Node::Char('\u{ad}'),
        '~' => Node::Char('\u{2011}'),
        'f' => Node::Kind(FOOTNOTE),
        'e' => Node::Kind(ENDNOTE),
        'g' => Node::Kind(GRAPHIC),
        'd' => Node::Field,
        'w' => Node::Set { items: vec![SetItem::Class(Class::Space)], neg: false },
        _ => return None,
    })
}

fn finish(root: Node, groups: usize, names: Vec<(String, usize)>, flags: Flags) -> Matcher {
    Matcher { root, flags, groups, names }
}

/// Plain text with Word's `^` special characters. A `^` followed by anything else is an ordinary caret.
/// `skip` drops characters from the text to find (for "Ignore punctuation" and "Ignore white-space").
pub fn compile_plain(text: &str, flags: Flags, skip: &dyn Fn(char) -> bool, sounds_like: bool) -> Result<Matcher, String> {
    if text.is_empty() {
        return Err("the text to find is empty".into());
    }
    let cs: Vec<char> = text.chars().collect();
    if sounds_like {
        let words: Vec<String> = text.split_whitespace().map(String::from).collect();
        let mut v = Vec::new();
        for (n, w) in words.iter().enumerate() {
            if n > 0 {
                v.push(Node::Rep { node: Box::new(Node::Set { items: vec![SetItem::Class(Class::Space)], neg: false }), min: 1, max: usize::MAX, greedy: true });
            }
            let code = soundex(w);
            if code.is_empty() {
                return Err("sounds like needs words made of letters".into());
            }
            v.push(Node::Sound(code));
        }
        return Ok(finish(Node::Cat(v), 0, vec![], flags));
    }
    let mut v = Vec::new();
    let mut i = 0;
    while i < cs.len() {
        let c = cs[i];
        if c == '^' && i + 1 < cs.len() {
            if let Some(n) = caret_node(cs[i + 1]) {
                v.push(n);
                i += 2;
                continue;
            }
        }
        if !skip(c) {
            v.push(Node::Char(c));
        }
        i += 1;
    }
    if v.is_empty() {
        return Err("the text to find has nothing left once the ignored characters are removed".into());
    }
    Ok(finish(Node::Cat(v), 0, vec![], flags))
}

struct Wild<'a> {
    cs: &'a [char],
    i: usize,
    groups: usize,
}

/// Word's wildcard notation.
pub fn compile_wildcards(text: &str, flags: Flags) -> Result<Matcher, String> {
    if text.is_empty() {
        return Err("the text to find is empty".into());
    }
    let cs: Vec<char> = text.chars().collect();
    let mut w = Wild { cs: &cs, i: 0, groups: 0 };
    let root = w.expr(false)?;
    if w.i < cs.len() {
        return Err("a ) has no ( before it".into());
    }
    let groups = w.groups;
    Ok(finish(root, groups, vec![], flags))
}

impl Wild<'_> {
    fn peek(&self) -> Option<char> {
        self.cs.get(self.i).copied()
    }

    fn expr(&mut self, nested: bool) -> Result<Node, String> {
        let mut items: Vec<Node> = Vec::new();
        while let Some(c) = self.peek() {
            if c == ')' {
                if nested {
                    break;
                }
                return Err("a ) has no ( before it".into());
            }
            self.i += 1;
            let atom = match c {
                '(' => {
                    self.groups += 1;
                    let n = self.groups;
                    let inner = self.expr(true)?;
                    if self.peek() != Some(')') {
                        return Err("a ( has no ) after it".into());
                    }
                    self.i += 1;
                    Node::Group(Box::new(inner), Some(n))
                }
                '?' => Node::AnyChar,
                '*' => Node::Rep { node: Box::new(Node::Dot), min: 0, max: usize::MAX, greedy: false },
                '<' => Node::WordStart,
                '>' => Node::WordEnd,
                '[' => self.set()?,
                '\\' => {
                    let Some(n) = self.peek() else { return Err("a \\ needs a character after it".into()) };
                    self.i += 1;
                    match n.to_digit(10) {
                        Some(d) if (1..=9).contains(&d) => Node::Backref(d as usize),
                        _ => Node::Char(n),
                    }
                }
                '^' => {
                    let n = self.peek();
                    match n.and_then(caret_node) {
                        Some(node) => {
                            self.i += 1;
                            node
                        }
                        None => Node::Char('^'),
                    }
                }
                c => Node::Char(c),
            };
            let atom = self.quant(atom)?;
            items.push(atom);
        }
        Ok(Node::Cat(items))
    }

    fn quant(&mut self, atom: Node) -> Result<Node, String> {
        let mut atom = atom;
        loop {
            match self.peek() {
                Some('@') => {
                    self.i += 1;
                    atom = Node::Rep { node: Box::new(atom), min: 1, max: usize::MAX, greedy: true };
                }
                Some('{') => {
                    let close = self.cs[self.i..].iter().position(|c| *c == '}').ok_or("a { has no } after it")?;
                    let body: String = self.cs[self.i + 1..self.i + close].iter().collect();
                    self.i += close + 1;
                    let (lo, hi) = match body.split_once(',') {
                        Some((a, b)) => (a.trim().parse::<usize>().ok(), b.trim().parse::<usize>().ok()),
                        None => {
                            let n = body.trim().parse::<usize>().map_err(|_| "{ } needs a number, like {2} or {1,3}".to_string())?;
                            (Some(n), Some(n))
                        }
                    };
                    let (min, max) = (lo.unwrap_or(0), hi.unwrap_or(usize::MAX));
                    if min > max {
                        return Err("{ } has its numbers the wrong way round".into());
                    }
                    atom = Node::Rep { node: Box::new(atom), min, max, greedy: true };
                }
                _ => return Ok(atom),
            }
        }
    }

    fn set(&mut self) -> Result<Node, String> {
        let mut neg = false;
        if self.peek() == Some('!') {
            neg = true;
            self.i += 1;
        }
        let mut items = Vec::new();
        let mut closed = false;
        while let Some(c) = self.peek() {
            self.i += 1;
            if c == ']' && !items.is_empty() {
                closed = true;
                break;
            }
            let mut a = c;
            if c == '\\' {
                a = self.peek().ok_or("a \\ needs a character after it")?;
                self.i += 1;
            }
            if self.peek() == Some('-') && self.cs.get(self.i + 1).is_some_and(|n| *n != ']') {
                self.i += 1;
                let mut b = self.peek().unwrap_or(a);
                self.i += 1;
                if b == '\\' {
                    b = self.peek().ok_or("a \\ needs a character after it")?;
                    self.i += 1;
                }
                if a > b {
                    return Err("a range in [ ] runs backwards".into());
                }
                items.push(SetItem::Range(a, b));
            } else {
                items.push(SetItem::Ch(a));
            }
        }
        if !closed {
            return Err("a [ has no ] after it".into());
        }
        Ok(Node::Set { items, neg })
    }
}

struct Rx<'a> {
    cs: &'a [char],
    i: usize,
    groups: usize,
    names: Vec<(String, usize)>,
}

/// JavaScript regular expression syntax.
pub fn compile_regex(text: &str, flags: Flags) -> Result<Matcher, String> {
    if text.is_empty() {
        return Err("the pattern is empty".into());
    }
    let cs: Vec<char> = text.chars().collect();
    let mut r = Rx { cs: &cs, i: 0, groups: 0, names: vec![] };
    let root = r.alt()?;
    if r.i < cs.len() {
        return Err("unmatched )".into());
    }
    // Names given to groups after a backreference by name need resolving; none are forward references here.
    Ok(finish(root, r.groups, r.names, flags))
}

impl Rx<'_> {
    fn peek(&self) -> Option<char> {
        self.cs.get(self.i).copied()
    }

    fn eat(&mut self, c: char) -> bool {
        if self.peek() == Some(c) {
            self.i += 1;
            true
        } else {
            false
        }
    }

    fn alt(&mut self) -> Result<Node, String> {
        let mut alts = vec![self.cat()?];
        while self.eat('|') {
            alts.push(self.cat()?);
        }
        Ok(if alts.len() == 1 { alts.pop().unwrap() } else { Node::Alt(alts) })
    }

    fn cat(&mut self) -> Result<Node, String> {
        let mut v = Vec::new();
        while let Some(c) = self.peek() {
            if c == '|' || c == ')' {
                break;
            }
            let atom = self.atom()?;
            v.push(self.quant(atom)?);
        }
        Ok(Node::Cat(v))
    }

    fn quant(&mut self, atom: Node) -> Result<Node, String> {
        let (min, max) = match self.peek() {
            Some('*') => (0, usize::MAX),
            Some('+') => (1, usize::MAX),
            Some('?') => (0, 1),
            Some('{') => {
                let Some(close) = self.cs[self.i..].iter().position(|c| *c == '}') else { return Ok(atom) };
                let body: String = self.cs[self.i + 1..self.i + close].iter().collect();
                let parsed = match body.split_once(',') {
                    Some((a, b)) => a.parse::<usize>().ok().map(|a| (a, if b.is_empty() { Some(usize::MAX) } else { b.parse::<usize>().ok() })),
                    None => body.parse::<usize>().ok().map(|n| (n, Some(n))),
                };
                match parsed {
                    Some((a, Some(b))) if a <= b => {
                        self.i += close;
                        (a, b)
                    }
                    // Not a quantifier: a literal brace, as in JavaScript.
                    _ => return Ok(atom),
                }
            }
            _ => return Ok(atom),
        };
        self.i += 1;
        let greedy = !self.eat('?');
        if matches!(atom, Node::LineStart | Node::LineEnd | Node::WordBoundary | Node::NotWordBoundary) {
            return Err("nothing to repeat".into());
        }
        Ok(Node::Rep { node: Box::new(atom), min, max, greedy })
    }

    fn atom(&mut self) -> Result<Node, String> {
        let c = self.peek().ok_or("the pattern ends too soon")?;
        self.i += 1;
        Ok(match c {
            '.' => Node::Dot,
            '^' => Node::LineStart,
            '$' => Node::LineEnd,
            '*' | '+' | '?' => return Err("nothing to repeat".into()),
            '(' => {
                if self.eat('?') {
                    if self.eat(':') {
                        let inner = self.alt()?;
                        self.close()?;
                        Node::Group(Box::new(inner), None)
                    } else if matches!(self.peek(), Some('=') | Some('!')) {
                        let neg = self.peek() == Some('!');
                        self.i += 1;
                        let inner = self.alt()?;
                        self.close()?;
                        Node::Look { node: Box::new(inner), ahead: true, neg }
                    } else if self.eat('<') {
                        if matches!(self.peek(), Some('=') | Some('!')) {
                            let neg = self.peek() == Some('!');
                            self.i += 1;
                            let inner = self.alt()?;
                            self.close()?;
                            Node::Look { node: Box::new(inner), ahead: false, neg }
                        } else {
                            let mut name = String::new();
                            while let Some(ch) = self.peek() {
                                self.i += 1;
                                if ch == '>' {
                                    break;
                                }
                                name.push(ch);
                            }
                            self.groups += 1;
                            let n = self.groups;
                            self.names.push((name, n));
                            let inner = self.alt()?;
                            self.close()?;
                            Node::Group(Box::new(inner), Some(n))
                        }
                    } else {
                        return Err("this group syntax isn't supported".into());
                    }
                } else {
                    self.groups += 1;
                    let n = self.groups;
                    let inner = self.alt()?;
                    self.close()?;
                    Node::Group(Box::new(inner), Some(n))
                }
            }
            '[' => self.class()?,
            '\\' => self.escape(false)?,
            c => Node::Char(c),
        })
    }

    fn close(&mut self) -> Result<(), String> {
        if self.eat(')') {
            Ok(())
        } else {
            Err("a ( has no ) after it".into())
        }
    }

    fn hex(&mut self, n: usize) -> Result<char, String> {
        let s: String = self.cs.get(self.i..self.i + n).ok_or("a hex escape ends too soon")?.iter().collect();
        self.i += n;
        u32::from_str_radix(&s, 16).ok().and_then(char::from_u32).ok_or_else(|| "not a valid character code".to_string())
    }

    /// After a backslash. In a class (`inset`) the node is a single item and \b is a backspace.
    fn escape(&mut self, inset: bool) -> Result<Node, String> {
        let c = self.peek().ok_or("a \\ needs a character after it")?;
        self.i += 1;
        let class = |cl| Node::Set { items: vec![SetItem::Class(cl)], neg: false };
        Ok(match c {
            'd' => class(Class::Digit),
            'D' => class(Class::NotDigit),
            'w' => class(Class::Word),
            'W' => class(Class::NotWord),
            's' => class(Class::Space),
            'S' => class(Class::NotSpace),
            'b' if !inset => Node::WordBoundary,
            'B' if !inset => Node::NotWordBoundary,
            'b' => Node::Char('\u{8}'),
            't' => Node::Char('\t'),
            // The line break and the paragraph mark are both "a new line" to someone typing \n.
            'n' | 'r' => Node::Set { items: vec![SetItem::Kind(PMARK | LINE)], neg: false },
            'f' => Node::Kind(PAGE),
            'v' => Node::Char('\u{b}'),
            '0' => Node::Char('\0'),
            'x' => Node::Char(self.hex(2)?),
            'u' => {
                if self.eat('{') {
                    let close = self.cs[self.i..].iter().position(|c| *c == '}').ok_or("a \\u{ has no }")?;
                    let s: String = self.cs[self.i..self.i + close].iter().collect();
                    self.i += close + 1;
                    Node::Char(u32::from_str_radix(&s, 16).ok().and_then(char::from_u32).ok_or("not a valid character code")?)
                } else {
                    Node::Char(self.hex(4)?)
                }
            }
            'p' | 'P' => {
                if !self.eat('{') {
                    return Err("\\p needs a name in braces, like \\p{L}".into());
                }
                let close = self.cs[self.i..].iter().position(|c| *c == '}').ok_or("a \\p{ has no }")?;
                let name: String = self.cs[self.i..self.i + close].iter().collect();
                self.i += close + 1;
                let cl = match name.as_str() {
                    "L" | "Letter" | "Alphabetic" => Class::Letter,
                    "Lu" | "Uppercase_Letter" | "Uppercase" => Class::Upper,
                    "Ll" | "Lowercase_Letter" | "Lowercase" => Class::Lower,
                    "N" | "Nd" | "Number" => Class::Number,
                    "P" | "Punctuation" => Class::Punct,
                    "Z" | "Zs" | "Space_Separator" => Class::Space,
                    _ => return Err(format!("\\p{{{name}}} isn't supported")),
                };
                let cl = if c == 'P' {
                    match cl {
                        Class::Letter => Class::NotLetter,
                        Class::Space => Class::NotSpace,
                        Class::Digit => Class::NotDigit,
                        other => return Ok(Node::Set { items: vec![SetItem::Class(other)], neg: true }),
                    }
                } else {
                    cl
                };
                class(cl)
            }
            'k' => {
                if !self.eat('<') {
                    return Err("\\k needs a group name, like \\k<name>".into());
                }
                let mut name = String::new();
                while let Some(ch) = self.peek() {
                    self.i += 1;
                    if ch == '>' {
                        break;
                    }
                    name.push(ch);
                }
                let n = self.names.iter().find(|(k, _)| *k == name).map(|(_, n)| *n).ok_or_else(|| format!("no group is named {name}"))?;
                Node::Backref(n)
            }
            d if d.is_ascii_digit() => {
                let mut n = d.to_digit(10).unwrap() as usize;
                while let Some(x) = self.peek().and_then(|x| x.to_digit(10)) {
                    n = n * 10 + x as usize;
                    self.i += 1;
                }
                Node::Backref(n)
            }
            c => Node::Char(c),
        })
    }

    fn class(&mut self) -> Result<Node, String> {
        let neg = self.eat('^');
        let mut items = Vec::new();
        loop {
            let c = self.peek().ok_or("a [ has no ] after it")?;
            self.i += 1;
            if c == ']' {
                break;
            }
            let lo = if c == '\\' {
                match self.escape(true)? {
                    Node::Char(x) => x,
                    Node::Set { items: mut it, .. } => {
                        items.append(&mut it);
                        continue;
                    }
                    _ => continue,
                }
            } else {
                c
            };
            if self.peek() == Some('-') && self.cs.get(self.i + 1).is_some_and(|n| *n != ']') {
                self.i += 1;
                let c2 = self.peek().ok_or("a range ends too soon")?;
                self.i += 1;
                let hi = if c2 == '\\' {
                    match self.escape(true)? {
                        Node::Char(x) => x,
                        _ => return Err("a range can't end in a class".into()),
                    }
                } else {
                    c2
                };
                if lo > hi {
                    return Err("a range in [ ] runs backwards".into());
                }
                items.push(SetItem::Range(lo, hi));
            } else {
                items.push(SetItem::Ch(lo));
            }
        }
        Ok(Node::Set { items, neg })
    }
}

/// Wrap a pattern so it only matches whole words (`both`), or at the start (`prefix`) or end (`suffix`) of one.
pub fn wrap_words(m: &mut Matcher, start: bool, end: bool) {
    let root = std::mem::replace(&mut m.root, Node::Cat(vec![]));
    let mut v = Vec::new();
    if start {
        v.push(Node::WordBoundary);
    }
    v.push(Node::Group(Box::new(root), None));
    if end {
        v.push(Node::WordBoundary);
    }
    m.root = Node::Cat(v);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hay(s: &str) -> Vec<Hc> {
        s.chars()
            .enumerate()
            .map(|(i, c)| {
                let mut h = Hc::plain(c);
                h.off = i as u32;
                if c == '\r' {
                    h.kind = PMARK;
                    h.len = 0;
                }
                h
            })
            .collect()
    }

    fn texts(m: &Matcher, s: &str) -> Vec<String> {
        let h = hay(s);
        m.find_all(&h, 1000).iter().map(|f| h[f.start..f.end].iter().map(|x| x.c).collect()).collect()
    }

    fn flags(ci: bool) -> Flags {
        Flags { ignore_case: ci, ..Default::default() }
    }

    #[test]
    fn plain_text_and_specials() {
        let m = compile_plain("a^pb", flags(false), &|_| false, false).unwrap();
        assert_eq!(texts(&m, "xa\rbx"), vec!["a\rb"]);
        let m = compile_plain("^#^#", flags(false), &|_| false, false).unwrap();
        assert_eq!(texts(&m, "ab 12 345"), vec!["12", "34"]);
        let m = compile_plain("x^2", flags(false), &|_| false, false).unwrap();
        assert_eq!(texts(&m, "E=x^2"), vec!["x^2"]);
        let m = compile_plain("a^^b", flags(false), &|_| false, false).unwrap();
        assert_eq!(texts(&m, "a^b"), vec!["a^b"]);
    }

    #[test]
    fn case_and_marks() {
        let m = compile_plain("apple", flags(true), &|_| false, false).unwrap();
        assert_eq!(texts(&m, "Apple APPLE apple"), vec!["Apple", "APPLE", "apple"]);
        let m = compile_plain("cafe", Flags { ignore_marks: true, ..Default::default() }, &|_| false, false).unwrap();
        assert_eq!(texts(&m, "café cafe"), vec!["café", "cafe"]);
    }

    #[test]
    fn wildcards_match_words_sets_and_counts() {
        let m = compile_wildcards("<s?t>", flags(false)).unwrap();
        assert_eq!(texts(&m, "sat set sit seat"), vec!["sat", "set", "sit"]);
        let m = compile_wildcards("[a-c]{2,3}", flags(false)).unwrap();
        assert_eq!(texts(&m, "abcd ca b"), vec!["abc", "ca"]);
        let m = compile_wildcards("[!a-m]at", flags(false)).unwrap();
        assert_eq!(texts(&m, "cat rat bat sat"), vec!["rat", "sat"]);
        let m = compile_wildcards("ap@le", flags(false)).unwrap();
        assert_eq!(texts(&m, "aple apple appple ale"), vec!["aple", "apple", "appple"]);
        let m = compile_wildcards("s*d", flags(false)).unwrap();
        assert_eq!(texts(&m, "sad started"), vec!["sad", "started"]);
        let m = compile_wildcards("(a)(b)\\2\\1", flags(false)).unwrap();
        assert_eq!(texts(&m, "abba"), vec!["abba"]);
        let m = compile_wildcards("\\*", flags(false)).unwrap();
        assert_eq!(texts(&m, "a*b"), vec!["*"]);
        assert!(compile_wildcards("[abc", flags(false)).is_err());
    }

    #[test]
    fn regex_basics() {
        let m = compile_regex(r"(\d+)-(\d+)", flags(false)).unwrap();
        let h = hay("pages 12-45 and 3-4");
        let all = m.find_all(&h, 10);
        assert_eq!(all.len(), 2);
        let g = all[0].caps[1].unwrap();
        assert_eq!(h[g.0..g.1].iter().map(|x| x.c).collect::<String>(), "12");
        let m = compile_regex(r"\bcat(s|tle)?\b", flags(false)).unwrap();
        assert_eq!(texts(&m, "cat cats cattle catalog"), vec!["cat", "cats", "cattle"]);
        let m = compile_regex(r"a.+?b", flags(false)).unwrap();
        assert_eq!(texts(&m, "a1b2b"), vec!["a1b"]);
        let m = compile_regex(r"(?<=\$)\d+", flags(false)).unwrap();
        assert_eq!(texts(&m, "cost $42 or 7"), vec!["42"]);
        let m = compile_regex(r"foo(?!bar)", flags(false)).unwrap();
        assert_eq!(texts(&m, "foobar foobaz"), vec!["foo"]);
        let m = compile_regex(r"(\w)\1", flags(false)).unwrap();
        assert_eq!(texts(&m, "hello book"), vec!["ll", "oo"]);
        let m = compile_regex(r"^\w+", flags(false)).unwrap();
        assert_eq!(texts(&m, "one two\rthree four"), vec!["one", "three"]);
        let m = compile_regex(r"\p{Lu}\p{Ll}+", flags(false)).unwrap();
        assert_eq!(texts(&m, "Hello world Élan"), vec!["Hello", "Élan"]);
        assert!(compile_regex("(a", flags(false)).is_err());
        assert!(compile_regex("*a", flags(false)).is_err());
    }

    #[test]
    fn regex_does_not_blow_up() {
        let m = compile_regex(r"(a+)+b", flags(false)).unwrap();
        let h = hay(&"a".repeat(60));
        // The step limit ends the search instead of hanging.
        assert!(m.find_all(&h, 10).is_empty());
    }

    #[test]
    fn group_repetition_survives_long_paragraphs() {
        let long = "word ".repeat(4000);
        // Fixed-width groups run as loops, so they match the whole paragraph.
        let m = compile_regex("(.)+", flags(false)).unwrap();
        let h = hay(&long);
        let found = m.find_all(&h, 10);
        assert_eq!(found.len(), 1);
        assert_eq!((found[0].start, found[0].end), (0, long.len()));
        assert_eq!(found[0].caps[1], Some((long.len() - 1, long.len())));
        // Groups that can't be a loop stop at the depth limit instead of overflowing the stack.
        let m = compile_regex(r"(\w+ )+", flags(false)).unwrap();
        assert!(m.find_all(&h, 10).is_empty());
        let m = compile_regex(r"(\w+ )+x", flags(false)).unwrap();
        assert!(m.find_all(&h, 10).is_empty());
        // A short one still matches.
        let m = compile_regex(r"(\w+ )+", flags(false)).unwrap();
        assert_eq!(texts(&m, "one two three"), vec!["one two "]);
    }

    #[test]
    fn a_long_search_text_does_not_nest_calls_per_character() {
        // A small stack stands in for the browser's: one call per pattern character would not fit in it.
        std::thread::Builder::new()
            .stack_size(256 * 1024)
            .spawn(|| {
                // Letters from a scrambled counter, so the text has no repeats to match early.
                let para: String = (0..20_000u64).map(|i| (b'a' + ((i * 2_654_435_761) >> 9) as u8 % 25) as char).collect();
                let h = hay(&para);
                for n in [MAX_FIND_CHARS, 5000] {
                    let needle = &para[19_000 - n..19_000];
                    let m = compile_plain(needle, flags(false), &|_| false, false).unwrap();
                    let found = m.find_all(&h, 10);
                    assert!(found.iter().any(|f| (f.start, f.end) == (19_000 - n, 19_000)), "{n}");
                    // A pattern that nearly matches everywhere (all but the last character) is found nowhere.
                    let miss = format!("{needle}z");
                    let m = compile_plain(&miss, flags(false), &|_| false, false).unwrap();
                    assert!(m.find_all(&h, 10).is_empty());
                    let mut m = compile_plain(needle, flags(false), &|_| false, false).unwrap();
                    wrap_words(&mut m, true, true);
                    assert!(m.find_all(&h, 10).len() <= 1);
                }
                // The longest pattern of other kinds: literals with groups, and a wildcard text.
                let lit = "ab".repeat(MAX_FIND_CHARS / 2 - 1);
                let m = compile_regex(&format!("({lit})c"), flags(false)).unwrap();
                assert!(m.find_all(&h, 10).is_empty());
                let long = "abc ".repeat(5000);
                let h = hay(&long);
                let m = compile_regex(&"(?:abc )".repeat(MAX_FIND_CHARS / 8), flags(false)).unwrap();
                assert!(!m.find_all(&h, 10).is_empty());
                let m = compile_regex(&"(\\w+ )".repeat(MAX_FIND_CHARS / 6), flags(false)).unwrap();
                assert!(!m.find_all(&h, 10).is_empty());
            })
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    fn whole_word_wrapping() {
        let mut m = compile_plain("cat", flags(false), &|_| false, false).unwrap();
        wrap_words(&mut m, true, true);
        assert_eq!(texts(&m, "cat concat cats cat."), vec!["cat", "cat"]);
        let mut m = compile_plain("cat", flags(false), &|_| false, false).unwrap();
        wrap_words(&mut m, true, false);
        assert_eq!(texts(&m, "cat concat cats"), vec!["cat", "cat"]);
    }

    #[test]
    fn sounds_like() {
        let m = compile_plain("smith", flags(true), &|_| false, true).unwrap();
        assert_eq!(texts(&m, "Smith and Smythe but not Jones"), vec!["Smith", "Smythe"]);
    }

    #[test]
    fn ignore_case_in_classes_and_ranges() {
        let m = compile_regex("[a-c]+", flags(true)).unwrap();
        assert_eq!(texts(&m, "ABc d"), vec!["ABc"]);
    }
}
