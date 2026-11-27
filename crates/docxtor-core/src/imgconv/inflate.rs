//! A small inflate (RFC 1951) with the zlib wrapper (RFC 1950), after the structure of zlib's `puff`.

struct Bits<'a> {
    data: &'a [u8],
    pos: usize,
    buf: u32,
    cnt: u32,
}

impl Bits<'_> {
    fn bits(&mut self, n: u32) -> Result<u32, String> {
        while self.cnt < n {
            let b = *self.data.get(self.pos).ok_or("Unexpected end of deflate data")?;
            self.pos += 1;
            self.buf |= (b as u32) << self.cnt;
            self.cnt += 8;
        }
        let v = self.buf & ((1u32 << n) - 1);
        self.buf >>= n;
        self.cnt -= n;
        Ok(v)
    }

    fn decode(&mut self, h: &Huff) -> Result<usize, String> {
        let (mut code, mut first, mut index) = (0i32, 0i32, 0i32);
        for len in 1..16 {
            code |= self.bits(1)? as i32;
            let count = h.count[len] as i32;
            if code - count < first {
                let at = (index + (code - first)) as usize;
                return h.symbol.get(at).map(|&s| s as usize).ok_or_else(|| "Bad deflate code".to_string());
            }
            index += count;
            first += count;
            first <<= 1;
            code <<= 1;
        }
        Err("Bad deflate code".into())
    }
}

struct Huff {
    count: [u16; 16],
    symbol: Vec<u16>,
}

fn build(lengths: &[u8]) -> Huff {
    let mut count = [0u16; 16];
    for &l in lengths {
        count[(l & 15) as usize] += 1;
    }
    let mut offs = [0u16; 16];
    for i in 1..15 {
        offs[i + 1] = offs[i].wrapping_add(count[i]);
    }
    let mut symbol = vec![0u16; lengths.len()];
    for (s, &l) in lengths.iter().enumerate() {
        let l = (l & 15) as usize;
        if l != 0 {
            if let Some(slot) = symbol.get_mut(offs[l] as usize) {
                *slot = s as u16;
            }
            offs[l] = offs[l].wrapping_add(1);
        }
    }
    Huff { count, symbol }
}

pub(crate) const LEN_BASE: [u16; 29] =
    [3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131, 163, 195, 227, 258];
pub(crate) const LEN_EXTRA: [u8; 29] = [0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0];
pub(crate) const DIST_BASE: [u16; 30] = [
    1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537, 2049, 3073, 4097,
    6145, 8193, 12289, 16385, 24577,
];
pub(crate) const DIST_EXTRA: [u8; 30] =
    [0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13, 13];

fn codes(br: &mut Bits, out: &mut Vec<u8>, max_out: usize, lit: &Huff, dist: &Huff) -> Result<(), String> {
    loop {
        let sym = br.decode(lit)?;
        if sym < 256 {
            if out.len() >= max_out {
                return Err("Deflate output is too large".into());
            }
            out.push(sym as u8);
        } else if sym == 256 {
            return Ok(());
        } else {
            let sym = sym - 257;
            if sym >= 29 {
                return Err("Bad deflate length".into());
            }
            let len = LEN_BASE[sym] as usize + br.bits(LEN_EXTRA[sym] as u32)? as usize;
            let ds = br.decode(dist)?;
            if ds >= 30 {
                return Err("Bad deflate distance".into());
            }
            let d = DIST_BASE[ds] as usize + br.bits(DIST_EXTRA[ds] as u32)? as usize;
            if d > out.len() {
                return Err("Deflate distance is too far back".into());
            }
            if out.len() + len > max_out {
                return Err("Deflate output is too large".into());
            }
            let start = out.len() - d;
            for i in 0..len {
                let b = out[start + i];
                out.push(b);
            }
        }
    }
}

fn fixed_tables() -> (Huff, Huff) {
    let mut l = [0u8; 288];
    for (i, v) in l.iter_mut().enumerate() {
        *v = match i {
            0..=143 => 8,
            144..=255 => 9,
            256..=279 => 7,
            _ => 8,
        };
    }
    (build(&l), build(&[5u8; 30]))
}

