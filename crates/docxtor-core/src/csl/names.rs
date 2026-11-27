//! Name lists: initials, inversion, "and", "et al." and the delimiters between them.

use super::locale::Locale;
use super::model::*;
use super::text;
use crate::cite_styles::Rich;
use crate::citations::Name;

/// The initials of a given name, each followed by `with`: "Jane Q." with ". " is "J. Q.", with "" it is "JQ".
fn initials(given: &str, with: &str, hyphen: bool) -> String {
    let mut out = String::new();
    for tok in given.split_whitespace() {
        let parts: Vec<&str> = tok.split('-').filter(|p| !p.is_empty()).collect();
        let mut seg = String::new();
        for (i, p) in parts.iter().enumerate() {
            let letters: Vec<char> = p.chars().filter(|c| c.is_alphabetic()).collect();
            if letters.is_empty() {
                continue;
            }
            // "J.K." and "JK" are two initials; any other word gives its first letter.
            let dotted = p.contains('.') && p.split('.').filter(|x| !x.is_empty()).all(|x| x.chars().count() == 1);
            let capitals = letters.len() >= 2 && letters.len() <= 3 && letters.iter().all(|c| c.is_uppercase());
            let each: Vec<char> = if dotted || capitals { letters.clone() } else { vec![letters[0]] };
            for c in each {
                seg.extend(c.to_uppercase());
                seg.push_str(with);
            }
            if hyphen && i + 1 < parts.len() {
                seg = seg.trim_end().to_string();
                seg.push('-');
            }
        }
        out.push_str(&seg);
    }
    out.trim_end().to_string()
}

