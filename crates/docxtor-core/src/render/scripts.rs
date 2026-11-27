//! Scripts and fields: complex-script and East Asian runs, field state and symbols.

use super::*;

/// Whether a character belongs to a complex script: the right-to-left scripts and the Indic, Thai, Lao, Tibetan,
/// Myanmar and Khmer ones, which Word formats with the complex-script font properties.
pub fn is_complex(c: char) -> bool {
    matches!(c as u32,
        0x0590..=0x08FF | 0x0900..=0x0DFF | 0x0E00..=0x0FFF | 0x1000..=0x109F | 0x1780..=0x17FF | 0xFB1D..=0xFDFF | 0xFE70..=0xFEFF
    )
}

/// The kind of script a stretch of text is in, which decides the font, size, bold and italic it takes from a run.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Script {
    Complex,
    EastAsian,
    Other,
}

/// Split text into stretches by script: complex script (right to left and Indic), East Asian, and the rest. Neutral
/// characters (spaces, digits, punctuation) go with the stretch before them, except that ASCII after East Asian text
/// (other than white space) is Latin: Word sets it in the Latin font. With `all` (a run marked right to left) it is all one complex stretch.
/// Text with no East Asian characters and no complex script is one piece.
pub(super) fn script_pieces(t: &str, all: bool) -> Vec<(&str, Script)> {
    if all {
        return vec![(t, Script::Complex)];
    }
    if !t.chars().any(|c| is_complex(c) || crate::east_asian::is_east_asian(c)) {
        return vec![(t, Script::Other)];
    }
    let mut out: Vec<(&str, Script)> = Vec::new();
    let mut start = 0;
    let mut cur: Option<Script> = None;
    for (i, c) in t.char_indices() {
        let kind = if is_complex(c) {
            Some(Script::Complex)
        } else if crate::east_asian::is_east_asian(c) {
            Some(Script::EastAsian)
        } else if c.is_alphabetic() || (cur == Some(Script::EastAsian) && c.is_ascii() && !c.is_whitespace()) {
            Some(Script::Other)
        } else {
            None
        };
        if let Some(k) = kind {
            if cur.is_some_and(|x| x != k) {
                out.push((&t[start..i], cur.unwrap()));
                start = i;
            }
            cur = Some(k);
        }
    }
    out.push((&t[start..], cur.unwrap_or(Script::Other)));
    out
}

/// The formatting of complex-script text: its own size, bold, italic and font where the run gives them.
pub(super) fn cs_variant(f: &RunFmt) -> RunFmt {
    let mut g = f.clone();
    g.bold = Some(f.bold_cs.unwrap_or(false));
    g.italic = Some(f.italic_cs.unwrap_or(false));
    if let Some(s) = f.size_cs {
        g.size = Some(s);
    }
    if let Some(font) = &f.font_cs {
        g.font = Some(font.clone());
    }
    strip_cs(g)
}

/// The formatting of East Asian text: marked as such, so the editor draws it in the run's East Asian font.
pub(super) fn ea_variant(f: &RunFmt) -> RunFmt {
    let mut g = strip_cs(f.clone());
    g.ext.ea.ea_text = Some(true);
    g
}

/// The complex-script properties have done their work once the text has the right ones: leave them out of the view model.
pub(super) fn strip_cs(mut f: RunFmt) -> RunFmt {
    f.bold_cs = None;
    f.italic_cs = None;
    f.size_cs = None;
    f.font_cs = None;
    f.font_cs_theme = None;
    f.font_ea_theme = None;
    f
}

#[derive(Default)]
pub(super) struct FieldState {
    pub(super) depth: u32,
    /// The instruction of each open field, outermost first.
    pub(super) instrs: Vec<String>,
    /// The instruction to put on the next piece of a field's result.
    pub(super) pending_fi: Option<String>,
    /// The next piece starts a field's result, so it is not joined to the text before it.
    pub(super) new_field: bool,
    /// Between the start of the outermost field and its result: its code is being read.
    pub(super) code_open: bool,
    pub(super) result_of: Option<String>,
    /// While a `REF`, `PAGEREF` or `NOTEREF` field with `\h` shows its result: `#bookmark`.
    pub(super) link: Option<String>,
    /// Source tags while inside the result of a `CITATION` field.
    pub(super) cite: Option<Vec<String>>,
    /// The next text starts the result of a citation, so it is not joined to the text before it: two
    /// citations of the same sources side by side stay two citations.
    pub(super) cite_new: bool,
    /// The `w:ffData` of the legacy form field being read, and how many form fields ended so far.
    pub(super) ff: Option<Element>,
    pub(super) form_n: usize,
}

/// The internal link (`#bookmark`) a cross-reference field with `\h` makes of its result.
pub(super) fn ref_link(instr: &str) -> Option<String> {
    crate::fields::link_of(instr)
}

/// The elements Word keeps for a page number or a part of today's date, as the value the layout fills in and a
/// stand-in until it does.
pub(super) fn date_element(name: &str) -> Option<(&'static str, &'static str)> {
    Some(match name {
        "w:pgNum" => ("pgNum", "1"),
        "w:dayShort" => ("dayShort", "1"),
        "w:dayLong" => ("dayLong", "Monday"),
        "w:monthShort" => ("monthShort", "Jan"),
        "w:monthLong" => ("monthLong", "January"),
        "w:yearShort" => ("yearShort", "00"),
        "w:yearLong" => ("yearLong", "2000"),
        _ => return None,
    })
}

pub(super) fn field_name(instr: &str) -> String {
    instr.split_whitespace().next().unwrap_or("").to_uppercase()
}

pub(super) fn sym_char(e: &Element) -> String {
    let code = e.attr("w:char").and_then(|c| u32::from_str_radix(c, 16).ok()).unwrap_or(0x25A1);
    let font = e.attr("w:font").unwrap_or("");
    if let Some(c) = crate::symbol_fonts::map_code(font, code) {
        return c.to_string();
    }
    let code = if (0xF000..=0xF0FF).contains(&code) { code - 0xF000 } else { code };
    let mapped = match (font, code) {
        ("Wingdings", 0xFC) => 0x2714,
        ("Wingdings", 0xFB) => 0x2718,
        ("Wingdings", 0xA8) => 0x25FB,
        ("Wingdings", 0x6F) => 0x25A1,
        ("Wingdings", 0x6C) => 0x25CF,
        ("Wingdings", 0xE0) => 0x2192,
        ("Symbol", 0xB7) => 0x2022,
        ("Symbol", 0xAE) => 0x2192,
        ("Symbol", 0xB4) => 0x00D7,
        ("Symbol", c) if (0x41..=0x5A).contains(&c) || (0x61..=0x7A).contains(&c) => greek(c),
        (_, c) => c,
    };
    char::from_u32(mapped).unwrap_or('\u{25A1}').to_string()
}

pub(super) fn greek(c: u32) -> u32 {
    let map = "ΑΒΧΔΕΦΓΗΙϑΚΛΜΝΟΠΘΡΣΤΥςΩΞΨΖ";
    let lower = "αβχδεφγηιϕκλμνοπθρστυϖωξψζ";
    let (s, i) = if c >= 0x61 { (lower, c - 0x61) } else { (map, c - 0x41) };
    s.chars().nth(i as usize).map(|ch| ch as u32).unwrap_or(c)
}
