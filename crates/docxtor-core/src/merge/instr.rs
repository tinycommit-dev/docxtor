//! The merge fields as Word writes them, and how to make them. They are ordinary complex fields: `MERGEFIELD`
//! (a column), `ADDRESSBLOCK`, `GREETINGLINE`, and the rules `IF`, `ASK`, `FILLIN`, `NEXT`, `NEXTIF`, `SKIPIF`,
//! `MERGEREC`, `MERGESEQ` and `SET`. Each shows a placeholder such as «First_Name» as its result until the merge
//! gives it a value.

use crate::fields::{self, Instr};
use crate::model::*;
use crate::xml::Element;
use serde::{Deserialize, Serialize};

/// The fields that take a value from the recipient or move through the records.
pub fn is_merge_code(code: &str) -> bool {
    matches!(code, "MERGEFIELD" | "ADDRESSBLOCK" | "GREETINGLINE" | "NEXT" | "NEXTIF" | "SKIPIF" | "MERGEREC" | "MERGESEQ" | "ASK" | "FILLIN" | "SET")
}

/// Fields a merge works out: the ones above, and `IF` and `REF` (which read what the rules set).
pub fn is_merge_instr(instr: &str) -> bool {
    let code = Instr::parse(instr).code;
    is_merge_code(&code) || code == "IF" || code == "COMPARE"
}

