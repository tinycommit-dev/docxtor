//! The address block and the greeting line: a recipient's name and postal address, and "Dear Mr. Randall,", laid out
//! from the columns of the list. Word's fields have names of their own (Courtesy Title, First Name, Address 1...) and
//! a list names its columns however it likes, so a field is found by the mapping a person made (Match Fields), or
//! else by a column that has the field's name or a name people commonly use for it.

use super::{norm, MailMerge};
use crate::fields::Instr;

/// Word's address fields, with the column names that stand for each when nothing is mapped.
pub const STANDARD_FIELDS: &[(&str, &[&str])] = &[
    ("Courtesy Title", &["courtesy title", "title", "salutation", "prefix", "honorific"]),
    ("First Name", &["first name", "given name", "forename", "first"]),
    ("Middle Name", &["middle name", "middle initial", "middle"]),
    ("Last Name", &["last name", "family name", "surname", "last"]),
    ("Suffix", &["suffix"]),
    ("Nickname", &["nickname", "nick name"]),
    ("Job Title", &["job title", "position", "occupation", "role"]),
    ("Company", &["company", "company name", "organization", "organisation", "business", "employer"]),
    ("Address 1", &["address 1", "address line 1", "address", "street address", "street address 1", "street", "street 1", "addr1", "line 1"]),
    ("Address 2", &["address 2", "address line 2", "street address 2", "street 2", "addr2", "suite", "apartment", "apt", "line 2"]),
    ("Address 3", &["address 3", "address line 3"]),
    ("City", &["city", "town", "locality"]),
    ("State", &["state", "province", "state or province", "state/province", "region", "county"]),
    ("Postal Code", &["postal code", "zip code", "zip", "postcode", "post code", "zipcode"]),
    ("Country or Region", &["country or region", "country", "nation"]),
    ("E-mail Address", &["e-mail address", "email address", "email", "e-mail"]),
    ("Work Phone", &["work phone", "business phone", "phone"]),
    ("Home Phone", &["home phone", "mobile phone", "mobile"]),
    ("Unique Identifier", &["unique identifier", "id", "customer id"]),
];

/// The shape of an address block when its field has no format of its own.
pub const DEFAULT_ADDRESS: &str = "<First Name> <Last Name> <Suffix>¶<Company>¶<Address 1>¶<Address 2>¶<City>, <State> <Postal Code>";
const DEFAULT_GREETING: &str = "<< _BEFORE_ Dear >><< _TITLE0_>><< _LAST0_>><< _AFTER_ ,>>";

/// The column that stands for one of Word's address fields.
pub fn standard_column(mm: &MailMerge, field: &str) -> Option<usize> {
    let want = norm(field);
    if let Some(col) = mm.mapping.iter().find(|(k, _)| norm(k) == want).and_then(|(_, v)| mm.column(v)) {
        return Some(col);
    }
    let aliases = STANDARD_FIELDS.iter().find(|(n, _)| norm(n) == want).map(|(_, a)| *a).unwrap_or(&[]);
    aliases.iter().find_map(|a| mm.column(a)).or_else(|| mm.column(field))
}

/// One recipient: a record of the list.
pub struct Recipient<'a> {
    pub mm: &'a MailMerge,
    /// The record as an index into the table, or none past the end of the list (every value is empty).
    pub row: Option<usize>,
}

impl Recipient<'_> {
    /// The value of a column by its name.
    pub fn column(&self, name: &str) -> Option<&str> {
        let c = self.mm.column(name)?;
        Some(self.mm.table.rows.get(self.row?)?.get(c)?.as_str())
    }

    /// The value of one of Word's address fields.
    pub fn field(&self, name: &str) -> String {
        let Some(row) = self.row.and_then(|r| self.mm.table.rows.get(r)) else { return String::new() };
        standard_column(self.mm, name).and_then(|c| row.get(c)).map(|v| v.trim().to_string()).unwrap_or_default()
    }
}

enum Tok {
    Lit(String),
    Field(String),
}

/// The text of a line as literals and the fields in angle brackets.
fn tokens(line: &str) -> Vec<Tok> {
    let mut out = Vec::new();
    let mut lit = String::new();
    let mut rest = line;
    while let Some(open) = rest.find('<') {
        let Some(close) = rest[open..].find('>') else { break };
        lit.push_str(&rest[..open]);
        if !lit.is_empty() {
            out.push(Tok::Lit(std::mem::take(&mut lit)));
        }
        out.push(Tok::Field(rest[open + 1..open + close].trim().to_string()));
        rest = &rest[open + close + 1..];
    }
    lit.push_str(rest);
    if !lit.is_empty() {
        out.push(Tok::Lit(lit));
    }
    out
}

