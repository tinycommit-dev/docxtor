//! Working out the value of a field from the document. `evaluate` takes a parsed instruction and what the
//! document knows (its properties, the time, the page the field is on, the table it sits in) and gives the new
//! result, with the field's switches applied: `\#` numeric pictures, `\@` date-time pictures and the `\*`
//! formats. Fields it doesn't know (cross-references, sequences, lists, citations) are left to their own code.

use crate::docprops::DocProps;
use crate::field_fmt::{self as fmt, Dt};
use crate::fields::{self, Instr, Replace};
use crate::formula;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// What the page that hosts the engine tells it: the engine has no clock, no file name and no layout of its own.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct FieldInput {
    /// The time now, as the reader's clock shows it: `2026-10-13T09:30:00` (a trailing `Z` is ignored).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub now: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_initials: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_address: Option<String>,
    /// The file's name, with its extension.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_name: Option<String>,
    /// The folder the file is in, when the page knows it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_size: Option<u64>,
    /// How many pages the layout has.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pages: Option<u32>,
}

/// Everything about the document that fields read, gathered once per update.
#[derive(Default)]
pub struct Env {
    pub input: FieldInput,
    pub lang: Option<String>,
    /// `docProps/core.xml` as local name (lower case) to text.
    pub core: HashMap<String, String>,
    /// `docProps/app.xml` as local name (lower case) to text.
    pub app: HashMap<String, String>,
    /// Custom document properties (`docProps/custom.xml`), by lower-case name.
    pub custom: HashMap<String, String>,
    /// Document variables (`w:docVars` in the settings), by lower-case name.
    pub vars: HashMap<String, String>,
    /// Values `SET` fields gave, by lower-case name.
    pub sets: HashMap<String, String>,
    /// The text of each bookmark, by lower-case name (formulas read numbers from them).
    pub bookmarks: HashMap<String, String>,
    pub words: usize,
    pub chars: usize,
    pub props: DocProps,
}

/// What differs from one field to the next: the page it is on and the table cell it is in.
#[derive(Default)]
pub struct Place<'a> {
    /// The page number as the field's section prints it.
    pub page: Option<String>,
    /// The section number (1 for the first).
    pub section: Option<usize>,
    pub table: Option<&'a Grid>,
    /// The (row, column) of the cell holding the field, counted from 0.
    pub cell: Option<(usize, usize)>,
}

/// A table as formulas see it: the numbers and text of its cells.
pub struct Grid {
    pub cells: Vec<Vec<formula::Cell>>,
}

enum Value {
    Text(String),
    Num(f64),
    Date(Dt),
}

struct Ctx<'a> {
    env: &'a Env,
    grid: Option<&'a Grid>,
    at: Option<(usize, usize)>,
}

impl formula::Context for Ctx<'_> {
    fn cell(&self, row: usize, col: usize) -> Option<formula::Cell> {
        self.grid?.cells.get(row)?.get(col).cloned()
    }
    fn position(&self) -> Option<(usize, usize)> {
        self.at
    }
    fn size(&self) -> (usize, usize) {
        self.grid.map(|g| (g.cells.len(), g.cells.iter().map(Vec::len).max().unwrap_or(0))).unwrap_or((0, 0))
    }
    fn name(&self, name: &str) -> Option<f64> {
        let key = name.to_lowercase();
        let text = self.env.sets.get(&key).or_else(|| self.env.bookmarks.get(&key))?;
        fmt::parse_number(text)
    }
}

fn plain(s: impl Into<String>) -> Replace {
    Replace { text: s.into(), ..Default::default() }
}

/// The time the page gave, if it did.
fn now(env: &Env) -> Option<Dt> {
    env.input.now.as_deref().and_then(Dt::parse_iso)
}

fn lang(env: &Env) -> Option<&str> {
    env.lang.as_deref()
}

fn date_default(env: &Env) -> String {
    format!("{} {}", fmt::default_date_picture(lang(env)), fmt::default_time_picture(lang(env)))
}

/// A number written in a field: decimal, or `0x` followed by hex digits.
fn parse_code(s: &str) -> Option<u32> {
    let t = s.trim();
    if let Some(h) = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
        return u32::from_str_radix(h, 16).ok();
    }
    t.parse().ok()
}

