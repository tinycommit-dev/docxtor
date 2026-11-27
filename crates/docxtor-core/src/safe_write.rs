//! Writing a file without ever leaving a broken one behind.
//!
//! `std::fs::write` truncates the file and then writes it, so a crash, a killed process or a full disk part way through leaves a
//! short file where a good document was. The command line and the MCP server write over the original, so they write like this:
//!
//! 1. the bytes go to a temporary file in the same directory (the same disk, so the last step is a rename and not a copy);
//! 2. the file is flushed and synced to disk, and read back;
//! 3. the read-back bytes must be the bytes meant, and pass the caller's check (for a document: Docxtor opens it again);
//! 4. the temporary file takes the target's permissions and is renamed over the target, which is replaced in one step.
//!
//! Any failure before the rename removes the temporary file and leaves the target exactly as it was, and the error says why.
//! A target that is a symbolic link is written through: the file it points to is replaced, and the link stays.

use std::io::Write;
use std::path::{Path, PathBuf};

/// Write `bytes` to `target` as described in the module notes. `check` gets the bytes as read back from the temporary file, after
/// they were synced; an `Err` from it stops the write and is named in the error.
pub fn write_atomic(target: impl AsRef<Path>, bytes: &[u8], check: impl FnOnce(&[u8]) -> Result<(), String>) -> Result<(), String> {
    let target = target.as_ref();
    let shown = target.display();
    // Write through a link, as `std::fs::write` does: the link is not replaced by a file.
    let real: PathBuf = if target.is_symlink() { std::fs::canonicalize(target).map_err(|e| format!("{shown}: {e}"))? } else { target.to_path_buf() };
    if real.is_dir() {
        return Err(format!("{shown}: is a folder, not a file"));
    }
    let dir = match real.parent() {
        Some(p) if !p.as_os_str().is_empty() => p.to_path_buf(),
        _ => PathBuf::from("."),
    };
    let name = real.file_name().map(|n| n.to_string_lossy().into_owned()).ok_or_else(|| format!("{shown}: not a file name"))?;
    let existed = real.exists();
    let unchanged = if existed { "the original is unchanged" } else { "nothing was written" };

    let (mut file, tmp) = create_temp(&dir, &name).map_err(|e| format!("{shown}: can't write in {}: {e}; {unchanged}", dir.display()))?;
    let result = (|| -> Result<(), String> {
        file.write_all(bytes).map_err(|e| format!("writing failed: {e}"))?;
        file.flush().map_err(|e| format!("writing failed: {e}"))?;
        file.sync_all().map_err(|e| format!("writing to disk failed: {e}"))?;
        drop(file);
        let back = std::fs::read(&tmp).map_err(|e| format!("the new file could not be read back: {e}"))?;
        if back != bytes {
            return Err(format!("the new file is not what was written ({} of {} bytes)", back.len(), bytes.len()));
        }
        check(&back).map_err(|e| format!("the new file did not pass its check: {e}"))?;
        if existed {
            // Same permissions as the file it replaces (the file's user, group and mode).
            let permissions = std::fs::metadata(&real).map_err(|e| format!("{e}"))?.permissions();
            std::fs::set_permissions(&tmp, permissions).map_err(|e| format!("could not copy the file's permissions: {e}"))?;
        }
        std::fs::rename(&tmp, &real).map_err(|e| format!("could not replace the file: {e}"))?;
        Ok(())
    })();
    match result {
        Ok(()) => {
            // The rename is only kept through a power cut once the folder itself is on disk. Best effort: not every system allows it.
            #[cfg(unix)]
            if let Ok(d) = std::fs::File::open(&dir) {
                let _ = d.sync_all();
            }
            Ok(())
        }
        Err(why) => {
            let _ = std::fs::remove_file(&tmp);
            Err(format!("{shown}: {why}; {unchanged}"))
        }
    }
}

