//! Reveal formatting: for a position, which formatting the text has and where each setting comes from (the document
//! defaults, the paragraph style and the styles it builds on, a character style, or formatting put on by hand).

use super::*;
use crate::props::ParaFmt;
use crate::style_edit;
use serde::Serialize;
use serde_json::{json, Value};

/// Where a setting comes from.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RevealRow {
    /// `font`, `size`, `bold`, `alignment`, `spaceAfter`... The interface words it.
    pub key: String,
    /// `font` for text settings, `paragraph` for the rest.
    pub group: &'static str,
    pub value: Value,
    /// `default`, `paragraphStyle`, `characterStyle` or `direct`.
    pub source: &'static str,
    /// The name of the style the setting comes from, for the two style sources.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub style: Option<String>,
    /// The style is the person's own: its name is shown as written.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub custom: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct RevealStyle {
    pub id: String,
    pub name: String,
    /// The style is the person's own: its name is shown as written.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub custom: bool,
}

/// The layers of formatting at a position, and every setting the text has with the layer it comes from.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Reveal {
    /// The paragraph style, and the styles it builds on (the style first, then its base).
    pub paragraph_style: RevealStyle,
    pub based_on: Vec<RevealStyle>,
    pub character_style: Option<RevealStyle>,
    /// The paragraph has formatting put on by hand.
    pub direct_paragraph: bool,
    /// The text has formatting put on by hand.
    pub direct_run: bool,
    pub rows: Vec<RevealRow>,
}

/// One layer of formatting: where it comes from, and what it sets.
struct Layer<P, R> {
    source: &'static str,
    style: Option<RevealStyle>,
    para: P,
    run: R,
}

type Getter<'a, T> = (&'static str, Box<dyn Fn(&T) -> Option<Value> + 'a>);

impl Document {
    pub fn reveal_formatting(&self, at: Pos) -> Option<Reveal> {
        let (blocks, _) = self.locate(at.p.0);
        let p = find_para(blocks, at.p.0)?;
        let ppr = p.ppr.as_ref();
        let para_id = ppr.and_then(|e| e.child("w:pStyle")).and_then(|s| s.attr("w:val")).map(String::from).or_else(|| self.styles.default_para.clone());
        let name_of = |id: &str| {
            let custom = self.styles.style_element(id).is_some_and(|e| e.attr("w:customStyle").is_some_and(|v| v == "1" || v == "true"));
            RevealStyle { id: id.to_string(), name: style_edit::display_name(self.styles.get(id).map(|s| s.name.as_str()).unwrap_or(id), custom), custom }
        };
        // The text at the position: the run before it, or the paragraph mark in an empty paragraph.
        let direct_rpr: Option<Element> = edit::rpr_at(&p.inlines, at.o).or_else(|| ppr.and_then(|e| e.child("w:rPr")).cloned());
        let char_id = direct_rpr.as_ref().and_then(|r| r.child("w:rStyle")).and_then(|s| s.attr("w:val")).filter(|s| self.styles.exists(s)).map(String::from);
        let direct_para = ppr.map(style_edit::formatting_of).is_some_and(|v| !v.is_empty());
        let direct_run = direct_rpr.as_ref().map(style_edit::formatting_of).is_some_and(|v| !v.is_empty());

        let mut paras: Vec<Layer<ParaFmt, RunFmt>> = vec![Layer { source: "default", style: None, para: self.styles.doc_ppr.clone(), run: self.styles.doc_rpr.clone() }];
        let mut chain = Vec::new();
        if let Some(id) = &para_id {
            for cid in self.styles.chain_ids(id) {
                if let Some((pf, rf)) = self.styles.own_format(&cid) {
                    paras.push(Layer { source: "paragraphStyle", style: Some(name_of(&cid)), para: pf.clone(), run: rf.clone() });
                }
                chain.push(name_of(&cid));
            }
        }
        chain.reverse();
        if let Some(id) = &char_id {
            for cid in self.styles.chain_ids(id) {
                if let Some((_, rf)) = self.styles.own_format(&cid) {
                    paras.push(Layer { source: "characterStyle", style: Some(name_of(&cid)), para: ParaFmt::default(), run: rf.clone() });
                }
            }
        }
        // Language is not part of `RunFmt`: read it from each layer's XML.
        let mut langs: Vec<(&'static str, Option<RevealStyle>, Option<String>)> = vec![("default", None, self.styles.doc.as_ref().and_then(|d| d.root.path(&["w:docDefaults", "w:rPrDefault", "w:rPr"])).and_then(crate::styles::lang_of))];
        if let Some(id) = &para_id {
            for cid in self.styles.chain_ids(id) {
                langs.push(("paragraphStyle", Some(name_of(&cid)), self.styles.own_lang(&cid)));
            }
        }
        if let Some(id) = &char_id {
            for cid in self.styles.chain_ids(id) {
                langs.push(("characterStyle", Some(name_of(&cid)), self.styles.own_lang(&cid)));
            }
        }
        if let Some(l) = direct_rpr.as_ref().and_then(crate::styles::lang_of) {
            langs.push(("direct", None, Some(l)));
        }
        let direct_p = ppr.map(ParaFmt::from_ppr).unwrap_or_default();
        let direct_r = direct_rpr.as_ref().map(RunFmt::from_rpr).unwrap_or_default();
        // Direct paragraph formatting sits above the paragraph style; direct text formatting above the character style.
        let mut layers = paras;
        layers.push(Layer { source: "direct", style: None, para: direct_p, run: direct_r });

        let theme = &self.styles.theme;
        let font_of = |f: &RunFmt| f.font.clone().or_else(|| f.font_theme.as_deref().and_then(|t| theme.resolve(t)));
        let color_of = |f: &RunFmt| {
            f.ext
                .color_theme
                .as_ref()
                .and_then(|c| theme.colors.as_ref().and_then(|t| c.hex(t)))
                .or_else(|| f.color.clone())
                .filter(|c| c != "auto")
        };
        let run_keys: Vec<Getter<'_, RunFmt>> = vec![
            ("font", Box::new(move |f| font_of(f).map(Value::from))),
            ("size", Box::new(|f| f.size.map(Value::from))),
            ("bold", Box::new(|f| f.bold.map(Value::from))),
            ("italic", Box::new(|f| f.italic.map(Value::from))),
            ("underline", Box::new(|f| f.underline.clone().map(Value::from))),
            ("strike", Box::new(|f| f.strike.map(Value::from))),
            ("allCaps", Box::new(|f| f.caps.map(Value::from))),
            ("smallCaps", Box::new(|f| f.small_caps.map(Value::from))),
            ("color", Box::new(move |f| color_of(f).map(Value::from))),
            ("highlight", Box::new(|f| f.highlight.clone().filter(|h| h != "none").map(Value::from))),
            ("shading", Box::new(|f| f.shading.clone().map(Value::from))),
            ("vertAlign", Box::new(|f| f.vert_align.clone().filter(|v| v != "baseline").map(Value::from))),
            ("characterSpacing", Box::new(|f| f.spacing.map(Value::from))),
            ("hidden", Box::new(|f| f.vanish.map(Value::from))),
        ];
        let para_keys: Vec<Getter<'_, ParaFmt>> = vec![
            ("alignment", Box::new(|f| f.align.clone().map(Value::from))),
            ("indentLeft", Box::new(|f| f.ind_left.map(Value::from))),
            ("indentRight", Box::new(|f| f.ind_right.map(Value::from))),
            ("firstLine", Box::new(|f| f.ind_first_line.or(f.ind_hanging.map(|h| -h)).map(Value::from))),
            ("spaceBefore", Box::new(|f| f.space_before.map(Value::from))),
            ("spaceAfter", Box::new(|f| f.space_after.map(Value::from))),
            ("lineSpacing", Box::new(|f| f.line.map(|l| json!({"line": l, "rule": f.line_rule.clone().unwrap_or_else(|| "auto".into())})))),
            ("keepWithNext", Box::new(|f| f.keep_next.map(Value::from))),
            ("keepLinesTogether", Box::new(|f| f.keep_lines.map(Value::from))),
            ("pageBreakBefore", Box::new(|f| f.page_break_before.map(Value::from))),
            ("widowControl", Box::new(|f| f.widow_control.map(Value::from))),
            ("outlineLevel", Box::new(|f| f.outline_lvl.filter(|l| *l < 9).map(|l| Value::from(l + 1)))),
            ("list", Box::new(|f| f.num_id.filter(|n| *n > 0).map(Value::from))),
            ("borders", Box::new(|f| f.borders.as_ref().filter(|b| b.top.is_some() || b.bottom.is_some() || b.left.is_some() || b.right.is_some()).map(|_| Value::from(true)))),
            ("paragraphShading", Box::new(|f| f.shading.clone().map(Value::from))),
            ("tabs", Box::new(|f| f.tabs.as_ref().filter(|t| !t.is_empty()).map(|t| Value::from(t.len())))),
            ("rightToLeft", Box::new(|f| f.bidi.map(Value::from))),
        ];

