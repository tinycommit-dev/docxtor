//! BibTeX in and out: `parse` reads what Google Scholar, Zotero, Mendeley, publishers and hand-edited `.bib`
//! files give, however messy, and `to_bibtex` writes sources back.

use crate::citations::{month_number, nz, Name, Source, SourceType, MONTH_NAMES};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Reading
// ---------------------------------------------------------------------------

struct Entry {
    kind: String,
    fields: Vec<(String, String)>,
}

struct Scanner<'a> {
    s: &'a [char],
    i: usize,
    macros: HashMap<String, String>,
}

impl<'a> Scanner<'a> {
    fn peek(&self) -> Option<char> {
        self.s.get(self.i).copied()
    }

    fn skip_ws(&mut self) {
        while self.peek().is_some_and(char::is_whitespace) {
            self.i += 1;
        }
    }

    fn word(&mut self) -> String {
        let start = self.i;
        while self.peek().is_some_and(|c| c.is_alphanumeric() || matches!(c, '_' | '-' | ':' | '.' | '+' | '/' | '\'' | '$' | '&' | '*' | '!' | '?' | '<' | '>' | '[' | ']' | ';' | '@')) {
            self.i += 1;
        }
        self.s[start..self.i].iter().collect()
    }

    /// A balanced `{...}` group; the text inside, with inner braces kept. An unclosed group runs to the end.
    fn braced(&mut self) -> String {
        let mut depth = 0;
        let mut out = String::new();
        while let Some(c) = self.peek() {
            self.i += 1;
            match c {
                '\\' => {
                    out.push(c);
                    if let Some(n) = self.peek() {
                        out.push(n);
                        self.i += 1;
                    }
                }
                '{' => {
                    if depth > 0 {
                        out.push(c);
                    }
                    depth += 1;
                }
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        return out;
                    }
                    out.push(c);
                }
                c => out.push(c),
            }
        }
        out
    }

    fn quoted(&mut self) -> String {
        self.i += 1;
        let mut depth = 0;
        let mut out = String::new();
        while let Some(c) = self.peek() {
            self.i += 1;
            match c {
                '\\' => {
                    out.push(c);
                    if let Some(n) = self.peek() {
                        out.push(n);
                        self.i += 1;
                    }
                }
                '{' => {
                    depth += 1;
                    out.push(c);
                }
                '}' => {
                    depth -= 1;
                    out.push(c);
                }
                '"' if depth <= 0 => return out,
                c => out.push(c),
            }
        }
        out
    }

    /// A field value: pieces joined with `#`, each a braced group, a quoted string, a number or a macro.
    /// The result is the text in one pair of braces, so name lists can tell `{Org Name}` from `Org Name`.
    fn value(&mut self) -> String {
        let mut pieces: Vec<String> = Vec::new();
        loop {
            self.skip_ws();
            match self.peek() {
                Some('{') => pieces.push(self.braced()),
                Some('"') => pieces.push(self.quoted()),
                Some(c) if c.is_alphanumeric() => {
                    let w = self.word();
                    let w = w.trim_end_matches(|c: char| !c.is_alphanumeric()).to_string();
                    pieces.push(self.macros.get(&w.to_lowercase()).cloned().unwrap_or(w));
                }
                _ => break,
            }
            self.skip_ws();
            if self.peek() == Some('#') {
                self.i += 1;
                continue;
            }
            break;
        }
        format!("{{{}}}", pieces.join(""))
    }
}

/// Top-level `{...}` groups of a string.
fn split_top(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut depth = 0;
    let mut cur = String::new();
    for c in s.chars() {
        match c {
            '{' => {
                if depth > 0 {
                    cur.push(c);
                }
                depth += 1;
            }
            '}' => {
                depth -= 1;
                if depth == 0 {
                    out.push(std::mem::take(&mut cur));
                } else {
                    cur.push(c);
                }
            }
            c if depth > 0 => cur.push(c),
            _ => {}
        }
    }
    out
}

