//! Reading a list of recipients from a text file: comma, semicolon, tab or bar separated values, the way a
//! spreadsheet saves them (quotes around a value that holds the separator, a quote or a line break).

use super::Table;

/// The text of a file: UTF-8 (with or without a byte order mark), UTF-16 with a mark, or Windows-1252.
pub fn decode_text(bytes: &[u8]) -> String {
    if let Some(rest) = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
        return String::from_utf8_lossy(rest).into_owned();
    }
    if bytes.starts_with(&[0xFF, 0xFE]) || bytes.starts_with(&[0xFE, 0xFF]) {
        let le = bytes[0] == 0xFF;
        let units: Vec<u16> = bytes[2..].chunks_exact(2).map(|c| if le { u16::from_le_bytes([c[0], c[1]]) } else { u16::from_be_bytes([c[0], c[1]]) }).collect();
        return String::from_utf16_lossy(&units);
    }
    match std::str::from_utf8(bytes) {
        Ok(s) => s.to_string(),
        // Not UTF-8: files saved by older spreadsheet programs are in Windows-1252.
        Err(_) => bytes.iter().map(|&b| cp1252(b)).collect(),
    }
}

fn cp1252(b: u8) -> char {
    const HIGH: [char; 32] = [
        '\u{20AC}', '\u{81}', '\u{201A}', '\u{0192}', '\u{201E}', '\u{2026}', '\u{2020}', '\u{2021}', '\u{02C6}', '\u{2030}', '\u{0160}', '\u{2039}', '\u{0152}', '\u{8D}', '\u{017D}', '\u{8F}', '\u{90}', '\u{2018}', '\u{2019}',
        '\u{201C}', '\u{201D}', '\u{2022}', '\u{2013}', '\u{2014}', '\u{02DC}', '\u{2122}', '\u{0161}', '\u{203A}', '\u{0153}', '\u{9D}', '\u{017E}', '\u{0178}',
    ];
    match b {
        0x80..=0x9F => HIGH[(b - 0x80) as usize],
        b => b as char,
    }
}

/// The separator a file uses: the one of `, ; <tab> |` that appears most in its first line outside quotes.
pub fn detect_delimiter(text: &str) -> char {
    let mut counts = [0usize; 4];
    let mut quoted = false;
    for c in text.chars() {
        match c {
            '"' => quoted = !quoted,
            '\n' | '\r' if !quoted => break,
            ',' if !quoted => counts[0] += 1,
            ';' if !quoted => counts[1] += 1,
            '\t' if !quoted => counts[2] += 1,
            '|' if !quoted => counts[3] += 1,
            _ => {}
        }
    }
    let seps = [',', ';', '\t', '|'];
    let mut best = 0;
    for i in 1..4 {
        if counts[i] > counts[best] {
            best = i;
        }
    }
    seps[best]
}

/// Split text into records of values.
pub fn records(text: &str, delimiter: char) -> Vec<Vec<String>> {
    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut row: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut quoted = false;
    let mut was_quoted = false;
    let mut chars = text.chars().peekable();
    let mut any = false;
    while let Some(c) = chars.next() {
        any = true;
        if quoted {
            if c == '"' {
                if chars.peek() == Some(&'"') {
                    chars.next();
                    cur.push('"');
                } else {
                    quoted = false;
                }
            } else {
                cur.push(c);
            }
            continue;
        }
        match c {
            '"' if cur.is_empty() && !was_quoted => {
                quoted = true;
                was_quoted = true;
            }
            c if c == delimiter => {
                row.push(std::mem::take(&mut cur));
                was_quoted = false;
            }
            '\r' | '\n' => {
                if c == '\r' && chars.peek() == Some(&'\n') {
                    chars.next();
                }
                row.push(std::mem::take(&mut cur));
                rows.push(std::mem::take(&mut row));
                was_quoted = false;
                any = false;
            }
            c => cur.push(c),
        }
    }
    if any || !row.is_empty() {
        row.push(cur);
        rows.push(row);
    }
    rows
}

