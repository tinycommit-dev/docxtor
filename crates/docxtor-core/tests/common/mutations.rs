//! The inputs and the mutations of the damaged-file harness (`tests/mutate_open.rs`).
//!
//! A case is a small JSON description: which entry point (`target`), which starting file (`fixture`), which part of
//! it to change (`part`) and how (`mutation`), and a `seed` for the random choices inside the mutation. The same
//! description always builds the same bytes, so a failure can be pasted back and replayed. Nothing here calls the
//! engine except to read the starting files (`Package`, `Document::new_blank`, the chart workbook builder).

use crate::common::{build, repo_path, Rng};
use docxtor_core::chart::model::ChartSeries;
use docxtor_core::chart::xlsx;
use docxtor_core::package::{Package, Part};
use serde_json::{json, Value};

pub const PW: &str = "docxtor-test-pw";

// ---------------------------------------------------------------------------
// Starting files
// ---------------------------------------------------------------------------

/// The bytes of a starting file. A name is a file of `fixtures/`, or one built here: `built:rich`, `built:plain`,
/// `flat:rich.docx` (that fixture as Flat OPC), `xlsx:base`, `emf:base`, `wmf:base`, `tiff:base`, `csv:base`,
/// `bib:base`, `csljson:base`, `csl:<file of web/src/csl>`, `text:empty`.
pub fn fixture(name: &str) -> Vec<u8> {
    match name {
        "built:rich" => build("rich").save().unwrap(),
        "built:plain" => build("plain").save().unwrap(),
        "xlsx:base" => base_xlsx(),
        "emf:base" => base_emf(),
        "wmf:base" => base_wmf(),
        "tiff:base" => base_tiff(),
        "csv:base" => b"name,email,city,amount\nAda Lovelace,ada@example.com,London,12.50\n\"Grace, Hopper\",grace@example.com,\"New\nYork\",7\nAlan Turing,alan@example.com,Wilmslow,3.25\n".to_vec(),
        "bib:base" => BIB.as_bytes().to_vec(),
        "csljson:base" => CSLJSON.as_bytes().to_vec(),
        "text:empty" => Vec::new(),
        n if n.starts_with("flat:") => flat_opc(&fixture(&n[5..])).into_bytes(),
        n if n.starts_with("csl:") => std::fs::read(repo_path(&format!("web/src/csl/{}", &n[4..]))).unwrap_or_else(|e| panic!("{n}: {e}")),
        n => std::fs::read(repo_path(&format!("fixtures/{n}"))).unwrap_or_else(|e| panic!("fixtures/{n}: {e}")),
    }
}

const BIB: &str = "@article{smith2020,\n  author = {Smith, John and Doe, Jane},\n  title = {A {Study} of \\\"{O}dd Things},\n  journal = {Journal of Things},\n  year = {2020},\n  volume = {12},\n  pages = {1--10}\n}\n@book{knuth1984,\n  author = {Donald E. Knuth},\n  title = {The {TeX}book},\n  publisher = {Addison-Wesley},\n  year = 1984\n}\n@string{jt = \"Journal of Things\"}\n@inproceedings{lamport94, author = \"Leslie Lamport\", title = \"LaTeX\", booktitle = jt # \" Proceedings\", year = {1994}}\n";

const CSLJSON: &str = r#"[{"id":"a1","type":"article-journal","title":"A study of odd things","author":[{"family":"Smith","given":"John"},{"family":"Doe","given":"Jane"}],"container-title":"Journal of Things","issued":{"date-parts":[[2020,3,1]]},"volume":"12","page":"1-10"},{"id":"b2","type":"book","title":"The TeXbook","author":[{"literal":"Knuth, Donald"}],"publisher":"Addison-Wesley","issued":{"raw":"1984"}}]"#;

fn base_xlsx() -> Vec<u8> {
    let cats: Vec<String> = ["North", "South", "East"].iter().map(|s| s.to_string()).collect();
    let series = vec![
        ChartSeries { name: "Sales".into(), values: vec![Some(1.0), Some(2.5), None], ..Default::default() },
        ChartSeries { name: "Costs".into(), values: vec![Some(0.5), Some(1.0), Some(4.0)], ..Default::default() },
    ];
    xlsx::build(&xlsx::plan(&cats, &series, false))
}

/// The series the base workbook was made from, for the chart workbook patcher.
pub fn base_series() -> (Vec<String>, Vec<ChartSeries>, Vec<xlsx::SeriesRefs>) {
    let cats: Vec<String> = ["North", "South", "East"].iter().map(|s| s.to_string()).collect();
    let series = vec![
        ChartSeries { name: "Sales".into(), values: vec![Some(1.0), Some(2.5), None], ..Default::default() },
        ChartSeries { name: "Costs".into(), values: vec![Some(0.5), Some(1.0), Some(4.0)], ..Default::default() },
    ];
    let refs = xlsx::plan(&cats, &series, false).refs;
    (cats, series, refs)
}

fn le32(v: &mut Vec<u8>, n: u32) {
    v.extend(n.to_le_bytes());
}

fn le16(v: &mut Vec<u8>, n: u16) {
    v.extend(n.to_le_bytes());
}

