//! What a pasted fragment may contain. A fragment is read from the clipboard, and any page the person copied from can
//! put one there, so it is checked before it goes in: only the namespaces Word uses, no embedded objects or document
//! parts, no data bindings, and field codes only from a list of harmless ones (the result text of any other field stays
//! as plain text). Docxtor's own copies pass through unchanged: they hold nothing this takes out.

use crate::xml::{Element, Node};

/// Namespace prefixes an element may have. Anything else (a foreign markup island, a custom schema) is left out.
const PREFIXES: &[&str] = &[
    "w", "r", "wp", "a", "pic", "m", "mc", "v", "o", "w10", "wps", "wpg", "wpc", "w14", "w15", "w16", "w16cid", "w16se", "wp14", "a14", "a16", "c", "wne", "wpi", "cx", "cx1",
];

/// Elements that carry active or external content, or a link out of the document. They go with what is inside them.
const BLOCKED: &[&str] = &["w:altChunk", "w:subDoc", "w:object", "w:control", "w:ocx", "w:movie", "w:dataBinding", "w:webSettings", "w:attachedSchema", "o:OLEObject", "o:ink"];

/// Wrappers that only tag their content: the tag goes, the content stays.
const UNWRAPPED: &[&str] = &["w:customXml", "w:smartTag"];

/// Field codes that only show a value. The rest (INCLUDETEXT, INCLUDEPICTURE, LINK, DDE, MACROBUTTON, IMPORT, DATABASE,
/// AUTOTEXT, PRINT, ASK, FILLIN and the like) read files, start programs or ask the person things.
const FIELDS: &[&str] = &[
    "PAGE", "NUMPAGES", "NUMWORDS", "NUMCHARS", "SECTION", "SECTIONPAGES", "DATE", "TIME", "CREATEDATE", "SAVEDATE", "PRINTDATE", "EDITTIME", "REVNUM", "SEQ", "REF", "PAGEREF",
    "NOTEREF", "STYLEREF", "TOC", "TC", "XE", "INDEX", "TA", "TOA", "BIBLIOGRAPHY", "CITATION", "AUTHOR", "TITLE", "SUBJECT", "KEYWORDS", "COMMENTS", "FILENAME", "FILESIZE",
    "LASTSAVEDBY", "DOCPROPERTY", "DOCVARIABLE", "TEMPLATE", "USERNAME", "USERINITIALS", "USERADDRESS", "QUOTE", "SET", "IF", "COMPARE", "FORMTEXT", "FORMCHECKBOX",
    "FORMDROPDOWN", "EQ", "SYMBOL", "HYPERLINK", "ADDIN", "MERGEFIELD", "AUTONUM", "AUTONUMLGL", "AUTONUMOUT", "LISTNUM", "ADVANCE", "=",
];

/// Link schemes a pasted link may use.
const SCHEMES: &[&str] = &["http", "https", "mailto", "tel", "ftp", "ftps", "sms", "news"];

/// Whether a link target is one a pasted fragment may carry: a web, mail or phone address, or one with no scheme.
#[cfg(test)]
pub(crate) fn safe_url(url: &str) -> bool {
    safe_url_for(url, false)
}

/// `safe_url`, and also `file:` addresses when `own` (Docxtor copied the fragment itself, so a link a document already
/// had comes back as it was; from any other clipboard a `file:` link would name a path on the person's machine).
pub(crate) fn safe_url_for(url: &str, own: bool) -> bool {
    let u = url.trim();
    if u.chars().any(|c| c.is_control()) {
        return false;
    }
    let colon = u.find(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '+' | '.' | '-'))).filter(|n| *n > 0 && u[*n..].starts_with(':'));
    match colon {
        Some(n) => {
            let scheme = u[..n].to_ascii_lowercase();
            SCHEMES.contains(&scheme.as_str()) || (own && scheme == "file")
        }
        None => true,
    }
}

/// The first word of a field's instruction, upper-cased (`=` for a formula).
fn field_name(instr: &str) -> String {
    let t = instr.trim_start();
    if t.starts_with('=') {
        return "=".into();
    }
    t.chars().take_while(|c| !c.is_whitespace() && !matches!(c, '"' | '\\' | '{' | '(')).collect::<String>().to_ascii_uppercase()
}

fn field_ok(instr: &str, own: bool) -> bool {
    let name = field_name(instr);
    if !FIELDS.contains(&name.as_str()) {
        return false;
    }
    if name == "HYPERLINK" {
        // The address is the first quoted string or word after the name; `\l` names a place in the document.
        let rest = instr.trim_start()[name.len()..].trim_start();
        let target = if let Some(q) = rest.strip_prefix('"') { q.split('"').next().unwrap_or("") } else { rest.split_whitespace().next().unwrap_or("") };
        return target.starts_with('\\') || safe_url_for(target, own);
    }
    true
}