/// A table from text: the first non-empty record is the header, the rest are recipients. Empty records are skipped.
pub fn parse(text: &str, delimiter: Option<char>) -> Result<(Table, char), String> {
    crate::budget::within(text.len(), crate::budget::caps::LIST_BYTES).map_err(|e| e.to_string())?;
    let text = text.strip_prefix('\u{FEFF}').unwrap_or(text);
    let delim = delimiter.unwrap_or_else(|| detect_delimiter(text));
    let mut rows = records(text, delim);
    rows.retain(|r| r.iter().any(|v| !v.trim().is_empty()));
    if rows.is_empty() {
        return Err("The file has no rows.".into());
    }
    let header = rows.remove(0);
    Ok((Table::from_rows(header, rows), delim))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quotes_and_breaks() {
        let t = "First Name,Last Name,Note\r\nAda,\"Lovelace\",\"Said \"\"hi\"\",\nthen left\"\n\nGrace,Hopper,\n";
        let (table, d) = parse(t, None).unwrap();
        assert_eq!(d, ',');
        assert_eq!(table.columns, ["First Name", "Last Name", "Note"]);
        assert_eq!(table.rows.len(), 2);
        assert_eq!(table.rows[0], ["Ada", "Lovelace", "Said \"hi\",\nthen left"]);
        assert_eq!(table.rows[1], ["Grace", "Hopper", ""]);
    }

    #[test]
    fn other_separators_and_encodings() {
        let (t, d) = parse("a;b\n1;2\n", None).unwrap();
        assert_eq!((d, t.rows[0].clone()), (';', vec!["1".to_string(), "2".to_string()]));
        let (t, d) = parse("a\tb\n1\t2", None).unwrap();
        assert_eq!((d, t.columns.len()), ('\t', 2));
        // A comma inside quotes does not count when the separator is chosen.
        assert_eq!(detect_delimiter("\"a,b,c\";d;e"), ';');
        assert_eq!(decode_text(&[0xEF, 0xBB, 0xBF, b'a']), "a");
        assert_eq!(decode_text(&[0x4A, 0x6F, 0x73, 0xE9]), "Jos\u{e9}");
        assert_eq!(decode_text(&[0xFF, 0xFE, b'h', 0, b'i', 0]), "hi");
    }

    #[test]
    fn header_names_are_made_usable() {
        let (t, _) = parse("Name,,Name,\nAda,x,y,z", None).unwrap();
        assert_eq!(t.columns, ["Name", "Column 2", "Name 2", "Column 4"]);
        // Short rows are padded, long rows are cut.
        let (t, _) = parse("a,b\n1\n1,2,3", None).unwrap();
        assert_eq!(t.rows, [vec!["1", ""], vec!["1", "2"]]);
        assert!(parse("\n\n", None).is_err());
    }

    #[test]
    fn a_header_with_thousands_of_repeated_names_is_quick_and_cut_to_the_column_limit() {
        // Each name is made unique in one step, not by comparing it with every column before it.
        let text = format!("{}\n1", ",x".repeat(100_000));
        let started = std::time::Instant::now();
        let (t, _) = parse(&text, None).unwrap();
        assert!(started.elapsed() < std::time::Duration::from_secs(5), "took {:?}", started.elapsed());
        assert_eq!(t.columns.len(), crate::budget::caps::LIST_COLS);
        assert_eq!(t.columns[1..4], ["x", "x 2", "x 3"]);
        let names: std::collections::HashSet<&String> = t.columns.iter().collect();
        assert_eq!(names.len(), t.columns.len(), "every name is different");
        // A name already used that looks like a numbered one is skipped over.
        let (t, _) = parse("Name 2,Name,Name\n1,2,3", None).unwrap();
        assert_eq!(t.columns, ["Name 2", "Name", "Name 3"]);
    }
}
