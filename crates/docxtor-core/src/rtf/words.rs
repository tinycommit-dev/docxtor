//! Control words: character, paragraph, border, table, list, style and destination words.

use super::*;
use crate::xml::Element;
use super::state::*;

impl Interp {
    pub(super) fn word(&mut self, name: &str, p: Option<i64>) {
        self.skip = 0;
        let (first, mut star, dest) = {
            let g = self.g();
            let t = (g.first, g.star, g.dest);
            g.first = false;
            g.star = false;
            t
        };
        if dest == Dest::Skip {
            return;
        }
        if dest == Dest::StyleEntry {
            star = false;
        }
        if first {
            if self.dest_word(name) {
                return;
            }
            if star {
                self.g().dest = Dest::Skip;
                return;
            }
        }
        if name == "u" && !first {
            self.unicode(p.unwrap_or(0));
            return;
        }
        if name == "uc" {
            self.g().uc = p.unwrap_or(1).clamp(0, 8) as usize;
            return;
        }
        let dest = self.g().dest;
        match dest {
            Dest::FontTbl => self.font_word(name, p),
            Dest::ColorTbl => self.color_word(name, p),
            Dest::Pict => self.pict_word(name, p),
            Dest::Pn => self.pn_word(name),
            Dest::ListDef | Dest::ListOverride | Dest::LfoLevel => {
                self.list_word(name, p);
            }
            Dest::ListLevel => {
                if !self.list_word(name, p) {
                    self.format_word(name, p);
                }
            }
            Dest::StyleEntry => {
                if !self.style_word(name, p) {
                    self.format_word(name, p);
                }
            }
            Dest::Body | Dest::FldRslt | Dest::Footnote => {
                if !self.body_word(name, p) {
                    self.format_word(name, p);
                }
            }
            Dest::PnText | Dest::LevelText => {
                self.char_word(name, p);
            }
            _ => {}
        }
    }

    /// A control word that opens a destination, when it is the first in its group.
    pub(super) fn dest_word(&mut self, name: &str) -> bool {
        let parent = self.g().dest;
        let (dest, own) = match name {
            "fonttbl" => (Dest::FontTbl, Own::None),
            "colortbl" => {
                self.colors.clear();
                self.color_acc = (None, None, None);
                (Dest::ColorTbl, Own::None)
            }
            "stylesheet" => (Dest::StyleSheet, Own::None),
            "listtable" => (Dest::ListTable, Own::None),
            "listoverridetable" => (Dest::ListOverrideTable, Own::None),
            "list" => {
                self.list_acc = ListDefAcc::default();
                (Dest::ListDef, Own::ListDef)
            }
            "listlevel" if parent == Dest::LfoLevel => (Dest::Skip, Own::None),
            "listlevel" => {
                self.level_acc = LevelDef { start: 1, ..Default::default() };
                (Dest::ListLevel, Own::ListLevel)
            }
            "leveltext" => {
                self.level_text.clear();
                (Dest::LevelText, Own::LevelText)
            }
            "listoverride" => {
                self.ovr_acc = Override::default();
                self.ovr_ls = 0;
                (Dest::ListOverride, Own::ListOverride)
            }
            "lfolevel" => {
                self.lfo_start = None;
                (Dest::LfoLevel, Own::Lfo)
            }
            "field" => {
                self.fields.push(FieldAcc::default());
                (Dest::Field, Own::Field)
            }
            "fldinst" => (Dest::FldInst, Own::None),
            "fldrslt" => {
                let own = if self.open_fldrslt() { Own::FldRsltLink } else { Own::FldRslt };
                (Dest::FldRslt, own)
            }
            "pict" => {
                self.pict = PictAcc { sx: 100, sy: 100, ..Default::default() };
                (Dest::Pict, Own::Pict)
            }
            "footnote" if parent.bodyish() => return self.open_footnote(),
            "pntext" => {
                self.pn_buf.clear();
                (Dest::PnText, Own::PnText)
            }
            "pn" => (Dest::Pn, Own::None),
            "shppict" | "upr" => return true,
            "shp" | "shpinst" | "shpgrp" | "sp" | "sn" | "sv" => (Dest::NoText, Own::None),
            "object" => {
                self.b.warn("object-dropped");
                (Dest::Skip, Own::None)
            }
            "footnote" | "info" | "header" | "headerl" | "headerr" | "headerf" | "footer" | "footerl" | "footerr" | "footerf" | "pgdsc" | "pgdsctbl" | "xmlnstbl"
            | "rsidtbl" | "generator" | "themedata" | "colorschememapping" | "latentstyles" | "datastore" | "bkmkstart" | "bkmkend" | "nonesttables" | "nonshppict"
            | "shptxt" | "shprslt" | "private" | "mmath" | "mmathpr" | "docvar" | "userprops" | "revtbl" | "fchars" | "lchars" | "defchp" | "defpap" | "ftnsep"
            | "ftnsepc" | "ftncn" | "aftnsep" | "aftnsepc" | "aftncn" | "pnseclvl" | "do" | "ud" | "xe" | "tc" | "txe" | "rxe" | "result" | "rsidroot"
            | "protusertbl" | "wgrffmtfilter" | "filetbl" | "levelnumbers" | "listname" | "liststylename" | "listtext" | "objdata" | "falt" | "fname" | "panose" => {
                (Dest::Skip, Own::None)
            }
            _ => return false,
        };
        let g = self.g();
        g.dest = dest;
        g.own = own;
        true
    }

