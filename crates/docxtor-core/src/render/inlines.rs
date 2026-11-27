//! Rendering the inline content of a paragraph: runs, fields, form fields and note references.

use super::*;

impl<'a> RenderCtx<'a> {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn inlines(
        &self,
        inlines: &[Inline],
        base: &RunFmt,
        start: &mut usize,
        segs: &mut Vec<RSeg>,
        field: &mut FieldState,
        link: Option<&str>,
        ins: bool,
    ) {
        for inl in inlines {
            match inl {
                Inline::Run(r) => self.run(r, base, start, segs, field, link, ins),
                Inline::Container(c) => {
                    let mut link_here = link.map(String::from);
                    let mut fld_simple = None;
                    match c.kind {
                        InlineKind::Hyperlink => {
                            link_here = c
                                .shell
                                .attr("r:id")
                                .and_then(|id| self.rels.get(id))
                                .map(|r| r.target.clone())
                                .or_else(|| c.shell.attr("w:anchor").map(|a| format!("#{a}")));
                        }
                        InlineKind::Field => {
                            fld_simple = c.shell.attr("w:instr").map(field_name);
                            field.pending_fi = c.shell.attr("w:instr").map(String::from);
                            field.new_field = true;
                            // A cross-reference with `\h` is a link to its bookmark.
                            if link_here.is_none() {
                                link_here = c.shell.attr("w:instr").and_then(ref_link);
                            }
                        }
                        _ => {}
                    }
                    // A simple field's result renders as its own segments (never merged with neighbours).
                    let outer = fld_simple.map(|f| field.result_of.replace(f));
                    // So does the content of a content control: the editor draws a control from the segments inside it.
                    if c.kind == InlineKind::Sdt {
                        field.cite_new = true;
                    }
                    self.inlines(&c.children, base, start, segs, field, link_here.as_deref(), ins || c.kind == InlineKind::Insertion);
                    if c.kind == InlineKind::Sdt {
                        field.cite_new = true;
                    }
                    if let Some(outer) = outer {
                        field.result_of = outer;
                        field.pending_fi = None;
                        field.new_field = false;
                    }
                }
                Inline::Opaque(e) => {
                    if crate::math::is_math(e) {
                        // An equation takes one position, like an image.
                        segs.push(RSeg {
                            o: *start,
                            k: "math",
                            x: None,
                            f: base.clone(),
                            link: None,
                            img: None,
                            label: None,
                            fld: None,
                            cite: None,
                            fi: None,
                            ins,
                            hid: false,
                            note: None,
                            shape: None,
                            chart: None,
                            chartex: None,
                            ruby: None,
                            hrule: None,
                            math: Some(crate::math::info(e)),
                            hidden: false,
                        });
                        *start += 1;
                        continue;
                    }
                    // Visible content Docxtor doesn't edit yet: shown read-only, zero length in the model.
                    if matches!(e.name.as_str(), "w:del" | "w:moveFrom") {
                        let mut ts = Vec::new();
                        e.find_all("w:delText", &mut ts);
                        let mut t2 = Vec::new();
                        e.find_all("w:t", &mut t2);
                        ts.extend(t2);
                        let text: String = ts.iter().map(|t| t.text()).collect();
                        if !text.is_empty() {
                            segs.push(RSeg {
                                o: *start,
                                k: "ghost",
                                x: Some(text),
                                f: base.clone(),
                                link: None,
                                img: None,
                                label: Some("del".into()),
                                fld: None,
                                cite: None,
                                fi: None,
                                ins: false,
                                hid: false,
                                note: None,
                                shape: None,
                                math: None,
                                chart: None,
                                chartex: None,
                                ruby: None,
                                hrule: None,
                                hidden: false,
                            });
                        }
                    }
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn run(&self, r: &Run, base: &RunFmt, start: &mut usize, segs: &mut Vec<RSeg>, field: &mut FieldState, link: Option<&str>, ins: bool) {
        let direct = r.rpr.as_ref().map(RunFmt::from_rpr).unwrap_or_default();
        let mut f = self.styles.finish_run(self.run_fmt(base, &direct));
        crate::charfmt::resolve_run(&mut f, self.shapes.theme(self.pkg));
        f.style = None;
        f.font_theme = None;
        let f_cs = cs_variant(&f);
        let f_ea = ea_variant(&f);
        // The East Asian language marks East Asian text only: the rest of the text doesn't carry it.
        let mut f_cs = f_cs;
        f_cs.ext.ea.lang_ea = None;
        let mut f = strip_cs(f);
        f.ext.ea.lang_ea = None;
        let seg = |k: &'static str, o: usize, x: Option<String>, f: &RunFmt, fld: &Option<String>, link: Option<&str>, cite: &Option<Vec<String>>| RSeg {
            o,
            k,
            // Google Docs marks where a code block starts and ends with private characters (U+EC03, U+EC02) that
            // have no glyph: they show as nothing, one position wide, as in Google Docs.
            x: if k == "t" { x.map(google_marks) } else { x },
            f: f.clone(),
            link: link.map(String::from),
            img: None,
            label: None,
            fld: fld.clone(),
            cite: cite.clone(),
            fi: None,
            ins,
            hid: false,
            note: None,
            shape: None,
            math: None,
            chart: None,
            chartex: None,
            ruby: None,
            hrule: None,
            hidden: false,
        };
        for item in &r.items {
            let n0 = segs.len();
            let fresh_cite = std::mem::take(&mut field.cite_new) | std::mem::take(&mut field.new_field);
            // The result of a cross-reference with `\h` links to its bookmark.
            let eff_link: Option<String> = link.map(String::from).or_else(|| field.link.clone());
            let link = eff_link.as_deref();
            match item {
                RunItem::Text(t) if f.font.as_deref().is_some_and(crate::symbol_fonts::is_symbol_font) && t.chars().any(|c| crate::symbol_fonts::map_code(f.font.as_deref().unwrap_or(""), c as u32).is_some()) => {
                    // Text in Symbol, Wingdings or Webdings: each character is a picture, drawn as the Unicode
                    // character that looks like it. Each one takes the place the original character did.
                    let font = f.font.clone().unwrap_or_default();
                    let mut at = *start;
                    for c in t.chars() {
                        match crate::symbol_fonts::map_code(&font, c as u32) {
                            Some(m) => {
                                let mut sf = f.clone();
                                sf.font = None;
                                segs.push(seg("sym", at, Some(m.to_string()), &sf, &field.result_of, link, &field.cite));
                            }
                            None => segs.push(seg("t", at, Some(c.to_string()), &f, &field.result_of, link, &field.cite)),
                        }
                        at += c.len_utf16();
                    }
                }
                RunItem::Text(t) => {
                    // Complex-script text (Arabic, Hebrew, Thai, ...) takes the complex-script size, bold, italic and font.
                    let mut at = *start;
                    let mut fresh_cite = fresh_cite;
                    for (piece, script) in script_pieces(t, f.rtl == Some(true)) {
                        let pf_new = match script {
                            Script::Complex => f_cs.clone(),
                            Script::EastAsian => f_ea.clone(),
                            Script::Other => f.clone(),
                        };
                        let len = piece.encode_utf16().count();
                        if let Some(RSeg { k: "t", x: Some(prev), f: pf, link: pl, fld: pfl, ins: pi, cite: pc, hid: ph, .. }) = segs.last_mut() {
                            if *pf == pf_new && pl.as_deref() == link && *pfl == field.result_of && *pi == ins && *pc == field.cite && !fresh_cite && *ph == field.code_open {
                                prev.push_str(&google_marks(piece.to_string()));
                                at += len;
                                continue;
                            }
                        }
                        fresh_cite = false;
                        let mut s = seg("t", at, Some(piece.to_string()), &pf_new, &field.result_of, link, &field.cite);
                        s.hid = field.code_open;
                        segs.push(s);
                        at += len;
                    }
                }
                RunItem::Tab => segs.push(seg("tab", *start, None, &f, &field.result_of, link, &field.cite)),
                RunItem::Break(e) => {
                    let k = match e.attr("w:type") {
                        Some("page") => "pbr",
                        Some("column") => "cbr",
                        _ => "br",
                    };
                    segs.push(seg(k, *start, None, &f, &field.result_of, link, &field.cite));
                }
                // A watermark is drawn from the page setup, not in the text.
                RunItem::Object(e) if crate::marks::is_watermark_pict(e) => {}
                RunItem::Object(e) => {
                    let mut s = seg("obj", *start, None, &f, &field.result_of, link, &field.cite);
                    s.hidden = crate::shapes::object_props::is_hidden_drawing(e);
                    if let Some(h) = vml_hrule(e) {
                        s.k = "hrule";
                        s.label = Some("Horizontal line".into());
                        s.hrule = Some(h);
                    } else if crate::ruby::is_ruby(e) {
                        let (bf, info) = self.ruby_info(e, base, &direct);
                        s.k = "ruby";
                        s.f = bf;
                        s.ruby = Some(Box::new(info));
                    } else if let Some(sh) = self.shape(e) {
                        s.k = "shape";
                        s.label = Some(
                            if sh.kind == "diagram" {
                                "SmartArt graphic"
                            } else if sh.warp.is_some() {
                                "WordArt"
                            } else if sh.text_box {
                                "Text box"
                            } else {
                                "Shape"
                            }
                            .into(),
                        );
                        s.shape = Some(Box::new(sh));
                    } else if let Some((img, model)) = self.chart(e) {
                        s.k = "chart";
                        s.img = Some(Box::new(img));
                        s.chart = Some(Box::new(model));
                    } else if let Some((img, model)) = self.chartex(e) {
                        s.k = "chart";
                        s.img = Some(Box::new(img));
                        s.label = Some(model.summary());
                        s.chartex = Some(Box::new(model));
                    } else if let Some(img) = self.image(e) {
                        s.k = "img";
                        s.img = Some(Box::new(img));
                    } else if let Some((id, label, endnote)) = self.note_ref(e) {
                        s.k = "note";
                        s.x = Some(id);
                        s.label = Some(label);
                        s.note = endnote.then_some("endnote");
                    } else if matches!(e.name.as_str(), "w:footnoteReference" | "w:endnoteReference") && e.attr("w:customMarkFollows").is_some() {
                        // The custom mark that follows is the visible mark; the reference itself shows nothing.
                        s.k = "note";
                        s.x = e.attr("w:id").map(String::from);
                        s.label = Some(String::new());
                        s.note = (e.name == "w:endnoteReference").then_some("endnote");
                    } else if let Some((fld, shown)) = date_element(&e.name) {
                        s.k = "val";
                        s.fld = Some(fld.into());
                        s.x = Some(shown.into());
                        s.label = None;
                    } else {
                        s.label = Some(self.object_label(e));
                        if e.name == "w:ptab" {
                            // An absolute position tab: where the text after it goes, from what, and the leader.
                            s.x = Some(format!(
                                "ptab:{}:{}:{}",
                                e.attr("w:alignment").unwrap_or("left"),
                                e.attr("w:relativeTo").unwrap_or("margin"),
                                e.attr("w:leader").unwrap_or("none")
                            ));
                        }
                        if e.name == "w:sym" {
                            let font = e.attr("w:font").unwrap_or("");
                            s.k = "sym";
                            s.x = Some(sym_char(e));
                            s.label = None;
                            // A symbol font's character is drawn as its look-alike; any other font draws its own.
                            if crate::symbol_fonts::is_symbol_font(font) {
                                s.f.font = None;
                            } else if !font.is_empty() {
                                s.f.font = Some(font.to_string());
                            }
                        }
                    }
                    segs.push(s);
                }
                RunItem::Hidden(e) => match e.name.as_str() {
                    "w:fldChar" => match e.attr("w:fldCharType") {
                        Some("begin") => {
                            field.depth += 1;
                            field.instrs.push(String::new());
                            field.ff = if field.depth == 1 { e.child("w:ffData").cloned() } else { None };
                            if field.depth == 1 {
                                field.code_open = true;
                            }
                            // A legacy check box: drawn as a box, checked or not.
                            if field.depth == 1 {
                                if let Some(cb) = e.child("w:ffData").and_then(|d| d.child("w:checkBox")) {
                                    let on = |el: &Element| !matches!(el.attr("w:val"), Some("0" | "false" | "off"));
                                    let checked = cb.child("w:checked").map(on).unwrap_or_else(|| cb.child("w:default").is_some_and(on));
                                    let mut s = seg("ff", *start, Some(if checked { "1" } else { "0" }.into()), &f, &None, link, &None);
                                    s.label = Some("checkbox".into());
                                    // The number of the form field in its paragraph: ticking it is an op on that field.
                                    s.fi = Some(field.form_n.to_string());
                                    segs.push(s);
                                }
                            }
                        }
                        Some("separate") => {
                            if field.depth == 1 {
                                field.code_open = false;
                                let instr = field.instrs.first().cloned().unwrap_or_default();
                                field.result_of = Some(field_name(&instr));
                                field.link = ref_link(&instr);
                                field.new_field = true;
                                field.pending_fi = Some(instr.clone());
                                if field.result_of.as_deref() == Some("CITATION") {
                                    field.cite = crate::citations::CiteInstr::parse(&instr).map(|c| c.tags);
                                    field.cite_new = field.cite.is_some();
                                }
                            }
                        }
                        Some("end") => {
                            let instr = field.instrs.pop().unwrap_or_default();
                            field.depth = field.depth.saturating_sub(1);
                            if field.depth == 0 {
                                field.code_open = false;
                                self.form_seg(field, &instr, f.clone(), *start, ins, link, segs);
                                // An equation field is drawn where it ends.
                                if field_name(&instr) == "EQ" {
                                    if let Some(ml) = crate::field_eq::eq_to_mathml(&instr) {
                                        let mut s = seg("eq", *start, None, &f, &None, None, &None);
                                        s.fi = Some(instr.clone());
                                        s.math = Some(crate::math::MathInfo { mathml: ml, latex: crate::field_eq::eq_plain(&instr), display: false, jc: None });
                                        segs.push(s);
                                    }
                                }
                                field.result_of = None;
                                field.link = None;
                                field.cite = None;
                                field.pending_fi = None;
                                field.new_field = false;
                            }
                        }
                        _ => {}
                    },
                    "w:instrText" => {
                        if let Some(i) = field.instrs.last_mut() {
                            i.push_str(&e.text());
                        }
                    }
                    _ => {}
                },
            }
            // The first piece of a field's result carries the field's instruction.
            if segs.len() > n0 {
                if let Some(fi) = field.pending_fi.take() {
                    segs[n0].fi = Some(fi);
                }
            }
            *start += item.len16();
        }
    }

    /// At the end of a legacy form field: a check box or drop-down has no text of its own, so it gets a segment
    /// (zero width, like a tracked deletion) the editor draws and clicks; `x` is `dropdown:n`,
    /// `n` being the field's number among the paragraph's form fields, and `label` the entry a drop-down shows.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn form_seg(&self, field: &mut FieldState, instr: &str, f: RunFmt, at: usize, ins: bool, link: Option<&str>, segs: &mut Vec<RSeg>) {
        let name = field_name(instr);
        let ff = field.ff.take();
        if !matches!(name.as_str(), "FORMTEXT" | "FORMCHECKBOX" | "FORMDROPDOWN") {
            return;
        }
        let n = field.form_n;
        field.form_n += 1;
        // A text field is its result; a check box is drawn at the start of the field (`ff`).
        if name == "FORMTEXT" || name == "FORMCHECKBOX" {
            return;
        }
        let (kind, label) = if name == "FORMCHECKBOX" {
            ("checkbox", None)
        } else {
            let dd = ff.as_ref().and_then(|e| e.child("w:ddList"));
            let entries: Vec<&str> = dd.map(|d| d.children_named("w:listEntry").filter_map(|e| e.attr("w:val")).collect()).unwrap_or_default();
            let sel = dd.and_then(|d| d.child("w:result")).and_then(|r| r.attr("w:val")).and_then(|v| v.parse::<usize>().ok()).unwrap_or(0);
            ("dropdown", Some(entries.get(sel).copied().unwrap_or("").to_string()))
        };
        segs.push(RSeg {
            o: at,
            k: "form",
            x: Some(format!("{kind}:{n}")),
            f,
            link: link.map(String::from),
            img: None,
            label,
            fld: None,
            cite: None,
            ins,
            note: None,
            shape: None,
            chart: None,
            chartex: None,
            ruby: None,
            hrule: None,
            math: None,
            fi: None,
            hid: false,
            hidden: false,
        });
    }

    /// A note reference (or a note's own mark): note id, formatted number and whether it is an endnote.
    pub(super) fn note_ref(&self, e: &Element) -> Option<(String, String, bool)> {
        match e.name.as_str() {
            // A reference whose mark is the text that follows it (`customMarkFollows`) shows nothing of its own.
            "w:footnoteReference" | "w:endnoteReference" if e.attr("w:customMarkFollows").is_some() => {
                Some((e.attr("w:id")?.to_string(), String::new(), e.name == "w:endnoteReference"))
            }
            "w:footnoteReference" => {
                let id = e.attr("w:id")?.to_string();
                let n = match self.notes.get(&id) {
                    Some(n) => *n,
                    None => {
                        let n = self.footnote_counter.get() + 1;
                        self.footnote_counter.set(n);
                        n
                    }
                };
                let label = self.note_labels.get(&id).cloned().unwrap_or_else(|| self.footnote_fmt.label(n));
                Some((id, label, false))
            }
            "w:endnoteReference" => {
                let id = e.attr("w:id")?.to_string();
                let n = match self.endnotes.get(&id) {
                    Some(n) => *n,
                    None => {
                        let n = self.endnote_counter.get() + 1;
                        self.endnote_counter.set(n);
                        n
                    }
                };
                let label = self.note_labels.get(&format!("e:{id}")).cloned().unwrap_or_else(|| self.endnote_fmt.label(n));
                Some((id, label, true))
            }
            "w:footnoteRef" => {
                let n = self.current_note?;
                Some((String::new(), self.current_note_label.clone().unwrap_or_else(|| self.footnote_fmt.label(n)), false))
            }
            "w:endnoteRef" => {
                let n = self.current_note?;
                Some((String::new(), self.current_note_label.clone().unwrap_or_else(|| self.endnote_fmt.label(n)), true))
            }
            _ => None,
        }
    }
}
