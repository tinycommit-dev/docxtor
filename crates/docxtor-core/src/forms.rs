//! Legacy form fields: `FORMTEXT`, `FORMCHECKBOX` and `FORMDROPDOWN`, complex fields whose first `w:fldChar`
//! holds a `w:ffData`. Reading what a field is, building the markup Word writes, and changing the state a
//! person sets (a ticked box, the chosen entry, the typed text).

use crate::model::*;
use crate::ops::Pos;
use crate::xml::{Element, Node};
use serde::{Deserialize, Serialize};

/// The text Word puts in a new text field: five en spaces.
pub const EMPTY_RESULT: &str = "\u{2002}\u{2002}\u{2002}\u{2002}\u{2002}";

/// A legacy form field as the editor needs it. `n` is its number among the form fields of the paragraph.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RForm {
    pub n: usize,
    /// `text`, `checkbox` or `dropdown`.
    pub kind: String,
    /// Where the field starts, and (for a text field) where its result starts and ends.
    pub at: usize,
    pub start: usize,
    pub end: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub disabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub help: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    /// A text field: `regular`, `number`, `date`, `currentDate`, `currentTime` or `calculated`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_length: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checked: Option<bool>,
    /// A check box: its size in points (absent: automatic), and whether it starts ticked.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<f64>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub default_checked: bool,
    /// A drop-down: the entries and the chosen one.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub items: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected: Option<usize>,
}

/// What the Properties dialog of a form field edits; fields left out stay as they are.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FormProps {
    pub name: Option<String>,
    pub disabled: Option<bool>,
    pub help: Option<String>,
    pub status: Option<String>,
    pub text_type: Option<String>,
    pub default: Option<String>,
    pub max_length: Option<u32>,
    pub format: Option<String>,
    /// A check box's size in points; 0 for automatic.
    pub size: Option<f64>,
    pub default_checked: Option<bool>,
    pub items: Option<Vec<String>>,
}

/// Legacy form field ops. The wire form is `{"op": "form_field", "action": "...", ...}`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum FormOp {
    /// Insert a field (`text`, `checkbox` or `dropdown`) at a position.
    Insert {
        at: Pos,
        kind: String,
        #[serde(default)]
        props: FormProps,
    },
    /// Fill in field `n` of paragraph `at`: tick or untick a box, choose an entry, or type text.
    Set {
        at: Pos,
        n: usize,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        checked: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        index: Option<usize>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        text: Option<String>,
    },
    /// Change a field's properties.
    Props { at: Pos, n: usize, props: FormProps },
}

/// A form field found in a paragraph.
#[derive(Debug, Clone)]
pub struct Found {
    pub n: usize,
    pub kind: &'static str,
    /// The run (counting runs through wrappers, in order) and item holding `w:fldChar` begin.
    pub begin: (usize, usize),
    pub at: usize,
    pub start: usize,
    pub end: usize,
    pub ff: Option<Element>,
}

fn kind_of(instr: &str) -> Option<&'static str> {
    match instr.split_whitespace().next()?.to_uppercase().as_str() {
        "FORMTEXT" => Some("text"),
        "FORMCHECKBOX" => Some("checkbox"),
        "FORMDROPDOWN" => Some("dropdown"),
        _ => None,
    }
}

/// Every run of a paragraph in order, through links, content controls and other wrappers, with the offset
/// where it starts.
pub fn runs_of(inlines: &[Inline]) -> Vec<(&Run, usize)> {
    fn go<'a>(inlines: &'a [Inline], at: &mut usize, out: &mut Vec<(&'a Run, usize)>) {
        for i in inlines {
            match i {
                Inline::Run(r) => {
                    out.push((r, *at));
                    *at += r.len16();
                }
                Inline::Container(c) => go(&c.children, at, out),
                Inline::Opaque(_) => *at += i.len16(),
            }
        }
    }
    let mut out = Vec::new();
    go(inlines, &mut 0, &mut out);
    out
}

