//! Defining lists: new `w:abstractNum`/`w:num` definitions written the way Word writes them, and reading
//! a list's definition back for the editor's dialogs and galleries.

use super::*;
use crate::props::RunPatch;
use serde::Deserialize;
use std::collections::HashSet;

fn default_format() -> String {
    "decimal".into()
}

/// One level of a list being defined. Sizes are in twips (indents, tab) or points (`size`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LevelDef {
    /// `w:numFmt`: `bullet`, `decimal`, `lowerRoman`, `upperLetter`, ... (see `numfmt::KNOWN_FORMATS`).
    #[serde(default = "default_format")]
    pub format: String,
    /// `w:lvlText`: the bullet character, or the label with the counts in it (`%1.`, `%1.%2.`, `(%3)`).
    pub text: String,
    /// The count the level starts at (default 1).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start: Option<i64>,
    /// `left`, `center` or `right`: where the label sits in the space before the text.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub align: Option<String>,
    /// What follows the label: `tab` (default), `space` or `nothing`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suffix: Option<String>,
    /// Left indent of the text, and how far the label hangs out before it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub left: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hanging: Option<i64>,
    /// A tab stop for the text after the label (`w:tab w:val="num"`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tab: Option<i64>,
    /// Font, size in points, bold, italic and colour (hex) of the label.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bold: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub italic: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    /// `w:lvlRestart`: 0 never restarts; n restarts after level n or higher. None restarts after any higher level.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub restart: Option<i64>,
    /// Legal numbering: the counts of earlier levels show as 1, 2, 3 whatever their own format.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub legal: bool,
    /// The paragraph style (a heading) linked to the level: its paragraphs take this level.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub style: Option<String>,
    /// A picture bullet: a media part added with `add_media`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub picture: Option<String>,
    /// The picture's size in points (default 9 by 9).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub picture_size: Option<(f64, f64)>,
}

impl LevelDef {
    pub fn is_bullet(&self) -> bool {
        self.format == "bullet"
    }
}

/// A list to define: one to nine levels (the rest are made from the last one).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ListDef {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub levels: Vec<LevelDef>,
}

/// What a level looks like at its first count, for gallery tiles.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct LevelPreview {
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font: Option<String>,
    /// A picture bullet.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pic: Option<LabelPic>,
}

/// A list the document uses that Docxtor didn't make from a preset (a gallery's "In this document" entry).
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ListDefinition {
    /// A num that uses the definition (what `apply_list` takes), and every num that does.
    pub num_id: i64,
    pub nums: Vec<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub bullet: bool,
    /// The first three levels.
    pub levels: Vec<LevelPreview>,
}

/// A stable pseudo-random 8-digit hex number, for `w:nsid` and `w:tmpl` (Word writes random ones).
fn hex8(seed: &str, salt: u64) -> String {
    let mut h: u64 = 0xcbf29ce484222325 ^ salt;
    for b in seed.bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x100000001b3);
    }
    format!("{:08X}", (h >> 16) as u32)
}

/// The text of a deeper level made from level `k`'s text: the same label with one more count.
fn deeper_text(text: &str, k: usize, i: usize) -> String {
    let mut found: Vec<usize> = (1..=9).filter_map(|n| text.find(&format!("%{n}"))).collect();
    found.sort();
    if let [.., a, b] = found[..] {
        // 1.1. becomes 1.1.1.: the separator between the last two counts goes between the last and the new one.
        let sep = &text[a + 2..b];
        let end = b + 2;
        return format!("{}{}%{}{}", &text[..end], sep, i + 1, &text[end..]);
    }
    text.replace(&format!("%{}", k + 1), &format!("%{}", i + 1))
}