        let mut rows = Vec::new();
        for (key, get) in &run_keys {
            // The top-most layer that sets it; a hand-set "off" is worth showing, an inherited "off" is not.
            if let Some((l, v)) = layers.iter().rev().find_map(|l| get(&l.run).map(|v| (l, v))) {
                if v == Value::Bool(false) && l.source != "direct" {
                    continue;
                }
                rows.push(RevealRow { key: (*key).into(), group: "font", value: v, source: l.source, style: l.style.as_ref().map(|s| s.name.clone()), custom: l.style.as_ref().is_some_and(|s| s.custom) });
            }
        }
        if let Some((source, style, v)) = langs.iter().rev().find_map(|(s, n, v)| v.as_ref().map(|v| (*s, n.clone(), v.clone()))) {
            rows.push(RevealRow { key: "language".into(), group: "font", value: Value::from(v), source, custom: style.as_ref().is_some_and(|s| s.custom), style: style.map(|s| s.name) });
        }
        for (key, get) in &para_keys {
            if let Some((l, v)) = layers.iter().rev().filter(|l| l.source != "characterStyle").find_map(|l| get(&l.para).map(|v| (l, v))) {
                if v == Value::Bool(false) && l.source != "direct" {
                    continue;
                }
                // Widow control is on unless a layer turns it off.
                rows.push(RevealRow { key: (*key).into(), group: "paragraph", value: v, source: l.source, style: l.style.as_ref().map(|s| s.name.clone()), custom: l.style.as_ref().is_some_and(|s| s.custom) });
            }
        }
        Some(Reveal { paragraph_style: para_id.as_deref().map(name_of).unwrap_or(RevealStyle { id: "Normal".into(), name: "Normal".into(), custom: false }), based_on: chain.into_iter().skip(1).collect(), character_style: char_id.as_deref().map(name_of), direct_paragraph: direct_para, direct_run, rows })
    }
}