/// Compare two operands the way `IF` and `COMPARE` do: as numbers when both are, otherwise as text without
/// case, where `=` and `<>` understand `?` and `*`.
fn compare(a: &str, op: &str, b: &str) -> Option<bool> {
    if let (Some(x), Some(y)) = (fmt::parse_number(a), fmt::parse_number(b)) {
        let eps = 1e-9 * x.abs().max(y.abs()).max(1.0);
        return Some(match op {
            "=" => (x - y).abs() <= eps,
            "<>" => (x - y).abs() > eps,
            "<" => x < y,
            ">" => x > y,
            "<=" => x <= y + eps,
            ">=" => x >= y - eps,
            _ => return None,
        });
    }
    let (x, y) = (a.to_lowercase(), b.to_lowercase());
    Some(match op {
        "=" => wildcard(&y, &x),
        "<>" => !wildcard(&y, &x),
        "<" => x < y,
        ">" => x > y,
        "<=" => x <= y,
        ">=" => x >= y,
        _ => return None,
    })
}

/// `?` matches one character, `*` any run. Linear in the text per star (remember the last star, retry one
/// character further), so a pattern full of stars cannot blow up.
fn wildcard(pattern: &str, text: &str) -> bool {
    let p: Vec<char> = pattern.chars().collect();
    let t: Vec<char> = text.chars().collect();
    let (mut pi, mut ti) = (0usize, 0usize);
    let mut star: Option<(usize, usize)> = None;
    while ti < t.len() {
        if pi < p.len() && p[pi] == '*' {
            star = Some((pi, ti));
            pi += 1;
        } else if pi < p.len() && (p[pi] == '?' || p[pi] == t[ti]) {
            pi += 1;
            ti += 1;
        } else if let Some((sp, st)) = star {
            pi = sp + 1;
            ti = st + 1;
            star = Some((sp, st + 1));
        } else {
            return false;
        }
    }
    p[pi..].iter().all(|c| *c == '*')
}

fn prop_value(env: &Env, name: &str) -> Option<Value> {
    let key = name.trim().to_lowercase().replace([' ', '_'], "");
    let core = |k: &str| env.core.get(k).cloned();
    let app = |k: &str| env.app.get(k).cloned();
    let date = |s: Option<String>| s.and_then(|s| Dt::parse_iso(&s)).map(Value::Date);
    let text = |s: Option<String>| s.map(Value::Text);
    let found = match key.as_str() {
        "title" => text(core("title")),
        "subject" => text(core("subject")),
        "author" => text(core("creator")),
        "keywords" => text(core("keywords")),
        "comments" => text(core("description")),
        "category" => text(core("category")),
        "lastsavedby" | "lastauthor" => text(core("lastmodifiedby")),
        "revisionnumber" => text(core("revision")),
        "contentstatus" | "status" => text(core("contentstatus")),
        "creationdate" | "createtime" => date(core("created")),
        "lastsavedtime" | "savetime" => date(core("modified")),
        "lastprinted" | "lastprinteddate" => date(core("lastprinted")),
        "company" => text(app("company")),
        "manager" => text(app("manager")),
        "template" => text(app("template")),
        "application" | "nameofapplication" => text(app("application")),
        "hyperlinkbase" => text(app("hyperlinkbase")),
        "totaleditingtime" | "edittime" => text(app("totaltime")),
        "numberofpages" | "pages" => Some(Value::Num(env.input.pages.map(f64::from).or_else(|| app("pages").and_then(|v| v.parse().ok()))?)),
        "numberofwords" | "words" => Some(Value::Num(env.words as f64)),
        "numberofcharacters" | "characters" => Some(Value::Num(env.chars as f64)),
        "numberofcharacterswithspaces" | "characterswithspaces" => text(app("characterswithspaces")),
        "numberoflines" | "lines" => text(app("lines")),
        "numberofparagraphs" | "paragraphs" => text(app("paragraphs")),
        _ => None,
    };
    found.or_else(|| {
        let wanted = name.trim().to_lowercase();
        env.custom.get(&wanted).cloned().map(Value::Text)
    })
}

