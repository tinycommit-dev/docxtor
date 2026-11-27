//! TIFF to PNG: the first image of a baseline or extended TIFF, decoded to RGBA.
//!
//! Supported: both byte orders, strips and tiles, chunky and planar layouts, no compression, LZW,
//! PackBits, Deflate, CCITT Group 3 and 4, the horizontal predictor (8 and 16 bit), 1/2/4/8/16 bits per
//! sample, grey, palette, RGB, CMYK and unsubsampled YCbCr, alpha as an extra sample, and orientation.

use super::{ccitt, inflate, lzw, png};

/// Pixels allowed in one image: a 400 dpi A4 scan fits, a claim of 100 megapixels does not. Tests use a
/// smaller cap so that corrupted sizes stay quick to reject.
const MAX_PIXELS: u64 = crate::budget::caps::TIFF_PIXELS;
const MAX_BYTES: u64 = crate::budget::caps::TIFF_BYTES;
/// Compressed strips or tiles allowed; more than this is a file built to make a long list of blocks.
const MAX_BLOCKS: usize = crate::budget::caps::TIFF_BLOCKS;

#[derive(Clone, Copy)]
struct Rd<'a> {
    data: &'a [u8],
    le: bool,
}

impl Rd<'_> {
    fn u16(&self, o: usize) -> Option<u16> {
        let b = self.data.get(o..o.checked_add(2)?)?;
        Some(if self.le { u16::from_le_bytes([b[0], b[1]]) } else { u16::from_be_bytes([b[0], b[1]]) })
    }

    fn u32(&self, o: usize) -> Option<u32> {
        let b = self.data.get(o..o.checked_add(4)?)?;
        let a = [b[0], b[1], b[2], b[3]];
        Some(if self.le { u32::from_le_bytes(a) } else { u32::from_be_bytes(a) })
    }
}

struct Entry {
    tag: u16,
    typ: u16,
    count: u32,
    /// Offset of the four-byte value/offset field.
    field: usize,
}

fn type_size(t: u16) -> usize {
    match t {
        1 | 2 | 6 | 7 => 1,
        3 | 8 => 2,
        4 | 9 | 13 => 4,
        16..=18 => 8,
        _ => 0,
    }
}

impl Rd<'_> {
    /// The integer values of an entry (rationals and floats are skipped).
    fn values(&self, e: &Entry) -> Vec<u32> {
        let size = type_size(e.typ);
        if size == 0 {
            return vec![];
        }
        let count = (e.count as usize).min(self.data.len());
        let base = if size * count <= 4 {
            e.field
        } else {
            match self.u32(e.field) {
                Some(o) => o as usize,
                None => return vec![],
            }
        };
        let mut v = Vec::with_capacity(count.min(1 << 16));
        for i in 0..count {
            let Some(o) = base.checked_add(i * size) else { break };
            let x = match e.typ {
                1 | 2 | 6 | 7 => self.data.get(o).map(|&b| b as u32),
                3 | 8 => self.u16(o).map(|x| x as u32),
                4 | 9 | 13 => self.u32(o),
                _ => self.u32(if self.le { o } else { o + 4 }),
            };
            match x {
                Some(x) => v.push(x),
                None => break,
            }
        }
        v
    }
}

struct Info {
    w: usize,
    h: usize,
    bps: u32,
    spp: usize,
    comp: u32,
    photo: u32,
    planar: u32,
    predictor: u32,
    fill_order: u32,
    orientation: u32,
    rps: usize,
    offsets: Vec<u32>,
    counts: Vec<u32>,
    tile: Option<(usize, usize)>,
    colormap: Vec<u32>,
    extra: Vec<u32>,
    subsampling: (u32, u32),
    t4: u32,
    le: bool,
}

