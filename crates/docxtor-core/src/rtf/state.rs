//! The reader's state: formatting, groups, destinations, tables, lists, notes and the interpreter that holds them.

use crate::fragment::{Builder, FragNote};
use crate::xml::Element;
use std::collections::{BTreeMap, HashMap, HashSet};

/// Where the text of a group goes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Dest {
    Body,
    Skip,
    /// Text is dropped but groups inside (a picture in a shape) are still read.
    NoText,
    FontTbl,
    ColorTbl,
    StyleSheet,
    StyleEntry,
    ListTable,
    ListDef,
    ListLevel,
    LevelText,
    ListOverrideTable,
    ListOverride,
    LfoLevel,
    Field,
    FldInst,
    FldRslt,
    Footnote,
    Pict,
    PnText,
    Pn,
}

impl Dest {
    pub(super) fn bodyish(self) -> bool {
        matches!(self, Dest::Body | Dest::FldRslt | Dest::Footnote)
    }
}

/// What a group does when it closes (not inherited by the groups inside it).
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Own {
    None,
    Footnote,
    FldRslt,
    FldRsltLink,
    Field,
    Style,
    ListDef,
    ListLevel,
    ListOverride,
    Lfo,
    Pict,
    PnText,
    LevelText,
}

/// The border the next `\brdrs`, `\brdrw`... words describe.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Bt {
    None,
    PTop,
    PLeft,
    PBottom,
    PRight,
    PBetween,
    PBox,
    CTop,
    CLeft,
    CBottom,
    CRight,
    RTop,
    RLeft,
    RBottom,
    RRight,
    RH,
    RV,
}

#[derive(Clone, PartialEq, Debug)]
pub(super) struct Brd {
    pub(super) style: &'static str,
    /// Twips.
    pub(super) w: i64,
    pub(super) color: Option<i64>,
    /// Twips.
    pub(super) space: i64,
}

impl Brd {
    pub(super) fn new() -> Brd {
        Brd { style: "single", w: 10, color: None, space: 0 }
    }
}

#[derive(Clone, PartialEq)]
pub(super) struct TabStop {
    pub(super) pos: i64,
    pub(super) kind: &'static str,
    pub(super) leader: Option<&'static str>,
}

/// Character formatting as the stream states it. `None` means "not said".
#[derive(Clone, PartialEq, Default)]
pub(super) struct CharFmt {
    pub(super) bold: Option<bool>,
    pub(super) italic: Option<bool>,
    pub(super) ul: Option<&'static str>,
    pub(super) ul_color: Option<i64>,
    pub(super) strike: Option<bool>,
    pub(super) dstrike: Option<bool>,
    pub(super) hidden: Option<bool>,
    pub(super) scaps: Option<bool>,
    pub(super) caps: Option<bool>,
    pub(super) font: Option<i64>,
    /// Half-points.
    pub(super) size: Option<i64>,
    pub(super) color: Option<i64>,
    pub(super) highlight: Option<i64>,
    pub(super) bg: Option<i64>,
    pub(super) sup: Option<i8>,
    /// Half-points, positive is raised.
    pub(super) pos: Option<i64>,
    /// Twips.
    pub(super) expand: Option<i64>,
    pub(super) scale: Option<i64>,
    pub(super) lang: Option<i64>,
    pub(super) cstyle: Option<i64>,
}

impl CharFmt {
    pub(super) fn over(&self, top: &CharFmt) -> CharFmt {
        CharFmt {
            bold: top.bold.or(self.bold),
            italic: top.italic.or(self.italic),
            ul: top.ul.or(self.ul),
            ul_color: top.ul_color.or(self.ul_color),
            strike: top.strike.or(self.strike),
            dstrike: top.dstrike.or(self.dstrike),
            hidden: top.hidden.or(self.hidden),
            scaps: top.scaps.or(self.scaps),
            caps: top.caps.or(self.caps),
            font: top.font.or(self.font),
            size: top.size.or(self.size),
            color: top.color.or(self.color),
            highlight: top.highlight.or(self.highlight),
            bg: top.bg.or(self.bg),
            sup: top.sup.or(self.sup),
            pos: top.pos.or(self.pos),
            expand: top.expand.or(self.expand),
            scale: top.scale.or(self.scale),
            lang: top.lang.or(self.lang),
            cstyle: top.cstyle.or(self.cstyle),
        }
    }
}

