//! Groups and text: opening and closing groups, destinations, and turning bytes and symbols into text.

use super::*;
use crate::fragment::Builder;
use std::collections::{BTreeMap, HashMap, HashSet};
use super::{codepages::*, lexer::*, state::*};

impl Interp {
    pub(super) fn new() -> Interp {
        let mut b = Builder::new();
        b.set_block(true);
        Interp {
            b,
            stack: vec![Gs::root()],
            over: 0,
            rtf_seen: false,
            done: false,
            skip: 0,
            hi: None,
            any_content: false,
            ansicpg: 1252,
            deff: 0,
            deflang: 0,
            fonts: HashMap::new(),
            font_cur: None,
            colors: Vec::new(),
            color_acc: (None, None, None),
            pstyles: HashMap::new(),
            cstyles: HashMap::new(),
            style_acc: StyleAcc::default(),
            style_ids: HashMap::new(),
            style_busy: HashSet::new(),
            used_ids: HashSet::new(),
            eff: HashMap::new(),
            lists: HashMap::new(),
            list_acc: ListDefAcc::default(),
            level_acc: LevelDef::default(),
            level_text: Vec::new(),
            overrides: HashMap::new(),
            ovr_acc: Override::default(),
            ovr_ls: 0,
            lfo_start: None,
            ls_use: HashMap::new(),
            pn_buf: String::new(),
            pn_seen: false,
            pn_spec: None,
            legacy: None,
            fields: Vec::new(),
            pict: PictAcc::default(),
            stories: vec![Story::default()],
            notes: BTreeMap::new(),
            note_count: 0,
            builtins: HashMap::new(),
        }
    }

    /// Word's definition of a built-in style, added to the fragment once; its style id.
    pub(super) fn builtin(&mut self, name: &'static str) -> Option<String> {
        if let Some(v) = self.builtins.get(name) {
            return v.clone();
        }
        let v = self.b.builtin_style(name);
        self.builtins.insert(name, v.clone());
        v
    }

    pub(super) fn g(&mut self) -> &mut Gs {
        let n = self.stack.len();
        &mut self.stack[n - 1]
    }

    pub(super) fn story(&mut self) -> &mut Story {
        let n = self.stories.len();
        &mut self.stories[n - 1]
    }

    pub(super) fn run(&mut self, src: &str) {
        let src = src.strip_prefix('\u{feff}').unwrap_or(src);
        let mut lx = Lexer::new(src);
        while let Some(t) = lx.next_tok() {
            if self.done {
                break;
            }
            match t {
                Tok::Open => self.open_group(),
                Tok::Close => self.close_group_tok(),
                _ if self.over > 0 => {}
                Tok::Word(n, p) => self.word(n, p),
                Tok::Sym(c) => self.sym(c),
                Tok::Hex(b) => self.hex(b),
                Tok::Text(s) => self.text(s),
                Tok::Bin(s) => self.bin(s),
            }
        }
    }

    // -----------------------------------------------------------------------
    // Groups
    // -----------------------------------------------------------------------

    pub(super) fn open_group(&mut self) {
        if self.stack.len() >= MAX_DEPTH {
            self.over += 1;
            return;
        }
        self.g().first = false;
        let mut g = self.stack[self.stack.len() - 1].clone();
        g.own = Own::None;
        g.first = true;
        g.star = false;
        if g.dest == Dest::StyleSheet {
            g.dest = Dest::StyleEntry;
            g.own = Own::Style;
            g.ch = CharFmt::default();
            g.pf = ParaFmt::default();
            self.style_acc = StyleAcc::default();
        }
        self.stack.push(g);
        self.skip = 0;
        self.hi = None;
    }

    pub(super) fn close_group_tok(&mut self) {
        if self.over > 0 {
            self.over -= 1;
            return;
        }
        self.close_group();
    }

    pub(super) fn close_group(&mut self) {
        if self.stack.len() <= 1 {
            return;
        }
        if self.stack.len() == 2 {
            // The last paragraph may not end with \par; it still has the formatting of this group.
            self.finalize_body();
        }
        let own = self.stack[self.stack.len() - 1].own;
        if own != Own::None {
            let g = self.stack[self.stack.len() - 1].clone();
            self.finish_own(own, &g);
        }
        self.stack.pop();
        self.skip = 0;
        self.hi = None;
        if self.stack.len() == 1 && self.rtf_seen {
            self.done = true;
        }
    }

    pub(super) fn finalize_body(&mut self) {
        if self.stories.len() != 1 {
            return;
        }
        self.end_para(EndKind::Soft);
        if self.story().tbl.row_open || !self.story().tbl.cells.is_empty() || !self.story().tbl.cell_blocks.is_empty() {
            self.end_row();
        }
        self.flush_table();
    }

