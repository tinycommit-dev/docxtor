//! The Word 97 to 2003 reader on a file made here byte by byte from the format's description: a heading, formatted
//! runs, a list, a table, a hyperlink field, a footnote and a picture, so each part of the reader is exercised.

use docxtor_core::model::walk_paragraphs;
use docxtor_core::open::{self, Format};
use docxtor_core::package::Package;
use docxtor_core::Document;

fn u16le(v: &mut Vec<u8>, x: usize) {
    v.extend_from_slice(&(x as u16).to_le_bytes());
}

fn u32le(v: &mut Vec<u8>, x: usize) {
    v.extend_from_slice(&(x as u32).to_le_bytes());
}

fn sprm(op: u16, operand: &[u8]) -> Vec<u8> {
    let mut v = op.to_le_bytes().to_vec();
    v.extend_from_slice(operand);
    v
}

/// A character formatting page: runs of (start, end, sprms) that cover the text one after the other.
fn chpx_fkp(runs: &[(usize, usize, Vec<u8>)]) -> Vec<u8> {
    let mut page = vec![0u8; 512];
    let n = runs.len();
    for (i, r) in runs.iter().enumerate() {
        page[i * 4..i * 4 + 4].copy_from_slice(&(r.0 as u32).to_le_bytes());
    }
    page[n * 4..n * 4 + 4].copy_from_slice(&(runs[n - 1].1 as u32).to_le_bytes());
    let mut top = 510usize;
    for (i, r) in runs.iter().enumerate() {
        if r.2.is_empty() {
            continue;
        }
        let len = 1 + r.2.len();
        top = (top - len) & !1;
        page[top] = r.2.len() as u8;
        page[top + 1..top + 1 + r.2.len()].copy_from_slice(&r.2);
        page[(n + 1) * 4 + i] = (top / 2) as u8;
    }
    page[511] = n as u8;
    page
}

/// A paragraph formatting page: (start, end, istd, sprms) for each paragraph.
fn papx_fkp(runs: &[(usize, usize, usize, Vec<u8>)]) -> Vec<u8> {
    let mut page = vec![0u8; 512];
    let n = runs.len();
    for (i, r) in runs.iter().enumerate() {
        page[i * 4..i * 4 + 4].copy_from_slice(&(r.0 as u32).to_le_bytes());
    }
    page[n * 4..n * 4 + 4].copy_from_slice(&(runs[n - 1].1 as u32).to_le_bytes());
    let mut top = 510usize;
    for (i, r) in runs.iter().enumerate() {
        let mut data = (r.2 as u16).to_le_bytes().to_vec();
        data.extend_from_slice(&r.3);
        let len = if data.len() % 2 == 1 { 1 + data.len() } else { 2 + data.len() };
        top = (top - len) & !1;
        if data.len() % 2 == 1 {
            page[top] = data.len().div_ceil(2) as u8;
            page[top + 1..top + 1 + data.len()].copy_from_slice(&data);
        } else {
            page[top] = 0;
            page[top + 1] = (data.len() / 2) as u8;
            page[top + 2..top + 2 + data.len()].copy_from_slice(&data);
        }
        page[(n + 1) * 4 + i * 13] = (top / 2) as u8;
    }
    page[511] = n as u8;
    page
}

fn std_def(name: &str, sti: usize, base: usize, papx: &[u8], chpx: &[u8]) -> Vec<u8> {
    let mut s = Vec::new();
    u16le(&mut s, sti);
    u16le(&mut s, 1 | (base << 4)); // stk 1: a paragraph style
    u16le(&mut s, 2); // two property blocks
    u16le(&mut s, 0);
    u16le(&mut s, 0);
    let units: Vec<u16> = name.encode_utf16().collect();
    u16le(&mut s, units.len());
    for u in units {
        u16le(&mut s, u as usize);
    }
    u16le(&mut s, 0);
    if s.len() % 2 == 1 {
        s.push(0);
    }
    let mut upx_pap = vec![0, 0];
    upx_pap.extend_from_slice(papx);
    for upx in [upx_pap, chpx.to_vec()] {
        u16le(&mut s, upx.len());
        s.extend_from_slice(&upx);
        if upx.len() % 2 == 1 {
            s.push(0);
        }
    }
    s
}

const PNG: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==";