/// The value of a field, before its switches. None for fields this module doesn't compute.
fn value_of(i: &Instr, instr: &str, env: &Env, place: &Place) -> Option<Value> {
    let arg = || i.first_arg().map(fields::unescape);
    Some(match i.code.as_str() {
        "DATE" | "TIME" => Value::Date(now(env)?),
        "CREATEDATE" => Value::Date(Dt::parse_iso(env.core.get("created")?)?),
        "SAVEDATE" => Value::Date(Dt::parse_iso(env.core.get("modified")?)?),
        "PRINTDATE" => Value::Date(Dt::parse_iso(env.core.get("lastprinted")?)?),
        "EDITTIME" => Value::Num(env.app.get("totaltime")?.parse().ok()?),
        "REVNUM" => Value::Text(env.core.get("revision")?.clone()),
        "AUTHOR" => Value::Text(arg().or_else(|| env.core.get("creator").cloned()).unwrap_or_default()),
        "LASTSAVEDBY" => Value::Text(env.core.get("lastmodifiedby").cloned().unwrap_or_default()),
        "TITLE" => Value::Text(arg().or_else(|| env.core.get("title").cloned()).unwrap_or_default()),
        "SUBJECT" => Value::Text(arg().or_else(|| env.core.get("subject").cloned()).unwrap_or_default()),
        "KEYWORDS" => Value::Text(arg().or_else(|| env.core.get("keywords").cloned()).unwrap_or_default()),
        "COMMENTS" => Value::Text(arg().or_else(|| env.core.get("description").cloned()).unwrap_or_default()),
        "FILENAME" => {
            let name = env.input.file_name.clone()?;
            if i.has("p") {
                match env.input.file_path.as_deref().filter(|p| !p.is_empty()) {
                    Some(p) => Value::Text(format!("{}/{name}", p.trim_end_matches(['/', '\\']))),
                    None => Value::Text(name),
                }
            } else {
                Value::Text(name)
            }
        }
        "FILESIZE" => {
            let bytes = env.input.file_size? as f64;
            Value::Num(if i.has("m") {
                (bytes / 1_048_576.0).round()
            } else if i.has("k") {
                (bytes / 1024.0).round()
            } else {
                bytes
            })
        }
        "NUMWORDS" => Value::Num(env.words as f64),
        "NUMCHARS" => Value::Num(env.chars as f64),
        "NUMPAGES" => Value::Num(f64::from(env.input.pages?)),
        "PAGE" => Value::Text(place.page.clone()?),
        "SECTION" => Value::Num(place.section? as f64),
        "DOCPROPERTY" => prop_value(env, &arg()?)?,
        "DOCVARIABLE" => {
            let name = arg()?;
            match env.vars.get(&name.to_lowercase()) {
                Some(v) => Value::Text(v.clone()),
                None => Value::Text("Error! No document variable supplied.".into()),
            }
        }
        "USERNAME" => Value::Text(arg().or_else(|| env.input.user_name.clone()).unwrap_or_default()),
        "USERINITIALS" => Value::Text(arg().or_else(|| env.input.user_initials.clone()).unwrap_or_default()),
        "USERADDRESS" => Value::Text(arg().or_else(|| env.input.user_address.clone()).unwrap_or_default()),
        "TEMPLATE" => Value::Text(env.app.get("template").cloned().filter(|t| !t.is_empty()).unwrap_or_else(|| "Normal.dotm".into())),
        "QUOTE" => Value::Text(i.toks.iter().filter(|t| !t.is_switch()).map(|t| fields::unescape(&t.text)).collect::<Vec<_>>().join(" ")),
        "SET" | "ADVANCE" => Value::Text(String::new()),
        "REF" => {
            // A reference to a `SET` variable. Bookmarks are the cross-reference code's.
            let name = arg()?.to_lowercase();
            Value::Text(env.sets.get(&name)?.clone())
        }
        "IF" => {
            let words: Vec<&fields::Tok> = i.toks.iter().filter(|t| !t.is_switch()).collect();
            if words.len() < 4 {
                return None;
            }
            let yes = compare(&words[0].text, &words[1].text, &words[2].text)?;
            let pick = if yes { words.get(3) } else { words.get(4) };
            Value::Text(pick.map(|t| fields::unescape(&t.text)).unwrap_or_default())
        }
        "COMPARE" => {
            let words: Vec<&fields::Tok> = i.toks.iter().filter(|t| !t.is_switch()).collect();
            if words.len() < 3 {
                return None;
            }
            Value::Num(f64::from(u8::from(compare(&words[0].text, &words[1].text, &words[2].text)?)))
        }
        "=" => {
            let expr = fields::formula_expr(instr)?;
            let ctx = Ctx { env, grid: place.table, at: place.cell };
            match formula::evaluate(&expr, &ctx) {
                Ok(n) => Value::Num(n),
                Err(e) => Value::Text(format!("Error! {e}")),
            }
        }
        _ => return None,
    })
}

