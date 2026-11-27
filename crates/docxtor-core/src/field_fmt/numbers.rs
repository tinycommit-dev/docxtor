//! Numbers: `\#` pictures, rounding and parsing.


/// The decimal digits of a non-negative finite number, rounded to 15 significant digits: the integer digits
/// (no leading zeros, empty for 0) and the fraction digits (no trailing zeros).
pub(super) fn decimal_parts(abs: f64) -> (Vec<u8>, Vec<u8>) {
    if abs == 0.0 || !abs.is_finite() {
        return (Vec::new(), Vec::new());
    }
    let s = format!("{abs:.14e}");
    let (mant, exp) = s.split_once('e').unwrap_or((&s, "0"));
    let exp: i32 = exp.parse().unwrap_or(0);
    let digits: Vec<u8> = mant.bytes().filter(u8::is_ascii_digit).map(|b| b - b'0').collect();
    let point = exp + 1;
    let (int, mut frac) = if point <= 0 {
        let mut f = vec![0; (-point) as usize];
        f.extend_from_slice(&digits);
        (Vec::new(), f)
    } else if point as usize >= digits.len() {
        let mut i = digits.clone();
        i.resize(point as usize, 0);
        (i, Vec::new())
    } else {
        (digits[..point as usize].to_vec(), digits[point as usize..].to_vec())
    };
    while frac.last() == Some(&0) {
        frac.pop();
    }
    (int, frac)
}

/// Rounds half up (away from zero for the absolute value) to `n` fraction digits. The fraction comes back
/// padded with zeros to exactly `n` digits.
pub(super) fn round_parts(mut int: Vec<u8>, frac: Vec<u8>, n: usize) -> (Vec<u8>, Vec<u8>) {
    let up = frac.get(n).is_some_and(|d| *d >= 5);
    let mut f: Vec<u8> = frac.into_iter().take(n).collect();
    f.resize(n, 0);
    if up {
        let mut carry = true;
        for d in f.iter_mut().rev() {
            if *d == 9 {
                *d = 0;
            } else {
                *d += 1;
                carry = false;
                break;
            }
        }
        if carry {
            for d in int.iter_mut().rev() {
                if *d == 9 {
                    *d = 0;
                } else {
                    *d += 1;
                    carry = false;
                    break;
                }
            }
            if carry {
                int.insert(0, 1);
            }
        }
    }
    (int, f)
}

pub(super) fn digits_to_string(d: &[u8]) -> String {
    d.iter().map(|x| (b'0' + x) as char).collect()
}

/// A number as a field shows it with no picture. Integers have no decimal point, anything else has up to 15
/// significant digits and no trailing zeros.
pub fn general_number(value: f64) -> String {
    if !value.is_finite() {
        return value.to_string();
    }
    let (int, frac) = decimal_parts(value.abs());
    if int.is_empty() && frac.is_empty() {
        return "0".to_string();
    }
    let mut s = String::new();
    if value < 0.0 {
        s.push('-');
    }
    if int.is_empty() {
        s.push('0');
    } else {
        s.push_str(&digits_to_string(&int));
    }
    if !frac.is_empty() {
        s.push('.');
        s.push_str(&digits_to_string(&frac));
    }
    s
}

/// Reads a number out of text the way Word does for `\#` on a text result. It accepts a leading `-` or `+`,
/// parentheses for a negative, the currency symbols `$`, `€` and `£`, thousands commas, one decimal point and
/// a trailing `%` (which does not scale the value). `None` when the text is not a number.
pub fn parse_number(s: &str) -> Option<f64> {
    let mut t = s.trim();
    let mut neg = false;
    if let Some(inner) = t.strip_prefix('(').and_then(|r| r.strip_suffix(')')) {
        neg = true;
        t = inner.trim();
    }
    let is_cur = |c: char| matches!(c, '$' | '€' | '£');
    t = t.trim_end_matches('%').trim_end();
    t = t.trim_end_matches(is_cur).trim_end();
    t = t.trim_start_matches(is_cur).trim_start();
    if let Some(r) = t.strip_prefix('-') {
        neg = !neg;
        t = r;
    } else if let Some(r) = t.strip_prefix('+') {
        t = r;
    }
    t = t.trim_start_matches(is_cur).trim_start();
    let mut digits = String::new();
    let mut seen_dot = false;
    let mut prev_digit = false;
    let mut any_digit = false;
    for c in t.chars() {
        match c {
            '0'..='9' => {
                digits.push(c);
                prev_digit = true;
                any_digit = true;
            }
            ',' if prev_digit && !seen_dot => prev_digit = false,
            '.' if !seen_dot => {
                seen_dot = true;
                prev_digit = false;
                digits.push('.');
            }
            _ => return None,
        }
    }
    if !any_digit {
        return None;
    }
    let v: f64 = digits.parse().ok()?;
    Some(if neg { -v } else { v })
}

