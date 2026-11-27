//! Styles, lists and the final assembly of paragraphs and the fragment.

use crate::fragment::{Fragment, LevelSpec, NAMESPACES};
use crate::props::{put_ppr, put_rpr};
use crate::xml::{Element, Node};
use super::{codepages::*, state::*};

impl Interp {
    pub(super) fn color_hex(&self, idx: i64) -> Option<String> {
        if idx < 0 {
            return None;
        }
        self.colors.get(idx as usize).copied().flatten().map(hex_of)
    }

    pub(super) fn font_name(&self, id: i64) -> Option<String> {
        self.fonts.get(&id).map(|f| f.name.trim().to_string()).filter(|n| !n.is_empty())
    }

    /// A paragraph style's own formatting laid over the styles it builds on.
    pub(super) fn eff_style(&mut self, n: i64, depth: usize) -> (CharFmt, ParaFmt) {
        if let Some(v) = self.eff.get(&n) {
            return v.clone();
        }
        let Some(def) = self.pstyles.get(&n) else { return (CharFmt::default(), ParaFmt::default()) };
        let (own_c, own_p, based) = (def.ch.clone(), def.pf.clone(), def.based);
        let (bc, bp) = match based {
            Some(b) if b != n && depth < 16 => self.eff_style(b, depth + 1),
            _ => (CharFmt::default(), ParaFmt::default()),
        };
        let r = (bc.over(&own_c), bp.over(&own_p));
        self.eff.insert(n, r.clone());
        r
    }

    /// What a paragraph in this style gets before any direct formatting.
    pub(super) fn para_base(&mut self, style: Option<i64>) -> (CharFmt, ParaFmt) {
        let n = style.unwrap_or(0);
        if n != 0 && !self.pstyles.contains_key(&n) {
            return self.eff_style(0, 0);
        }
        self.eff_style(n, 0)
    }

    pub(super) fn run_base(&mut self, pstyle: Option<i64>, cstyle: Option<i64>) -> CharFmt {
        let (mut c, _) = self.para_base(pstyle);
        if let Some(cs) = cstyle.and_then(|n| self.cstyles.get(&n)) {
            c = c.over(&cs.ch);
        }
        c
    }

    pub(super) fn ensure_style(&mut self, is_char: bool, n: i64) -> Option<String> {
        if let Some(v) = self.style_ids.get(&(is_char, n)) {
            return v.clone();
        }
        if !self.style_busy.insert((is_char, n)) {
            return None;
        }
        let r = self.build_style(is_char, n);
        self.style_busy.remove(&(is_char, n));
        self.style_ids.insert((is_char, n), r.clone());
        r
    }

    pub(super) fn build_style(&mut self, is_char: bool, n: i64) -> Option<String> {
        let def = if is_char { self.cstyles.get(&n)? } else { self.pstyles.get(&n)? };
        let (mut name, based, next, ch, pf) = (def.name.clone(), def.based, def.next, def.ch.clone(), def.pf.clone());
        let lname = name.to_lowercase();
        if !is_char && (n == 0 || lname == "normal") || lname == "default paragraph font" {
            return None;
        }
        if name.is_empty() {
            name = format!("Style{n}");
        }
        if let Some(id) = self.b.builtin_style(&name) {
            let char_builtin = matches!(id.as_str(), "Hyperlink" | "FootnoteReference" | "EndnoteReference");
            if char_builtin == is_char {
                self.used_ids.insert(id.clone());
                return Some(id);
            }
        }
        let mut id: String = name.chars().filter(|c| c.is_ascii_alphanumeric()).collect();
        if id.is_empty() {
            id = format!("Style{n}");
        }
        let base_id = id.clone();
        let mut k = 2;
        while self.used_ids.contains(&id) {
            id = format!("{base_id}{k}");
            k += 1;
        }
        self.used_ids.insert(id.clone());
        let based_id = match based {
            Some(0) if !is_char => Some("Normal".to_string()),
            Some(b) if b != n => self.ensure_style(is_char, b),
            _ => None,
        };
        let next_id = if is_char {
            None
        } else {
            match next {
                Some(nx) if nx == n => Some(id.clone()),
                Some(0) => Some("Normal".to_string()),
                Some(nx) => self.style_ids.get(&(false, nx)).cloned().flatten(),
                None => None,
            }
        };
        let mut st = Element::new("w:style")
            .with_attr("w:type", if is_char { "character" } else { "paragraph" })
            .with_attr("w:customStyle", "1")
            .with_attr("w:styleId", id.as_str())
            .with_child(Element::new("w:name").with_attr("w:val", name.as_str()));
        if let Some(b) = based_id {
            st.children.push(Node::Element(Element::new("w:basedOn").with_attr("w:val", b)));
        }
        if let Some(nx) = next_id {
            st.children.push(Node::Element(Element::new("w:next").with_attr("w:val", nx)));
        }
        st.children.push(Node::Element(Element::new("w:qFormat")));
        if !is_char {
            let ppr = self.ppr(&pf, &ParaFmt::default(), None, None);
            if !ppr.children.is_empty() {
                st.children.push(Node::Element(ppr));
            }
        }
        let rpr = self.rpr(&ch, &CharFmt { font: Some(self.deff), ..Default::default() }, None);
        if !rpr.children.is_empty() {
            st.children.push(Node::Element(rpr));
        }
        self.b.style(st);
        Some(id)
    }

