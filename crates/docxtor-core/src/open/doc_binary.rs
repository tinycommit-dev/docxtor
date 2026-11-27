//! The binary Word format (Word 97 to 2003, .doc) into a fragment.
//!
//! A .doc is a compound file (see `cfb`). The `WordDocument` stream holds the header (the FIB) and the
//! formatting pages; the `0Table` or `1Table` stream holds the piece table that says where the text is,
//! the style sheet, the font table and the list definitions. The text is read through the piece table;
//! character and paragraph formatting come from the formatting pages (CHPX and PAPX) and the style sheet.
//!
//! Kept: text, character formatting (bold, italic, underline, strike, caps, sizes, fonts, colours,
//! highlight, super- and subscript, hidden), paragraph formatting (alignment, indents, spacing, tab stops,
//! keep options, shading), headings, Title and Subtitle (as Word's own styles), bulleted and numbered
//! lists, tables (cell widths, merged cells, borders), hyperlinks, simple fields, footnotes and endnotes,
//! JPEG and PNG pictures in the text, the first section's page size and margins, and the document title,
//! author, subject and keywords. Deleted text of tracked changes is left out.
//!
//! Lost: headers and footers, comments, text boxes and drawn shapes, pictures that float or are
//! metafiles, equations, embedded objects, other styles' own definitions (a paragraph keeps the formatting
//! its style gave it, not the style), columns, section breaks (the first section's page setup is used)
//! and password-protected files (they give an error). Word 6 and 95 files give their text only.

use super::text::{cp1252, Converted, Meta, Page};
use super::{Note, OpenError};
use crate::cfb::CfbReader;
use crate::fragment::{inline_drawing, sniff_image, Builder, FragNote, LevelSpec};
use crate::props::{put_ordered, put_ppr, put_rpr};
use crate::xml::{Element, Node};
use std::collections::HashMap;

const TCPR_ORDER: &[&str] = &["w:cnfStyle", "w:tcW", "w:gridSpan", "w:hMerge", "w:vMerge", "w:tcBorders", "w:shd", "w:noWrap", "w:tcMar", "w:textDirection", "w:tcFitText", "w:vAlign", "w:hideMark"];

fn u16_at(b: &[u8], i: usize) -> usize {
    b.get(i..i + 2).map(|s| u16::from_le_bytes([s[0], s[1]]) as usize).unwrap_or(0)
}

fn i16_at(b: &[u8], i: usize) -> i32 {
    b.get(i..i + 2).map(|s| i16::from_le_bytes([s[0], s[1]]) as i32).unwrap_or(0)
}

fn u32_at(b: &[u8], i: usize) -> usize {
    b.get(i..i + 4).map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]) as usize).unwrap_or(0)
}

fn i32_at(b: &[u8], i: usize) -> i64 {
    b.get(i..i + 4).map(|s| i32::from_le_bytes([s[0], s[1], s[2], s[3]]) as i64).unwrap_or(0)
}

// ---------------------------------------------------------------------------
// The file information block
// ---------------------------------------------------------------------------

struct Fib {
    n_fib: usize,
    encrypted: bool,
    table_1: bool,
    ccp: [usize; 11],
    /// (fc, lcb) pairs of the FibRgFcLcb.
    fclcb: Vec<(usize, usize)>,
}

impl Fib {
    fn parse(wd: &[u8]) -> Option<Fib> {
        if wd.len() < 64 || u16_at(wd, 0) != 0xA5EC {
            return None;
        }
        let flags = u16_at(wd, 10);
        let mut p = 32;
        let csw = u16_at(wd, p);
        p += 2 + csw * 2;
        let cslw = u16_at(wd, p);
        p += 2;
        let mut ccp = [0usize; 11];
        for (i, c) in ccp.iter_mut().enumerate() {
            *c = u32_at(wd, p + (3 + i) * 4);
        }
        p += cslw * 4;
        let cb = u16_at(wd, p);
        p += 2;
        let fclcb = (0..cb).map_while(|i| (p + i * 8 + 8 <= wd.len()).then(|| (u32_at(wd, p + i * 8), u32_at(wd, p + i * 8 + 4)))).collect();
        Some(Fib { n_fib: u16_at(wd, 2), encrypted: flags & 0x100 != 0, table_1: flags & 0x200 != 0, ccp, fclcb })
    }

    fn get(&self, i: usize) -> Option<(usize, usize)> {
        self.fclcb.get(i).copied().filter(|(_, l)| *l > 0)
    }
}

// FibRgFcLcb indexes.
const STSHF: usize = 1;
const FND_REF: usize = 2;
const FND_TXT: usize = 3;
const SED: usize = 6;
const BTE_CHPX: usize = 12;
const BTE_PAPX: usize = 13;
const FFN: usize = 15;
const CLX: usize = 33;
const END_REF: usize = 46;
const END_TXT: usize = 47;
const LST: usize = 73;
const LFO: usize = 74;