/// One piece of a numeric picture section.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum PTok {
    Lit(String),
    /// `0`, `#` or `x`.
    Digit(char),
    Dec,
    Thou,
    /// A `-` or `+` sign placeholder.
    Sign(char),
}

pub(super) fn is_ph(c: char) -> bool {
    matches!(c, '0' | '#' | 'x')
}

/// Splits a picture at `;` outside quotes and escapes.
pub(super) fn split_sections(picture: &str) -> Vec<String> {
    let mut out = vec![String::new()];
    let mut in_q = false;
    let mut esc = false;
    for c in picture.chars() {
        if esc {
            esc = false;
        } else if c == '\\' {
            esc = true;
        } else if c == '\'' {
            in_q = !in_q;
        } else if c == ';' && !in_q {
            out.push(String::new());
            continue;
        }
        if let Some(last) = out.last_mut() {
            last.push(c);
        }
    }
    out
}

pub(super) fn parse_section(sec: &str, signs: bool) -> Vec<PTok> {
    let chars: Vec<char> = sec.chars().collect();
    let mut toks: Vec<PTok> = Vec::new();
    let mut seen_digit = false;
    let mut seen_dec = false;
    let mut i = 0;
    let push_lit = |toks: &mut Vec<PTok>, s: &str| match toks.last_mut() {
        Some(PTok::Lit(l)) => l.push_str(s),
        _ => toks.push(PTok::Lit(s.to_string())),
    };
    while i < chars.len() {
        let c = chars[i];
        match c {
            '\'' => {
                let mut lit = String::new();
                i += 1;
                while i < chars.len() {
                    if chars[i] == '\'' {
                        if chars.get(i + 1) == Some(&'\'') {
                            lit.push('\'');
                            i += 2;
                            continue;
                        }
                        break;
                    }
                    if chars[i] == '\\' && i + 1 < chars.len() {
                        i += 1;
                    }
                    lit.push(chars[i]);
                    i += 1;
                }
                i += 1;
                push_lit(&mut toks, &lit);
                continue;
            }
            '\\' => {
                if let Some(n) = chars.get(i + 1) {
                    push_lit(&mut toks, &n.to_string());
                }
                i += 2;
                continue;
            }
            c if is_ph(c) => {
                toks.push(PTok::Digit(c));
                seen_digit = true;
            }
            '.' => {
                let prev_ph = matches!(toks.last(), Some(PTok::Digit(_)));
                let next_ph = chars.get(i + 1).is_some_and(|n| is_ph(*n));
                if !seen_dec && (prev_ph || next_ph) {
                    toks.push(PTok::Dec);
                    seen_dec = true;
                } else {
                    push_lit(&mut toks, ".");
                }
            }
            ',' => {
                let prev_ph = matches!(toks.last(), Some(PTok::Digit(_)));
                let next_ph = chars.get(i + 1).is_some_and(|n| is_ph(*n));
                if !seen_dec && prev_ph && next_ph {
                    toks.push(PTok::Thou);
                } else {
                    push_lit(&mut toks, ",");
                }
            }
            '-' | '+' if signs && !seen_digit => toks.push(PTok::Sign(c)),
            other => push_lit(&mut toks, &other.to_string()),
        }
        i += 1;
    }
    toks
}

pub(super) fn push_digit(out: &mut Vec<String>, ch: char, shown: &mut usize, thou: bool) {
    if thou && *shown > 0 && (*shown).is_multiple_of(3) {
        out.push(",".to_string());
    }
    out.push(ch.to_string());
    *shown += 1;
}