    // -----------------------------------------------------------------------
    // Lists
    // -----------------------------------------------------------------------

    pub(super) fn list_for_ls(&mut self, ls: i64) -> Option<ListUse> {
        if let Some(u) = self.ls_use.get(&ls) {
            return u.clone();
        }
        let u = self.make_list(ls);
        self.ls_use.insert(ls, u.clone());
        u
    }

    pub(super) fn make_list(&mut self, ls: i64) -> Option<ListUse> {
        let ov = self.overrides.get(&ls)?;
        let starts = ov.starts.clone();
        let levels = self.lists.get(&ov.listid)?.clone();
        if levels.is_empty() {
            return None;
        }
        let mut specs = Vec::new();
        let mut ind = Vec::new();
        for (i, lv) in levels.iter().enumerate() {
            let spec = self.level_spec(i, lv, starts.get(i).copied().flatten());
            ind.push((spec.left, spec.hanging));
            specs.push(spec);
        }
        while ind.len() < 9 {
            let l = ind.len() as i64;
            ind.push((720 * (l + 1), 360));
        }
        let num_id = self.b.list(&specs);
        Some(ListUse { num_id, ind })
    }

    pub(super) fn level_spec(&self, i: usize, lv: &LevelDef, start: Option<i64>) -> LevelSpec {
        let left = lv.li.unwrap_or(720 * (i as i64 + 1));
        let hanging = match lv.fi {
            Some(f) if f < 0 => -f,
            Some(_) => 0,
            None => 360,
        };
        let start = start.unwrap_or(lv.start);
        let (fmt, text, font) = match lv.nfc {
            23 => {
                let fname = lv.font.and_then(|f| self.font_name(f));
                let (glyph, font) = self.bullet_glyph(&lv.text, fname.as_deref());
                ("bullet".to_string(), glyph, Some(font.to_string()))
            }
            255 => ("none".to_string(), String::new(), None),
            n => {
                let fmt = match n {
                    1 => "upperRoman",
                    2 => "lowerRoman",
                    3 => "upperLetter",
                    4 => "lowerLetter",
                    5 => "ordinal",
                    22 => "decimalZero",
                    _ => "decimal",
                };
                (fmt.to_string(), self.level_pattern(&lv.text, i), None)
            }
        };
        LevelSpec { fmt, text, start, left, hanging, font }
    }

    /// `\leveltext` as a numbering pattern: the first unit is a length, `\'00` to `\'08` stand for a level's number.
    pub(super) fn level_pattern(&self, units: &[(u32, bool)], lvl: usize) -> String {
        let mut out = String::new();
        for &(u, raw) in units.iter().skip(1) {
            if raw && u <= 8 {
                out.push_str(&format!("%{}", u + 1));
            } else if u < 0x20 {
                continue;
            } else if raw {
                out.push(decode_byte(self.ansicpg, u as u8));
            } else if let Some(c) = char::from_u32(u) {
                out.push(c);
            }
        }
        if !out.contains('%') {
            out.push_str(&format!("%{}.", lvl + 1));
        }
        out
    }