/// Given names written in full, except that a lone initial is written with `with` after it.
fn keep_given(given: &str, with: &str) -> String {
    given
        .split_whitespace()
        .map(|tok| {
            let bare = tok.trim_end_matches('.');
            if bare.chars().count() == 1 && bare.chars().all(|c| c.is_uppercase()) {
                format!("{bare}{}", with.trim_end())
            } else {
                tok.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
        .trim_end()
        .to_string()
}

fn styled(text: &str, fmt: Option<&Fmt>, loc: &Locale) -> Rich {
    let mut r = Rich::plain_of(text);
    if let Some(f) = fmt {
        r = super::eval::decorate(r, f, loc);
    }
    r
}

/// One name as it reads, in the order and form the options ask for.
fn one(n: &Name, inverted: bool, o: &NameOpts, node: Option<&NameNode>, loc: &Locale) -> Rich {
    let (family, given) = match n {
        Name::Literal(l) => return styled(l, node.and_then(|n| n.family.as_ref()), loc),
        Name::Person { family, given } => (family.as_str(), given.as_deref().unwrap_or("")),
    };
    let family_r = styled(family, node.and_then(|n| n.family.as_ref()), loc);
    if o.form.as_deref() == Some("short") {
        return family_r;
    }
    let given_text = match o.initialize_with.as_deref() {
        Some(with) if o.initialize != Some(false) => initials(given, with, o.initialize_with_hyphen != Some(false)),
        Some(with) => keep_given(given, with),
        None => given.split_whitespace().collect::<Vec<_>>().join(" "),
    };
    if given_text.is_empty() {
        return family_r;
    }
    let given_r = styled(&given_text, node.and_then(|n| n.given.as_ref()), loc);
    let mut out = Rich::new();
    if inverted {
        out.append(&family_r);
        out.t(o.sort_separator.as_deref().unwrap_or(", "));
        out.append(&given_r);
    } else {
        out.append(&given_r);
        out.t(" ");
        out.append(&family_r);
    }
    out
}

/// A list of names. `subsequent` picks the et-al limits meant for later citations of the same item.
pub fn list(names: &[&Name], o: &NameOpts, node: Option<&NameNode>, et_al: Option<&(Fmt, String)>, loc: &Locale, subsequent: bool) -> Rich {
    let n = names.len();
    if n == 0 {
        return Rich::new();
    }
    let (min, first) = if subsequent {
        (o.et_al_subsequent_min.or(o.et_al_min), o.et_al_subsequent_use_first.or(o.et_al_use_first))
    } else {
        (o.et_al_min, o.et_al_use_first)
    };
    let mut shown: &[&Name] = names;
    let mut truncated = false;
    if let (Some(min), Some(first)) = (min, first) {
        if n >= min && first < n {
            shown = &names[..first.max(1)];
            truncated = true;
        }
    }
    if o.form.as_deref() == Some("count") {
        return Rich::plain_of(&shown.len().to_string());
    }
    let delim = o.delimiter.as_deref().unwrap_or(", ");
    let inverted = |i: usize| match o.name_as_sort_order {
        Some(SortOrder::All) => true,
        Some(SortOrder::First) => i == 0,
        None => false,
    };
    let pieces: Vec<Rich> = shown.iter().enumerate().map(|(i, nm)| one(nm, inverted(i), o, node, loc)).collect();
    let mut out = Rich::new();
    let precedes = |setting: Option<Precedes>, count: usize, prev_inverted: bool| -> bool {
        match setting.unwrap_or(Precedes::Contextual) {
            Precedes::Always => true,
            Precedes::Never => false,
            Precedes::AfterInverted => prev_inverted,
            Precedes::Contextual => count >= 3,
        }
    };
    if truncated {
        out.append(&text::join(pieces, delim));
        if o.et_al_use_last == Some(true) && n > shown.len() + 1 {
            let last = one(names[n - 1], inverted(n - 1), o, node, loc);
            out.t(delim);
            out.t("\u{2026} ");
            out.append(&last);
        } else {
            let (fmt, term) = et_al.map(|(f, t)| (Some(f), t.as_str())).unwrap_or((None, "et-al"));
            let t = loc.term(term, Form::Long, false).unwrap_or_else(|| "et al.".into());
            let mut etal = Rich::plain_of(&t);
            if let Some(f) = fmt {
                etal = super::eval::decorate(etal, f, loc);
            }
            let with_delim = match o.delimiter_precedes_et_al.unwrap_or(Precedes::Contextual) {
                Precedes::Always => true,
                Precedes::Never => false,
                Precedes::AfterInverted => inverted(shown.len() - 1),
                Precedes::Contextual => shown.len() >= 2,
            };
            text::push_chunk(&mut out, if with_delim { delim } else { " " });
            out.append(&etal);
        }
    } else if shown.len() >= 2 && o.and.is_some() {
        let k = shown.len();
        let and = match o.and {
            Some(And::Symbol) => loc.term("and", Form::Symbol, false).unwrap_or_else(|| "&".into()),
            _ => loc.term("and", Form::Long, false).unwrap_or_else(|| "and".into()),
        };
        let mut it = pieces.into_iter();
        let head: Vec<Rich> = it.by_ref().take(k - 1).collect();
        let last = it.next().unwrap_or_default();
        out.append(&text::join(head, delim));
        let sep = if precedes(o.delimiter_precedes_last, k, inverted(k - 2)) { delim.trim_end().to_string() + " " } else { " ".to_string() };
        text::push_chunk(&mut out, &sep);
        out.t(&and);
        out.t(" ");
        out.append(&last);
    } else {
        out.append(&text::join(pieces, delim));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initials_forms() {
        assert_eq!(initials("Jane Q.", ". ", true), "J. Q.");
        assert_eq!(initials("Jane Q.", "", true), "JQ");
        assert_eq!(initials("Jane Q.", ".", true), "J.Q.");
        assert_eq!(initials("Jean-Pierre", ". ", true), "J.-P.");
        assert_eq!(initials("J.K.", ". ", true), "J. K.");
        assert_eq!(keep_given("J. K. Rowling", ". "), "J. K. Rowling");
    }
}
