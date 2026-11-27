//! The lexer: RTF text into tokens.


pub(super) enum Tok<'a> {
    Open,
    Close,
    Word(&'a str, Option<i64>),
    Sym(char),
    Hex(u8),
    Text(&'a str),
    /// The bytes after `\binN`, as the characters that stand for them.
    Bin(&'a str),
}

pub(super) struct Lexer<'a> {
    pub(super) s: &'a str,
    pub(super) i: usize,
}

pub(super) fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

impl<'a> Lexer<'a> {
    pub(super) fn new(s: &'a str) -> Self {
        Lexer { s, i: 0 }
    }

    pub(super) fn next_tok(&mut self) -> Option<Tok<'a>> {
        let b = self.s.as_bytes();
        loop {
            if self.i >= b.len() {
                return None;
            }
            match b[self.i] {
                b'{' => {
                    self.i += 1;
                    return Some(Tok::Open);
                }
                b'}' => {
                    self.i += 1;
                    return Some(Tok::Close);
                }
                b'\r' | b'\n' => self.i += 1,
                b'\\' => return Some(self.control()),
                _ => {
                    let start = self.i;
                    while self.i < b.len() && !matches!(b[self.i], b'{' | b'}' | b'\\' | b'\r' | b'\n') {
                        self.i += 1;
                    }
                    return Some(Tok::Text(&self.s[start..self.i]));
                }
            }
        }
    }

    pub(super) fn control(&mut self) -> Tok<'a> {
        let b = self.s.as_bytes();
        let n = b.len();
        self.i += 1;
        if self.i >= n {
            return Tok::Text("");
        }
        let c = b[self.i];
        if c.is_ascii_alphabetic() {
            let st = self.i;
            while self.i < n && b[self.i].is_ascii_alphabetic() {
                self.i += 1;
            }
            let name = &self.s[st..self.i];
            let mut j = self.i;
            let neg = j < n && b[j] == b'-';
            if neg {
                j += 1;
            }
            let mut param = None;
            if j < n && b[j].is_ascii_digit() {
                let mut v: i64 = 0;
                while j < n && b[j].is_ascii_digit() {
                    v = (v * 10 + (b[j] - b'0') as i64).min(1 << 40);
                    j += 1;
                }
                param = Some(if neg { -v } else { v });
                self.i = j;
            }
            if self.i < n && b[self.i] == b' ' {
                self.i += 1;
            }
            if name == "bin" {
                if let Some(k) = param.filter(|k| *k > 0) {
                    let rest = &self.s[self.i..];
                    let end = rest.char_indices().nth(k as usize).map(|(i, _)| i).unwrap_or(rest.len());
                    self.i += end;
                    return Tok::Bin(&rest[..end]);
                }
            }
            return Tok::Word(name, param);
        }
        match c {
            b'\'' => {
                let h = b.get(self.i + 1).copied().and_then(hex_val);
                let l = b.get(self.i + 2).copied().and_then(hex_val);
                match (h, l) {
                    (Some(h), Some(l)) => {
                        self.i += 3;
                        Tok::Hex(h << 4 | l)
                    }
                    _ => {
                        self.i += 1;
                        Tok::Text("")
                    }
                }
            }
            b'\r' | b'\n' => {
                self.i += 1;
                if c == b'\r' && self.i < n && b[self.i] == b'\n' {
                    self.i += 1;
                }
                Tok::Word("par", None)
            }
            _ => {
                let ch = self.s[self.i..].chars().next().unwrap_or(' ');
                self.i += ch.len_utf8();
                Tok::Sym(ch)
            }
        }
    }
}
