//! MHT ("web page, single file") and other MIME messages: the parts of a message, decoded, and the web page at
//! its root with the pictures it refers to. Word and many mail systems write chunks this way.
//!
//! Only what a document needs is read: multipart bodies (nested), `quoted-printable`, `base64`, 7bit and 8bit
//! bodies, folded header lines, and the `Content-ID` and `Content-Location` names pictures are found by.

use std::collections::HashMap;

/// One leaf of a message.
#[derive(Debug, Clone, Default)]
pub struct Leaf {
    /// `text/html`, `image/png`, ... in lower case.
    pub content_type: String,
    pub charset: Option<String>,
    /// `Content-ID` without the angle brackets.
    pub cid: Option<String>,
    pub location: Option<String>,
    pub data: Vec<u8>,
}

/// Whether the bytes start like a MIME message (a header block with a version or a content type).
pub fn looks_like_mime(data: &[u8]) -> bool {
    let head = String::from_utf8_lossy(&data[..data.len().min(2000)]).to_ascii_lowercase();
    let first = head.trim_start();
    first.starts_with("mime-version:")
        || first.starts_with("content-type:")
        || ((first.starts_with("from:") || first.starts_with("subject:") || first.starts_with("date:") || first.starts_with("x-")) && head.contains("mime-version:"))
}

/// All the leaves of a message, in order.
pub fn leaves(data: &[u8]) -> Vec<Leaf> {
    let mut out = Vec::new();
    walk(data, &mut out, 0);
    out
}

fn walk(data: &[u8], out: &mut Vec<Leaf>, depth: usize) {
    if depth > 8 {
        return;
    }
    let (headers, body) = split_head(data);
    let h = parse_headers(headers);
    let ct_full = h.get("content-type").cloned().unwrap_or_else(|| "text/plain".into());
    let (ct, params) = split_params(&ct_full);
    if ct.starts_with("multipart/") {
        let Some(boundary) = params.get("boundary") else { return };
        for part in split_multipart(body, boundary) {
            walk(part, out, depth + 1);
        }
        return;
    }
    let enc = h.get("content-transfer-encoding").map(|s| s.trim().to_ascii_lowercase()).unwrap_or_default();
    let data = match enc.as_str() {
        "base64" => base64_decode(body),
        "quoted-printable" => qp_decode(body),
        _ => body.to_vec(),
    };
    out.push(Leaf {
        content_type: ct,
        charset: params.get("charset").cloned(),
        cid: h.get("content-id").map(|v| v.trim().trim_start_matches('<').trim_end_matches('>').to_string()).filter(|v| !v.is_empty()),
        location: h.get("content-location").map(|v| v.trim().to_string()).filter(|v| !v.is_empty()),
        data,
    });
}

/// The header block and the body, split at the first empty line.
fn split_head(data: &[u8]) -> (&[u8], &[u8]) {
    let mut i = 0;
    while i < data.len() {
        // A line break followed by another one ends the headers.
        if data[i] == b'\n' {
            if data.get(i + 1) == Some(&b'\n') {
                return (&data[..i], &data[i + 2..]);
            }
            if data.get(i + 1) == Some(&b'\r') && data.get(i + 2) == Some(&b'\n') {
                return (&data[..i], &data[i + 3..]);
            }
        }
        i += 1;
    }
    (data, &[])
}

fn parse_headers(block: &[u8]) -> HashMap<String, String> {
    let text = String::from_utf8_lossy(block);
    let mut out: HashMap<String, String> = HashMap::new();
    let mut last: Option<String> = None;
    for line in text.lines() {
        if line.starts_with(' ') || line.starts_with('\t') {
            if let Some(k) = &last {
                if let Some(v) = out.get_mut(k) {
                    v.push(' ');
                    v.push_str(line.trim());
                }
            }
            continue;
        }
        let Some((k, v)) = line.split_once(':') else { continue };
        let k = k.trim().to_ascii_lowercase();
        out.entry(k.clone()).or_insert_with(|| v.trim().to_string());
        last = Some(k);
    }
    out
}

/// `text/html; charset="utf-8"; boundary=x` into the lower-case type and its parameters.
fn split_params(value: &str) -> (String, HashMap<String, String>) {
    let mut params = HashMap::new();
    let mut parts = split_semicolons(value).into_iter();
    let ct = parts.next().unwrap_or_default().trim().to_ascii_lowercase();
    for p in parts {
        if let Some((k, v)) = p.split_once('=') {
            let v = v.trim().trim_matches('"').to_string();
            params.insert(k.trim().to_ascii_lowercase(), v);
        }
    }
    (ct, params)
}

/// Split at semicolons that are not inside quotes.
fn split_semicolons(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quoted = false;
    for c in s.chars() {
        match c {
            '"' => {
                quoted = !quoted;
                cur.push(c);
            }
            ';' if !quoted => out.push(std::mem::take(&mut cur)),
            _ => cur.push(c),
        }
    }
    out.push(cur);
    out
}