/// Paragraph formatting as the stream states it.
#[derive(Clone, PartialEq, Default)]
pub(super) struct ParaFmt {
    pub(super) jc: Option<&'static str>,
    pub(super) li: Option<i64>,
    pub(super) ri: Option<i64>,
    pub(super) fi: Option<i64>,
    pub(super) sb: Option<i64>,
    pub(super) sa: Option<i64>,
    pub(super) sbauto: Option<bool>,
    pub(super) saauto: Option<bool>,
    pub(super) sl: Option<i64>,
    pub(super) slmult: Option<bool>,
    pub(super) keep: Option<bool>,
    pub(super) keepn: Option<bool>,
    pub(super) pagebb: Option<bool>,
    pub(super) widow: Option<bool>,
    pub(super) hyph: Option<bool>,
    pub(super) outline: Option<i64>,
    pub(super) bidi: Option<bool>,
    pub(super) tabs: Vec<TabStop>,
    /// Top, left, bottom, right, between.
    pub(super) borders: [Option<Brd>; 5],
    pub(super) shd_pct: Option<i64>,
    pub(super) shd_fore: Option<i64>,
    pub(super) shd_fill: Option<i64>,
    pub(super) style: Option<i64>,
    pub(super) ls: Option<i64>,
    pub(super) ilvl: Option<i64>,
    pub(super) intbl: bool,
    pub(super) itap: i64,
}

impl ParaFmt {
    pub(super) fn over(&self, top: &ParaFmt) -> ParaFmt {
        let mut borders: [Option<Brd>; 5] = Default::default();
        for (i, slot) in borders.iter_mut().enumerate() {
            *slot = top.borders[i].clone().or_else(|| self.borders[i].clone());
        }
        ParaFmt {
            jc: top.jc.or(self.jc),
            li: top.li.or(self.li),
            ri: top.ri.or(self.ri),
            fi: top.fi.or(self.fi),
            sb: top.sb.or(self.sb),
            sa: top.sa.or(self.sa),
            sbauto: top.sbauto.or(self.sbauto),
            saauto: top.saauto.or(self.saauto),
            sl: top.sl.or(self.sl),
            slmult: top.slmult.or(self.slmult),
            keep: top.keep.or(self.keep),
            keepn: top.keepn.or(self.keepn),
            pagebb: top.pagebb.or(self.pagebb),
            widow: top.widow.or(self.widow),
            hyph: top.hyph.or(self.hyph),
            outline: top.outline.or(self.outline),
            bidi: top.bidi.or(self.bidi),
            tabs: if top.tabs.is_empty() { self.tabs.clone() } else { top.tabs.clone() },
            borders,
            shd_pct: top.shd_pct.or(self.shd_pct),
            shd_fore: top.shd_fore.or(self.shd_fore),
            shd_fill: top.shd_fill.or(self.shd_fill),
            style: None,
            ls: None,
            ilvl: None,
            intbl: false,
            itap: 0,
        }
    }
}

#[derive(Clone)]
pub(super) struct Gs {
    pub(super) dest: Dest,
    pub(super) own: Own,
    pub(super) ch: CharFmt,
    pub(super) pf: ParaFmt,
    pub(super) uc: usize,
    pub(super) first: bool,
    pub(super) star: bool,
    pub(super) bt: Bt,
    pub(super) tab_kind: &'static str,
    pub(super) tab_leader: Option<&'static str>,
}