    pub(super) fn finish_own(&mut self, own: Own, g: &Gs) {
        match own {
            Own::None => {}
            Own::Footnote => self.close_footnote(),
            Own::FldRslt => {}
            Own::FldRsltLink => {
                self.flush_run();
                self.close_link();
                self.story().link_active = None;
            }
            Own::Field => {
                self.fields.pop();
            }
            Own::Style => self.finish_style(g),
            Own::ListLevel => {
                let mut lv = std::mem::take(&mut self.level_acc);
                lv.font = g.ch.font;
                lv.li = g.pf.li;
                lv.fi = g.pf.fi;
                if self.list_acc.levels.len() < 9 {
                    self.list_acc.levels.push(lv);
                }
            }
            Own::ListDef => {
                let acc = std::mem::take(&mut self.list_acc);
                self.lists.insert(acc.id, acc.levels);
            }
            Own::ListOverride => {
                let mut o = std::mem::take(&mut self.ovr_acc);
                if self.ovr_ls > 0 {
                    o.starts.truncate(9);
                    self.overrides.insert(self.ovr_ls, o);
                }
            }
            Own::Lfo => {
                let s = self.lfo_start.take();
                if self.ovr_acc.starts.len() < 9 {
                    self.ovr_acc.starts.push(s);
                }
            }
            Own::Pict => self.finish_pict(),
            Own::PnText => self.pn_seen = true,
            Own::LevelText => {
                let t = std::mem::take(&mut self.level_text);
                self.level_acc.text = t;
            }
        }
    }

    // -----------------------------------------------------------------------
    // Tokens
    // -----------------------------------------------------------------------

    pub(super) fn sym(&mut self, c: char) {
        if self.skip > 0 {
            self.skip -= 1;
            return;
        }
        match c {
            '*' => {
                let g = self.g();
                if g.first {
                    g.star = true;
                }
            }
            '\\' | '{' | '}' => {
                self.g().first = false;
                self.emit_char(c);
            }
            '~' => {
                self.g().first = false;
                self.emit_char('\u{00A0}');
            }
            '-' => {
                self.g().first = false;
                self.emit_char('\u{00AD}');
            }
            '_' => {
                self.g().first = false;
                self.emit_char('\u{2011}');
            }
            _ => {}
        }
    }

    pub(super) fn hex(&mut self, b: u8) {
        if self.skip > 0 {
            self.skip -= 1;
            return;
        }
        self.g().first = false;
        let dest = self.g().dest;
        match dest {
            Dest::LevelText => self.level_text.push((b as u32, true)),
            Dest::Body | Dest::FldRslt | Dest::Footnote | Dest::FontTbl | Dest::StyleEntry | Dest::FldInst | Dest::PnText => {
                let cp = self.cur_cp();
                self.emit_char(decode_byte(cp, b));
            }
            _ => {}
        }
    }

    pub(super) fn text(&mut self, s: &str) {
        let mut s = s;
        while self.skip > 0 && !s.is_empty() {
            let n = s.chars().next().map(|c| c.len_utf8()).unwrap_or(1);
            s = &s[n..];
            self.skip -= 1;
        }
        if s.is_empty() {
            return;
        }
        self.g().first = false;
        self.emit_str(s);
    }

    pub(super) fn bin(&mut self, s: &str) {
        let dest = self.g().dest;
        if dest == Dest::Pict {
            for c in s.chars() {
                if self.pict.data.len() < MAX_PICT_BYTES && (c as u32) < 256 {
                    self.pict.data.push(c as u32 as u8);
                }
            }
        }
    }

    pub(super) fn cur_cp(&mut self) -> u16 {
        let dest = self.g().dest;
        let fid = if dest == Dest::FontTbl { self.font_cur } else { self.g().ch.font };
        let doc = self.ansicpg;
        match fid.and_then(|f| self.fonts.get(&f)) {
            Some(f) => match f.cpg {
                Some(c) if c > 0 => c.min(65535) as u16,
                _ => charset_cp(f.charset, doc),
            },
            None => doc,
        }
    }

    pub(super) fn emit_char(&mut self, c: char) {
        let mut buf = [0u8; 4];
        self.emit_str(c.encode_utf8(&mut buf));
    }

    pub(super) fn emit_str(&mut self, s: &str) {
        let dest = self.g().dest;
        match dest {
            Dest::Body | Dest::FldRslt | Dest::Footnote => self.body_text(s),
            Dest::FontTbl => {
                if let Some(f) = self.font_cur {
                    let font = self.fonts.entry(f).or_default();
                    match s.find(';') {
                        Some(i) => {
                            font.name.push_str(&s[..i]);
                            self.font_cur = None;
                        }
                        None => font.name.push_str(s),
                    }
                }
            }
            Dest::ColorTbl => {
                for c in s.chars() {
                    if c == ';' {
                        let acc = std::mem::take(&mut self.color_acc);
                        let entry = match acc {
                            (None, None, None) => None,
                            (r, g, b) => Some((r.unwrap_or(0), g.unwrap_or(0), b.unwrap_or(0))),
                        };
                        if self.colors.len() < 4096 {
                            self.colors.push(entry);
                        }
                    }
                }
            }
            Dest::StyleEntry => self.style_acc.name.push_str(s),
            Dest::FldInst => {
                if let Some(f) = self.fields.last_mut() {
                    if f.inst.len() < 8192 {
                        f.inst.push_str(s);
                    }
                }
            }
            Dest::Pict => self.pict_text(s),
            Dest::PnText => {
                if self.pn_buf.len() < 256 {
                    self.pn_buf.push_str(s);
                }
            }
            Dest::LevelText => {
                for c in s.chars() {
                    if c != ';' {
                        self.level_text.push((c as u32, false));
                    }
                }
            }
            _ => {}
        }
    }

    pub(super) fn pict_text(&mut self, s: &str) {
        for &b in s.as_bytes() {
            if let Some(v) = hex_val(b) {
                match self.pict.nib.take() {
                    Some(h) => {
                        if self.pict.data.len() < MAX_PICT_BYTES {
                            self.pict.data.push(h << 4 | v);
                        }
                    }
                    None => self.pict.nib = Some(v),
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    // Control words
    // -----------------------------------------------------------------------
}