pub fn run_mut(inlines: &mut [Inline], idx: usize) -> Option<&mut Run> {
    fn go<'a>(inlines: &'a mut [Inline], idx: usize, seen: &mut usize) -> Option<&'a mut Run> {
        for i in inlines.iter_mut() {
            match i {
                Inline::Run(r) => {
                    if *seen == idx {
                        return Some(r);
                    }
                    *seen += 1;
                }
                Inline::Container(c) => {
                    if let Some(r) = go(&mut c.children, idx, seen) {
                        return Some(r);
                    }
                }
                Inline::Opaque(_) => {}
            }
        }
        None
    }
    go(inlines, idx, &mut 0)
}

/// The form fields of a paragraph in the order they begin.
pub fn find(inlines: &[Inline]) -> Vec<Found> {
    struct Open {
        begin: (usize, usize),
        at: usize,
        instr: String,
        sep: Option<usize>,
        ff: Option<Element>,
        done_instr: bool,
    }
    let mut stack: Vec<Open> = Vec::new();
    let mut out: Vec<Found> = Vec::new();
    for (ri, (r, run_at)) in runs_of(inlines).into_iter().enumerate() {
        let mut at = run_at;
        for (ii, item) in r.items.iter().enumerate() {
            match item {
                RunItem::Hidden(e) if e.name == "w:fldChar" => match e.attr("w:fldCharType") {
                    Some("begin") => stack.push(Open { begin: (ri, ii), at, instr: String::new(), sep: None, ff: e.child("w:ffData").cloned(), done_instr: false }),
                    Some("separate") => {
                        if let Some(t) = stack.last_mut() {
                            t.sep = Some(at);
                            t.done_instr = true;
                        }
                    }
                    Some("end") => {
                        if let Some(o) = stack.pop() {
                            if let Some(kind) = kind_of(&o.instr) {
                                out.push(Found { n: 0, kind, begin: o.begin, at: o.at, start: o.sep.unwrap_or(at), end: at, ff: o.ff });
                            }
                        }
                    }
                    _ => {}
                },
                RunItem::Hidden(e) if e.name == "w:instrText" => {
                    if let Some(t) = stack.last_mut().filter(|t| !t.done_instr) {
                        t.instr.push_str(&e.text());
                    }
                }
                _ => {}
            }
            at += item.len16();
        }
    }
    out.sort_by_key(|f| f.begin);
    for (n, f) in out.iter_mut().enumerate() {
        f.n = n;
    }
    out
}

fn ff_val(ff: &Element, name: &str) -> Option<String> {
    ff.child(name).and_then(|e| e.attr("w:val")).map(String::from)
}

/// The views of a paragraph's form fields.
pub fn views(inlines: &[Inline]) -> Vec<RForm> {
    find(inlines)
        .into_iter()
        .map(|f| {
            let mut v = RForm { n: f.n, kind: f.kind.into(), at: f.at, start: f.start, end: f.end, ..Default::default() };
            if let Some(ff) = &f.ff {
                v.name = ff_val(ff, "w:name");
                v.disabled = ff.child("w:enabled").is_some_and(|e| matches!(e.attr("w:val"), Some("0" | "false" | "off")));
                v.help = ff_val(ff, "w:helpText");
                v.status = ff_val(ff, "w:statusText");
                if let Some(t) = ff.child("w:textInput") {
                    v.text_type = t.child("w:type").and_then(|e| e.attr("w:val")).map(String::from);
                    v.default = t.child("w:default").and_then(|e| e.attr("w:val")).map(String::from);
                    v.max_length = t.child("w:maxLength").and_then(|e| e.attr("w:val")).and_then(|x| x.parse().ok()).filter(|n| *n > 0);
                    v.format = t.child("w:format").and_then(|e| e.attr("w:val")).map(String::from);
                }
                if let Some(c) = ff.child("w:checkBox") {
                    v.checked = Some(c.child("w:checked").is_some_and(|e| !matches!(e.attr("w:val"), Some("0" | "false" | "off"))));
                    v.size = c.child("w:size").and_then(|e| e.attr("w:val")).and_then(|x| x.parse::<f64>().ok()).map(|h| h / 2.0);
                    v.default_checked = c.child("w:default").is_some_and(|e| matches!(e.attr("w:val"), Some("1" | "true" | "on")));
                }
                if let Some(d) = ff.child("w:ddList") {
                    v.items = d.children_named("w:listEntry").filter_map(|e| e.attr("w:val")).map(String::from).collect();
                    v.selected = d.child("w:result").and_then(|e| e.attr("w:val")).and_then(|x| x.parse().ok()).or(if v.items.is_empty() { None } else { Some(0) });
                }
            } else if f.kind == "checkbox" {
                v.checked = Some(false);
            }
            v
        })
        .collect()
}

