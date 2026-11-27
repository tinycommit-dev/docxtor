//! Cell and range references: parsing them, and which way a chart lays its series out.

use super::*;

/// A rectangle of cells, 1-based and inclusive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Area {
    pub(super) r1: u32,
    pub(super) c1: u32,
    pub(super) r2: u32,
    pub(super) c2: u32,
}

impl Area {
    pub(super) fn cell(r: u32, c: u32) -> Area {
        Area { r1: r, c1: c, r2: r, c2: c }
    }
    pub(super) fn contains(&self, o: &Area) -> bool {
        self.r1 <= o.r1 && self.c1 <= o.c1 && self.r2 >= o.r2 && self.c2 >= o.c2
    }
    pub(super) fn contains_cell(&self, r: u32, c: u32) -> bool {
        self.contains(&Area::cell(r, c))
    }
    pub(super) fn union(&self, o: &Area) -> Area {
        Area { r1: self.r1.min(o.r1), c1: self.c1.min(o.c1), r2: self.r2.max(o.r2), c2: self.c2.max(o.c2) }
    }
    pub(super) fn intersects(&self, o: &Area) -> bool {
        self.r1 <= o.r2 && o.r1 <= self.r2 && self.c1 <= o.c2 && o.c1 <= self.c2
    }
    pub(super) fn cells(&self) -> u64 {
        (self.r2 - self.r1 + 1) as u64 * (self.c2 - self.c1 + 1) as u64
    }
    /// `$B$2:$B$5`, or `$B$2` for one cell.
    pub(super) fn abs(&self) -> String {
        let one = |r: u32, c: u32| format!("${}${}", col_name(c as usize - 1), r);
        if self.r1 == self.r2 && self.c1 == self.c2 {
            one(self.r1, self.c1)
        } else {
            format!("{}:{}", one(self.r1, self.c1), one(self.r2, self.c2))
        }
    }
    /// `B2:B5` (table refs), or `B2` for one cell.
    pub(super) fn plain(&self) -> String {
        if self.r1 == self.r2 && self.c1 == self.c2 {
            cell_name(self.r1, self.c1)
        } else {
            format!("{}:{}", cell_name(self.r1, self.c1), cell_name(self.r2, self.c2))
        }
    }
}

pub(super) fn cell_name(r: u32, c: u32) -> String {
    format!("{}{}", col_name(c as usize - 1), r)
}

/// `B2`, `$B$2`, `b2`: (row, column), 1-based.
pub(super) fn parse_cell(s: &str) -> Option<(u32, u32)> {
    let b = s.as_bytes();
    let mut i = 0;
    if b.get(i) == Some(&b'$') {
        i += 1;
    }
    let cs = i;
    let mut col: u32 = 0;
    while i < b.len() && b[i].is_ascii_alphabetic() {
        col = col * 26 + (b[i].to_ascii_uppercase() - b'A') as u32 + 1;
        i += 1;
        if i - cs > 3 {
            return None;
        }
    }
    if i == cs {
        return None;
    }
    if b.get(i) == Some(&b'$') {
        i += 1;
    }
    let rs = i;
    while i < b.len() && b[i].is_ascii_digit() {
        i += 1;
        if i - rs > 7 {
            return None;
        }
    }
    if i == rs || i != b.len() {
        return None;
    }
    let row: u32 = s[rs..i].parse().ok()?;
    ((1..=MAX_ROW).contains(&row) && (1..=MAX_COL).contains(&col)).then_some((row, col))
}

pub(super) fn parse_area(s: &str) -> Option<Area> {
    let mut parts = s.trim().split(':');
    let a = parse_cell(parts.next()?)?;
    let b = match parts.next() {
        Some(p) => parse_cell(p)?,
        None => a,
    };
    if parts.next().is_some() {
        return None;
    }
    Some(Area { r1: a.0.min(b.0), c1: a.1.min(b.1), r2: a.0.max(b.0), c2: a.1.max(b.1) })
}

/// A range on one sheet: `'My Sheet'!$A$2:$A$5`.
#[derive(Debug, Clone)]
pub(super) struct Ref {
    /// The sheet name, unquoted.
    pub(super) sheet: String,
    /// The sheet part as written, with the `!` (`'My Sheet'!`), reused for the new ranges.
    pub(super) prefix: String,
    pub(super) area: Area,
}

