//! A forgiving HTML parser: tags, attributes, entities, void elements and the end tags browsers imply.
//! It also understands the conditional markers Word puts in its HTML (`<![if !supportLists]>`).

#[derive(Debug, Clone)]
pub enum HNode {
    Text(String),
    El(HEl),
}

#[derive(Debug, Clone, Default)]
pub struct HEl {
    pub tag: String,
    pub attrs: Vec<(String, String)>,
    pub children: Vec<HNode>,
}

impl HEl {
    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attrs.iter().find(|(k, _)| k == name).map(|(_, v)| v.as_str())
    }

    pub fn classes(&self) -> impl Iterator<Item = &str> {
        self.attr("class").unwrap_or("").split_whitespace()
    }

    /// The text under this element.
    pub fn text(&self) -> String {
        let mut s = String::new();
        fn go(e: &HEl, s: &mut String) {
            for c in &e.children {
                match c {
                    HNode::Text(t) => s.push_str(t),
                    HNode::El(e) => go(e, s),
                }
            }
        }
        go(self, &mut s);
        s
    }

    /// Whether any descendant matches.
    pub fn any(&self, f: &dyn Fn(&HEl) -> bool) -> bool {
        self.children.iter().any(|c| match c {
            HNode::El(e) => f(e) || e.any(f),
            _ => false,
        })
    }
}

const VOID: &[&str] = &["br", "hr", "img", "meta", "link", "input", "col", "area", "base", "wbr", "source", "track", "embed", "param", "basefont", "frame"];
const CLOSES_P: &[&str] = &[
    "address", "article", "aside", "blockquote", "div", "dl", "fieldset", "footer", "form", "h1", "h2", "h3", "h4", "h5", "h6", "header", "hr", "main", "nav", "ol", "p", "pre", "section", "table", "ul", "li", "figure", "details",
];
const SCOPE_STOPS: &[&str] = &["td", "th", "table", "li", "blockquote", "div", "ul", "ol", "body", "caption", "button"];

