//! Checking an XML part is well formed, and recovering what parses from one that isn't.
//!
//! Recovery keeps every tag and run of text up to a bad spot, drops the bad spot (up to the next `<`),
//! and carries on, so a part with one broken tag keeps the rest. A part cut short loses what is past the
//! cut; `xml::parse` closes whatever is still open.

use crate::xml;
use quick_xml::events::Event;
use quick_xml::Reader;

/// What recovery made of a part.
#[derive(Debug, Clone)]
pub struct Recovered {
    pub bytes: Vec<u8>,
    /// How many bad spots were dropped (a part cut short counts one).
    pub dropped: usize,
    /// The part ended in the middle of something.
    pub cut: bool,
}

/// The part's text: UTF-8 (with or without a byte order mark) or UTF-16.
pub fn decode(bytes: &[u8]) -> String {
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        return String::from_utf8_lossy(&bytes[3..]).into_owned();
    }
    if bytes.starts_with(&[0xFF, 0xFE]) || bytes.starts_with(&[0xFE, 0xFF]) {
        let le = bytes[0] == 0xFF;
        let units: Vec<u16> = bytes[2..].chunks_exact(2).map(|c| if le { u16::from_le_bytes([c[0], c[1]]) } else { u16::from_be_bytes([c[0], c[1]]) }).collect();
        return String::from_utf16_lossy(&units).replacen("encoding=\"UTF-16\"", "encoding=\"UTF-8\"", 1);
    }
    String::from_utf8_lossy(bytes).into_owned()
}

/// Whether the part is well formed: one root, every tag closed and matched.
pub fn well_formed(bytes: &[u8]) -> bool {
    let text = decode(bytes);
    let mut reader = Reader::from_str(&text);
    reader.config_mut().check_end_names = true;
    let mut depth = 0usize;
    let mut roots = 0usize;
    loop {
        match reader.read_event() {
            Ok(Event::Start(_)) => {
                if depth == 0 {
                    roots += 1;
                }
                depth += 1;
            }
            Ok(Event::Empty(_)) => {
                if depth == 0 {
                    roots += 1;
                }
            }
            Ok(Event::End(_)) => {
                if depth == 0 {
                    return false;
                }
                depth -= 1;
            }
            Ok(Event::Eof) => return depth == 0 && roots == 1,
            Ok(_) => {}
            Err(_) => return false,
        }
    }
}

/// `&` that doesn't start an entity becomes `&amp;`, and characters XML 1.0 doesn't allow are dropped.
fn tidy(text: &str) -> String {
    let b = text.as_bytes();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        if c == b'&' {
            // &name; or &#123; or &#x1F;
            let rest = &text[i + 1..];
            // An entity name is at most 12 characters: look 13 bytes ahead, no further (a scan to the end per `&` is quadratic).
            let ok = rest.as_bytes()[..rest.len().min(13)].iter().position(|c| *c == b';').is_some_and(|e| {
                let name = &rest[..e];
                e > 0 && e <= 12 && (name.strip_prefix('#').is_some_and(|n| !n.is_empty() && (n.chars().all(|c| c.is_ascii_digit()) || n.strip_prefix('x').is_some_and(|h| !h.is_empty() && h.chars().all(|c| c.is_ascii_hexdigit())))) || name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'))
            });
            out.push_str(if ok { "&" } else { "&amp;" });
            i += 1;
            continue;
        }
        let ch = text[i..].chars().next().unwrap();
        let bad = (ch as u32) < 0x20 && !matches!(ch, '\t' | '\n' | '\r');
        if !bad {
            out.push(ch);
        }
        i += ch.len_utf8();
    }
    out
}

fn valid_name(n: &str) -> bool {
    let mut it = n.chars();
    it.next().is_some_and(|c| c.is_alphabetic() || c == '_' || c == ':') && it.all(|c| c.is_alphanumeric() || matches!(c, '_' | ':' | '-' | '.'))
}

