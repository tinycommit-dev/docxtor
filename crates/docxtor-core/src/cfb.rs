//! Compound File Binary (the OLE container, MS-CFB): a small reader and writer.
//!
//! Word keeps a password-protected .docx in one of these: a few named streams (`EncryptionInfo`,
//! `EncryptedPackage`, and the data space streams) instead of a zip. The reader takes streams out by path
//! (`"\u{6}DataSpaces/Version"`); the writer builds a version 3 file (512-byte sectors) from paths and bytes.

use std::collections::BTreeMap;

pub const MAGIC: [u8; 8] = [0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];

const FREE: u32 = 0xFFFF_FFFF;
const END: u32 = 0xFFFF_FFFE;
const FATSECT: u32 = 0xFFFF_FFFD;
const DIFSECT: u32 = 0xFFFF_FFFC;
const NOSTREAM: u32 = 0xFFFF_FFFF;
const MINI_CUTOFF: usize = 4096;
const MAX_REG: u32 = 0xFFFF_FFFA;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CfbError {
    #[error("not a compound file")]
    NotCfb,
    #[error("the compound file is damaged: {0}")]
    Corrupt(&'static str),
}

/// Whether the bytes start with the compound file signature.
pub fn is_cfb(bytes: &[u8]) -> bool {
    bytes.len() >= 8 && bytes[..8] == MAGIC
}

fn u16_at(b: &[u8], i: usize) -> u16 {
    u16::from_le_bytes([b[i], b[i + 1]])
}
fn u32_at(b: &[u8], i: usize) -> u32 {
    u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]])
}
fn u64_at(b: &[u8], i: usize) -> u64 {
    let mut a = [0u8; 8];
    a.copy_from_slice(&b[i..i + 8]);
    u64::from_le_bytes(a)
}

#[derive(Debug, Clone)]
struct Entry {
    name: String,
    kind: u8, // 1 storage, 2 stream, 5 root
    left: u32,
    right: u32,
    child: u32,
    start: u32,
    size: u64,
}

/// A parsed compound file: its streams can be read by path.
pub struct CfbReader<'a> {
    data: &'a [u8],
    sector_size: usize,
    mini_size: usize,
    /// How many sectors the file really has; bounds every loop that follows the file's own numbers.
    max_sectors: usize,
    fat: Vec<u32>,
    mini_fat: Vec<u32>,
    entries: Vec<Entry>,
    mini_stream: Vec<u8>,
}