/// Named character references the clipboard commonly carries.
fn named_entity(name: &str) -> Option<&'static str> {
    Some(match name {
        "amp" => "&",
        "lt" => "<",
        "gt" => ">",
        "quot" => "\"",
        "apos" => "'",
        "nbsp" => "\u{a0}",
        "copy" => "\u{a9}",
        "reg" => "\u{ae}",
        "trade" => "\u{2122}",
        "hellip" => "\u{2026}",
        "mdash" => "\u{2014}",
        "ndash" => "\u{2013}",
        "lsquo" => "\u{2018}",
        "rsquo" => "\u{2019}",
        "sbquo" => "\u{201a}",
        "ldquo" => "\u{201c}",
        "rdquo" => "\u{201d}",
        "bdquo" => "\u{201e}",
        "bull" => "\u{2022}",
        "middot" => "\u{b7}",
        "euro" => "\u{20ac}",
        "pound" => "\u{a3}",
        "yen" => "\u{a5}",
        "cent" => "\u{a2}",
        "sect" => "\u{a7}",
        "para" => "\u{b6}",
        "deg" => "\u{b0}",
        "plusmn" => "\u{b1}",
        "times" => "\u{d7}",
        "divide" => "\u{f7}",
        "frac12" => "\u{bd}",
        "frac14" => "\u{bc}",
        "frac34" => "\u{be}",
        "sup1" => "\u{b9}",
        "sup2" => "\u{b2}",
        "sup3" => "\u{b3}",
        "micro" => "\u{b5}",
        "laquo" => "\u{ab}",
        "raquo" => "\u{bb}",
        "iexcl" => "\u{a1}",
        "iquest" => "\u{bf}",
        "larr" => "\u{2190}",
        "rarr" => "\u{2192}",
        "uarr" => "\u{2191}",
        "darr" => "\u{2193}",
        "harr" => "\u{2194}",
        "ensp" => "\u{2002}",
        "emsp" => "\u{2003}",
        "thinsp" => "\u{2009}",
        "zwnj" => "\u{200c}",
        "zwj" => "\u{200d}",
        "shy" => "\u{ad}",
        "dagger" => "\u{2020}",
        "Dagger" => "\u{2021}",
        "permil" => "\u{2030}",
        "prime" => "\u{2032}",
        "Prime" => "\u{2033}",
        "minus" => "\u{2212}",
        "ne" => "\u{2260}",
        "le" => "\u{2264}",
        "ge" => "\u{2265}",
        "infin" => "\u{221e}",
        "asymp" => "\u{2248}",
        "sum" => "\u{2211}",
        "radic" => "\u{221a}",
        "alpha" => "\u{3b1}",
        "beta" => "\u{3b2}",
        "gamma" => "\u{3b3}",
        "delta" => "\u{3b4}",
        "pi" => "\u{3c0}",
        "sigma" => "\u{3c3}",
        "omega" => "\u{3c9}",
        "mu" => "\u{3bc}",
        "lambda" => "\u{3bb}",
        "theta" => "\u{3b8}",
        "Omega" => "\u{3a9}",
        "Delta" => "\u{394}",
        "agrave" => "\u{e0}",
        "aacute" => "\u{e1}",
        "acirc" => "\u{e2}",
        "atilde" => "\u{e3}",
        "auml" => "\u{e4}",
        "aring" => "\u{e5}",
        "aelig" => "\u{e6}",
        "ccedil" => "\u{e7}",
        "egrave" => "\u{e8}",
        "eacute" => "\u{e9}",
        "ecirc" => "\u{ea}",
        "euml" => "\u{eb}",
        "igrave" => "\u{ec}",
        "iacute" => "\u{ed}",
        "icirc" => "\u{ee}",
        "iuml" => "\u{ef}",
        "ntilde" => "\u{f1}",
        "ograve" => "\u{f2}",
        "oacute" => "\u{f3}",
        "ocirc" => "\u{f4}",
        "otilde" => "\u{f5}",
        "ouml" => "\u{f6}",
        "oslash" => "\u{f8}",
        "ugrave" => "\u{f9}",
        "uacute" => "\u{fa}",
        "ucirc" => "\u{fb}",
        "uuml" => "\u{fc}",
        "yacute" => "\u{fd}",
        "szlig" => "\u{df}",
        "Agrave" => "\u{c0}",
        "Aacute" => "\u{c1}",
        "Acirc" => "\u{c2}",
        "Atilde" => "\u{c3}",
        "Auml" => "\u{c4}",
        "Aring" => "\u{c5}",
        "AElig" => "\u{c6}",
        "Ccedil" => "\u{c7}",
        "Egrave" => "\u{c8}",
        "Eacute" => "\u{c9}",
        "Ecirc" => "\u{ca}",
        "Euml" => "\u{cb}",
        "Igrave" => "\u{cc}",
        "Iacute" => "\u{cd}",
        "Icirc" => "\u{ce}",
        "Iuml" => "\u{cf}",
        "Ntilde" => "\u{d1}",
        "Ograve" => "\u{d2}",
        "Oacute" => "\u{d3}",
        "Ocirc" => "\u{d4}",
        "Otilde" => "\u{d5}",
        "Ouml" => "\u{d6}",
        "Oslash" => "\u{d8}",
        "Ugrave" => "\u{d9}",
        "Uacute" => "\u{da}",
        "Ucirc" => "\u{db}",
        "Uuml" => "\u{dc}",
        _ => return None,
    })
}

