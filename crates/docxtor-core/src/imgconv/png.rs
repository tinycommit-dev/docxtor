//! A PNG writer that needs nothing but std. The pixel data is deflated with a small LZ77 matcher and
//! the fixed Huffman codes (good on flat areas such as scanned pages), or stored uncompressed when
//! that is smaller. The CRC and Adler checksums are written by hand.

use super::inflate::{DIST_BASE, DIST_EXTRA, LEN_BASE, LEN_EXTRA};

const fn make_crc_table() -> [u32; 256] {
    let mut table = [0u32; 256];
    let mut i = 0;
    while i < 256 {
        let mut c = i as u32;
        let mut k = 0;
        while k < 8 {
            c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
            k += 1;
        }
        table[i] = c;
        i += 1;
    }
    table
}

static CRC_TABLE: [u32; 256] = make_crc_table();

/// CRC-32 (the PNG/zip polynomial) of the concatenation of `parts`.
pub fn crc32(parts: &[&[u8]]) -> u32 {
    let mut c = 0xFFFF_FFFFu32;
    for part in parts {
        for &b in *part {
            c = CRC_TABLE[((c ^ b as u32) & 0xFF) as usize] ^ (c >> 8);
        }
    }
    c ^ 0xFFFF_FFFF
}

/// Adler-32 checksum of `data`.
pub fn adler32(data: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for chunk in data.chunks(5552) {
        for &x in chunk {
            a += x as u32;
            b += a;
        }
        a %= 65521;
        b %= 65521;
    }
    (b << 16) | a
}

fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    out.extend_from_slice(&crc32(&[kind, data]).to_be_bytes());
}

/// Wrap `raw` in a zlib stream made of stored blocks.
fn zlib_stored(raw: &[u8]) -> Vec<u8> {
    let mut z = Vec::with_capacity(raw.len() + raw.len() / 65535 * 5 + 16);
    z.extend_from_slice(&[0x78, 0x01]);
    if raw.is_empty() {
        z.extend_from_slice(&[1, 0, 0, 0xFF, 0xFF]);
    }
    let mut blocks = raw.chunks(65535).peekable();
    while let Some(block) = blocks.next() {
        z.push(u8::from(blocks.peek().is_none()));
        let len = block.len() as u16;
        z.extend_from_slice(&len.to_le_bytes());
        z.extend_from_slice(&(!len).to_le_bytes());
        z.extend_from_slice(block);
    }
    z.extend_from_slice(&adler32(raw).to_be_bytes());
    z
}

/// Deflate bits go out least significant first.
struct BitWriter {
    out: Vec<u8>,
    acc: u32,
    n: u32,
}

impl BitWriter {
    fn put(&mut self, v: u32, bits: u32) {
        self.acc |= v << self.n;
        self.n += bits;
        while self.n >= 8 {
            self.out.push(self.acc as u8);
            self.acc >>= 8;
            self.n -= 8;
        }
    }

    /// Huffman codes are packed most significant bit first.
    fn code(&mut self, code: u32, len: u32) {
        self.put(code.reverse_bits() >> (32 - len), len);
    }

    fn literal_or_length(&mut self, sym: u32) {
        match sym {
            0..=143 => self.code(0x30 + sym, 8),
            144..=255 => self.code(0x190 + sym - 144, 9),
            256..=279 => self.code(sym - 256, 7),
            _ => self.code(0xC0 + sym - 280, 8),
        }
    }
}

/// One fixed-Huffman deflate block with matches found through a hash of the next three bytes.
fn deflate_fixed(raw: &[u8]) -> Vec<u8> {
    const HASH_BITS: u32 = 15;
    let hash = |i: usize| {
        let v = u32::from_le_bytes([raw[i], raw[i + 1], raw[i + 2], 0]);
        (v.wrapping_mul(0x9E37_79B1) >> (32 - HASH_BITS)) as usize
    };
    let mut head = vec![0u32; 1 << HASH_BITS];
    let mut w = BitWriter { out: Vec::with_capacity(raw.len() / 8 + 64), acc: 0, n: 0 };
    w.put(1, 1);
    w.put(1, 2);
    let n = raw.len();
    let mut i = 0;
    while i < n {
        let mut best = 0;
        let mut dist = 0;
        if i + 3 <= n {
            let h = hash(i);
            let cand = head[h] as usize;
            head[h] = (i + 1) as u32;
            if cand != 0 && i - (cand - 1) <= 32768 {
                let c = cand - 1;
                let max = 258.min(n - i);
                let mut l = 0;
                while l < max && raw[c + l] == raw[i + l] {
                    l += 1;
                }
                if l >= 4 {
                    best = l;
                    dist = i - c;
                }
            }
        }
        if best == 0 {
            w.literal_or_length(raw[i] as u32);
            i += 1;
            continue;
        }
        let lc = LEN_BASE.iter().rposition(|&b| b as usize <= best).unwrap_or(0);
        w.literal_or_length(257 + lc as u32);
        w.put((best - LEN_BASE[lc] as usize) as u32, LEN_EXTRA[lc] as u32);
        let dc = DIST_BASE.iter().rposition(|&b| b as usize <= dist).unwrap_or(0);
        w.code(dc as u32, 5);
        w.put((dist - DIST_BASE[dc] as usize) as u32, DIST_EXTRA[dc] as u32);
        for j in i + 1..(i + best).min(n.saturating_sub(2)) {
            head[hash(j)] = (j + 1) as u32;
        }
        i += best;
    }
    w.literal_or_length(256);
    if w.n > 0 {
        w.put(0, 8 - w.n);
    }
    w.out
}