/// Lays the digits into one section. `int` has no leading zeros, `frac` has exactly as many digits as the section
/// has fraction placeholders. `neg` and `zero` describe the rounded value for the sign placeholders.
pub(super) fn render_section(toks: &[PTok], int: &[u8], frac: &[u8], neg: bool, zero: bool) -> String {
    let dec_at = toks.iter().position(|t| *t == PTok::Dec);
    let int_toks = &toks[..dec_at.unwrap_or(toks.len())];
    let frac_toks: &[PTok] = dec_at.map_or(&[], |d| &toks[d + 1..]);
    let thou = int_toks.contains(&PTok::Thou);
    let k = int_toks.iter().filter(|t| matches!(t, PTok::Digit(_))).count();
    let trunc = int_toks.iter().find_map(|t| if let PTok::Digit(c) = t { Some(*c == 'x') } else { None }).unwrap_or(false);

    // Integer part, built from the right.
    let mut rev: Vec<String> = Vec::new();
    let mut shown = 0usize;
    if k == 0 && !trunc {
        for d in int.iter().rev() {
            push_digit(&mut rev, (b'0' + d) as char, &mut shown, thou);
        }
    }
    let mut pos = 0usize;
    for tok in int_toks.iter().rev() {
        match tok {
            PTok::Digit(c) => {
                let d = int.len().checked_sub(pos + 1).map(|ix| int[ix]);
                pos += 1;
                let ch = match d {
                    Some(d) => Some((b'0' + d) as char),
                    None if *c == '0' => Some('0'),
                    None => None,
                };
                if let Some(ch) = ch {
                    push_digit(&mut rev, ch, &mut shown, thou);
                }
                if pos == k && !trunc {
                    for ix in (0..int.len().saturating_sub(k)).rev() {
                        push_digit(&mut rev, (b'0' + int[ix]) as char, &mut shown, thou);
                    }
                }
            }
            PTok::Lit(l) => rev.push(l.clone()),
            PTok::Sign(s) => rev.push(sign_char(*s, neg, zero).to_string()),
            PTok::Thou | PTok::Dec => {}
        }
    }
    rev.reverse();
    let mut out: String = rev.concat();

    // Fraction part, left to right.
    let ph: Vec<char> = frac_toks.iter().filter_map(|t| if let PTok::Digit(c) = t { Some(*c) } else { None }).collect();
    let mut show = vec![false; ph.len()];
    let mut seen = false;
    for j in (0..ph.len()).rev() {
        if ph[j] == '0' || frac.get(j).is_some_and(|d| *d != 0) {
            seen = true;
        }
        show[j] = seen;
    }
    if show.iter().any(|s| *s) {
        out.push('.');
    }
    let mut j = 0;
    for tok in frac_toks {
        match tok {
            PTok::Digit(_) => {
                if show[j] {
                    out.push((b'0' + frac.get(j).copied().unwrap_or(0)) as char);
                }
                j += 1;
            }
            PTok::Lit(l) => out.push_str(l),
            PTok::Sign(s) => out.push(sign_char(*s, neg, zero)),
            PTok::Thou | PTok::Dec => {}
        }
    }
    out
}

pub(super) fn sign_char(s: char, neg: bool, zero: bool) -> char {
    if neg {
        '-'
    } else if s == '+' && !zero {
        '+'
    } else {
        ' '
    }
}

/// Word's `\#` numeric picture applied to a number.
///
/// `0` is a digit placeholder that shows `0` when the number has no digit there. `#` shows nothing when there is
/// no digit. `x` as the first placeholder left of the decimal point cuts off the digits to its left. `.` is the
/// decimal point and `,` between placeholders is the thousands separator; the output always uses `,` and `.`.
/// A leading `-` shows a minus for a negative number and a space otherwise, a leading `+` shows `+`, `-` or a
/// space for zero. Other characters, and text in single quotes, are copied. `%` does not scale the number.
///
/// Sections are separated by `;`. With one section a negative number gets a minus sign. With two, the first is
/// for positive numbers and zero and the second for negative numbers. With three, the third is for zero. In the
/// negative section the number is shown without a sign, so the section decides how it looks. Digits to the right
/// of the last fraction placeholder are rounded off, half away from zero. A picture with no placeholders at all
/// is returned as it is.
pub fn numeric_picture(value: f64, picture: &str) -> String {
    if !value.is_finite() {
        return general_number(value);
    }
    let sections = split_sections(picture);
    let single = sections.len() == 1;
    let parsed: Vec<Vec<PTok>> = sections.iter().map(|s| parse_section(s, single)).collect();
    let has_ph = |t: &Vec<PTok>| t.iter().any(|x| matches!(x, PTok::Digit(_)));
    if !parsed.iter().any(has_ph) {
        return picture.to_string();
    }
    let idx = if value < 0.0 && sections.len() >= 2 {
        1
    } else if value == 0.0 && sections.len() >= 3 {
        2
    } else {
        0
    };
    let toks = &parsed[idx];
    let nfrac = toks
        .iter()
        .skip_while(|t| **t != PTok::Dec)
        .filter(|t| matches!(t, PTok::Digit(_)))
        .count();
    let (int, frac) = decimal_parts(value.abs());
    let (int, frac) = round_parts(int, frac, nfrac);
    let is_zero = int.is_empty() && frac.iter().all(|d| *d == 0);
    let neg = value < 0.0 && !is_zero;
    let body = render_section(toks, &int, &frac, neg && single, is_zero);
    let has_sign = toks.iter().any(|t| matches!(t, PTok::Sign(_)));
    if single && neg && !has_sign {
        format!("-{body}")
    } else {
        body
    }
}