fn scan(text: &str) -> Vec<Entry> {
    let chars: Vec<char> = text.chars().collect();
    let mut sc = Scanner { s: &chars, i: 0, macros: HashMap::new() };
    let mut out = Vec::new();
    while sc.i < chars.len() {
        // Anything outside an entry is a comment.
        if chars[sc.i] != '@' {
            sc.i += 1;
            continue;
        }
        sc.i += 1;
        sc.skip_ws();
        let kind = {
            let start = sc.i;
            while sc.peek().is_some_and(|c| c.is_alphabetic()) {
                sc.i += 1;
            }
            chars[start..sc.i].iter().collect::<String>().to_lowercase()
        };
        sc.skip_ws();
        let Some(open) = sc.peek().filter(|c| matches!(c, '{' | '(')) else { continue };
        let close = if open == '{' { '}' } else { ')' };
        sc.i += 1;
        match kind.as_str() {
            "comment" | "preamble" => {
                // Skip to the matching close.
                let mut depth = 1;
                while let Some(c) = sc.peek() {
                    sc.i += 1;
                    if c == open {
                        depth += 1;
                    } else if c == close {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                }
                continue;
            }
            "string" => {
                sc.skip_ws();
                let name = sc.word().to_lowercase();
                sc.skip_ws();
                if sc.peek() == Some('=') {
                    sc.i += 1;
                    let v = sc.value();
                    let v = split_top(&v).join("");
                    sc.macros.insert(name, v);
                }
                while let Some(c) = sc.peek() {
                    sc.i += 1;
                    if c == close {
                        break;
                    }
                }
                continue;
            }
            _ => {}
        }
        sc.skip_ws();
        // The cite key runs to the first comma (it may be missing: `@misc{,`).
        let mut key = String::new();
        while let Some(c) = sc.peek() {
            if c == ',' || c == close || c == '=' {
                break;
            }
            key.push(c);
            sc.i += 1;
        }
        // `@misc{title = {x}, ...}`: no key, we read a field.
        if sc.peek() == Some('=') {
            sc.i -= key.chars().count();
            key.clear();
        }
        if sc.peek() == Some(',') {
            sc.i += 1;
        }
        let mut fields = Vec::new();
        loop {
            sc.skip_ws();
            match sc.peek() {
                None => break,
                Some(c) if c == close => {
                    sc.i += 1;
                    break;
                }
                Some(',') => {
                    sc.i += 1;
                    continue;
                }
                // The next entry begins: this one was never closed.
                Some('@') => break,
                _ => {}
            }
            let name = sc.word().to_lowercase();
            if name.is_empty() {
                sc.i += 1;
                continue;
            }
            sc.skip_ws();
            if sc.peek() != Some('=') {
                continue;
            }
            sc.i += 1;
            let v = sc.value();
            fields.push((name, v));
        }
        out.push(Entry { kind, fields });
    }
    out
}

// ---------------------------------------------------------------------------
// LaTeX to text
// ---------------------------------------------------------------------------

fn compose(accent: char, base: char) -> Option<char> {
    let table: &[(&str, &str)] = match accent {
        '\'' => &[("aeiouyAEIOUYcCnNsSzZlL", "áéíóúýÁÉÍÓÚÝćĆńŃśŚźŹĺĹ")],
        '`' => &[("aeiouAEIOU", "àèìòùÀÈÌÒÙ")],
        '^' => &[("aeiouAEIOUcCgGhHjJsSwWyY", "âêîôûÂÊÎÔÛĉĈĝĜĥĤĵĴŝŜŵŴŷŶ")],
        '"' => &[("aeiouyAEIOUY", "äëïöüÿÄËÏÖÜŸ")],
        '~' => &[("anoANOiIuU", "ãñõÃÑÕĩĨũŨ")],
        'c' => &[("cCsStTgGkKlLnNrRaeAE", "çÇşŞţŢģĢķĶļĻņŅŗŖąęĄĘ")],
        'v' => &[("cCsSzZrRnNdDeEtTlLaA", "čČšŠžŽřŘňŇďĎěĚťŤľĽǎǍ")],
        'r' => &[("aAuU", "åÅůŮ")],
        '=' => &[("aeiouAEIOU", "āēīōūĀĒĪŌŪ")],
        '.' => &[("zZcCeEgGIaA", "żŻċĊėĖġĠİȧȦ")],
        'H' => &[("oOuU", "őŐűŰ")],
        'u' => &[("aAeEgGiIoOuU", "ăĂĕĔğĞĭĬŏŎŭŬ")],
        'k' => &[("aAeEiIuU", "ąĄęĘįĮųŲ")],
        _ => return None,
    };
    for (from, to) in table {
        if let Some(i) = from.chars().position(|c| c == base) {
            return to.chars().nth(i);
        }
    }
    None
}

fn special(cmd: &str) -> Option<&'static str> {
    Some(match cmd {
        "ss" => "ß",
        "o" => "ø",
        "O" => "Ø",
        "aa" => "å",
        "AA" => "Å",
        "ae" => "æ",
        "AE" => "Æ",
        "oe" => "œ",
        "OE" => "Œ",
        "l" => "ł",
        "L" => "Ł",
        "i" => "ı",
        "j" => "ȷ",
        "dh" => "ð",
        "DH" => "Ð",
        "th" => "þ",
        "TH" => "Þ",
        "textendash" => "–",
        "textemdash" => "—",
        "textquoteright" => "’",
        "textquoteleft" => "‘",
        "textquotedblleft" => "“",
        "textquotedblright" => "”",
        "textasciitilde" => "~",
        "textunderscore" => "_",
        "textampersand" => "&",
        "textless" => "<",
        "textgreater" => ">",
        "textbackslash" => "\\",
        "copyright" => "©",
        "pounds" => "£",
        "euro" => "€",
        "dots" | "ldots" | "textellipsis" => "…",
        "newline" | "par" | "linebreak" => " ",
        "relax" | "bibinitperiod" | "bibnamedelima" | "bibnamedelimb" | "bibnamedelimc" | "bibnamedelimd" | "bibnamedelimi" => "",
        _ => return None,
    })
}