/// An EMF that draws: pens, brushes, a rectangle, an ellipse, lines, a polygon, a transform, save and restore.
fn base_emf() -> Vec<u8> {
    let mut e = Vec::new();
    le32(&mut e, 1);
    le32(&mut e, 88);
    for n in [0i32, 0, 99, 49, 0, 0, 2646, 1323] {
        e.extend(n.to_le_bytes());
    }
    le32(&mut e, 0x464D4520);
    for n in [0x10000u32, 0, 0, 1, 0, 0, 0] {
        le32(&mut e, n);
    }
    for n in [1024i32, 768, 320, 240] {
        e.extend(n.to_le_bytes());
    }
    e.resize(88, 0);
    let rec = |e: &mut Vec<u8>, ty: u32, body: &[u32]| {
        le32(e, ty);
        le32(e, 8 + 4 * body.len() as u32);
        for b in body {
            le32(e, *b);
        }
    };
    rec(&mut e, 38, &[1, 0, 2, 0, 0x0000FF]); // pen
    rec(&mut e, 39, &[2, 0, 0x00FF00, 0]); // brush
    rec(&mut e, 37, &[0x8000_0001]); // select stock object
    rec(&mut e, 37, &[1]);
    rec(&mut e, 37, &[2]);
    rec(&mut e, 33, &[]); // save dc
    rec(&mut e, 43, &[5, 5, 60, 40]); // rectangle
    rec(&mut e, 42, &[10, 10, 90, 45]); // ellipse
    rec(&mut e, 27, &[0, 0]); // move to
    rec(&mut e, 54, &[99, 49]); // line to
    rec(&mut e, 3, &[0, 0, 99, 49, 3, 10, 10, 50, 10, 30, 40]); // polygon
    rec(&mut e, 35, &[0x3F800000, 0, 0, 0x3F800000, 0, 0]); // world transform
    rec(&mut e, 34, &[0xFFFF_FFFF]); // restore dc
    rec(&mut e, 14, &[0, 16, 20]); // end
    let total = e.len() as u32;
    e[48..52].copy_from_slice(&total.to_le_bytes());
    e[52..56].copy_from_slice(&15u32.to_le_bytes());
    e
}

/// A placeable WMF with a pen, a brush, a rectangle, an ellipse, lines and a polygon.
fn base_wmf() -> Vec<u8> {
    let mut w = Vec::new();
    le32(&mut w, 0x9AC6CDD7);
    le16(&mut w, 0);
    for n in [0i16, 0, 100, 50] {
        w.extend(n.to_le_bytes());
    }
    le16(&mut w, 96);
    le32(&mut w, 0);
    le16(&mut w, 0);
    le16(&mut w, 1);
    le16(&mut w, 9);
    le16(&mut w, 0x300);
    le32(&mut w, 0);
    le16(&mut w, 2);
    le32(&mut w, 8);
    le16(&mut w, 0);
    let rec = |w: &mut Vec<u8>, func: u16, params: &[u16]| {
        le32(w, 3 + params.len() as u32);
        le16(w, func);
        for p in params {
            le16(w, *p);
        }
    };
    rec(&mut w, 0x020B, &[0, 0]);
    rec(&mut w, 0x020C, &[50, 100]);
    rec(&mut w, 0x02FA, &[0, 1, 0, 0x00FF, 0]);
    rec(&mut w, 0x02FC, &[0, 0, 0xFF00, 0]);
    rec(&mut w, 0x012D, &[0]);
    rec(&mut w, 0x012D, &[1]);
    rec(&mut w, 0x041B, &[40, 60, 5, 5]);
    rec(&mut w, 0x0418, &[45, 90, 10, 10]);
    rec(&mut w, 0x0214, &[0, 0]);
    rec(&mut w, 0x0213, &[49, 99]);
    rec(&mut w, 0x0324, &[3, 10, 10, 50, 10, 30, 40]);
    rec(&mut w, 0x0000, &[]);
    let words = ((w.len() - 22) / 2) as u32;
    w[28..32].copy_from_slice(&words.to_le_bytes());
    w
}

/// A 2 x 2 uncompressed RGB TIFF. The entries are at 10 + 12 * i: width, height, bits, compression, photometric,
/// strip offset, samples, rows per strip, strip byte count.
fn base_tiff() -> Vec<u8> {
    let mut t = vec![b'I', b'I', 42, 0];
    le32(&mut t, 8);
    let entries: [(u16, u16, u32, u32); 9] = [(256, 3, 1, 2), (257, 3, 1, 2), (258, 3, 3, 122), (259, 3, 1, 1), (262, 3, 1, 2), (273, 4, 1, 128), (277, 3, 1, 3), (278, 3, 1, 2), (279, 4, 1, 12)];
    le16(&mut t, entries.len() as u16);
    for (tag, ty, count, value) in entries {
        le16(&mut t, tag);
        le16(&mut t, ty);
        le32(&mut t, count);
        le32(&mut t, value);
    }
    le32(&mut t, 0);
    for _ in 0..3 {
        le16(&mut t, 8);
    }
    t.extend([255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 255]);
    t
}

fn b64(data: &[u8]) -> String {
    const T: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for c in data.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if c.len() > 1 { T[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if c.len() > 2 { T[n as usize & 63] as char } else { '=' });
    }
    out
}