/// One line of an address: the fields that have a value, with the separators between them that still have a value
/// on both sides. A line of nothing is empty.
fn address_line(line: &str, value: &dyn Fn(&str) -> String) -> String {
    let toks = tokens(line);
    let mut out = String::new();
    let mut lead = String::new();
    let mut pending: Option<String> = None;
    let mut seen_field = false;
    let mut emitted = false;
    for t in toks {
        match t {
            Tok::Lit(s) => {
                if !seen_field {
                    lead = s;
                } else if emitted && pending.is_none() {
                    pending = Some(s);
                }
            }
            Tok::Field(name) => {
                seen_field = true;
                let v = value(&name);
                if v.is_empty() {
                    continue;
                }
                if emitted {
                    out.push_str(pending.take().as_deref().unwrap_or(" "));
                } else {
                    out.push_str(&lead);
                }
                out.push_str(&v);
                pending = None;
                emitted = true;
            }
        }
    }
    out
}

/// The text of an `ADDRESSBLOCK` field for a recipient, lines separated by `\n`.
pub fn address_block(r: &Recipient, i: &Instr) -> String {
    let format = i.value("f").filter(|f| !f.trim().is_empty()).unwrap_or_else(|| DEFAULT_ADDRESS.to_string());
    let mode = i.value("c").and_then(|c| c.trim().parse::<u8>().ok()).unwrap_or(0);
    let except = i.value("e").unwrap_or_default();
    let country = r.field("Country or Region");
    let show_country = match mode {
        0 => false,
        1 => !country.is_empty(),
        _ => !country.is_empty() && !country.eq_ignore_ascii_case(except.trim()),
    };
    let value = |name: &str| -> String {
        if norm(name) == norm("Country or Region") {
            return if show_country { country.clone() } else { String::new() };
        }
        let v = r.field(name);
        if v.is_empty() {
            r.column(name).map(|v| v.trim().to_string()).unwrap_or_default()
        } else {
            v
        }
    };
    let mut lines: Vec<String> = format.split(['¶', '\n']).map(|l| address_line(l, &value)).filter(|l| !l.trim().is_empty()).collect();
    if show_country && !format.to_lowercase().contains("country") {
        lines.push(country.to_uppercase());
    }
    lines.join("\n")
}

/// The text of a `GREETINGLINE` field: "Dear Mr. Randall,", or the text for a recipient with no name.
pub fn greeting_line(r: &Recipient, i: &Instr) -> String {
    let format = i.value("f").filter(|f| !f.trim().is_empty()).unwrap_or_else(|| DEFAULT_GREETING.to_string());
    let invalid = i.value("e").unwrap_or_else(|| "Dear Sir or Madam,".to_string());
    let (mut before, mut after) = (String::new(), String::new());
    let mut names: Vec<String> = Vec::new();
    // Groups are `<< ... >>`, or `< ... >` in a hand-written code.
    let mut rest = format.as_str();
    while let Some(open) = rest.find('<') {
        let (skip, end_mark) = if rest[open..].starts_with("<<") { (2, ">>") } else { (1, ">") };
        let body_start = open + skip;
        let Some(close) = rest[body_start..].find(end_mark) else { break };
        let body = rest[body_start..body_start + close].trim();
        rest = &rest[body_start + close + end_mark.len()..];
        if let Some(t) = body.strip_prefix("_BEFORE_") {
            before = t.trim().to_string();
        } else if let Some(t) = body.strip_prefix("_AFTER_") {
            after = t.trim().to_string();
        } else {
            let field = match body.trim_matches('_').to_ascii_uppercase().as_str() {
                "TITLE0" => "Courtesy Title",
                "FIRST0" => "First Name",
                "LAST0" => "Last Name",
                "SUFFIX0" => "Suffix",
                "NICK0" => "Nickname",
                _ => "",
            };
            if !field.is_empty() {
                names.push(r.field(field));
            }
        }
    }
    if r.field("First Name").is_empty() && r.field("Last Name").is_empty() {
        return invalid;
    }
    let name = names.into_iter().filter(|n| !n.is_empty()).collect::<Vec<_>>().join(" ");
    if name.is_empty() {
        return invalid;
    }
    if before.is_empty() {
        format!("{name}{after}")
    } else {
        format!("{before} {name}{after}")
    }
}

#[cfg(test)]
mod tests {
    use super::super::Table;
    use super::*;
    use std::sync::Arc;