/// A zlib stream for `raw`, compressed when that helps.
fn zlib(raw: &[u8]) -> Vec<u8> {
    let packed = deflate_fixed(raw);
    // Stored blocks cost five bytes per 64 KiB.
    if packed.len() + 6 >= raw.len() + raw.len() / 65535 * 5 + 11 {
        return zlib_stored(raw);
    }
    let mut z = Vec::with_capacity(packed.len() + 6);
    z.extend_from_slice(&[0x78, 0x01]);
    z.extend_from_slice(&packed);
    z.extend_from_slice(&adler32(raw).to_be_bytes());
    z
}

/// Encode 8-bit RGBA pixels (`w * h * 4` bytes, rows top to bottom) as a PNG. Missing bytes read as
/// transparent black; a zero-sized image becomes one transparent pixel.
pub fn encode_png_rgba(w: u32, h: u32, rgba: &[u8]) -> Vec<u8> {
    let (w, h) = if w == 0 || h == 0 { (1, 1) } else { (w, h) };
    let row = w as usize * 4;
    let mut raw = Vec::with_capacity((row + 1) * h as usize);
    for y in 0..h as usize {
        raw.push(0);
        let start = (y * row).min(rgba.len());
        let end = (start + row).min(rgba.len());
        raw.extend_from_slice(&rgba[start..end]);
        raw.resize((y + 1) * (row + 1), 0);
    }
    let mut png = Vec::with_capacity(raw.len() + 128);
    png.extend_from_slice(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]);
    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&w.to_be_bytes());
    ihdr.extend_from_slice(&h.to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
    chunk(&mut png, b"IHDR", &ihdr);
    chunk(&mut png, b"IDAT", &zlib(&raw));
    chunk(&mut png, b"IEND", &[]);
    png
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::imgconv::testutil::decode_png;

    #[test]
    fn checksums_match_known_values() {
        assert_eq!(crc32(&[b"123456789"]), 0xCBF4_3926);
        assert_eq!(crc32(&[b"1234", b"56789"]), 0xCBF4_3926);
        assert_eq!(adler32(b"Wikipedia"), 0x11E6_0398);
    }

    #[test]
    fn round_trips_pixels() {
        let rgba: Vec<u8> = (0..3 * 2 * 4).map(|i| i as u8 * 7).collect();
        let png = encode_png_rgba(3, 2, &rgba);
        let (w, h, back) = decode_png(&png);
        assert_eq!((w, h), (3, 2));
        assert_eq!(back, rgba);
    }

    #[test]
    fn large_images_use_several_blocks() {
        let rgba = vec![200u8; 300 * 300 * 4];
        let (w, h, back) = decode_png(&encode_png_rgba(300, 300, &rgba));
        assert_eq!((w, h, back.len()), (300, 300, rgba.len()));
        assert!(back.iter().all(|&b| b == 200));
    }

    #[test]
    fn flat_images_are_compressed_and_noise_is_stored() {
        let flat = vec![255u8; 400 * 300 * 4];
        let png = encode_png_rgba(400, 300, &flat);
        assert!(png.len() < 5_000, "flat image took {} bytes", png.len());
        let (_, _, back) = decode_png(&png);
        assert_eq!(back, flat);
        // Pseudo-random bytes do not compress; the output stays correct whichever way it is written.
        let mut x = 1u32;
        let noise: Vec<u8> = (0..64 * 64 * 4)
            .map(|_| {
                x = x.wrapping_mul(1664525).wrapping_add(1013904223);
                (x >> 24) as u8
            })
            .collect();
        let (_, _, back) = decode_png(&encode_png_rgba(64, 64, &noise));
        assert_eq!(back, noise);
    }

    #[test]
    fn matches_cover_overlap_distance_limits_and_every_length() {
        // Patterned data with repeats at short and long distances, and runs of many lengths.
        let mut data = Vec::new();
        for k in 0..400usize {
            data.extend((0..(k % 37 + 1)).map(|i| (i * 3 + k) as u8));
            data.extend(std::iter::repeat_n((k % 5) as u8, k % 300));
            if k % 50 == 0 {
                data.extend_from_within(..data.len().min(40_000) / 3);
            }
        }
        let z = zlib(&data);
        assert_eq!(crate::imgconv::inflate::zlib_decompress(&z, data.len()).unwrap(), data);
        assert!(z.len() < data.len());
        assert_eq!(&z[z.len() - 4..], &adler32(&data).to_be_bytes());
    }

    #[test]
    fn short_or_empty_input_is_padded() {
        let (w, h, back) = decode_png(&encode_png_rgba(2, 2, &[1, 2, 3, 4]));
        assert_eq!((w, h), (2, 2));
        assert_eq!(&back[..5], &[1, 2, 3, 4, 0]);
        let (w, h, back) = decode_png(&encode_png_rgba(0, 0, &[]));
        assert_eq!((w, h, back), (1, 1, vec![0, 0, 0, 0]));
    }
}