/// A definition's nine levels: those given, then the rest made from the last given one.
fn complete(def: &ListDef) -> Result<Vec<LevelDef>, String> {
    if def.levels.is_empty() || def.levels.len() > 9 {
        return Err("a list has one to nine levels".into());
    }
    let mut out = def.levels.clone();
    let k = out.len() - 1;
    let last = out[k].clone();
    for i in k + 1..9 {
        let mut l = last.clone();
        l.left = Some(last.left.unwrap_or(720 * (k as i64 + 1)) + 720 * (i - k) as i64);
        l.style = None;
        l.start = last.start;
        if !l.is_bullet() {
            // A level that shows its own count only goes through 1. a. i. as the presets do; an outline (1.1.1.) stays numeric.
            let own_count_only = last.text.matches('%').count() == 1;
            if own_count_only && matches!(l.format.as_str(), "decimal" | "lowerLetter" | "upperLetter" | "lowerRoman" | "upperRoman" | "decimalZero") {
                l.format = ["decimal", "lowerLetter", "lowerRoman"][i % 3].into();
            }
            l.text = deeper_text(&out[i - 1].text, i - 1, i);
        }
        out.push(l);
    }
    for (i, l) in out.iter_mut().enumerate() {
        if l.left.is_none() {
            l.left = Some(720 * (i as i64 + 1));
        }
        if l.hanging.is_none() {
            l.hanging = Some(360);
        }
        if l.is_bullet() && l.text.is_empty() && l.picture.is_none() {
            l.text = "\u{2022}".into();
        }
    }
    Ok(out)
}

fn level_xml(i: usize, l: &LevelDef, pic_id: Option<i64>) -> Element {
    let mut lvl = Element::new("w:lvl").with_attr("w:ilvl", i.to_string());
    let val = |name: &'static str, v: String| Element::new(name).with_attr("w:val", v);
    lvl = lvl.with_child(val("w:start", l.start.unwrap_or(1).to_string()));
    lvl = lvl.with_child(val("w:numFmt", l.format.clone()));
    if let Some(r) = l.restart {
        lvl = lvl.with_child(val("w:lvlRestart", r.to_string()));
    }
    if let Some(s) = l.style.as_deref().filter(|s| !s.is_empty()) {
        lvl = lvl.with_child(val("w:pStyle", s.to_string()));
    }
    if l.legal {
        lvl = lvl.with_child(Element::new("w:isLgl"));
    }
    if let Some(s) = l.suffix.as_deref().filter(|s| *s == "space" || *s == "nothing") {
        lvl = lvl.with_child(val("w:suff", s.to_string()));
    }
    let text = if pic_id.is_some() { "\u{F0B7}".to_string() } else { l.text.clone() };
    lvl = lvl.with_child(val("w:lvlText", text));
    if let Some(id) = pic_id {
        lvl = lvl.with_child(val("w:lvlPicBulletId", id.to_string()));
    }
    let jc = l.align.as_deref().filter(|a| matches!(*a, "center" | "right")).unwrap_or("left");
    lvl = lvl.with_child(val("w:lvlJc", jc.to_string()));
    let mut ppr = Element::new("w:pPr");
    if let Some(tab) = l.tab {
        ppr = ppr.with_child(Element::new("w:tabs").with_child(Element::new("w:tab").with_attr("w:val", "num").with_attr("w:pos", tab.to_string())));
    }
    let mut ind = Element::new("w:ind").with_attr("w:left", l.left.unwrap_or(720).max(0).to_string());
    let hanging = l.hanging.unwrap_or(360);
    if hanging > 0 {
        ind.set_attr("w:hanging", hanging.to_string());
    } else if hanging < 0 {
        ind.set_attr("w:firstLine", (-hanging).to_string());
    }
    ppr = ppr.with_child(ind);
    lvl = lvl.with_child(ppr);
    let mut rpr = Element::new("w:rPr");
    let font = if pic_id.is_some() { Some("Symbol".to_string()) } else { l.font.clone().filter(|f| !f.is_empty()) };
    let patch = RunPatch { font, size: l.size.filter(|s| *s > 0.0), bold: l.bold.filter(|b| *b), italic: l.italic.filter(|b| *b), color: l.color.clone().filter(|c| !c.is_empty() && c != "auto"), ..Default::default() };
    patch.apply(&mut rpr, &RunFmt::default());
    if let Some(f) = rpr.child_mut("w:rFonts") {
        // Word writes the font for ASCII and Latin text and a hint, not the East Asian and complex ones.
        f.remove_attr("w:eastAsia");
        f.set_attr("w:hint", "default");
    }
    if !rpr.children.is_empty() {
        lvl = lvl.with_child(rpr);
    }
    lvl
}