fn allowed_name(name: &str) -> bool {
    !BLOCKED.contains(&name) && name.split_once(':').is_some_and(|(p, _)| PREFIXES.contains(&p))
}

/// Leave out the elements that aren't allowed, with what is inside them; unwrap the tagging ones.
fn strip_names(e: &mut Element) {
    let mut kept = Vec::with_capacity(e.children.len());
    for n in std::mem::take(&mut e.children) {
        match n {
            Node::Element(mut c) => {
                if UNWRAPPED.contains(&c.name.as_str()) {
                    strip_names(&mut c);
                    kept.extend(c.children);
                } else if allowed_name(&c.name) {
                    strip_names(&mut c);
                    kept.push(Node::Element(c));
                }
            }
            other => kept.push(other),
        }
    }
    e.children = kept;
}

/// The instruction of every complex field in the element, in the order its `begin` comes.
fn instructions(e: &Element, instr: &mut Vec<String>, open: &mut Vec<usize>) {
    for c in e.elements() {
        if c.name == "w:r" {
            for m in c.elements() {
                match m.name.as_str() {
                    "w:fldChar" => match m.attr("w:fldCharType") {
                        Some("begin") => {
                            instr.push(String::new());
                            open.push(instr.len() - 1);
                        }
                        Some("end") => {
                            open.pop();
                        }
                        _ => {}
                    },
                    "w:instrText" => {
                        if let Some(i) = open.last() {
                            instr[*i].push_str(&m.text());
                        }
                    }
                    _ => {}
                }
            }
        } else {
            instructions(c, instr, open);
        }
    }
}

struct Pass<'a> {
    own: bool,
    ok: &'a [bool],
    next: usize,
    /// Open fields: (index, still reading the instruction).
    open: Vec<(usize, bool)>,
}

impl Pass<'_> {
    /// Whether text here belongs to the instruction of a field that isn't allowed.
    fn in_bad_instruction(&self) -> bool {
        self.open.iter().any(|(i, instr)| *instr && !self.ok[*i])
    }

    /// Whether this run should go: it holds a disallowed field's mark or instruction, or sits inside one's instruction.
    fn drop_run(&mut self, r: &Element) -> bool {
        let mut drop = self.in_bad_instruction();
        for c in r.elements() {
            match c.name.as_str() {
                "w:fldChar" => match c.attr("w:fldCharType") {
                    Some("begin") => {
                        let i = self.next;
                        self.next += 1;
                        self.open.push((i, true));
                        drop |= !self.ok[i];
                    }
                    Some("separate") => match self.open.last_mut() {
                        Some((i, instr)) => {
                            *instr = false;
                            drop |= !self.ok[*i];
                        }
                        None => drop = true,
                    },
                    Some("end") => match self.open.pop() {
                        Some((i, _)) => drop |= !self.ok[i],
                        None => drop = true,
                    },
                    _ => {}
                },
                "w:instrText" => {
                    if self.open.last().is_none_or(|(i, _)| !self.ok[*i]) {
                        drop = true;
                    }
                }
                _ => {}
            }
        }
        drop
    }

    fn clean(&mut self, e: &mut Element) {
        let mut kept = Vec::with_capacity(e.children.len());
        for n in std::mem::take(&mut e.children) {
            let Node::Element(mut c) = n else {
                kept.push(n);
                continue;
            };
            if c.name == "w:fldSimple" {
                let own = self.own;
                let ok = c.attr("w:instr").is_some_and(|i| field_ok(i, own));
                self.clean(&mut c);
                if ok {
                    kept.push(Node::Element(c));
                } else {
                    // The result stays as plain content; the instruction goes.
                    kept.extend(c.children);
                }
                continue;
            }
            if c.name == "w:r" {
                if self.drop_run(&c) {
                    continue;
                }
            } else {
                self.clean(&mut c);
            }
            kept.push(Node::Element(c));
        }
        e.children = kept;
    }
}