/// A .docx as Flat OPC (one XML file), the way Word's "Word XML Document" saves it.
fn flat_opc(docx: &[u8]) -> String {
    let pkg = Package::open(docx).unwrap();
    let ct = String::from_utf8(pkg.get("[Content_Types].xml").unwrap().to_vec()).unwrap();
    let types = docxtor_core::xml::parse(ct.as_bytes()).unwrap();
    let ctype = |name: &str| -> String {
        if let Some(o) = types.root.elements().find(|e| e.local() == "Override" && e.attr("PartName") == Some(format!("/{name}").as_str())) {
            return o.attr("ContentType").unwrap().to_string();
        }
        let ext = name.rsplit('.').next().unwrap();
        types.root.elements().find(|e| e.local() == "Default" && e.attr("Extension") == Some(ext)).and_then(|d| d.attr("ContentType")).unwrap_or("application/octet-stream").to_string()
    };
    let mut out = String::from(r#"<?xml version="1.0" standalone="yes"?><?mso-application progid="Word.Document"?><pkg:package xmlns:pkg="http://schemas.microsoft.com/office/2006/xmlPackage">"#);
    for p in &pkg.parts {
        let head = format!(r#"<pkg:part pkg:name="/{}" pkg:contentType="{}""#, p.name, ctype(&p.name));
        if p.name.ends_with(".xml") || p.name.ends_with(".rels") {
            let text = String::from_utf8_lossy(&p.data);
            let body = text.split_once("?>").map(|x| x.1).unwrap_or(&text);
            out.push_str(&format!("{head}><pkg:xmlData>{}</pkg:xmlData></pkg:part>", body.trim()));
        } else {
            out.push_str(&format!(r#"{head} pkg:compression="store"><pkg:binaryData>{}</pkg:binaryData></pkg:part>"#, b64(&p.data)));
        }
    }
    out.push_str("</pkg:package>");
    out
}

// ---------------------------------------------------------------------------
// Looking inside XML text without parsing it (the files are broken on purpose)
// ---------------------------------------------------------------------------

fn is_boundary(b: Option<&u8>) -> bool {
    matches!(b, Some(b' ' | b'>' | b'/' | b'\n' | b'\t' | b'\r'))
}

/// Where the next `<tag` (a whole name, not a longer one) starts at or after `from`.
fn next_open(t: &str, from: usize, tag: &str) -> Option<usize> {
    let open = format!("<{tag}");
    let mut at = from;
    while let Some(i) = t.get(at..)?.find(&open) {
        let i = i + at;
        if is_boundary(t.as_bytes().get(i + open.len())) {
            return Some(i);
        }
        at = i + 1;
    }
    None
}

/// The first element called `tag`, from its `<` to just after its end, with elements of the same name inside it counted.
pub fn find_element(t: &str, tag: &str) -> Option<(usize, usize)> {
    let start = next_open(t, 0, tag)?;
    let gt = t[start..].find('>')? + start;
    if t.as_bytes()[gt - 1] == b'/' {
        return Some((start, gt + 1));
    }
    let close = format!("</{tag}>");
    let (mut depth, mut pos) = (1usize, gt + 1);
    while depth > 0 {
        let nc = t[pos..].find(&close)? + pos;
        match next_open(t, pos, tag).filter(|o| *o < nc) {
            Some(o) => {
                let g = t[o..].find('>')? + o;
                if t.as_bytes()[g - 1] != b'/' {
                    depth += 1;
                }
                pos = g + 1;
            }
            None => {
                depth -= 1;
                pos = nc + close.len();
            }
        }
    }
    Some((start, pos))
}

/// The names of the elements in a text, in order of first appearance.
pub fn tags_of(t: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let b = t.as_bytes();
    let mut i = 0;
    while let Some(k) = t[i..].find('<') {
        let s = i + k + 1;
        if b.get(s).is_some_and(|c| c.is_ascii_alphabetic()) {
            let e = t[s..].find(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, ':' | '_' | '-' | '.'))).map_or(t.len(), |x| x + s);
            let name = &t[s..e];
            if !out.iter().any(|o| o == name) {
                out.push(name.to_string());
            }
        }
        i = s;
    }
    out
}

/// The start tag's end of the root element, and where its end tag starts.
fn root_span(t: &str) -> Option<(usize, usize)> {
    let b = t.as_bytes();
    let mut i = 0;
    loop {
        let k = t[i..].find('<')? + i;
        match b.get(k + 1) {
            Some(b'?') | Some(b'!') => i = t[k..].find('>')? + k + 1,
            _ => break,
        }
    }
    let k = t[i..].find('<')? + i;
    let gt = t[k..].find('>')? + k;
    if b[gt - 1] == b'/' {
        return None;
    }
    let close = t.rfind("</")?;
    (close > gt).then_some((gt + 1, close))
}

// ---------------------------------------------------------------------------
// Mutations of one blob (a part, or a whole file)
// ---------------------------------------------------------------------------

fn splice(t: &str, at: usize, insert: &str) -> String {
    let mut s = String::with_capacity(t.len() + insert.len());
    s.push_str(&t[..at]);
    s.push_str(insert);
    s.push_str(&t[at..]);
    s
}

fn na(why: impl Into<String>) -> Result<Vec<u8>, String> {
    Err(format!("n/a: {}", why.into()))
}

fn arg_n(m: &Value, key: &str, default: u64) -> usize {
    m[key].as_u64().unwrap_or(default) as usize
}

/// Every `="value"` of attributes in a text: (start of value, end of value, attribute name).
fn attributes(t: &str) -> Vec<(usize, usize, &str)> {
    let mut out = Vec::new();
    let mut i = 0;
    while let Some(k) = t[i..].find("=\"") {
        let eq = i + k;
        let vs = eq + 2;
        let Some(ve) = t[vs..].find('"').map(|x| x + vs) else { break };
        let ns = t[..eq].rfind(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, ':' | '_' | '-' | '.'))).map_or(0, |x| x + 1);
        if ns < eq {
            out.push((vs, ve, &t[ns..eq]));
        }
        i = ve + 1;
    }
    out
}

/// A deflate stream (fixed Huffman codes) that unpacks to `n` zero bytes and is about 160 times smaller.
fn deflate_zeros(n: usize) -> Vec<u8> {
    struct Bits {
        out: Vec<u8>,
        acc: u64,
        have: u32,
    }
    impl Bits {
        fn put(&mut self, v: u32, n: u32) {
            self.acc |= (v as u64) << self.have;
            self.have += n;
            while self.have >= 8 {
                self.out.push(self.acc as u8);
                self.acc >>= 8;
                self.have -= 8;
            }
        }
        /// Huffman codes go in most significant bit first.
        fn code(&mut self, v: u32, n: u32) {
            let mut r = 0;
            for i in 0..n {
                r |= ((v >> i) & 1) << (n - 1 - i);
            }
            self.put(r, n);
        }
    }
    let mut b = Bits { out: Vec::new(), acc: 0, have: 0 };
    b.put(1, 1);
    b.put(1, 2);
    b.code(0x30, 8); // one literal zero
    let mut left = n.saturating_sub(1);
    while left >= 258 {
        b.code(0xC5, 8); // length 258
        b.code(0, 5); // distance 1
        left -= 258;
    }
    for _ in 0..left {
        b.code(0x30, 8);
    }
    b.code(0, 7); // end of block
    if b.have > 0 {
        b.out.push(b.acc as u8);
    }
    b.out
}

