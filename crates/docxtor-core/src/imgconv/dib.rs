//! Device-independent bitmaps (the pictures inside EMF and WMF records) as files a browser shows.

const MAX_PIXELS: u64 = 100_000_000;

/// A bitmap ready to embed: its MIME type, file bytes and size in pixels.
pub struct Picture {
    pub mime: &'static str,
    pub bytes: Vec<u8>,
    pub w: usize,
    pub h: usize,
    /// Rows run top to bottom (otherwise bottom to top, the usual case).
    pub top_down: bool,
}

/// Standard base64 (with padding).
pub fn base64(data: &[u8]) -> String {
    const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for c in data.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        out.push(A[(n >> 18) as usize & 63] as char);
        out.push(A[(n >> 12) as usize & 63] as char);
        out.push(if c.len() > 1 { A[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if c.len() > 2 { A[n as usize & 63] as char } else { '=' });
    }
    out
}

/// A `data:` URI for a picture.
pub fn data_uri(p: &Picture) -> String {
    format!("data:{};base64,{}", p.mime, base64(&p.bytes))
}

fn u16_at(b: &[u8], o: usize) -> Option<u16> {
    b.get(o..o + 2).map(|x| u16::from_le_bytes([x[0], x[1]]))
}

fn u32_at(b: &[u8], o: usize) -> Option<u32> {
    b.get(o..o + 4).map(|x| u32::from_le_bytes([x[0], x[1], x[2], x[3]]))
}

/// What the bitmap header says.
struct Header {
    size: usize,
    w: i64,
    h: i64,
    bpp: usize,
    comp: u32,
    colors: usize,
}

fn header(bmi: &[u8]) -> Option<Header> {
    let size = u32_at(bmi, 0)? as usize;
    if size == 12 {
        let bpp = u16_at(bmi, 10)? as usize;
        let colors = if bpp <= 8 { 1usize << bpp } else { 0 };
        return Some(Header { size, w: u16_at(bmi, 4)? as i64, h: u16_at(bmi, 6)? as i64, bpp, comp: 0, colors });
    }
    // 40 is BITMAPINFOHEADER, 124 the largest (BITMAPV5HEADER).
    if !(40..=124).contains(&size) {
        return None;
    }
    let bpp = u16_at(bmi, 14)? as usize;
    let comp = u32_at(bmi, 16)?;
    let used = u32_at(bmi, 32)? as usize;
    let colors = if bpp > 0 && bpp <= 8 { if used == 0 || used > 1 << bpp { 1 << bpp } else { used } } else { used.min(256) };
    Some(Header { size, w: u32_at(bmi, 4)? as i32 as i64, h: u32_at(bmi, 8)? as i32 as i64, bpp, comp, colors })
}

/// Bytes the header, any bit masks and the colour table take.
fn bmi_len(hd: &Header) -> usize {
    let masks = if hd.size == 40 && hd.comp == 3 { 12 } else { 0 };
    if hd.size == 12 { 12 + hd.colors * 3 } else { hd.size + masks + hd.colors * 4 }
}

/// Build a displayable picture from a bitmap-info blob and its pixel bits.
pub fn picture(bmi: &[u8], bits: &[u8]) -> Option<Picture> {
    let hd = header(bmi)?;
    let (w, h) = (hd.w, hd.h.abs());
    if w <= 0 || h <= 0 || (w as u64) * (h as u64) > MAX_PIXELS || bits.is_empty() {
        return None;
    }
    let (wu, hu) = (w as usize, h as usize);
    if hd.size != 12 && (hd.comp == 4 || hd.comp == 5) {
        let mime = if hd.comp == 4 { "image/jpeg" } else { "image/png" };
        return Some(Picture { mime, bytes: bits.to_vec(), w: wu, h: hu, top_down: true });
    }
    if !matches!(hd.bpp, 1 | 4 | 8 | 16 | 24 | 32) || hd.comp > 3 {
        return None;
    }
    let want = bmi_len(&hd);
    let mut info = bmi[..want.min(bmi.len())].to_vec();
    info.resize(want, 0);
    let stride = (wu * hd.bpp).div_ceil(32) * 4;
    let mut pixels = bits.to_vec();
    if hd.comp == 0 || hd.comp == 3 {
        let need = stride.checked_mul(hu)?;
        // A bitmap that is missing a little of its end is padded; one that claims far more than the
        // record holds is not worth showing (and would cost a lot of memory to pad).
        if need > 1 << 28 || need > pixels.len().saturating_mul(2).max(1 << 16) {
            return None;
        }
        if pixels.len() < need {
            pixels.resize(need, 0);
        }
        pixels.truncate(need);
        if hd.bpp == 32 && hd.comp == 0 {
            for px in pixels.chunks_exact_mut(4) {
                px[3] = 0xFF;
            }
        }
    }
    let mut out = Vec::with_capacity(54 + info.len() + pixels.len() + 16);
    let (mut head, offset) = if hd.size == 12 {
        // Old-style header: write a modern one with four-byte colour table entries.
        let mut v = Vec::new();
        v.extend_from_slice(&40u32.to_le_bytes());
        v.extend_from_slice(&(w as i32).to_le_bytes());
        v.extend_from_slice(&(hd.h as i32).to_le_bytes());
        v.extend_from_slice(&1u16.to_le_bytes());
        v.extend_from_slice(&(hd.bpp as u16).to_le_bytes());
        v.extend_from_slice(&[0; 24]);
        for e in 0..hd.colors {
            let at = 12 + e * 3;
            v.extend_from_slice(&[info[at], info[at + 1], info[at + 2], 0]);
        }
        let len = v.len();
        (v, len)
    } else {
        (info.clone(), info.len())
    };
    out.extend_from_slice(b"BM");
    out.extend_from_slice(&((14 + offset + pixels.len()) as u32).to_le_bytes());
    out.extend_from_slice(&[0; 4]);
    out.extend_from_slice(&((14 + offset) as u32).to_le_bytes());
    out.append(&mut head);
    out.extend_from_slice(&pixels);
    Some(Picture { mime: "image/bmp", bytes: out, w: wu, h: hu, top_down: hd.h < 0 })
}

/// A picture from a DIB: a bitmap-info header, colour table and pixel bits one after the other.
pub fn picture_from_dib(dib: &[u8]) -> Option<Picture> {
    let hd = header(dib)?;
    let at = bmi_len(&hd);
    picture(dib.get(..at)?, dib.get(at..)?)
}

/// Wrap a DIB (a bitmap-info header, colour table and pixel bits one after the other) in a BMP file
/// header so it can be shown as `data:image/bmp;base64,...`.
pub fn dib_to_bmp(dib: &[u8]) -> Option<Vec<u8>> {
    let p = picture_from_dib(dib)?;
    (p.mime == "image/bmp").then_some(p.bytes)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::imgconv::testutil::W;

    /// A 40-byte-header DIB: `bpp`-bit, `w` by `h` (negative h is top-down), with `palette` entries (BGR0).
    pub fn make_dib(w: i32, h: i32, bpp: u16, palette: &[[u8; 4]], bits: &[u8]) -> Vec<u8> {
        let mut d = W::new();
        d.u32(40).i32(w).i32(h).u16(1).u16(bpp).u32(0).u32(bits.len() as u32).i32(0).i32(0);
        d.u32(palette.len() as u32).u32(0);
        for p in palette {
            d.bytes(p);
        }
        d.bytes(bits);
        d.b
    }

    #[test]
    fn base64_matches_rfc_vectors() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn wraps_a_24_bit_dib() {
        // 2x1 pixels: a row is 6 bytes padded to 8.
        let dib = make_dib(2, 1, 24, &[], &[1, 2, 3, 4, 5, 6, 0, 0]);
        let bmp = dib_to_bmp(&dib).unwrap();
        assert_eq!(&bmp[..2], b"BM");
        assert_eq!(u32_at(&bmp, 2).unwrap() as usize, bmp.len());
        assert_eq!(u32_at(&bmp, 10), Some(54));
        assert_eq!(&bmp[54..], &[1, 2, 3, 4, 5, 6, 0, 0]);
    }

    #[test]
    fn palette_offsets_and_padding_of_short_bits() {
        let pal = [[0, 0, 0, 0], [255, 255, 255, 0]];
        let dib = make_dib(8, 2, 1, &pal, &[0xAA]);
        let bmp = dib_to_bmp(&dib).unwrap();
        assert_eq!(u32_at(&bmp, 10), Some(14 + 40 + 8));
        // Two rows of four bytes each, the missing one padded with zeros.
        assert_eq!(bmp.len(), 14 + 40 + 8 + 8);
    }

    #[test]
    fn thirty_two_bit_pixels_become_opaque() {
        let dib = make_dib(1, 1, 32, &[], &[10, 20, 30, 0]);
        let bmp = dib_to_bmp(&dib).unwrap();
        assert_eq!(&bmp[54..], &[10, 20, 30, 255]);
    }

    #[test]
    fn old_style_header_is_upgraded() {
        let mut d = W::new();
        d.u32(12).u16(2).u16(1).u16(1).u16(8);
        for i in 0..256u32 {
            d.u8(i as u8).u8(0).u8(0);
        }
        d.bytes(&[7, 9, 0, 0]);
        let bmp = dib_to_bmp(&d.b).unwrap();
        assert_eq!(u32_at(&bmp, 14), Some(40));
        assert_eq!(u32_at(&bmp, 10), Some(14 + 40 + 1024));
        assert_eq!(&bmp[14 + 40 + 4..14 + 40 + 8], &[1, 0, 0, 0]);
    }

    #[test]
    fn embedded_jpeg_and_png_pass_through() {
        let mut d = W::new();
        d.u32(40).i32(3).i32(2).u16(1).u16(0).u32(5).u32(0).i32(0).i32(0).u32(0).u32(0);
        let p = picture(&d.b, &[0x89, b'P', b'N', b'G']).unwrap();
        assert_eq!((p.mime, p.w, p.h), ("image/png", 3, 2));
    }

    #[test]
    fn bad_headers_are_none() {
        assert!(dib_to_bmp(&[]).is_none());
        assert!(dib_to_bmp(&[1, 2, 3, 4, 5]).is_none());
        assert!(dib_to_bmp(&make_dib(0, 4, 24, &[], &[0; 16])).is_none());
        assert!(dib_to_bmp(&make_dib(100_000, 100_000, 24, &[], &[0; 16])).is_none());
        assert!(dib_to_bmp(&make_dib(2, 2, 3, &[], &[0; 16])).is_none());
        assert!(dib_to_bmp(&make_dib(2, 2, 24, &[], &[])).is_none());
    }
}