impl Numbering {
    /// Add a list definition and a num that uses it. `pictures` maps the media parts the levels name to the
    /// relationship (of `numbering.xml`) that points at each. Returns `(abstractNumId, numId)`.
    pub fn define(&mut self, def: &ListDef, pictures: &HashMap<String, String>) -> Result<(i64, i64), String> {
        let levels = complete(def)?;
        for l in &levels {
            if l.format.is_empty() {
                return Err("a level needs a number format".into());
            }
            if !l.is_bullet() && !l.text.contains('%') && l.format != "none" {
                return Err(format!("the label text '{}' has no count in it (write %1 for the first level's count)", l.text));
            }
        }
        // A picture bullet per distinct picture.
        let mut pic_ids: HashMap<String, i64> = HashMap::new();
        let mut next_pic = self.pic_bullets.keys().max().map(|m| m + 1).unwrap_or(0);
        let mut new_pics: Vec<Element> = Vec::new();
        for l in &levels {
            let Some(media) = l.picture.as_deref() else { continue };
            if pic_ids.contains_key(media) {
                continue;
            }
            let rid = pictures.get(media).ok_or_else(|| format!("picture {media} isn't in the document"))?;
            let (w, h) = l.picture_size.filter(|(w, h)| *w > 0.0 && *h > 0.0).unwrap_or((9.0, 9.0));
            let shape = Element::new("v:shape")
                .with_attr("id", format!("_x0000_i{}", 1025 + next_pic))
                .with_attr("type", "#_x0000_t75")
                .with_attr("style", format!("width:{w}pt;height:{h}pt"))
                .with_attr("o:bullet", "t")
                .with_child(Element::new("v:imagedata").with_attr("r:id", rid).with_attr("o:title", ""));
            new_pics.push(Element::new("w:numPicBullet").with_attr("w:numPicBulletId", next_pic.to_string()).with_child(Element::new("w:pict").with_child(shape)));
            pic_ids.insert(media.to_string(), next_pic);
            next_pic += 1;
        }
        if !new_pics.is_empty() {
            let root = self.ensure_doc();
            for (prefix, uri) in [("xmlns:v", "urn:schemas-microsoft-com:vml"), ("xmlns:o", "urn:schemas-microsoft-com:office:office"), ("xmlns:r", "http://schemas.openxmlformats.org/officeDocument/2006/relationships")] {
                if root.attr(prefix).is_none() {
                    root.set_attr(prefix, uri);
                }
            }
            let at = root.children.iter().position(|n| !matches!(n, Node::Element(e) if e.name == "w:numPicBullet")).unwrap_or(root.children.len());
            for (k, pic) in new_pics.into_iter().enumerate() {
                root.children.insert(at + k, Node::Element(pic));
            }
            self.dirty = true;
            self.reload();
        }

        // Each heading is linked to one level only: take the style off any level that had it.
        let linked: HashSet<&str> = levels.iter().filter_map(|l| l.style.as_deref()).filter(|s| !s.is_empty()).collect();
        if !linked.is_empty() {
            if let Some(doc) = self.doc.as_mut() {
                for a in doc.root.elements_mut().filter(|e| e.name == "w:abstractNum") {
                    for lvl in a.elements_mut().filter(|e| e.name == "w:lvl") {
                        if lvl.child("w:pStyle").and_then(|p| p.attr("w:val")).is_some_and(|s| linked.contains(s)) {
                            lvl.remove_children("w:pStyle");
                        }
                    }
                }
            }
        }

        let abs_id = self.next_abstract_id();
        let num_id = self.next_num_id();
        let seed = format!("{}{}{:?}", abs_id, def.name.as_deref().unwrap_or(""), levels.iter().map(|l| l.text.as_str()).collect::<Vec<_>>());
        let multi = if def.levels.len() > 1 { "multilevel" } else { "hybridMultilevel" };
        let mut abs = Element::new("w:abstractNum")
            .with_attr("w:abstractNumId", abs_id.to_string())
            .with_child(Element::new("w:nsid").with_attr("w:val", hex8(&seed, 1)))
            .with_child(Element::new("w:multiLevelType").with_attr("w:val", multi))
            .with_child(Element::new("w:tmpl").with_attr("w:val", hex8(&seed, 2)));
        if let Some(name) = def.name.as_deref().filter(|n| !n.is_empty()) {
            abs = abs.with_child(Element::new("w:name").with_attr("w:val", name));
        }
        for (i, l) in levels.iter().enumerate() {
            let pic = l.picture.as_deref().and_then(|m| pic_ids.get(m).copied());
            abs = abs.with_child(level_xml(i, l, pic));
        }
        let num = Element::new("w:num").with_attr("w:numId", num_id.to_string()).with_child(Element::new("w:abstractNumId").with_attr("w:val", abs_id.to_string()));
        self.insert_abstract(abs);
        self.insert_num(num);
        Ok((abs_id, num_id))
    }

