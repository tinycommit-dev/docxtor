//! `docxtor` — inspect, convert and edit .docx files from the command line.

use docxtor_core::office_crypt::{self, CryptError, CryptSession};
use docxtor_core::{ai, fidelity, package::Package, safe_write, Document};
use std::cell::RefCell;
use std::io::Read;
use std::process::ExitCode;
use std::sync::OnceLock;

const HELP: &str = "docxtor — open-source DOCX engine

USAGE:
  docxtor read <file.docx> [--start pN] [--limit N]   Print the document as DocScript (AI view)
  docxtor outline <file.docx>                         Headings with ids
  docxtor text <file.docx>                            Plain text
  docxtor markdown <file.docx>                        Markdown (GitHub-flavoured, with footnotes)
  docxtor export <file.docx> --format html|epub|odt|rtf [-o out]
                                                    Web page, EPUB book, OpenDocument text or Rich Text (html and rtf go
                                                    to the screen without -o; the others are written next to the file)
  docxtor json <file.docx>                            Rendered view model (JSON)
  docxtor info <file.docx>                            Summary + lossless round-trip check
  docxtor roundtrip <in.docx> <out.docx>              Open and re-save, then report fidelity
  docxtor fidelity <original.docx> <other.docx>       Compare two files (Docxtor fidelity score)
  docxtor apply <file.docx> <ops.json|-> [-o out]     Apply Docxtor ops (JSON object or array)
  docxtor tool <file.docx> <tool> <args-json> [-o out]  Run an AI tool (see `docxtor tools`)
  docxtor tools                                       Print AI tool definitions (JSON)
  docxtor new <out.docx> [--markdown <file.md|->]     Create a document (optionally from Markdown)
  docxtor convert <file> [out.docx]                   Make a .docx from an .odt, .rtf, .html, .doc, Word 2003 XML or Flat OPC file,
                                                    or repair a damaged .docx (says what was fixed or lost)
  docxtor corpus <dir> [--json] [--min <score>]       Score every .docx in a folder (CI fidelity gate)
  docxtor diff <old.docx> <new.docx> [--json]         Paragraph-level changes between two versions
  docxtor check <file.docx> [--json]                  Accessibility problems (alt text, headings, tables, links, contrast)
  docxtor compare <old.docx> <new.docx> -o <redline.docx> [--author name] [--date iso]
                                                    The new document with tracked changes against the old one
  docxtor combine <original.docx> <revised.docx> -o <out.docx> [--author name] [--date iso]
                                                    The revised document with what differs from the original marked as tracked
                                                    changes by --author; tracked changes it already has keep their authors
  docxtor inspect <file.docx> [--json] [--remove all|kind,kind] [-o out]
                                                    What the file carries besides the page (names, comments, tracked changes,
                                                    hidden text, custom data); --remove takes the chosen kinds out
  docxtor count <file.docx> [--extras] [--json]       Paragraphs, words and characters (--extras adds notes and text boxes)
  docxtor sources <file.docx> [--json|--bibtex]       The document's sources and citation style (Word's Manage Sources)
  docxtor cite-import <file.docx> <refs.bib|refs.json> [-o out]
                                                    Add the sources in a BibTeX or CSL-JSON file (skipping ones already there)
  docxtor encrypt <in.docx> <out.docx> --password <pw>  Protect a document with a password (Word's Agile Encryption, AES-256)
  docxtor decrypt <in.docx> <out.docx> --password <pw>  Remove the password from a protected document
  docxtor merge <template.docx> <recipients.csv|.xlsx> -o <out.docx> [--sheet name] [--type letters|labels|envelopes|directory] [--from n --to n]
                                                    Mail merge: the template (with merge fields) and a list of recipients become a new
                                                    document with a section for each record (without -o, merged.docx next to the template)

Without -o, edits are written back to the input file.

Password-protected documents: every command that opens a document takes --password <pw> (or the DOCXTOR_PASSWORD
environment variable, which keeps it out of the process list). A document opened with a password is saved
protected again with the same password. `corpus` skips protected files unless it is given a password.";

static PASSWORD: OnceLock<Option<String>> = OnceLock::new();

thread_local! {
    /// The protected document opened last, so that saving writes it protected again.
    static SESSION: RefCell<Option<CryptSession>> = const { RefCell::new(None) };
}

/// Take `--password <pw>` out of the arguments (so it can sit anywhere) and fall back to `DOCXTOR_PASSWORD`.
fn take_password(args: Vec<String>) -> Vec<String> {
    let mut out = Vec::with_capacity(args.len());
    let mut password = None;
    let mut it = args.into_iter();
    while let Some(a) = it.next() {
        if a == "--password" {
            password = it.next();
        } else if let Some(v) = a.strip_prefix("--password=") {
            password = Some(v.to_string());
        } else {
            out.push(a);
        }
    }
    let _ = PASSWORD.set(password.or_else(|| std::env::var("DOCXTOR_PASSWORD").ok().filter(|p| !p.is_empty())));
    out
}

fn password() -> Option<&'static str> {
    PASSWORD.get().and_then(|p| p.as_deref())
}