/// The CRC of `n` zero bytes without going over them (zlib's crc32_combine, by doubling).
pub fn crc_of_zeros(n: usize) -> u32 {
    fn times(mat: &[u32; 32], mut vec: u32) -> u32 {
        let (mut sum, mut i) = (0, 0);
        while vec != 0 {
            if vec & 1 != 0 {
                sum ^= mat[i];
            }
            vec >>= 1;
            i += 1;
        }
        sum
    }
    fn square(sq: &mut [u32; 32], mat: &[u32; 32]) {
        for i in 0..32 {
            sq[i] = times(mat, mat[i]);
        }
    }
    fn combine(mut crc1: u32, crc2: u32, mut len2: u64) -> u32 {
        if len2 == 0 {
            return crc1;
        }
        let (mut even, mut odd) = ([0u32; 32], [0u32; 32]);
        odd[0] = 0xEDB8_8320;
        let mut row = 1u32;
        for o in odd.iter_mut().skip(1) {
            *o = row;
            row <<= 1;
        }
        square(&mut even, &odd);
        square(&mut odd, &even);
        loop {
            square(&mut even, &odd);
            if len2 & 1 != 0 {
                crc1 = times(&even, crc1);
            }
            len2 >>= 1;
            if len2 == 0 {
                break;
            }
            square(&mut odd, &even);
            if len2 & 1 != 0 {
                crc1 = times(&odd, crc1);
            }
            len2 >>= 1;
            if len2 == 0 {
                break;
            }
        }
        crc1 ^ crc2
    }
    let (mut result, mut power, mut len) = (0u32, zip_crc(&[0]), 1u64);
    let mut left = n as u64;
    while left > 0 {
        if left & 1 != 0 {
            result = combine(result, power, len);
        }
        power = combine(power, power, len);
        len *= 2;
        left >>= 1;
    }
    result
}

fn zip_crc(data: &[u8]) -> u32 {
    let mut c = flate2::Crc::new();
    c.update(data);
    c.sum()
}

/// A zip file written by hand: (name, method 0 or 8, packed bytes, crc, unpacked size).
pub fn write_zip(entries: &[(String, u16, Vec<u8>, u32, u64)]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut central = Vec::new();
    for (name, method, data, crc, size) in entries {
        let at = out.len() as u32;
        let header = |v: &mut Vec<u8>, central: bool| {
            le32(v, if central { 0x02014B50 } else { 0x04034B50 });
            if central {
                le16(v, 20);
            }
            le16(v, 20);
            le16(v, 0);
            le16(v, *method);
            le16(v, 0);
            le16(v, 0x21);
            le32(v, *crc);
            le32(v, data.len() as u32);
            le32(v, *size as u32);
            le16(v, name.len() as u16);
            le16(v, 0);
            if central {
                le16(v, 0);
                le16(v, 0);
                le16(v, 0);
                le32(v, 0);
                le32(v, at);
            }
            v.extend(name.as_bytes());
        };
        header(&mut out, false);
        out.extend(data);
        header(&mut central, true);
    }
    let cd_at = out.len() as u32;
    let cd_len = central.len() as u32;
    out.extend(central);
    le32(&mut out, 0x06054B50);
    le16(&mut out, 0);
    le16(&mut out, 0);
    le16(&mut out, entries.len() as u16);
    le16(&mut out, entries.len() as u16);
    le32(&mut out, cd_len);
    le32(&mut out, cd_at);
    le16(&mut out, 0);
    out
}

pub fn blob_of_kind(with: &str, rng: &mut Rng) -> Vec<u8> {
    match with {
        "png" => vec![0x89, b'P', b'N', b'G', 13, 10, 26, 10, 0, 0, 0, 13, b'I', b'H', b'D', b'R'],
        "zip" => fixture("simple.docx"),
        "random" => (0..4096).map(|_| rng.next() as u8).collect(),
        "json" => br#"{"a":[1,2,3],"b":null}"#.to_vec(),
        "html" => b"<html><body><p>hello</p></body></html>".to_vec(),
        "rtf" => br"{\rtf1\ansi hello}".to_vec(),
        "text" => b"hello, this is not what the reader expects".to_vec(),
        "cfb" => fixture("formats/sample.doc"),
        "emf" => base_emf(),
        "tiff" => base_tiff(),
        "xml" => b"<?xml version=\"1.0\"?><a xmlns=\"urn:x\"><b/></a>".to_vec(),
        "bom16" => {
            let mut v = vec![0xFF, 0xFE];
            for c in "<w:document/>".encode_utf16() {
                v.extend(c.to_le_bytes());
            }
            v
        }
        _ => b"?".to_vec(),
    }
}