// ---------------------------------------------------------------------------
// Properties
// ---------------------------------------------------------------------------

/// Split a property list into (opcode, operand) pairs.
fn sprms(g: &[u8]) -> Vec<(u16, &[u8])> {
    let mut out = Vec::new();
    let mut i = 0;
    while i + 2 <= g.len() {
        let op = u16::from_le_bytes([g[i], g[i + 1]]);
        i += 2;
        let size = match op >> 13 {
            0 | 1 => 1,
            2 | 4 | 5 => 2,
            3 => 4,
            7 => 3,
            _ => {
                if op == 0xD608 {
                    let cb = u16_at(g, i);
                    i += 2;
                    cb.saturating_sub(1)
                } else {
                    let n = g.get(i).copied().unwrap_or(0) as usize;
                    i += 1;
                    n
                }
            }
        };
        if i + size > g.len() {
            break;
        }
        out.push((op, &g[i..i + size]));
        i += size;
    }
    out
}

#[derive(Default, Clone, PartialEq)]
struct Chp {
    bold: Option<bool>,
    italic: Option<bool>,
    strike: Option<bool>,
    caps: Option<bool>,
    small_caps: Option<bool>,
    hidden: Option<bool>,
    underline: Option<u8>,
    /// Half-points.
    size: Option<usize>,
    color: Option<String>,
    highlight: Option<u8>,
    /// 1 superscript, 2 subscript.
    vert: Option<u8>,
    font: Option<usize>,
    istd: Option<usize>,
    deleted: bool,
    special: bool,
    pic: Option<usize>,
    symbol: Option<(usize, usize)>,
}

fn toggle(cur: Option<bool>, op: &[u8]) -> Option<bool> {
    match op.first() {
        Some(0) => Some(false),
        Some(1) => Some(true),
        Some(0x81) => Some(!cur.unwrap_or(false)),
        _ => cur,
    }
}

const ICO: [&str; 17] = ["", "000000", "0000FF", "00FFFF", "00FF00", "FF00FF", "FF0000", "FFFF00", "FFFFFF", "000080", "008080", "008000", "800080", "800000", "808000", "808080", "C0C0C0"];
const HIGHLIGHT: [&str; 17] = ["", "black", "blue", "cyan", "green", "magenta", "red", "yellow", "white", "darkBlue", "darkCyan", "darkGreen", "darkMagenta", "darkRed", "darkYellow", "darkGray", "lightGray"];

impl Chp {
    fn apply(&mut self, g: &[u8]) {
        for (op, v) in sprms(g) {
            match op {
                0x0835 => self.bold = toggle(self.bold, v),
                0x0836 => self.italic = toggle(self.italic, v),
                0x0837 => self.strike = toggle(self.strike, v),
                0x083A => self.small_caps = toggle(self.small_caps, v),
                0x083B => self.caps = toggle(self.caps, v),
                0x083C => self.hidden = toggle(self.hidden, v),
                0x0800 => self.deleted = v.first() == Some(&1),
                0x0855 => self.special = v.first() == Some(&1),
                0x2A3E => self.underline = v.first().copied(),
                0x4A43 => self.size = Some(u16_at(v, 0)),
                0x2A42 => {
                    let i = v.first().copied().unwrap_or(0) as usize;
                    self.color = if i == 0 || i >= ICO.len() { None } else { Some(ICO[i].to_string()) };
                }
                0x6870 => {
                    if v.len() >= 4 {
                        self.color = if v[3] == 0xFF { None } else { Some(format!("{:02X}{:02X}{:02X}", v[0], v[1], v[2])) };
                    }
                }
                0x2A0C => self.highlight = v.first().copied().filter(|i| *i > 0),
                0x2A48 => self.vert = v.first().copied().filter(|i| *i == 1 || *i == 2),
                0x4A4F => self.font = Some(u16_at(v, 0)),
                0x4A30 => self.istd = Some(u16_at(v, 0)),
                0x6A03 => self.pic = Some(u32_at(v, 0)),
                0x6A09 => self.symbol = Some((u16_at(v, 0), u16_at(v, 2))),
                _ => {}
            }
        }
    }

    /// Overlay `top` on `self`: what `top` sets wins.
    fn overlay(&mut self, top: &Chp) {
        macro_rules! take {
            ($($f:ident),*) => { $( if top.$f.is_some() { self.$f = top.$f.clone(); } )* };
        }
        take!(bold, italic, strike, caps, small_caps, hidden, underline, size, color, highlight, vert, font);
    }
}

#[derive(Default, Clone)]
struct Tab {
    pos: i32,
    kind: u8,
    leader: u8,
}

#[derive(Default, Clone)]
struct Pap {
    istd: usize,
    jc: Option<u8>,
    left: Option<i32>,
    right: Option<i32>,
    first: Option<i32>,
    before: Option<usize>,
    after: Option<usize>,
    line: Option<(i32, bool)>,
    keep_next: Option<bool>,
    keep_lines: Option<bool>,
    page_break: Option<bool>,
    in_table: bool,
    ttp: bool,
    ilvl: Option<usize>,
    ilfo: Option<i64>,
    shade: Option<String>,
    tabs: Vec<Tab>,
    tdef: Option<Vec<u8>>,
}