fn read_info(rd: &Rd) -> Result<Info, String> {
    let d = rd.data;
    if d.len() < 8 {
        return Err("Not a TIFF file".into());
    }
    match (&d[..2], rd.u16(2)) {
        (b"II", Some(42)) | (b"MM", Some(42)) => {}
        (b"II", Some(43)) | (b"MM", Some(43)) => return Err("BigTIFF is not supported".into()),
        _ => return Err("Not a TIFF file".into()),
    }
    let ifd = rd.u32(4).ok_or("Not a TIFF file")? as usize;
    let n = rd.u16(ifd).ok_or("The TIFF directory is outside the file")? as usize;
    let mut entries = Vec::new();
    for i in 0..n {
        let at = ifd + 2 + i * 12;
        let (Some(tag), Some(typ), Some(count)) = (rd.u16(at), rd.u16(at + 2), rd.u32(at + 4)) else { break };
        entries.push(Entry { tag, typ, count, field: at + 8 });
    }
    let get = |tag: u16| entries.iter().find(|e| e.tag == tag).map(|e| rd.values(e)).unwrap_or_default();
    let one = |tag: u16, default: u32| get(tag).first().copied().unwrap_or(default);
    let w = one(256, 0) as usize;
    let h = one(257, 0) as usize;
    if w == 0 || h == 0 {
        return Err("The TIFF has no size".into());
    }
    if (w as u64) * (h as u64) > MAX_PIXELS {
        return Err("The TIFF is too large".into());
    }
    let spp = one(277, 1) as usize;
    if spp == 0 || spp > 8 {
        return Err(format!("TIFF with {spp} samples per pixel is not supported"));
    }
    let bps_list = get(258);
    let bps = bps_list.first().copied().unwrap_or(1);
    if bps_list.iter().take(spp).any(|&b| b != bps) {
        return Err("TIFF with different bit depths per sample is not supported".into());
    }
    if !matches!(bps, 1 | 2 | 4 | 8 | 16) {
        return Err(format!("TIFF with {bps} bits per sample is not supported"));
    }
    if matches!(one(339, 1), 3 | 4) {
        return Err("TIFF with floating point samples is not supported".into());
    }
    let comp = one(259, 1);
    let photo = one(262, if spp == 1 { 1 } else { 2 });
    let offsets = if get(324).is_empty() { get(273) } else { get(324) };
    let counts = if get(325).is_empty() { get(279) } else { get(325) };
    let tile = match (one(322, 0) as usize, one(323, 0) as usize) {
        (0, _) | (_, 0) => None,
        (tw, th) => Some((tw, th)),
    };
    if let Some((tw, th)) = tile {
        if (tw as u64) * (th as u64) > MAX_PIXELS {
            return Err("The TIFF tiles are too large".into());
        }
    }
    let spp_planes = if one(284, 1) == 2 && spp > 1 { spp } else { 1 };
    let per_plane = match tile {
        Some((tw, th)) => w.div_ceil(tw).saturating_mul(h.div_ceil(th)),
        None => h.div_ceil((one(278, u32::MAX) as usize).clamp(1, h)),
    };
    if per_plane.saturating_mul(spp_planes) > MAX_BLOCKS {
        return Err("The TIFF has too many strips or tiles".into());
    }
    let sub = get(530);
    Ok(Info {
        w,
        h,
        bps,
        spp,
        comp,
        photo,
        planar: one(284, 1),
        predictor: one(317, 1),
        fill_order: one(266, 1),
        orientation: one(274, 1),
        rps: (one(278, u32::MAX) as usize).clamp(1, h),
        offsets,
        counts,
        tile,
        colormap: get(320),
        extra: get(338),
        subsampling: (sub.first().copied().unwrap_or(2), sub.get(1).copied().unwrap_or(2)),
        t4: one(292, 0),
        le: rd.le,
    })
}

/// One compressed strip or tile.
struct Block {
    x0: usize,
    y0: usize,
    /// Stored size (tiles are stored whole, even where they overhang the image).
    bw: usize,
    bh: usize,
    plane: usize,
    index: usize,
}

fn row_bytes(width: usize, channels: usize, bps: u32) -> Result<usize, String> {
    let bits = width as u64 * channels as u64 * bps as u64;
    let bytes = bits.div_ceil(8);
    if bytes > MAX_BYTES {
        return Err("The TIFF rows are too large".into());
    }
    Ok(bytes as usize)
}

fn blocks(info: &Info) -> Vec<Block> {
    let planes = if info.planar == 2 && info.spp > 1 { info.spp } else { 1 };
    let mut v = Vec::new();
    let per_plane;
    if let Some((tw, th)) = info.tile {
        let across = info.w.div_ceil(tw);
        let down = info.h.div_ceil(th);
        per_plane = across * down;
        for plane in 0..planes {
            for t in 0..per_plane {
                let index = plane * per_plane + t;
                if index >= info.offsets.len() {
                    break;
                }
                let (tx, ty) = (t % across, t / across);
                v.push(Block { x0: tx * tw, y0: ty * th, bw: tw, bh: th, plane, index });
            }
        }
    } else {
        let strips = info.h.div_ceil(info.rps);
        per_plane = strips;
        for plane in 0..planes {
            for s in 0..strips {
                let index = plane * per_plane + s;
                if index >= info.offsets.len() {
                    break;
                }
                let y0 = s * info.rps;
                let bh = info.rps.min(info.h - y0);
                v.push(Block { x0: 0, y0, bw: info.w, bh, plane, index });
            }
        }
    }
    v
}

fn compressed<'a>(info: &Info, data: &'a [u8], index: usize) -> Option<std::borrow::Cow<'a, [u8]>> {
    let start = *info.offsets.get(index)? as usize;
    if start >= data.len() {
        return None;
    }
    let end = match info.counts.get(index) {
        Some(&c) if c > 0 => start.saturating_add(c as usize).min(data.len()),
        _ => data.len(),
    };
    let slice = &data[start..end];
    Some(if info.fill_order == 2 {
        std::borrow::Cow::Owned(slice.iter().map(|b| b.reverse_bits()).collect())
    } else {
        std::borrow::Cow::Borrowed(slice)
    })
}

fn undo_predictor(info: &Info, raw: &mut [u8], row_len: usize, channels: usize) -> Result<(), String> {
    match (info.predictor, info.bps) {
        (1, _) => {}
        (2, 8) => {
            for row in raw.chunks_mut(row_len.max(1)) {
                for i in channels..row.len() {
                    row[i] = row[i].wrapping_add(row[i - channels]);
                }
            }
        }
        (2, 16) => {
            let le = info.le;
            let rd = |b: &[u8]| if le { u16::from_le_bytes([b[0], b[1]]) } else { u16::from_be_bytes([b[0], b[1]]) };
            for row in raw.chunks_mut(row_len.max(1)) {
                let words = row.len() / 2;
                for i in channels..words {
                    let v = rd(&row[i * 2..]).wrapping_add(rd(&row[(i - channels) * 2..]));
                    let b = if le { v.to_le_bytes() } else { v.to_be_bytes() };
                    row[i * 2..i * 2 + 2].copy_from_slice(&b);
                }
            }
        }
        (2, n) => return Err(format!("TIFF predictor with {n}-bit samples is not supported")),
        _ => return Err("TIFF floating point predictor is not supported".into()),
    }
    Ok(())
}