/// The .doc file. Returns its bytes.
fn build() -> Vec<u8> {
    let main = "Chapter one\rPlain bold italic text\x02 and more\rFirst item\rSecond item\rCell A\x07Cell B\x07\x07See \x13 HYPERLINK \"https://example.com/page\" \x14the site\x15 now\r\x01\rLast words\r";
    let ftn = "\x02 The footnote text.\r";
    let text = format!("{main}{ftn}");
    let text_fc = 0x800usize;
    let fc = |cp: usize| text_fc + cp;

    // Where things are in the text.
    let at = |s: &str| main.find(s).unwrap();
    let par_end = |from: usize| from + main[from..].find(['\r', '\x07']).unwrap() + 1;

    // Paragraph formatting for each paragraph of the main text.
    let mut paps: Vec<(usize, usize, usize, Vec<u8>)> = Vec::new();
    let mut cp = 0;
    while cp < main.len() {
        let end = par_end(cp);
        let para = &main[cp..end];
        let (istd, g) = if para.starts_with("Chapter") {
            (1, vec![])
        } else if para.starts_with("First item") || para.starts_with("Second item") {
            (0, [sprm(0x460B, &[1, 0]), sprm(0x260A, &[0])].concat())
        } else if para.starts_with("Cell A") || para.starts_with("Cell B") {
            (0, sprm(0x2416, &[1]))
        } else if para == "\x07" {
            // The end of the row: its cells, 1 inch wide each, with a single border.
            let mut tdef = vec![2u8];
            for c in [0i16, 1440, 2880] {
                tdef.extend_from_slice(&c.to_le_bytes());
            }
            for _ in 0..2 {
                tdef.extend_from_slice(&[0, 0, 0, 0]);
                for _ in 0..4 {
                    tdef.extend_from_slice(&[4, 1, 1, 0]);
                }
            }
            let mut op = 0xD608u16.to_le_bytes().to_vec();
            op.extend_from_slice(&((tdef.len() + 1) as u16).to_le_bytes());
            op.extend_from_slice(&tdef);
            (0, [sprm(0x2416, &[1]), sprm(0x2417, &[1]), op].concat())
        } else if para.starts_with("Last words") {
            (0, sprm(0x2403, &[1])) // centred
        } else {
            (0, vec![])
        };
        paps.push((fc(cp), fc(end), istd, g));
        cp = end;
    }
    // The footnote's paragraph, after the main text.
    paps.push((fc(main.len()), fc(text.len()), 0, vec![]));

    // Character formatting: plain, with bold and italic words and the picture mark.
    let bold = at("bold");
    let italic = at("italic");
    let pic = at("\x01");
    let mut cuts = vec![(bold, bold + 4, sprm(0x0835, &[1])), (italic, italic + 6, sprm(0x0836, &[1])), (pic, pic + 1, [sprm(0x0855, &[1]), sprm(0x6A03, &0u32.to_le_bytes())].concat())];
    cuts.sort_by_key(|c| c.0);
    let mut chps: Vec<(usize, usize, Vec<u8>)> = Vec::new();
    let mut pos = 0;
    for (a, b, g) in cuts {
        if a > pos {
            chps.push((fc(pos), fc(a), vec![]));
        }
        chps.push((fc(a), fc(b), g));
        pos = b;
    }
    chps.push((fc(pos), fc(text.len()), vec![]));

    // The table stream.
    let mut t: Vec<u8> = Vec::new();
    // The piece table: one piece of 8-bit text.
    let clx_at = t.len();
    t.push(2);
    let mut plc = Vec::new();
    u32le(&mut plc, 0);
    u32le(&mut plc, text.len());
    u16le(&mut plc, 0);
    u32le(&mut plc, (text_fc * 2) | 0x4000_0000);
    u16le(&mut plc, 0);
    u32le(&mut t, plc.len());
    t.extend_from_slice(&plc);
    let clx_len = t.len() - clx_at;

    let mut bte = |pn: usize, first: usize, last: usize| {
        let at = t.len();
        u32le(&mut t, first);
        u32le(&mut t, last);
        u32le(&mut t, pn);
        (at, 12)
    };
    let (chpx_at, chpx_len) = bte(8, fc(0), fc(text.len()));
    let (papx_at, papx_len) = bte(9, fc(0), fc(text.len()));

    // Fonts: 0 Times New Roman, 1 Symbol.
    let ffn_at = t.len();
    u16le(&mut t, 2);
    u16le(&mut t, 0);
    for name in ["Times New Roman", "Symbol"] {
        let units: Vec<u16> = name.encode_utf16().collect();
        let size = 39 + (units.len() + 1) * 2;
        t.push(size as u8);
        t.extend_from_slice(&[0u8; 39]);
        for u in units {
            u16le(&mut t, u as usize);
        }
        u16le(&mut t, 0);
    }
    let ffn_len = t.len() - ffn_at;

    // The style sheet: Normal (10 pt) and heading 1.
    let stsh_at = t.len();
    let mut stsh = Vec::new();
    u16le(&mut stsh, 18);
    for v in [2usize, 10, 0, 0, 0, 0, 0, 0, 0] {
        u16le(&mut stsh, v);
    }
    for def in [std_def("Normal", 0, 0x0FFF, &[], &sprm(0x4A43, &[20, 0])), std_def("heading 1", 1, 0, &[], &[])] {
        u16le(&mut stsh, def.len());
        stsh.extend_from_slice(&def);
        if def.len() % 2 == 1 {
            stsh.push(0);
        }
    }
    t.extend_from_slice(&stsh);
    let stsh_len = stsh.len();

    // Footnotes: one reference, its text.
    let fr_at = t.len();
    u32le(&mut t, at("\x02"));
    u32le(&mut t, main.len());
    u16le(&mut t, 0);
    let fr_len = t.len() - fr_at;
    let ft_at = t.len();
    u32le(&mut t, 0);
    u32le(&mut t, ftn.len());
    u32le(&mut t, ftn.len() + 1);
    let ft_len = t.len() - ft_at;

    // One simple bulleted list (a Symbol bullet), and the list-to-number table that names it.
    let lst_at = t.len();
    u16le(&mut t, 1);
    u32le(&mut t, 777); // lsid
    u32le(&mut t, 0);
    t.extend_from_slice(&[0u8; 18]);
    t.push(1); // a simple list
    t.push(0);
    let lst_len = t.len() - lst_at;
    // Its only level.
    let mut lvl = Vec::new();
    u32le(&mut lvl, 1);
    lvl.push(23); // a bullet
    lvl.push(0);
    lvl.extend_from_slice(&[0u8; 9]);
    lvl.push(0);
    u32le(&mut lvl, 0);
    u32le(&mut lvl, 0);
    let chpx_bytes = sprm(0x4A4F, &[1, 0]); // font 1: Symbol
    let papx_bytes = [sprm(0x840F, &720i16.to_le_bytes()), sprm(0x8411, &(-360i16).to_le_bytes())].concat();
    lvl.push(chpx_bytes.len() as u8);
    lvl.push(papx_bytes.len() as u8);
    lvl.extend_from_slice(&[0, 0]);
    lvl.extend_from_slice(&papx_bytes);
    lvl.extend_from_slice(&chpx_bytes);
    u16le(&mut lvl, 1);
    u16le(&mut lvl, 0xF0B7);
    t.extend_from_slice(&lvl);
    let lfo_at = t.len();
    u32le(&mut t, 1);
    u32le(&mut t, 777);
    t.extend_from_slice(&[0u8; 12]);
    let lfo_len = t.len() - lfo_at;

    // The main stream: the header, the text, and the two formatting pages.
    let mut w = vec![0u8; 0x1400];
    w[0..2].copy_from_slice(&0xA5ECu16.to_le_bytes());
    w[2..4].copy_from_slice(&0xC1u16.to_le_bytes());
    w[10..12].copy_from_slice(&0x0200u16.to_le_bytes()); // fWhichTblStm: 1Table
    let mut p = 32;
    w[p..p + 2].copy_from_slice(&14u16.to_le_bytes());
    p += 2 + 28;
    w[p..p + 2].copy_from_slice(&22u16.to_le_bytes());
    p += 2;
    let set = |w: &mut Vec<u8>, off: usize, v: usize| w[off..off + 4].copy_from_slice(&(v as u32).to_le_bytes());
    set(&mut w, p + 12, main.len()); // ccpText
    set(&mut w, p + 16, ftn.len()); // ccpFtn
    p += 22 * 4;
    w[p..p + 2].copy_from_slice(&93u16.to_le_bytes());
    p += 2;
    for (idx, fc_, lcb) in [(1, stsh_at, stsh_len), (2, fr_at, fr_len), (3, ft_at, ft_len), (12, chpx_at, chpx_len), (13, papx_at, papx_len), (15, ffn_at, ffn_len), (33, clx_at, clx_len), (73, lst_at, lst_len), (74, lfo_at, lfo_len)] {
        set(&mut w, p + idx * 8, fc_);
        set(&mut w, p + idx * 8 + 4, lcb);
    }
    let bytes: Vec<u8> = text.bytes().collect();
    w[text_fc..text_fc + bytes.len()].copy_from_slice(&bytes);
    w[8 * 512..9 * 512].copy_from_slice(&chpx_fkp(&chps));
    w[9 * 512..10 * 512].copy_from_slice(&papx_fkp(&paps));

    // The picture, in the data stream: a header, then a PNG record.
    let png = docxtor_core::fragment::b64_decode(PNG).unwrap();
    let mut rec = Vec::new();
    u16le(&mut rec, 0x6E0 << 4);
    u16le(&mut rec, 0xF01E);
    u32le(&mut rec, 16 + 1 + png.len());
    rec.extend_from_slice(&[7u8; 16]);
    rec.push(0xFF);
    rec.extend_from_slice(&png);
    let mut d = vec![0u8; 0x44];
    d[4..6].copy_from_slice(&0x44u16.to_le_bytes());
    d[6..8].copy_from_slice(&0x64u16.to_le_bytes());
    d[28..30].copy_from_slice(&1440u16.to_le_bytes()); // one inch wide
    d[30..32].copy_from_slice(&720u16.to_le_bytes()); // half an inch high
    d[32..34].copy_from_slice(&1000u16.to_le_bytes());
    d[34..36].copy_from_slice(&1000u16.to_le_bytes());
    let total = d.len() + rec.len();
    d[0..4].copy_from_slice(&(total as u32).to_le_bytes());
    d.extend_from_slice(&rec);

    docxtor_core::cfb::write(&[("WordDocument", w), ("1Table", t), ("Data", d)])
}