impl Pap {
    fn apply(&mut self, g: &[u8]) {
        for (op, v) in sprms(g) {
            let b = v.first().copied().unwrap_or(0);
            match op {
                0x2403 | 0x2461 => self.jc = Some(b),
                0x2405 => self.keep_lines = Some(b == 1),
                0x2406 => self.keep_next = Some(b == 1),
                0x2407 => self.page_break = Some(b == 1),
                0x840F | 0x845E => self.left = Some(i16_at(v, 0)),
                0x840E | 0x845D => self.right = Some(i16_at(v, 0)),
                0x8411 | 0x8460 => self.first = Some(i16_at(v, 0)),
                0x6412 => self.line = Some((u16_at(v, 0) as i16 as i32, u16_at(v, 2) == 1)),
                0xA413 => self.before = Some(u16_at(v, 0)),
                0xA414 => self.after = Some(u16_at(v, 0)),
                0x2416 => self.in_table = b == 1,
                0x2417 => self.ttp = b == 1,
                0x260A => self.ilvl = Some(b as usize),
                0x460B => self.ilfo = Some(i16_at(v, 0) as i64),
                0x442D => {
                    // Shd80: a pattern and two colours; a solid pattern with a foreground is the fill.
                    let w = u16_at(v, 0);
                    let (fore, back, pat) = (w & 0x1F, (w >> 5) & 0x1F, (w >> 10) & 0x3F);
                    let c = if pat == 0 { back } else { fore };
                    self.shade = (c > 0 && c < ICO.len()).then(|| ICO[c].to_string());
                }
                0xC615 => self.apply_tabs(v),
                0xD608 => self.tdef = Some(v.to_vec()),
                _ => {}
            }
        }
    }

    fn apply_tabs(&mut self, v: &[u8]) {
        // itbdDelMax, the positions to delete (and their closes), then itbdAddMax, positions and descriptors.
        let n_del = v.first().copied().unwrap_or(0) as usize;
        let mut p = 1 + n_del * 4;
        for i in 0..n_del {
            let pos = i16_at(v, 1 + i * 2);
            self.tabs.retain(|t| t.pos != pos);
        }
        let n_add = v.get(p).copied().unwrap_or(0) as usize;
        p += 1;
        for i in 0..n_add {
            let pos = i16_at(v, p + i * 2);
            let d = v.get(p + n_add * 2 + i).copied().unwrap_or(0);
            self.tabs.retain(|t| t.pos != pos);
            self.tabs.push(Tab { pos, kind: d & 7, leader: (d >> 3) & 7 });
        }
        self.tabs.sort_by_key(|t| t.pos);
    }

    fn overlay(&mut self, top: &Pap) {
        macro_rules! take {
            ($($f:ident),*) => { $( if top.$f.is_some() { self.$f = top.$f.clone(); } )* };
        }
        take!(jc, left, right, first, before, after, line, keep_next, keep_lines, page_break, ilvl, ilfo, shade);
        if !top.tabs.is_empty() {
            self.tabs = top.tabs.clone();
        }
    }
}

// ---------------------------------------------------------------------------
// Pieces and formatting runs
// ---------------------------------------------------------------------------

struct Piece {
    cp: usize,
    cp_end: usize,
    fc: usize,
    compressed: bool,
}

struct Runs {
    starts: Vec<usize>,
    ends: Vec<usize>,
    data: Vec<Vec<u8>>,
}

impl Runs {
    fn find(&self, fc: usize) -> Option<&[u8]> {
        let i = self.starts.partition_point(|s| *s <= fc);
        if i == 0 {
            return None;
        }
        (fc < self.ends[i - 1]).then(|| self.data[i - 1].as_slice())
    }
}

struct StyleDef {
    name: String,
    base: Option<usize>,
    pap: Vec<u8>,
    chp: Vec<u8>,
    is_para: bool,
}

struct ListLevel {
    spec: LevelSpec,
}

struct Reader<'a> {
    wd: &'a [u8],
    table: &'a [u8],
    data: Option<Vec<u8>>,
    pieces: Vec<Piece>,
    text: Vec<u16>,
    chp: Runs,
    pap: Runs,
    fonts: Vec<String>,
    styles: Vec<Option<StyleDef>>,
    styles_pap: HashMap<usize, Pap>,
    styles_chp: HashMap<usize, Chp>,
    lists: Vec<Vec<ListLevel>>,
    lfo: Vec<i64>,
    list_nums: HashMap<i64, i64>,
    b: Builder,
    notes: Vec<(String, FragNote)>,
    note_refs: Vec<usize>,
    note_ranges: Vec<(usize, usize)>,
    end_refs: Vec<usize>,
    end_ranges: Vec<(usize, usize)>,
    lost_pictures: usize,
    lost_shapes: usize,
    truncated: bool,
}