/// Change one blob as the mutation says. `Err("n/a: ...")` when the mutation has nothing to act on in this blob.
pub fn mutate_blob(blob: &[u8], m: &Value, seed: u64) -> Result<Vec<u8>, String> {
    let mut rng = Rng::new(seed);
    let kind = m["kind"].as_str().unwrap_or("none");
    let text = || String::from_utf8_lossy(blob).into_owned();
    match kind {
        "none" => Ok(blob.to_vec()),
        "empty" => Ok(Vec::new()),
        "truncate" => Ok(blob[..blob.len() * arg_n(m, "permille", 500) / 1000].to_vec()),
        "flip" => {
            if blob.is_empty() {
                return na("empty");
            }
            let mut v = blob.to_vec();
            let head = m["head"].as_bool().unwrap_or(false);
            for _ in 0..arg_n(m, "count", 4) {
                let limit = if head && rng.one_in(2) { v.len().min(1024) } else { v.len() };
                let i = rng.below(limit);
                v[i] ^= (rng.below(255) + 1) as u8;
            }
            Ok(v)
        }
        "poke" => {
            let (off, width, value) = (arg_n(m, "offset", 0), arg_n(m, "width", 4), m["value"].as_u64().unwrap_or(0));
            if off + width > blob.len() {
                return na("offset beyond the end");
            }
            let mut v = blob.to_vec();
            v[off..off + width].copy_from_slice(&value.to_le_bytes()[..width]);
            Ok(v)
        }
        "wrong" => Ok(blob_of_kind(m["with"].as_str().unwrap_or("text"), &mut rng)),
        "digits" => {
            // The digits after a token (for example `spinCount="`) become nines of the same length.
            let token = m["token"].as_str().unwrap_or("spinCount=\"").as_bytes();
            let Some(i) = blob.windows(token.len()).position(|w| w == token) else { return na("token not found") };
            let mut v = blob.to_vec();
            let mut k = i + token.len();
            while k < v.len() && v[k].is_ascii_digit() {
                v[k] = b'9';
                k += 1;
            }
            Ok(v)
        }
        "fill" => {
            // A long run of one character put in at a place: after a literal (or at the start).
            let t = text();
            let at = match m["after"].as_str() {
                Some(lit) => match t.find(lit) {
                    Some(i) => i + lit.len(),
                    None => return na("literal not found"),
                },
                None => 0,
            };
            let run = m["ch"].as_str().unwrap_or("a").repeat(arg_n(m, "size", 1 << 20));
            Ok(splice(&t, at, &run).into_bytes())
        }
        "repeat-lit" => {
            // A literal text put in n times at the end, the start, or after the first line.
            let t = text();
            let lit = m["lit"].as_str().unwrap_or("x");
            let at = match m["at"].as_str().unwrap_or("end") {
                "start" => 0,
                "after-first-line" => t.find('\n').map_or(t.len(), |i| i + 1),
                "before-first-newline" => t.find('\n').unwrap_or(t.len()),
                lit2 if lit2.starts_with("after:") => match t.find(&lit2[6..]) {
                    Some(i) => i + lit2.len() - 6,
                    None => return na("literal not found"),
                },
                // Before the last closing brace or tag: keeps the file balanced.
                lit2 if lit2.starts_with("before-last:") => match t.rfind(&lit2[12..]) {
                    Some(i) => i,
                    None => return na("literal not found"),
                },
                _ => t.len(),
            };
            Ok(splice(&t, at, &lit.repeat(arg_n(m, "n", 1000))).into_bytes())
        }
        "repeat" => {
            // One element (named, or one picked by the seed) written n times instead of once.
            let t = text();
            let tag = match m["tag"].as_str() {
                Some(t) => t.to_string(),
                None => {
                    let tags: Vec<String> = tags_of(&t).into_iter().filter(|g| find_element(&t, g).is_some()).collect();
                    if tags.is_empty() {
                        return na("no elements");
                    }
                    rng.pick(&tags).clone()
                }
            };
            let Some((s, e)) = find_element(&t, &tag) else { return na(format!("no <{tag}>")) };
            let n = arg_n(m, "n", 100_000);
            let mut out = String::with_capacity(t.len() + (e - s) * n);
            out.push_str(&t[..e]);
            for _ in 1..n {
                out.push_str(&t[s..e]);
            }
            out.push_str(&t[e..]);
            Ok(out.into_bytes())
        }
        "nest" => {
            // open x n, then close x n: around an element or a literal, inside the root, or at a place.
            let t = text();
            let (open, close, n) = (m["open"].as_str().unwrap_or("<w:zz>"), m["close"].as_str().unwrap_or("</w:zz>"), arg_n(m, "n", 100_000));
            let (o, c) = (open.repeat(n), close.repeat(n));
            let mut out = String::with_capacity(t.len() + o.len() + c.len());
            match m["mode"].as_str().unwrap_or("root") {
                "around" => {
                    let span = match (m["tag"].as_str(), m["lit"].as_str()) {
                        (Some(tag), _) => find_element(&t, tag),
                        (_, Some(lit)) => t.find(lit).map(|i| (i, i + lit.len())),
                        _ => None,
                    };
                    let Some((s, e)) = span else { return na("nothing to wrap") };
                    out.push_str(&t[..s]);
                    out.push_str(&o);
                    out.push_str(&t[s..e]);
                    out.push_str(&c);
                    out.push_str(&t[e..]);
                }
                "root" => {
                    let Some((s, e)) = root_span(&t) else { return na("no root element") };
                    out.push_str(&t[..s]);
                    out.push_str(&o);
                    out.push_str(&t[s..e]);
                    out.push_str(&c);
                    out.push_str(&t[e..]);
                }
                "inner" => {
                    // Directly inside an element's start tag, or after a literal.
                    let at = match (m["after_tag"].as_str(), m["lit"].as_str()) {
                        (Some(tag), _) => match next_open(&t, 0, tag).and_then(|i| t[i..].find('>').map(|g| g + i + 1)) {
                            Some(x) => x,
                            None => return na("no such element"),
                        },
                        (_, Some(lit)) => match t.find(lit) {
                            Some(i) => i + lit.len(),
                            None => return na("literal not found"),
                        },
                        _ => 0,
                    };
                    out.push_str(&t[..at]);
                    out.push_str(&o);
                    out.push_str(&c);
                    out.push_str(&t[at..]);
                }
                "span" => {
                    // Open after a literal, close before the last occurrence of another: the whole file between.
                    let s = m["after"].as_str().and_then(|l| t.find(l).map(|i| i + l.len())).unwrap_or(0);
                    let e = m["before_last"].as_str().and_then(|l| t.rfind(l)).unwrap_or(t.len());
                    if e < s {
                        return na("bad span");
                    }
                    out.push_str(&t[..s]);
                    out.push_str(&o);
                    out.push_str(&t[s..e]);
                    out.push_str(&c);
                    out.push_str(&t[e..]);
                }
                other => return na(format!("unknown nest mode {other}")),
            }
            Ok(out.into_bytes())
        }
        "attr" => {
            // One attribute's value becomes a very long string.
            let t = text();
            let attrs = attributes(&t);
            let pool: Vec<_> = attrs.iter().filter(|(_, _, n)| m["name"].as_str().is_none_or(|w| w == *n)).collect();
            if pool.is_empty() {
                return na("no attribute");
            }
            let (s, e, _) = **rng.pick(&pool);
            let long = m["ch"].as_str().unwrap_or("9").repeat(arg_n(m, "size", 1 << 20));
            Ok(format!("{}{}{}", &t[..s], long, &t[e..]).into_bytes())
        }
        "number" => {
            // Attributes that hold a number (of a named element, or any) get a hostile number.
            let t = text();
            let value = m["value"].as_str().unwrap_or("2147483647");
            let mut spots: Vec<(usize, usize)> = Vec::new();
            match m["elem"].as_str() {
                Some(elem) => {
                    let mut from = 0;
                    while let Some(i) = next_open(&t, from, elem) {
                        let g = t[i..].find('>').map_or(t.len(), |x| x + i);
                        for (s, e, name) in attributes(&t[i..g]) {
                            if name == m["attr"].as_str().unwrap_or("w:val") {
                                spots.push((s + i, e + i));
                            }
                        }
                        from = i + 1;
                    }
                }
                None => {
                    let nums: Vec<_> = attributes(&t).into_iter().filter(|(s, e, _)| *e > *s && t[*s..*e].trim_start_matches('-').bytes().all(|b| b.is_ascii_digit())).collect();
                    if !nums.is_empty() {
                        let (s, e, _) = *rng.pick(&nums);
                        spots.push((s, e));
                    }
                }
            }
            if spots.is_empty() {
                return na("no number there");
            }
            let mut out = String::with_capacity(t.len());
            let mut last = 0;
            for (s, e) in spots {
                out.push_str(&t[last..s]);
                out.push_str(value);
                last = e;
            }
            out.push_str(&t[last..]);
            Ok(out.into_bytes())
        }
        "text-number" => {
            // The text of every `<elem>123</elem>` becomes a hostile number (shared string indexes, for example).
            let t = text();
            let (elem, value) = (m["elem"].as_str().unwrap_or("v"), m["value"].as_str().unwrap_or("4294967295"));
            let (open, close) = (format!("<{elem}>"), format!("</{elem}>"));
            let mut out = String::with_capacity(t.len());
            let (mut last, mut hit) = (0, false);
            while let Some(i) = t[last..].find(&open).map(|i| i + last) {
                let s = i + open.len();
                let Some(e) = t[s..].find(&close).map(|x| x + s) else { break };
                out.push_str(&t[last..s]);
                if t[s..e].trim().bytes().all(|b| b.is_ascii_digit()) {
                    out.push_str(value);
                    hit = true;
                } else {
                    out.push_str(&t[s..e]);
                }
                last = e;
            }
            if !hit {
                return na("no numbers in <elem>");
            }
            out.push_str(&t[last..]);
            Ok(out.into_bytes())
        }
        "csl-loop" => {
            // Macros that call themselves, or each other: the style cannot be finished.
            let t = text();
            let Some(style_end) = next_open(&t, 0, "style").and_then(|i| t[i..].find('>').map(|g| g + i + 1)) else { return na("no <style>") };
            let Some(layout_end) = next_open(&t, 0, "layout").and_then(|i| t[i..].find('>').map(|g| g + i + 1)) else { return na("no <layout>") };
            let defs = if m["how"].as_str() == Some("pair") { "<macro name=\"zza\"><text macro=\"zzb\"/></macro><macro name=\"zzb\"><text macro=\"zza\"/></macro>" } else { "<macro name=\"zza\"><text macro=\"zza\"/></macro>" };
            let mut out = splice(&t, layout_end, "<text macro=\"zza\"/>");
            out = splice(&out, style_end, defs);
            Ok(out.into_bytes())
        }
        other => na(format!("unknown mutation {other}")),
    }
}