fn dynamic_tables(br: &mut Bits) -> Result<(Huff, Huff), String> {
    const ORDER: [usize; 19] = [16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15];
    let nlen = br.bits(5)? as usize + 257;
    let ndist = br.bits(5)? as usize + 1;
    let ncode = br.bits(4)? as usize + 4;
    if nlen > 286 || ndist > 30 {
        return Err("Bad deflate table sizes".into());
    }
    let mut cl = [0u8; 19];
    for &o in ORDER.iter().take(ncode) {
        cl[o] = br.bits(3)? as u8;
    }
    let clh = build(&cl);
    let mut lengths = vec![0u8; nlen + ndist];
    let mut i = 0;
    while i < nlen + ndist {
        let sym = br.decode(&clh)?;
        if sym < 16 {
            lengths[i] = sym as u8;
            i += 1;
        } else {
            let (prev, rep) = match sym {
                16 => {
                    if i == 0 {
                        return Err("Bad deflate repeat".into());
                    }
                    (lengths[i - 1], 3 + br.bits(2)? as usize)
                }
                17 => (0, 3 + br.bits(3)? as usize),
                _ => (0, 11 + br.bits(7)? as usize),
            };
            if i + rep > nlen + ndist {
                return Err("Bad deflate repeat".into());
            }
            for _ in 0..rep {
                lengths[i] = prev;
                i += 1;
            }
        }
    }
    Ok((build(&lengths[..nlen]), build(&lengths[nlen..])))
}

/// Inflate a raw deflate stream. `max_out` bounds the output so a hostile stream cannot fill memory.
pub fn inflate(data: &[u8], max_out: usize) -> Result<Vec<u8>, String> {
    let mut br = Bits { data, pos: 0, buf: 0, cnt: 0 };
    let mut out = Vec::new();
    loop {
        let last = br.bits(1)?;
        match br.bits(2)? {
            0 => {
                br.buf = 0;
                br.cnt = 0;
                let p = br.pos;
                let hdr = data.get(p..p + 4).ok_or("Unexpected end of deflate data")?;
                let len = u16::from_le_bytes([hdr[0], hdr[1]]) as usize;
                let nlen = u16::from_le_bytes([hdr[2], hdr[3]]) as usize;
                if len != (!nlen & 0xFFFF) {
                    return Err("Bad stored block".into());
                }
                let body = data.get(p + 4..p + 4 + len).ok_or("Unexpected end of deflate data")?;
                if out.len() + len > max_out {
                    return Err("Deflate output is too large".into());
                }
                out.extend_from_slice(body);
                br.pos = p + 4 + len;
            }
            1 => {
                let (l, d) = fixed_tables();
                codes(&mut br, &mut out, max_out, &l, &d)?;
            }
            2 => {
                let (l, d) = dynamic_tables(&mut br)?;
                codes(&mut br, &mut out, max_out, &l, &d)?;
            }
            _ => return Err("Bad deflate block type".into()),
        }
        if last == 1 {
            return Ok(out);
        }
    }
}

/// Decompress a zlib stream (the Adler checksum is not verified).
pub fn zlib_decompress(data: &[u8], max_out: usize) -> Result<Vec<u8>, String> {
    if data.len() < 2 || data[0] & 0x0F != 8 || (u16::from(data[0]) << 8 | u16::from(data[1])) % 31 != 0 {
        return Err("Bad zlib header".into());
    }
    if data[1] & 0x20 != 0 {
        return Err("zlib preset dictionaries are not supported".into());
    }
    inflate(&data[2..], max_out)
}

