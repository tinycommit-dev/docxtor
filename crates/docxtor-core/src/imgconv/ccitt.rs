//! CCITT fax decoding for TIFF: Group 4 (T.6), Group 3 one and two dimensional (T.4) and the
//! byte-aligned Modified Huffman variant. Output is one byte per pixel, 0 for black and 255 for white.

use std::collections::HashMap;

/// Which fax scheme a strip uses.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Scheme {
    /// T.6, all rows coded against the row above.
    G4,
    /// T.4. `two_d` rows are tagged after each EOL; `byte_align` starts every row on a byte.
    G3 { two_d: bool, byte_align: bool },
}

const WHITE: &[(u16, &str)] = &[
    (0, "00110101"), (1, "000111"), (2, "0111"), (3, "1000"), (4, "1011"), (5, "1100"), (6, "1110"), (7, "1111"),
    (8, "10011"), (9, "10100"), (10, "00111"), (11, "01000"), (12, "001000"), (13, "000011"), (14, "110100"),
    (15, "110101"), (16, "101010"), (17, "101011"), (18, "0100111"), (19, "0001100"), (20, "0001000"),
    (21, "0010111"), (22, "0000011"), (23, "0000100"), (24, "0101000"), (25, "0101011"), (26, "0010011"),
    (27, "0100100"), (28, "0011000"), (29, "00000010"), (30, "00000011"), (31, "00011010"), (32, "00011011"),
    (33, "00010010"), (34, "00010011"), (35, "00010100"), (36, "00010101"), (37, "00010110"), (38, "00010111"),
    (39, "00101000"), (40, "00101001"), (41, "00101010"), (42, "00101011"), (43, "00101100"), (44, "00101101"),
    (45, "00000100"), (46, "00000101"), (47, "00001010"), (48, "00001011"), (49, "01010010"), (50, "01010011"),
    (51, "01010100"), (52, "01010101"), (53, "00100100"), (54, "00100101"), (55, "01011000"), (56, "01011001"),
    (57, "01011010"), (58, "01011011"), (59, "01001010"), (60, "01001011"), (61, "00110010"), (62, "00110011"),
    (63, "00110100"),
    (64, "11011"), (128, "10010"), (192, "010111"), (256, "0110111"), (320, "00110110"), (384, "00110111"),
    (448, "01100100"), (512, "01100101"), (576, "01101000"), (640, "01100111"), (704, "011001100"),
    (768, "011001101"), (832, "011010010"), (896, "011010011"), (960, "011010100"), (1024, "011010101"),
    (1088, "011010110"), (1152, "011010111"), (1216, "011011000"), (1280, "011011001"), (1344, "011011010"),
    (1408, "011011011"), (1472, "010011000"), (1536, "010011001"), (1600, "010011010"), (1664, "011000"),
    (1728, "010011011"),
];

