//! Reading a zip that may be damaged: a missing or cut central directory, entries that were cut short,
//! entries that fail their checksum.
//!
//! A sound zip is read through the `zip` crate, entry by entry; an entry that fails keeps what could be
//! read from it. When the central directory can't be read (a truncated download, a file that was cut off),
//! the file is scanned from the start for local file headers, as unzip tools do when they repair, and each
//! entry is inflated on its own until the data runs out.

use crate::budget::Budget;
use flate2::{Decompress, FlushDecompress, Status};
use std::io::{Cursor, Read};

const LOCAL: [u8; 4] = [0x50, 0x4B, 0x03, 0x04];
const CENTRAL: [u8; 4] = [0x50, 0x4B, 0x01, 0x02];

/// One file out of the zip.
#[derive(Debug, Clone)]
pub struct RawEntry {
    pub name: String,
    pub data: Vec<u8>,
    pub stored: bool,
    /// The data stops early or fails its checksum: it is what could be read.
    pub damaged: bool,
}

/// What was read, and how.
#[derive(Debug, Clone, Default)]
pub struct ZipRead {
    pub entries: Vec<RawEntry>,
    /// The central directory could not be read, so the entries were found by scanning for local headers.
    pub scanned: bool,
}

impl ZipRead {
    pub fn damaged(&self) -> impl Iterator<Item = &RawEntry> {
        self.entries.iter().filter(|e| e.damaged)
    }
}

/// Whether the bytes start like a zip (a local file header, or an empty archive).
pub fn looks_like_zip(bytes: &[u8]) -> bool {
    bytes.starts_with(&LOCAL) || bytes.starts_with(&[0x50, 0x4B, 0x05, 0x06])
}

/// Read every file in the zip, repairing what can be repaired. `None` when no file at all could be found.
///
/// Every entry spends from `budget` (its bytes and one part); when the budget runs out reading stops, and
/// the caller finds out from `budget.check()`.
pub fn read(bytes: &[u8], budget: &Budget) -> Option<ZipRead> {
    if let Some(r) = read_directory(bytes, budget) {
        return Some(r);
    }
    let r = scan(bytes, budget);
    if r.entries.is_empty() {
        None
    } else {
        Some(r)
    }
}

/// The sound path: the central directory is there. An entry that can't be read to its end keeps its partial data.
fn read_directory(bytes: &[u8], budget: &Budget) -> Option<ZipRead> {
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).ok()?;
    let mut out = ZipRead::default();
    let mut unreadable = 0;
    for i in 0..zip.len() {
        if budget.spend_part().is_err() {
            break;
        }
        let Ok(mut f) = zip.by_index(i) else {
            unreadable += 1;
            continue;
        };
        if f.is_dir() {
            continue;
        }
        let Ok(name) = f.name() else { continue };
        let name = name.trim_start_matches('/').to_string();
        let stored = f.compression() == zip::CompressionMethod::Stored;
        // What the entry says it holds can't be trusted: reserve no more than the budget has left.
        let left = budget.bytes_left();
        let mut data = Vec::with_capacity((f.size() as usize).min(1 << 24).min(left as usize));
        let damaged = (&mut f).take(left.saturating_add(1)).read_to_end(&mut data).is_err();
        if budget.spend_bytes(data.len() as u64).is_err() {
            break;
        }
        out.entries.push(RawEntry { name, data, stored, damaged });
    }
    // Entries the directory lists but the zip crate refuses (an odd method, a bad header): the scan may know them.
    if unreadable > 0 && !budget.is_out() {
        let scanned = scan(bytes, budget);
        for e in scanned.entries {
            if !out.entries.iter().any(|x| x.name == e.name) {
                out.entries.push(e);
            }
        }
    }
    Some(out)
}

fn u16_at(b: &[u8], i: usize) -> usize {
    u16::from_le_bytes([b[i], b[i + 1]]) as usize
}

fn u32_at(b: &[u8], i: usize) -> usize {
    u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]]) as usize
}

fn find(hay: &[u8], needle: &[u8; 4], from: usize) -> Option<usize> {
    hay.get(from..)?.windows(4).position(|w| w == needle).map(|i| i + from)
}