    pub(super) fn font_word(&mut self, name: &str, p: Option<i64>) {
        let n = p.unwrap_or(0);
        match name {
            "f" => {
                self.font_cur = Some(n);
                self.fonts.entry(n).or_default();
            }
            "fcharset" => {
                if let Some(f) = self.font_cur {
                    self.fonts.entry(f).or_default().charset = n;
                }
            }
            "cpg" => {
                if let Some(f) = self.font_cur {
                    self.fonts.entry(f).or_default().cpg = Some(n);
                }
            }
            _ => {}
        }
    }

    pub(super) fn color_word(&mut self, name: &str, p: Option<i64>) {
        let v = p.unwrap_or(0).clamp(0, 255) as u8;
        match name {
            "red" => self.color_acc.0 = Some(v),
            "green" => self.color_acc.1 = Some(v),
            "blue" => self.color_acc.2 = Some(v),
            _ => {}
        }
    }

    pub(super) fn pict_word(&mut self, name: &str, p: Option<i64>) {
        let n = p.unwrap_or(0);
        match name {
            "pngblip" | "jpegblip" => self.pict.kind = PKind::Raster,
            "emfblip" | "wmetafile" | "macpict" | "pmmetafile" | "dibitmap" | "wbitmap" => self.pict.kind = PKind::Unsupported,
            "picw" => self.pict.picw = n,
            "pich" => self.pict.pich = n,
            "picwgoal" => self.pict.wgoal = n,
            "pichgoal" => self.pict.hgoal = n,
            "picscalex" => self.pict.sx = n,
            "picscaley" => self.pict.sy = n,
            _ => {}
        }
    }

    pub(super) fn pn_word(&mut self, name: &str) {
        let spec = self.pn_spec.get_or_insert(PnSpec { bullet: false, fmt: "decimal" });
        match name {
            "pnlvlblt" => spec.bullet = true,
            "pnlvlbody" | "pnlvl" => spec.bullet = false,
            "pndec" | "pnord" | "pncard" | "pnordt" => spec.fmt = "decimal",
            "pnucrm" => spec.fmt = "upperRoman",
            "pnlcrm" => spec.fmt = "lowerRoman",
            "pnucltr" => spec.fmt = "upperLetter",
            "pnlcltr" => spec.fmt = "lowerLetter",
            _ => {}
        }
    }