/// The `w:ffData` of field `n`, for changing.
pub fn ff_mut(inlines: &mut [Inline], n: usize) -> Option<&mut Element> {
    let found = find(inlines);
    let f = found.get(n)?;
    let run = run_mut(inlines, f.begin.0)?;
    match run.items.get_mut(f.begin.1)? {
        RunItem::Hidden(e) => {
            if e.child("w:ffData").is_none() {
                e.children.insert(0, Node::Element(Element::new("w:ffData")));
            }
            e.child_mut("w:ffData")
        }
        _ => None,
    }
}

pub fn set_checked(ff: &mut Element, checked: bool) {
    let cb = match ff.child_mut("w:checkBox") {
        Some(c) => c,
        None => return,
    };
    cb.remove_children("w:checked");
    if checked {
        cb.children.push(Node::Element(Element::new("w:checked")));
    }
}

pub fn set_selected(ff: &mut Element, index: usize) {
    if let Some(d) = ff.child_mut("w:ddList") {
        let order = ["w:result", "w:default", "w:listEntry"];
        crate::props::put_ordered(d, Element::new("w:result").with_attr("w:val", index.to_string()), &order);
    }
}

const FF_ORDER: &[&str] = &["w:name", "w:label", "w:tabIndex", "w:enabled", "w:calcOnExit", "w:entryMacro", "w:exitMacro", "w:helpText", "w:statusText", "w:ddList", "w:textInput", "w:checkBox"];

/// Apply properties to a `w:ffData`.
pub fn patch(ff: &mut Element, p: &FormProps) -> Result<(), String> {
    let set = |ff: &mut Element, name: &'static str, v: &Option<String>| {
        if let Some(v) = v {
            if v.is_empty() {
                ff.remove_children(name);
            } else {
                crate::props::put_ordered(ff, Element::new(name).with_attr("w:val", v), FF_ORDER);
            }
        }
    };
    set(ff, "w:name", &p.name);
    set(ff, "w:helpText", &p.help);
    set(ff, "w:statusText", &p.status);
    if let Some(d) = p.disabled {
        ff.remove_children("w:enabled");
        if !d {
            crate::props::put_ordered(ff, Element::new("w:enabled"), FF_ORDER);
        } else {
            crate::props::put_ordered(ff, Element::new("w:enabled").with_attr("w:val", "0"), FF_ORDER);
        }
    }
    if let Some(t) = ff.child_mut("w:textInput") {
        let order = ["w:type", "w:default", "w:maxLength", "w:format"];
        let put = |t: &mut Element, name: &'static str, v: Option<String>| {
            t.remove_children(name);
            if let Some(v) = v.filter(|v| !v.is_empty()) {
                crate::props::put_ordered(t, Element::new(name).with_attr("w:val", v), &order);
            }
        };
        if let Some(ty) = &p.text_type {
            put(t, "w:type", (ty != "regular").then(|| ty.clone()));
        }
        if let Some(d) = &p.default {
            put(t, "w:default", Some(d.clone()));
        }
        if let Some(m) = p.max_length {
            put(t, "w:maxLength", (m > 0).then(|| m.to_string()));
        }
        if let Some(f) = &p.format {
            put(t, "w:format", Some(f.clone()));
        }
    }
    if let Some(c) = ff.child_mut("w:checkBox") {
        let order = ["w:size", "w:sizeAuto", "w:default", "w:checked"];
        if let Some(sz) = p.size {
            c.remove_children("w:size");
            c.remove_children("w:sizeAuto");
            if sz > 0.0 {
                crate::props::put_ordered(c, Element::new("w:size").with_attr("w:val", ((sz * 2.0).round() as i64).to_string()), &order);
            } else {
                crate::props::put_ordered(c, Element::new("w:sizeAuto"), &order);
            }
        }
        if let Some(d) = p.default_checked {
            crate::props::put_ordered(c, Element::new("w:default").with_attr("w:val", if d { "1" } else { "0" }), &order);
        }
    }
    if let Some(items) = &p.items {
        let d = ff.child_mut("w:ddList").ok_or("only a drop-down field has entries")?;
        d.remove_children("w:listEntry");
        for it in items {
            d.children.push(Node::Element(Element::new("w:listEntry").with_attr("w:val", it)));
        }
        let keep = d.child("w:result").and_then(|e| e.attr("w:val")).and_then(|v| v.parse::<usize>().ok()).unwrap_or(0);
        if keep >= items.len() {
            set_selected(ff, 0);
        }
    }
    Ok(())
}