/// The scan: local file headers one after the other, from the start of the file.
pub fn scan(bytes: &[u8], budget: &Budget) -> ZipRead {
    let mut out = ZipRead { scanned: true, ..Default::default() };
    let mut pos = 0;
    let mut index: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    let mut slots: Vec<bool> = Vec::new();
    while let Some(at) = find(bytes, &LOCAL, pos) {
        if at + 30 > bytes.len() || budget.spend_part().is_err() {
            break;
        }
        let flags = u16_at(bytes, at + 6);
        let method = u16_at(bytes, at + 8);
        let csize = u32_at(bytes, at + 18);
        let nlen = u16_at(bytes, at + 26);
        let elen = u16_at(bytes, at + 28);
        let start = at + 30 + nlen + elen;
        if start > bytes.len() || nlen == 0 {
            pos = at + 4;
            continue;
        }
        let name = String::from_utf8_lossy(&bytes[at + 30..at + 30 + nlen]).replace('\\', "/").trim_start_matches('/').to_string();
        let descriptor = flags & 8 != 0;
        let (data, damaged, used) = match method {
            0 => {
                // Stored: the header says how long, unless a data descriptor follows.
                let known = !descriptor && csize > 0;
                let mut end = if known && start + csize <= bytes.len() { start + csize } else { next_header(bytes, start) };
                if descriptor && end >= start + 16 && bytes[end - 16..end - 12] == [0x50, 0x4B, 0x07, 0x08] {
                    end -= 16;
                }
                (bytes[start..end].to_vec(), known && start + csize > bytes.len(), end - start)
            }
            8 => inflate(&bytes[start..], budget.bytes_left().saturating_add(1) as usize),
            _ => {
                // An unknown method: leave the entry out and carry on after its data.
                pos = if csize > 0 && !descriptor { start + csize } else { next_header(bytes, start) };
                continue;
            }
        };
        if budget.spend_bytes(data.len() as u64).is_err() {
            break;
        }
        // An entry with no data is followed at once by the next header, which the search must still see.
        pos = start + used;
        if name.ends_with('/') {
            continue;
        }
        let stored = method == 0;
        // A later entry with the same name replaces an earlier one, as in a zip that was appended to.
        // (An index by name keeps this from rescanning every earlier entry, which is quadratic for a big file.)
        if let Some(old) = index.insert(name.clone(), out.entries.len()) {
            slots[old] = false;
        }
        slots.push(true);
        out.entries.push(RawEntry { name, data, stored, damaged });
    }
    let mut live = slots.into_iter();
    out.entries.retain(|_| live.next().unwrap_or(true));
    out
}

/// Where the next local or central header starts after `from`, or the end of the file.
fn next_header(bytes: &[u8], from: usize) -> usize {
    // One pass that stops at whichever signature comes first. Searching for each signature separately walks to
    // the end of the file when one of them never appears (no central directory), once per entry: quadratic.
    match bytes.get(from..) {
        Some(tail) => tail.windows(4).position(|w| w == LOCAL || w == CENTRAL).map_or(bytes.len(), |i| i + from),
        None => bytes.len(),
    }
}