const BLACK: &[(u16, &str)] = &[
    (0, "0000110111"), (1, "010"), (2, "11"), (3, "10"), (4, "011"), (5, "0011"), (6, "0010"), (7, "00011"),
    (8, "000101"), (9, "000100"), (10, "0000100"), (11, "0000101"), (12, "0000111"), (13, "00000100"),
    (14, "00000111"), (15, "000011000"), (16, "0000010111"), (17, "0000011000"), (18, "0000001000"),
    (19, "00001100111"), (20, "00001101000"), (21, "00001101100"), (22, "00000110111"), (23, "00000101000"),
    (24, "00000010111"), (25, "00000011000"), (26, "000011001010"), (27, "000011001011"), (28, "000011001100"),
    (29, "000011001101"), (30, "000001101000"), (31, "000001101001"), (32, "000001101010"),
    (33, "000001101011"), (34, "000011010010"), (35, "000011010011"), (36, "000011010100"),
    (37, "000011010101"), (38, "000011010110"), (39, "000011010111"), (40, "000001101100"),
    (41, "000001101101"), (42, "000011011010"), (43, "000011011011"), (44, "000001010100"),
    (45, "000001010101"), (46, "000001010110"), (47, "000001010111"), (48, "000001100100"),
    (49, "000001100101"), (50, "000001010010"), (51, "000001010011"), (52, "000000100100"),
    (53, "000000110111"), (54, "000000111000"), (55, "000000100111"), (56, "000000101000"),
    (57, "000001011000"), (58, "000001011001"), (59, "000000101011"), (60, "000000101100"),
    (61, "000001011010"), (62, "000001100110"), (63, "000001100111"),
    (64, "0000001111"), (128, "000011001000"), (192, "000011001001"), (256, "000001011011"),
    (320, "000000110011"), (384, "000000110100"), (448, "000000110101"), (512, "0000001101100"),
    (576, "0000001101101"), (640, "0000001001010"), (704, "0000001001011"), (768, "0000001001100"),
    (832, "0000001001101"), (896, "0000001110010"), (960, "0000001110011"), (1024, "0000001110100"),
    (1088, "0000001110101"), (1152, "0000001110110"), (1216, "0000001110111"), (1280, "0000001010010"),
    (1344, "0000001010011"), (1408, "0000001010100"), (1472, "0000001010101"), (1536, "0000001011010"),
    (1600, "0000001011011"), (1664, "0000001100100"), (1728, "0000001100101"),
];

/// Make-up codes shared by both colours.
const EXTENDED: &[(u16, &str)] = &[
    (1792, "00000001000"), (1856, "00000001100"), (1920, "00000001101"), (1984, "000000010010"),
    (2048, "000000010011"), (2112, "000000010100"), (2176, "000000010101"), (2240, "000000010110"),
    (2304, "000000010111"), (2368, "000000011100"), (2432, "000000011101"), (2496, "000000011110"),
    (2560, "000000011111"),
];

/// Code table keyed by (length, value).
type Table = HashMap<(u8, u16), u16>;

fn table(codes: &[(u16, &str)]) -> Table {
    let mut t = Table::new();
    for &(run, bits) in codes.iter().chain(EXTENDED) {
        let v = u16::from_str_radix(bits, 2).unwrap_or(0);
        t.insert((bits.len() as u8, v), run);
    }
    t
}

struct Reader<'a> {
    data: &'a [u8],
    pos: usize, // in bits
}

impl Reader<'_> {
    fn bit(&mut self) -> Option<u32> {
        let b = *self.data.get(self.pos >> 3)?;
        let v = (b >> (7 - (self.pos & 7))) & 1;
        self.pos += 1;
        Some(v as u32)
    }

    fn peek(&self, n: usize) -> u32 {
        let mut v = 0;
        for i in 0..n {
            let p = self.pos + i;
            let bit = self.data.get(p >> 3).map_or(0, |b| (b >> (7 - (p & 7))) & 1);
            v = (v << 1) | bit as u32;
        }
        v
    }

    fn at_end(&self) -> bool {
        self.pos >= self.data.len() * 8
    }

    fn align(&mut self) {
        self.pos = (self.pos + 7) & !7;
    }
}

struct Tables {
    white: Table,
    black: Table,
}

fn read_run(r: &mut Reader, t: &Tables, white: bool) -> Result<usize, String> {
    let table = if white { &t.white } else { &t.black };
    let mut total = 0usize;
    loop {
        let (mut len, mut code) = (0u8, 0u16);
        let run = loop {
            let b = r.bit().ok_or("Fax data ended early")?;
            code = (code << 1) | b as u16;
            len += 1;
            if let Some(&run) = table.get(&(len, code)) {
                break run;
            }
            if len >= 13 {
                return Err("Bad fax code".into());
            }
        };
        total += run as usize;
        if run < 64 {
            return Ok(total);
        }
        if total > 1 << 20 {
            return Err("Fax run is too long".into());
        }
    }
}