/// Decode one block to 8-bit samples, `bw * bh * channels` of them, interleaved.
fn decode_block(info: &Info, data: &[u8], b: &Block, channels: usize) -> Result<Vec<u8>, String> {
    let src = compressed(info, data, b.index).ok_or("A TIFF strip lies outside the file")?;
    if matches!(info.comp, 2..=4) {
        let scheme = match info.comp {
            4 => ccitt::Scheme::G4,
            3 => ccitt::Scheme::G3 { two_d: info.t4 & 1 != 0, byte_align: info.t4 & 4 != 0 },
            _ => ccitt::Scheme::G3 { two_d: false, byte_align: true },
        };
        // A white run is a 0 bit and a black run a 1 bit; the photometric tag then says which is light.
        let mut grey = ccitt::decode(&src, b.bw, b.bh, scheme)?;
        grey.iter_mut().for_each(|v| *v = 255 - *v);
        return Ok(grey);
    }
    let row_len = row_bytes(b.bw, channels, info.bps)?;
    let expected = (row_len as u64) * (b.bh as u64);
    if expected > MAX_BYTES {
        return Err("The TIFF strips are too large".into());
    }
    let expected = expected as usize;
    let mut raw = match info.comp {
        // Uncompressed data has to be there: a short strip is a file claiming more than it holds.
        1 if src.len() < expected => return Err("A TIFF strip is shorter than its size says".into()),
        1 => src[..expected].to_vec(),
        5 => lzw::decode(&src, expected)?,
        32773 => lzw::packbits(&src, expected),
        8 | 32946 => inflate::zlib_decompress(&src, expected)?,
        6 | 7 => return Err("TIFF with JPEG compression is not supported".into()),
        c => return Err(format!("TIFF compression {c} is not supported")),
    };
    raw.resize(expected, 0);
    undo_predictor(info, &mut raw, row_len, channels)?;
    // Unpack to bytes. Palette indexes stay as they are; other samples are scaled to 0..255.
    let scale = info.photo != 3;
    let n = b.bw * channels;
    let mut out = vec![0u8; n * b.bh];
    for (row, dst) in raw.chunks(row_len.max(1)).zip(out.chunks_mut(n.max(1))) {
        match info.bps {
            8 => dst.copy_from_slice(&row[..n]),
            16 => {
                if !scale {
                    return Err("TIFF palette with 16-bit indexes is not supported".into());
                }
                let at = usize::from(info.le);
                for (i, d) in dst.iter_mut().enumerate() {
                    *d = row[i * 2 + at];
                }
            }
            bps => {
                let (per_byte, mask) = (8 / bps as usize, (1u8 << bps) - 1);
                let mul = if scale { 255 / mask } else { 1 };
                for (i, d) in dst.iter_mut().enumerate() {
                    let shift = 8 - bps as usize * (i % per_byte + 1);
                    *d = ((row[i / per_byte] >> shift) & mask) * mul;
                }
            }
        }
    }
    Ok(out)
}

/// How many bytes of picture data one stored byte can at most stand for, by compression. A file that claims
/// more than this for the data it holds is rejected before anything is allocated for it.
fn max_expansion(info: &Info) -> u64 {
    match info.comp {
        1 => 1,
        32773 => 70,
        8 | 32946 => 1100,
        5 => 3000,
        // A fax row can be a single bit, so a stored byte can hold eight rows.
        _ => 8 * (info.w as u64).div_ceil(8),
    }
}

/// The bytes of strip or tile data present in the file (counted once each, never more than the file).
fn stored_bytes(info: &Info, data: &[u8]) -> u64 {
    let total: u64 = (0..info.offsets.len()).filter_map(|i| compressed(info, data, i)).map(|c| c.len() as u64).sum();
    total.min(data.len() as u64)
}

fn expand(info: &Info, samples: &[u8], photo: u32) -> Result<Vec<u8>, String> {
    let spp = info.spp;
    let nb = match photo {
        0 | 1 | 3 => 1,
        2 | 6 => 3,
        5 => 4,
        p => return Err(format!("TIFF photometric interpretation {p} is not supported")),
    };
    if spp < nb {
        return Err("The TIFF has fewer samples than its colour model needs".into());
    }
    if photo == 6 && info.subsampling != (1, 1) {
        return Err("TIFF YCbCr with subsampling is not supported".into());
    }
    let alpha = match info.extra.first() {
        Some(1 | 2) if spp > nb => Some(info.extra[0] == 1),
        _ => None,
    };
    let palette = if photo == 3 {
        let n = 1usize << info.bps.min(8);
        if info.colormap.len() < 3 * n {
            return Err("The TIFF palette is incomplete".into());
        }
        Some((n, &info.colormap))
    } else {
        None
    };
    let mut rgba = Vec::with_capacity(info.w * info.h * 4);
    for px in samples.chunks_exact(spp) {
        let (r, g, b) = match photo {
            0 => (255 - px[0], 255 - px[0], 255 - px[0]),
            1 => (px[0], px[0], px[0]),
            3 => {
                let (n, cm) = palette.unwrap_or((1, &info.colormap));
                let i = (px[0] as usize).min(n - 1);
                ((cm[i] >> 8) as u8, (cm[n + i] >> 8) as u8, (cm[2 * n + i] >> 8) as u8)
            }
            5 => {
                let k = 255 - px[3] as u32;
                let f = |c: u8| ((255 - c as u32) * k / 255) as u8;
                (f(px[0]), f(px[1]), f(px[2]))
            }
            6 => {
                let (y, cb, cr) = (px[0] as f32, px[1] as f32 - 128.0, px[2] as f32 - 128.0);
                let c = |v: f32| v.round().clamp(0.0, 255.0) as u8;
                (c(y + 1.402 * cr), c(y - 0.344_136 * cb - 0.714_136 * cr), c(y + 1.772 * cb))
            }
            _ => (px[0], px[1], px[2]),
        };
        let a = alpha.map_or(255, |_| px[nb]);
        let unpremul = |c: u8| if alpha == Some(true) && a > 0 { (c as u32 * 255 / a as u32).min(255) as u8 } else { c };
        rgba.extend_from_slice(&[unpremul(r), unpremul(g), unpremul(b), a]);
    }
    Ok(rgba)
}