/// The check for a .docx: Docxtor opens it again, so a file that would not open is never put in place of one that does.
pub fn check_docx(bytes: &[u8]) -> Result<(), String> {
    crate::Document::open(bytes).map(|_| ()).map_err(|e| e.to_string())
}

/// Write a .docx over `target` safely, after reading it back and opening it.
pub fn write_docx(target: impl AsRef<Path>, bytes: &[u8]) -> Result<(), String> {
    write_atomic(target, bytes, check_docx)
}

/// Write a password-protected .docx. What was encrypted was opened before it was (`check_docx` on the plain bytes); here the file
/// must still be a protected one when read back.
pub fn write_protected_docx(target: impl AsRef<Path>, bytes: &[u8]) -> Result<(), String> {
    write_atomic(target, bytes, |b| if crate::office_crypt::is_encrypted(b) { Ok(()) } else { Err("it is not a protected file".into()) })
}

/// Write any other file (an export, a report) over `target` safely: through a temporary file, read back, then renamed.
pub fn write_file(target: impl AsRef<Path>, bytes: &[u8]) -> Result<(), String> {
    write_atomic(target, bytes, |_| Ok(()))
}

fn create_temp(dir: &Path, name: &str) -> std::io::Result<(std::fs::File, PathBuf)> {
    use std::sync::atomic::{AtomicU32, Ordering};
    static COUNT: AtomicU32 = AtomicU32::new(0);
    let mut last = None;
    for _ in 0..16 {
        let n = COUNT.fetch_add(1, Ordering::Relaxed);
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.subsec_nanos()).unwrap_or(0);
        let tmp = dir.join(format!(".{name}.docxtor-{}-{stamp:x}-{n}.tmp", std::process::id()));
        match std::fs::OpenOptions::new().write(true).create_new(true).open(&tmp) {
            Ok(f) => return Ok((f, tmp)),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => last = Some(e),
            Err(e) => return Err(e),
        }
    }
    Err(last.unwrap_or_else(|| std::io::Error::other("no free temporary name")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("docxtor-safe-write-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn files(dir: &Path) -> Vec<String> {
        let mut v: Vec<String> = std::fs::read_dir(dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
        v.sort();
        v
    }

    #[test]
    fn writes_through_a_temporary_file_in_the_same_folder_then_renames() {
        let dir = scratch("through");
        let target = dir.join("a.docx");
        std::fs::write(&target, b"old").unwrap();
        let seen = std::cell::RefCell::new(Vec::new());
        write_atomic(&target, b"new bytes", |back| {
            // At the moment of the check the new bytes are in a temporary file next to the target, and the target is still the old file.
            let mut now = files(&dir);
            now.sort();
            seen.borrow_mut().extend(now);
            assert_eq!(std::fs::read(&target).unwrap(), b"old");
            assert_eq!(back, b"new bytes");
            Ok(())
        })
        .unwrap();
        let seen = seen.into_inner();
        assert_eq!(seen.len(), 2, "{seen:?}");
        let tmp = seen.iter().find(|n| *n != "a.docx").unwrap();
        assert!(tmp.starts_with(".a.docx.docxtor-") && tmp.ends_with(".tmp"), "{tmp}");
        assert_eq!(std::fs::read(&target).unwrap(), b"new bytes");
        assert_eq!(files(&dir), vec!["a.docx"], "no temporary file is left");
    }

    #[test]
    fn a_new_file_is_made_the_same_way() {
        let dir = scratch("fresh");
        let target = dir.join("b.docx");
        write_file(&target, b"hello").unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), b"hello");
        assert_eq!(files(&dir), vec!["b.docx"]);
    }

    #[test]
    fn a_failed_check_leaves_the_original_byte_for_byte_and_says_why() {
        let dir = scratch("check");
        let target = dir.join("c.docx");
        let original: Vec<u8> = (0..=255u8).cycle().take(5000).collect();
        std::fs::write(&target, &original).unwrap();
        let err = write_atomic(&target, b"half a file", |_| Err("it ends too soon".into())).unwrap_err();
        assert!(err.contains("it ends too soon") && err.contains("the original is unchanged") && err.contains("c.docx"), "{err}");
        assert_eq!(std::fs::read(&target).unwrap(), original);
        assert_eq!(files(&dir), vec!["c.docx"], "the temporary file is removed");
    }

    #[test]
    fn a_docx_that_does_not_open_is_never_put_in_place_of_one_that_does() {
        let dir = scratch("docx");
        let target = dir.join("d.docx");
        let mut doc = crate::Document::new_blank();
        let good = doc.save().unwrap();
        write_docx(&target, &good).unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), good);
        // Truncated, as a full disk would leave it.
        let err = write_docx(&target, &good[..good.len() / 2]).unwrap_err();
        assert!(err.contains("did not pass its check") && err.contains("the original is unchanged"), "{err}");
        assert_eq!(std::fs::read(&target).unwrap(), good);
        assert_eq!(files(&dir), vec!["d.docx"]);
    }

    #[test]
    fn a_protected_file_must_still_be_protected_when_read_back() {
        let dir = scratch("protected");
        let target = dir.join("e.docx");
        let mut doc = crate::Document::new_blank();
        let plain = doc.save().unwrap();
        let err = write_protected_docx(&target, &plain).unwrap_err();
        assert!(err.contains("not a protected file"), "{err}");
        assert!(!target.exists(), "nothing was written");
        let enc = crate::office_crypt::encrypt(&plain, "pw", &mut |b: &mut [u8]| b.iter_mut().enumerate().for_each(|(i, x)| *x = i as u8));
        write_protected_docx(&target, &enc).unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), enc);
    }

    #[cfg(unix)]
    #[test]
    fn the_targets_permissions_are_kept() {
        use std::os::unix::fs::PermissionsExt;
        let dir = scratch("mode");
        let target = dir.join("f.docx");
        std::fs::write(&target, b"old").unwrap();
        std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o640)).unwrap();
        write_file(&target, b"new").unwrap();
        assert_eq!(std::fs::metadata(&target).unwrap().permissions().mode() & 0o777, 0o640);
    }

    #[cfg(unix)]
    #[test]
    fn a_symbolic_link_stays_a_link_and_its_file_is_replaced() {
        let dir = scratch("link");
        let real = dir.join("real.docx");
        let link = dir.join("link.docx");
        std::fs::write(&real, b"old").unwrap();
        std::os::unix::fs::symlink(&real, &link).unwrap();
        write_file(&link, b"new").unwrap();
        assert!(link.is_symlink());
        assert_eq!(std::fs::read(&real).unwrap(), b"new");
    }

    #[cfg(unix)]
    #[test]
    fn a_folder_that_cannot_be_written_gives_a_clear_error_and_leaves_the_original() {
        use std::os::unix::fs::PermissionsExt;
        let dir = scratch("readonly");
        let target = dir.join("g.docx");
        std::fs::write(&target, b"precious").unwrap();
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o555)).unwrap();
        // A process that ignores permissions (root) can't show this.
        let blocked = std::fs::File::create(dir.join("probe")).is_err();
        let result = write_file(&target, b"new");
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
        if !blocked {
            return;
        }
        let err = result.unwrap_err();
        assert!(err.contains("g.docx") && err.contains("can't write in") && err.contains("the original is unchanged"), "{err}");
        assert_eq!(std::fs::read(&target).unwrap(), b"precious");
        assert_eq!(files(&dir), vec!["g.docx"]);
    }

    #[test]
    fn a_folder_is_not_replaced() {
        let dir = scratch("isdir");
        let target = dir.join("sub");
        std::fs::create_dir(&target).unwrap();
        let err = write_file(&target, b"x").unwrap_err();
        assert!(err.contains("is a folder"), "{err}");
        assert!(target.is_dir());
    }
}