/// Replace character references in text.
pub fn decode_entities(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        rest = &rest[i..];
        let end = rest[1..].find([';', ' ', '&', '<', '\n']).map(|e| e + 1);
        let Some(end) = end.filter(|e| rest.as_bytes()[*e] == b';' && *e <= 12) else {
            out.push('&');
            rest = &rest[1..];
            continue;
        };
        let name = &rest[1..end];
        let decoded = if let Some(num) = name.strip_prefix('#') {
            let code = if let Some(h) = num.strip_prefix(['x', 'X']) { u32::from_str_radix(h, 16).ok() } else { num.parse().ok() };
            code.and_then(char::from_u32).map(|c| c.to_string())
        } else {
            named_entity(name).map(String::from)
        };
        match decoded {
            Some(d) => {
                out.push_str(&d);
                rest = &rest[end + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// What the parser is skipping (Word's conditional content).
#[derive(PartialEq)]
enum Skip {
    No,
    /// Until an `[endif]` marker.
    ToEndif,
}

struct P<'a> {
    s: &'a str,
    i: usize,
}

impl P<'_> {
    fn starts(&self, p: &str) -> bool {
        self.s[self.i..].starts_with(p)
    }
}

/// Parse a document or fragment into the children of an implied root.
pub fn parse(html: &str) -> Vec<HNode> {
    let mut p = P { s: html, i: 0 };
    // stack[0] is the root.
    let mut stack: Vec<HEl> = vec![HEl { tag: "#root".into(), ..Default::default() }];
    let mut skip = Skip::No;
    let mut text = String::new();
    macro_rules! flush_text {
        () => {
            if !text.is_empty() {
                if skip == Skip::No {
                    stack.last_mut().unwrap().children.push(HNode::Text(decode_entities(&text)));
                }
                text.clear();
            }
        };
    }
    while p.i < p.s.len() {
        let rest = &p.s[p.i..];
        if !rest.starts_with('<') {
            let end = rest.find('<').unwrap_or(rest.len());
            text.push_str(&rest[..end]);
            p.i += end;
            continue;
        }
        if p.starts("<!--") {
            flush_text!();
            let body_start = p.i + 4;
            let end = p.s[body_start..].find("-->").map(|e| body_start + e);
            let comment = &p.s[body_start..end.unwrap_or(p.s.len())];
            let c = comment.trim();
            if c.starts_with("[if") && c.ends_with("<![endif]") {
                // `<!--[if gte mso 9]>...<![endif]-->`: hidden from other browsers, and from us.
            } else if let Some(cond) = c.strip_prefix("[if ") {
                // `<!--[if !supportLists]-->` ... `<!--[endif]-->` wraps the list number text Word writes by hand.
                if cond.contains("supportLists") || cond.contains("gte vml") {
                    skip = Skip::ToEndif;
                }
            } else if c.starts_with("[endif") {
                skip = Skip::No;
            }
            p.i = end.map(|e| e + 3).unwrap_or(p.s.len());
            continue;
        }
        if p.starts("<![") {
            flush_text!();
            let end = p.s[p.i..].find('>').map(|e| p.i + e).unwrap_or(p.s.len());
            let decl = &p.s[p.i + 3..end.min(p.s.len())];
            if decl.starts_with("if ") {
                if decl.contains("supportLists") || decl.contains("gte vml") || decl.contains("gte mso") {
                    skip = Skip::ToEndif;
                }
            } else if decl.starts_with("endif") {
                skip = Skip::No;
            }
            p.i = (end + 1).min(p.s.len());
            continue;
        }
        if p.starts("<!") || p.starts("<?") {
            flush_text!();
            let end = p.s[p.i..].find('>').map(|e| p.i + e + 1).unwrap_or(p.s.len());
            p.i = end;
            continue;
        }
        if p.starts("</") {
            flush_text!();
            let end = p.s[p.i..].find('>').map(|e| p.i + e).unwrap_or(p.s.len());
            let name = p.s[p.i + 2..end].trim().to_ascii_lowercase();
            p.i = (end + 1).min(p.s.len());
            if skip == Skip::ToEndif {
                continue;
            }
            // Close back to the matching open element, if there is one.
            if let Some(pos) = stack.iter().rposition(|e| e.tag == name) {
                if pos > 0 {
                    while stack.len() > pos {
                        let e = stack.pop().unwrap();
                        stack.last_mut().unwrap().children.push(HNode::El(e));
                    }
                }
            }
            continue;
        }
        // A start tag, or a stray `<`.
        let after = rest.as_bytes().get(1).copied().unwrap_or(b' ');
        if !(after.is_ascii_alphabetic()) {
            text.push('<');
            p.i += 1;
            continue;
        }
        flush_text!();
        let tag_end = find_tag_end(p.s, p.i);
        let inner = &p.s[p.i + 1..tag_end];
        let self_closing = inner.ends_with('/');
        let inner = inner.trim_end_matches('/');
        let name_end = inner.find(|c: char| c.is_whitespace()).unwrap_or(inner.len());
        let name = inner[..name_end].to_ascii_lowercase();
        let attrs = parse_attrs(&inner[name_end..]);
        p.i = (tag_end + 1).min(p.s.len());
        if skip == Skip::ToEndif {
            continue;
        }
        // Raw text elements.
        if name == "style" || name == "script" || name == "textarea" || name == "title" {
            let close = format!("</{name}");
            let lower_from = p.i;
            let end = (lower_from..p.s.len()).find(|&k| p.s.is_char_boundary(k) && p.s[k..].len() >= close.len() && p.s.as_bytes()[k..k + close.len()].eq_ignore_ascii_case(close.as_bytes())).unwrap_or(p.s.len());
            let body = &p.s[lower_from..end];
            let mut el = HEl { tag: name.clone(), attrs, children: Vec::new() };
            if name == "style" {
                el.children.push(HNode::Text(body.to_string()));
            }
            stack.last_mut().unwrap().children.push(HNode::El(el));
            p.i = p.s[end..].find('>').map(|e| end + e + 1).unwrap_or(p.s.len());
            continue;
        }
        imply_end_tags(&mut stack, &name);
        let el = HEl { tag: name.clone(), attrs, children: Vec::new() };
        if VOID.contains(&name.as_str()) || self_closing {
            stack.last_mut().unwrap().children.push(HNode::El(el));
        } else if stack.len() > 300 {
            // Absurdly deep: stop nesting.
            stack.last_mut().unwrap().children.push(HNode::El(el));
        } else {
            stack.push(el);
        }
    }
    flush_text!();
    while stack.len() > 1 {
        let e = stack.pop().unwrap();
        stack.last_mut().unwrap().children.push(HNode::El(e));
    }
    stack.pop().unwrap().children
}

/// The `>` that ends the tag starting at `start`, skipping over quoted attribute values.
fn find_tag_end(s: &str, start: usize) -> usize {
    let b = s.as_bytes();
    let mut i = start + 1;
    let mut quote = 0u8;
    while i < b.len() {
        let c = b[i];
        if quote != 0 {
            if c == quote {
                quote = 0;
            }
        } else if c == b'"' || c == b'\'' {
            quote = c;
        } else if c == b'>' {
            return i;
        }
        i += 1;
    }
    b.len()
}

fn parse_attrs(s: &str) -> Vec<(String, String)> {
    let b = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        while i < b.len() && (b[i].is_ascii_whitespace() || b[i] == b'/') {
            i += 1;
        }
        let ks = i;
        while i < b.len() && !b[i].is_ascii_whitespace() && b[i] != b'=' && b[i] != b'/' {
            i += 1;
        }
        if ks == i {
            i += 1;
            continue;
        }
        let key = s[ks..i].to_ascii_lowercase();
        while i < b.len() && b[i].is_ascii_whitespace() {
            i += 1;
        }
        let mut val = String::new();
        if i < b.len() && b[i] == b'=' {
            i += 1;
            while i < b.len() && b[i].is_ascii_whitespace() {
                i += 1;
            }
            if i < b.len() && (b[i] == b'"' || b[i] == b'\'') {
                let q = b[i];
                i += 1;
                let vs = i;
                while i < b.len() && b[i] != q {
                    i += 1;
                }
                val = decode_entities(&s[vs..i]);
                i += 1;
            } else {
                let vs = i;
                while i < b.len() && !b[i].is_ascii_whitespace() {
                    i += 1;
                }
                val = decode_entities(&s[vs..i]);
            }
        }
        out.push((key, val));
    }
    out
}

/// Close the elements that a new start tag ends by itself (`<p>` after `<p>`, `<li>` after `<li>`, cells and rows).
fn imply_end_tags(stack: &mut Vec<HEl>, name: &str) {
    let close_to = |stack: &mut Vec<HEl>, pos: usize| {
        while stack.len() > pos {
            let e = stack.pop().unwrap();
            stack.last_mut().unwrap().children.push(HNode::El(e));
        }
    };
    if CLOSES_P.contains(&name) {
        for k in (1..stack.len()).rev() {
            let t = stack[k].tag.as_str();
            if t == "p" {
                close_to(stack, k);
                break;
            }
            if SCOPE_STOPS.contains(&t) {
                break;
            }
        }
    }
    let ends = |tag: &str| -> &'static [&'static str] {
        match tag {
            "li" => &["li"],
            "td" | "th" => &["td", "th"],
            "tr" => &["td", "th", "tr"],
            "thead" | "tbody" | "tfoot" => &["td", "th", "tr", "thead", "tbody", "tfoot"],
            "dt" | "dd" => &["dt", "dd"],
            "option" => &["option"],
            _ => &[],
        }
    };
    let targets = ends(name);
    if targets.is_empty() {
        return;
    }
    let stops: &[&str] = match name {
        "li" => &["ul", "ol"],
        "td" | "th" => &["tr", "table"],
        "tr" => &["table", "thead", "tbody", "tfoot"],
        _ => &["table"],
    };
    for k in (1..stack.len()).rev() {
        let t = stack[k].tag.as_str();
        if stops.contains(&t) {
            break;
        }
        if targets.contains(&t) {
            // Close this and everything above it, then keep looking for an outer one only for rows.
            close_to(stack, k);
            if !matches!(name, "tr" | "thead" | "tbody" | "tfoot") {
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(nodes: &[HNode]) -> String {
        nodes.iter().map(|n| match n { HNode::Text(t) => t.clone(), HNode::El(e) => format!("<{}>{}</{}>", e.tag, text(&e.children), e.tag) }).collect()
    }

    #[test]
    fn nesting_and_implied_ends() {
        let n = parse("<p>one<p>two<ul><li>a<li>b</ul><table><tr><td>1<td>2<tr><td>3</table>");
        assert_eq!(text(&n), "<p>one</p><p>two</p><ul><li>a</li><li>b</li></ul><table><tr><td>1</td><td>2</td></tr><tr><td>3</td></tr></table>");
    }

    #[test]
    fn entities_and_attributes() {
        let n = parse(r#"<a href="a?x=1&amp;y=2" class=big data-x='q'>caf&eacute; &#8212; &#x41; &bogus; &</a>"#);
        let HNode::El(a) = &n[0] else { panic!() };
        assert_eq!(a.attr("href"), Some("a?x=1&y=2"));
        assert_eq!(a.attr("class"), Some("big"));
        assert_eq!(a.text(), "caf\u{e9} \u{2014} A &bogus; &");
    }

    #[test]
    fn word_conditional_content_is_skipped() {
        let n = parse("<p>a<![if !supportLists]><span>1.</span><![endif]>b<!--[if gte mso 9]><xml>x</xml><![endif]-->c<!--[if !supportLists]-->2.<!--[endif]-->d</p>");
        assert_eq!(text(&n), "<p>abcd</p>");
    }

    #[test]
    fn styles_and_scripts_are_raw_text() {
        let n = parse("<style>p { color: red } </style><script>if (a < b) {}</script><p>x</p>");
        assert_eq!(text(&n), "<style>p { color: red } </style><script></script><p>x</p>");
    }
}
