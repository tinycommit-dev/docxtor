//! Pictures a browser cannot show, converted to ones it can: TIFF to PNG, EMF and WMF to SVG.
//!
//! The converters are plain std code with no outside dependencies, so they build for the browser
//! engine too. Nothing here panics on bad input; a file that cannot be converted gives an error.

pub mod ccitt;
pub mod dib;
pub mod emf;
pub mod gdi;
mod gdi_draw;
pub mod inflate;
pub mod lzw;
pub mod png;
pub mod tiff;
pub mod wmf;

pub use dib::dib_to_bmp;
pub use emf::emf_to_svg;
pub use png::encode_png_rgba;
pub use tiff::tiff_to_png;
pub use wmf::wmf_to_svg;

/// A picture in a form the browser shows.
pub struct Display {
    pub mime: &'static str,
    pub ext: &'static str,
    pub bytes: Vec<u8>,
}

#[derive(PartialEq)]
enum Kind {
    Tiff,
    Emf,
    Wmf,
    /// Already shown by browsers.
    Browser,
}

fn sniff(bytes: &[u8]) -> Option<Kind> {
    let u32_at = |o: usize| bytes.get(o..o + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]));
    let u16_at = |o: usize| bytes.get(o..o + 2).map(|b| u16::from_le_bytes([b[0], b[1]]));
    if bytes.starts_with(b"II*\0") || bytes.starts_with(b"MM\0*") {
        return Some(Kind::Tiff);
    }
    if u32_at(0) == Some(1) && bytes.get(40..44) == Some(b" EMF") {
        return Some(Kind::Emf);
    }
    if u32_at(0) == Some(0x9AC6_CDD7) {
        return Some(Kind::Wmf);
    }
    if matches!(u16_at(0), Some(1 | 2)) && u16_at(2) == Some(9) && matches!(u16_at(4), Some(0x0100 | 0x0300)) {
        return Some(Kind::Wmf);
    }
    let head = &bytes[..bytes.len().min(512)];
    let browser = bytes.starts_with(&[0x89, b'P', b'N', b'G'])
        || bytes.starts_with(&[0xFF, 0xD8])
        || bytes.starts_with(b"GIF8")
        || bytes.starts_with(b"BM")
        || (bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP"))
        || head.windows(4).any(|w| w == b"<svg");
    browser.then_some(Kind::Browser)
}

/// The most a compressed metafile may unpack to.
const MAX_METAFILE: usize = crate::budget::caps::METAFILE_BYTES;

/// The browser-displayable form of a picture, or `None` when it already is one (png, jpeg, gif, svg,
/// webp, bmp) or cannot be converted. The bytes' magic decides first, then the extension.
pub fn display_form(ext: &str, bytes: &[u8]) -> Option<Display> {
    // `.emz` and `.wmz` are gzipped EMF and WMF: unpack, then treat as the metafile inside.
    if bytes.starts_with(&[0x1f, 0x8b]) {
        let inner = inflate::gunzip(bytes, MAX_METAFILE).ok()?;
        if inner.starts_with(&[0x1f, 0x8b]) {
            return None;
        }
        let ext = if ext.trim_start_matches('.').eq_ignore_ascii_case("wmz") { "wmf" } else { "emf" };
        return display_form(ext, &inner);
    }
    let kind = sniff(bytes).or_else(|| match ext.trim_start_matches('.').to_ascii_lowercase().as_str() {
        "tif" | "tiff" => Some(Kind::Tiff),
        "emf" => Some(Kind::Emf),
        "wmf" => Some(Kind::Wmf),
        _ => None,
    })?;
    match kind {
        Kind::Tiff => tiff_to_png(bytes).ok().map(|b| Display { mime: "image/png", ext: "png", bytes: b }),
        Kind::Emf => emf_to_svg(bytes).ok().map(svg_display),
        Kind::Wmf => wmf_to_svg(bytes).ok().map(svg_display),
        Kind::Browser => None,
    }
}

fn svg_display(svg: String) -> Display {
    Display { mime: "image/svg+xml", ext: "svg", bytes: svg.into_bytes() }
}

#[cfg(test)]
pub(crate) mod testutil {
    use super::inflate::zlib_decompress;
    use super::png::crc32;

    /// A little-endian byte writer for building test files.
    #[derive(Default)]
    pub struct W {
        pub b: Vec<u8>,
    }

