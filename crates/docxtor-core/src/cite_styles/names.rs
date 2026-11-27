//! Author and editor names in each style's order and punctuation.

use super::*;

pub(super) fn given_full(n: &Name) -> &str {
    n.given().unwrap_or("")
}

/// "Smith, J. Q." (family first, initials).
pub(super) fn fam_init(n: &Name) -> String {
    match n {
        Name::Literal(l) => l.clone(),
        Name::Person { family, .. } => match n.given().map(initials).filter(|i| !i.is_empty()) {
            Some(i) => format!("{family}, {i}"),
            None => family.clone(),
        },
    }
}

/// "J. Q. Smith" (initials first).
pub(super) fn init_fam(n: &Name) -> String {
    match n {
        Name::Literal(l) => l.clone(),
        Name::Person { family, .. } => match n.given().map(initials).filter(|i| !i.is_empty()) {
            Some(i) => format!("{i} {family}"),
            None => family.clone(),
        },
    }
}

/// "Smith, John Q." (family first, full given names).
pub(super) fn fam_given(n: &Name) -> String {
    match n {
        Name::Literal(l) => l.clone(),
        Name::Person { family, .. } => match given_full(n) {
            "" => family.clone(),
            g => format!("{family}, {g}"),
        },
    }
}

/// "John Q. Smith" (given names first).
pub(super) fn given_fam(n: &Name) -> String {
    match n {
        Name::Literal(l) => l.clone(),
        Name::Person { family, .. } => match given_full(n) {
            "" => family.clone(),
            g => format!("{g} {family}"),
        },
    }
}

pub(super) fn families(names: &[Name]) -> Vec<String> {
    names.iter().map(|n| n.family().to_string()).collect()
}

/// A name list inside a sentence: the names, then "et al." when there are too many.
pub(super) fn apa_authors(names: &[Name]) -> String {
    let f: Vec<String> = names.iter().map(fam_init).collect();
    match f.len() {
        0 => String::new(),
        1 => f[0].clone(),
        2 => format!("{}, & {}", f[0], f[1]),
        n if n <= 20 => format!("{}, & {}", f[..n - 1].join(", "), f[n - 1]),
        n => format!("{}, . . . {}", f[..19].join(", "), f[n - 1]),
    }
}

pub(super) fn apa_editors(names: &[Name]) -> String {
    let f: Vec<String> = names.iter().map(init_fam).collect();
    match f.len() {
        0 => String::new(),
        1 => f[0].clone(),
        2 => format!("{} & {}", f[0], f[1]),
        n => format!("{}, & {}", f[..n - 1].join(", "), f[n - 1]),
    }
}

pub(super) fn mla_authors(names: &[Name]) -> String {
    match names.len() {
        0 => String::new(),
        1 => fam_given(&names[0]),
        2 => format!("{}, and {}", fam_given(&names[0]), given_fam(&names[1])),
        _ => format!("{}, et al.", fam_given(&names[0])),
    }
}

pub(super) fn mla_editors(names: &[Name]) -> String {
    let f: Vec<String> = names.iter().map(given_fam).collect();
    match f.len() {
        0 => String::new(),
        1 | 2 => join_and(&f, "and", false),
        _ => format!("{}, et al.", f[0]),
    }
}

pub(super) fn chicago_authors(names: &[Name]) -> String {
    let mut f: Vec<String> = names.iter().enumerate().map(|(i, n)| if i == 0 { fam_given(n) } else { given_fam(n) }).collect();
    if f.len() > 10 {
        f.truncate(7);
        return format!("{}, et al.", f.join(", "));
    }
    match f.len() {
        0 => String::new(),
        1 => f[0].clone(),
        // The first name is inverted, so even two authors need the comma: "Smith, John, and Karen Lee".
        2 => format!("{}, and {}", f[0], f[1]),
        _ => join_and(&f, "and", true),
    }
}

pub(super) fn chicago_editors(names: &[Name]) -> String {
    let f: Vec<String> = names.iter().map(given_fam).collect();
    join_and(&f, "and", true)
}

pub(super) fn harvard_authors(names: &[Name]) -> String {
    let f: Vec<String> = names.iter().map(fam_init).collect();
    match f.len() {
        0 => String::new(),
        1 => f[0].clone(),
        2 | 3 => join_and(&f, "and", false),
        _ => format!("{} et al.", f[0]),
    }
}

pub(super) fn harvard_editors(names: &[Name]) -> String {
    let f: Vec<String> = names.iter().map(fam_init).collect();
    match f.len() {
        0 => String::new(),
        1 => f[0].clone(),
        2 | 3 => join_and(&f, "and", false),
        _ => format!("{} et al.", f[0]),
    }
}

pub(super) fn ieee_authors(names: &[Name]) -> String {
    let f: Vec<String> = names.iter().map(init_fam).collect();
    match f.len() {
        0 => String::new(),
        1 => f[0].clone(),
        2 => join_and(&f, "and", false),
        n if n <= 6 => join_and(&f, "and", true),
        _ => format!("{} et al.", f[0]),
    }
}