    pub(super) fn list_word(&mut self, name: &str, p: Option<i64>) -> bool {
        let n = p.unwrap_or(0);
        let dest = self.g().dest;
        match (dest, name) {
            (Dest::ListDef, "listid") => self.list_acc.id = n,
            (Dest::ListLevel, "levelnfc" | "levelnfcn") => self.level_acc.nfc = n,
            (Dest::ListLevel, "levelstartat") => self.level_acc.start = n,
            (Dest::LfoLevel, "levelstartat") => self.lfo_start = Some(n),
            (Dest::ListOverride, "ls") => self.ovr_ls = n,
            (Dest::ListOverride, "listid") => self.ovr_acc.listid = n,
            _ => return false,
        }
        true
    }

    pub(super) fn style_word(&mut self, name: &str, p: Option<i64>) -> bool {
        let n = p.unwrap_or(0);
        match name {
            "s" => {
                self.style_acc.kind = SKind::Para;
                self.style_acc.num = n;
            }
            "cs" => {
                self.style_acc.kind = SKind::Char;
                self.style_acc.num = n;
            }
            "ts" | "ds" => self.style_acc.kind = SKind::Other,
            "sbasedon" => self.style_acc.based = Some(n),
            "snext" => self.style_acc.next = Some(n),
            _ => return false,
        }
        true
    }

    pub(super) fn finish_style(&mut self, g: &Gs) {
        let acc = std::mem::take(&mut self.style_acc);
        // "heading 1,h1": the first name is the style's name, the rest are aliases.
        let name = acc.name.split(';').next().unwrap_or("").split(',').next().unwrap_or("").trim().to_string();
        let mut pf = g.pf.clone();
        pf.style = None;
        let def = StyleDef { name, based: acc.based, next: acc.next, ch: g.ch.clone(), pf };
        match acc.kind {
            SKind::Para => {
                self.pstyles.entry(acc.num).or_insert(def);
            }
            SKind::Char => {
                self.cstyles.entry(acc.num).or_insert(def);
            }
            SKind::Other => {}
        }
    }

    /// Words that act on the document or produce text or structure.
    pub(super) fn body_word(&mut self, name: &str, p: Option<i64>) -> bool {
        match name {
            "rtf" => self.rtf_seen = true,
            "ansicpg" => self.ansicpg = p.unwrap_or(1252).clamp(0, 65535) as u16,
            "deff" => self.deff = p.unwrap_or(0),
            "deflang" => self.deflang = p.unwrap_or(0),
            "par" => self.end_para(EndKind::Par),
            "sect" => self.end_para(EndKind::Soft),
            "cell" => self.end_cell(),
            "row" => self.end_row(),
            "nestcell" => self.end_para(EndKind::Par),
            "nestrow" => {}
            "line" => self.push_piece(Element::new("w:br")),
            "tab" => self.push_piece(Element::new("w:tab")),
            "page" => self.push_piece(Element::new("w:br").with_attr("w:type", "page")),
            "column" => self.push_piece(Element::new("w:br").with_attr("w:type", "column")),
            "chftn" => self.note_mark(),
            "ftnalt" => self.mark_endnote(),
            _ => {
                if let Some(c) = special_char(name) {
                    self.emit_char(c);
                } else {
                    return false;
                }
            }
        }
        true
    }

    pub(super) fn unicode(&mut self, n: i64) {
        let v = (if n < 0 { n + 65536 } else { n }) as u32 & 0xFFFF;
        let uc = self.g().uc;
        self.skip = uc;
        if (0xD800..0xDC00).contains(&v) {
            self.hi = Some(v as u16);
            return;
        }
        let c = if (0xDC00..0xE000).contains(&v) {
            match self.hi.take() {
                Some(h) => char::from_u32(0x10000 + (((h as u32) - 0xD800) << 10) + (v - 0xDC00)),
                None => Some('\u{FFFD}'),
            }
        } else {
            self.hi = None;
            char::from_u32(v)
        };
        if self.g().dest == Dest::LevelText {
            self.level_text.push((v, false));
        } else if let Some(c) = c {
            self.emit_char(c);
        }
    }

    pub(super) fn format_word(&mut self, name: &str, p: Option<i64>) -> bool {
        if self.char_word(name, p) || self.para_word(name, p) {
            return true;
        }
        if self.g().dest.bodyish() {
            return self.table_word(name, p);
        }
        false
    }