fn rng(buf: &mut [u8]) {
    office_crypt::os_random(buf);
}

/// The .docx bytes in a file. A password-protected file is decrypted with the password given, and the
/// session is kept so that saving protects it again.
fn read_docx(path: &str) -> Result<Vec<u8>, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
    if !office_crypt::is_encrypted(&bytes) {
        return Ok(bytes);
    }
    let Some(pw) = password() else {
        return Err(format!("{path} is protected with a password; give it with --password or DOCXTOR_PASSWORD"));
    };
    let (plain, session) = office_crypt::open(&bytes, pw).map_err(|e| match e {
        CryptError::WrongPassword => format!("{path}: that password isn't right"),
        e => format!("{path}: {e}"),
    })?;
    SESSION.with(|s| *s.borrow_mut() = Some(session));
    Ok(plain)
}

/// Write .docx bytes, protected when the document was opened protected.
fn write_docx(path: &str, bytes: Vec<u8>) -> Result<(), String> {
    let protected = SESSION.with(|s| s.borrow().is_some());
    if protected {
        office_crypt::try_os_random(&mut [0u8; 1])?;
    }
    // Never over the original in place: a temporary file, read back and opened again, then renamed (see docxtor_core::safe_write).
    if !protected {
        return safe_write::write_docx(path, &bytes);
    }
    safe_write::check_docx(&bytes).map_err(|e| format!("{path}: the new file did not pass its check: {e}; the original is unchanged"))?;
    let out = SESSION.with(|s| s.borrow_mut().as_mut().map(|session| session.encrypt(&bytes, &mut rng))).unwrap_or(bytes);
    safe_write::write_protected_docx(path, &out)
}

fn main() -> ExitCode {
    let args = take_password(std::env::args().skip(1).collect());
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn flag(args: &[String], name: &str) -> Option<String> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned()
}

/// The package of the file as it was given, for comparing a save with it. A damaged file does not open as a package:
/// the repaired package (what Docxtor reads) stands in for it.
fn original_package(bytes: &[u8]) -> Result<Package, String> {
    match Package::open(bytes) {
        Ok(p) => Ok(p),
        Err(_) => docxtor_core::open::open_package(bytes, &docxtor_core::budget::Budget::file()).map(|(p, _)| p).map_err(|e| e.message),
    }
}

fn open(path: &str) -> Result<Document, String> {
    let bytes = read_docx(path)?;
    Document::open(&bytes).map_err(|e| format!("{path}: {e}"))
}

/// The current time in UTC as ISO 8601 (`2026-10-12T09:30:00Z`).
fn iso_now() -> String {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0) as i64;
    let (days, rem) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    // Days since 1970-01-01 to a civil date (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z", rem / 3600, rem % 3600 / 60, rem % 60)
}

