//! TIFF-style LZW: codes are 9 to 12 bits, most significant bit first, with the "early change"
//! convention (the width grows one code before the table is full). Also PackBits.

const CLEAR: u16 = 256;
const EOI: u16 = 257;

/// Decode a TIFF LZW strip into at most `max_out` bytes. Stops quietly at the end code or when the
/// output is full; a stream that ends early returns what was decoded.
pub fn decode(data: &[u8], max_out: usize) -> Result<Vec<u8>, String> {
    // Each table entry is (prefix code, last byte, string length).
    let mut prefix = vec![0u16; 4096];
    let mut suffix = vec![0u8; 4096];
    let mut length = vec![0u16; 4096];
    for i in 0..256 {
        suffix[i] = i as u8;
        length[i] = 1;
    }
    let mut out: Vec<u8> = Vec::with_capacity(max_out.min(1 << 24));
    let (mut buf, mut cnt, mut pos) = (0u32, 0u32, 0usize);
    let mut width = 9u32;
    let mut next = 258usize;
    let mut prev: Option<usize> = None;
    let mut first_byte = 0u8;
    while out.len() < max_out {
        while cnt < width {
            let Some(&b) = data.get(pos) else { return Ok(out) };
            pos += 1;
            buf = (buf << 8) | b as u32;
            cnt += 8;
        }
        let code = ((buf >> (cnt - width)) & ((1 << width) - 1)) as usize;
        cnt -= width;
        if code == EOI as usize {
            break;
        }
        if code == CLEAR as usize {
            next = 258;
            width = 9;
            prev = None;
            continue;
        }
        let entry_len;
        match prev {
            None => {
                if code >= 256 {
                    return Err("Bad LZW stream".into());
                }
                entry_len = 1;
            }
            Some(p) => {
                if code < next {
                    entry_len = length[code] as usize;
                } else if code == next && next < 4096 {
                    entry_len = length[p] as usize + 1;
                } else {
                    return Err("Bad LZW code".into());
                }
            }
        }
        // Write the string for `code` (or prev + its first byte for the KwKwK case).
        let start = out.len();
        let take = entry_len.min(max_out - start);
        out.resize(start + entry_len, 0);
        if let Some(p) = prev {
            if code == next {
                // The new string is prev followed by prev's first byte.
                let mut c = p;
                let mut at = start + entry_len - 1;
                out[at] = first_byte;
                at -= 1;
                loop {
                    out[at] = suffix[c];
                    if length[c] == 1 {
                        break;
                    }
                    c = prefix[c] as usize;
                    at -= 1;
                }
            } else {
                write_string(&mut out[start..], code, &prefix, &suffix, &length);
            }
        } else {
            out[start] = code as u8;
        }
        let new_first = out[start];
        if let Some(p) = prev {
            if next < 4096 {
                prefix[next] = p as u16;
                suffix[next] = new_first;
                length[next] = length[p] + 1;
                next += 1;
                if next + 1 >= (1 << width) && width < 12 {
                    width += 1;
                }
            }
        }
        if take < entry_len {
            out.truncate(start + take);
        }
        first_byte = new_first;
        prev = Some(code);
    }
    Ok(out)
}

fn write_string(dst: &mut [u8], code: usize, prefix: &[u16], suffix: &[u8], length: &[u16]) {
    let mut c = code;
    let mut at = dst.len();
    while at > 0 {
        at -= 1;
        dst[at] = suffix[c];
        if length[c] == 1 {
            break;
        }
        c = prefix[c] as usize;
    }
}