/// The parts between the boundary lines.
fn split_multipart<'a>(body: &'a [u8], boundary: &str) -> Vec<&'a [u8]> {
    let delim = format!("--{boundary}");
    let delim = delim.as_bytes();
    // Offsets of every boundary line (at the start of a line).
    let mut marks: Vec<(usize, usize, bool)> = Vec::new();
    let mut i = 0;
    while i + delim.len() <= body.len() {
        let at_line_start = i == 0 || body[i - 1] == b'\n';
        if at_line_start && &body[i..i + delim.len()] == delim {
            let after = i + delim.len();
            let closing = body.get(after..after + 2) == Some(b"--");
            // The line ends at the next line break.
            let mut end = after;
            while end < body.len() && body[end] != b'\n' {
                end += 1;
            }
            marks.push((i, (end + 1).min(body.len()), closing));
            i = end + 1;
            continue;
        }
        i += 1;
    }
    let mut parts = Vec::new();
    for w in marks.windows(2) {
        let (_, start, closing_a) = w[0];
        if closing_a {
            break;
        }
        let mut end = w[1].0;
        // The line break before a boundary belongs to the boundary.
        if end > start && body[end - 1] == b'\n' {
            end -= 1;
            if end > start && body[end - 1] == b'\r' {
                end -= 1;
            }
        }
        parts.push(&body[start.min(end)..end]);
    }
    parts
}

fn base64_decode(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len() * 3 / 4);
    let mut acc = 0u32;
    let mut bits = 0;
    for &c in data {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            b'=' => break,
            _ => continue,
        };
        acc = (acc << 6) | v as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    out
}

fn qp_decode(data: &[u8]) -> Vec<u8> {
    let hex = |c: u8| (c as char).to_digit(16).map(|d| d as u8);
    let mut out = Vec::with_capacity(data.len());
    let mut i = 0;
    while i < data.len() {
        let c = data[i];
        if c == b'=' {
            // A soft line break.
            if data.get(i + 1) == Some(&b'\n') {
                i += 2;
                continue;
            }
            if data.get(i + 1) == Some(&b'\r') && data.get(i + 2) == Some(&b'\n') {
                i += 3;
                continue;
            }
            if let (Some(h), Some(l)) = (data.get(i + 1).and_then(|c| hex(*c)), data.get(i + 2).and_then(|c| hex(*c))) {
                out.push(h * 16 + l);
                i += 3;
                continue;
            }
        }
        out.push(c);
        i += 1;
    }
    out
}

/// The text of a body in the charset the message names: UTF-8, UTF-16, or one of the Western single-byte sets.
pub fn decode_text(data: &[u8], charset: Option<&str>) -> String {
    if data.starts_with(&[0xFF, 0xFE]) || data.starts_with(&[0xFE, 0xFF]) {
        return crate::open::xmlfix::decode(data);
    }
    let data = data.strip_prefix(&[0xEF, 0xBB, 0xBF][..]).unwrap_or(data);
    let cs = charset.map(|c| c.trim().to_ascii_lowercase()).unwrap_or_default();
    match cs.as_str() {
        "utf-16" | "utf-16le" => {
            let units: Vec<u16> = data.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
            String::from_utf16_lossy(&units)
        }
        "utf-16be" => {
            let units: Vec<u16> = data.chunks_exact(2).map(|c| u16::from_be_bytes([c[0], c[1]])).collect();
            String::from_utf16_lossy(&units)
        }
        "iso-8859-1" | "latin1" => data.iter().map(|&b| b as char).collect(),
        "windows-1252" | "cp1252" | "us-ascii" | "ascii" => data.iter().map(|&b| cp1252(b)).collect(),
        _ => match std::str::from_utf8(data) {
            Ok(s) => s.to_string(),
            Err(_) => data.iter().map(|&b| cp1252(b)).collect(),
        },
    }
}

const CP1252_HI: [u16; 32] = [
    0x20AC, 0x81, 0x201A, 0x0192, 0x201E, 0x2026, 0x2020, 0x2021, 0x02C6, 0x2030, 0x0160, 0x2039, 0x0152, 0x8D, 0x017D, 0x8F, 0x90, 0x2018, 0x2019, 0x201C, 0x201D, 0x2022, 0x2013, 0x2014, 0x02DC, 0x2122, 0x0161,
    0x203A, 0x0153, 0x9D, 0x017E, 0x0178,
];

pub fn cp1252(b: u8) -> char {
    match b {
        0x80..=0x9F => char::from_u32(CP1252_HI[(b - 0x80) as usize] as u32).unwrap_or('?'),
        _ => b as char,
    }
}