fn read_arg(s: &str) -> Result<String, String> {
    if s == "-" {
        let mut buf = String::new();
        std::io::stdin().read_to_string(&mut buf).map_err(|e| e.to_string())?;
        Ok(buf)
    } else if std::path::Path::new(s).exists() {
        std::fs::read_to_string(s).map_err(|e| e.to_string())
    } else {
        Ok(s.to_string())
    }
}

fn save(doc: &mut Document, path: &str) -> Result<(), String> {
    let bytes = doc.save_stamped(&docxtor_core::docprops::now_iso()).map_err(|e| e.to_string())?;
    write_docx(path, bytes)
}

fn print_report(r: &fidelity::FidelityReport) {
    println!("Fidelity score: {:.1}%", r.score);
    println!("Parts identical: {}/{}", r.parts_identical, r.parts_total);
    for m in &r.missing {
        println!("  missing: {m}");
    }
    for a in &r.added {
        println!("  added:   {a}");
    }
    for c in &r.changed {
        println!("  changed: {} (similarity {:.1}%, text {})", c.name, c.similarity * 100.0, if c.text_equal { "identical" } else { "differs" });
    }
}

fn walk(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                out.extend(walk(&p));
            } else if p.extension().is_some_and(|x| x.eq_ignore_ascii_case("docx")) && !p.file_name().is_some_and(|n| n.to_string_lossy().starts_with("~$")) {
                out.push(p);
            }
        }
    }
    out
}

fn texts(doc: &Document) -> Vec<String> {
    let mut v = Vec::new();
    docxtor_core::model::walk_paragraphs(&doc.body, &mut |p| v.push(p.plain_text()));
    v
}

/// Score one file: does it open, is an untouched save lossless, and how much survives
/// when the whole body is re-serialized from Docxtor's model.
fn score_file(path: &std::path::Path) -> serde_json::Value {
    let name = path.display().to_string();
    let bytes = match read_docx(&name) {
        Ok(b) => b,
        Err(e) => return serde_json::json!({"file": name, "opened": false, "error": e}),
    };
    let result = std::panic::catch_unwind(|| -> Result<serde_json::Value, String> {
        let orig = original_package(&bytes)?;
        let mut doc = Document::open(&bytes).map_err(|e| e.to_string())?;
        let untouched = fidelity::compare(&orig, &Package::open(&doc.save().map_err(|e| e.to_string())?).map_err(|e| e.to_string())?);
        let before = texts(&doc);
        doc.apply(&docxtor_core::Op::DeleteBlocks { ids: vec![] }, false).map_err(|e| e.to_string())?;
        let out = doc.save().map_err(|e| e.to_string())?;
        let reser = fidelity::compare(&orig, &Package::open(&out).map_err(|e| e.to_string())?);
        let again = Document::open(&out).map_err(|e| format!("re-open failed: {e}"))?;
        // Every XML part must stay namespace-well-formed or Word will refuse the file.
        let saved = Package::open(&out).map_err(|e| e.to_string())?;
        for part in &saved.parts {
            if part.name.ends_with(".xml") || part.name.ends_with(".rels") {
                let x = docxtor_core::xml::parse(&part.data).map_err(|e| format!("{}: {e}", part.name))?;
                docxtor_core::xml::check_namespaces(&x).map_err(|e| format!("{}: {e}", part.name))?;
            }
        }
        Ok(serde_json::json!({
            "file": name, "opened": true,
            "untouched": untouched.score, "reserialized": reser.score,
            "textPreserved": texts(&again) == before,
            "changed": reser.changed,
        }))
    });
    match result {
        Ok(Ok(v)) => v,
        Ok(Err(e)) => serde_json::json!({"file": name, "opened": false, "error": e}),
        Err(_) => serde_json::json!({"file": name, "opened": false, "error": "panic"}),
    }
}