    impl W {
        pub fn new() -> W {
            W::default()
        }
        pub fn u8(&mut self, v: u8) -> &mut W {
            self.b.push(v);
            self
        }
        pub fn u16(&mut self, v: u16) -> &mut W {
            self.b.extend_from_slice(&v.to_le_bytes());
            self
        }
        pub fn i16(&mut self, v: i16) -> &mut W {
            self.b.extend_from_slice(&v.to_le_bytes());
            self
        }
        pub fn u32(&mut self, v: u32) -> &mut W {
            self.b.extend_from_slice(&v.to_le_bytes());
            self
        }
        pub fn i32(&mut self, v: i32) -> &mut W {
            self.b.extend_from_slice(&v.to_le_bytes());
            self
        }
        pub fn f32(&mut self, v: f32) -> &mut W {
            self.b.extend_from_slice(&v.to_le_bytes());
            self
        }
        pub fn bytes(&mut self, v: &[u8]) -> &mut W {
            self.b.extend_from_slice(v);
            self
        }
        pub fn len(&self) -> usize {
            self.b.len()
        }
    }

    /// Decode a PNG written by `encode_png_rgba` (8-bit RGBA, filter 0) to (width, height, pixels).
    pub fn decode_png(png: &[u8]) -> (u32, u32, Vec<u8>) {
        assert_eq!(&png[..8], &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]);
        let (mut at, mut w, mut h, mut idat) = (8, 0u32, 0u32, Vec::new());
        while at + 12 <= png.len() {
            let len = u32::from_be_bytes([png[at], png[at + 1], png[at + 2], png[at + 3]]) as usize;
            let kind = &png[at + 4..at + 8];
            let data = &png[at + 8..at + 8 + len];
            let crc = u32::from_be_bytes([
                png[at + 8 + len],
                png[at + 9 + len],
                png[at + 10 + len],
                png[at + 11 + len],
            ]);
            assert_eq!(crc, crc32(&[kind, data]), "bad chunk crc");
            match kind {
                b"IHDR" => {
                    w = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
                    h = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);
                    assert_eq!(&data[8..10], &[8, 6]);
                }
                b"IDAT" => idat.extend_from_slice(data),
                _ => {}
            }
            at += 12 + len;
        }
        let raw = zlib_decompress(&idat, usize::MAX).unwrap();
        let row = w as usize * 4;
        assert_eq!(raw.len(), (row + 1) * h as usize);
        let mut px = Vec::new();
        for r in raw.chunks(row + 1) {
            assert_eq!(r[0], 0);
            px.extend_from_slice(&r[1..]);
        }
        (w, h, px)
    }

    /// All numbers (as written by the SVG converters) that follow `attr="` in `svg`, in order.
    pub fn attr_numbers(svg: &str, attr: &str) -> Vec<Vec<f64>> {
        let pat = format!(" {attr}=\"");
        let mut out = Vec::new();
        let mut rest = svg;
        while let Some(i) = rest.find(&pat) {
            rest = &rest[i + pat.len()..];
            let end = rest.find('"').unwrap_or(rest.len());
            out.push(numbers(&rest[..end]));
        }
        out
    }

    /// The numbers in a string such as a path or a transform, ignoring everything else.
    pub fn numbers(s: &str) -> Vec<f64> {
        let mut out = Vec::new();
        let b = s.as_bytes();
        let mut i = 0;
        while i < b.len() {
            let start = i;
            if b[i] == b'-' || b[i] == b'.' || b[i].is_ascii_digit() {
                i += 1;
                while i < b.len() && (b[i].is_ascii_digit() || b[i] == b'.') {
                    i += 1;
                }
                if let Ok(v) = s[start..i].parse::<f64>() {
                    out.push(v);
                }
            } else {
                i += 1;
            }
        }
        out
    }

    /// The first element whose tag starts with `<tag`, as text up to the closing `>`.
    pub fn element<'a>(svg: &'a str, tag: &str) -> Option<&'a str> {
        let i = svg.find(&format!("<{tag}"))?;
        let rest = &svg[i..];
        Some(&rest[..rest.find('>')? + 1])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::testutil::W;

    #[test]
    fn browser_formats_and_unknowns_are_left_alone() {
        assert!(display_form("png", &[0x89, b'P', b'N', b'G', 1, 2, 3]).is_none());
        assert!(display_form("jpg", &[0xFF, 0xD8, 0xFF]).is_none());
        assert!(display_form("svg", b"<?xml version=\"1.0\"?><svg xmlns=\"x\"/>").is_none());
        assert!(display_form("bin", b"hello").is_none());
        assert!(display_form("", &[]).is_none());
        // Magic beats the extension.
        assert!(display_form("tif", &[0x89, b'P', b'N', b'G']).is_none());
    }

    #[test]
    fn converts_each_kind_to_its_display_form() {
        use super::emf::tests::EmfBuilder;
        use super::tiff::tests::Tiff;
        use super::wmf::tests::WmfBuilder;
        let tiff = Tiff::new(true, 1, 1).basics(8, 3, 1, 2).block(&[9, 8, 7]).build();
        let d = display_form("tif", &tiff).unwrap();
        assert_eq!((d.mime, d.ext), ("image/png", "png"));
        assert_eq!(testutil::decode_png(&d.bytes).2, vec![9, 8, 7, 255]);

        let mut emf = EmfBuilder::new([0, 0, 99, 99], [0, 0, 2500, 2500]);
        emf.ints(43, &[0, 0, 10, 10]);
        // The extension is wrong but the bytes say EMF.
        let d = display_form("png", &emf.build()).unwrap();
        assert_eq!((d.mime, d.ext), ("image/svg+xml", "svg"));
        assert!(String::from_utf8(d.bytes).unwrap().starts_with("<svg xmlns=\"http://www.w3.org/2000/svg\""));

        let mut wmf = WmfBuilder::placeable([0, 0, 100, 100], 96);
        wmf.rec(0x041B, &[10, 10, 0, 0]);
        let d = display_form("", &wmf.build()).unwrap();
        assert_eq!(d.mime, "image/svg+xml");
        let mut plain = WmfBuilder::plain();
        plain.rec(0x041B, &[10, 10, 0, 0]);
        assert!(display_form("wmf", &plain.build()).is_some());
    }

    /// A gzip stream around `data`, in one stored block (the CRC is not checked by the reader).
    fn gzip_stored(data: &[u8]) -> Vec<u8> {
        let mut g = vec![0x1f, 0x8b, 8, 0, 0, 0, 0, 0, 0, 3, 1];
        g.extend_from_slice(&(data.len() as u16).to_le_bytes());
        g.extend_from_slice(&(!(data.len() as u16)).to_le_bytes());
        g.extend_from_slice(data);
        g.extend_from_slice(&[0; 8]);
        g
    }

    #[test]
    fn a_gzipped_metafile_shows_like_the_metafile() {
        use super::wmf::tests::WmfBuilder;
        let mut wmf = WmfBuilder::placeable([0, 0, 100, 100], 96);
        wmf.rec(0x041B, &[10, 10, 0, 0]);
        let wmf = wmf.build();
        let plain = display_form("wmf", &wmf).unwrap();
        let packed = display_form("wmz", &gzip_stored(&wmf)).unwrap();
        assert_eq!((packed.mime, packed.ext), ("image/svg+xml", "svg"));
        assert_eq!(packed.bytes, plain.bytes);
        // A gzip that is not a metafile, or is cut short, is not shown (and does not panic).
        assert!(display_form("emz", &gzip_stored(b"not a picture")).is_none());
        assert!(display_form("emz", &gzip_stored(&wmf)[..20]).is_none());
        // A header with a name and comment is skipped properly.
        let mut named = gzip_stored(&wmf);
        named[3] = 8 | 16;
        named.splice(10..10, b"a.wmf\0note\0".iter().copied());
        assert!(display_form("wmz", &named).is_some());
        // Gzip inside gzip is refused.
        assert!(display_form("emz", &gzip_stored(&gzip_stored(&wmf))).is_none());
    }

    #[test]
    fn sniffs_by_magic_then_extension() {
        let mut emf = W::new();
        emf.u32(1).u32(88);
        emf.bytes(&[0; 32]).bytes(b" EMF");
        assert!(matches!(sniff(&emf.b), Some(Kind::Emf)));
        assert!(matches!(sniff(&[0xD7, 0xCD, 0xC6, 0x9A, 0, 0]), Some(Kind::Wmf)));
        assert!(matches!(sniff(&[1, 0, 9, 0, 0, 3, 0, 0]), Some(Kind::Wmf)));
        assert!(matches!(sniff(b"II*\0\x08\0\0\0"), Some(Kind::Tiff)));
        assert!(matches!(sniff(b"MM\0*\0\0\0\x08"), Some(Kind::Tiff)));
        // A broken file with a picture extension still gets tried, and fails cleanly.
        assert!(display_form("emf", &[1, 2, 3]).is_none());
        assert!(display_form("TIFF", &[1, 2, 3]).is_none());
    }
}