/// A merge field or rule to insert (Insert Merge Field, Address Block, Greeting Line and Rules).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MergeFieldSpec {
    /// A column of the recipients. `before` and `after` are only written when the value isn't empty; `case` is
    /// `upper`, `lower`, `first_cap` or `caps`; `picture` is a `\#` or `\@` picture.
    Field {
        name: String,
        #[serde(default)]
        before: Option<String>,
        #[serde(default)]
        after: Option<String>,
        #[serde(default)]
        case: Option<String>,
        #[serde(default)]
        picture: Option<String>,
    },
    /// The recipient's name and postal address.
    AddressBlock {
        /// How the name reads: `first_last` (Joshua Randall), `title_first_last` (Mr. Joshua Randall), `title_last`,
        /// `first_middle_last` or `title_first_middle_last`. Default `first_last`.
        #[serde(default)]
        name: Option<String>,
        /// Put the company name on its own line.
        #[serde(default)]
        company: bool,
        /// `never`, `always` or `different` (only when the country isn't `except`). Default `never`.
        #[serde(default)]
        country: Option<String>,
        #[serde(default)]
        except: Option<String>,
        /// A full format string instead of `name` and `company` (lines separated by ¶, fields in angle brackets).
        #[serde(default)]
        format: Option<String>,
    },
    /// "Dear Mr. Randall,".
    GreetingLine {
        /// The words before the name, default `Dear `.
        #[serde(default)]
        salutation: Option<String>,
        /// `title_last` (Mr. Randall, the default), `first` (Joshua), `first_last`, `title_first_last`.
        #[serde(default)]
        name: Option<String>,
        /// What ends the line: `,` (the default), `:` or nothing.
        #[serde(default)]
        punctuation: Option<String>,
        /// What to write when the recipient has no name: `Dear Sir or Madam,` by default.
        #[serde(default)]
        invalid: Option<String>,
    },
    /// IF a column compares to a value, insert one text, otherwise another.
    If {
        field: String,
        /// `eq`, `ne`, `lt`, `gt`, `le`, `ge`, `empty` or `not_empty`.
        op: String,
        #[serde(default)]
        value: String,
        then: String,
        #[serde(default)]
        otherwise: String,
    },
    /// Ask a question when the merge starts and keep the answer under a bookmark name.
    Ask {
        bookmark: String,
        prompt: String,
        #[serde(default)]
        default: String,
        /// Ask once for the whole merge rather than for every record.
        #[serde(default)]
        once: bool,
    },
    /// Ask a question and insert the answer.
    FillIn {
        prompt: String,
        #[serde(default)]
        default: String,
        #[serde(default)]
        once: bool,
    },
    /// Go on to the next record in the same document (labels, directories).
    Next,
    /// Go on to the next record when the condition holds.
    NextIf { field: String, op: String, #[serde(default)] value: String },
    /// Leave this record out of the merge when the condition holds.
    SkipIf { field: String, op: String, #[serde(default)] value: String },
    /// The record's number in the list.
    MergeRec,
    /// How many records have been merged so far.
    MergeSeq,
    /// Give a bookmark a value for the rest of the merge.
    Set { bookmark: String, value: String },
}

fn hidden(el: Element) -> Inline {
    Inline::Run(Run { attrs: vec![], t_attrs: None, rpr: None, items: vec![RunItem::Hidden(el)] })
}

fn fld_char(kind: &str) -> Inline {
    hidden(Element::new("w:fldChar").with_attr("w:fldCharType", kind))
}

fn instr_run(text: &str) -> Inline {
    hidden(Element::new("w:instrText").with_attr("xml:space", "preserve").with_text(text))
}

fn text_run(text: &str, rpr: Option<Element>) -> Inline {
    Inline::Run(Run::of_text(rpr, text))
}

/// The name as `MERGEFIELD` writes it: spaces become underscores.
pub fn field_word(name: &str) -> String {
    let n = name.trim().replace(' ', "_");
    if n.chars().any(|c| c.is_whitespace() || c == '"' || c == '\\') {
        format!("\"{}\"", fields::escape(&n, false))
    } else {
        n
    }
}

/// The placeholder a field shows until the merge fills it: «First_Name».
pub fn placeholder(name: &str) -> String {
    format!("«{}»", name.trim().replace(' ', "_"))
}

fn q(s: &str) -> String {
    format!("\"{}\"", fields::escape(s, false))
}

fn compare_words(op: &str) -> Result<(&'static str, bool), String> {
    // The symbol, and whether the right side is the empty text whatever `value` says.
    Ok(match op {
        "eq" => ("=", false),
        "ne" => ("<>", false),
        "lt" => ("<", false),
        "gt" => (">", false),
        "le" => ("<=", false),
        "ge" => (">=", false),
        "empty" => ("=", true),
        "not_empty" => ("<>", true),
        other => return Err(format!("unknown comparison '{other}'")),
    })
}

/// A field whose code has a merge field in it: `IF "{ MERGEFIELD x }" = "a" ...`. `head` is the code before the
/// nested field (`IF "`), `tail` what follows it.
fn nested(head: &str, name: &str, tail: &str, shown: &str, rpr: Option<&Element>) -> Vec<Inline> {
    let mut out = vec![fld_char("begin"), instr_run(head)];
    out.extend(fields::complex(&format!(" MERGEFIELD {} ", field_word(name)), &placeholder(name), Some(fields::no_proof()), false));
    out.push(instr_run(tail));
    out.push(fld_char("separate"));
    if !shown.is_empty() {
        out.push(text_run(shown, rpr.cloned()));
    }
    out.push(fld_char("end"));
    out
}

fn case_switch(case: &str) -> Result<&'static str, String> {
    Ok(match case {
        "upper" => "Upper",
        "lower" => "Lower",
        "first_cap" => "FirstCap",
        "caps" => "Caps",
        other => return Err(format!("unknown case '{other}'; use upper, lower, first_cap or caps")),
    })
}

const NAME_FORMATS: [(&str, &str, &str); 5] = [
    ("first_last", "<First Name> <Last Name> <Suffix>", "Joshua Randall Jr."),
    ("title_first_last", "<Courtesy Title> <First Name> <Last Name> <Suffix>", "Mr. Joshua Randall Jr."),
    ("title_last", "<Courtesy Title> <Last Name> <Suffix>", "Mr. Randall Jr."),
    ("first_middle_last", "<First Name> <Middle Name> <Last Name> <Suffix>", "Joshua Q. Randall Jr."),
    ("title_first_middle_last", "<Courtesy Title> <First Name> <Middle Name> <Last Name> <Suffix>", "Mr. Joshua Q. Randall Jr."),
];

/// The names of the formats for the recipient's name in an address block, with an example of each.
pub fn address_name_formats() -> Vec<(&'static str, &'static str)> {
    NAME_FORMATS.iter().map(|(id, _, example)| (*id, *example)).collect()
}

/// Word's separator between the lines of an address format.
pub const LINE_MARK: char = '¶';