fn read_runs(table: &[u8], wd: &[u8], fc: usize, lcb: usize, papx: bool) -> Runs {
    let mut r = Runs { starts: Vec::new(), ends: Vec::new(), data: Vec::new() };
    let Some(plc) = table.get(fc..fc + lcb) else { return r };
    let n = lcb.saturating_sub(4) / 8;
    for i in 0..n {
        let pn = u32_at(plc, (n + 1) * 4 + i * 4) & 0x3F_FFFF;
        let Some(page) = wd.get(pn * 512..pn * 512 + 512) else { continue };
        let crun = page[511] as usize;
        // The run table (and the 13-byte entries of a paragraph page) must fit in the page.
        if (crun + 1) * 4 + crun * if papx { 13 } else { 1 } > 511 {
            continue;
        }
        for k in 0..crun {
            let (a, b) = (u32_at(page, k * 4), u32_at(page, k * 4 + 4));
            if b <= a {
                continue;
            }
            let g: Vec<u8> = if papx {
                let off = page[(crun + 1) * 4 + k * 13] as usize * 2;
                if off == 0 || off >= 511 {
                    Vec::new()
                } else {
                    let cb = page[off] as usize;
                    let (start, len) = if cb == 0 { (off + 2, page.get(off + 1).copied().unwrap_or(0) as usize * 2) } else { (off + 1, (cb * 2).saturating_sub(1)) };
                    page.get(start..(start + len).min(511)).map(|s| s.to_vec()).unwrap_or_default()
                }
            } else {
                let off = page[(crun + 1) * 4 + k] as usize * 2;
                if off == 0 || off >= 511 {
                    Vec::new()
                } else {
                    let cb = page[off] as usize;
                    page.get(off + 1..(off + 1 + cb).min(511)).map(|s| s.to_vec()).unwrap_or_default()
                }
            };
            r.starts.push(a);
            r.ends.push(b);
            r.data.push(g);
        }
    }
    // Pages come in file order, which is also text order, but keep the search honest.
    let mut idx: Vec<usize> = (0..r.starts.len()).collect();
    idx.sort_by_key(|i| r.starts[*i]);
    Runs { starts: idx.iter().map(|i| r.starts[*i]).collect(), ends: idx.iter().map(|i| r.ends[*i]).collect(), data: idx.iter().map(|i| r.data[*i].clone()).collect() }
}

impl<'a> Reader<'a> {
    fn fc_of(&self, cp: usize) -> Option<usize> {
        let i = self.pieces.partition_point(|p| p.cp_end <= cp);
        let p = self.pieces.get(i)?;
        (cp >= p.cp).then(|| p.fc + (cp - p.cp) * if p.compressed { 1 } else { 2 })
    }

    fn font_name(&self, ftc: usize) -> Option<&str> {
        self.fonts.get(ftc).map(String::as_str).filter(|s| !s.is_empty())
    }

    // -- styles -------------------------------------------------------------

    fn load_styles(&mut self, fc: usize, lcb: usize) {
        let Some(s) = self.table.get(fc..fc + lcb) else { return };
        let cb_stshi = u16_at(s, 0);
        let cstd = u16_at(s, 2);
        let base_len = u16_at(s, 4);
        let mut p = 2 + cb_stshi;
        let mut defs = Vec::with_capacity(cstd);
        for _ in 0..cstd {
            let cb = u16_at(s, p);
            p += 2;
            if cb == 0 || p + cb > s.len() {
                defs.push(None);
                p += cb;
                continue;
            }
            let std = &s[p..p + cb];
            p += cb + (cb & 1);
            let w1 = u16_at(std, 2);
            let w2 = u16_at(std, 4);
            let stk = w1 & 0xF;
            let base = w1 >> 4;
            let cupx = w2 & 0xF;
            let mut q = base_len;
            let n = u16_at(std, q);
            let name: String = (0..n).filter_map(|i| std.get(q + 2 + i * 2..q + 4 + i * 2)).map(|c| u16::from_le_bytes([c[0], c[1]])).collect::<Vec<u16>>().iter().map(|u| char::from_u32(*u as u32).unwrap_or('?')).collect();
            q += 2 + n * 2 + 2;
            q += q & 1;
            let mut upx = Vec::new();
            for _ in 0..cupx {
                let l = u16_at(std, q);
                q += 2;
                upx.push(std.get(q..(q + l).min(std.len())).unwrap_or(&[]).to_vec());
                q += l + (l & 1);
            }
            let is_para = stk == 1;
            let (pap, chp) = if is_para {
                (upx.first().map(|u| u.get(2..).unwrap_or(&[]).to_vec()).unwrap_or_default(), upx.get(1).cloned().unwrap_or_default())
            } else {
                (Vec::new(), upx.first().cloned().unwrap_or_default())
            };
            defs.push(Some(StyleDef { name, base: (base != 0x0FFF).then_some(base), pap, chp, is_para }));
        }
        self.styles = defs;
    }