pub(super) fn parse_ref(s: &str) -> Option<Ref> {
    let s = s.trim();
    let s = s.strip_prefix('=').unwrap_or(s);
    let (sheet, rest) = if let Some(q) = s.strip_prefix('\'') {
        let mut name = String::new();
        let mut chars = q.char_indices().peekable();
        let mut end = None;
        while let Some((i, ch)) = chars.next() {
            if ch == '\'' {
                if matches!(chars.peek(), Some((_, '\''))) {
                    chars.next();
                    name.push('\'');
                } else {
                    end = Some(i);
                    break;
                }
            } else {
                name.push(ch);
            }
        }
        let rest = q[end? + 1..].strip_prefix('!')?;
        (name, rest)
    } else {
        let bang = s.find('!')?;
        let name = &s[..bang];
        if name.chars().any(|c| c.is_whitespace() || "'\"(),;".contains(c)) {
            return None;
        }
        (name.to_string(), &s[bang + 1..])
    };
    // Sheet names can't hold these; `[1]Sheet1` is an external workbook and `Sheet1:Sheet3` a 3-D ref.
    if sheet.is_empty() || sheet.contains(['[', ']', ':', '\\', '/', '?', '*']) {
        return None;
    }
    let area = parse_area(rest)?;
    let prefix = s[..s.len() - rest.len()].to_string();
    Some(Ref { sheet, prefix, area })
}

/// The old ranges of one series.
pub(super) struct Old {
    pub(super) name: Option<Ref>,
    pub(super) cat: Option<Ref>,
    pub(super) val: Ref,
    pub(super) size: Option<Ref>,
}

impl Old {
    pub(super) fn parse(r: &SeriesRefs, size: Option<&str>) -> Option<Old> {
        let opt = |s: &str| -> Option<Option<Ref>> {
            if s.trim().is_empty() {
                Some(None)
            } else {
                parse_ref(s).map(Some)
            }
        };
        let name = opt(&r.name)?;
        if name.as_ref().is_some_and(|n| n.area.cells() != 1) {
            return None;
        }
        Some(Old { name, cat: opt(&r.cat)?, val: parse_ref(&r.val)?, size: opt(size.unwrap_or(""))? })
    }
}

/// Whether series run down columns (`true`) or along rows.
pub(super) fn orientation(olds: &[Option<Old>]) -> Option<bool> {
    let mut found: Option<bool> = None;
    for old in olds.iter().flatten() {
        for r in [Some(&old.val), old.cat.as_ref(), old.size.as_ref()].into_iter().flatten() {
            let a = r.area;
            let dir = match (a.r1 == a.r2, a.c1 == a.c2) {
                (true, true) => continue,
                (false, true) => true,
                (true, false) => false,
                (false, false) => return None,
            };
            if found.is_some_and(|f| f != dir) {
                return None;
            }
            found = Some(dir);
        }
    }
    if let Some(f) = found {
        return Some(f);
    }
    // Every range is one cell: the name sits above the value (columns) or left of it (rows).
    let first = olds.iter().flatten().next()?;
    Some(!matches!(&first.name, Some(n) if n.area.r1 == first.val.area.r1 && n.area.c1 != first.val.area.c1))
}

/// Coordinates along a series (`along`) and across series, for either layout.
#[derive(Clone, Copy)]
pub(super) struct Orient(pub(super) bool);

impl Orient {
    pub(super) fn rc(&self, along: u32, across: u32) -> (u32, u32) {
        if self.0 {
            (along, across)
        } else {
            (across, along)
        }
    }
    pub(super) fn along(&self, r: u32, c: u32) -> u32 {
        if self.0 {
            r
        } else {
            c
        }
    }
    pub(super) fn across(&self, r: u32, c: u32) -> u32 {
        if self.0 {
            c
        } else {
            r
        }
    }
    pub(super) fn max_along(&self) -> u32 {
        if self.0 {
            MAX_ROW
        } else {
            MAX_COL
        }
    }
    pub(super) fn max_across(&self) -> u32 {
        if self.0 {
            MAX_COL
        } else {
            MAX_ROW
        }
    }
    pub(super) fn line(&self, along: u32, across: u32, n: u32) -> Area {
        let (r1, c1) = self.rc(along, across);
        let (r2, c2) = self.rc(along + n - 1, across);
        Area { r1, c1, r2, c2 }
    }
}