    pub(super) fn char_word(&mut self, name: &str, p: Option<i64>) -> bool {
        let n = p.unwrap_or(0);
        let on = p != Some(0);
        let c = &mut self.g().ch;
        match name {
            "plain" => *c = CharFmt::default(),
            "b" => c.bold = Some(on),
            "i" => c.italic = Some(on),
            "ul" => c.ul = Some(if on { "single" } else { "none" }),
            "ulnone" => c.ul = Some("none"),
            "ulw" => c.ul = Some("words"),
            "uld" => c.ul = Some("dotted"),
            "uldb" => c.ul = Some("double"),
            "ulth" => c.ul = Some("thick"),
            "uldash" => c.ul = Some("dash"),
            "uldashd" => c.ul = Some("dotDash"),
            "uldashdd" => c.ul = Some("dotDotDash"),
            "ulwave" => c.ul = Some("wave"),
            "ulthd" => c.ul = Some("dottedHeavy"),
            "ulthdash" => c.ul = Some("dashedHeavy"),
            "ulhwave" => c.ul = Some("wavyHeavy"),
            "ululdbwave" => c.ul = Some("wavyDouble"),
            "ulc" => c.ul_color = Some(n),
            "strike" => c.strike = Some(on),
            "striked" => c.dstrike = Some(on),
            "v" => c.hidden = Some(on),
            "scaps" => c.scaps = Some(on),
            "caps" => c.caps = Some(on),
            "fs" => c.size = Some(n),
            "f" => c.font = Some(n),
            "cf" => c.color = Some(n),
            "highlight" => c.highlight = Some(n),
            "cb" | "chcbpat" => c.bg = Some(n),
            "super" => c.sup = Some(1),
            "sub" => c.sup = Some(-1),
            "nosupersub" => c.sup = Some(0),
            "up" => c.pos = Some(p.unwrap_or(6)),
            "dn" => c.pos = Some(-p.unwrap_or(6)),
            "expnd" => c.expand = Some(n.saturating_mul(5)),
            "expndtw" => c.expand = Some(n),
            "charscalex" => c.scale = Some(n),
            "lang" => c.lang = Some(n),
            "cs" => c.cstyle = if n > 0 { Some(n) } else { None },
            _ => return false,
        }
        true
    }

    pub(super) fn para_word(&mut self, name: &str, p: Option<i64>) -> bool {
        let n = p.unwrap_or(0);
        let on = p != Some(0);
        let g = self.g();
        match name {
            "pard" => {
                g.pf = ParaFmt::default();
                g.tab_kind = "left";
                g.tab_leader = None;
                g.bt = Bt::None;
            }
            "ql" => g.pf.jc = Some("left"),
            "qc" => g.pf.jc = Some("center"),
            "qr" => g.pf.jc = Some("right"),
            "qj" => g.pf.jc = Some("both"),
            "li" | "lin" => g.pf.li = Some(n),
            "ri" | "rin" => g.pf.ri = Some(n),
            "fi" => g.pf.fi = Some(n),
            "sb" => g.pf.sb = Some(n),
            "sa" => g.pf.sa = Some(n),
            "sbauto" => g.pf.sbauto = Some(on),
            "saauto" => g.pf.saauto = Some(on),
            "sl" => g.pf.sl = Some(n),
            "slmult" => g.pf.slmult = Some(n != 0),
            "keep" => g.pf.keep = Some(on),
            "keepn" => g.pf.keepn = Some(on),
            "pagebb" => g.pf.pagebb = Some(on),
            "widctlpar" => g.pf.widow = Some(true),
            "nowidctlpar" => g.pf.widow = Some(false),
            "hyphpar" => g.pf.hyph = Some(on),
            "outlinelevel" => g.pf.outline = Some(n),
            "rtlpar" => g.pf.bidi = Some(true),
            "ltrpar" => g.pf.bidi = Some(false),
            "tqr" => g.tab_kind = "right",
            "tqc" => g.tab_kind = "center",
            "tqdec" => g.tab_kind = "decimal",
            "tldot" => g.tab_leader = Some("dot"),
            "tlmdot" => g.tab_leader = Some("middleDot"),
            "tlhyph" => g.tab_leader = Some("hyphen"),
            "tlul" => g.tab_leader = Some("underscore"),
            "tlth" => g.tab_leader = Some("heavy"),
            "tx" => {
                if g.pf.tabs.len() < 64 {
                    let t = TabStop { pos: n, kind: g.tab_kind, leader: g.tab_leader };
                    g.pf.tabs.push(t);
                }
                g.tab_kind = "left";
                g.tab_leader = None;
            }
            "tb" => {
                g.tab_kind = "left";
                g.tab_leader = None;
            }
            "shading" => g.pf.shd_pct = Some(n),
            "cfpat" => g.pf.shd_fore = Some(n),
            "cbpat" => g.pf.shd_fill = Some(n),
            "s" => g.pf.style = if n > 0 { Some(n) } else { None },
            "ls" => g.pf.ls = if n > 0 { Some(n) } else { None },
            "ilvl" => g.pf.ilvl = Some(n),
            "intbl" => g.pf.intbl = on,
            "itap" => g.pf.itap = n,
            _ => return self.border_word(name, p),
        }
        true
    }