/// Rotate or mirror RGBA pixels so that the stored top-left becomes the visual top-left.
fn orient(w: usize, h: usize, rgba: Vec<u8>, o: u32) -> (usize, usize, Vec<u8>) {
    if !(2..=8).contains(&o) {
        return (w, h, rgba);
    }
    let (nw, nh) = if o >= 5 { (h, w) } else { (w, h) };
    let mut out = vec![0u8; rgba.len()];
    for y in 0..h {
        for x in 0..w {
            let (dx, dy) = match o {
                2 => (w - 1 - x, y),
                3 => (w - 1 - x, h - 1 - y),
                4 => (x, h - 1 - y),
                5 => (y, x),
                6 => (h - 1 - y, x),
                7 => (h - 1 - y, w - 1 - x),
                _ => (y, w - 1 - x),
            };
            let s = (y * w + x) * 4;
            let d = (dy * nw + dx) * 4;
            out[d..d + 4].copy_from_slice(&rgba[s..s + 4]);
        }
    }
    (nw, nh, out)
}

/// Decode the first image of a TIFF to RGBA.
pub fn decode(bytes: &[u8]) -> Result<(usize, usize, Vec<u8>), String> {
    let le = match bytes.get(..2) {
        Some(b"II") => true,
        Some(b"MM") => false,
        _ => return Err("Not a TIFF file".into()),
    };
    let rd = Rd { data: bytes, le };
    let info = read_info(&rd)?;
    let fax = matches!(info.comp, 2..=4);
    if fax && (info.bps != 1 || info.spp != 1) {
        return Err("TIFF fax data must be one bit per pixel".into());
    }
    let spp = info.spp;
    // Refuse what cannot be decoded before allocating for it.
    match info.comp {
        1..=5 | 8 | 32773 | 32946 => {}
        6 | 7 => return Err("TIFF with JPEG compression is not supported".into()),
        c => return Err(format!("TIFF compression {c} is not supported")),
    }
    if let p @ (4 | 7..) = info.photo {
        return Err(format!("TIFF photometric interpretation {p} is not supported"));
    }
    let samples_total = (info.w as u64) * (info.h as u64) * (spp as u64);
    if samples_total > 4 * MAX_PIXELS {
        return Err("The TIFF is too large".into());
    }
    let picture_bytes = (samples_total * info.bps as u64).div_ceil(8);
    if picture_bytes > stored_bytes(&info, bytes).saturating_mul(max_expansion(&info)) {
        return Err("The TIFF claims more picture than its data holds".into());
    }
    let planar = info.planar == 2 && spp > 1;
    let channels = if planar { 1 } else { spp };
    let mut samples = vec![0u8; info.w * info.h * spp];
    let mut done = 0;
    let mut first_err = None;
    for b in blocks(&info) {
        let blk = match decode_block(&info, bytes, &b, channels) {
            Ok(blk) => blk,
            Err(e) => {
                first_err.get_or_insert(e);
                continue;
            }
        };
        done += 1;
        let (vw, vh) = (b.bw.min(info.w - b.x0), b.bh.min(info.h - b.y0));
        for yy in 0..vh {
            let src_row = &blk[(yy * b.bw) * channels..];
            let dst_at = ((b.y0 + yy) * info.w + b.x0) * spp;
            if planar {
                for xx in 0..vw {
                    samples[dst_at + xx * spp + b.plane] = src_row[xx];
                }
            } else {
                samples[dst_at..dst_at + vw * spp].copy_from_slice(&src_row[..vw * spp]);
            }
        }
    }
    if done == 0 {
        return Err(first_err.unwrap_or_else(|| "The TIFF has no image data".into()));
    }
    let rgba = expand(&info, &samples, info.photo)?;
    drop(samples);
    Ok(orient(info.w, info.h, rgba, info.orientation))
}