/// The format string of an address block: the name, the company, the street, the city line.
pub fn address_format(name: Option<&str>, company: bool) -> Result<String, String> {
    let id = name.unwrap_or("first_last");
    let name_part = NAME_FORMATS.iter().find(|(n, _, _)| *n == id).map(|(_, f, _)| *f).ok_or_else(|| format!("unknown name format '{id}'"))?;
    let mut lines = vec![name_part.to_string()];
    if company {
        lines.push("<Company>".into());
    }
    lines.extend(["<Address 1>".to_string(), "<Address 2>".into(), "<City>, <State> <Postal Code>".into()]);
    Ok(lines.join(&LINE_MARK.to_string()))
}

fn greeting_format(salutation: &str, name: &str, punctuation: &str) -> Result<String, String> {
    let names = match name {
        "title_last" => "<< _TITLE0_>><< _LAST0_>>",
        "first" => "<< _FIRST0_>>",
        "first_last" => "<< _FIRST0_>><< _LAST0_>>",
        "title_first_last" => "<< _TITLE0_>><< _FIRST0_>><< _LAST0_>>",
        other => return Err(format!("unknown name format '{other}'")),
    };
    Ok(format!("<< _BEFORE_ {salutation}>>{names}<< _AFTER_ {punctuation}>>"))
}

/// What a merge field or rule is, as the runs of a complex field. `rpr` is the formatting at the place it goes in.
pub fn build(spec: &MergeFieldSpec, rpr: Option<Element>) -> Result<Vec<Inline>, String> {
    let np = rpr.clone().or_else(|| Some(fields::no_proof()));
    let one = |instr: String, shown: &str| fields::complex(&instr, shown, np.clone(), false);
    Ok(match spec {
        MergeFieldSpec::Field { name, before, after, case, picture } => {
            if name.trim().is_empty() {
                return Err("Say which column the field takes its value from.".into());
            }
            let mut instr = format!(" MERGEFIELD {}", field_word(name));
            if let Some(c) = case.as_deref().filter(|c| !c.is_empty()) {
                instr.push_str(&format!(" \\* {}", case_switch(c)?));
            }
            if let Some(p) = picture.as_deref().filter(|p| !p.trim().is_empty()) {
                let sw = if p.chars().any(|c| matches!(c, 'y' | 'M' | 'd' | 'h' | 'H' | 's' | 'a' | 'A')) { "\\@" } else { "\\#" };
                instr.push_str(&format!(" {sw} {}", q(p)));
            }
            if let Some(b) = before.as_deref().filter(|b| !b.is_empty()) {
                instr.push_str(&format!(" \\b {}", q(b)));
            }
            if let Some(a) = after.as_deref().filter(|a| !a.is_empty()) {
                instr.push_str(&format!(" \\f {}", q(a)));
            }
            instr.push(' ');
            fields::complex(&instr, &placeholder(name), np, false)
        }
        MergeFieldSpec::AddressBlock { name, company, country, except, format } => {
            let fmt = match format.as_deref().filter(|f| !f.trim().is_empty()) {
                Some(f) => f.to_string(),
                None => address_format(name.as_deref(), *company)?,
            };
            let mode = match country.as_deref().unwrap_or("never") {
                "never" => 0,
                "always" => 1,
                "different" => 2,
                other => return Err(format!("unknown country setting '{other}'; use never, always or different")),
            };
            let mut instr = format!(" ADDRESSBLOCK \\f {} \\l 1033 \\c {mode}", q(&fmt));
            if mode == 2 {
                if let Some(e) = except.as_deref().filter(|e| !e.trim().is_empty()) {
                    instr.push_str(&format!(" \\e {}", q(e)));
                }
            }
            instr.push_str(" \\d ");
            one(instr, "«AddressBlock»")
        }
        MergeFieldSpec::GreetingLine { salutation, name, punctuation, invalid } => {
            let sal = salutation.as_deref().unwrap_or("Dear ");
            let fmt = greeting_format(sal, name.as_deref().unwrap_or("title_last"), punctuation.as_deref().unwrap_or(","))?;
            let bad = invalid.as_deref().unwrap_or("Dear Sir or Madam,");
            one(format!(" GREETINGLINE \\f {} \\l 1033 \\m \\e {} ", q(&fmt), q(bad)), "«GreetingLine»")
        }
        MergeFieldSpec::If { field, op, value, then, otherwise } => {
            if field.trim().is_empty() {
                return Err("Say which column to compare.".into());
            }
            let (sym, blank) = compare_words(op)?;
            let right = if blank { "" } else { value.as_str() };
            let tail = format!("\" {sym} {} {} {} ", q(right), q(then), q(otherwise));
            nested(" IF \"", field, &tail, otherwise, np.as_ref())
        }
        MergeFieldSpec::Ask { bookmark, prompt, default, once } => {
            let bm = bookmark.trim().replace(' ', "_");
            if bm.is_empty() {
                return Err("Give the bookmark a name.".into());
            }
            let mut instr = format!(" ASK {bm} {}", q(prompt));
            if !default.is_empty() {
                instr.push_str(&format!(" \\d {}", q(default)));
            }
            if *once {
                instr.push_str(" \\o");
            }
            instr.push(' ');
            one(instr, &format!("«Ask_{bm}»"))
        }
        MergeFieldSpec::FillIn { prompt, default, once } => {
            let mut instr = format!(" FILLIN {}", q(prompt));
            if !default.is_empty() {
                instr.push_str(&format!(" \\d {}", q(default)));
            }
            if *once {
                instr.push_str(" \\o");
            }
            instr.push(' ');
            one(instr, if default.is_empty() { "«Fill_in»" } else { default.as_str() })
        }
        MergeFieldSpec::Next => one(" NEXT ".into(), "«Next_Record»"),
        MergeFieldSpec::NextIf { field, op, value } | MergeFieldSpec::SkipIf { field, op, value } => {
            let code = if matches!(spec, MergeFieldSpec::NextIf { .. }) { "NEXTIF" } else { "SKIPIF" };
            let (sym, blank) = compare_words(op)?;
            let right = if blank { "" } else { value.as_str() };
            let tail = format!("\" {sym} {} ", q(right));
            let shown = if code == "NEXTIF" { "«Next_Record_If»" } else { "«Skip_Record_If»" };
            nested(&format!(" {code} \""), field, &tail, shown, np.as_ref())
        }
        MergeFieldSpec::MergeRec => one(" MERGEREC ".into(), "1"),
        MergeFieldSpec::MergeSeq => one(" MERGESEQ ".into(), "1"),
        MergeFieldSpec::Set { bookmark, value } => {
            let bm = bookmark.trim().replace(' ', "_");
            if bm.is_empty() {
                return Err("Give the bookmark a name.".into());
            }
            one(format!(" SET {bm} {} ", q(value)), &format!("«Set_{bm}»"))
        }
    })
}