// ---------------------------------------------------------------------------
// Mutations of the package (parts and relationships)
// ---------------------------------------------------------------------------

const REL_BASE: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/";

/// The part a relationship file belongs to: `word/_rels/document.xml.rels` belongs to `word/document.xml`.
fn source_of(rels: &str) -> String {
    match rels.rsplit_once("_rels/") {
        Some((dir, file)) => format!("{dir}{}", file.trim_end_matches(".rels")),
        None => String::new(),
    }
}

fn put_part(pkg: &mut Package, name: &str, data: Vec<u8>) {
    match pkg.parts.iter_mut().find(|p| p.name == name) {
        Some(p) => p.data = data,
        None => pkg.parts.push(Part { name: name.to_string(), data, stored: false }),
    }
}

/// Where a changed relationship points.
fn rel_target(how: &str, source: &str) -> String {
    let base = source.rsplit('/').next().unwrap_or(source);
    match how {
        "self" => base.to_string(),
        "missing" => "missing/nothing.xml".to_string(),
        "main" => "/word/document.xml".to_string(),
        "rels" => "_rels/document.xml.rels".to_string(),
        "root" => "../_rels/.rels".to_string(),
        "traversal" => "../../../../etc/passwd".to_string(),
        "empty" => String::new(),
        "styles" => "/word/styles.xml".to_string(),
        "content-types" => "/[Content_Types].xml".to_string(),
        _ => "chunk1.docx".to_string(),
    }
}

/// Add a part and make sure the content types file knows its extension.
fn add_docx_part(pkg: &mut Package, name: &str, bytes: Vec<u8>) {
    put_part(pkg, name, bytes);
    if let Some(p) = pkg.parts.iter_mut().find(|p| p.name == "[Content_Types].xml") {
        let t = String::from_utf8_lossy(&p.data).into_owned();
        if !t.contains("Extension=\"docx\"") {
            p.data = t.replacen("</Types>", "<Default Extension=\"docx\" ContentType=\"application/vnd.openxmlformats-officedocument.wordprocessingml.document\"/></Types>", 1).into_bytes();
        }
    }
}