/// Inflate a raw deflate stream. Returns what came out, whether the stream was cut short or broken, and how
/// many input bytes were used.
fn inflate(input: &[u8], limit: usize) -> (Vec<u8>, bool, usize) {
    let mut d = Decompress::new(false);
    let mut out = Vec::new();
    let mut buf = vec![0u8; 64 << 10];
    loop {
        let before_in = d.total_in();
        let before_out = d.total_out();
        let consumed = d.total_in() as usize;
        let r = d.decompress(&input[consumed.min(input.len())..], &mut buf, FlushDecompress::None);
        let produced = (d.total_out() - before_out) as usize;
        out.extend_from_slice(&buf[..produced]);
        match r {
            Ok(Status::StreamEnd) => return (out, false, d.total_in() as usize),
            Ok(_) => {
                if out.len() > limit {
                    return (out, true, d.total_in() as usize);
                }
                // No progress means the input ran out before the stream ended.
                if d.total_in() == before_in && produced == 0 {
                    return (out, true, d.total_in() as usize);
                }
            }
            Err(_) => return (out, true, d.total_in() as usize),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn make(parts: &[(&str, &str)]) -> Vec<u8> {
        let mut w = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for (n, d) in parts {
            w.start_file(*n, zip::write::SimpleFileOptions::default()).unwrap();
            w.write_all(d.as_bytes()).unwrap();
        }
        w.finish().unwrap().into_inner()
    }

    #[test]
    fn sound_zip_reads_every_entry() {
        let z = make(&[("a.txt", "alpha"), ("dir/b.txt", &"beta ".repeat(500))]);
        let r = read(&z, &Budget::file()).unwrap();
        assert!(!r.scanned);
        assert_eq!(r.entries.len(), 2);
        assert_eq!(r.entries[0].data, b"alpha");
        assert!(r.damaged().next().is_none());
    }

    #[test]
    fn a_missing_central_directory_is_scanned() {
        let text = "gamma ".repeat(2000);
        let z = make(&[("a.txt", "alpha"), ("b.txt", &text)]);
        // Cut the file in the middle of the central directory.
        let at = find(&z, &CENTRAL, 0).unwrap();
        let r = read(&z[..at + 10], &Budget::file()).unwrap();
        assert!(r.scanned);
        assert_eq!(r.entries.len(), 2);
        assert_eq!(r.entries[1].data, text.as_bytes());
        assert!(r.damaged().next().is_none());
    }

    #[test]
    fn a_cut_entry_keeps_what_was_read() {
        // Text that doesn't compress well, so the cut falls inside the stream.
        let mut seed = 12345u32;
        let text: String = (0..20000)
            .map(|_| {
                seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                (b'a' + (seed >> 24) as u8 % 26) as char
            })
            .collect();
        let z = make(&[("a.txt", "alpha"), ("big.txt", &text)]);
        let at = find(&z, &LOCAL, 4).unwrap();
        let r = read(&z[..at + 30 + 7 + 5000], &Budget::file()).unwrap();
        let big = r.entries.iter().find(|e| e.name == "big.txt").unwrap();
        assert!(big.damaged);
        assert!(!big.data.is_empty() && big.data.len() < text.len());
        assert!(text.as_bytes().starts_with(&big.data));
        assert_eq!(r.entries[0].data, b"alpha");
    }

    #[test]
    fn many_entries_without_a_central_directory_scan_in_linear_time() {
        // Stored entries with a data descriptor flag and no sizes, and no central directory anywhere: every entry
        // used to search the whole rest of the file for a central header.
        let n = 30_000;
        let mut z = Vec::new();
        for i in 0..n {
            z.extend_from_slice(&LOCAL);
            z.extend_from_slice(&[20, 0, 8, 0, 0, 0]); // version, flags (descriptor), method 0
            z.extend_from_slice(&[0u8; 16]); // time, date, crc, sizes
            z.extend_from_slice(&[8, 0, 0, 0]); // name length, extra length
            z.extend_from_slice(format!("e{i:07}").as_bytes());
        }
        // The same names twice at the end: later entries replace earlier ones.
        for i in 0..3 {
            z.extend_from_slice(&LOCAL);
            z.extend_from_slice(&[20, 0, 8, 0, 0, 0]);
            z.extend_from_slice(&[0u8; 16]);
            z.extend_from_slice(&[8, 0, 0, 0]);
            z.extend_from_slice(format!("e{i:07}").as_bytes());
        }
        let t = std::time::Instant::now();
        let r = read(&z, &Budget::new(crate::budget::Limits { parts: 100_000, ..crate::budget::Limits::file() })).unwrap();
        assert!(t.elapsed().as_secs() < 3, "took {:?}", t.elapsed());
        assert!(r.scanned);
        assert_eq!(r.entries.len(), n);
        // Replaced entries moved to the end, as before.
        assert_eq!(r.entries.last().unwrap().name, "e0000002");
    }

    #[test]
    fn nothing_found_in_noise() {
        assert!(read(b"not a zip at all", &Budget::file()).is_none());
        assert!(read(&[], &Budget::file()).is_none());
    }
}