    fn style_chp(&mut self, istd: usize, depth: usize) -> Chp {
        if let Some(c) = self.styles_chp.get(&istd) {
            return c.clone();
        }
        let mut out = Chp::default();
        let Some(Some(def)) = self.styles.get(istd) else { return out };
        let (base, chp) = (def.base, def.chp.clone());
        if let (Some(b), true) = (base, depth < 20) {
            out = self.style_chp(b, depth + 1);
        }
        out.apply(&chp);
        self.styles_chp.insert(istd, out.clone());
        out
    }

    fn style_pap(&mut self, istd: usize, depth: usize) -> Pap {
        if let Some(c) = self.styles_pap.get(&istd) {
            return c.clone();
        }
        let mut out = Pap::default();
        let Some(Some(def)) = self.styles.get(istd) else { return out };
        let (base, pap) = (def.base, def.pap.clone());
        if let (Some(b), true) = (base, depth < 20) {
            out = self.style_pap(b, depth + 1);
        }
        out.apply(&pap);
        self.styles_pap.insert(istd, out.clone());
        out
    }

    /// The Word style a paragraph style stands for (a heading, Title or Subtitle), as its id.
    fn word_style(&mut self, istd: usize) -> Option<String> {
        let name = self.styles.get(istd)?.as_ref().filter(|d| d.is_para)?.name.clone();
        let l = name.to_lowercase();
        let known = (l.starts_with("heading ") && l[8..].parse::<u8>().is_ok_and(|n| (1..=6).contains(&n))) || l == "title" || l == "subtitle";
        known.then(|| self.b.builtin_style(&name)).flatten()
    }

    fn char_style_id(&mut self, istd: usize) -> Option<String> {
        let def = self.styles.get(istd)?.as_ref().filter(|d| !d.is_para)?;
        let l = def.name.to_lowercase();
        let want = match l.as_str() {
            "hyperlink" => "Hyperlink",
            "strong" => "Strong",
            "emphasis" => "Emphasis",
            _ => return None,
        };
        self.b.builtin_style(want)
    }

    // -- fonts --------------------------------------------------------------

    fn load_fonts(&mut self, fc: usize, lcb: usize) {
        let Some(s) = self.table.get(fc..fc + lcb) else { return };
        let n = u16_at(s, 0);
        let mut p = 4;
        for _ in 0..n {
            let cb = s.get(p).copied().unwrap_or(0) as usize;
            if cb == 0 || p + 1 + cb > s.len() {
                break;
            }
            let ffn = &s[p + 1..p + 1 + cb];
            let units: Vec<u16> = ffn.get(39..).unwrap_or(&[]).chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).take_while(|u| *u != 0).collect();
            self.fonts.push(String::from_utf16_lossy(&units));
            p += 1 + cb;
        }
    }

    // -- lists --------------------------------------------------------------

    fn load_lists(&mut self, lst: Option<(usize, usize)>, lfo: Option<(usize, usize)>) {
        let (Some((fc, lcb)), Some((lfc, llcb))) = (lst, lfo) else { return };
        let Some(s) = self.table.get(fc..) else { return };
        let c_lst = u16_at(s, 0);
        // The LVLs follow the LSTF array.
        let mut p = 2 + c_lst * 28;
        let _ = lcb;
        let mut lsids = Vec::new();
        let mut lists: Vec<Vec<ListLevel>> = Vec::new();
        for i in 0..c_lst {
            let lsid = i32_at(s, 2 + i * 28);
            lsids.push(lsid);
            let simple = s.get(2 + i * 28 + 26).copied().unwrap_or(0) & 1 == 1;
            let n_lvl = if simple { 1 } else { 9 };
            let mut levels = Vec::new();
            for _ in 0..n_lvl {
                if p + 28 > s.len() {
                    break;
                }
                let start = i32_at(s, p);
                let nfc = s[p + 4] as usize;
                let cb_chpx = s[p + 24] as usize;
                let cb_papx = s[p + 25] as usize;
                let papx = s.get(p + 28..p + 28 + cb_papx).unwrap_or(&[]).to_vec();
                let chpx = s.get(p + 28 + cb_papx..p + 28 + cb_papx + cb_chpx).unwrap_or(&[]).to_vec();
                let q = p + 28 + cb_papx + cb_chpx;
                let cch = u16_at(s, q);
                let units: Vec<u16> = (0..cch).filter_map(|k| s.get(q + 2 + k * 2..q + 4 + k * 2)).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
                p = q + 2 + cch * 2;
                let mut pap = Pap::default();
                pap.apply(&papx);
                let mut chp = Chp::default();
                chp.apply(&chpx);
                levels.push(ListLevel { spec: self.level_spec(nfc, start as i64, &units, &pap, &chp, levels.len()) });
            }
            lists.push(levels);
        }
        self.lists = lists;
        // PlfLfo: the count, then 16 bytes per LFO; each names a list by its lsid.
        if let Some(l) = self.table.get(lfc..lfc + llcb) {
            let n = u32_at(l, 0);
            for i in 0..n.min(4000) {
                let lsid = i32_at(l, 4 + i * 16);
                self.lfo.push(lsids.iter().position(|x| *x == lsid).map(|x| x as i64).unwrap_or(-1));
            }
        }
    }

    fn level_spec(&self, nfc: usize, start: i64, xst: &[u16], pap: &Pap, chp: &Chp, level: usize) -> LevelSpec {
        let left = pap.left.map(|v| v as i64).unwrap_or(720 * (level as i64 + 1));
        let hanging = pap.first.map(|v| (-v as i64).max(0)).unwrap_or(360);
        if nfc == 23 {
            let c = xst.first().copied().unwrap_or(0xF0B7);
            let font = chp.font.and_then(|f| self.font_name(f)).map(String::from);
            let glyph = match &font {
                Some(f) => crate::symbol_fonts::map_code(f, c as u32).map(|ch| ch.to_string()),
                None => None,
            };
            let text = glyph.unwrap_or_else(|| if (0xF000..=0xF0FF).contains(&c) { "\u{2022}".into() } else { char::from_u32(c as u32).map(|ch| ch.to_string()).unwrap_or_else(|| "\u{2022}".into()) });
            return LevelSpec { fmt: "bullet".into(), text, start: 1, left, hanging, font: Some("Arial".into()) };
        }
        let fmt = match nfc {
            1 => "upperRoman",
            2 => "lowerRoman",
            3 => "upperLetter",
            4 => "lowerLetter",
            255 => "none",
            _ => "decimal",
        };
        // Placeholders are characters 0 to 8 standing for the level numbers.
        let text: String = xst.iter().map(|&u| if u < 9 { format!("%{}", u + 1) } else { char::from_u32(u as u32).map(|c| c.to_string()).unwrap_or_default() }).collect();
        LevelSpec { fmt: fmt.into(), text, start: start.max(0), left, hanging, font: None }
    }

    fn list_num(&mut self, ilfo: i64) -> Option<i64> {
        if ilfo <= 0 {
            return None;
        }
        if let Some(n) = self.list_nums.get(&ilfo) {
            return Some(*n);
        }
        let lst = *self.lfo.get(ilfo as usize - 1)?;
        let levels: Vec<LevelSpec> = self.lists.get(usize::try_from(lst).ok()?)?.iter().map(|l| l.spec.clone()).collect();
        if levels.is_empty() {
            return None;
        }
        let n = self.b.list(&levels);
        self.list_nums.insert(ilfo, n);
        Some(n)
    }
}