/// Decode a PackBits run-length strip into at most `max_out` bytes.
pub fn packbits(data: &[u8], max_out: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(max_out.min(1 << 24));
    let mut i = 0;
    while i < data.len() && out.len() < max_out {
        let n = data[i] as i8;
        i += 1;
        if n >= 0 {
            let len = n as usize + 1;
            let end = (i + len).min(data.len());
            out.extend_from_slice(&data[i..end]);
            i = end;
        } else if n != -128 {
            if let Some(&b) = data.get(i) {
                let len = 1 - n as isize;
                out.extend(std::iter::repeat_n(b, len as usize));
            }
            i += 1;
        }
    }
    out.truncate(max_out);
    out
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::collections::HashMap;

    /// A TIFF LZW encoder, written the textbook way, to test the decoder against.
    pub fn encode(data: &[u8]) -> Vec<u8> {
        struct Bw {
            out: Vec<u8>,
            acc: u32,
            n: u32,
        }
        impl Bw {
            fn put(&mut self, code: u32, width: u32) {
                self.acc = (self.acc << width) | code;
                self.n += width;
                while self.n >= 8 {
                    self.out.push((self.acc >> (self.n - 8)) as u8);
                    self.n -= 8;
                }
                self.acc &= (1 << self.n) - 1;
            }
        }
        let mut bw = Bw { out: vec![], acc: 0, n: 0 };
        let mut table: HashMap<(u32, u8), u32> = HashMap::new();
        let mut next = 258u32;
        let width_for = |next: u32| {
            let mut w = 9;
            while next >= (1 << w) {
                w += 1;
            }
            w
        };
        bw.put(CLEAR as u32, 9);
        let mut cur: Option<u32> = None;
        for &b in data {
            match cur {
                None => cur = Some(b as u32),
                Some(c) => {
                    if let Some(&code) = table.get(&(c, b)) {
                        cur = Some(code);
                    } else {
                        bw.put(c, width_for(next));
                        table.insert((c, b), next);
                        next += 1;
                        if next == 4094 {
                            bw.put(CLEAR as u32, width_for(next));
                            table.clear();
                            next = 258;
                        }
                        cur = Some(b as u32);
                    }
                }
            }
        }
        if let Some(c) = cur {
            bw.put(c, width_for(next));
            next += 1;
        }
        bw.put(EOI as u32, width_for(next));
        if bw.n > 0 {
            let pad = 8 - bw.n;
            bw.put(0, pad);
        }
        bw.out
    }

    fn sample(n: usize) -> Vec<u8> {
        let mut x = 12345u32;
        (0..n)
            .map(|i| {
                x = x.wrapping_mul(1103515245).wrapping_add(12345);
                if i % 3 == 0 { (x >> 16) as u8 } else { (i / 7) as u8 }
            })
            .collect()
    }

    #[test]
    fn round_trips_short_runs() {
        for data in [vec![5u8], vec![1, 1, 1, 1, 1, 1, 1, 1], b"abababababab".to_vec(), vec![]] {
            assert_eq!(decode(&encode(&data), 1000).unwrap(), data);
        }
    }

    #[test]
    fn round_trips_across_width_changes_and_clear() {
        for n in [600, 3000, 20000] {
            let data = sample(n);
            assert_eq!(decode(&encode(&data), n).unwrap(), data, "n = {n}");
        }
        let runs = vec![9u8; 50_000];
        assert_eq!(decode(&encode(&runs), runs.len()).unwrap(), runs);
    }

    #[test]
    fn output_is_capped_and_garbage_is_an_error_or_short() {
        let data = sample(500);
        assert_eq!(decode(&encode(&data), 100).unwrap(), data[..100]);
        // Code 511 with no clear code before it names a string that does not exist.
        assert_eq!(decode(&[0xFF, 0xFF, 0xFF, 0xFF], 100).unwrap_err(), "Bad LZW stream");
        // A stream that runs out mid-code returns what it has.
        assert!(decode(&[0xFF], 100).unwrap().is_empty());
        assert!(decode(&[], 10).unwrap().is_empty());
    }

    #[test]
    fn packbits_literals_and_runs() {
        // 2 literal bytes, then a run of 4 sevens, a no-op, then one literal.
        let data = [1u8, 10, 20, 0xFD, 7, 0x80, 0, 99];
        assert_eq!(packbits(&data, 100), vec![10, 20, 7, 7, 7, 7, 99]);
        assert_eq!(packbits(&data, 3), vec![10, 20, 7]);
        assert_eq!(packbits(&[5, 1], 100), vec![1]);
    }
}