    fn list() -> MailMerge {
        let cols = ["Title", "Given Name", "Surname", "Company Name", "Street", "Apt", "Town", "Province", "ZIP", "Country"];
        let rows = [
            ["Ms.", "Ada", "Lovelace", "Analytical Engines", "12 Crumb Rd", "", "London", "", "N1 9GU", "United Kingdom"],
            ["", "Grace", "Hopper", "", "1 Navy Way", "Suite 4", "Arlington", "VA", "22202", "United States"],
            ["", "", "", "Acme", "5 Main St", "", "Springfield", "IL", "62701", ""],
        ];
        MailMerge { table: Arc::new(Table::from_rows(cols.map(String::from).to_vec(), rows.iter().map(|r| r.map(String::from).to_vec()).collect())), ..Default::default() }
    }

    fn at(mm: &MailMerge, row: usize) -> Recipient<'_> {
        Recipient { mm, row: Some(row) }
    }

    #[test]
    fn finds_columns_by_the_names_people_use() {
        let mm = list();
        assert_eq!(standard_column(&mm, "First Name"), Some(1));
        assert_eq!(standard_column(&mm, "Postal Code"), Some(8));
        assert_eq!(standard_column(&mm, "Address 1"), Some(4));
        assert_eq!(standard_column(&mm, "Courtesy Title"), Some(0));
        assert_eq!(standard_column(&mm, "Nickname"), None);
        let mut mm = list();
        mm.mapping.insert("First Name".into(), "Surname".into());
        assert_eq!(at(&mm, 0).field("First Name"), "Lovelace");
    }

    #[test]
    fn address_block_drops_empty_lines_and_separators() {
        let mm = list();
        let code = Instr::parse(r#" ADDRESSBLOCK \f "<Courtesy Title> <First Name> <Last Name> <Suffix>¶<Company>¶<Address 1>¶<Address 2>¶<City>, <State> <Postal Code>" \c 2 \e "United States" "#);
        assert_eq!(address_block(&at(&mm, 0), &code), "Ms. Ada Lovelace\nAnalytical Engines\n12 Crumb Rd\nLondon, N1 9GU\nUNITED KINGDOM");
        assert_eq!(address_block(&at(&mm, 1), &code), "Grace Hopper\n1 Navy Way\nSuite 4\nArlington, VA 22202");
        // No name: the line goes, the rest stays.
        assert_eq!(address_block(&at(&mm, 2), &code), "Acme\n5 Main St\nSpringfield, IL 62701");
        // The country is never added unless asked for.
        let never = Instr::parse(r#" ADDRESSBLOCK \c 0 "#);
        assert_eq!(address_block(&at(&mm, 0), &never), "Ada Lovelace\nAnalytical Engines\n12 Crumb Rd\nLondon, N1 9GU");
        let always = Instr::parse(r#" ADDRESSBLOCK \c 1 "#);
        assert!(address_block(&at(&mm, 1), &always).ends_with("UNITED STATES"));
        // Past the end of the list there is nothing.
        assert_eq!(address_block(&Recipient { mm: &mm, row: None }, &code), "");
    }

    #[test]
    fn greeting_lines() {
        let mm = list();
        let code = Instr::parse(r#" GREETINGLINE \f "<< _BEFORE_ Dear >><< _TITLE0_>><< _LAST0_>><< _AFTER_ ,>>" \l 1033 \m \e "Dear Sir or Madam," "#);
        assert_eq!(greeting_line(&at(&mm, 0), &code), "Dear Ms. Lovelace,");
        // No title: the name is just the last name.
        assert_eq!(greeting_line(&at(&mm, 1), &code), "Dear Hopper,");
        assert_eq!(greeting_line(&at(&mm, 2), &code), "Dear Sir or Madam,");
        let first = Instr::parse(r#" GREETINGLINE \f "<< _BEFORE_ Hello >><< _FIRST0_>><< _AFTER_ !>>" "#);
        assert_eq!(greeting_line(&at(&mm, 1), &first), "Hello Grace!");
        let plain = Instr::parse(r#" GREETINGLINE "#);
        assert_eq!(greeting_line(&at(&mm, 0), &plain), "Dear Ms. Lovelace,");
        // The single bracket form a person might type.
        let single = Instr::parse(r#" GREETINGLINE \f "<_BEFORE_ To><_FIRST0_><_LAST0_><_AFTER_ :>" "#);
        assert_eq!(greeting_line(&at(&mm, 1), &single), "To Grace Hopper:");
    }
}