fn texts(doc: &Document) -> Vec<String> {
    let mut v = Vec::new();
    walk_paragraphs(&doc.body, &mut |p| v.push(p.plain_text()));
    v
}

#[test]
fn a_word_97_file_made_by_hand_opens_with_all_its_parts() {
    let bytes = build();
    let o = open::open(&bytes, "made.doc").unwrap();
    assert_eq!(o.report.format, Format::Doc);
    let mut doc = o.doc;
    let t = texts(&doc);
    assert_eq!(t[0], "Chapter one", "{t:?}");
    assert!(t.iter().any(|x| x == "Plain bold italic text and more"), "{t:?}");
    for want in ["First item", "Second item", "Cell A", "Cell B", "Last words"] {
        assert!(t.iter().any(|x| x == want), "{want} in {t:?}");
    }
    assert!(t.iter().any(|x| x == "See the site now"), "{t:?}");
    let saved = doc.save().unwrap();
    let pkg = Package::open(&saved).unwrap();
    let body = String::from_utf8(pkg.get("word/document.xml").unwrap().to_vec()).unwrap();
    // The heading is Word's heading style.
    assert!(body.contains(r#"<w:pStyle w:val="Heading1"/>"#), "{body}");
    // Bold and italic words, centred text.
    assert!(body.contains("<w:b/>") && body.contains("<w:i/>") && body.contains(r#"<w:jc w:val="center"/>"#), "{body}");
    // The list is a list with a bullet.
    assert!(body.contains("<w:numPr>"), "{body}");
    let numbering = String::from_utf8(pkg.get("word/numbering.xml").unwrap().to_vec()).unwrap();
    assert!(numbering.contains(r#"w:val="bullet""#), "{numbering}");
    // The table has two cells with borders, and the hyperlink and the picture are there.
    assert!(body.contains("<w:tbl>") && body.contains("<w:tcBorders>") && body.matches("<w:tc>").count() == 2, "{body}");
    assert!(body.contains("<w:hyperlink") && body.contains("<w:drawing>"), "{body}");
    assert!(pkg.parts.iter().any(|p| p.name.starts_with("word/media/")));
    // The footnote.
    assert!(body.contains("<w:footnoteReference"), "{body}");
    let notes = String::from_utf8(pkg.get("word/footnotes.xml").unwrap().to_vec()).unwrap();
    assert!(notes.contains("The footnote text."), "{notes}");
    // The Normal style's 10 points are the document's size.
    let styles = String::from_utf8(pkg.get("word/styles.xml").unwrap().to_vec()).unwrap();
    assert!(styles.contains(r#"<w:sz w:val="20"/>"#), "{styles}");
}

#[test]
fn deleted_text_of_a_tracked_change_is_left_out() {
    // Mark "bold" as deleted (sprmCFRMarkDel) by rewriting the formatting page of a file built the same way.
    let bytes = build();
    let cfb = docxtor_core::cfb::CfbReader::new(&bytes).unwrap();
    let mut w = cfb.stream("WordDocument").unwrap().unwrap();
    let t = cfb.stream("1Table").unwrap().unwrap();
    let d = cfb.stream("Data").unwrap().unwrap();
    // Bold is sprm 0x0835 with operand 1: turn it into "deleted" (0x0800).
    let page = &mut w[8 * 512..9 * 512];
    let at = page.windows(3).position(|x| x == [0x35, 0x08, 1]).unwrap();
    page[at] = 0x00;
    page[at + 1] = 0x08;
    let out = docxtor_core::cfb::write(&[("WordDocument", w), ("1Table", t), ("Data", d)]);
    let o = open::open(&out, "tracked.doc").unwrap();
    assert!(texts(&o.doc).iter().any(|x| x == "Plain  italic text and more"), "{:?}", texts(&o.doc));
}

/// Cut, flip and overwrite bytes of each sample many times: opening may fail, but it must not panic or hang.
#[test]
fn damaged_input_never_panics() {
    let fixture = |n: &str| std::fs::read(format!("{}/../../fixtures/{n}", env!("CARGO_MANIFEST_DIR"))).unwrap();
    let samples: Vec<(&str, Vec<u8>)> = vec![
        ("made.doc", build()),
        ("sample.doc", fixture("formats/sample.doc")),
        ("sample.odt", fixture("formats/sample.odt")),
        ("sample.rtf", fixture("formats/sample.rtf")),
        ("sample.html", fixture("formats/sample.html")),
        ("sample.xml", fixture("formats/sample-word2003.xml")),
        ("rich.docx", fixture("rich.docx")),
        ("long.docx", fixture("long.docx")),
    ];
    let mut seed = 0x2545_F491u32;
    let mut next = move || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        seed
    };
    for (name, bytes) in &samples {
        for round in 0..if *name == "made.doc" || *name == "sample.doc" { 100 } else { 20 } {
            let mut b = bytes.clone();
            match round % 4 {
                0 => b.truncate(next() as usize % bytes.len().max(1)),
                1 => {
                    for _ in 0..1 + next() % 20 {
                        let i = next() as usize % b.len().max(1);
                        if let Some(x) = b.get_mut(i) {
                            *x ^= (next() & 0xFF) as u8;
                        }
                    }
                }
                2 => {
                    let i = next() as usize % b.len().max(1);
                    let n = (next() % 600) as usize;
                    for x in b.iter_mut().skip(i).take(n) {
                        *x = (next() & 0xFF) as u8;
                    }
                }
                _ => {
                    let i = next() as usize % b.len().max(1);
                    b.drain(i..(i + (next() % 3000) as usize).min(b.len()));
                }
            }
            let _ = open::open(&b, name);
        }
    }
    // Damage inside the streams of the hand-made .doc (the file's own structure stays sound, so the reader
    // gets as far as the text, the formatting pages, the lists and the pictures).
    let bytes = build();
    let cfb = docxtor_core::cfb::CfbReader::new(&bytes).unwrap();
    let streams: Vec<(&str, Vec<u8>)> = ["WordDocument", "1Table", "Data"].iter().map(|n| (*n, cfb.stream(n).unwrap().unwrap())).collect();
    for round in 0..120 {
        let mut s = streams.clone();
        let which = next() as usize % s.len();
        for _ in 0..1 + next() % 8 {
            let len = s[which].1.len();
            let i = next() as usize % len;
            s[which].1[i] = match next() % 3 {
                0 => 0,
                1 => 0xFF,
                _ => (next() & 0xFF) as u8,
            };
        }
        let _ = round;
        let _ = open::open(&docxtor_core::cfb::write(&s), "streams.doc");
    }
}

#[test]
fn a_cut_down_file_is_an_error_not_a_crash() {
    let bytes = build();
    for cut in [100usize, 600, 2000, bytes.len() / 2] {
        let _ = open::open(&bytes[..cut.min(bytes.len())], "cut.doc");
    }
    // Garbage in the formatting pages: no panic, and the text is still there.
    let cfb = docxtor_core::cfb::CfbReader::new(&bytes).unwrap();
    let mut w = cfb.stream("WordDocument").unwrap().unwrap();
    for b in w[8 * 512..10 * 512].iter_mut() {
        *b = 0xFF;
    }
    let t = cfb.stream("1Table").unwrap().unwrap();
    let out = docxtor_core::cfb::write(&[("WordDocument", w), ("1Table", t)]);
    let o = open::open(&out, "garbled.doc").unwrap();
    assert!(texts(&o.doc).iter().any(|x| x.contains("Chapter one")));
}