impl<'a> CfbReader<'a> {
    pub fn new(data: &'a [u8]) -> Result<Self, CfbError> {
        if !is_cfb(data) {
            return Err(CfbError::NotCfb);
        }
        if data.len() < 512 {
            return Err(CfbError::Corrupt("the header is cut short"));
        }
        let major = u16_at(data, 26);
        let sector_shift = u16_at(data, 30);
        let mini_shift = u16_at(data, 32);
        if !(major == 3 && sector_shift == 9 || major == 4 && sector_shift == 12) || mini_shift != 6 {
            return Err(CfbError::Corrupt("unknown sector size"));
        }
        let sector_size = 1usize << sector_shift;
        let mini_size = 1usize << mini_shift;
        let n_fat = u32_at(data, 44) as usize;
        let first_dir = u32_at(data, 48);
        let first_minifat = u32_at(data, 60);
        let n_minifat = u32_at(data, 64) as usize;
        let first_difat = u32_at(data, 68);
        let n_difat = u32_at(data, 72) as usize;

        // A chain, a FAT or a DIFAT list cannot name more sectors than the file holds, whatever its header says.
        let max_sectors = (data.len() / sector_size).saturating_sub(1);
        let mut r = CfbReader { data, sector_size, mini_size, max_sectors, fat: Vec::new(), mini_fat: Vec::new(), entries: Vec::new(), mini_stream: Vec::new() };

        // The FAT sector numbers: 109 in the header, the rest in DIFAT sectors.
        let mut fat_sectors: Vec<u32> = Vec::new();
        for i in 0..109 {
            let s = u32_at(data, 76 + i * 4);
            if s <= MAX_REG {
                fat_sectors.push(s);
            }
        }
        let per = sector_size / 4 - 1;
        let mut d = first_difat;
        let mut seen = 0;
        while d <= MAX_REG {
            if seen >= n_difat.max(1).saturating_add(4).min(r.max_sectors.max(1)) {
                return Err(CfbError::Corrupt("the DIFAT chain is too long"));
            }
            let sec = r.sector(d).ok_or(CfbError::Corrupt("a DIFAT sector is missing"))?;
            for i in 0..per {
                let s = u32_at(sec, i * 4);
                if s <= MAX_REG {
                    fat_sectors.push(s);
                }
            }
            d = u32_at(sec, per * 4);
            seen += 1;
        }
        fat_sectors.truncate(n_fat);
        if fat_sectors.len() > r.max_sectors {
            return Err(CfbError::Corrupt("the FAT names more sectors than the file has"));
        }
        for s in &fat_sectors {
            let sec = r.sector(*s).ok_or(CfbError::Corrupt("a FAT sector is missing"))?;
            for i in 0..sector_size / 4 {
                r.fat.push(u32_at(sec, i * 4));
            }
        }
        if r.fat.is_empty() {
            return Err(CfbError::Corrupt("no FAT"));
        }

        // Directory.
        let dir = r.chain_bytes(first_dir, None)?;
        for chunk in dir.chunks_exact(128) {
            let name_len = (u16_at(chunk, 64) as usize).min(64);
            let units: Vec<u16> = (0..name_len / 2).map(|i| u16_at(chunk, i * 2)).take_while(|u| *u != 0).collect();
            r.entries.push(Entry {
                name: String::from_utf16_lossy(&units),
                kind: chunk[66],
                left: u32_at(chunk, 68),
                right: u32_at(chunk, 72),
                child: u32_at(chunk, 76),
                start: u32_at(chunk, 116),
                size: if major == 3 { u32_at(chunk, 120) as u64 } else { u64_at(chunk, 120) },
            });
        }
        if r.entries.first().map(|e| e.kind) != Some(5) {
            return Err(CfbError::Corrupt("no root entry"));
        }

        // Mini FAT and the mini stream.
        if n_minifat > 0 && first_minifat <= MAX_REG {
            let mf = r.chain_bytes(first_minifat, None)?;
            r.mini_fat = mf.chunks_exact(4).map(|c| u32_at(c, 0)).collect();
        }
        let root = &r.entries[0];
        if root.size > 0 && root.start <= MAX_REG {
            let (start, size) = (root.start, root.size as usize);
            r.mini_stream = r.chain_bytes(start, Some(size))?;
        }
        Ok(r)
    }