/// The charset a web page names in a `<meta>` tag, if it does.
pub fn meta_charset(html: &[u8]) -> Option<String> {
    let head = String::from_utf8_lossy(&html[..html.len().min(4096)]).to_ascii_lowercase();
    let at = head.find("charset=")? + 8;
    let rest = head[at..].trim_start_matches(['"', '\'']);
    let end = rest.find(|c: char| !(c.is_ascii_alphanumeric() || c == '-' || c == '_')).unwrap_or(rest.len());
    let v = &rest[..end];
    (!v.is_empty()).then(|| v.to_string())
}

/// The web page of a message and the pictures it refers to, by every name the page may use for them: `cid:` plus the
/// content id, the location as written, and the last part of the location.
pub struct Page {
    pub html: String,
    pub resources: HashMap<String, Vec<u8>>,
}

pub fn page(data: &[u8]) -> Option<Page> {
    let leaves = leaves(data);
    let root = leaves.iter().find(|l| l.content_type == "text/html" || l.content_type == "application/xhtml+xml").or_else(|| leaves.iter().find(|l| l.content_type == "text/plain"))?;
    let charset = root.charset.clone().or_else(|| meta_charset(&root.data));
    let html = decode_text(&root.data, charset.as_deref());
    let mut resources = HashMap::new();
    for l in &leaves {
        if std::ptr::eq(l, root) || l.content_type.starts_with("text/") {
            continue;
        }
        if let Some(cid) = &l.cid {
            resources.insert(format!("cid:{cid}"), l.data.clone());
        }
        if let Some(loc) = &l.location {
            resources.insert(loc.clone(), l.data.clone());
            resources.insert(basename(loc), l.data.clone());
        }
    }
    Some(Page { html, resources })
}

/// The last part of a path or address, without a query, percent-decoded.
pub fn basename(s: &str) -> String {
    let s = s.split(['?', '#']).next().unwrap_or(s);
    let last = s.rsplit(['/', '\\']).next().unwrap_or(s);
    percent_decode(last)
}

pub fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let hex = |c: u8| (c as char).to_digit(16).map(|d| d as u8);
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let (Some(h), Some(l)) = (hex(b[i + 1]), hex(b[i + 2])) {
                out.push(h * 16 + l);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8(out).unwrap_or_else(|_| s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const MHT: &str = "MIME-Version: 1.0\r\nContent-Type: multipart/related; boundary=\"----=_Part_1\"; type=\"text/html\"\r\n\r\n------=_Part_1\r\nContent-Type: text/html; charset=\"utf-8\"\r\nContent-Transfer-Encoding: quoted-printable\r\nContent-Location: file:///C:/x/page.htm\r\n\r\n<html><body><p>Caf=C3=A9 <img src=3D\"pic.png\"> and =\r\nmore</p></body></html>\r\n------=_Part_1\r\nContent-Type: image/png\r\nContent-Transfer-Encoding: base64\r\nContent-Location: file:///C:/x/pic.png\r\nContent-ID: <pic1@x>\r\n\r\niVBORw0KGgo=\r\n------=_Part_1--\r\n";

    #[test]
    fn reads_a_web_page_archive() {
        assert!(looks_like_mime(MHT.as_bytes()));
        let p = page(MHT.as_bytes()).unwrap();
        assert!(p.html.contains("Café <img src=\"pic.png\"> and more"), "{}", p.html);
        assert_eq!(p.resources.get("pic.png").map(|d| d.len()), Some(8));
        assert!(p.resources.contains_key("cid:pic1@x"));
        assert!(p.resources.contains_key("file:///C:/x/pic.png"));
    }

    #[test]
    fn nested_parts_and_bare_messages() {
        let nested = "Content-Type: multipart/mixed; boundary=outer\n\n--outer\nContent-Type: multipart/alternative; boundary=inner\n\n--inner\nContent-Type: text/plain\n\nplain\n--inner\nContent-Type: text/html\n\n<p>rich</p>\n--inner--\n--outer--\n";
        let l = leaves(nested.as_bytes());
        assert_eq!(l.len(), 2);
        assert_eq!(page(nested.as_bytes()).unwrap().html.trim(), "<p>rich</p>");
        let bare = "Content-Type: text/html\n\n<b>x</b>";
        assert_eq!(page(bare.as_bytes()).unwrap().html, "<b>x</b>");
    }

    #[test]
    fn charsets() {
        assert_eq!(decode_text(&[0x63, 0x61, 0x66, 0xE9], Some("iso-8859-1")), "café");
        assert_eq!(decode_text(&[0x93, 0x68, 0x94], Some("windows-1252")), "\u{201C}h\u{201D}");
        assert_eq!(meta_charset(b"<meta http-equiv=\"Content-Type\" content=\"text/html; charset=windows-1252\">").as_deref(), Some("windows-1252"));
        assert_eq!(basename("file:///C:/a%20b/pic.png?x=1"), "pic.png");
    }
}