// ---------------------------------------------------------------------------
// Reading the file
// ---------------------------------------------------------------------------

pub fn read(bytes: &[u8]) -> Result<Converted, OpenError> {
    let cfb = CfbReader::new(bytes).map_err(|e| OpenError::new("damaged", e.to_string()))?;
    let wd = cfb.stream("WordDocument").ok().flatten().ok_or_else(|| OpenError::new("damaged", "The Word document stream is missing."))?;
    let fib = Fib::parse(&wd).ok_or_else(|| OpenError::new("damaged", "The Word file's header can't be read."))?;
    if fib.encrypted {
        return Err(OpenError::new("doc-encrypted", "This old-format Word file is protected with a password."));
    }
    let meta = summary(&cfb);
    if fib.n_fib < 0xC1 {
        return Ok(word6(&wd, meta));
    }
    let table = cfb
        .stream(if fib.table_1 { "1Table" } else { "0Table" })
        .ok()
        .flatten()
        .ok_or_else(|| OpenError::new("damaged", "The Word file's table stream is missing."))?;
    let (clx_fc, clx_lcb) = fib.get(CLX).ok_or_else(|| OpenError::new("damaged", "The Word file has no piece table."))?;
    let clx = table.get(clx_fc..clx_fc + clx_lcb).ok_or_else(|| OpenError::new("damaged", "The Word file's piece table is cut short."))?;
    let pieces = read_pieces(clx).ok_or_else(|| OpenError::new("damaged", "The Word file's piece table can't be read."))?;

    let mut r = Reader {
        wd: &wd,
        table: &table,
        data: cfb.stream("Data").ok().flatten(),
        pieces,
        text: Vec::new(),
        chp: Runs { starts: vec![], ends: vec![], data: vec![] },
        pap: Runs { starts: vec![], ends: vec![], data: vec![] },
        fonts: Vec::new(),
        styles: Vec::new(),
        styles_pap: HashMap::new(),
        styles_chp: HashMap::new(),
        lists: Vec::new(),
        lfo: Vec::new(),
        list_nums: HashMap::new(),
        b: Builder::new(),
        notes: Vec::new(),
        note_refs: Vec::new(),
        note_ranges: Vec::new(),
        end_refs: Vec::new(),
        end_ranges: Vec::new(),
        lost_pictures: 0,
        lost_shapes: 0,
        truncated: false,
    };
    r.build_text();
    if let Some((fc, l)) = fib.get(BTE_CHPX) {
        r.chp = read_runs(&table, &wd, fc, l, false);
    }
    if let Some((fc, l)) = fib.get(BTE_PAPX) {
        r.pap = read_runs(&table, &wd, fc, l, true);
    }
    if let Some((fc, l)) = fib.get(FFN) {
        r.load_fonts(fc, l);
    }
    if let Some((fc, l)) = fib.get(STSHF) {
        r.load_styles(fc, l);
    }
    r.load_lists(fib.get(LST), fib.get(LFO));

    // Notes first, so the references in the text can name them.
    let ftn_start = fib.ccp[0];
    // Stories follow one another: text, footnotes, headers and footers, (one unused), comments, endnotes, text boxes.
    let edn_start = fib.ccp[0] + fib.ccp[1] + fib.ccp[2] + fib.ccp[4];
    r.load_notes(fib.get(FND_REF), fib.get(FND_TXT), ftn_start, false);
    r.load_notes(fib.get(END_REF), fib.get(END_TXT), edn_start, true);

    let blocks = r.story(0, fib.ccp[0].min(r.text.len()));
    for e in blocks {
        r.b.block(e);
    }
    let mut notes = Vec::new();
    if r.truncated {
        notes.push(Note::new("text-cut"));
    }
    if r.lost_pictures > 0 {
        notes.push(Note::count("picture-unsupported", r.lost_pictures));
    }
    if r.lost_shapes > 0 {
        notes.push(Note::count("lost-drawings", r.lost_shapes));
    }
    let has_text = |a: usize, n: usize| r.text.get(a..(a + n).min(r.text.len())).is_some_and(|s| s.iter().any(|u| *u > 0x20));
    if has_text(fib.ccp[0] + fib.ccp[1], fib.ccp[2]) {
        notes.push(Note::new("lost-headers"));
    }
    if has_text(fib.ccp[0] + fib.ccp[1] + fib.ccp[2], fib.ccp[4] + fib.ccp[3]) {
        notes.push(Note::new("lost-comments"));
    }
    if has_text(edn_start + fib.ccp[5], fib.ccp[6] + fib.ccp[7]) {
        notes.push(Note::new("lost-text-boxes"));
    }
    let page = fib.get(SED).and_then(|(fc, l)| page_of(&table, &wd, fc, l));
    let mut b = r.b;
    b.set_block(true);
    let mut fragment = b.finish();
    for (k, n) in r.notes {
        fragment.notes.insert(k, n);
    }
    // The Normal style's font and size are the document's defaults.
    let normal = r.styles.first().and_then(|d| d.as_ref()).map(|d| d.chp.clone()).unwrap_or_default();
    let mut c = Chp::default();
    c.apply(&normal);
    let font = c.font.or(Some(0)).and_then(|f| r.fonts.get(f).cloned()).filter(|f| !f.is_empty());
    let size = Some(c.size.unwrap_or(20) as f64 / 2.0);
    fragment.defaults = Some(crate::fragment::FragDefaults { font, size });
    Ok(Converted { fragment, meta, page, notes })
}