/// Insert a field's runs at an offset. A field that ends right there is closed first, so a new field goes after
/// it and not inside its result.
pub fn insert_at(inlines: &mut Vec<Inline>, off: usize, new: Vec<Inline>) {
    crate::edit::split_at(inlines, off);
    let mut start = 0;
    for i in 0..inlines.len() {
        let len = inlines[i].len16();
        if start == off {
            let mut at = i;
            while at < inlines.len() && inlines[at].len16() == 0 && matches!(&inlines[at], Inline::Run(r) if !r.items.is_empty() && r.items.iter().all(|it| matches!(it, RunItem::Hidden(e) if e.name == "w:fldChar" && e.attr("w:fldCharType") == Some("end")))) {
                at += 1;
            }
            for (k, n) in new.into_iter().enumerate() {
                inlines.insert(at + k, n);
            }
            return;
        }
        if start < off && off < start + len {
            if let Inline::Container(c) = &mut inlines[i] {
                insert_at(&mut c.children, off - start, new);
                return;
            }
        }
        start += len;
    }
    inlines.extend(new);
}

/// A new form field as Word writes it, one run per part.
pub fn build(kind: &str, p: &FormProps, name: &str) -> Result<Vec<Inline>, String> {
    let mut ff = Element::new("w:ffData")
        .with_child(Element::new("w:name").with_attr("w:val", name))
        .with_child(Element::new("w:enabled"))
        .with_child(Element::new("w:calcOnExit").with_attr("w:val", "0"));
    let (instr, with_result) = match kind {
        "text" => {
            ff.children.push(Node::Element(Element::new("w:textInput")));
            (" FORMTEXT ", true)
        }
        "checkbox" => {
            ff.children.push(Node::Element(Element::new("w:checkBox").with_child(Element::new("w:sizeAuto")).with_child(Element::new("w:default").with_attr("w:val", "0"))));
            (" FORMCHECKBOX ", false)
        }
        "dropdown" => {
            ff.children.push(Node::Element(Element::new("w:ddList")));
            (" FORMDROPDOWN ", false)
        }
        other => return Err(format!("unknown form field kind '{other}'")),
    };
    let mut props = p.clone();
    props.name = None;
    if kind == "dropdown" && props.items.is_none() {
        props.items = Some(Vec::new());
    }
    patch(&mut ff, &props)?;
    if kind == "dropdown" {
        set_selected(&mut ff, 0);
    }
    let run = |item: RunItem| Inline::Run(Run { attrs: vec![], t_attrs: None, rpr: None, items: vec![item] });
    let mut begin = Element::new("w:fldChar").with_attr("w:fldCharType", "begin");
    begin.children.push(Node::Element(ff));
    let mut out = vec![
        run(RunItem::Hidden(begin)),
        run(RunItem::Hidden(Element::new("w:instrText").with_attr("xml:space", "preserve").with_text(instr))),
        run(RunItem::Hidden(Element::new("w:fldChar").with_attr("w:fldCharType", "separate"))),
    ];
    if with_result {
        let text = p.default.clone().filter(|d| !d.is_empty()).unwrap_or_else(|| EMPTY_RESULT.to_string());
        out.push(Inline::Run(Run::of_text(None, &text)));
    }
    out.push(run(RunItem::Hidden(Element::new("w:fldChar").with_attr("w:fldCharType", "end"))));
    Ok(out)
}