    /// The definition behind a num, as the dialogs show it. A symbol-font bullet comes back as the Unicode
    /// character that looks the same, in no font.
    pub fn definition(&self, num_id: i64) -> Option<ListDef> {
        self.nums.get(&num_id)?;
        let mut levels = Vec::new();
        for i in 0..9 {
            let Some(l) = self.level(num_id, i) else { break };
            let bullet = l.fmt == "bullet";
            let symbol = bullet && crate::list_glyphs::is_symbol_font(l.rpr.font.as_deref());
            let text = if bullet { crate::list_glyphs::map_bullet(&l.text, l.rpr.font.as_deref()) } else { l.text.clone() };
            levels.push(LevelDef {
                format: l.fmt.clone(),
                text,
                start: Some(l.start),
                align: Some(l.jc.clone()),
                suffix: Some(l.suff.clone()),
                left: l.ppr.ind_left,
                hanging: l.ppr.ind_hanging.or(l.ppr.ind_first_line.map(|f| -f)),
                tab: l.ppr.tabs.as_ref().and_then(|t| t.iter().find(|t| t.val == "num").map(|t| t.pos)),
                font: if symbol { None } else { l.rpr.font.clone() },
                size: l.rpr.size,
                bold: l.rpr.bold,
                italic: l.rpr.italic,
                color: l.rpr.color.clone().filter(|c| c != "auto"),
                restart: l.restart,
                legal: l.is_lgl,
                style: l.pstyle.clone(),
                picture: self.label_pic(l).map(|p| p.src),
                picture_size: self.label_pic(l).map(|p| (p.w, p.h)),
            });
        }
        if levels.is_empty() {
            return None;
        }
        let abs = self.abstracts.get(&self.nums.get(&num_id)?.abstract_id)?;
        Some(ListDef { name: abs.name.clone().filter(|n| !n.starts_with("Docxtor ")), levels })
    }

    /// What a level shows at its first count, with the earlier levels at theirs: `1.`, `1.1.`, `•`.
    fn preview(&self, num_id: i64, ilvl: i64) -> Option<LevelPreview> {
        let l = self.level(num_id, ilvl)?;
        let pic = if l.fmt == "bullet" { self.label_pic(l) } else { None };
        if l.fmt == "bullet" {
            let symbol = crate::list_glyphs::is_symbol_font(l.rpr.font.as_deref());
            return Some(LevelPreview { text: crate::list_glyphs::map_bullet(&l.text, l.rpr.font.as_deref()), font: if symbol { None } else { l.rpr.font.clone() }, pic });
        }
        let mut t = l.text.clone();
        for n in 0..9i64 {
            let ph = format!("%{}", n + 1);
            if t.contains(&ph) {
                let lv = self.level(num_id, n);
                let fmt = if l.is_lgl { "decimal" } else { lv.map(|x| x.fmt.as_str()).unwrap_or("decimal") };
                t = t.replace(&ph, &crate::numfmt::format(lv.map(|x| x.start).unwrap_or(1), fmt, None));
            }
        }
        Some(LevelPreview { text: t, font: l.rpr.font.clone(), pic: None })
    }