/// Word 6 and 95: the text sits between two offsets in the main stream, without a piece table.
fn word6(wd: &[u8], meta: Meta) -> Converted {
    let (min, mac) = (u32_at(wd, 0x18), u32_at(wd, 0x1C));
    let text: String = wd.get(min..mac.min(wd.len())).unwrap_or(&[]).iter().map(|&b| cp1252(b)).collect();
    let mut b = Builder::new();
    for line in text.split('\r') {
        let clean: String = line.chars().filter(|c| !c.is_control() || *c == '\t').collect();
        let mut p = Element::new("w:p");
        if !clean.is_empty() {
            p.children.push(Node::Element(Element::new("w:r").with_child(Element::new("w:t").with_attr("xml:space", "preserve").with_text(clean))));
        }
        b.block(p);
    }
    b.set_block(true);
    Converted { fragment: b.finish(), meta, page: None, notes: vec![Note::new("doc-word6")] }
}

fn read_pieces(clx: &[u8]) -> Option<Vec<Piece>> {
    let mut p = 0;
    while clx.get(p) == Some(&1) {
        p += 3 + u16_at(clx, p + 1);
    }
    if clx.get(p) != Some(&2) {
        return None;
    }
    let lcb = u32_at(clx, p + 1);
    let plc = clx.get(p + 5..p + 5 + lcb)?;
    let n = (lcb.saturating_sub(4)) / 12;
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let (cp, cp_end) = (u32_at(plc, i * 4), u32_at(plc, i * 4 + 4));
        let pcd = (n + 1) * 4 + i * 8;
        let raw = u32_at(plc, pcd + 2);
        let compressed = raw & 0x4000_0000 != 0;
        let fc = if compressed { (raw & !0x4000_0000) / 2 } else { raw };
        if cp_end > cp {
            out.push(Piece { cp, cp_end, fc, compressed });
        }
    }
    (!out.is_empty()).then_some(out)
}