enum Mode {
    Pass,
    Horizontal,
    Vertical(i32),
    Eol,
}

fn read_mode(r: &mut Reader) -> Result<Mode, String> {
    let mut code = 0u32;
    for len in 1..=7 {
        code = (code << 1) | r.bit().ok_or("Fax data ended early")?;
        match (len, code) {
            (1, 1) => return Ok(Mode::Vertical(0)),
            (3, 0b011) => return Ok(Mode::Vertical(1)),
            (3, 0b010) => return Ok(Mode::Vertical(-1)),
            (3, 0b001) => return Ok(Mode::Horizontal),
            (4, 0b0001) => return Ok(Mode::Pass),
            (6, 0b000011) => return Ok(Mode::Vertical(2)),
            (6, 0b000010) => return Ok(Mode::Vertical(-2)),
            (7, 0b0000011) => return Ok(Mode::Vertical(3)),
            (7, 0b0000010) => return Ok(Mode::Vertical(-3)),
            (7, 0) => {
                // 0000000: the start of an EOL or EOFB (twelve bits, 000000000001).
                return Ok(Mode::Eol);
            }
            _ => {}
        }
    }
    Err("Unsupported fax mode".into())
}

/// Decode one two-dimensional row. `reference` and the result are colour-change positions, starting
/// from white (so even entries change to black).
fn row_2d(r: &mut Reader, t: &Tables, reference: &[usize], width: usize) -> Result<Vec<usize>, String> {
    let mut cur: Vec<usize> = Vec::new();
    let mut a0: i64 = -1;
    let mut white = true;
    let mut bi = 0usize;
    while a0 < width as i64 {
        while bi > 0 && reference[bi - 1] as i64 > a0 {
            bi -= 1;
        }
        while bi < reference.len() && (reference[bi] as i64 <= a0 || (bi & 1 == 0) != white) {
            bi += 1;
        }
        let b1 = reference.get(bi).copied().unwrap_or(width);
        let b2 = reference.get(bi + 1).copied().unwrap_or(width);
        match read_mode(r)? {
            Mode::Pass => a0 = b2 as i64,
            Mode::Horizontal => {
                let start = a0.max(0) as usize;
                let r1 = read_run(r, t, white)?;
                let r2 = read_run(r, t, !white)?;
                let a1 = (start + r1).min(width);
                let a2 = (a1 + r2).min(width);
                cur.push(a1);
                cur.push(a2);
                a0 = a2 as i64;
            }
            Mode::Vertical(d) => {
                let a1 = (b1 as i64 + d as i64).clamp(0, width as i64);
                if a1 < a0 {
                    return Err("Bad fax vertical code".into());
                }
                cur.push(a1 as usize);
                a0 = a1;
                white = !white;
            }
            Mode::Eol => return Err("Unexpected fax end of line".into()),
        }
    }
    Ok(cur)
}

/// Decode one one-dimensional row into colour-change positions.
fn row_1d(r: &mut Reader, t: &Tables, width: usize) -> Result<Vec<usize>, String> {
    let mut cur = Vec::new();
    let mut x = 0usize;
    let mut white = true;
    while x < width {
        x = (x + read_run(r, t, white)?).min(width);
        cur.push(x);
        white = !white;
    }
    Ok(cur)
}

fn paint(row: &mut [u8], changes: &[usize]) {
    let mut x = 0usize;
    let mut white = true;
    for &c in changes.iter().chain(std::iter::once(&row.len())) {
        let c = c.min(row.len());
        if c > x {
            row[x..c].fill(if white { 255 } else { 0 });
            x = c;
        }
        white = !white;
    }
}

/// Skip fill bits and one EOL if present; true when an EOL was consumed.
fn skip_eol(r: &mut Reader) -> bool {
    if r.peek(11) != 0 {
        return false;
    }
    while !r.at_end() {
        if r.bit() == Some(1) {
            return true;
        }
    }
    false
}