impl Gs {
    pub(super) fn root() -> Gs {
        Gs { dest: Dest::Body, own: Own::None, ch: CharFmt::default(), pf: ParaFmt::default(), uc: 1, first: false, star: false, bt: Bt::None, tab_kind: "left", tab_leader: None }
    }
}

#[derive(Default, Clone)]
pub(super) struct Font {
    pub(super) name: String,
    pub(super) charset: i64,
    pub(super) cpg: Option<i64>,
}

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub(super) enum SKind {
    #[default]
    Para,
    Char,
    Other,
}

#[derive(Default)]
pub(super) struct StyleAcc {
    pub(super) kind: SKind,
    pub(super) num: i64,
    pub(super) based: Option<i64>,
    pub(super) next: Option<i64>,
    pub(super) name: String,
}

pub(super) struct StyleDef {
    pub(super) name: String,
    pub(super) based: Option<i64>,
    pub(super) next: Option<i64>,
    pub(super) ch: CharFmt,
    pub(super) pf: ParaFmt,
}

#[derive(Default, Clone)]
pub(super) struct LevelDef {
    pub(super) nfc: i64,
    pub(super) start: i64,
    /// The `\leveltext` units: a code and whether it came from `\'hh`.
    pub(super) text: Vec<(u32, bool)>,
    pub(super) font: Option<i64>,
    pub(super) li: Option<i64>,
    pub(super) fi: Option<i64>,
}

#[derive(Default)]
pub(super) struct ListDefAcc {
    pub(super) id: i64,
    pub(super) levels: Vec<LevelDef>,
}

#[derive(Default)]
pub(super) struct Override {
    pub(super) listid: i64,
    pub(super) starts: Vec<Option<i64>>,
}

/// A paragraph's list: numId, level, and the level's left indent and hanging.
pub(super) type ListRef = (i64, i64, Option<(i64, i64)>);

/// A list the body uses: its numId and each level's left indent and hanging.
#[derive(Clone)]
pub(super) struct ListUse {
    pub(super) num_id: i64,
    pub(super) ind: Vec<(i64, i64)>,
}

#[derive(Clone, PartialEq)]
pub(super) struct LinkTarget {
    pub(super) url: Option<String>,
    pub(super) anchor: Option<String>,
}

#[derive(Default)]
pub(super) struct FieldAcc {
    pub(super) inst: String,
}

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub(super) enum PKind {
    #[default]
    Unknown,
    Raster,
    Unsupported,
}

#[derive(Default)]
pub(super) struct PictAcc {
    pub(super) kind: PKind,
    pub(super) data: Vec<u8>,
    pub(super) nib: Option<u8>,
    pub(super) picw: i64,
    pub(super) pich: i64,
    pub(super) wgoal: i64,
    pub(super) hgoal: i64,
    pub(super) sx: i64,
    pub(super) sy: i64,
}

#[derive(Default, Clone, PartialEq)]
pub(super) struct PnSpec {
    pub(super) bullet: bool,
    pub(super) fmt: &'static str,
}

pub(super) struct PendRun {
    pub(super) fmt: CharFmt,
    pub(super) pstyle: Option<i64>,
    pub(super) link: bool,
    pub(super) pieces: Vec<Element>,
    pub(super) text: String,
}

pub(super) struct LinkAcc {
    pub(super) target: LinkTarget,
    pub(super) children: Vec<Element>,
}

#[derive(Default)]
pub(super) struct CurPara {
    pub(super) inlines: Vec<Element>,
    pub(super) link: Option<LinkAcc>,
}

#[derive(Default, Clone)]
pub(super) struct CellDef {
    pub(super) right: i64,
    pub(super) borders: [Option<Brd>; 4],
    pub(super) shd: Option<i64>,
    pub(super) vmerge: u8,
    pub(super) hmerge: u8,
    pub(super) valign: Option<&'static str>,
}