fn page_of(table: &[u8], wd: &[u8], fc: usize, lcb: usize) -> Option<Page> {
    let plc = table.get(fc..fc + lcb)?;
    let n = (lcb.saturating_sub(4)) / 16;
    if n == 0 {
        return None;
    }
    let sepx = u32_at(plc, (n + 1) * 4 + 2) ;
    if sepx == 0xFFFF_FFFF || sepx >= wd.len() {
        return None;
    }
    let cb = u16_at(wd, sepx);
    let g = wd.get(sepx + 2..sepx + 2 + cb)?;
    let (mut w, mut h, mut l, mut r, mut t, mut b, mut land) = (12240i64, 15840i64, 1800i64, 1800i64, 1440i64, 1440i64, false);
    for (op, v) in sprms(g) {
        match op {
            0xB01F => w = u16_at(v, 0) as i64,
            0xB020 => h = u16_at(v, 0) as i64,
            0xB021 => l = u16_at(v, 0) as i64,
            0xB022 => r = u16_at(v, 0) as i64,
            0x9023 => t = i16_at(v, 0).abs() as i64,
            0x9024 => b = i16_at(v, 0).abs() as i64,
            0x301D => land = v.first() == Some(&2),
            _ => {}
        }
    }
    if land && w < h {
        std::mem::swap(&mut w, &mut h);
    }
    (w >= 1000 && h >= 1000).then_some(Page { width: w, height: h, margins: [t, r, b, l] })
}

/// The document information stream: title, subject, author, keywords and comments.
fn summary(cfb: &CfbReader) -> Meta {
    let mut m = Meta::default();
    let Some(s) = cfb.stream("\u{5}SummaryInformation").ok().flatten() else { return m };
    let off = u32_at(&s, 44);
    let Some(set) = s.get(off..) else { return m };
    let n = u32_at(set, 4);
    for i in 0..n.min(40) {
        let (id, po) = (u32_at(set, 8 + i * 8), u32_at(set, 12 + i * 8));
        let Some(v) = set.get(po..) else { continue };
        let ty = u32_at(v, 0) & 0xFFFF;
        let len = u32_at(v, 4);
        let text = match ty {
            0x1E => v.get(8..8 + len).map(|b| b.iter().take_while(|c| **c != 0).map(|&c| cp1252(c)).collect::<String>()),
            0x1F => v.get(8..8 + len * 2).map(|b| String::from_utf16_lossy(&b.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).take_while(|u| *u != 0).collect::<Vec<_>>())),
            _ => None,
        }
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty());
        match id {
            2 => m.title = text,
            3 => m.subject = text,
            4 => m.author = text,
            5 => m.keywords = text,
            6 => m.description = text,
            _ => {}
        }
    }
    m
}

impl<'a> Reader<'a> {
    /// All the document's text as UTF-16 units, one per character position.
    fn build_text(&mut self) {
        // A character takes at least a byte of the file, so a piece table can't honestly claim more than that.
        let total = self.pieces.last().map(|p| p.cp_end).unwrap_or(0).min(self.wd.len());
        self.text = vec![0; total];
        for p in &self.pieces {
            if p.cp >= self.text.len() {
                continue;
            }
            let n = (p.cp_end - p.cp).min(self.text.len() - p.cp);
            for k in 0..n {
                let u = if p.compressed {
                    match self.wd.get(p.fc + k) {
                        Some(&b) => cp1252(b) as u32 as u16,
                        None => {
                            self.truncated = true;
                            0
                        }
                    }
                } else {
                    match self.wd.get(p.fc + k * 2..p.fc + k * 2 + 2) {
                        Some(c) => u16::from_le_bytes([c[0], c[1]]),
                        None => {
                            self.truncated = true;
                            0
                        }
                    }
                };
                self.text[p.cp + k] = u;
            }
        }
    }

    fn load_notes(&mut self, refs: Option<(usize, usize)>, txt: Option<(usize, usize)>, story_start: usize, end: bool) {
        let (Some((rf, rl)), Some((tf, tl))) = (refs, txt) else { return };
        let (Some(r), Some(t)) = (self.table.get(rf..rf + rl), self.table.get(tf..tf + tl)) else { return };
        let n = rl.saturating_sub(4) / 6;
        let cps: Vec<usize> = (0..n).map(|i| u32_at(r, i * 4)).collect();
        let tcps: Vec<usize> = (0..=n).map(|i| u32_at(t, i * 4)).collect();
        let ranges: Vec<(usize, usize)> = (0..n).map(|i| (story_start + tcps[i], story_start + tcps[i + 1])).collect();
        if end {
            self.end_refs = cps;
            self.end_ranges = ranges;
        } else {
            self.note_refs = cps;
            self.note_ranges = ranges;
        }
    }
}

#[path = "doc_story.rs"]
mod story;