/// The code a field would be written with (all the instruction text, a nested field's included), for a dialog to
/// show and preview.
pub fn instruction_of(spec: &MergeFieldSpec) -> Result<String, String> {
    let inlines = build(spec, None)?;
    let mut out = String::new();
    for i in &inlines {
        if let Inline::Run(r) = i {
            for it in &r.items {
                if let RunItem::Hidden(e) = it {
                    if e.name == "w:instrText" {
                        out.push_str(&e.text());
                    }
                }
            }
        }
    }
    Ok(out)
}

/// The column a `MERGEFIELD` field takes its value from.
pub fn field_name(instr: &Instr) -> Option<String> {
    instr.first_arg().map(fields::unescape).filter(|n| !n.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn instr_of(inlines: &[Inline]) -> String {
        let mut out = String::new();
        let mut depth = 0;
        for i in inlines {
            if let Inline::Run(r) = i {
                for it in &r.items {
                    if let RunItem::Hidden(e) = it {
                        match (e.name.as_str(), e.attr("w:fldCharType")) {
                            ("w:fldChar", Some("begin")) => depth += 1,
                            ("w:fldChar", Some("end")) => depth -= 1,
                            ("w:instrText", _) => out.push_str(&e.text()),
                            _ => {}
                        }
                    }
                }
            }
        }
        assert_eq!(depth, 0);
        out
    }

    #[test]
    fn writes_merge_fields_as_word_does() {
        let f = build(&MergeFieldSpec::Field { name: "First Name".into(), before: Some("Dear ".into()), after: Some(",".into()), case: Some("upper".into()), picture: None }, None).unwrap();
        assert_eq!(instr_of(&f), r#" MERGEFIELD First_Name \* Upper \b "Dear " \f "," "#);
        let i = Instr::parse(&instr_of(&f));
        assert_eq!((i.code.as_str(), field_name(&i).as_deref(), i.value("b").as_deref(), i.value("f").as_deref()), ("MERGEFIELD", Some("First_Name"), Some("Dear "), Some(",")));
        assert_eq!(instr_of(&build(&MergeFieldSpec::Field { name: "Joined".into(), before: None, after: None, case: None, picture: Some("MMMM d, yyyy".into()) }, None).unwrap()), r#" MERGEFIELD Joined \@ "MMMM d, yyyy" "#);
        assert_eq!(instr_of(&build(&MergeFieldSpec::Field { name: "Fee".into(), before: None, after: None, case: None, picture: Some("#,##0.00".into()) }, None).unwrap()), r##" MERGEFIELD Fee \# "#,##0.00" "##);
        assert!(build(&MergeFieldSpec::Field { name: " ".into(), before: None, after: None, case: None, picture: None }, None).is_err());
    }

    #[test]
    fn rules_nest_the_field_they_compare() {
        let f = build(&MergeFieldSpec::If { field: "State".into(), op: "eq".into(), value: "CA".into(), then: "West".into(), otherwise: "Elsewhere".into() }, None).unwrap();
        assert_eq!(instr_of(&f), r#" IF " MERGEFIELD State " = "CA" "West" "Elsewhere" "#);
        let f = build(&MergeFieldSpec::SkipIf { field: "Email".into(), op: "empty".into(), value: "ignored".into() }, None).unwrap();
        assert_eq!(instr_of(&f), r#" SKIPIF " MERGEFIELD Email " = "" "#);
        assert_eq!(instr_of(&build(&MergeFieldSpec::Ask { bookmark: "Due date".into(), prompt: "When is it due?".into(), default: "Friday".into(), once: true }, None).unwrap()), r#" ASK Due_date "When is it due?" \d "Friday" \o "#);
        assert_eq!(instr_of(&build(&MergeFieldSpec::FillIn { prompt: "Who signs?".into(), default: String::new(), once: false }, None).unwrap()), r#" FILLIN "Who signs?" "#);
        assert_eq!(instr_of(&build(&MergeFieldSpec::Set { bookmark: "Sig".into(), value: "Ada".into() }, None).unwrap()), r#" SET Sig "Ada" "#);
        assert!(build(&MergeFieldSpec::If { field: "x".into(), op: "near".into(), value: String::new(), then: String::new(), otherwise: String::new() }, None).is_err());
    }

    #[test]
    fn address_and_greeting_codes() {
        let a = build(&MergeFieldSpec::AddressBlock { name: Some("title_first_last".into()), company: true, country: Some("different".into()), except: Some("United States".into()), format: None }, None).unwrap();
        let i = Instr::parse(&instr_of(&a));
        assert_eq!(i.code, "ADDRESSBLOCK");
        assert_eq!(i.value("c").as_deref(), Some("2"));
        assert_eq!(i.value("e").as_deref(), Some("United States"));
        assert_eq!(i.value("f").unwrap(), "<Courtesy Title> <First Name> <Last Name> <Suffix>¶<Company>¶<Address 1>¶<Address 2>¶<City>, <State> <Postal Code>");
        let g = build(&MergeFieldSpec::GreetingLine { salutation: None, name: None, punctuation: None, invalid: None }, None).unwrap();
        let i = Instr::parse(&instr_of(&g));
        assert_eq!(i.value("f").unwrap(), "<< _BEFORE_ Dear >><< _TITLE0_>><< _LAST0_>><< _AFTER_ ,>>");
        assert_eq!(i.value("e").as_deref(), Some("Dear Sir or Madam,"));
        assert!(is_merge_instr(" IF x = y \"a\" "));
        assert!(!is_merge_instr(" DATE "));
    }
}