    fn sector(&self, n: u32) -> Option<&'a [u8]> {
        let start = (n as usize).checked_add(1)?.checked_mul(self.sector_size)?;
        self.data.get(start..start.checked_add(self.sector_size)?)
    }

    /// The bytes of a chain of regular sectors, cut to `size` when given.
    fn chain_bytes(&self, start: u32, size: Option<usize>) -> Result<Vec<u8>, CfbError> {
        let mut out = Vec::new();
        let mut s = start;
        let mut steps = 0usize;
        while s <= MAX_REG {
            if steps >= self.max_sectors.min(self.fat.len()) {
                return Err(CfbError::Corrupt("a sector chain loops"));
            }
            out.extend_from_slice(self.sector(s).ok_or(CfbError::Corrupt("a sector is past the end of the file"))?);
            s = *self.fat.get(s as usize).ok_or(CfbError::Corrupt("a sector has no FAT entry"))?;
            steps += 1;
            if let Some(n) = size {
                if out.len() >= n {
                    break;
                }
            }
        }
        if let Some(n) = size {
            if out.len() < n {
                return Err(CfbError::Corrupt("a stream is shorter than it says"));
            }
            out.truncate(n);
        }
        Ok(out)
    }

    fn mini_bytes(&self, start: u32, size: usize) -> Result<Vec<u8>, CfbError> {
        let mut out = Vec::with_capacity(size);
        let mut s = start;
        let mut steps = 0usize;
        while s <= MAX_REG && out.len() < size {
            if steps > self.mini_fat.len() {
                return Err(CfbError::Corrupt("a mini sector chain loops"));
            }
            let a = (s as usize).checked_mul(self.mini_size).ok_or(CfbError::Corrupt("a mini sector is missing"))?;
            let end = a.checked_add(self.mini_size).ok_or(CfbError::Corrupt("a mini sector is missing"))?;
            out.extend_from_slice(self.mini_stream.get(a..end).ok_or(CfbError::Corrupt("a mini sector is missing"))?);
            s = *self.mini_fat.get(s as usize).ok_or(CfbError::Corrupt("a mini sector has no entry"))?;
            steps += 1;
        }
        if out.len() < size {
            return Err(CfbError::Corrupt("a stream is shorter than it says"));
        }
        out.truncate(size);
        Ok(out)
    }

    /// Find a child of directory `dir` by name (case-insensitive, as the format compares names).
    fn find_child(&self, dir: usize, name: &str) -> Option<usize> {
        let want = name.to_uppercase();
        let mut stack = vec![self.entries.get(dir)?.child];
        let mut visited = 0;
        while let Some(i) = stack.pop() {
            if i == NOSTREAM || visited > self.entries.len() {
                continue;
            }
            visited += 1;
            let e = self.entries.get(i as usize)?;
            if e.kind != 0 && e.name.to_uppercase() == want {
                return Some(i as usize);
            }
            stack.push(e.left);
            stack.push(e.right);
        }
        None
    }

    fn find(&self, path: &str) -> Option<usize> {
        let mut at = 0;
        for part in path.split('/') {
            at = self.find_child(at, part)?;
        }
        Some(at)
    }

    /// The bytes of the stream at `path` (storages separated by `/`), or None when there is no such stream.
    pub fn stream(&self, path: &str) -> Result<Option<Vec<u8>>, CfbError> {
        let Some(i) = self.find(path) else { return Ok(None) };
        let e = &self.entries[i];
        if e.kind != 2 {
            return Ok(None);
        }
        let size = e.size as usize;
        if size == 0 {
            return Ok(Some(Vec::new()));
        }
        if size < MINI_CUTOFF {
            self.mini_bytes(e.start, size).map(Some)
        } else {
            self.chain_bytes(e.start, Some(size)).map(Some)
        }
    }

    /// The paths of every stream in the file.
    pub fn stream_paths(&self) -> Vec<String> {
        let mut out = Vec::new();
        self.walk(0, "", &mut out, 0);
        out
    }

    fn walk(&self, dir: usize, prefix: &str, out: &mut Vec<String>, depth: usize) {
        if depth > 16 {
            return;
        }
        let mut stack = vec![self.entries[dir].child];
        let mut visited = 0;
        while let Some(i) = stack.pop() {
            if i == NOSTREAM || visited > self.entries.len() {
                continue;
            }
            visited += 1;
            let Some(e) = self.entries.get(i as usize) else { continue };
            let path = if prefix.is_empty() { e.name.clone() } else { format!("{prefix}/{}", e.name) };
            match e.kind {
                2 => out.push(path),
                1 => self.walk(i as usize, &path, out, depth + 1),
                _ => {}
            }
            stack.push(e.left);
            stack.push(e.right);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Writer

#[derive(Default)]
struct Dir {
    streams: BTreeMap<String, Vec<u8>>,
    dirs: BTreeMap<String, Dir>,
}

struct OutEntry {
    name: String,
    kind: u8,
    left: u32,
    right: u32,
    child: u32,
    red: bool,
    start: u32,
    size: u64,
    data: Vec<u8>,
}

fn name_order(a: &str, b: &str) -> std::cmp::Ordering {
    let (ua, ub): (Vec<u16>, Vec<u16>) = (a.to_uppercase().encode_utf16().collect(), b.to_uppercase().encode_utf16().collect());
    ua.len().cmp(&ub.len()).then_with(|| ua.cmp(&ub))
}

/// Entries `ids` (already sorted) as a balanced search tree; returns its root.
fn build_tree(entries: &mut [OutEntry], ids: &[usize]) -> u32 {
    if ids.is_empty() {
        return NOSTREAM;
    }
    let mid = ids.len() / 2;
    let id = ids[mid];
    let left = build_tree(entries, &ids[..mid]);
    let right = build_tree(entries, &ids[mid + 1..]);
    entries[id].left = left;
    entries[id].right = right;
    id as u32
}

fn tree_depth(n: usize) -> usize {
    // Levels of a tree with n nodes, balanced as build_tree splits it.
    let mut d = 0;
    let mut m = n;
    while m > 0 {
        d += 1;
        m /= 2;
    }
    d
}

fn add_dir(dir: &Dir, entries: &mut Vec<OutEntry>, parent: usize) {
    let mut kids: Vec<(String, bool)> = dir.streams.keys().map(|k| (k.clone(), false)).chain(dir.dirs.keys().map(|k| (k.clone(), true))).collect();
    kids.sort_by(|a, b| name_order(&a.0, &b.0));
    let mut ids = Vec::new();
    for (name, is_dir) in &kids {
        let idx = entries.len();
        let data = if *is_dir { Vec::new() } else { dir.streams[name].clone() };
        entries.push(OutEntry { name: name.clone(), kind: if *is_dir { 1 } else { 2 }, left: NOSTREAM, right: NOSTREAM, child: NOSTREAM, red: false, start: END, size: data.len() as u64, data });
        ids.push(idx);
        if *is_dir {
            add_dir(&dir.dirs[name], entries, idx);
        }
    }
    let depth = tree_depth(ids.len());
    let root = build_tree(entries, &ids);
    entries[parent].child = root;
    // Colour: a complete tree is all black; otherwise the nodes on the deepest level are red.
    if ids.len() + 1 != 1 << depth {
        let mut level = vec![(root, 1usize)];
        while let Some((i, d)) = level.pop() {
            if i == NOSTREAM {
                continue;
            }
            let (l, r) = (entries[i as usize].left, entries[i as usize].right);
            if d == depth {
                entries[i as usize].red = true;
            }
            level.push((l, d + 1));
            level.push((r, d + 1));
        }
    }
}

fn sectors_for(len: usize, size: usize) -> usize {
    len.div_ceil(size)
}

/// Build a compound file (version 3) from streams. A path may name storages with `/`
/// (`"\u{6}DataSpaces/DataSpaceInfo/StrongEncryptionDataSpace"`); storages are created as needed.
pub fn write(streams: &[(&str, Vec<u8>)]) -> Vec<u8> {
    let mut root = Dir::default();
    for (path, data) in streams {
        let parts: Vec<&str> = path.split('/').collect();
        let mut d = &mut root;
        for p in &parts[..parts.len() - 1] {
            d = d.dirs.entry((*p).to_string()).or_default();
        }
        d.streams.insert(parts[parts.len() - 1].to_string(), data.clone());
    }
    let mut entries = vec![OutEntry { name: "Root Entry".into(), kind: 5, left: NOSTREAM, right: NOSTREAM, child: NOSTREAM, red: false, start: END, size: 0, data: Vec::new() }];
    add_dir(&root, &mut entries, 0);

    // Small streams go in the mini stream.
    let mut mini_stream: Vec<u8> = Vec::new();
    let mut mini_fat: Vec<u32> = Vec::new();
    for e in entries.iter_mut().filter(|e| e.kind == 2 && !e.data.is_empty() && e.data.len() < MINI_CUTOFF) {
        let n = sectors_for(e.data.len(), 64);
        e.start = mini_fat.len() as u32;
        for i in 0..n {
            mini_fat.push(if i + 1 == n { END } else { (mini_fat.len() + 1) as u32 });
        }
        mini_stream.extend_from_slice(&e.data);
        mini_stream.resize(mini_stream.len().next_multiple_of(64), 0);
    }
    entries[0].size = mini_stream.len() as u64;

    // Sector counts: directory, mini FAT, mini stream, big streams; then FAT and DIFAT sectors for all of it.
    let dir_sectors = sectors_for(entries.len(), 4);
    let minifat_sectors = sectors_for(mini_fat.len() * 4, 512);
    let ministream_sectors = sectors_for(mini_stream.len(), 512);
    let big: Vec<usize> = (0..entries.len()).filter(|&i| entries[i].kind == 2 && entries[i].data.len() >= MINI_CUTOFF).collect();
    let big_sectors: usize = big.iter().map(|&i| sectors_for(entries[i].data.len(), 512)).sum();
    let base = dir_sectors + minifat_sectors + ministream_sectors + big_sectors;
    let (mut n_fat, mut n_difat) = (1usize, 0usize);
    loop {
        let need_fat = (base + n_fat + n_difat).div_ceil(128);
        let need_difat = need_fat.saturating_sub(109).div_ceil(127);
        if need_fat == n_fat && need_difat == n_difat {
            break;
        }
        n_fat = need_fat;
        n_difat = need_difat;
    }

    let total = base + n_fat + n_difat;
    let mut fat = vec![FREE; n_fat * 128];
    let mut body: Vec<u8> = Vec::with_capacity(total * 512);
    let mut next = 0u32;
    fn chain(fat: &mut [u32], next: &mut u32, count: usize) -> u32 {
        let first = *next;
        for i in 0..count {
            fat[(*next as usize) + i] = if i + 1 == count { END } else { *next + i as u32 + 1 };
        }
        *next += count as u32;
        if count == 0 {
            END
        } else {
            first
        }
    }

    let first_dir = chain(&mut fat, &mut next, dir_sectors);
    let first_minifat = chain(&mut fat, &mut next, minifat_sectors);
    let first_ministream = chain(&mut fat, &mut next, ministream_sectors);
    entries[0].start = if ministream_sectors == 0 { END } else { first_ministream };
    let mut big_starts = Vec::new();
    for &i in &big {
        big_starts.push((i, chain(&mut fat, &mut next, sectors_for(entries[i].data.len(), 512))));
    }
    for (i, s) in big_starts {
        entries[i].start = s;
    }
    let fat_first = next;
    for i in 0..n_fat {
        fat[(next as usize) + i] = FATSECT;
    }
    next += n_fat as u32;
    let difat_first = next;
    for i in 0..n_difat {
        fat[(next as usize) + i] = DIFSECT;
    }

    // Directory stream.
    let mut dir_bytes = Vec::with_capacity(dir_sectors * 512);
    for e in &entries {
        let units: Vec<u16> = e.name.encode_utf16().collect();
        let mut rec = [0u8; 128];
        for (i, u) in units.iter().take(31).enumerate() {
            rec[i * 2..i * 2 + 2].copy_from_slice(&u.to_le_bytes());
        }
        rec[64..66].copy_from_slice(&(((units.len().min(31) + 1) * 2) as u16).to_le_bytes());
        rec[66] = e.kind;
        rec[67] = if e.red { 0 } else { 1 };
        rec[68..72].copy_from_slice(&e.left.to_le_bytes());
        rec[72..76].copy_from_slice(&e.right.to_le_bytes());
        rec[76..80].copy_from_slice(&e.child.to_le_bytes());
        rec[116..120].copy_from_slice(&e.start.to_le_bytes());
        rec[120..124].copy_from_slice(&(e.size as u32).to_le_bytes());
        dir_bytes.extend_from_slice(&rec);
    }
    // Unused slots in the last directory sector.
    while dir_bytes.len() < dir_sectors * 512 {
        let mut rec = [0u8; 128];
        rec[68..80].copy_from_slice(&[0xFF; 12]);
        dir_bytes.extend_from_slice(&rec);
    }
    body.extend_from_slice(&dir_bytes);

    let mut mf = Vec::with_capacity(minifat_sectors * 512);
    for v in &mini_fat {
        mf.extend_from_slice(&v.to_le_bytes());
    }
    mf.resize(minifat_sectors * 512, 0xFF);
    body.extend_from_slice(&mf);

    let mut ms = mini_stream;
    ms.resize(ministream_sectors * 512, 0);
    body.extend_from_slice(&ms);

    for &i in &big {
        let mut d = entries[i].data.clone();
        d.resize(sectors_for(d.len(), 512) * 512, 0);
        body.extend_from_slice(&d);
    }
    for v in &fat {
        body.extend_from_slice(&v.to_le_bytes());
    }
    body.truncate((base + n_fat) * 512);

    // DIFAT sectors: the FAT sector numbers after the first 109.
    let fat_numbers: Vec<u32> = (0..n_fat as u32).map(|i| fat_first + i).collect();
    for d in 0..n_difat {
        let mut sec = vec![0xFFu8; 512];
        for k in 0..127 {
            if let Some(n) = fat_numbers.get(109 + d * 127 + k) {
                sec[k * 4..k * 4 + 4].copy_from_slice(&n.to_le_bytes());
            }
        }
        let link = if d + 1 == n_difat { END } else { difat_first + d as u32 + 1 };
        sec[508..512].copy_from_slice(&link.to_le_bytes());
        body.extend_from_slice(&sec);
    }

    let mut header = vec![0u8; 512];
    header[..8].copy_from_slice(&MAGIC);
    header[24..26].copy_from_slice(&0x003Eu16.to_le_bytes());
    header[26..28].copy_from_slice(&3u16.to_le_bytes());
    header[28..30].copy_from_slice(&0xFFFEu16.to_le_bytes());
    header[30..32].copy_from_slice(&9u16.to_le_bytes());
    header[32..34].copy_from_slice(&6u16.to_le_bytes());
    header[44..48].copy_from_slice(&(n_fat as u32).to_le_bytes());
    header[48..52].copy_from_slice(&first_dir.to_le_bytes());
    header[56..60].copy_from_slice(&(MINI_CUTOFF as u32).to_le_bytes());
    header[60..64].copy_from_slice(&(if minifat_sectors == 0 { END } else { first_minifat }).to_le_bytes());
    header[64..68].copy_from_slice(&(minifat_sectors as u32).to_le_bytes());
    header[68..72].copy_from_slice(&(if n_difat == 0 { END } else { difat_first }).to_le_bytes());
    header[72..76].copy_from_slice(&(n_difat as u32).to_le_bytes());
    for i in 0..109 {
        let v = fat_numbers.get(i).copied().unwrap_or(FREE);
        header[76 + i * 4..80 + i * 4].copy_from_slice(&v.to_le_bytes());
    }
    header.extend_from_slice(&body);
    header
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pattern(n: usize, seed: u8) -> Vec<u8> {
        (0..n).map(|i| (i as u8).wrapping_mul(31).wrapping_add(seed)).collect()
    }

    /// A header with the given DIFAT fields and `sectors` zeroed sectors after it.
    fn crafted(major: u16, n_fat: u32, first_dir: u32, first_difat: u32, n_difat: u32, sectors: usize) -> Vec<u8> {
        let shift: u16 = if major == 3 { 9 } else { 12 };
        let ss = 1usize << shift;
        let mut h = vec![0u8; ss + sectors * ss];
        h[..8].copy_from_slice(&MAGIC);
        h[26..28].copy_from_slice(&major.to_le_bytes());
        h[30..32].copy_from_slice(&shift.to_le_bytes());
        h[32..34].copy_from_slice(&6u16.to_le_bytes());
        h[44..48].copy_from_slice(&n_fat.to_le_bytes());
        h[48..52].copy_from_slice(&first_dir.to_le_bytes());
        h[60..64].copy_from_slice(&FREE.to_le_bytes());
        h[68..72].copy_from_slice(&first_difat.to_le_bytes());
        h[72..76].copy_from_slice(&n_difat.to_le_bytes());
        for i in 0..109 {
            h[76 + i * 4..80 + i * 4].copy_from_slice(&FREE.to_le_bytes());
        }
        h
    }

    #[test]
    fn a_huge_difat_count_on_a_tiny_file_fails_fast() {
        // One zeroed sector whose "next DIFAT" link points back at sector 0, with 4 billion DIFAT sectors claimed.
        let f = crafted(3, 1, 0, 0, 0xFFFF_FFFF, 1);
        let t = std::time::Instant::now();
        assert!(CfbReader::new(&f).is_err());
        assert!(t.elapsed().as_millis() < 500);
    }

    #[test]
    fn a_looping_chain_does_not_copy_sector_zero_thousands_of_times() {
        // Every FAT slot in the header names sector 0, so the FAT is 109 x entries of zeros: the directory chain
        // 0 -> 0 -> 0 loops. The walk must stop after the sectors the file has, not after the FAT's length.
        for major in [3u16, 4] {
            let mut f = crafted(major, 109, 0, FREE, 0, 1);
            for i in 0..109 {
                f[76 + i * 4..80 + i * 4].copy_from_slice(&0u32.to_le_bytes());
            }
            let t = std::time::Instant::now();
            assert!(CfbReader::new(&f).is_err());
            assert!(t.elapsed().as_millis() < 500);
        }
    }

    #[test]
    fn round_trips_small_and_large_streams() {
        let a = pattern(10, 1);
        let b = pattern(63, 2);
        let c = pattern(64, 3);
        let d = pattern(4095, 4);
        let e = pattern(4096, 5);
        let f = pattern(100_000, 6);
        let streams: Vec<(&str, Vec<u8>)> =
            vec![("A", a.clone()), ("Dir/B", b.clone()), ("Dir/Sub/C", c.clone()), ("D", d.clone()), ("E", e.clone()), ("\u{6}Big", f.clone()), ("Empty", vec![])];
        let file = write(&streams);
        assert!(is_cfb(&file));
        assert_eq!(file.len() % 512, 0);
        let r = CfbReader::new(&file).unwrap();
        assert_eq!(r.stream("A").unwrap().unwrap(), a);
        assert_eq!(r.stream("Dir/B").unwrap().unwrap(), b);
        assert_eq!(r.stream("Dir/Sub/C").unwrap().unwrap(), c);
        assert_eq!(r.stream("D").unwrap().unwrap(), d);
        assert_eq!(r.stream("E").unwrap().unwrap(), e);
        assert_eq!(r.stream("\u{6}Big").unwrap().unwrap(), f);
        assert_eq!(r.stream("Empty").unwrap().unwrap(), Vec::<u8>::new());
        assert!(r.stream("Nope").unwrap().is_none());
        assert!(r.stream("Dir").unwrap().is_none());
        assert_eq!(r.stream_paths().len(), 7);
    }

    #[test]
    fn names_are_found_without_regard_to_case() {
        let file = write(&[("EncryptionInfo", vec![1, 2, 3])]);
        let r = CfbReader::new(&file).unwrap();
        assert_eq!(r.stream("encryptioninfo").unwrap().unwrap(), vec![1, 2, 3]);
    }

    #[test]
    fn a_file_needing_difat_sectors_round_trips() {
        // More than 109 FAT sectors: about 7 MB.
        let big = pattern(8_000_000, 9);
        let file = write(&[("Big", big.clone()), ("Small", vec![7; 5])]);
        let n_difat = u32_at(&file, 72);
        assert!(n_difat >= 1);
        let r = CfbReader::new(&file).unwrap();
        assert_eq!(r.stream("Big").unwrap().unwrap(), big);
        assert_eq!(r.stream("Small").unwrap().unwrap(), vec![7; 5]);
    }

    #[test]
    fn many_entries_form_a_search_tree() {
        let names: Vec<String> = (0..40).map(|i| format!("Stream{i}")).collect();
        let streams: Vec<(&str, Vec<u8>)> = names.iter().map(|n| (n.as_str(), n.as_bytes().to_vec())).collect();
        let file = write(&streams);
        let r = CfbReader::new(&file).unwrap();
        for n in &names {
            assert_eq!(r.stream(n).unwrap().unwrap(), n.as_bytes());
        }
    }

    #[test]
    fn rejects_things_that_are_not_compound_files() {
        assert_eq!(CfbReader::new(b"PK\x03\x04 not a compound file").err(), Some(CfbError::NotCfb));
        let mut cut = MAGIC.to_vec();
        cut.extend_from_slice(&[0; 100]);
        assert!(matches!(CfbReader::new(&cut), Err(CfbError::Corrupt(_))));
    }
}