/// A field's new result, or None when this module doesn't compute that field (or lacks what it needs).
pub fn evaluate(instr: &str, env: &Env, place: &Place) -> Option<Replace> {
    let i = Instr::parse(instr);
    if i.code == "SYMBOL" {
        return symbol(&i);
    }
    let value = value_of(&i, instr, env, place)?;
    Some(plain(finish(&i, value, env)))
}

/// A `SYMBOL` field: the character with the given code, in the font and size the switches give.
fn symbol(i: &Instr) -> Option<Replace> {
    let code = parse_code(i.first_arg()?)?;
    let font = i.value("f");
    let size = i.value("s").and_then(|s| s.trim().parse::<f64>().ok()).map(|pt| (pt * 2.0).round() as u32);
    let unicode = i.has("u");
    let ch = match font.as_deref() {
        // The symbol fonts keep their characters in the private use area, as Word writes them.
        Some(f) if !unicode && crate::symbol_fonts::is_symbol_font(f) && code <= 0xFF => char::from_u32(0xF000 + code)?,
        _ => char::from_u32(code)?,
    };
    Some(Replace { text: ch.to_string(), rstyle: None, font, size })
}

/// Apply `\#`, `\@` and the `\*` formats.
fn finish(i: &Instr, v: Value, env: &Env) -> String {
    let number = match &v {
        Value::Num(n) => Some(*n),
        Value::Text(t) => fmt::parse_number(t),
        Value::Date(_) => None,
    };
    let mut text = match &v {
        Value::Date(dt) => {
            let default = if i.code == "DATE" {
                fmt::default_date_picture(lang(env)).to_string()
            } else if i.code == "TIME" {
                fmt::default_time_picture(lang(env)).to_string()
            } else {
                date_default(env)
            };
            fmt::date_picture(dt, &i.value("@").unwrap_or(default), lang(env))
        }
        Value::Num(n) => match i.value("#") {
            Some(p) => fmt::numeric_picture(*n, &p),
            None => fmt::general_number(*n),
        },
        Value::Text(t) => match (i.value("#"), number) {
            (Some(p), Some(n)) => fmt::numeric_picture(n, &p),
            _ => t.clone(),
        },
    };
    for f in i.formats() {
        if let Some(c) = fmt::apply_case(&text, &f) {
            text = c;
        } else if let Some(n) = number {
            if let Some(s) = fmt::number_format(n, &f) {
                text = s;
            }
        }
    }
    text
}

/// A field's switches (`\#`, `\@` and the `\*` formats) applied to a value the layout worked out, such as a page
/// number: `apply_switches(" PAGE \\* ROMAN ", "4")` is `IV`. A value that isn't a number keeps its text.
pub fn apply_switches(instr: &str, value: &str) -> String {
    let i = Instr::parse(instr);
    finish(&i, Value::Text(value.to_string()), &Env::default())
}

