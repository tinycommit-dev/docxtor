//! Performance counts for the engine: exact numbers that do not move with how busy the machine is. Not a test of
//! correctness: it does nothing unless `DOCXTOR_PERF=1`, and then prints one `@@perf {json}` line per document size for
//! `perf/quick.mts` to gate (see docs/performance/index.md).
//!
//! For each size (paragraphs, `DOCXTOR_PERF_SIZES`, default 100,1000,5000) and for each of open, a keystroke (an
//! `insert_text` that merges into the last undo step, then `render_changed`, as the editor does), a paragraph split, an
//! undo and a save, it reports the allocations made, the bytes requested, the most bytes live at once above where the
//! step began, the whole-document body copies, and the paragraphs the document walks visited (`walked`: typing should
//! not make it grow with the length of the document). A keystroke is the mean over 50.
//!
//!   DOCXTOR_PERF=1 cargo test --release -p docxtor-core --test perf_counts -- --nocapture --test-threads=1
//!
//! With `--features perf-plant`, `DOCXTOR_PLANT_BUSY`, `DOCXTOR_PLANT_ALLOC_MB` and `DOCXTOR_PLANT_DOUBLE_CLONE` plant
//! regressions the harness must catch.

#[path = "common/proc.rs"]
mod proc;

use docxtor_core::{perf, Document};
use serde_json::{json, Value};

#[global_allocator]
static ALLOC: proc::Counting = proc::Counting;

