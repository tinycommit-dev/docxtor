//! A small tag scanner that reads tags with their byte positions so edits can splice the text.


#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Kind {
    Open,
    Close,
    Empty,
}

#[derive(Debug)]
pub(super) struct Tag<'a> {
    pub(super) start: usize,
    pub(super) end: usize,
    pub(super) name: &'a str,
    pub(super) attrs: Vec<(&'a str, &'a str)>,
    pub(super) kind: Kind,
}

impl<'a> Tag<'a> {
    pub(super) fn local(&self) -> &'a str {
        local(self.name)
    }
    pub(super) fn attr(&self, name: &str) -> Option<&'a str> {
        self.attrs.iter().find(|(k, _)| *k == name).map(|(_, v)| *v)
    }
    pub(super) fn prefix(&self) -> &'a str {
        match self.name.rfind(':') {
            Some(i) => &self.name[..=i],
            None => "",
        }
    }
}

pub(super) fn local(name: &str) -> &str {
    name.rsplit(':').next().unwrap_or(name)
}

pub(super) fn is_ws(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\r' | b'\n')
}

/// Byte offset of `part` (a slice of `s`) in `s`.
pub(super) fn off(s: &str, part: &str) -> usize {
    part.as_ptr() as usize - s.as_ptr() as usize
}

/// The next tag at or after `from`, skipping text, comments and processing instructions. `Some(None)` at the
/// end of the text; `None` when the text can't be scanned.
pub(super) fn next_tag(s: &str, from: usize) -> Option<Option<Tag<'_>>> {
    let b = s.as_bytes();
    let mut i = from;
    loop {
        let lt = match s.get(i..)?.find('<') {
            Some(p) => i + p,
            None => return Some(None),
        };
        let rest = &s[lt..];
        if let Some(after) = rest.strip_prefix("<!--") {
            i = lt + 4 + after.find("-->")? + 3;
            continue;
        }
        if rest.starts_with("<![CDATA[") {
            i = lt + rest.find("]]>")? + 3;
            continue;
        }
        if rest.starts_with("<?") {
            i = lt + rest.find("?>")? + 2;
            continue;
        }
        if rest.starts_with("<!") {
            return None;
        }
        if rest.starts_with("</") {
            let e = rest.find('>')?;
            let name = rest[2..e].trim_end();
            if name.is_empty() {
                return None;
            }
            return Some(Some(Tag { start: lt, end: lt + e + 1, name, attrs: Vec::new(), kind: Kind::Close }));
        }
        let mut j = lt + 1;
        while j < b.len() && !is_ws(b[j]) && b[j] != b'>' && b[j] != b'/' {
            j += 1;
        }
        let name = &s[lt + 1..j];
        if name.is_empty() {
            return None;
        }
        let mut attrs = Vec::new();
        loop {
            while j < b.len() && is_ws(b[j]) {
                j += 1;
            }
            match *b.get(j)? {
                b'>' => return Some(Some(Tag { start: lt, end: j + 1, name, attrs, kind: Kind::Open })),
                b'/' => {
                    if *b.get(j + 1)? != b'>' {
                        return None;
                    }
                    return Some(Some(Tag { start: lt, end: j + 2, name, attrs, kind: Kind::Empty }));
                }
                _ => {
                    let ks = j;
                    while j < b.len() && !is_ws(b[j]) && b[j] != b'=' && b[j] != b'>' && b[j] != b'/' {
                        j += 1;
                    }
                    let key = &s[ks..j];
                    if key.is_empty() {
                        return None;
                    }
                    while j < b.len() && is_ws(b[j]) {
                        j += 1;
                    }
                    if *b.get(j)? != b'=' {
                        return None;
                    }
                    j += 1;
                    while j < b.len() && is_ws(b[j]) {
                        j += 1;
                    }
                    let q = *b.get(j)?;
                    if q != b'"' && q != b'\'' {
                        return None;
                    }
                    let vs = j + 1;
                    let ve = vs + s[vs..].find(q as char)?;
                    if s[vs..ve].contains('<') {
                        return None;
                    }
                    attrs.push((key, &s[vs..ve]));
                    j = ve + 1;
                }
            }
        }
    }
}

/// Skip to the end of the element whose start tag was just read.
pub(super) fn skip_subtree(s: &str, pos: &mut usize) -> Option<()> {
    let mut depth = 1usize;
    loop {
        let t = next_tag(s, *pos)??;
        *pos = t.end;
        match t.kind {
            Kind::Open => depth += 1,
            Kind::Close => {
                depth -= 1;
                if depth == 0 {
                    return Some(());
                }
            }
            Kind::Empty => {}
        }
    }
}

pub(super) fn unescape(s: &str) -> Option<String> {
    if !s.contains('&') {
        return Some(s.to_string());
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(p) = rest.find('&') {
        out.push_str(&rest[..p]);
        let after = &rest[p + 1..];
        let semi = after.find(';')?;
        let ent = &after[..semi];
        match ent {
            "amp" => out.push('&'),
            "lt" => out.push('<'),
            "gt" => out.push('>'),
            "quot" => out.push('"'),
            "apos" => out.push('\''),
            _ if ent.starts_with("#x") || ent.starts_with("#X") => out.push(char::from_u32(u32::from_str_radix(&ent[2..], 16).ok()?)?),
            _ if ent.starts_with('#') => out.push(char::from_u32(ent[1..].parse().ok()?)?),
            _ => return None,
        }
        rest = &after[semi + 1..];
    }
    out.push_str(rest);
    Some(out)
}

pub(super) fn esc_text(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '\t' | '\n' | '\r' => o.push(c),
            c if (c as u32) < 0x20 || c == '\u{FFFE}' || c == '\u{FFFF}' => {}
            c => o.push(c),
        }
    }
    o
}

pub(super) fn esc_attr(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            '\t' => o.push_str("&#9;"),
            '\n' => o.push_str("&#10;"),
            '\r' => o.push_str("&#13;"),
            c if (c as u32) < 0x20 || c == '\u{FFFE}' || c == '\u{FFFF}' => {}
            c => o.push(c),
        }
    }
    o
}

/// Apply `(start, end, replacement)` edits to `text`. Inserts (`start == end`) at one position keep their
/// order and come before a replacement starting there. `None` if two edits overlap.
pub(super) fn splice(text: &str, mut edits: Vec<(usize, usize, String)>) -> Option<String> {
    edits.sort_by_key(|e| (e.0, e.1));
    let mut out = String::with_capacity(text.len() + edits.iter().map(|e| e.2.len()).sum::<usize>());
    let mut last = 0;
    for (s, e, r) in edits {
        if s < last || e < s || e > text.len() {
            return None;
        }
        out.push_str(text.get(last..s)?);
        out.push_str(&r);
        last = e;
    }
    out.push_str(text.get(last..)?);
    Some(out)
}