/// A short preview of a field for the Insert field dialog: the result it would have now, or None.
pub fn preview(instr: &str, env: &Env, place: &Place) -> Option<String> {
    evaluate(instr, env, place).map(|r| r.text)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env() -> Env {
        let mut e = Env::default();
        e.input.now = Some("2026-10-13T09:05:00".into());
        e.input.pages = Some(12);
        e.input.file_name = Some("Report.docx".into());
        e.input.file_size = Some(20_480);
        e.core.insert("creator".into(), "Ada Lovelace".into());
        e.core.insert("title".into(), "Notes on the engine".into());
        e.core.insert("created".into(), "2020-02-29T10:00:00Z".into());
        e.app.insert("company".into(), "Analytical".into());
        e.custom.insert("project".into(), "Babbage".into());
        e.vars.insert("colour".into(), "teal".into());
        e.sets.insert("who".into(), "Ada".into());
        e.words = 1234;
        e
    }

    #[test]
    fn crafted_wildcards_finish_fast() {
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let text = "a".repeat(40);
            let pat = format!("{}b", "*a".repeat(19));
            let r1 = wildcard(&pat, &text);
            let r2 = wildcard("*a*a*", "xxaxxaxx");
            let r3 = wildcard("a?c*", "ABC and more".to_lowercase().as_str());
            let _ = tx.send((r1, r2, r3));
        });
        let got = rx.recv_timeout(std::time::Duration::from_secs(5)).expect("wildcard hung");
        assert_eq!(got, (false, true, true));
    }

    fn eval(instr: &str) -> String {
        evaluate(instr, &env(), &Place::default()).map(|r| r.text).unwrap_or_else(|| "<none>".into())
    }

    #[test]
    fn dates_and_times() {
        assert_eq!(eval(r#" DATE \@ "dddd, MMMM d, yyyy" "#), "Tuesday, October 13, 2026");
        assert_eq!(eval(r#" TIME \@ "h:mm am/pm" "#), "9:05 am");
        assert_eq!(eval(r#" DATE \@ "yyyy-MM-dd" "#), "2026-10-13");
        assert_eq!(eval(r#" CREATEDATE \@ "d MMMM yyyy" "#), "29 February 2020");
        assert_eq!(eval(" DATE "), "10/13/2026");
    }

    #[test]
    fn properties() {
        assert_eq!(eval(" AUTHOR "), "Ada Lovelace");
        assert_eq!(eval(r#" AUTHOR \* Upper "#), "ADA LOVELACE");
        assert_eq!(eval(" DOCPROPERTY Company "), "Analytical");
        assert_eq!(eval(" DOCPROPERTY Project "), "Babbage");
        assert_eq!(eval(r#" DOCPROPERTY "Title" "#), "Notes on the engine");
        assert_eq!(eval(" DOCVARIABLE colour "), "teal");
        assert_eq!(eval(" USERNAME "), "");
        assert_eq!(eval(" FILENAME "), "Report.docx");
        assert_eq!(eval(r#" FILESIZE \k "#), "20");
        assert_eq!(eval(" NUMWORDS "), "1234");
        assert_eq!(eval(r##" NUMWORDS \# "#,##0" "##), "1,234");
        assert_eq!(eval(r" NUMPAGES \* roman "), "xii");
        assert_eq!(eval(r" NUMPAGES \* CardText \* Caps "), "Twelve");
        assert_eq!(eval(" SEQ Figure "), "<none>");
    }

    #[test]
    fn logic_and_text() {
        assert_eq!(eval(r#" IF 3 > 2 "yes" "no" "#), "yes");
        assert_eq!(eval(r#" IF "Apple" = "app*" "yes" "no" "#), "yes");
        assert_eq!(eval(r#" IF 1 = 2 "yes" "#), "");
        assert_eq!(eval(r#" COMPARE 5 <= 4 "#), "0");
        assert_eq!(eval(r#" QUOTE "Hello" "#), "Hello");
        assert_eq!(eval(" REF who "), "Ada");
        assert_eq!(eval(" REF nobody "), "<none>");
    }

    #[test]
    fn formulas() {
        assert_eq!(eval(r#" = 2 + 3 * 4 "#), "14");
        assert_eq!(eval(r#"=10/4 \# "0.00" "#), "2.50");
        assert_eq!(eval(" = 1/0 "), "Error! Division by zero");
    }

    #[test]
    fn symbols() {
        let r = evaluate(r#" SYMBOL 169 \f "Wingdings" \s 14 "#, &env(), &Place::default()).unwrap();
        assert_eq!(r.text, "\u{F0A9}");
        assert_eq!(r.font.as_deref(), Some("Wingdings"));
        assert_eq!(r.size, Some(28));
        assert_eq!(evaluate(" SYMBOL 0x41 ", &env(), &Place::default()).unwrap().text, "A");
    }

    #[test]
    fn switches_on_a_value_from_the_layout() {
        assert_eq!(apply_switches(r" PAGE \* ROMAN ", "4"), "IV");
        assert_eq!(apply_switches(r" NUMPAGES \* CardText \* Caps ", "3"), "Three");
        assert_eq!(apply_switches(r#" NUMPAGES \# "000" "#, "3"), "003");
        assert_eq!(apply_switches(" PAGE ", "iv"), "iv");
        assert_eq!(apply_switches(r" PAGE \* MERGEFORMAT ", "7"), "7");
    }

    #[test]
    fn page_values() {
        let place = Place { page: Some("iv".into()), section: Some(2), ..Default::default() };
        assert_eq!(evaluate(" PAGE ", &env(), &place).unwrap().text, "iv");
        assert_eq!(evaluate(" SECTION ", &env(), &place).unwrap().text, "2");
        assert!(evaluate(" PAGE ", &env(), &Place::default()).is_none());
    }
}