    /// The lists in `used` that aren't Docxtor's own presets, one per definition: the gallery's "In this document"
    /// entries.
    pub fn definitions_in_use(&self, used: &HashSet<i64>) -> Vec<ListDefinition> {
        let mut ids: Vec<i64> = used.iter().copied().filter(|n| self.nums.contains_key(n)).collect();
        ids.sort();
        // The nums of each definition, in the order their first num comes.
        let mut groups: Vec<(i64, Vec<i64>)> = Vec::new();
        for id in ids {
            let abs = self.effective_abstract(self.nums[&id].abstract_id);
            match groups.iter_mut().find(|(a, _)| *a == abs) {
                Some((_, nums)) => nums.push(id),
                None => groups.push((abs, vec![id])),
            }
        }
        let mut out = Vec::new();
        for (abs, nums) in groups {
            let id = nums[0];
            if self.abstracts.get(&abs).and_then(|a| a.name.as_deref()).is_some_and(|n| preset_from_name(n).is_some()) {
                continue;
            }
            // The checklist boxes are a preset whatever their definition is called.
            if self.checkbox_at(id, 0).is_some() {
                continue;
            }
            let levels: Vec<LevelPreview> = (0..3).filter_map(|l| self.preview(id, l)).collect();
            if levels.is_empty() {
                continue;
            }
            out.push(ListDefinition { num_id: id, nums, name: self.abstracts.get(&abs).and_then(|a| a.name.clone()), bullet: self.is_bullet(id), levels });
        }
        out
    }

    /// Put the heading styles a definition links to into the nums that use it: the `numPr` each linked style needs.
    /// `(style id, level)` for every level of a num that names a style.
    pub fn linked_styles(&self, num_id: i64) -> Vec<(String, i64)> {
        (0..9).filter_map(|l| self.level(num_id, l).and_then(|lv| lv.pstyle.clone()).map(|s| (s, l))).collect()
    }