/// Add a relationship (with a made-up id) to a relationships file.
fn add_rel(pkg: &mut Package, rels: &str, id: &str, kind: &str, target: &str, external: bool) {
    let rel = format!("<Relationship Id=\"{id}\" Type=\"{REL_BASE}{kind}\" Target=\"{target}\"{}/>", if external { " TargetMode=\"External\"" } else { "" });
    let t = pkg.get(rels).map(|d| String::from_utf8_lossy(d).into_owned()).unwrap_or_else(|| "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?><Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"></Relationships>".to_string());
    let at = t.rfind("</Relationships>").unwrap_or(t.len());
    put_part(pkg, rels, splice(&t, at, &rel).into_bytes());
}

/// An altChunk in the body of the main document that points at relationship `id`.
fn add_alt_chunk(pkg: &mut Package, main: &str, id: &str) {
    let t = String::from_utf8_lossy(pkg.get(main).unwrap_or_default()).into_owned();
    if let Some(i) = t.find("<w:body>") {
        let chunk = format!("<w:altChunk xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\" r:id=\"{id}\"/>");
        put_part(pkg, main, splice(&t, i + 8, &chunk).into_bytes());
    }
}

/// The package changed at the level of parts and relationships.
fn mutate_package(pkg: &mut Package, part: &str, m: &Value, seed: u64) -> Result<(), String> {
    let mut rng = Rng::new(seed);
    let main = pkg.main_document_path();
    match m["kind"].as_str().unwrap_or("") {
        "rel" => {
            // Every relationship of a file (or those of one type) is pointed somewhere it should not go.
            let how = m["how"].as_str().unwrap_or("self");
            let source = source_of(part);
            let want = m["type"].as_str();
            let Some(data) = pkg.get(part).map(|d| String::from_utf8_lossy(d).into_owned()) else { return Err("n/a: no such part".into()) };
            let mut out = String::new();
            let mut last = 0;
            let mut hit = false;
            let mut from = 0;
            while let Some(i) = next_open(&data, from, "Relationship") {
                let g = data[i..].find('>').map_or(data.len(), |x| x + i);
                let tag = &data[i..g];
                if want.is_none_or(|w| tag.contains(&format!("/{w}\""))) {
                    if let Some((s, e, _)) = attributes(tag).into_iter().find(|(_, _, n)| *n == "Target") {
                        out.push_str(&data[last..i + s]);
                        out.push_str(&rel_target(how, &source));
                        last = i + e;
                        hit = true;
                    }
                }
                from = g;
            }
            if !hit {
                return Err("n/a: no relationship of that kind".into());
            }
            out.push_str(&data[last..]);
            put_part(pkg, part, out.into_bytes());
        }
        "rel-add" => {
            // A new relationship of one type (header, footer, comments, chunk, ...) to a place.
            let how = m["how"].as_str().unwrap_or("self");
            let source = source_of(part);
            let kind = m["type"].as_str().unwrap_or("header");
            add_rel(pkg, part, "rIdMut1", kind, &rel_target(how, &source), how == "external");
        }
        "chunk" => {
            // The main document includes another .docx (altChunk): one level, a chain that is `depth` deep, or a
            // document that includes the same chunk twice at every level (so 2^depth copies).
            let depth = arg_n(m, "depth", 1);
            let fan = arg_n(m, "fan", 1);
            let inner = match m["with"].as_str().unwrap_or("simple") {
                "self" => pkg.to_bytes().map_err(|e| e.to_string())?,
                f => fixture(&format!("{f}.docx")),
            };
            let mut chunk = inner;
            for _ in 1..depth {
                let mut p = Package::open(&chunk).map_err(|e| e.to_string())?;
                let mrel = format!("{}_rels/{}.rels", main.rsplit_once('/').map(|x| format!("{}/", x.0)).unwrap_or_default(), main.rsplit('/').next().unwrap_or(""));
                add_docx_part(&mut p, "word/chunk1.docx", chunk.clone());
                for k in 0..fan {
                    let id = format!("rIdMut{k}");
                    add_rel(&mut p, &mrel, &id, "aFChunk", "chunk1.docx", false);
                    add_alt_chunk(&mut p, &main, &id);
                }
                for part in p.parts.iter_mut() {
                    part.stored = true;
                }
                chunk = p.to_bytes().map_err(|e| e.to_string())?;
            }
            add_docx_part(pkg, "word/chunk1.docx", chunk);
            let mrel = format!("{}_rels/{}.rels", main.rsplit_once('/').map(|x| format!("{}/", x.0)).unwrap_or_default(), main.rsplit('/').next().unwrap_or(""));
            for k in 0..fan {
                let id = format!("rIdMut{k}");
                add_rel(pkg, &mrel, &id, "aFChunk", "chunk1.docx", false);
                add_alt_chunk(pkg, &main, &id);
            }
        }
        "chunk-self" => {
            // The chunk relationship points at the main document itself (or at the chunk's own relationships file).
            let mrel = format!("{}_rels/{}.rels", main.rsplit_once('/').map(|x| format!("{}/", x.0)).unwrap_or_default(), main.rsplit('/').next().unwrap_or(""));
            let target = if m["how"].as_str() == Some("rels") { rel_target("rels", &main) } else { rel_target("self", &main) };
            add_rel(pkg, &mrel, "rIdMut0", "aFChunk", &target, false);
            add_alt_chunk(pkg, &main, "rIdMut0");
        }
        "many-parts" => {
            // n small parts (media, custom XML) the file does not need.
            let n = arg_n(m, "n", 100_000);
            for i in 0..n {
                pkg.parts.push(Part { name: format!("word/media/extra{i}.png"), data: vec![0x89, b'P'], stored: true });
            }
        }
        "odd-name" => {
            let names = ["../word/document.xml", "/word/document.xml", "word//document.xml", "word/./document.xml", "WORD/DOCUMENT.XML", "word\\document.xml", "word/document.xml ", "[Content_Types].xml/", ""];
            let name = *rng.pick(&names);
            if let Some(p) = pkg.parts.iter().find(|p| p.name == part).cloned() {
                pkg.parts.push(Part { name: name.to_string(), ..p });
            }
        }
        k => return Err(format!("n/a: unknown package mutation {k}")),
    }
    Ok(())
}

/// The bytes of a case's input.
pub fn build_input(case: &Value) -> Result<Vec<u8>, String> {
    let base = fixture(case["fixture"].as_str().unwrap_or(""));
    let seed = case["seed"].as_u64().unwrap_or(0);
    let part = case["part"].as_str().unwrap_or("(file)");
    let m = &case["mutation"];
    match part {
        "(file)" | "(zip)" => mutate_blob(&base, m, seed),
        _ => {
            let mut pkg = Package::open(&base).map_err(|e| format!("fixture is not a zip: {e}"))?;
            match m["kind"].as_str().unwrap_or("") {
                "dup-part" => {
                    // The same part name twice, the second with different content (the zip crate will not write that).
                    let mut entries: Vec<(String, u16, Vec<u8>, u32, u64)> = pkg.parts.iter().map(|p| (p.name.clone(), 0, p.data.clone(), zip_crc(&p.data), p.data.len() as u64)).collect();
                    let other = b"<?xml version=\"1.0\"?><w:document xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><w:body/></w:document>".to_vec();
                    entries.push((part.to_string(), 0, other.clone(), zip_crc(&other), other.len() as u64));
                    return Ok(write_zip(&entries));
                }
                "rel" | "rel-add" | "chunk" | "chunk-self" | "many-parts" | "odd-name" => mutate_package(&mut pkg, part, m, seed)?,
                "bomb" => {
                    // The part is replaced by a few kilobytes that unpack to a gigabyte or more.
                    let n = arg_n(m, "size", 1 << 30);
                    let mut entries: Vec<(String, u16, Vec<u8>, u32, u64)> = Vec::new();
                    for p in &pkg.parts {
                        if p.name != part {
                            entries.push((p.name.clone(), 0, p.data.clone(), zip_crc(&p.data), p.data.len() as u64));
                        }
                    }
                    entries.push((part.to_string(), 8, deflate_zeros(n), crc_of_zeros(n), n as u64));
                    return Ok(write_zip(&entries));
                }
                _ => {
                    let data = match (m["kind"].as_str(), m["with"].as_str()) {
                        (Some("wrong"), Some(w)) if w.starts_with("other:") => pkg.get(&w[6..]).map(|d| d.to_vec()).ok_or("n/a: no such part")?,
                        _ => {
                            let current = pkg.get(part).ok_or_else(|| format!("n/a: no part {part}"))?.to_vec();
                            mutate_blob(&current, m, seed)?
                        }
                    };
                    put_part(&mut pkg, part, data);
                }
            }
            pkg.to_bytes().map_err(|e| e.to_string())
        }
    }
}

/// The pieces of a case a person needs to read: the part with its numbers made general (`header1` and `header2`
/// are the same sort of part), and a short name for the mutation.
pub fn part_class(part: &str) -> String {
    let mut out = String::new();
    let mut digits = false;
    for c in part.chars() {
        if c.is_ascii_digit() {
            if !digits {
                out.push('N');
            }
            digits = true;
        } else {
            digits = false;
            out.push(c);
        }
    }
    out
}

/// The hostile JSON text for an `ops-json` case: shapes of real ops with one value swapped for something hostile, a
/// huge batch, or JSON nested very deeply. `shape` is one of the recorded op shapes with live ids already put in.
pub fn hostile_ops(shape: &Value, m: &Value, seed: u64) -> String {
    let mut rng = Rng::new(seed);
    let n = arg_n(m, "n", 100_000);
    let how = m["kind"].as_str().unwrap_or("leaf");
    match how {
        "deep-array" => format!("{}{}", "[".repeat(n), "]".repeat(n)),
        "deep-object" => format!("{}1{}", "{\"a\":".repeat(n), "}".repeat(n)),
        "deep-in-op" => {
            // A real op with one field replaced by a deeply nested array.
            let mut v = shape.clone();
            if let Some(o) = v.as_object_mut() {
                let key = o.keys().filter(|k| *k != "op").nth(rng.below(o.len().max(2) - 1)).cloned().unwrap_or("x".into());
                let deep: Value = Value::String(format!("{}{}", "[".repeat(n), "]".repeat(n)));
                o.insert(key.clone(), deep);
                // The string is turned back into text below, so the nesting is raw JSON, not a string.
                let text = serde_json::to_string(&Value::Object(o.clone())).unwrap();
                let needle = format!("\"{}{}\"", "[".repeat(n), "]".repeat(n));
                return text.replace(&needle, &format!("{}{}", "[".repeat(n), "]".repeat(n)));
            }
            v.to_string()
        }
        "huge-batch" => {
            let one = shape.to_string();
            let mut s = String::from("[");
            for i in 0..n {
                if i > 0 {
                    s.push(',');
                }
                s.push_str(&one);
            }
            s.push(']');
            s
        }
        _ => {
            // One value somewhere in the op is replaced.
            let mut v = shape.clone();
            let mut spots: Vec<Vec<String>> = Vec::new();
            fn walk(v: &Value, path: &mut Vec<String>, out: &mut Vec<Vec<String>>) {
                match v {
                    Value::Object(m) => {
                        for (k, x) in m {
                            path.push(k.clone());
                            if k != "op" {
                                out.push(path.clone());
                            }
                            walk(x, path, out);
                            path.pop();
                        }
                    }
                    Value::Array(a) => {
                        for (i, x) in a.iter().enumerate() {
                            path.push(i.to_string());
                            out.push(path.clone());
                            walk(x, path, out);
                            path.pop();
                        }
                    }
                    _ => {}
                }
            }
            walk(&v, &mut Vec::new(), &mut spots);
            if spots.is_empty() {
                return v.to_string();
            }
            let path = rng.pick(&spots).clone();
            let replacement = match how {
                "huge-number" => rng.pick(&[json!(4294967295u64), json!(18446744073709551615u64), json!(9223372036854775807i64), json!(2147483648u64), json!(1e308), json!(100000000u64)]).clone(),
                "negative" => rng.pick(&[json!(-1), json!(-2147483648i64), json!(-9223372036854775808i64), json!(-0.5)]).clone(),
                "float" => rng.pick(&[json!(1.5), json!(0.1), json!(1e-300), json!(123456789.123)]).clone(),
                "huge-string" => Value::String(m["ch"].as_str().unwrap_or("a").repeat(arg_n(m, "size", 1 << 24))),
                "wrong-type" => rng.pick(&[json!(null), json!(true), json!("x"), json!([]), json!({}), json!([1, 2, 3]), json!({"a": 1}), json!(0)]).clone(),
                "huge-array" => Value::Array(vec![json!(0); n]),
                "odd-string" => Value::String(rng.pick(&["\u{0}", "\u{FFFF}", "\u{202E}abc", "\\u0000", "%s%s%s", "<script>", "../../x", "\u{1F600}\u{200D}\u{1F600}"]).to_string()),
                _ => json!(0),
            };
            let mut cur = &mut v;
            for (i, key) in path.iter().enumerate() {
                let last = i + 1 == path.len();
                match cur {
                    Value::Object(m) => {
                        if last {
                            m.insert(key.clone(), replacement.clone());
                            break;
                        }
                        cur = m.get_mut(key).unwrap();
                    }
                    Value::Array(a) => {
                        let idx: usize = key.parse().unwrap_or(0);
                        if last {
                            a[idx] = replacement.clone();
                            break;
                        }
                        cur = &mut a[idx];
                    }
                    _ => break,
                }
            }
            v.to_string()
        }
    }
}