/// Apply Word's text field format to what was typed: `UPPERCASE`, `LOWERCASE`, `First capital`, `Title case`.
pub fn apply_format(text: &str, format: &str) -> String {
    match format.to_lowercase().as_str() {
        "uppercase" => text.to_uppercase(),
        "lowercase" => text.to_lowercase(),
        "first capital" => {
            let mut c = text.chars();
            match c.next() {
                Some(f) => f.to_uppercase().collect::<String>() + &c.as_str().to_lowercase(),
                None => String::new(),
            }
        }
        "title case" => {
            let mut out = String::new();
            let mut start = true;
            for ch in text.chars() {
                if ch.is_alphanumeric() {
                    if start {
                        out.extend(ch.to_uppercase());
                    } else {
                        out.extend(ch.to_lowercase());
                    }
                    start = false;
                } else {
                    start = true;
                    out.push(ch);
                }
            }
            out
        }
        _ => text.to_string(),
    }
}

impl FormOp {
    /// The flags of the op (see `OpClass`): every form field op edits the text it is in.
    pub fn class(&self) -> crate::ops::OpClass {
        match self {
            FormOp::Insert { .. } | FormOp::Set { .. } | FormOp::Props { .. } => crate::ops::OpClass::BODY,
        }
    }

    pub fn anchor(&self) -> Option<NodeId> {
        match self {
            FormOp::Insert { at, .. } | FormOp::Set { at, .. } | FormOp::Props { at, .. } => Some(at.p.0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_built_field_reads_back() {
        let mut inl = vec![Inline::Run(Run { attrs: vec![], t_attrs: None, rpr: None, items: vec![RunItem::Text("Name: ".into())] })];
        inl.extend(build("text", &FormProps { max_length: Some(10), ..Default::default() }, "Text1").unwrap());
        inl.extend(build("checkbox", &FormProps::default(), "Check1").unwrap());
        inl.extend(build("dropdown", &FormProps { items: Some(vec!["a".into(), "b".into()]), ..Default::default() }, "Drop1").unwrap());
        let v = views(&inl);
        assert_eq!(v.len(), 3);
        assert_eq!((v[0].kind.as_str(), v[0].max_length, v[0].at, v[0].start, v[0].end), ("text", Some(10), 6, 6, 11));
        assert_eq!((v[1].kind.as_str(), v[1].checked), ("checkbox", Some(false)));
        assert_eq!((v[2].kind.as_str(), v[2].items.len(), v[2].selected), ("dropdown", 2, Some(0)));
        set_checked(ff_mut(&mut inl, 1).unwrap(), true);
        set_selected(ff_mut(&mut inl, 2).unwrap(), 1);
        let v = views(&inl);
        assert_eq!(v[1].checked, Some(true));
        assert_eq!(v[2].selected, Some(1));
    }

    #[test]
    fn formats() {
        assert_eq!(apply_format("hello WORLD", "Title case"), "Hello World");
        assert_eq!(apply_format("hello", "UPPERCASE"), "HELLO");
        assert_eq!(apply_format("hELLO", "First capital"), "Hello");
    }
}