    /// A num's definition and the relationship each picture of a level needs, for the document to add.
    pub fn picture_parts(def: &ListDef) -> Vec<String> {
        let mut v: Vec<String> = def.levels.iter().filter_map(|l| l.picture.clone()).collect();
        v.sort();
        v.dedup();
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lvl(format: &str, text: &str) -> LevelDef {
        LevelDef { format: format.into(), text: text.into(), ..Default::default() }
    }

    #[test]
    fn deeper_levels_are_made_from_the_last() {
        let def = ListDef { name: None, levels: vec![lvl("decimal", "%1.")] };
        let l = complete(&def).unwrap();
        assert_eq!(l.len(), 9);
        assert_eq!(l[1].format, "lowerLetter");
        assert_eq!(l[1].text, "%2.");
        assert_eq!(l[2].format, "lowerRoman");
        assert_eq!(l[3].format, "decimal");
        assert_eq!(l[3].left, Some(720 * 4));
        let outline = ListDef { name: None, levels: vec![lvl("decimal", "%1."), lvl("decimal", "%1.%2.")] };
        let l = complete(&outline).unwrap();
        assert_eq!(l[2].text, "%1.%2.%3.");
        assert_eq!(l[3].text, "%1.%2.%3.%4.");
        assert!(complete(&ListDef::default()).is_err());
    }

    #[test]
    fn define_writes_what_word_writes() {
        let mut n = Numbering::parse(None);
        let def = ListDef {
            name: Some("Legal".into()),
            levels: vec![
                LevelDef { start: Some(3), align: Some("right".into()), suffix: Some("space".into()), tab: Some(900), font: Some("Arial".into()), bold: Some(true), color: Some("FF0000".into()), size: Some(11.0), restart: Some(0), style: Some("Heading1".into()), ..lvl("upperRoman", "Article %1") },
                LevelDef { legal: true, ..lvl("decimal", "%1.%2.") },
            ],
        };
        let (abs, num) = n.define(&def, &HashMap::new()).unwrap();
        let xml = n.doc.as_ref().unwrap().root.to_xml();
        assert!(xml.contains(&format!(r#"w:abstractNumId="{abs}""#)));
        let a = n.doc.as_ref().unwrap().root.child("w:abstractNum").unwrap();
        let names: Vec<_> = a.elements().map(|e| e.name.as_str()).take(5).collect();
        assert_eq!(names, ["w:nsid", "w:multiLevelType", "w:tmpl", "w:name", "w:lvl"]);
        assert_eq!(a.child("w:multiLevelType").unwrap().attr("w:val"), Some("multilevel"));
        let l0 = a.child("w:lvl").unwrap();
        let order: Vec<_> = l0.elements().map(|e| e.name.as_str()).collect();
        assert_eq!(order, ["w:start", "w:numFmt", "w:lvlRestart", "w:pStyle", "w:suff", "w:lvlText", "w:lvlJc", "w:pPr", "w:rPr"]);
        let level = n.level(num, 0).unwrap();
        assert_eq!((level.start, level.fmt.as_str(), level.jc.as_str(), level.suff.as_str()), (3, "upperRoman", "right", "space"));
        assert_eq!(level.restart, Some(0));
        assert_eq!(level.pstyle.as_deref(), Some("Heading1"));
        assert_eq!(level.ppr.tabs.as_ref().unwrap()[0].val, "num");
        assert_eq!(level.rpr.color.as_deref(), Some("FF0000"));
        assert_eq!(level.rpr.size, Some(11.0));
        assert!(n.level(num, 1).unwrap().is_lgl);
        assert!(n.level(num, 8).is_some());
        assert_eq!(n.linked_styles(num), [("Heading1".to_string(), 0)]);
        // A second definition takes Heading1 off the first.
        let again = ListDef { name: None, levels: vec![LevelDef { style: Some("Heading1".into()), ..lvl("decimal", "%1.") }] };
        let (_, second) = n.define(&again, &HashMap::new()).unwrap();
        assert_eq!(n.level(num, 0).unwrap().pstyle, None);
        assert_eq!(n.level(second, 0).unwrap().pstyle.as_deref(), Some("Heading1"));
        // Read back, it is the definition that was written.
        let back = n.definition(num).unwrap();
        assert_eq!(back.name.as_deref(), Some("Legal"));
        assert_eq!(back.levels[0].format, "upperRoman");
        assert_eq!(back.levels[0].align.as_deref(), Some("right"));
        assert_eq!(back.levels[0].tab, Some(900));
        assert_eq!(back.levels.len(), 9);
        // The text must show a count.
        let bad = ListDef { name: None, levels: vec![lvl("decimal", "none")] };
        assert!(n.define(&bad, &HashMap::new()).is_err());
    }

    #[test]
    fn picture_bullets_need_a_picture_part() {
        let mut n = Numbering::parse(None);
        let def = ListDef { name: None, levels: vec![LevelDef { picture: Some("word/media/docxtor1.png".into()), ..lvl("bullet", "") }] };
        assert!(n.define(&def, &HashMap::new()).is_err());
        let mut pics = HashMap::new();
        pics.insert("word/media/docxtor1.png".to_string(), "rId1".to_string());
        let (_, num) = n.define(&def, &pics).unwrap();
        n.rels.insert("rId1".into(), "word/media/docxtor1.png".into());
        let root = &n.doc.as_ref().unwrap().root;
        assert!(root.attr("xmlns:v").is_some() && root.attr("xmlns:r").is_some() && root.attr("xmlns:o").is_some());
        let names: Vec<_> = root.elements().map(|e| e.name.as_str()).collect();
        assert_eq!(names, ["w:numPicBullet", "w:abstractNum", "w:num"]);
        let level = n.level(num, 0).unwrap();
        assert_eq!(level.pic, Some(0));
        let pic = n.label_pic(level).unwrap();
        assert_eq!((pic.src.as_str(), pic.w, pic.h), ("word/media/docxtor1.png", 9.0, 9.0));
        assert_eq!(n.definition(num).unwrap().levels[0].picture.as_deref(), Some("word/media/docxtor1.png"));
    }
}