fn run(args: &[String]) -> Result<(), String> {
    let cmd = args.first().map(String::as_str).unwrap_or("help");
    let arg = |i: usize| args.get(i).cloned().ok_or_else(|| format!("missing argument\n\n{HELP}"));
    match cmd {
        "read" => {
            let mut doc = open(&arg(1)?)?;
            let start = flag(args, "--start").map(|s| s.trim_start_matches(|c: char| c.is_alphabetic()).parse().unwrap_or(0));
            let limit = flag(args, "--limit").and_then(|s| s.parse().ok()).unwrap_or(100_000);
            print!("{}", ai::docscript(&mut doc, &ai::ScriptOpts { start, limit }));
        }
        "outline" => print!("{}", ai::outline(&mut open(&arg(1)?)?)),
        "text" => print!("{}", docxtor_core::export::to_text(&open(&arg(1)?)?)),
        "markdown" | "md" => print!("{}", docxtor_core::export::to_markdown(&open(&arg(1)?)?)),
        "export" => {
            let path = arg(1)?;
            let doc = open(&path)?;
            let format = flag(args, "--format").ok_or("missing --format html|epub|odt|rtf")?;
            let bytes: Vec<u8> = match format.as_str() {
                "html" => docxtor_core::export::html::to_html(&doc).into_bytes(),
                "rtf" => docxtor_core::export::rtf::to_rtf(&doc).into_bytes(),
                "epub" => docxtor_core::export::epub::to_epub(&doc, Some(&iso_now()))?,
                "odt" => docxtor_core::export::odt::to_odt(&doc)?,
                other => return Err(format!("unknown format {other}; use html, epub, odt or rtf")),
            };
            match flag(args, "-o") {
                Some(out) => {
                    safe_write::write_file(&out, &bytes)?;
                    println!("wrote {out}");
                }
                None if matches!(format.as_str(), "html" | "rtf") => {
                    use std::io::Write as _;
                    std::io::stdout().write_all(&bytes).map_err(|e| e.to_string())?;
                }
                None => {
                    let out = std::path::Path::new(&path).with_extension(&format);
                    safe_write::write_file(&out, &bytes)?;
                    println!("wrote {}", out.display());
                }
            }
        }
        "json" => {
            let mut doc = open(&arg(1)?)?;
            let v = serde_json::json!({ "page": doc.page_setup(), "doc": doc.render_all(), "styles": doc.style_list() });
            println!("{}", serde_json::to_string_pretty(&v).unwrap());
        }
        "info" => {
            let path = arg(1)?;
            let bytes = read_docx(&path)?;
            let mut doc = Document::open(&bytes).map_err(|e| e.to_string())?;
            println!("{}", ai::summary(&mut doc));
            let out = doc.save().map_err(|e| e.to_string())?;
            let r = fidelity::compare(&original_package(&bytes)?, &Package::open(&out).map_err(|e| e.to_string())?);
            println!("Unedited round-trip fidelity: {:.1}%", r.score);
        }
        "roundtrip" => {
            let (input, output) = (arg(1)?, arg(2)?);
            let bytes = read_docx(&input)?;
            let mut doc = Document::open(&bytes).map_err(|e| e.to_string())?;
            doc.touch_all();
            // Force full re-serialization of the body to measure the model, not the cache.
            doc.apply(&docxtor_core::Op::DeleteBlocks { ids: vec![] }, false).map_err(|e| e.to_string())?;
            let out = doc.save().map_err(|e| e.to_string())?;
            write_docx(&output, out.clone())?;
            print_report(&fidelity::compare(&original_package(&bytes)?, &Package::open(&out).map_err(|e| e.to_string())?));
        }
        "fidelity" => {
            let a = read_docx(&arg(1)?)?;
            let b = read_docx(&arg(2)?)?;
            let r = fidelity::compare(&original_package(&a)?, &original_package(&b)?);
            if args.iter().any(|a| a == "--json") {
                println!("{}", serde_json::to_string_pretty(&r).unwrap());
            } else {
                print_report(&r);
            }
        }
        "apply" => {
            let path = arg(1)?;
            let mut doc = open(&path)?;
            let ops = read_arg(&arg(2)?)?;
            let res = doc.apply_json(&ops, false).map_err(|e| e.to_string())?;
            println!("{}", serde_json::to_string(&res).unwrap());
            save(&mut doc, &flag(args, "-o").unwrap_or(path))?;
        }
        "tool" => {
            let path = arg(1)?;
            let mut doc = open(&path)?;
            let name = arg(2)?;
            let json = read_arg(&args.get(3).cloned().unwrap_or_else(|| "{}".into()))?;
            let v: serde_json::Value = serde_json::from_str(&json).map_err(|e| e.to_string())?;
            let out = ai::call_tool(&mut doc, &name, &v)?;
            println!("{out}");
            if doc.is_modified() {
                save(&mut doc, &flag(args, "-o").unwrap_or(path))?;
            }
        }
        "merge" => {
            let path = arg(1)?;
            let list = arg(2)?;
            let mut doc = open(&path)?;
            let bytes = std::fs::read(&list).map_err(|e| format!("{list}: {e}"))?;
            let name = std::path::Path::new(&list).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| list.clone());
            let r = docxtor_core::merge::read_recipients(&bytes, &name, flag(args, "--sheet").as_deref())?;
            let main_type = match flag(args, "--type").as_deref().unwrap_or("") {
                "" => {
                    let i = doc.merge_info();
                    if i.active { i.main_type } else { "formLetters".into() }
                }
                "letters" => "formLetters".into(),
                "labels" => "mailingLabels".into(),
                "envelopes" => "envelopes".into(),
                "directory" => "catalog".into(),
                other => return Err(format!("unknown type {other}; use letters, labels, envelopes or directory")),
            };
            let def = docxtor_core::merge::MailMergeDef { main_type, source: Some(r.source), columns: r.columns, rows: r.rows, ..Default::default() };
            doc.apply(&docxtor_core::Op::SetMailMerge { merge: Some(def) }, false).map_err(|e| e.to_string())?;
            let range = docxtor_core::document::MergeRange { from: flag(args, "--from").and_then(|v| v.parse::<usize>().ok()).map(|n| n.saturating_sub(1)), to: flag(args, "--to").and_then(|v| v.parse().ok()) };
            let (out, result) = doc.merge_to_docx(&Default::default(), &range)?;
            let target = flag(args, "-o").unwrap_or_else(|| std::path::Path::new(&path).with_file_name("merged.docx").display().to_string());
            safe_write::write_docx(&target, &out)?;
            println!("merged {} records into {} copies ({} left out): wrote {target}", result.records, result.copies, result.skipped);
        }
        "corpus" => {
            let dir = arg(1)?;
            let min: f64 = flag(args, "--min").and_then(|v| v.parse().ok()).unwrap_or(0.0);
            let mut files: Vec<_> = walk(std::path::Path::new(&dir));
            files.sort();
            if password().is_none() {
                let before = files.len();
                files.retain(|f| !std::fs::read(f).is_ok_and(|b| office_crypt::is_encrypted(&b)));
                if files.len() < before {
                    eprintln!("skipped {} password-protected file(s); give --password to include them", before - files.len());
                }
            }
            let mut rows = Vec::new();
            for f in &files {
                rows.push(score_file(f));
            }
            let ok: Vec<&serde_json::Value> = rows.iter().filter(|r| r["opened"] == true).collect();
            let avg = |k: &str| if ok.is_empty() { 0.0 } else { ok.iter().map(|r| r[k].as_f64().unwrap_or(0.0)).sum::<f64>() / ok.len() as f64 };
            let worst = |k: &str| ok.iter().map(|r| r[k].as_f64().unwrap_or(0.0)).fold(100.0, f64::min);
            let summary = serde_json::json!({
                "files": rows.len(),
                "opened": ok.len(),
                "openRate": if rows.is_empty() { 0.0 } else { ok.len() as f64 * 100.0 / rows.len() as f64 },
                "untouchedAvg": avg("untouched"),
                "reserializedAvg": avg("reserialized"),
                "reserializedMin": worst("reserialized"),
                "textPreserved": ok.iter().filter(|r| r["textPreserved"] == true).count(),
            });
            if args.iter().any(|a| a == "--json") {
                println!("{}", serde_json::to_string_pretty(&serde_json::json!({"summary": summary, "files": rows})).unwrap());
            } else {
                println!("{:<48} {:>8} {:>10} {:>12}  text", "file", "opens", "untouched", "reserialized");
                for r in &rows {
                    let name = r["file"].as_str().unwrap_or("");
                    let short = if name.len() > 47 { &name[name.len() - 47..] } else { name };
                    if r["opened"] == true {
                        println!("{:<48} {:>8} {:>9.1}% {:>11.1}%  {}", short, "yes", r["untouched"].as_f64().unwrap_or(0.0), r["reserialized"].as_f64().unwrap_or(0.0), if r["textPreserved"] == true { "ok" } else { "DIFFERS" });
                    } else {
                        println!("{:<48} {:>8}  {}", short, "NO", r["error"].as_str().unwrap_or(""));
                    }
                }
                println!("\n{} files · opened {:.1}% · untouched {:.2}% · reserialized avg {:.2}% (min {:.1}%) · text preserved {}/{}",
                    summary["files"], summary["openRate"].as_f64().unwrap(), summary["untouchedAvg"].as_f64().unwrap(),
                    summary["reserializedAvg"].as_f64().unwrap(), summary["reserializedMin"].as_f64().unwrap(), summary["textPreserved"], summary["opened"]);
            }
            if summary["reserializedMin"].as_f64().unwrap_or(0.0) < min || ok.len() < rows.len() {
                return Err(format!("fidelity gate failed (min reserialized score {min}, all files must open)"));
            }
        }
        "diff" => {
            let a = open(&arg(1)?)?;
            let b = open(&arg(2)?)?;
            let d = docxtor_core::diff::diff_documents(&a, &b);
            if args.iter().any(|x| x == "--json") {
                println!("{}", serde_json::to_string_pretty(&d).unwrap());
            } else {
                println!("{} changed · {} added · {} removed", d.changed, d.added, d.removed);
                for e in &d.entries {
                    match e.kind {
                        "added" => println!("+ [{}] {}", e.index + 1, e.after.as_deref().unwrap_or("")),
                        "removed" => println!("- [{}] {}", e.index + 1, e.before.as_deref().unwrap_or("")),
                        _ => println!("~ [{}] {}\n    → {}", e.index + 1, e.before.as_deref().unwrap_or(""), e.after.as_deref().unwrap_or("")),
                    }
                }
            }
        }
        "check" => {
            let doc = open(&arg(1)?)?;
            if args.iter().any(|x| x == "--json") {
                println!("{}", serde_json::to_string_pretty(&doc.accessibility()).unwrap());
            } else {
                print!("{}", doc.accessibility_report());
            }
        }
        "compare" => {
            let old = open(&arg(1)?)?;
            let new = open(&arg(2)?)?;
            let out = flag(args, "-o").ok_or("compare needs -o <redline.docx>")?;
            let author = flag(args, "--author").unwrap_or_else(|| "Docxtor".into());
            let date = flag(args, "--date").unwrap_or_else(iso_now);
            let bytes = docxtor_core::compare::redline(&old, &new, &author, &date).map_err(|e| e.to_string())?;
            safe_write::write_docx(&out, &bytes)?;
            let d = new.compare_summary(&old);
            println!("{} changed · {} added · {} removed", d.changed, d.added, d.removed);
            println!("wrote {out}");
        }
        "combine" => {
            let original = open(&arg(1)?)?;
            let revised = open(&arg(2)?)?;
            let out = flag(args, "-o").ok_or("combine needs -o <out.docx>")?;
            let author = flag(args, "--author").unwrap_or_else(|| "Docxtor".into());
            let date = flag(args, "--date").unwrap_or_else(iso_now);
            let bytes = docxtor_core::compare::combine(&original, &revised, &author, &date).map_err(|e| e.to_string())?;
            safe_write::write_docx(&out, &bytes)?;
            println!("wrote {out}");
        }
        "inspect" => {
            let mut doc = open(&arg(1)?)?;
            if let Some(what) = flag(args, "--remove") {
                let what: Vec<String> = if what == "all" {
                    docxtor_core::document::CATEGORIES.iter().filter(|c| **c != "embedded").map(|c| c.to_string()).collect()
                } else {
                    what.split(',').map(|c| c.trim().to_string()).collect()
                };
                let r = doc.apply_batch(&[docxtor_core::Op::RemoveHiddenData { what }], false).map_err(|e| e.to_string())?;
                println!("removed {} item(s)", r.first().and_then(|r| r.count).unwrap_or(0));
                let out = flag(args, "-o").unwrap_or(arg(1)?);
                safe_write::write_docx(&out, &doc.save().map_err(|e| e.to_string())?)?;
                println!("wrote {out}");
            }
            if args.iter().any(|x| x == "--json") {
                println!("{}", serde_json::to_string_pretty(&doc.inspect()).unwrap());
            } else {
                for f in doc.inspect() {
                    println!("{:<16} {}{}", f.id, f.count, if f.detail.is_empty() { String::new() } else { format!("  {}", f.detail.join("; ")) });
                }
            }
        }
        "count" => {
            let doc = open(&arg(1)?)?;
            let s = doc.text_stats();
            let t = if args.iter().any(|x| x == "--extras") { s.main.plus(&s.notes).plus(&s.boxes) } else { s.main };
            if args.iter().any(|x| x == "--json") {
                println!("{}", serde_json::to_string_pretty(&serde_json::json!({ "counted": t, "notes": s.notes, "boxes": s.boxes })).unwrap());
            } else {
                println!("{} paragraphs, {} words, {} characters ({} without spaces)", t.paragraphs, t.words, t.chars, t.chars_no_spaces);
            }
        }
        "sources" => {
            let mut doc = open(&arg(1)?)?;
            if args.iter().any(|x| x == "--json") {
                let v = serde_json::json!({ "style": doc.citation_style(), "sources": doc.sources(), "citations": doc.citations() });
                println!("{}", serde_json::to_string_pretty(&v).unwrap());
            } else if args.iter().any(|x| x == "--bibtex") {
                print!("{}", docxtor_core::bibtex::to_bibtex(&doc.sources()));
            } else {
                println!("{}", ai::call_tool(&mut doc, "list_sources", &serde_json::json!({}))?);
            }
        }
        "cite-import" => {
            let path = arg(1)?;
            let refs = arg(2)?;
            let mut doc = open(&path)?;
            let text = std::fs::read_to_string(&refs).map_err(|e| format!("{refs}: {e}"))?;
            let lower = refs.to_lowercase();
            let json_like = text.trim_start_matches('\u{feff}').trim_start().starts_with(['[', '{']) && !text.contains('@');
            let found = if lower.ends_with(".json") || (json_like && !lower.ends_with(".bib") && !lower.ends_with(".bibtex")) {
                docxtor_core::csl_json::parse(&text)?
            } else {
                docxtor_core::bibtex::parse(&text)?
            };
            // Skip what is already there: the same DOI, or the same title and year.
            let have = doc.sources();
            let same = |a: &docxtor_core::citations::Source, b: &docxtor_core::citations::Source| {
                let doi = |s: &docxtor_core::citations::Source| s.doi.as_deref().map(|d| d.trim().to_lowercase()).filter(|d| !d.is_empty());
                (doi(a).is_some() && doi(a) == doi(b)) || (a.title.trim().eq_ignore_ascii_case(b.title.trim()) && !a.title.trim().is_empty() && a.year == b.year)
            };
            let total = found.len();
            let mut fresh: Vec<docxtor_core::citations::Source> = Vec::new();
            for s in found {
                if !have.iter().chain(fresh.iter()).any(|h| same(h, &s)) {
                    fresh.push(s);
                }
            }
            let ops: Vec<docxtor_core::Op> = fresh.iter().map(|s| docxtor_core::Op::AddSource { source: s.clone() }).collect();
            if !ops.is_empty() {
                doc.apply_batch(&ops, false).map_err(|e| e.to_string())?;
            }
            let after = doc.sources();
            for s in &after[have.len()..] {
                println!("added {} ({})", s.tag, s.title);
            }
            println!("{} source(s) added, {} already in the document", fresh.len(), total - fresh.len());
            if !ops.is_empty() {
                save(&mut doc, &flag(args, "-o").unwrap_or(path))?;
            }
        }
        "convert" => {
            // Any file Docxtor can open (.odt, .rtf, .html, .doc, Word 2003 XML, Flat OPC, a damaged or Strict .docx) as a .docx.
            let input = arg(1)?;
            let bytes = std::fs::read(&input).map_err(|e| format!("{input}: {e}"))?;
            let out = args.get(2).filter(|a| !a.starts_with('-')).cloned().unwrap_or_else(|| format!("{}.docx", input.rsplit_once('.').map(|(s, _)| s).unwrap_or(&input)));
            let (docx, report) = docxtor_core::open::to_docx(&bytes, &input).map_err(|e| format!("{input}: {e}"))?;
            safe_write::write_docx(&out, &docx)?;
            println!("wrote {out}");
            for note in &report.notes {
                println!("  {}", docxtor_core::open::describe(note));
            }
        }
        "tools" => println!("{}", serde_json::to_string_pretty(&ai::tool_definitions()).unwrap()),
        "encrypt" | "decrypt" => {
            let (input, output) = (arg(1)?, arg(2)?);
            let pw = password().ok_or_else(|| format!("{cmd} needs --password <pw> (or DOCXTOR_PASSWORD)"))?;
            let bytes = std::fs::read(&input).map_err(|e| format!("{input}: {e}"))?;
            let out = if cmd == "encrypt" {
                if office_crypt::is_encrypted(&bytes) {
                    return Err(format!("{input} is already protected with a password; decrypt it first"));
                }
                Package::open(&bytes).map_err(|e| format!("{input}: {e}"))?;
                office_crypt::try_os_random(&mut [0u8; 1])?;
                office_crypt::encrypt(&bytes, pw, &mut rng)
            } else {
                office_crypt::decrypt(&bytes, pw).map_err(|e| match e {
                    CryptError::WrongPassword => format!("{input}: that password isn't right"),
                    CryptError::NotEncrypted => format!("{input} is not protected with a password"),
                    e => format!("{input}: {e}"),
                })?
            };
            if cmd == "encrypt" {
                safe_write::write_protected_docx(&output, &out)?;
            } else {
                safe_write::write_docx(&output, &out)?;
            }
            println!("{} {output}", if cmd == "encrypt" { "protected" } else { "unprotected" });
        }
        "new" => {
            let out = arg(1)?;
            let mut doc = Document::new_blank();
            if let Some(md) = flag(args, "--markdown") {
                let md = read_arg(&md)?;
                let first = docxtor_core::model::paragraph_order(&doc.body)[0];
                ai::call_tool(&mut doc, "insert_content", &serde_json::json!({"before": first, "markdown": md}))?;
                ai::call_tool(&mut doc, "delete_blocks", &serde_json::json!({"ids": [first]}))?;
            }
            if let Some(pw) = password() {
                SESSION.with(|s| *s.borrow_mut() = Some(CryptSession::new(pw)));
            }
            save(&mut doc, &out)?;
            println!("created {out}");
        }
        _ => println!("{HELP}"),
    }
    Ok(())
}