    /// The bullet character of a level and the font to show it in. Symbol and Wingdings private-use
    /// characters become ordinary Unicode bullets.
    pub(super) fn bullet_glyph(&self, units: &[(u32, bool)], font: Option<&str>) -> (String, &'static str) {
        let Some(&(u, raw)) = units.iter().skip(1).find(|(u, raw)| !(*raw && *u <= 8)) else {
            return ("\u{2022}".into(), "Arial");
        };
        let fname = font.map(|f| f.to_lowercase()).unwrap_or_default();
        let wing = fname.contains("wingdings");
        let symbolic = wing || fname.contains("symbol") || fname.contains("webdings");
        let code = if (0xF000..=0xF0FF).contains(&u) {
            Some(u & 0xFF)
        } else if symbolic {
            Some(u)
        } else {
            None
        };
        if let Some(c) = code {
            let g = if wing {
                match c {
                    0xA7 => "\u{25AA}",
                    0x76 => "\u{2756}",
                    0xD8 => "\u{27A2}",
                    0xFC => "\u{2713}",
                    0x6E => "\u{25A0}",
                    0x71 => "\u{2751}",
                    0x6F | 0xA8 => "\u{25A1}",
                    _ => "\u{2022}",
                }
            } else {
                "\u{2022}"
            };
            return (g.into(), "Arial");
        }
        let ch = if raw { decode_byte(self.ansicpg, u as u8) } else { char::from_u32(u).unwrap_or('\u{2022}') };
        match ch {
            'o' => ("o".into(), "Courier New"),
            '\u{00B7}' => ("\u{2022}".into(), "Arial"),
            c if (c as u32) < 0x20 => ("\u{2022}".into(), "Arial"),
            c => (c.to_string(), "Arial"),
        }
    }

    /// A Word 97 list paragraph (`\pntext`): one list shared by the paragraphs that follow each other.
    pub(super) fn legacy_list(&mut self, pntext: &str, pf: &ParaFmt) -> ListRef {
        let spec = match self.pn_spec.clone() {
            Some(s) => s,
            None => {
                let t = pntext.trim();
                let mut chars = t.chars();
                let first = chars.next();
                let s = if first.is_some_and(|c| c.is_ascii_digit()) {
                    PnSpec { bullet: false, fmt: "decimal" }
                } else if t.chars().count() <= 4 && t.ends_with(['.', ')']) && first.is_some_and(|c| c.is_alphabetic()) {
                    PnSpec { bullet: false, fmt: if first.is_some_and(|c| c.is_uppercase()) { "upperLetter" } else { "lowerLetter" } }
                } else {
                    PnSpec { bullet: true, fmt: "decimal" }
                };
                self.pn_spec = Some(s.clone());
                s
            }
        };
        let left = pf.li.unwrap_or(720);
        let hanging = match pf.fi {
            Some(f) if f < 0 => -f,
            _ => 360,
        };
        if let Some((s, u)) = &self.legacy {
            if *s == spec {
                return (u.num_id, 0, Some((left, hanging)));
            }
        }
        let mut level = if spec.bullet { LevelSpec::bullet(0) } else { LevelSpec::numbered(0, spec.fmt) };
        level.left = left;
        level.hanging = hanging;
        let num_id = self.b.list(&[level]);
        self.legacy = Some((spec, ListUse { num_id, ind: vec![(left, hanging); 9] }));
        (num_id, 0, Some((left, hanging)))
    }

    // -----------------------------------------------------------------------
    // Formatting to XML
    // -----------------------------------------------------------------------