#[derive(Default, Clone)]
pub(super) struct RowDef {
    pub(super) left: i64,
    pub(super) gaph: Option<i64>,
    pub(super) height: i64,
    pub(super) header: bool,
    pub(super) keep: bool,
    pub(super) jc: Option<&'static str>,
    pub(super) cells: Vec<CellDef>,
    /// Top, left, bottom, right, inside horizontal, inside vertical.
    pub(super) borders: [Option<Brd>; 6],
}

pub(super) struct RowOut {
    pub(super) def: RowDef,
    pub(super) cells: Vec<Vec<Element>>,
}

#[derive(Default)]
pub(super) struct Tbl {
    pub(super) rows: Vec<RowOut>,
    pub(super) cells: Vec<Vec<Element>>,
    pub(super) cell_blocks: Vec<Element>,
    pub(super) row_open: bool,
    pub(super) def: RowDef,
    pub(super) cell_acc: CellDef,
}

pub(super) struct NoteInfo {
    pub(super) key: String,
    pub(super) endnote: bool,
}

/// A run of blocks being built: the body, or one note.
#[derive(Default)]
pub(super) struct Story {
    pub(super) blocks: Vec<Element>,
    pub(super) cur: Option<CurPara>,
    pub(super) run: Option<PendRun>,
    pub(super) link_active: Option<LinkTarget>,
    pub(super) tbl: Tbl,
    pub(super) note: Option<NoteInfo>,
    pub(super) chftn_done: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum EndKind {
    /// `\par`: an empty paragraph is made when none is open.
    Par,
    /// The end of a group or the text: only a paragraph that has content.
    Soft,
    /// `\cell`: the paragraph belongs to the cell whatever `\intbl` says.
    Cell,
}

pub(super) struct Interp {
    pub(super) b: Builder,
    pub(super) stack: Vec<Gs>,
    pub(super) over: usize,
    pub(super) rtf_seen: bool,
    /// The `\rtf` group has closed: whatever follows it is not part of the document.
    pub(super) done: bool,
    pub(super) skip: usize,
    pub(super) hi: Option<u16>,
    pub(super) any_content: bool,
    pub(super) ansicpg: u16,
    pub(super) deff: i64,
    pub(super) deflang: i64,
    pub(super) fonts: HashMap<i64, Font>,
    pub(super) font_cur: Option<i64>,
    pub(super) colors: Vec<Option<(u8, u8, u8)>>,
    pub(super) color_acc: (Option<u8>, Option<u8>, Option<u8>),
    pub(super) pstyles: HashMap<i64, StyleDef>,
    pub(super) cstyles: HashMap<i64, StyleDef>,
    pub(super) style_acc: StyleAcc,
    pub(super) style_ids: HashMap<(bool, i64), Option<String>>,
    pub(super) style_busy: HashSet<(bool, i64)>,
    pub(super) used_ids: HashSet<String>,
    pub(super) eff: HashMap<i64, (CharFmt, ParaFmt)>,
    pub(super) lists: HashMap<i64, Vec<LevelDef>>,
    pub(super) list_acc: ListDefAcc,
    pub(super) level_acc: LevelDef,
    pub(super) level_text: Vec<(u32, bool)>,
    pub(super) overrides: HashMap<i64, Override>,
    pub(super) ovr_acc: Override,
    pub(super) ovr_ls: i64,
    pub(super) lfo_start: Option<i64>,
    pub(super) ls_use: HashMap<i64, Option<ListUse>>,
    pub(super) pn_buf: String,
    pub(super) pn_seen: bool,
    pub(super) pn_spec: Option<PnSpec>,
    pub(super) legacy: Option<(PnSpec, ListUse)>,
    pub(super) fields: Vec<FieldAcc>,
    pub(super) pict: PictAcc,
    pub(super) stories: Vec<Story>,
    pub(super) notes: BTreeMap<String, FragNote>,
    pub(super) note_count: usize,
    pub(super) builtins: HashMap<&'static str, Option<String>>,
}