/// LaTeX markup to plain text: accents, special letters, protected braces, dashes, simple commands.
pub fn latex_to_text(s: &str) -> String {
    let c: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < c.len() {
        match c[i] {
            '\\' if i + 1 < c.len() => {
                let n = c[i + 1];
                if matches!(n, '\'' | '`' | '^' | '"' | '~' | '=' | '.') {
                    // \'e, \'{e}, \'{\i}
                    let mut j = i + 2;
                    let mut base = None;
                    if j < c.len() && c[j] == '{' {
                        let mut k = j + 1;
                        if k < c.len() && c[k] == '\\' && k + 1 < c.len() && c[k + 1] == 'i' {
                            base = Some('i');
                            k += 2;
                        } else if k < c.len() {
                            base = Some(c[k]);
                            k += 1;
                        }
                        while k < c.len() && c[k] != '}' {
                            k += 1;
                        }
                        j = (k + 1).min(c.len());
                    } else if j < c.len() && c[j] == '\\' && j + 1 < c.len() && c[j + 1] == 'i' {
                        base = Some('i');
                        j += 2;
                    } else if j < c.len() {
                        base = Some(c[j]);
                        j += 1;
                    }
                    if let Some(b) = base {
                        out.push(compose(n, b).unwrap_or(b));
                    }
                    i = j;
                } else if matches!(n, '&' | '%' | '_' | '$' | '#' | '{' | '}') {
                    out.push(n);
                    i += 2;
                } else if n == ' ' || n == ',' || n == ';' || n == '!' || n == '/' || n == '-' {
                    if n == ' ' {
                        out.push(' ');
                    }
                    i += 2;
                } else if n == '\\' {
                    out.push(' ');
                    i += 2;
                } else if n.is_ascii_alphabetic() {
                    let mut j = i + 1;
                    while j < c.len() && c[j].is_ascii_alphabetic() {
                        j += 1;
                    }
                    let cmd: String = c[i + 1..j].iter().collect();
                    // Accent commands with a letter name: \c{c}, \v{s}, \H{o}, \u{g}, \r{a}, \k{a}.
                    if cmd.len() == 1 && matches!(cmd.as_str(), "c" | "v" | "H" | "u" | "r" | "k") {
                        let mut k = j;
                        while k < c.len() && c[k] == ' ' {
                            k += 1;
                        }
                        let (base, end) = if k < c.len() && c[k] == '{' {
                            let mut e = k + 1;
                            while e < c.len() && c[e] != '}' {
                                e += 1;
                            }
                            (c.get(k + 1).copied(), (e + 1).min(c.len()))
                        } else {
                            (c.get(k).copied(), (k + 1).min(c.len()))
                        };
                        if let Some(b) = base {
                            out.push(compose(cmd.chars().next().unwrap(), b).unwrap_or(b));
                        }
                        i = end;
                        continue;
                    }
                    // A command that takes a braced argument keeps the argument (\textit{x}, \url{x}, \emph{x}).
                    let mut k = j;
                    if let Some(sp) = special(&cmd) {
                        out.push_str(sp);
                        // `{\o}` and `\o{}` end with an empty group or a space that belongs to the command.
                        if k < c.len() && c[k] == ' ' {
                            k += 1;
                        } else if k + 1 < c.len() && c[k] == '{' && c[k + 1] == '}' {
                            k += 2;
                        }
                        i = k;
                        continue;
                    }
                    // Unknown or formatting command: drop the command name, keep the argument's text.
                    if k < c.len() && c[k] == ' ' && k + 1 < c.len() && c[k + 1] == '{' {
                        k += 1;
                    }
                    i = k;
                } else {
                    i += 2;
                }
            }
            '{' | '}' => i += 1,
            '$' => i += 1,
            '~' => {
                out.push(' ');
                i += 1;
            }
            '-' if c.get(i + 1) == Some(&'-') => {
                if c.get(i + 2) == Some(&'-') {
                    out.push('—');
                    i += 3;
                } else {
                    out.push('–');
                    i += 2;
                }
            }
            '`' if c.get(i + 1) == Some(&'`') => {
                out.push('“');
                i += 2;
            }
            '\'' if c.get(i + 1) == Some(&'\'') => {
                out.push('”');
                i += 2;
            }
            ch => {
                out.push(ch);
                i += 1;
            }
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// A field value (with its outer braces) as plain text.
fn text_of(v: &str) -> String {
    latex_to_text(&split_top(v).join(""))
}

/// Split a name list on top-level " and ".
fn split_names(v: &str) -> Vec<String> {
    let inner = split_top(v).join("");
    let chars: Vec<char> = inner.chars().collect();
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut depth = 0;
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        match c {
            '{' => depth += 1,
            '}' => depth -= 1,
            _ => {}
        }
        if depth == 0 && c.is_whitespace() && i + 4 < chars.len() && chars[i + 1..i + 4].iter().collect::<String>().eq_ignore_ascii_case("and") && chars[i + 4].is_whitespace() {
            out.push(std::mem::take(&mut cur));
            i += 4;
            continue;
        }
        cur.push(c);
        i += 1;
    }
    out.push(cur);
    out.into_iter().map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()
}

fn parse_name(raw: &str) -> Option<Name> {
    let raw = raw.trim();
    if raw.is_empty() || raw.eq_ignore_ascii_case("others") || raw.eq_ignore_ascii_case("et al.") || raw.eq_ignore_ascii_case("et al") {
        return None;
    }
    // `{World Health Organization}`: one braced group and nothing else is an organisation.
    if raw.starts_with('{') && raw.ends_with('}') {
        let groups = split_top(raw);
        if groups.len() == 1 && !groups[0].contains(',') {
            return Some(Name::Literal(latex_to_text(&groups[0])));
        }
    }
    // Commas outside braces separate family from given names.
    let mut depth = 0;
    let mut parts: Vec<String> = vec![String::new()];
    for ch in raw.chars() {
        match ch {
            '{' => depth += 1,
            '}' => depth -= 1,
            ',' if depth == 0 => {
                parts.push(String::new());
                continue;
            }
            _ => {}
        }
        parts.last_mut().unwrap().push(ch);
    }
    let parts: Vec<String> = parts.iter().map(|p| latex_to_text(p)).collect();
    match parts.len() {
        1 => Name::parse(&parts[0]),
        2 => Some(Name::person(&parts[0], &parts[1])),
        _ => {
            // "Smith, Jr., John": the suffix stays with the family name.
            Some(Name::person(&format!("{}, {}", parts[0], parts[1]), &parts[2..].join(" ")))
        }
    }
}

fn names_of(v: &str) -> Vec<Name> {
    split_names(v).iter().filter_map(|n| parse_name(n)).collect()
}

// ---------------------------------------------------------------------------
// Entries to sources
// ---------------------------------------------------------------------------

fn field<'a>(e: &'a Entry, names: &[&str]) -> Option<&'a str> {
    names.iter().find_map(|n| e.fields.iter().find(|(k, v)| k == n && !text_of(v).is_empty()).map(|(_, v)| v.as_str()))
}