    pub(super) fn rpr(&self, f: &CharFmt, base: &CharFmt, rstyle: Option<&str>) -> Element {
        let mut r = Element::new("w:rPr");
        if let Some(s) = rstyle {
            put_rpr(&mut r, Element::new("w:rStyle").with_attr("w:val", s));
        }
        if let Some(fid) = f.font {
            if let Some(name) = self.font_name(fid) {
                let base_name = base.font.or(Some(self.deff)).and_then(|b| self.font_name(b));
                if base_name.as_deref() != Some(name.as_str()) {
                    put_rpr(&mut r, Element::new("w:rFonts").with_attr("w:ascii", name.as_str()).with_attr("w:hAnsi", name.as_str()).with_attr("w:cs", name.as_str()));
                }
            }
        }
        let tog = |r: &mut Element, name: &'static str, cs: Option<&'static str>, v: Option<bool>, b: Option<bool>| {
            if let Some(v) = v {
                if v != b.unwrap_or(false) {
                    let mk = |n: &'static str| if v { Element::new(n) } else { Element::new(n).with_attr("w:val", "0") };
                    put_rpr(r, mk(name));
                    if let Some(cs) = cs {
                        put_rpr(r, mk(cs));
                    }
                }
            }
        };
        tog(&mut r, "w:b", Some("w:bCs"), f.bold, base.bold);
        tog(&mut r, "w:i", Some("w:iCs"), f.italic, base.italic);
        tog(&mut r, "w:caps", None, f.caps, base.caps);
        tog(&mut r, "w:smallCaps", None, f.scaps, base.scaps);
        tog(&mut r, "w:strike", None, f.strike, base.strike);
        tog(&mut r, "w:dstrike", None, f.dstrike, base.dstrike);
        tog(&mut r, "w:vanish", None, f.hidden, base.hidden);
        if let Some(c) = f.color {
            let hex = self.color_hex(c);
            let bhex = base.color.and_then(|b| self.color_hex(b));
            match hex {
                Some(h) => {
                    if bhex.as_deref() != Some(h.as_str()) {
                        put_rpr(&mut r, Element::new("w:color").with_attr("w:val", h));
                    }
                }
                None => {
                    if bhex.is_some() {
                        put_rpr(&mut r, Element::new("w:color").with_attr("w:val", "auto"));
                    }
                }
            }
        }
        if let Some(e) = f.expand.filter(|e| *e != 0 && Some(*e) != base.expand) {
            put_rpr(&mut r, Element::new("w:spacing").with_attr("w:val", e.to_string()));
        }
        if let Some(s) = f.scale.filter(|s| *s > 0 && *s != 100 && Some(*s) != base.scale) {
            put_rpr(&mut r, Element::new("w:w").with_attr("w:val", s.min(600).to_string()));
        }
        let raised = matches!(f.sup, Some(1) | Some(-1));
        if let Some(p) = f.pos.filter(|p| *p != 0 && !raised && Some(*p) != base.pos) {
            put_rpr(&mut r, Element::new("w:position").with_attr("w:val", p.to_string()));
        }
        if let Some(s) = f.size.filter(|s| *s > 0 && Some(*s) != base.size) {
            put_rpr(&mut r, Element::new("w:sz").with_attr("w:val", s.to_string()));
            put_rpr(&mut r, Element::new("w:szCs").with_attr("w:val", s.to_string()));
        }
        if let Some(h) = f.highlight.filter(|h| *h > 0 && Some(*h) != base.highlight) {
            if let Some(rgb) = self.colors.get(h as usize).copied().flatten() {
                match nearest_highlight(rgb) {
                    Some(name) => put_rpr(&mut r, Element::new("w:highlight").with_attr("w:val", name)),
                    None => put_rpr(&mut r, self.shd_el(&hex_of(rgb))),
                }
            }
        }
        if let Some(u) = f.ul {
            let b = base.ul;
            let write = if u == "none" { b.is_some_and(|b| b != "none") } else { b != Some(u) };
            if write {
                let mut e = Element::new("w:u").with_attr("w:val", u);
                if let Some(c) = f.ul_color.filter(|c| *c > 0).and_then(|c| self.color_hex(c)) {
                    e.set_attr("w:color", c);
                }
                put_rpr(&mut r, e);
            }
        }
        if let Some(bg) = f.bg.filter(|b| *b > 0 && Some(*b) != base.bg).and_then(|b| self.color_hex(b)) {
            put_rpr(&mut r, self.shd_el(&bg));
        }
        if let Some(s) = f.sup.filter(|s| Some(*s) != base.sup) {
            match s {
                1 => put_rpr(&mut r, Element::new("w:vertAlign").with_attr("w:val", "superscript")),
                -1 => put_rpr(&mut r, Element::new("w:vertAlign").with_attr("w:val", "subscript")),
                _ => {
                    if matches!(base.sup, Some(1) | Some(-1)) {
                        put_rpr(&mut r, Element::new("w:vertAlign").with_attr("w:val", "baseline"));
                    }
                }
            }
        }
        if let Some(tag) = f.lang.filter(|l| Some(*l) != base.lang).and_then(lang_tag) {
            put_rpr(&mut r, Element::new("w:lang").with_attr("w:val", tag));
        }
        r
    }