fn markdown(n: usize) -> String {
    (0..n)
        .map(|i| {
            if i % 50 == 0 {
                format!("## Heading {i}")
            } else if i % 7 == 0 {
                format!("- item **{i}** of the list")
            } else {
                format!("Paragraph {i} with *some* words to lay out and a little more text.")
            }
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn op(v: Value) -> docxtor_core::ops::Op {
    serde_json::from_value(v).expect("an op")
}

/// What one step cost: allocations, bytes requested, peak bytes above the start, body copies, paragraphs walked.
fn measure<T>(per: usize, f: impl FnOnce() -> T) -> (Value, T) {
    let clones = perf::body_clones();
    let walked = perf::paragraphs_walked();
    proc::reset_totals();
    let out = f();
    let (allocs, total, peak) = proc::totals();
    let c = perf::body_clones() - clones;
    let w = perf::paragraphs_walked() - walked;
    (json!({ "allocs": allocs / per, "bytes": total / per, "peak": peak, "clones": c as f64 / per as f64, "walked": w as f64 / per as f64 }), out)
}

#[test]
fn perf_counts() {
    if std::env::var("DOCXTOR_PERF").as_deref() != Ok("1") {
        return;
    }
    let sizes: Vec<usize> = std::env::var("DOCXTOR_PERF_SIZES").unwrap_or_else(|_| "100,1000,5000".into()).split(',').filter_map(|s| s.trim().parse().ok()).collect();
    for n in sizes {
        // Build the document the same way as packages/core/test/bench-typing.mts and perf/lib/docs.mts.
        let mut d = Document::new_blank();
        let first = d.render_all().order[0];
        d.apply(&op(json!({"op": "insert_markdown", "before": first, "markdown": markdown(n)})), false).unwrap();
        let bytes = d.save().unwrap();
        drop(d);

        let (open, mut doc) = measure(1, || {
            let mut doc = Document::open(&bytes).unwrap();
            doc.render_all();
            doc
        });
        let order = doc.render_all().order;
        let mid = order[order.len() / 2];
        // The first keystroke starts a new undo step; the rest merge into it, as typing does.
        doc.apply(&op(json!({"op": "insert_text", "at": {"p": mid, "o": 0}, "text": "x"})), false).unwrap();
        doc.render_changed();
        let keys = 50;
        let (key, _) = measure(keys, || {
            for i in 1..=keys {
                doc.apply(&op(json!({"op": "insert_text", "at": {"p": mid, "o": i}, "text": "x"})), true).unwrap();
                doc.render_changed();
            }
        });
        let (split, _) = measure(1, || {
            doc.apply(&op(json!({"op": "split_paragraph", "at": {"p": mid, "o": 10}})), false).unwrap();
            doc.render_changed();
        });
        let (undo, _) = measure(1, || {
            doc.undo();
            doc.render_changed();
        });
        let (save, _) = measure(1, || doc.save().unwrap());
        println!("@@perf {}", json!({ "paragraphs": n, "docx_bytes": bytes.len(), "open": open, "key": key, "split": split, "undo": undo, "save": save }));
    }
}

/// Where opening one file's memory goes: `DOCXTOR_PERF_FILE=<file.docx> cargo test --release -p docxtor-core --test
/// perf_counts open_one_file -- --nocapture`. Prints, in MB, the peak above the start and what stays, for parsing the
/// main part's XML alone and for opening the document.
#[test]
fn open_one_file() {
    let Ok(path) = std::env::var("DOCXTOR_PERF_FILE") else { return };
    let bytes = std::fs::read(&path).unwrap();
    let mb = |b: usize| b as f64 / 1_048_576.0;
    let pkg = docxtor_core::package::Package::open(&bytes).unwrap();
    let main = pkg.main_document_path();
    let xml = pkg.get(&main).unwrap().to_vec();
    drop(pkg);
    let base = proc::live_bytes();
    proc::reset_totals();
    let doc = docxtor_core::xml::parse(&xml).unwrap();
    let (allocs, _, peak) = proc::totals();
    println!("parse {main} ({:.1} MB of XML): peak {:.1} MB, kept {:.1} MB, {allocs} allocations", mb(xml.len()), mb(peak), mb(proc::live_bytes() - base));
    drop(doc);
    let base = proc::live_bytes();
    proc::reset_totals();
    let d = Document::open(&bytes).unwrap();
    let (allocs, _, peak) = proc::totals();
    println!("open: peak {:.1} MB, kept {:.1} MB, {allocs} allocations", mb(peak), mb(proc::live_bytes() - base));
    // What the steps after opening hold (the random-step harness does each of these).
    let mut d = d;
    let stage = |name: &str, f: &mut dyn FnMut()| {
        let base = proc::live_bytes();
        proc::reset_totals();
        f();
        let (_, _, peak) = proc::totals();
        println!("{name}: peak {:.1} MB, kept {:.1} MB", mb(peak), mb(proc::live_bytes().saturating_sub(base)));
    };
    let mut model = serde_json::Value::Null;
    stage("render_all", &mut || {
        d.render_all();
    });
    stage("render model as JSON", &mut || model = serde_json::to_value(d.render_snapshot()).unwrap());
    drop(model);
    let first = d.render_all().order[0];
    let edit = |d: &mut Document, v: Value| d.apply(&op(v), false).unwrap();
    stage("first edit", &mut || {
        edit(&mut d, json!({"op":"insert_text","at":{"p":first,"o":0},"text":"x"}));
    });
    stage("second edit", &mut || {
        edit(&mut d, json!({"op":"insert_text","at":{"p":first,"o":0},"text":"y"}));
    });
    for (name, v) in [
        ("replace_font", json!({"op":"replace_font","from":"Times New Roman","to":"Arial"})),
        ("set_page_background", json!({"op":"set_page_background","fill":{"kind":"solid","color":"fff2cc"}})),
        ("replace_font again", json!({"op":"replace_font","from":"Arial","to":"Georgia"})),
    ] {
        stage(name, &mut || {
            let _ = d.apply(&op(v.clone()), false);
        });
    }
    stage("undo", &mut || {
        d.undo();
    });
    let mut saved = Vec::new();
    stage("save", &mut || saved = d.save().unwrap());
    stage("open the save", &mut || drop(Document::open(&saved).unwrap()));
    drop(d);
}

/// Where memory goes for each paragraph (very long documents use a lot of memory). Does nothing unless
/// `DOCXTOR_MEMORY=1`; sizes in `DOCXTOR_MEMORY_SIZES` (default 1000,10000,50000). Prints live bytes per paragraph that each
/// stage leaves behind: open, the first full render (the model the browser gets), one edit with its undo step, a save.
///
///   DOCXTOR_MEMORY=1 cargo test --release -p docxtor-core --test perf_counts memory_per_paragraph -- --nocapture
#[test]
fn memory_per_paragraph() {
    if std::env::var("DOCXTOR_MEMORY").as_deref() != Ok("1") {
        return;
    }
    let sizes: Vec<usize> = std::env::var("DOCXTOR_MEMORY_SIZES").unwrap_or_else(|_| "1000,10000,50000".into()).split(',').filter_map(|s| s.trim().parse().ok()).collect();
    {
        use docxtor_core::{model, props, render};
        use std::mem::size_of;
        use docxtor_core::{charfmt, east_asian};
        println!(
            "@@sizes2 RunExt={} TextFx={} Border={} ThemeRef={} RunEa={} EaLayout={} RImage={} RShape={} MathInfo={} ChartModel={} ChartExModel={} RRuby={} RHrule={} ListLabel={}",
            size_of::<charfmt::RunExt>(), size_of::<charfmt::TextFx>(), size_of::<props::Border>(), size_of::<charfmt::ThemeRef>(), size_of::<east_asian::RunEa>(), size_of::<east_asian::EaLayout>(),
            size_of::<render::RImage>(), size_of::<docxtor_core::shapes::RShape>(), size_of::<docxtor_core::math::MathInfo>(), size_of::<docxtor_core::chart::ChartModel>(), size_of::<docxtor_core::chart::chartex::ChartExModel>(), size_of::<docxtor_core::ruby::RRuby>(), size_of::<render::RHrule>(), size_of::<docxtor_core::numbering::ListLabel>()
        );
        println!(
            "@@sizes RBlock={} RPara={} RSeg={} RunFmt={} ParaFmt={} RTable={} | model::Block={} Paragraph={} Run={} Inline={} RunItem={} Element={}",
            size_of::<render::RBlock>(), size_of::<render::RPara>(), size_of::<render::RSeg>(), size_of::<props::RunFmt>(), size_of::<props::ParaFmt>(), size_of::<render::RTable>(),
            size_of::<model::Block>(), size_of::<model::Paragraph>(), size_of::<model::Run>(), size_of::<model::Inline>(), size_of::<model::RunItem>(), size_of::<docxtor_core::xml::Element>()
        );
    }
    let word_like = std::env::var("DOCXTOR_MEMORY_WORD").as_deref() == Ok("1");
    for n in sizes {
        let text = (0..n).map(|i| format!("Paragraph {i}: the quick brown fox jumps over the lazy dog, again and again, all afternoon.")).collect::<Vec<_>>().join("\n\n");
        let mut d = Document::new_blank();
        let first = d.render_all().order[0];
        d.apply(&op(json!({"op": "insert_markdown", "before": first, "markdown": text})), false).unwrap();
        let mut bytes = d.save().unwrap();
        drop(d);
        if word_like {
            // Paragraphs as Word writes them: revision-save ids on the paragraph and on every run, paragraph properties, two runs with fonts, size and language.
            let mut pkg = docxtor_core::package::Package::open(&bytes).unwrap();
            let main = pkg.main_document_path();
            let body: String = (0..n)
                .map(|i| format!("<w:p w:rsidR=\"00A{:05X}\" w:rsidRDefault=\"00B{:05X}\" w:rsidP=\"00C{:05X}\"><w:pPr><w:spacing w:after=\"120\" w:line=\"276\" w:lineRule=\"auto\"/><w:jc w:val=\"both\"/></w:pPr><w:r w:rsidRPr=\"00D{:05X}\"><w:rPr><w:rFonts w:ascii=\"Calibri\" w:hAnsi=\"Calibri\" w:cs=\"Calibri\"/><w:sz w:val=\"22\"/><w:szCs w:val=\"22\"/><w:lang w:val=\"en-US\"/></w:rPr><w:t xml:space=\"preserve\">Paragraph {i}: the quick brown fox jumps over the lazy dog, </w:t></w:r><w:r w:rsidR=\"00E{:05X}\"><w:rPr><w:rFonts w:ascii=\"Calibri\" w:hAnsi=\"Calibri\" w:cs=\"Calibri\"/><w:b/><w:sz w:val=\"22\"/><w:szCs w:val=\"22\"/><w:lang w:val=\"en-US\"/></w:rPr><w:t>again and again, all afternoon.</w:t></w:r></w:p>", i, i, i, i, i))
                .collect();
            let xml = format!("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\r\n<w:document xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><w:body>{body}<w:sectPr><w:pgSz w:w=\"12240\" w:h=\"15840\"/><w:pgMar w:top=\"1440\" w:right=\"1440\" w:bottom=\"1440\" w:left=\"1440\" w:header=\"720\" w:footer=\"720\" w:gutter=\"0\"/></w:sectPr></w:body></w:document>");
            pkg.set(&main, xml.into_bytes());
            bytes = pkg.to_bytes().unwrap();
        }
        let per = |b: usize| b as f64 / n as f64;
        let live = proc::live_bytes;
        let mut line = format!("@@memory n={n} docx={:.0}", per(bytes.len()));

        let pkg_base = live();
        let (pkg, _) = docxtor_core::open::open_package(&bytes, &docxtor_core::budget::Budget::file()).unwrap();
        let pkg_kept = live() - pkg_base;
        let main = pkg.main_document_path();
        let xml_len = pkg.get(&main).unwrap().len();
        let xml_base = live();
        let tree = docxtor_core::xml::parse(pkg.get(&main).unwrap()).unwrap();
        let xml_kept = live() - xml_base;
        drop(tree);
        drop(pkg);
        line += &format!(" | package={:.0} (document.xml {:.0}) xml_tree={:.0}", per(pkg_kept), per(xml_len), per(xml_kept));

        let base = live();
        proc::reset_totals();
        let mut doc = Document::open(&bytes).unwrap();
        let (_, _, open_peak) = proc::totals();
        line += &format!(" | open kept={:.0} peak={:.0}", per(live() - base), per(open_peak));
        let base = live();
        proc::reset_totals();
        let out = doc.render_all();
        let (_, _, render_peak) = proc::totals();
        let with_out = live() - base;
        let order = out.order.clone();
        let json = serde_json::to_string(&out).unwrap();
        line += &format!(" | render_all: out={:.0} json={:.0} peak={:.0}", per(with_out), per(json.len()), per(render_peak));
        drop(json);
        drop(out);
        line += &format!(" kept after={:.0}", per(live().saturating_sub(base)));
        let base = live();
        let mid = order[order.len() / 2];
        doc.apply(&op(json!({"op": "insert_text", "at": {"p": mid, "o": 0}, "text": "x"})), false).unwrap();
        let after_edit = live().saturating_sub(base);
        let c = doc.render_changed();
        drop(c);
        line += &format!(" | edit1 kept={:.0} (undo_weight {:.0}) +render_changed={:.0}", per(after_edit), per(doc.undo_weight().0), per(live().saturating_sub(base)));
        // A second edit in another paragraph, as its own undo step: what one more step costs once the first has paid for any cache.
        let base2 = live();
        let other = order[order.len() / 3];
        doc.apply(&op(json!({"op": "insert_text", "at": {"p": other, "o": 0}, "text": "y"})), false).unwrap();
        doc.render_changed();
        line += &format!(" edit2 kept={} B (undo_weight {} B)", live().saturating_sub(base2), doc.undo_weight().0);
        let base = live();
        proc::reset_totals();
        let saved = doc.save().unwrap();
        let (_, _, save_peak) = proc::totals();
        line += &format!(" | save kept={:.0} peak={:.0}", per(live().saturating_sub(base)), per(save_peak));
        drop(saved);
        drop(doc);
        println!("{line}");
    }
}