fn month_of(v: &str) -> Option<String> {
    let t = text_of(v);
    month_number(&t).map(|n| MONTH_NAMES[n as usize - 1].to_string()).or(Some(t).filter(|t| !t.is_empty()))
}

fn clean_pages(p: &str) -> String {
    p.replace(['–', '—'], "-").replace("--", "-").replace(" - ", "-")
}

fn entry_to_source(e: &Entry) -> Option<Source> {
    let get = |names: &[&str]| field(e, names).map(text_of).filter(|t| !t.is_empty());
    let mut s = Source { title: get(&["title"]).unwrap_or_default(), ..Default::default() };
    if let Some(a) = field(e, &["author"]) {
        s.authors = names_of(a);
    }
    if let Some(a) = field(e, &["editor"]) {
        s.editors = names_of(a);
    }
    if s.authors.is_empty() {
        // Corporate authors are often given as `organization` or `institution` on reports and manuals.
        if let Some(o) = field(e, &["organization", "institution"]).filter(|_| matches!(e.kind.as_str(), "manual" | "techreport" | "report")) {
            s.authors = vec![Name::Literal(text_of(o))];
        }
    }
    s.kind = match e.kind.as_str() {
        "article" | "periodical" | "suppperiodical" => SourceType::Article,
        "book" | "mvbook" | "booklet" | "proceedings" | "mvproceedings" | "collection" | "mvcollection" | "reference" => SourceType::Book,
        "inbook" | "incollection" | "suppbook" | "bookinbook" | "inreference" => SourceType::Chapter,
        "inproceedings" | "conference" => SourceType::Conference,
        "techreport" | "report" | "manual" => SourceType::Report,
        "phdthesis" | "mastersthesis" | "thesis" => SourceType::Thesis,
        "online" | "electronic" | "www" | "webpage" => SourceType::Website,
        "misc" => {
            if field(e, &["url"]).is_some() && field(e, &["publisher", "doi"]).is_none() {
                SourceType::Website
            } else {
                SourceType::Misc
            }
        }
        _ => SourceType::Misc,
    };
    // Classic BibTeX @inbook: `title` is the book and `chapter` the part cited.
    if e.kind == "inbook" && field(e, &["booktitle"]).is_none() {
        match get(&["chapter"]).filter(|c| !c.chars().all(|ch| ch.is_ascii_digit())) {
            Some(chapter) => {
                s.container_title = Some(std::mem::replace(&mut s.title, chapter));
            }
            None => s.kind = SourceType::Book,
        }
    }
    s.container_title = s.container_title.take().or_else(|| match s.kind {
        SourceType::Article => get(&["journal", "journaltitle", "journal title"]),
        SourceType::Chapter | SourceType::Conference => get(&["booktitle", "maintitle", "journal"]),
        SourceType::Website => get(&["website", "journal", "journaltitle", "organization", "site"]),
        SourceType::Misc => get(&["journal", "journaltitle", "howpublished"]).filter(|h| !h.starts_with("\\url") && !h.starts_with("http")),
        _ => None,
    });
    // A date can be one field (`date = {2020-03-05}`) or year, month and day.
    if let Some(d) = get(&["date"]) {
        let mut it = d.split('-');
        s.year = it.next().map(str::trim).filter(|y| !y.is_empty()).map(String::from);
        s.month = it.next().and_then(month_number).map(|n| MONTH_NAMES[n as usize - 1].to_string());
        s.day = it.next().map(|d| d.trim().trim_start_matches('0').to_string()).filter(|d| !d.is_empty());
    }
    if let Some(y) = get(&["year"]) {
        s.year = Some(y);
    }
    if let Some(m) = field(e, &["month"]).and_then(month_of) {
        s.month = Some(m);
    }
    if let Some(d) = get(&["day"]) {
        s.day = Some(d);
    }
    s.publisher = get(&["publisher", "institution", "school"]).or_else(|| if s.kind == SourceType::Conference { get(&["organization"]) } else { None });
    s.place = get(&["address", "location"]);
    s.volume = get(&["volume"]);
    s.issue = get(&["number", "issue"]);
    s.pages = get(&["pages"]).map(|p| clean_pages(&p));
    s.edition = get(&["edition"]);
    s.url = get(&["url"]).or_else(|| get(&["howpublished"]).filter(|h| h.starts_with("http")));
    s.doi = get(&["doi"]);
    // doi.org's BibTeX repeats the DOI as a link.
    if s.doi.is_some() && s.url.as_deref().is_some_and(|u| u.contains("doi.org/")) {
        s.url = None;
    }
    s.accessed = get(&["urldate", "accessed", "lastchecked"]).map(|d| normalize_date(&d));
    s.genre = match e.kind.as_str() {
        "phdthesis" => Some("Doctoral dissertation".to_string()),
        "mastersthesis" => Some("Master's thesis".to_string()),
        "techreport" => get(&["type"]).or_else(|| Some("Technical report".to_string())),
        _ => get(&["type"]),
    };
    if s.kind == SourceType::Report {
        if let (Some(n), Some(g)) = (get(&["number"]), s.genre.clone()) {
            if e.kind == "techreport" {
                s.genre = Some(format!("{g} {n}"));
                s.issue = None;
            }
        }
    }
    if s.title.is_empty() && s.authors.is_empty() && s.editors.is_empty() && s.year.is_none() && s.container_title.is_none() {
        return None;
    }
    // A conference paper's organisation isn't its publisher when a publisher is given too.
    Some(s)
}