    /// Paragraph properties: only what differs from `base` (the paragraph style's own formatting).
    pub(super) fn ppr(&self, pf: &ParaFmt, base: &ParaFmt, style: Option<&str>, list: Option<ListRef>) -> Element {
        let mut e = Element::new("w:pPr");
        if let Some(s) = style {
            put_ppr(&mut e, Element::new("w:pStyle").with_attr("w:val", s));
        }
        let tog = |e: &mut Element, name: &'static str, v: Option<bool>, b: Option<bool>, default: bool| {
            if let Some(v) = v {
                if v != b.unwrap_or(default) {
                    put_ppr(e, if v { Element::new(name) } else { Element::new(name).with_attr("w:val", "0") });
                }
            }
        };
        tog(&mut e, "w:keepNext", pf.keepn, base.keepn, false);
        tog(&mut e, "w:keepLines", pf.keep, base.keep, false);
        tog(&mut e, "w:pageBreakBefore", pf.pagebb, base.pagebb, false);
        tog(&mut e, "w:widowControl", pf.widow, base.widow, true);
        if let Some((num_id, ilvl, _)) = list {
            put_ppr(&mut e, Element::new("w:numPr").with_child(Element::new("w:ilvl").with_attr("w:val", ilvl.to_string())).with_child(Element::new("w:numId").with_attr("w:val", num_id.to_string())));
        }
        if pf.borders != base.borders {
            let mut pb = Element::new("w:pBdr");
            for (i, tag) in ["w:top", "w:left", "w:bottom", "w:right", "w:between"].iter().enumerate() {
                if let Some(b) = pf.borders[i].as_ref().filter(|b| base.borders[i].as_ref() != Some(*b)) {
                    pb.children.push(Node::Element(self.border_el(tag, b)));
                }
            }
            if !pb.children.is_empty() {
                put_ppr(&mut e, pb);
            }
        }
        if (pf.shd_fill, pf.shd_pct, pf.shd_fore) != (base.shd_fill, base.shd_pct, base.shd_fore) {
            let idx = pf.shd_fill.or(if pf.shd_pct.unwrap_or(0) >= 5000 { pf.shd_fore } else { None });
            if let Some(h) = idx.and_then(|i| self.color_hex(i)) {
                put_ppr(&mut e, self.shd_el(&h));
            }
        }
        if !pf.tabs.is_empty() && pf.tabs != base.tabs {
            let mut tabs = Element::new("w:tabs");
            let mut ts = pf.tabs.clone();
            ts.sort_by_key(|t| t.pos);
            for t in ts {
                let mut te = Element::new("w:tab").with_attr("w:val", t.kind);
                if let Some(l) = t.leader {
                    te.set_attr("w:leader", l);
                }
                te.set_attr("w:pos", t.pos.to_string());
                tabs.children.push(Node::Element(te));
            }
            put_ppr(&mut e, tabs);
        }
        if pf.hyph == Some(false) && base.hyph != Some(false) {
            put_ppr(&mut e, Element::new("w:suppressAutoHyphens"));
        }
        if pf.bidi == Some(true) && base.bidi != Some(true) {
            put_ppr(&mut e, Element::new("w:bidi"));
        }
        let mut sp = Element::new("w:spacing");
        if let Some(v) = pf.sb.filter(|v| Some(*v) != base.sb) {
            sp.set_attr("w:before", v.max(0).to_string());
        }
        if pf.sbauto == Some(true) && base.sbauto != Some(true) {
            sp.set_attr("w:beforeAutospacing", "1");
        }
        if let Some(v) = pf.sa.filter(|v| Some(*v) != base.sa) {
            sp.set_attr("w:after", v.max(0).to_string());
        }
        if pf.saauto == Some(true) && base.saauto != Some(true) {
            sp.set_attr("w:afterAutospacing", "1");
        }
        if let Some(sl) = pf.sl {
            if (pf.sl, pf.slmult) != (base.sl, base.slmult) {
                if sl == 0 {
                    if base.sl.is_some_and(|b| b != 0) {
                        sp.set_attr("w:line", "240");
                        sp.set_attr("w:lineRule", "auto");
                    }
                } else if pf.slmult == Some(true) {
                    sp.set_attr("w:line", sl.abs().to_string());
                    sp.set_attr("w:lineRule", "auto");
                } else if sl < 0 {
                    sp.set_attr("w:line", (-sl).to_string());
                    sp.set_attr("w:lineRule", "exact");
                } else {
                    sp.set_attr("w:line", sl.to_string());
                    sp.set_attr("w:lineRule", "atLeast");
                }
            }
        }
        if !sp.attrs.is_empty() {
            put_ppr(&mut e, sp);
        }
        let list_ind = list.and_then(|(_, _, i)| i);
        let same_as_level = list_ind.is_some_and(|(l, h)| pf.li.unwrap_or(0) == l && -pf.fi.unwrap_or(0) == h);
        if !same_as_level {
            let mut ind = Element::new("w:ind");
            let mut put = |attr: &'static str, v: Option<i64>, b: Option<i64>| {
                if let Some(v) = v.filter(|v| Some(*v) != b && !(*v == 0 && b.is_none())) {
                    ind.set_attr(attr, v.to_string());
                }
            };
            put("w:left", pf.li, base.li);
            put("w:right", pf.ri, base.ri);
            if let Some(f) = pf.fi.filter(|f| Some(*f) != base.fi && !(*f == 0 && base.fi.is_none())) {
                if f < 0 {
                    ind.set_attr("w:hanging", (-f).to_string());
                } else {
                    ind.set_attr("w:firstLine", f.to_string());
                }
            }
            if !ind.attrs.is_empty() {
                put_ppr(&mut e, ind);
            }
        }
        if let Some(j) = pf.jc.filter(|j| Some(*j) != base.jc && !(*j == "left" && base.jc.is_none())) {
            put_ppr(&mut e, Element::new("w:jc").with_attr("w:val", j));
        }
        if let Some(o) = pf.outline.filter(|o| (0..=8).contains(o) && Some(*o) != base.outline) {
            put_ppr(&mut e, Element::new("w:outlineLvl").with_attr("w:val", o.to_string()));
        }
        e
    }

    pub(super) fn build_para(&mut self, cur: CurPara, pf: &ParaFmt) -> Element {
        let mut style_id = pf.style.and_then(|n| self.ensure_style(false, n));
        if style_id.is_none() {
            if let Some(info) = self.story().note.as_ref() {
                let name = if info.endnote { "Endnote Text" } else { "Footnote Text" };
                style_id = self.builtin(name);
            }
        }
        let (_, base) = self.para_base(pf.style);
        let pn_item = std::mem::take(&mut self.pn_seen);
        let pn_text = std::mem::take(&mut self.pn_buf);
        let mut list: Option<ListRef> = None;
        if let Some(ls) = pf.ls {
            if let Some(u) = self.list_for_ls(ls) {
                let ilvl = pf.ilvl.unwrap_or(0).clamp(0, 8);
                list = Some((u.num_id, ilvl, u.ind.get(ilvl as usize).copied()));
            }
        } else if pn_item {
            list = Some(self.legacy_list(&pn_text, pf));
        }
        if !pn_item {
            self.legacy = None;
            self.pn_spec = None;
        }
        if list.is_some() && style_id.is_none() {
            style_id = self.builtin("List Paragraph");
        }
        let ppr = self.ppr(pf, &base, style_id.as_deref(), list);
        let mut p = Element::new("w:p");
        if !ppr.children.is_empty() {
            p.children.push(Node::Element(ppr));
        }
        p.children.extend(cur.inlines.into_iter().map(Node::Element));
        p
    }

    // -----------------------------------------------------------------------
    // The end
    // -----------------------------------------------------------------------

    pub(super) fn finish(mut self) -> Fragment {
        while self.stack.len() > 1 {
            self.close_group();
        }
        self.over = 0;
        while self.stories.len() > 1 {
            self.stories.pop();
        }
        self.finalize_body();
        let normal = self.pstyles.get(&0).map(|s| s.ch.clone()).unwrap_or_default();
        let font = normal.font.or(Some(self.deff)).and_then(|f| self.font_name(f));
        let size = normal.size.map(|s| s as f64 / 2.0);
        let defaults = if font.is_some() || size.is_some() { Some(crate::fragment::FragDefaults { font, size }) } else { None };
        let root = self.stories.pop().unwrap_or_default();
        let Interp { mut b, notes, .. } = self;
        for el in root.blocks {
            b.block(el);
        }
        let mut f = b.finish();
        f.block = true;
        f.defaults = defaults;
        let note_xml: String = notes.values().map(|n| n.xml.as_str()).collect();
        f.notes = notes;
        for (p, u) in NAMESPACES {
            let used = note_xml.contains(&format!("<{p}:")) || note_xml.contains(&format!(" {p}:"));
            if used && !f.ns.iter().any(|(q, _)| q == p) {
                f.ns.push((p.to_string(), u.to_string()));
            }
        }
        f
    }
}