/// Make a fragment's element safe to insert (see the module docs). Returns false when the element itself is not allowed.
pub(crate) fn sanitize(el: &mut Element, own: bool) -> bool {
    if !allowed_name(&el.name) {
        return false;
    }
    strip_names(el);
    let mut instr = Vec::new();
    instructions(el, &mut instr, &mut Vec::new());
    let ok: Vec<bool> = instr.iter().map(|i| field_ok(i, own)).collect();
    let mut pass = Pass { own, ok: &ok, next: 0, open: Vec::new() };
    pass.clean(el);
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::xml;

    fn clean(src: &str) -> String {
        let doc = xml::parse(src.as_bytes()).unwrap();
        let mut el = doc.root;
        assert!(sanitize(&mut el, false));
        el.to_xml()
    }

    const W: &str = r#"xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main""#;

    fn field(instr: &str, result: &str) -> String {
        format!(
            r#"<w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText>{instr}</w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:t>{result}</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r>"#
        )
    }

    #[test]
    fn harmless_fields_and_plain_text_pass_through() {
        let src = format!(r#"<w:p {W}><w:r><w:t>Page </w:t></w:r>{}</w:p>"#, field(" PAGE ", "3"));
        let out = clean(&src);
        assert!(out.contains("w:instrText") && out.contains("w:fldChar") && out.contains("PAGE"), "{out}");
    }

    #[test]
    fn dangerous_complex_fields_keep_only_their_result() {
        let src = format!(r#"<w:p {W}>{}</w:p>"#, field(r#" INCLUDETEXT "C:\secret.docx" "#, "shown"));
        let out = clean(&src);
        assert!(!out.contains("instrText") && !out.contains("fldChar") && !out.contains("INCLUDETEXT"), "{out}");
        assert!(out.contains("shown"), "{out}");
    }

    #[test]
    fn dangerous_simple_fields_unwrap() {
        let src = format!(
            r#"<w:p {W}><w:fldSimple w:instr=" DDEAUTO cmd calc "><w:r><w:t>result</w:t></w:r></w:fldSimple><w:fldSimple w:instr=" PAGE "><w:r><w:t>1</w:t></w:r></w:fldSimple></w:p>"#
        );
        let out = clean(&src);
        assert!(!out.contains("DDEAUTO"), "{out}");
        assert!(out.contains("result") && out.contains(r#"w:instr=" PAGE ""#), "{out}");
    }

    #[test]
    fn a_nested_field_inside_a_bad_instruction_goes() {
        // { LINK { PAGE } } result
        let src = format!(
            r#"<w:p {W}><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText> LINK Excel.Sheet.12 </w:instrText></w:r>{}<w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:t>kept</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r></w:p>"#,
            field(" PAGE ", "9")
        );
        let out = clean(&src);
        assert!(!out.contains("instrText") && !out.contains("fldChar") && !out.contains("LINK"), "{out}");
        assert!(out.contains("kept") && !out.contains(">9<"), "{out}");
    }

    #[test]
    fn objects_and_foreign_markup_go() {
        let src = format!(
            r#"<w:p {W} xmlns:x="urn:x"><w:r><w:object><w:t>ole</w:t></w:object></w:r><x:script/><w:altChunk/><w:customXml w:element="x"><w:r><w:t>tagged</w:t></w:r></w:customXml><w:r><w:t>text</w:t></w:r></w:p>"#
        );
        let out = clean(&src);
        assert!(!out.contains("object") && !out.contains("x:script") && !out.contains("altChunk") && !out.contains("customXml"), "{out}");
        assert!(out.contains("text") && out.contains("tagged"), "{out}");
    }

    #[test]
    fn links_need_a_safe_scheme() {
        assert!(safe_url("https://example.com/a"));
        assert!(safe_url("mailto:a@b.c"));
        assert!(safe_url("#heading"));
        assert!(safe_url("page.html"));
        assert!(!safe_url("javascript:alert(1)"));
        assert!(!safe_url("  JavaScript:alert(1)"));
        assert!(!safe_url("file:///etc/passwd"));
        assert!(safe_url_for("file://server/share/Q3 report.docx", true));
        assert!(!safe_url_for("javascript:alert(1)", true));
        assert!(!safe_url_for("data:text/html,x", true));
        assert!(!safe_url("data:text/html,x"));
        assert!(!safe_url("\u{1}javascript:x"));
    }

    #[test]
    fn hyperlink_fields_check_their_target() {
        assert!(field_ok(r#" HYPERLINK "https://example.com" "#, false));
        assert!(field_ok(r#" HYPERLINK \l "bm" "#, false));
        assert!(!field_ok(r#" HYPERLINK "javascript:alert(1)" "#, false));
        assert!(!field_ok(r#" HYPERLINK "file://server/share/a.docx" "#, false));
        assert!(field_ok(r#" HYPERLINK "file://server/share/a.docx" "#, true));
        assert!(!field_ok(r#" HYPERLINK "javascript:alert(1)" "#, true));
        assert!(field_ok(" =SUM(ABOVE) ", false));
        assert!(!field_ok(" MACROBUTTON Run ", false));
    }
}