/// `2021-3-5`, `2021/03/05`, `March 5, 2021` to `2021-03-05` (as much of it as there is).
fn normalize_date(d: &str) -> String {
    let t = d.trim();
    let nums: Vec<&str> = t.split(['-', '/', '.']).collect();
    if !nums.is_empty() && nums[0].len() == 4 && nums[0].chars().all(|c| c.is_ascii_digit()) {
        let mut out = nums[0].to_string();
        if let Some(m) = nums.get(1).and_then(|m| month_number(m)) {
            out.push_str(&format!("-{m:02}"));
            if let Some(dd) = nums.get(2).and_then(|d| d.trim().parse::<u32>().ok()) {
                out.push_str(&format!("-{dd:02}"));
            }
        }
        return out;
    }
    // "March 5, 2021" or "5 March 2021"
    let words: Vec<String> = t.replace(',', " ").split_whitespace().map(String::from).collect();
    let year = words.iter().find(|w| w.len() == 4 && w.chars().all(|c| c.is_ascii_digit()));
    let month = words.iter().find_map(|w| month_number(w).filter(|_| w.chars().all(|c| c.is_alphabetic() || c == '.')));
    let day = words.iter().find_map(|w| w.parse::<u32>().ok().filter(|d| (1..=31).contains(d) && w.len() <= 2));
    match (year, month, day) {
        (Some(y), Some(m), Some(d)) => format!("{y}-{m:02}-{d:02}"),
        (Some(y), Some(m), None) => format!("{y}-{m:02}"),
        (Some(y), _, _) => y.clone(),
        _ => t.to_string(),
    }
}

/// Read BibTeX (or BibLaTeX) text into sources, with no tags (they are made when the sources are added).
pub fn parse(text: &str) -> Result<Vec<Source>, String> {
    crate::budget::within(text.len(), crate::budget::caps::LIST_BYTES).map_err(|e| e.to_string())?;
    let entries = scan(text);
    if entries.is_empty() {
        return Err("No BibTeX entries found. An entry looks like @article{key, title = {...}, author = {...}}.".into());
    }
    let sources: Vec<Source> = entries.iter().filter_map(entry_to_source).collect();
    if sources.is_empty() {
        return Err("The BibTeX entries have no titles, authors or years to read.".into());
    }
    Ok(sources)
}

// ---------------------------------------------------------------------------
// Writing
// ---------------------------------------------------------------------------

fn escape(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        match c {
            '&' | '%' | '_' | '#' | '$' => {
                out.push('\\');
                out.push(c);
            }
            '–' => out.push_str("--"),
            '—' => out.push_str("---"),
            c => out.push(c),
        }
    }
    out
}

fn name_out(n: &Name) -> String {
    match n {
        Name::Person { family, given } => match given.as_deref().filter(|g| !g.is_empty()) {
            Some(g) => format!("{}, {}", escape(family), escape(g)),
            None => escape(family),
        },
        Name::Literal(l) => format!("{{{}}}", escape(l)),
    }
}

/// A key like `smith2020`, unique among `used`: then `smith2020a` to `smith2020z`, then `smith2020-27` and on.
fn key_for(s: &Source, used: &mut Keys) -> String {
    let base = if !s.tag.is_empty() {
        s.tag.clone()
    } else {
        let fam: String = s.authors.first().map(|a| a.family().to_string()).unwrap_or_else(|| "ref".into());
        let fam: String = fam.chars().filter_map(crate::cite_styles::fold_char).filter(|c| c.is_ascii_alphanumeric()).collect();
        format!("{}{}", if fam.is_empty() { "ref".into() } else { fam }, nz(&s.year).unwrap_or(""))
    };
    let mut key = base.clone();
    // Where the suffixes for this base go on from, so a thousand sources by one author in one year stay quick.
    let n = used.next.entry(base.clone()).or_insert(0);
    while used.taken.contains(&key) {
        key = if *n < 26 { format!("{base}{}", (b'a' + *n as u8) as char) } else { format!("{base}-{}", *n + 1) };
        *n += 1;
    }
    used.taken.insert(key.clone());
    key
}