/// Convert a TIFF to a PNG.
pub fn tiff_to_png(bytes: &[u8]) -> Result<Vec<u8>, String> {
    crate::budget::within(bytes.len(), crate::budget::caps::PICTURE_BYTES).map_err(|e| e.to_string())?;
    let (w, h, rgba) = decode(bytes)?;
    Ok(png::encode_png_rgba(w as u32, h as u32, &rgba))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::imgconv::ccitt::tests::{pack, run_code};
    use crate::imgconv::lzw::tests::encode as lzw_encode;
    use crate::imgconv::testutil::decode_png;

    const SHORT: u16 = 3;
    const LONG: u16 = 4;

    /// A little TIFF writer: header, strip data, extra values, then the directory.
    pub(crate) struct Tiff {
        le: bool,
        tags: Vec<(u16, u16, Vec<u32>)>,
        blocks: Vec<Vec<u8>>,
        tiled: bool,
    }

    impl Tiff {
        pub(crate) fn new(le: bool, w: u32, h: u32) -> Tiff {
            Tiff { le, tags: vec![], blocks: vec![], tiled: false }.tag(256, LONG, &[w]).tag(257, LONG, &[h])
        }

        fn tag(mut self, tag: u16, typ: u16, vals: &[u32]) -> Tiff {
            self.tags.retain(|t| t.0 != tag);
            self.tags.push((tag, typ, vals.to_vec()));
            self
        }

        /// The usual tags: bits per sample, compression, photometric, samples per pixel.
        pub(crate) fn basics(self, bps: u32, spp: u32, comp: u32, photo: u32) -> Tiff {
            self.tag(258, SHORT, &vec![bps; spp as usize])
                .tag(259, SHORT, &[comp])
                .tag(262, SHORT, &[photo])
                .tag(277, SHORT, &[spp])
        }

        pub(crate) fn block(mut self, data: &[u8]) -> Tiff {
            self.blocks.push(data.to_vec());
            self
        }

        fn put16(&self, out: &mut Vec<u8>, v: u16) {
            out.extend_from_slice(&if self.le { v.to_le_bytes() } else { v.to_be_bytes() });
        }

        fn put32(&self, out: &mut Vec<u8>, v: u32) {
            out.extend_from_slice(&if self.le { v.to_le_bytes() } else { v.to_be_bytes() });
        }

        pub(crate) fn build(&self) -> Vec<u8> {
            let mut out = Vec::new();
            out.extend_from_slice(if self.le { b"II" } else { b"MM" });
            self.put16(&mut out, 42);
            self.put32(&mut out, 0);
            let mut offsets = vec![];
            let mut counts = vec![];
            for b in &self.blocks {
                offsets.push(out.len() as u32);
                counts.push(b.len() as u32);
                out.extend_from_slice(b);
                if out.len() % 2 == 1 {
                    out.push(0);
                }
            }
            let mut tags = self.tags.clone();
            let (to, tc) = if self.tiled { (324, 325) } else { (273, 279) };
            tags.push((to, LONG, offsets));
            tags.push((tc, LONG, counts));
            tags.sort_by_key(|t| t.0);
            let mut entries = vec![];
            for (tag, typ, vals) in &tags {
                let mut data = Vec::new();
                for &v in vals {
                    if *typ == SHORT {
                        self.put16(&mut data, v as u16);
                    } else {
                        self.put32(&mut data, v);
                    }
                }
                let field = if data.len() <= 4 {
                    data.resize(4, 0);
                    data
                } else {
                    let at = out.len() as u32;
                    out.extend_from_slice(&data);
                    let mut f = Vec::new();
                    self.put32(&mut f, at);
                    f
                };
                entries.push((*tag, *typ, vals.len() as u32, field));
            }
            let ifd = out.len() as u32;
            self.put16(&mut out, entries.len() as u16);
            for (tag, typ, count, field) in entries {
                self.put16(&mut out, tag);
                self.put16(&mut out, typ);
                self.put32(&mut out, count);
                out.extend_from_slice(&field);
            }
            self.put32(&mut out, 0);
            let at = if self.le { ifd.to_le_bytes() } else { ifd.to_be_bytes() };
            out[4..8].copy_from_slice(&at);
            out
        }
    }

    fn px(rgba: &[u8], i: usize) -> [u8; 4] {
        [rgba[i * 4], rgba[i * 4 + 1], rgba[i * 4 + 2], rgba[i * 4 + 3]]
    }

    fn rgb_2x2() -> Vec<u8> {
        vec![255, 0, 0, 0, 255, 0, 0, 0, 255, 10, 20, 30]
    }

    #[test]
    fn uncompressed_rgb_little_endian_via_png() {
        let t = Tiff::new(true, 2, 2).basics(8, 3, 1, 2).tag(278, SHORT, &[2]).block(&rgb_2x2()).build();
        let (w, h, rgba) = decode_png(&tiff_to_png(&t).unwrap());
        assert_eq!((w, h), (2, 2));
        assert_eq!(px(&rgba, 0), [255, 0, 0, 255]);
        assert_eq!(px(&rgba, 1), [0, 255, 0, 255]);
        assert_eq!(px(&rgba, 2), [0, 0, 255, 255]);
        assert_eq!(px(&rgba, 3), [10, 20, 30, 255]);
    }

    #[test]
    fn big_endian_and_several_strips() {
        let data = rgb_2x2();
        let t = Tiff::new(false, 2, 2)
            .basics(8, 3, 1, 2)
            .tag(278, SHORT, &[1])
            .block(&data[..6])
            .block(&data[6..])
            .build();
        let (_, _, rgba) = decode(&t).unwrap();
        assert_eq!(px(&rgba, 1), [0, 255, 0, 255]);
        assert_eq!(px(&rgba, 3), [10, 20, 30, 255]);
    }

    #[test]
    fn grey_black_is_zero_and_white_is_zero() {
        for (photo, expect) in [(1, [0, 100, 255]), (0, [255, 155, 0])] {
            let t = Tiff::new(true, 3, 1).basics(8, 1, 1, photo).block(&[0, 100, 255]).build();
            let (_, _, rgba) = decode(&t).unwrap();
            for (i, e) in expect.iter().enumerate() {
                assert_eq!(px(&rgba, i), [*e, *e, *e, 255], "photo {photo} pixel {i}");
            }
        }
    }

    #[test]
    fn palette_four_bit() {
        // 16 entries; index 3 is red, index 12 is blue. Colour map values are 16-bit.
        let mut cm = vec![0u32; 48];
        cm[3] = 0xFFFF; // red of entry 3
        cm[32 + 12] = 0xFFFF; // blue of entry 12
        let t = Tiff::new(true, 3, 1).basics(4, 1, 1, 3).tag(320, SHORT, &cm).block(&[0x3C, 0x30]).build();
        let (_, _, rgba) = decode(&t).unwrap();
        assert_eq!(px(&rgba, 0), [255, 0, 0, 255]);
        assert_eq!(px(&rgba, 1), [0, 0, 255, 255]);
        assert_eq!(px(&rgba, 2), [255, 0, 0, 255]);
    }

    #[test]
    fn one_bit_rows_are_byte_aligned() {
        // 10 pixels wide: two bytes per row. Row 0 alternates, row 1 is all set.
        let t = Tiff::new(true, 10, 2)
            .basics(1, 1, 1, 1)
            .block(&[0b1010_1010, 0b1000_0000, 0xFF, 0xC0])
            .build();
        let (_, _, rgba) = decode(&t).unwrap();
        let grey: Vec<u8> = (0..20).map(|i| rgba[i * 4]).collect();
        assert_eq!(&grey[..10], &[255, 0, 255, 0, 255, 0, 255, 0, 255, 0]);
        assert!(grey[10..].iter().all(|&g| g == 255));
        // White is zero flips it.
        let t = Tiff::new(true, 8, 1).basics(1, 1, 1, 0).block(&[0b1000_0001]).build();
        let (_, _, rgba) = decode(&t).unwrap();
        assert_eq!((rgba[0], rgba[4], rgba[28]), (0, 255, 0));
    }

    #[test]
    fn two_bit_grey() {
        let t = Tiff::new(true, 4, 1).basics(2, 1, 1, 1).block(&[0b00_01_10_11]).build();
        let (_, _, rgba) = decode(&t).unwrap();
        let g: Vec<u8> = (0..4).map(|i| rgba[i * 4]).collect();
        assert_eq!(g, vec![0, 85, 170, 255]);
    }

    #[test]
    fn lzw_compressed_rgb() {
        let data: Vec<u8> = (0..12 * 12 * 3).map(|i| ((i * 5) % 251) as u8).collect();
        let t = Tiff::new(true, 12, 12).basics(8, 3, 5, 2).block(&lzw_encode(&data)).build();
        let (w, h, rgba) = decode(&t).unwrap();
        assert_eq!((w, h), (12, 12));
        for i in 0..144 {
            assert_eq!(px(&rgba, i), [data[i * 3], data[i * 3 + 1], data[i * 3 + 2], 255], "pixel {i}");
        }
    }

    #[test]
    fn packbits_compressed() {
        // Row of 6 grey pixels: literal 1,2 then a run of four 9s.
        let t = Tiff::new(false, 6, 1).basics(8, 1, 32773, 1).block(&[1, 1, 2, 0xFD, 9]).build();
        let (_, _, rgba) = decode(&t).unwrap();
        let g: Vec<u8> = (0..6).map(|i| rgba[i * 4]).collect();
        assert_eq!(g, vec![1, 2, 9, 9, 9, 9]);
    }

    #[test]
    fn deflate_compressed() {
        let data = rgb_2x2();
        let mut z = vec![0x78, 0x01, 1, data.len() as u8, 0, !(data.len() as u8), 0xFF];
        z.extend_from_slice(&data);
        z.extend_from_slice(&[0, 0, 0, 0]);
        for comp in [8, 32946] {
            let t = Tiff::new(true, 2, 2).basics(8, 3, comp, 2).block(&z).build();
            let (_, _, rgba) = decode(&t).unwrap();
            assert_eq!(px(&rgba, 3), [10, 20, 30, 255]);
        }
    }

    #[test]
    fn horizontal_predictor_eight_and_sixteen_bit() {
        // RGB: each pixel adds to the one on its left, per channel.
        let t = Tiff::new(true, 3, 1)
            .basics(8, 3, 1, 2)
            .tag(317, SHORT, &[2])
            .block(&[10, 20, 30, 1, 2, 3, 255, 0, 1])
            .build();
        let (_, _, rgba) = decode(&t).unwrap();
        assert_eq!(px(&rgba, 0), [10, 20, 30, 255]);
        assert_eq!(px(&rgba, 1), [11, 22, 33, 255]);
        assert_eq!(px(&rgba, 2), [10, 22, 34, 255]);
        // 16-bit grey, big endian: 0x1000, +0x0100, +0x0100.
        let t = Tiff::new(false, 3, 1)
            .basics(16, 1, 1, 1)
            .tag(317, SHORT, &[2])
            .block(&[0x10, 0x00, 0x01, 0x00, 0x01, 0x00])
            .build();
        let (_, _, rgba) = decode(&t).unwrap();
        let g: Vec<u8> = (0..3).map(|i| rgba[i * 4]).collect();
        assert_eq!(g, vec![0x10, 0x11, 0x12]);
    }

    #[test]
    fn sixteen_bit_takes_the_high_byte_in_either_order() {
        let le = Tiff::new(true, 2, 1).basics(16, 1, 1, 1).block(&[0x34, 0x12, 0xFF, 0x80]).build();
        let be = Tiff::new(false, 2, 1).basics(16, 1, 1, 1).block(&[0x12, 0x34, 0x80, 0xFF]).build();
        for t in [le, be] {
            let (_, _, rgba) = decode(&t).unwrap();
            assert_eq!((rgba[0], rgba[4]), (0x12, 0x80));
        }
    }

    #[test]
    fn planar_configuration_two() {
        // 2x1 image, three planes of one strip each (planes are stored one after the other).
        let t = Tiff::new(true, 2, 1)
            .basics(8, 3, 1, 2)
            .tag(284, SHORT, &[2])
            .block(&[1, 2])
            .block(&[3, 4])
            .block(&[5, 6])
            .build();
        let (_, _, rgba) = decode(&t).unwrap();
        assert_eq!(px(&rgba, 0), [1, 3, 5, 255]);
        assert_eq!(px(&rgba, 1), [2, 4, 6, 255]);
    }

    #[test]
    fn tiles_clip_at_the_edge() {
        // A 3x3 grey image in 2x2 tiles: 4 tiles, the right and bottom ones overhang.
        let mut t = Tiff::new(true, 3, 3)
            .basics(8, 1, 1, 1)
            .tag(322, SHORT, &[2])
            .tag(323, SHORT, &[2])
            .block(&[1, 2, 4, 5])
            .block(&[3, 0, 6, 0])
            .block(&[7, 8, 0, 0])
            .block(&[9, 0, 0, 0]);
        t.tiled = true;
        let (_, _, rgba) = decode(&t.build()).unwrap();
        let g: Vec<u8> = (0..9).map(|i| rgba[i * 4]).collect();
        assert_eq!(g, vec![1, 2, 3, 4, 5, 6, 7, 8, 9]);
    }

    #[test]
    fn alpha_unassociated_and_associated() {
        let t = Tiff::new(true, 1, 1).basics(8, 4, 1, 2).tag(338, SHORT, &[2]).block(&[10, 20, 30, 128]).build();
        assert_eq!(px(&decode(&t).unwrap().2, 0), [10, 20, 30, 128]);
        let t = Tiff::new(true, 1, 1).basics(8, 4, 1, 2).tag(338, SHORT, &[1]).block(&[50, 100, 25, 128]).build();
        assert_eq!(px(&decode(&t).unwrap().2, 0), [99, 199, 49, 128]);
        // Grey with alpha.
        let t = Tiff::new(true, 1, 1).basics(8, 2, 1, 1).tag(338, SHORT, &[2]).block(&[77, 200]).build();
        assert_eq!(px(&decode(&t).unwrap().2, 0), [77, 77, 77, 200]);
        // An unspecified extra sample is ignored.
        let t = Tiff::new(true, 1, 1).basics(8, 4, 1, 2).tag(338, SHORT, &[0]).block(&[1, 2, 3, 0]).build();
        assert_eq!(px(&decode(&t).unwrap().2, 0)[3], 255);
    }

    #[test]
    fn cmyk_and_ycbcr() {
        let t = Tiff::new(true, 2, 1).basics(8, 4, 1, 5).block(&[0, 0, 0, 0, 255, 0, 0, 0]).build();
        let (_, _, rgba) = decode(&t).unwrap();
        assert_eq!(px(&rgba, 0), [255, 255, 255, 255]);
        assert_eq!(px(&rgba, 1), [0, 255, 255, 255]);
        let t = Tiff::new(true, 1, 1).basics(8, 3, 1, 6).tag(530, SHORT, &[1, 1]).block(&[128, 128, 128]).build();
        assert_eq!(px(&decode(&t).unwrap().2, 0), [128, 128, 128, 255]);
        let t = Tiff::new(true, 2, 2).basics(8, 3, 1, 6).block(&[0; 12]).build();
        assert!(decode(&t).unwrap_err().contains("subsampling"));
    }

    #[test]
    fn orientation_is_applied() {
        // 2x1 image [A, B]. Orientation 2 mirrors, 6 rotates a quarter turn clockwise.
        let base = |o: u32| Tiff::new(true, 2, 1).basics(8, 1, 1, 1).tag(274, SHORT, &[o]).block(&[10, 20]).build();
        let g = |t: Vec<u8>| {
            let (w, h, rgba) = decode(&t).unwrap();
            (w, h, (0..w * h).map(|i| rgba[i * 4]).collect::<Vec<u8>>())
        };
        assert_eq!(g(base(1)), (2, 1, vec![10, 20]));
        assert_eq!(g(base(2)), (2, 1, vec![20, 10]));
        assert_eq!(g(base(3)), (2, 1, vec![20, 10]));
        assert_eq!(g(base(6)), (1, 2, vec![10, 20]));
        assert_eq!(g(base(8)), (1, 2, vec![20, 10]));
        assert_eq!(g(base(5)), (1, 2, vec![10, 20]));
    }

    #[test]
    fn group4_fax_page() {
        // 8x2: row 1 has black pixels 3 and 4 (horizontal mode), row 2 repeats it with V0 codes.
        let mut bits = String::from("001");
        bits.push_str(&run_code(true, 3));
        bits.push_str(&run_code(false, 2));
        bits.push_str("1111");
        let t = Tiff::new(false, 8, 2).basics(1, 1, 4, 0).tag(266, SHORT, &[1]).block(&pack(&bits)).build();
        let (_, _, rgba) = decode(&t).unwrap();
        let g: Vec<u8> = (0..16).map(|i| rgba[i * 4]).collect();
        assert_eq!(&g[..8], &[255, 255, 255, 0, 0, 255, 255, 255]);
        assert_eq!(&g[8..], &g[..8]);
        // With min-is-black the same bits mean the opposite: the white runs are the dark pixels.
        let t = Tiff::new(false, 8, 2).basics(1, 1, 4, 1).block(&pack(&bits)).build();
        let (_, _, rgba) = decode(&t).unwrap();
        assert_eq!(rgba[0], 0);
        assert_eq!(rgba[3 * 4], 255);
    }

    #[test]
    fn unsupported_inputs_give_clear_errors() {
        let jpeg = Tiff::new(true, 2, 2).basics(8, 3, 7, 6).block(&[0; 10]).build();
        assert_eq!(decode(&jpeg).unwrap_err(), "TIFF with JPEG compression is not supported");
        let old = Tiff::new(true, 2, 2).basics(8, 3, 6, 6).block(&[0; 10]).build();
        assert!(decode(&old).unwrap_err().contains("JPEG"));
        let lab = Tiff::new(true, 1, 1).basics(8, 3, 1, 8).block(&[0; 3]).build();
        assert!(decode(&lab).unwrap_err().contains("photometric"));
        let bits12 = Tiff::new(true, 1, 1).basics(12, 1, 1, 1).block(&[0; 3]).build();
        assert!(decode(&bits12).unwrap_err().contains("12 bits"));
        let huge = Tiff::new(true, 100_000, 100_000).basics(8, 1, 1, 1).block(&[0; 3]).build();
        assert!(decode(&huge).unwrap_err().contains("too large"));
        assert!(decode(b"II+\0\x08\0\0\0\0\0\0\0").unwrap_err().contains("BigTIFF"));
        assert!(decode(b"hello world").is_err());
        assert!(decode(&[]).is_err());
    }

    #[test]
    fn a_tiny_file_cannot_claim_a_big_picture() {
        // 1,800 x 1,800 RGBA is under the pixel cap but the file holds a single byte of it.
        let t = Tiff::new(true, 1800, 1800).basics(8, 4, 1, 2).tag(338, SHORT, &[2]).tag(274, SHORT, &[6]).block(&[1]).build();
        let start = std::time::Instant::now();
        assert!(decode(&t).is_err());
        // The same claim under a compression that cannot expand that far.
        for comp in [5, 8, 32773] {
            let t = Tiff::new(true, 1800, 1800).basics(8, 4, comp, 2).block(&[1, 2, 3]).build();
            assert!(decode(&t).unwrap_err().contains("claims more"), "comp {comp}");
        }
        assert!(start.elapsed().as_secs() < 2);
    }

    #[test]
    fn a_short_uncompressed_strip_is_refused_not_padded() {
        let t = Tiff::new(true, 4, 4).basics(8, 1, 1, 1).block(&[1, 2, 3, 4, 5, 6, 7, 8]).build();
        assert!(decode(&t).is_err());
    }

    #[test]
    fn unsupported_compression_is_rejected_before_allocating() {
        // 2,000 x 2,000 x 3 would zero-fill 12 MB; JPEG-in-TIFF must fail without it.
        let t = Tiff::new(true, 2000, 2000).basics(8, 3, 7, 6).block(&[0; 20]).build();
        assert_eq!(decode(&t).unwrap_err(), "TIFF with JPEG compression is not supported");
        let t = Tiff::new(true, 2000, 2000).basics(8, 3, 1, 8).block(&[0; 20]).build();
        assert!(decode(&t).unwrap_err().contains("photometric"));
    }

    #[test]
    fn a_list_of_a_million_tiny_tiles_is_refused() {
        let mut t = Tiff::new(true, 3000, 1000).basics(8, 1, 1, 1).tag(322, SHORT, &[1]).tag(323, SHORT, &[1]).block(&[0; 4]);
        t.tiled = true;
        let start = std::time::Instant::now();
        assert!(decode(&t.build()).unwrap_err().contains("too many"));
        assert!(start.elapsed().as_secs() < 2);
    }

    #[test]
    fn truncated_and_garbage_files_never_panic() {
        let lzw = lzw_encode(&(0..300).map(|i| i as u8).collect::<Vec<_>>());
        let files = [
            Tiff::new(true, 10, 10).basics(8, 3, 1, 2).block(&[7; 300]).build(),
            Tiff::new(false, 10, 10).basics(8, 3, 5, 2).block(&lzw).build(),
            Tiff::new(true, 8, 2).basics(1, 1, 4, 0).block(&pack("0011110111")).build(),
        ];
        for f in &files {
            for n in 0..f.len() {
                let _ = decode(&f[..n]);
            }
            for i in 0..f.len() {
                let mut g = f.clone();
                g[i] ^= 0xA5;
                let _ = decode(&g);
            }
        }
    }
}