/// Recover what parses from a malformed part. `None` when nothing usable is left (no root element).
///
/// One pass over the tags, keeping the names of the open elements: a closing tag that matches an element
/// further up closes the ones in between, a closing tag nothing opened is dropped, and a tag that can't be
/// read is skipped up to the next `<`.
pub fn recover(bytes: &[u8]) -> Option<Recovered> {
    let text = tidy(&decode(bytes));
    let mut out = String::with_capacity(text.len());
    let mut rest: &str = &text;
    let mut stack: Vec<String> = Vec::new();
    let mut dropped = 0;
    let mut cut = false;
    'segments: loop {
        let mut reader = Reader::from_str(rest);
        reader.config_mut().check_end_names = false;
        let mut prev = 0usize;
        loop {
            let ev = reader.read_event();
            let pos = reader.buffer_position() as usize;
            // The reader only knows the tags it saw since the last restart, so it calls a closing tag
            // opened before that unmatched: that is for us to sort out.
            let unmatched = matches!(&ev, Err(quick_xml::Error::IllFormed(quick_xml::errors::IllFormedError::UnmatchedEndTag(_))));
            // A tag with a good name and attributes that don't parse keeps its name (the element is what holds the
            // text); its attributes are lost. A tag whose name is bad is skipped.
            let mut salvage: Option<(String, bool)> = None;
            let bad = match &ev {
                Err(_) => !unmatched,
                Ok(Event::Start(s)) | Ok(Event::Empty(s)) => {
                    let qname = s.name();
                    let name = qname.as_ref();
                    if !valid_name(name) {
                        true
                    } else {
                        if s.attributes().with_checks(false).any(|a| a.is_err()) {
                            salvage = Some((name.to_string(), matches!(ev, Ok(Event::Empty(_)))));
                        }
                        false
                    }
                }
                Ok(Event::End(e)) => !valid_name(e.name().as_ref()),
                _ => false,
            };
            if bad {
                dropped += 1;
                match rest.get(prev + 1..).and_then(|r| r.find('<')) {
                    Some(i) => {
                        rest = &rest[prev + 1 + i..];
                        continue 'segments;
                    }
                    None => {
                        cut = true;
                        break 'segments;
                    }
                }
            }
            let slice = &rest[prev..pos.min(rest.len())];
            let end_name = match &ev {
                Ok(Event::End(e)) => Some(e.name().as_ref().to_string()),
                Err(_) => Some(slice.trim_start_matches("</").trim_end_matches('>').trim().to_string()),
                _ => None,
            };
            match ev {
                Ok(Event::Eof) => break 'segments,
                Ok(Event::Start(s)) => {
                    stack.push(s.name().as_ref().to_string());
                    match &salvage {
                        Some((n, _)) => {
                            out.push_str(&format!("<{n}>"));
                            dropped += 1;
                        }
                        None => out.push_str(slice),
                    }
                }
                Ok(Event::Empty(_)) if salvage.is_some() => {
                    out.push_str(&format!("<{}/>", salvage.as_ref().map(|s| s.0.as_str()).unwrap_or("")));
                    dropped += 1;
                }
                Ok(Event::End(_)) | Err(_) => {
                    let name = end_name.unwrap_or_default();
                    if let Some(at) = stack.iter().rposition(|n| *n == name) {
                        while stack.len() > at + 1 {
                            let open = stack.pop().unwrap();
                            out.push_str(&format!("</{open}>"));
                            dropped += 1;
                        }
                        stack.pop();
                        out.push_str(slice);
                    } else {
                        // A closing tag nothing opened.
                        dropped += 1;
                    }
                }
                Ok(_) => out.push_str(slice),
            }
            prev = pos;
        }
    }
    // Elements still open at the end mean the part stopped early.
    cut |= !stack.is_empty();
    while let Some(open) = stack.pop() {
        out.push_str(&format!("</{open}>"));
    }
    let doc = xml::parse(out.as_bytes()).ok()?;
    // The recovered part is written from the parsed tree, so it is well formed and closed.
    Some(Recovered { bytes: doc.to_bytes(), dropped, cut })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_part_of_bare_ampersands_is_tidied_in_linear_time() {
        // Every `&` used to search the rest of the part for a `;`.
        let part = "&".repeat(1_000_000);
        let t = std::time::Instant::now();
        let tidied = tidy(&part);
        assert!(t.elapsed().as_secs() < 5, "took {:?}", t.elapsed());
        assert_eq!(tidied.len(), 5 * part.len());
        let r = recover(format!("<a>{}</a>", "&".repeat(1000)).as_bytes()).unwrap();
        assert!(String::from_utf8(r.bytes).unwrap().starts_with("<a>&amp;&amp;"));
        // Real entities still stand.
        assert_eq!(tidy("a &amp; b &#x41; &#66; &nbsp; & c"), "a &amp; b &#x41; &#66; &nbsp; &amp; c");
    }

    #[test]
    fn well_formed_checks() {
        assert!(well_formed(b"<a><b/>text</a>"));
        assert!(!well_formed(b"<a><b>text</a>"));
        assert!(!well_formed(b"<a><b/>text"));
        assert!(!well_formed(b"<a/><b/>"));
        assert!(!well_formed(b""));
    }

    #[test]
    fn a_cut_part_keeps_its_start() {
        let r = recover(b"<a><p>one</p><p>two</p><p>thr").unwrap();
        let s = String::from_utf8(r.bytes).unwrap();
        assert!(s.contains("one") && s.contains("two"), "{s}");
        assert!(well_formed(s.as_bytes()));
        assert!(recover(b"<a><p>one</p><p>two</p><p>thr").unwrap().cut);
        // Damage in the middle, with the part complete at the end, is not a cut.
        assert!(!recover(b"<a><p>one</p></x><p>two</p></a>").unwrap().cut);
    }

    #[test]
    fn a_cut_inside_a_tag_drops_the_tag() {
        let r = recover(b"<a><p>one</p><p>two</p><w:r w:ri").unwrap();
        assert!(r.cut);
        let s = String::from_utf8(r.bytes).unwrap();
        assert!(s.contains("two") && !s.contains("w:ri"), "{s}");
    }

    #[test]
    fn a_broken_tag_in_the_middle_keeps_both_sides() {
        let r = recover(b"<a><p>one</p><p <<>two</p><p>three</p></a>").unwrap();
        assert!(r.dropped >= 1);
        let s = String::from_utf8(r.bytes).unwrap();
        assert!(s.contains("one") && s.contains("three"), "{s}");
    }

    #[test]
    fn a_broken_run_tag_loses_only_its_own_opening() {
        let src = r#"<w:document><w:body><w:p><w:r><w:t>one</w:t></w:r></w:p><w:p><w:r <<><w:t xml:space="preserve">two</w:t></w:r></w:p><w:p><w:r><w:t>three</w:t></w:r></w:p></w:body></w:document>"#;
        let r = recover(src.as_bytes()).unwrap();
        let s = String::from_utf8(r.bytes).unwrap();
        assert!(s.contains("one") && s.contains("two") && s.contains("three"), "{s}");
        assert!(well_formed(s.as_bytes()), "{s}");
        // The run keeps its element, so the text stays in a run.
        assert!(s.contains(r#"<w:r><w:t xml:space="preserve">two</w:t></w:r>"#), "{s}");
    }

    #[test]
    fn stray_ampersands_and_control_characters_are_fixed() {
        let r = recover(b"<a><p>fish & chips \x01 &amp; more &nbsp;</p></a>").unwrap();
        let s = String::from_utf8(r.bytes).unwrap();
        assert!(s.contains("fish &amp; chips") && s.contains("more"), "{s}");
    }

    #[test]
    fn no_root_is_nothing() {
        assert!(recover(b"just words").is_none());
        assert!(recover(b"").is_none());
    }
}