/// The keys given so far, and for each base the next suffix to try.
#[derive(Default)]
struct Keys {
    taken: std::collections::HashSet<String>,
    next: std::collections::HashMap<String, usize>,
}

/// Sources as BibTeX entries.
pub fn to_bibtex(sources: &[Source]) -> String {
    let mut out = String::new();
    let mut used = Keys::default();
    for s in sources {
        let thesis_kind = if s.genre.as_deref().is_some_and(|g| g.to_lowercase().contains("master")) { "mastersthesis" } else { "phdthesis" };
        let kind = match s.kind {
            SourceType::Article => "article",
            SourceType::Book => "book",
            SourceType::Chapter => "incollection",
            SourceType::Conference => "inproceedings",
            SourceType::Report => "techreport",
            SourceType::Website => "online",
            SourceType::Thesis => thesis_kind,
            SourceType::Misc => "misc",
        };
        let key = key_for(s, &mut used);
        out.push_str(&format!("@{kind}{{{key},\n"));
        fn put(out: &mut String, name: &str, v: String) {
            if !v.is_empty() {
                out.push_str(&format!("  {name} = {{{v}}},\n"));
            }
        }
        put(&mut out, "author", s.authors.iter().map(name_out).collect::<Vec<_>>().join(" and "));
        put(&mut out, "editor", s.editors.iter().map(name_out).collect::<Vec<_>>().join(" and "));
        put(&mut out, "title", if s.title.trim().is_empty() { String::new() } else { format!("{{{}}}", escape(s.title.trim())) });
        let container = nz(&s.container_title).map(escape).unwrap_or_default();
        let cname = match s.kind {
            SourceType::Article => "journal",
            SourceType::Chapter | SourceType::Conference => "booktitle",
            SourceType::Website => "organization",
            SourceType::Misc => "howpublished",
            _ => "",
        };
        if !cname.is_empty() {
            put(&mut out, cname, container);
        }
        put(&mut out, "year", nz(&s.year).map(escape).unwrap_or_default());
        if let Some(m) = nz(&s.month).and_then(month_number) {
            let abbr = MONTH_NAMES[m as usize - 1][..3].to_lowercase();
            out.push_str(&format!("  month = {abbr},\n"));
        }
        let pub_name = match s.kind {
            SourceType::Thesis => "school",
            SourceType::Report => "institution",
            _ => "publisher",
        };
        put(&mut out, pub_name, nz(&s.publisher).map(escape).unwrap_or_default());
        put(&mut out, "address", nz(&s.place).map(escape).unwrap_or_default());
        put(&mut out, "volume", nz(&s.volume).map(escape).unwrap_or_default());
        put(&mut out, "number", nz(&s.issue).map(escape).unwrap_or_default());
        put(&mut out, "pages", nz(&s.pages).map(|p| escape(&p.replace('-', "--").replace("----", "--"))).unwrap_or_default());
        put(&mut out, "edition", nz(&s.edition).map(escape).unwrap_or_default());
        put(&mut out, "url", nz(&s.url).map(String::from).unwrap_or_default());
        put(&mut out, "doi", nz(&s.doi).map(String::from).unwrap_or_default());
        put(&mut out, "urldate", nz(&s.accessed).map(String::from).unwrap_or_default());
        if s.kind == SourceType::Report {
            put(&mut out, "type", nz(&s.genre).map(escape).unwrap_or_default());
        }
        if out.ends_with(",\n") {
            out.truncate(out.len() - 2);
            out.push('\n');
        }
        out.push_str("}\n\n");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn google_scholar_article() {
        let src = r#"@article{smith2020deep,
  title={Deep learning for citation parsing},
  author={Smith, John and Lee, Karen and van der Berg, Anna},
  journal={Journal of Machine Learning},
  volume={12},
  number={3},
  pages={45--67},
  year={2020},
  publisher={Elsevier}
}"#;
        let v = parse(src).unwrap();
        assert_eq!(v.len(), 1);
        let s = &v[0];
        assert_eq!(s.kind, SourceType::Article);
        assert_eq!(s.title, "Deep learning for citation parsing");
        assert_eq!(s.authors, vec![Name::person("Smith", "John"), Name::person("Lee", "Karen"), Name::person("van der Berg", "Anna")]);
        assert_eq!(s.container_title.as_deref(), Some("Journal of Machine Learning"));
        assert_eq!((s.volume.as_deref(), s.issue.as_deref(), s.pages.as_deref(), s.year.as_deref()), (Some("12"), Some("3"), Some("45-67"), Some("2020")));
        assert_eq!(s.publisher.as_deref(), Some("Elsevier"));
    }

    #[test]
    fn zotero_export_with_accents_and_extra_fields() {
        let src = r#"
@book{muller_philosophie_2019,
	address = {Z{\"u}rich},
	title = {{Die} {Philosophie} der {Zitate}: eine {Einf{\"u}hrung}},
	isbn = {978-3-16-148410-0},
	shorttitle = {Philosophie},
	language = {de},
	publisher = {Verlag f{\"u}r Wissenschaft},
	author = {M{\"u}ller, Hans and Garc{\'i}a, Jos{\'e} Mar{\'i}a},
	year = {2019},
	keywords = {citations, philosophy},
	file = {Full Text:/Users/me/Zotero/storage/ABC/muller.pdf:application/pdf},
}

@misc{who_report_2021,
	title = {Global health report},
	url = {https://www.who.int/report},
	author = {{World Health Organization}},
	urldate = {2021-03-05},
	year = {2021},
}
"#;
        let v = parse(src).unwrap();
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].title, "Die Philosophie der Zitate: eine Einführung");
        assert_eq!(v[0].authors, vec![Name::person("Müller", "Hans"), Name::person("García", "José María")]);
        assert_eq!(v[0].place.as_deref(), Some("Zürich"));
        assert_eq!(v[0].publisher.as_deref(), Some("Verlag für Wissenschaft"));
        assert_eq!(v[1].kind, SourceType::Website);
        assert_eq!(v[1].authors, vec![Name::Literal("World Health Organization".into())]);
        assert_eq!(v[1].accessed.as_deref(), Some("2021-03-05"));
    }

    #[test]
    fn chapters_conferences_reports_theses() {
        let src = r#"
@incollection{a, author = "Adams, Ruth", title = "On chapters", booktitle = "Book of Essays", editor = "Baker, Ed and Cole, Fran", pages = "12--34", publisher = "Routledge", address = "London", year = 2019}
@inproceedings{b, author = {Poe, Edgar}, title = {Ravens}, booktitle = {Proc. Gothic}, organization = {ACM}, year = {2018}, month = mar}
@techreport{c, author = {Roe, Rick}, title = {Findings}, institution = {RAND}, number = {TR-12}, year = {2017}}
@phdthesis{d, author = {Turing, Alan}, title = {Computable numbers}, school = {Cambridge}, year = {1938}}
@mastersthesis{e, author = {Doe, Jane}, title = {A thesis}, school = {MIT}, year = {2001}}
@online{f, author = {Rice, Anne}, title = {A page}, url = {https://example.com}, date = {2022-05-09}, urldate = {2023-01-15}, organization = {Citation Weekly}}
"#;
        let v = parse(src).unwrap();
        assert_eq!(v.len(), 6);
        assert_eq!(v[0].kind, SourceType::Chapter);
        assert_eq!(v[0].editors.len(), 2);
        assert_eq!(v[0].year.as_deref(), Some("2019"));
        assert_eq!(v[1].kind, SourceType::Conference);
        assert_eq!(v[1].publisher.as_deref(), Some("ACM"));
        assert_eq!(v[1].month.as_deref(), Some("March"));
        assert_eq!(v[2].kind, SourceType::Report);
        assert_eq!(v[2].genre.as_deref(), Some("Technical report TR-12"));
        assert_eq!(v[3].kind, SourceType::Thesis);
        assert_eq!(v[3].genre.as_deref(), Some("Doctoral dissertation"));
        assert_eq!(v[3].publisher.as_deref(), Some("Cambridge"));
        assert_eq!(v[4].genre.as_deref(), Some("Master's thesis"));
        assert_eq!(v[5].kind, SourceType::Website);
        assert_eq!((v[5].year.as_deref(), v[5].month.as_deref(), v[5].day.as_deref()), (Some("2022"), Some("May"), Some("9")));
        assert_eq!(v[5].container_title.as_deref(), Some("Citation Weekly"));
    }

    #[test]
    fn doi_org_and_mendeley_style_entries() {
        let src = r#"@article{LeCun_2015, title={Deep learning}, volume={521}, ISSN={1476-4687}, url={http://dx.doi.org/10.1038/nature14539}, DOI={10.1038/nature14539}, number={7553}, journal={Nature}, publisher={Springer Science and Business Media LLC}, author={LeCun, Yann and Bengio, Yoshua and Hinton, Geoffrey}, year={2015}, month=may, pages={436–444} }
@article{Bohr1913,
abstract = {On the constitution of atoms \& molecules. We find {\textquotedblleft}x{\textquotedblright}.},
author = {Bohr, N.},
file = {:C$\backslash$:/Users/me/bohr.pdf:pdf},
journal = {Philos. Mag.},
keywords = {atoms,quantum},
mendeley-groups = {Physics},
month = {07},
number = {151},
pages = {1\textendash25},
title = {{On the constitution of atoms and molecules}},
volume = {26},
year = {1913}
}
@inbook{kuhn,
  author = {Kuhn, Thomas},
  title = {The Structure of Scientific Revolutions},
  chapter = {The route to normal science},
  pages = {10-22},
  publisher = {Chicago},
  year = {1962}
}
@inbook{numbered, author = {Roe, Jane}, title = {A Book}, chapter = {3}, pages = {10-22}, year = {1999}}"#;
        let v = parse(src).unwrap();
        assert_eq!(v.len(), 4);
        assert_eq!(v[0].url, None);
        assert_eq!((v[0].doi.as_deref(), v[0].month.as_deref(), v[0].pages.as_deref()), (Some("10.1038/nature14539"), Some("May"), Some("436-444")));
        assert_eq!(v[1].month.as_deref(), Some("July"));
        assert_eq!((v[1].pages.as_deref(), v[1].title.as_str()), (Some("1-25"), "On the constitution of atoms and molecules"));
        assert_eq!((v[2].kind, v[2].title.as_str(), v[2].container_title.as_deref()), (SourceType::Chapter, "The route to normal science", Some("The Structure of Scientific Revolutions")));
        assert_eq!((v[3].kind, v[3].title.as_str()), (SourceType::Book, "A Book"));
    }

    #[test]
    fn latex_text() {
        assert_eq!(latex_to_text(r#"Sch{\"o}n"#), "Schön");
        assert_eq!(latex_to_text(r"Caf\'e and \'{e} and \'{\i}"), "Café and é and í");
        assert_eq!(latex_to_text(r"Fran\c{c}ois \v{S}koda \o rsted {\ss}"), "François Škoda ørsted ß");
        assert_eq!(latex_to_text(r"R\&D 50\% of \$5"), "R&D 50% of $5");
        assert_eq!(latex_to_text(r"pages 1--10 and a---b"), "pages 1–10 and a—b");
        assert_eq!(latex_to_text(r"\textit{Nature} and \emph{more}"), "Nature and more");
        assert_eq!(latex_to_text(r"{COVID}-19 \& $x$"), "COVID-19 & x");
    }

    #[test]
    fn messy_input() {
        // Comments between entries, missing trailing comma, uppercase names, macros, parentheses, unclosed entry.
        let src = r#"% my references
@STRING{jml = "Journal of Machine Learning"}
@Article{k1,
  AUTHOR = "Smith, J. and others",
  TITLE = {A {B}ayesian approach},
  JOURNAL = jml,
  YEAR = 2020,
  MONTH = {Mar.}
}
@comment{ignored stuff}
@book(k2,
  title = "Two " # "parts",
  author = {Lee, Karen},
  year = {2019}
)
@misc{
  title = {No key here},
  year = {2001}
}
@article{k3, title = {Never closed}, year = {1999},
"#;
        let v = parse(src).unwrap();
        assert_eq!(v.len(), 4);
        assert_eq!(v[0].authors, vec![Name::person("Smith", "J.")], "'others' is dropped");
        assert_eq!(v[0].title, "A Bayesian approach");
        assert_eq!(v[0].container_title.as_deref(), Some("Journal of Machine Learning"));
        assert_eq!(v[0].month.as_deref(), Some("March"));
        assert_eq!(v[1].title, "Two parts");
        assert_eq!(v[2].title, "No key here");
        assert_eq!(v[3].year.as_deref(), Some("1999"));
        assert!(parse("just some text").is_err());
        assert!(parse("").is_err());
    }

    #[test]
    fn name_forms() {
        assert_eq!(names_of("{{Smith, Jr., John} and Jane Roe and {Big Corp}}"), vec![Name::person("Smith, Jr.", "John"), Name::person("Roe", "Jane"), Name::Literal("Big Corp".into())]);
        assert_eq!(names_of("{Smith, A AND Lee, B and others}"), vec![Name::person("Smith", "A"), Name::person("Lee", "B")]);
        assert_eq!(names_of("{Ludwig van Beethoven}"), vec![Name::person("van Beethoven", "Ludwig")]);
    }

    #[test]
    fn many_sources_with_the_same_author_and_year_get_different_keys() {
        // a to z first, then numbers: no endless loop after 26 and no overflow after 256.
        let s: Vec<Source> = (0..300).map(|i| Source { authors: vec![Name::person("Smith", "A")], title: format!("T{i}"), year: Some("2020".into()), ..Default::default() }).collect();
        let out = to_bibtex(&s);
        let keys: std::collections::HashSet<&str> = out.lines().filter_map(|l| l.strip_prefix('@')).filter_map(|l| l.split_once('{')).map(|(_, k)| k.trim_end_matches(',')).collect();
        assert_eq!(keys.len(), 300, "{keys:?}");
        assert!(keys.contains("smith2020") && keys.contains("smith2020a") && keys.contains("smith2020z") && keys.contains("smith2020-27"), "{keys:?}");
    }

    #[test]
    fn writes_bibtex_that_reads_back() {
        let s = vec![
            Source {
                tag: "Smi20".into(),
                kind: SourceType::Article,
                authors: vec![Name::person("Smith", "John"), Name::Literal("The Group".into())],
                title: "Rock & roll".into(),
                container_title: Some("Journal_X".into()),
                year: Some("2020".into()),
                month: Some("March".into()),
                volume: Some("12".into()),
                pages: Some("45-67".into()),
                doi: Some("10.1/x".into()),
                ..Default::default()
            },
            Source { kind: SourceType::Thesis, authors: vec![Name::person("Doe", "Jane")], title: "T".into(), year: Some("2001".into()), publisher: Some("MIT".into()), genre: Some("Master's thesis".into()), ..Default::default() },
        ];
        let out = to_bibtex(&s);
        assert!(out.contains("@article{Smi20,"), "{out}");
        assert!(out.contains("pages = {45--67}"), "{out}");
        assert!(out.contains("month = mar,"), "{out}");
        assert!(out.contains("@mastersthesis{doe2001,"), "{out}");
        let back = parse(&out).unwrap();
        assert_eq!(back[0].title, "Rock & roll");
        assert_eq!(back[0].container_title.as_deref(), Some("Journal_X"));
        assert_eq!(back[0].authors, s[0].authors);
        assert_eq!(back[0].pages.as_deref(), Some("45-67"));
        assert_eq!(back[1].genre.as_deref(), Some("Master's thesis"));
    }
}