/// Decode `rows` rows of `width` pixels. Rows that cannot be decoded stay white; an error is returned
/// only when not a single row could be read.
pub fn decode(data: &[u8], width: usize, rows: usize, scheme: Scheme) -> Result<Vec<u8>, String> {
    let t = Tables { white: table(WHITE), black: table(BLACK) };
    let mut out = vec![255u8; width * rows];
    let mut r = Reader { data, pos: 0 };
    let mut reference: Vec<usize> = Vec::new();
    let mut decoded = 0usize;
    let mut last_err = String::new();
    for y in 0..rows {
        if r.at_end() {
            break;
        }
        let result = match scheme {
            Scheme::G4 => row_2d(&mut r, &t, &reference, width),
            Scheme::G3 { two_d, byte_align } => {
                let had_eol = skip_eol(&mut r);
                let tag_1d = if two_d && had_eol { r.bit() == Some(1) } else { true };
                if byte_align && !had_eol {
                    r.align();
                }
                if tag_1d { row_1d(&mut r, &t, width) } else { row_2d(&mut r, &t, &reference, width) }
            }
        };
        match result {
            Ok(changes) => {
                paint(&mut out[y * width..(y + 1) * width], &changes);
                reference = changes.into_iter().filter(|&c| c < width).collect();
                decoded += 1;
            }
            Err(e) => {
                last_err = e;
                break;
            }
        }
    }
    if decoded == 0 && rows > 0 {
        return Err(if last_err.is_empty() { "Fax data is empty".into() } else { last_err });
    }
    Ok(out)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// Bits as a string, packed MSB first.
    pub fn pack(bits: &str) -> Vec<u8> {
        let mut out = vec![0u8; bits.len().div_ceil(8)];
        for (i, c) in bits.chars().enumerate() {
            if c == '1' {
                out[i / 8] |= 0x80 >> (i % 8);
            }
        }
        out
    }

    pub fn run_code(white: bool, mut n: usize) -> String {
        let table = if white { WHITE } else { BLACK };
        let mut s = String::new();
        while n >= 64 {
            let mk = (n.min(2560) / 64 * 64) as u16;
            let code = table.iter().chain(EXTENDED).find(|(r, _)| *r == mk).unwrap().1;
            s.push_str(code);
            n -= mk as usize;
        }
        s.push_str(table.iter().find(|(r, _)| *r as usize == n).unwrap().1);
        s
    }

    #[test]
    fn code_tables_are_prefix_free_and_complete_enough() {
        for codes in [WHITE, BLACK] {
            let all: Vec<&str> = codes.iter().chain(EXTENDED).map(|c| c.1).collect();
            for (i, a) in all.iter().enumerate() {
                for (j, b) in all.iter().enumerate() {
                    assert!(i == j || !b.starts_with(a), "{a} is a prefix of {b}");
                }
            }
            let kraft: f64 = all.iter().map(|c| 0.5f64.powi(c.len() as i32)).sum();
            // The unused space is the EOL prefix (eleven zeros) and nothing else of note.
            assert!(kraft <= 1.0 - 0.5f64.powi(11) + 1e-9, "kraft {kraft}");
            assert!(kraft > 0.99, "kraft {kraft}");
        }
    }

    #[test]
    fn group4_vertical_horizontal_and_pass() {
        // Width 8. Row 1: horizontal mode, 3 white then 2 black, then V0 to finish (the rest is white).
        // Row 2 equals row 1: V0 V0 V0 (change to black at 3, back to white at 5, end of row at 8).
        let mut bits = String::new();
        bits.push_str("001");
        bits.push_str(&run_code(true, 3));
        bits.push_str(&run_code(false, 2));
        bits.push('1'); // a0 = 5, b1 = width: V0 ends the row
        bits.push_str("111"); // row 2: V0 V0 V0
        let out = decode(&pack(&bits), 8, 2, Scheme::G4).unwrap();
        let expect = [255, 255, 255, 0, 0, 255, 255, 255];
        assert_eq!(&out[..8], &expect);
        assert_eq!(&out[8..], &expect);
    }

    #[test]
    fn group4_pass_mode_and_vertical_offsets() {
        // Row 1 black from 2 to 6 of width 8: horizontal, white 2, black 4, V0 for the end.
        // Row 2: pass over the black run (0001), then VL1 ... instead test VR1 to move the edges by one:
        // V R1 (011) puts the change to black at 3, VR1 puts the change to white at 7, then V0.
        let mut bits = String::from("001");
        bits.push_str(&run_code(true, 2));
        bits.push_str(&run_code(false, 4));
        bits.push('1');
        bits.push_str("011011");
        bits.push('1');
        let out = decode(&pack(&bits), 8, 2, Scheme::G4).unwrap();
        assert_eq!(&out[..8], &[255, 255, 0, 0, 0, 0, 255, 255]);
        assert_eq!(&out[8..], &[255, 255, 255, 0, 0, 0, 0, 255]);
        // Pass mode: the second row is all white (pass past b2 = 6, then V0 at the end).
        let mut bits = String::from("001");
        bits.push_str(&run_code(true, 2));
        bits.push_str(&run_code(false, 4));
        bits.push('1');
        bits.push_str("0001");
        bits.push('1');
        let out = decode(&pack(&bits), 8, 2, Scheme::G4).unwrap();
        assert_eq!(&out[8..], &[255; 8]);
    }

    #[test]
    fn long_runs_use_makeup_codes() {
        let width = 3000;
        let mut bits = String::from("001");
        bits.push_str(&run_code(true, 10));
        bits.push_str(&run_code(false, 2000));
        bits.push('1');
        let out = decode(&pack(&bits), width, 1, Scheme::G4).unwrap();
        assert_eq!(out[9], 255);
        assert_eq!(out[10], 0);
        assert_eq!(out[2009], 0);
        assert_eq!(out[2010], 255);
    }

    #[test]
    fn group3_one_dimensional_rows_with_eol() {
        let eol = "000000000001";
        let mut bits = String::new();
        for black in [1usize, 4] {
            bits.push_str(eol);
            bits.push_str(&run_code(true, 2));
            bits.push_str(&run_code(false, black));
            bits.push_str(&run_code(true, 8 - 2 - black));
        }
        let out = decode(&pack(&bits), 8, 2, Scheme::G3 { two_d: false, byte_align: false }).unwrap();
        assert_eq!(&out[..8], &[255, 255, 0, 255, 255, 255, 255, 255]);
        assert_eq!(&out[8..], &[255, 255, 0, 0, 0, 0, 255, 255]);
    }

    #[test]
    fn modified_huffman_rows_are_byte_aligned() {
        let mut bits = String::new();
        for black in [3usize, 1] {
            bits.push_str(&run_code(true, 1));
            bits.push_str(&run_code(false, black));
            bits.push_str(&run_code(true, 8 - 1 - black));
            while bits.len() & 7 != 0 {
                bits.push('0');
            }
        }
        let out = decode(&pack(&bits), 8, 2, Scheme::G3 { two_d: false, byte_align: true }).unwrap();
        assert_eq!(&out[..8], &[255, 0, 0, 0, 255, 255, 255, 255]);
        assert_eq!(&out[8..], &[255, 0, 255, 255, 255, 255, 255, 255]);
    }

    #[test]
    fn garbage_is_an_error_or_blank() {
        assert!(decode(&[], 8, 4, Scheme::G4).is_err());
        let _ = decode(&[0, 0, 0, 0, 0, 0], 8, 4, Scheme::G4);
        let _ = decode(&[0xFF; 40], 8, 4, Scheme::G3 { two_d: true, byte_align: false });
    }
}