/// Decompress a gzip stream (RFC 1952; the CRC is not verified). Word's `.emz` and `.wmz` pictures are gzipped EMF and WMF.
pub fn gunzip(data: &[u8], max_out: usize) -> Result<Vec<u8>, String> {
    if data.len() < 18 || data[0] != 0x1f || data[1] != 0x8b || data[2] != 8 {
        return Err("Bad gzip header".into());
    }
    let flags = data[3];
    let mut at = 10usize;
    if flags & 4 != 0 {
        let n = data.get(at..at + 2).map(|b| u16::from_le_bytes([b[0], b[1]]) as usize).ok_or("Bad gzip header")?;
        at += 2 + n;
    }
    for bit in [8u8, 16] {
        if flags & bit != 0 {
            at += data.get(at..).and_then(|r| r.iter().position(|b| *b == 0)).ok_or("Bad gzip header")? + 1;
        }
    }
    if flags & 2 != 0 {
        at += 2;
    }
    inflate(data.get(at..).ok_or("Bad gzip header")?, max_out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::imgconv::png::adler32;

    #[test]
    fn stored_block() {
        let z = [0x78, 0x01, 1, 3, 0, 0xFC, 0xFF, b'a', b'b', b'c', 0, 0, 0, 0];
        assert_eq!(zlib_decompress(&z, 100).unwrap(), b"abc");
    }

    #[test]
    fn fixed_huffman_block() {
        // zlib level 9 output for "hello hello hello": fixed codes with a back reference.
        let z = [120, 218, 203, 72, 205, 201, 201, 87, 200, 64, 144, 0, 58, 46, 6, 125];
        assert_eq!(zlib_decompress(&z, 100).unwrap(), b"hello hello hello");
    }

    #[test]
    fn dynamic_huffman_block() {
        // zlib level 9 output for 689 bytes of words; the trailing Adler checksum verifies the result.
        let z: [u8; 194] = [
            120, 218, 117, 82, 225, 26, 130, 48, 8, 124, 149, 189, 90, 233, 88, 150, 74, 153, 43, 235, 233, 11, 216,
            4, 214, 215, 15, 249, 198, 110, 220, 29, 32, 224, 22, 122, 236, 242, 20, 231, 53, 0, 142, 3, 134, 227,
            130, 207, 57, 224, 35, 46, 138, 244, 152, 52, 185, 229, 161, 187, 104, 186, 158, 34, 227, 231, 60, 93,
            239, 133, 3, 190, 180, 192, 212, 105, 191, 161, 72, 249, 120, 120, 191, 138, 8, 61, 169, 39, 130, 25,
            34, 62, 145, 16, 200, 9, 137, 136, 158, 156, 49, 46, 215, 224, 220, 155, 174, 132, 156, 56, 170, 64, 98,
            39, 194, 200, 165, 230, 104, 156, 237, 132, 76, 83, 59, 178, 226, 102, 148, 155, 188, 178, 86, 11, 102,
            172, 52, 28, 205, 136, 193, 46, 71, 72, 36, 10, 92, 247, 144, 248, 147, 204, 52, 104, 230, 172, 38, 116,
            58, 58, 133, 95, 121, 186, 253, 219, 113, 187, 103, 191, 22, 111, 205, 253, 88, 4, 83, 129, 153, 112,
            211, 157, 186, 101, 61, 13, 2, 80, 225, 7, 28, 22, 1, 230,
        ];
        assert_eq!((z[2] >> 1) & 3, 2);
        let out = zlib_decompress(&z, 10_000).unwrap();
        assert_eq!(out.len(), 689);
        let tail = u32::from_be_bytes([z[190], z[191], z[192], z[193]]);
        assert_eq!(adler32(&out), tail);
    }

    #[test]
    fn limits_and_garbage_are_errors() {
        let z = [0x78, 0x01, 1, 3, 0, 0xFC, 0xFF, b'a', b'b', b'c'];
        assert!(zlib_decompress(&z, 2).is_err());
        assert!(zlib_decompress(&[], 10).is_err());
        assert!(zlib_decompress(&[0x78, 0x01, 0xFF, 0xFF, 0xFF], 10).is_err());
        assert!(inflate(&[0x07], 10).is_err());
    }
}