    pub(super) fn border_word(&mut self, name: &str, p: Option<i64>) -> bool {
        let n = p.unwrap_or(0);
        let target = match name {
            "brdrt" => Some(Bt::PTop),
            "brdrl" => Some(Bt::PLeft),
            "brdrb" => Some(Bt::PBottom),
            "brdrr" => Some(Bt::PRight),
            "brdrbtw" => Some(Bt::PBetween),
            "box" => Some(Bt::PBox),
            "clbrdrt" => Some(Bt::CTop),
            "clbrdrl" => Some(Bt::CLeft),
            "clbrdrb" => Some(Bt::CBottom),
            "clbrdrr" => Some(Bt::CRight),
            "trbrdrt" => Some(Bt::RTop),
            "trbrdrl" => Some(Bt::RLeft),
            "trbrdrb" => Some(Bt::RBottom),
            "trbrdrr" => Some(Bt::RRight),
            "trbrdrh" => Some(Bt::RH),
            "trbrdrv" => Some(Bt::RV),
            _ => None,
        };
        if let Some(t) = target {
            self.g().bt = t;
            self.set_border(&|b| *b = Brd::new());
            return true;
        }
        let style: Option<&'static str> = match name {
            "brdrs" | "brdrsh" => Some("single"),
            "brdrth" => Some("thick"),
            "brdrdb" => Some("double"),
            "brdrdot" => Some("dotted"),
            "brdrdash" => Some("dashed"),
            "brdrhair" => Some("single"),
            "brdrdashsm" => Some("dashSmallGap"),
            "brdrdashd" => Some("dotDash"),
            "brdrdashdd" => Some("dotDotDash"),
            "brdrtriple" => Some("triple"),
            "brdrwavy" | "brdrwavydb" => Some("wave"),
            "brdrinset" => Some("inset"),
            "brdroutset" => Some("outset"),
            "brdrengrave" => Some("engrave"),
            "brdremboss" => Some("emboss"),
            "brdrnil" | "brdrnone" => Some("nil"),
            _ => None,
        };
        if let Some(s) = style {
            let hair = name == "brdrhair";
            self.set_border(&|b| {
                b.style = s;
                if hair {
                    b.w = 2;
                }
            });
            return true;
        }
        match name {
            "brdrw" => self.set_border(&|b| b.w = n),
            "brdrcf" => self.set_border(&|b| b.color = Some(n)),
            "brdrsp" => self.set_border(&|b| b.space = n),
            _ => return false,
        }
        true
    }

    pub(super) fn set_border(&mut self, f: &dyn Fn(&mut Brd)) {
        let bt = self.g().bt;
        let apply = |slot: &mut Option<Brd>| f(slot.get_or_insert_with(Brd::new));
        match bt {
            Bt::None => {}
            Bt::PTop => apply(&mut self.g().pf.borders[0]),
            Bt::PLeft => apply(&mut self.g().pf.borders[1]),
            Bt::PBottom => apply(&mut self.g().pf.borders[2]),
            Bt::PRight => apply(&mut self.g().pf.borders[3]),
            Bt::PBetween => apply(&mut self.g().pf.borders[4]),
            Bt::PBox => {
                for i in 0..4 {
                    apply(&mut self.g().pf.borders[i]);
                }
            }
            Bt::CTop => apply(&mut self.story().tbl.cell_acc.borders[0]),
            Bt::CLeft => apply(&mut self.story().tbl.cell_acc.borders[1]),
            Bt::CBottom => apply(&mut self.story().tbl.cell_acc.borders[2]),
            Bt::CRight => apply(&mut self.story().tbl.cell_acc.borders[3]),
            Bt::RTop => apply(&mut self.story().tbl.def.borders[0]),
            Bt::RLeft => apply(&mut self.story().tbl.def.borders[1]),
            Bt::RBottom => apply(&mut self.story().tbl.def.borders[2]),
            Bt::RRight => apply(&mut self.story().tbl.def.borders[3]),
            Bt::RH => apply(&mut self.story().tbl.def.borders[4]),
            Bt::RV => apply(&mut self.story().tbl.def.borders[5]),
        }
    }

    pub(super) fn table_word(&mut self, name: &str, p: Option<i64>) -> bool {
        let n = p.unwrap_or(0);
        match name {
            "trowd" => {
                let st = self.story();
                st.tbl.def = RowDef::default();
                st.tbl.cell_acc = CellDef::default();
                st.tbl.row_open = true;
                self.g().bt = Bt::None;
            }
            "trgaph" => self.story().tbl.def.gaph = Some(n),
            "trleft" => self.story().tbl.def.left = n,
            "trrh" => self.story().tbl.def.height = n,
            "trhdr" => self.story().tbl.def.header = true,
            "trkeep" => self.story().tbl.def.keep = true,
            "trqc" => self.story().tbl.def.jc = Some("center"),
            "trqr" => self.story().tbl.def.jc = Some("right"),
            "trql" => self.story().tbl.def.jc = Some("left"),
            "cellx" => {
                let st = self.story();
                let mut c = std::mem::take(&mut st.tbl.cell_acc);
                c.right = n;
                if st.tbl.def.cells.len() < MAX_CELLS {
                    st.tbl.def.cells.push(c);
                }
                self.g().bt = Bt::None;
            }
            "clvmgf" => self.story().tbl.cell_acc.vmerge = 1,
            "clvmrg" => self.story().tbl.cell_acc.vmerge = 2,
            "clmgf" => self.story().tbl.cell_acc.hmerge = 1,
            "clmrg" => self.story().tbl.cell_acc.hmerge = 2,
            "clvertalt" => self.story().tbl.cell_acc.valign = Some("top"),
            "clvertalc" => self.story().tbl.cell_acc.valign = Some("center"),
            "clvertalb" => self.story().tbl.cell_acc.valign = Some("bottom"),
            "clcbpat" => self.story().tbl.cell_acc.shd = Some(n),
            _ => return false,
        }
        true
    }
}

/// Control words that stand for one character.
pub(super) fn special_char(name: &str) -> Option<char> {
    Some(match name {
        "emdash" => '\u{2014}',
        "endash" => '\u{2013}',
        "emspace" => '\u{2003}',
        "enspace" => '\u{2002}',
        "qmspace" => '\u{2005}',
        "bullet" => '\u{2022}',
        "lquote" => '\u{2018}',
        "rquote" => '\u{2019}',
        "ldblquote" => '\u{201C}',
        "rdblquote" => '\u{201D}',
        "zwj" => '\u{200D}',
        "zwnj" => '\u{200C}',
        "zwbo" => '\u{200B}',
        "zwnbo" => '\u{2060}',
        "ltrmark" => '\u{200E}',
        "rtlmark" => '\u{200F}',
        _ => return None,
    })
}

// ---------------------------------------------------------------------------
// Text, runs and paragraphs
// ---------------------------------------------------------------------------
