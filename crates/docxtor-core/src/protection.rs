//! Document protection: `w:documentProtection` in `settings.xml`, the password hash Word writes for it, the
//! `_MarkAsFinal` custom property, and the ops that set and stop protection. `document/protect.rs` applies it
//! to a document and enforces it.

use crate::ops::Pos;
use crate::xml::{Element, Node, XmlDoc};
use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};
use sha2::{Sha256, Sha384, Sha512};

pub const REL_CUSTOM_PROPS: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/custom-properties";
pub const CT_CUSTOM_PROPS: &str = "application/vnd.openxmlformats-officedocument.custom-properties+xml";
const NS_CUSTOM: &str = "http://schemas.openxmlformats.org/officeDocument/2006/custom-properties";
const NS_VT: &str = "http://schemas.openxmlformats.org/officeDocument/2006/docPropsVTypes";
/// The format id every custom property of Office documents carries.
const FMTID: &str = "{D5CDD505-2E9C-101B-9397-08002B2CF9AE}";

/// What the document restricts, read from `w:documentProtection`.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Protection {
    /// `none`, `readOnly`, `comments`, `trackedChanges` or `forms`.
    pub edit: String,
    /// Protection is switched on (the file's `w:enforcement`).
    pub enforced: bool,
    /// Formatting is limited to the styles not marked locked.
    pub formatting: bool,
    pub has_password: bool,
    /// The file only has the old 32-bit hash of the password.
    pub legacy_hash: bool,
}

impl Protection {
    /// Whether editing is restricted by the edit mode (read only, comments and forms), as opposed to formatting
    /// or tracked changes, which leave the text editable.
    pub fn restricts_editing(&self) -> bool {
        self.enforced && matches!(self.edit.as_str(), "readOnly" | "comments" | "forms")
    }
}

fn truthy(v: Option<&str>) -> bool {
    matches!(v, Some("1" | "true" | "on"))
}

pub fn read(settings: Option<&XmlDoc>) -> Protection {
    let Some(e) = settings.and_then(|d| d.root.child("w:documentProtection")) else { return Protection { edit: "none".into(), ..Default::default() } };
    Protection {
        edit: e.attr("w:edit").unwrap_or("none").to_string(),
        enforced: truthy(e.attr("w:enforcement")),
        formatting: truthy(e.attr("w:formatting")),
        has_password: e.attr("w:hash").or(e.attr("w:hashValue")).is_some() || e.attr("w:unprotectPassword").is_some(),
        legacy_hash: e.attr("w:hash").is_none() && e.attr("w:hashValue").is_none() && e.attr("w:unprotectPassword").is_some(),
    }
}

// ---------------------------------------------------------------------------
// The password hash
// ---------------------------------------------------------------------------

fn digest(alg: &str, data: &[u8]) -> Option<Vec<u8>> {
    Some(match alg {
        "SHA-1" => Sha1::digest(data).to_vec(),
        "SHA-256" => Sha256::digest(data).to_vec(),
        "SHA-384" => Sha384::digest(data).to_vec(),
        "SHA-512" => Sha512::digest(data).to_vec(),
        _ => return None,
    })
}

/// `cryptAlgorithmSid` to the algorithm's name.
fn alg_of_sid(sid: &str) -> Option<&'static str> {
    Some(match sid {
        "4" => "SHA-1",
        "12" => "SHA-256",
        "13" => "SHA-384",
        "14" => "SHA-512",
        _ => return None,
    })
}

/// The most rounds of password hashing Docxtor will run for a file. MS-OFFCRYPTO sets 10,000,000 as the largest
/// spin count; a file that asks for more is refused rather than left to hash for hours. It applies to the
/// document protection hash here and to the password key of an encrypted file (`office_crypt`).
pub const MAX_SPIN_COUNT: u32 = 10_000_000;

/// Word's hash of a protection password: `H(salt + password in UTF-16)`, then `spin` rounds of
/// `H(previous + round number as 4 bytes, little endian)`. None for an unknown algorithm or a spin count above
/// `MAX_SPIN_COUNT`.
pub fn hash_password(alg: &str, salt: &[u8], spin: u32, password: &str) -> Option<Vec<u8>> {
    if spin > MAX_SPIN_COUNT {
        return None;
    }
    let mut seed = salt.to_vec();
    for u in password.encode_utf16() {
        seed.extend_from_slice(&u.to_le_bytes());
    }
    let mut h = digest(alg, &seed)?;
    for i in 0..spin {
        h.extend_from_slice(&i.to_le_bytes());
        h = digest(alg, &h)?;
    }
    Some(h)
}

/// The 16-bit password verifier of Word 97 to 2003 (MS-OFFCRYPTO 2.3.7.1). The old `w:unprotectPassword`
/// carries it in one half of its four bytes.
pub fn legacy_verifier(password: &str) -> u16 {
    let bytes: Vec<u8> = password.chars().take(15).map(|c| (c as u32 & 0xFF) as u8).collect();
    let mut v: u16 = 0;
    let step = |v: u16, b: u8| -> u16 { (((v & 0x4000) != 0) as u16 | ((v << 1) & 0x7FFF)) ^ b as u16 };
    for b in bytes.iter().rev() {
        v = step(v, *b);
    }
    v = step(v, bytes.len() as u8);
    v ^ 0xCE4B
}

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub fn b64_encode(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for c in data.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        out.push(B64[(n >> 18) as usize & 63] as char);
        out.push(B64[(n >> 12) as usize & 63] as char);
        out.push(if c.len() > 1 { B64[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if c.len() > 2 { B64[n as usize & 63] as char } else { '=' });
    }
    out
}

pub fn b64_decode(s: &str) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    let mut buf = 0u32;
    let mut bits = 0;
    for ch in s.bytes().filter(|c| !c.is_ascii_whitespace() && *c != b'=') {
        let v = B64.iter().position(|b| *b == ch)? as u32;
        buf = (buf << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
            buf &= (1 << bits) - 1;
        }
    }
    Some(out)
}

/// Whether `password` opens the protection `e` describes (`w:documentProtection`). A document with no password
/// accepts anything. An error when the file asks for more hashing rounds than `MAX_SPIN_COUNT`.
pub fn check_password(e: &Element, password: &str) -> Result<bool, String> {
    // The newer attribute names and the ones Word 2010 and later write.
    let hash = e.attr("w:hashValue").or(e.attr("w:hash"));
    let salt = e.attr("w:saltValue").or(e.attr("w:salt"));
    let alg = e.attr("w:algorithmName").or_else(|| e.attr("w:cryptAlgorithmSid").and_then(alg_of_sid));
    let spin: u64 = match e.attr("w:spinCount").or(e.attr("w:cryptSpinCount")) {
        None => 0,
        Some(v) => v.trim().parse().unwrap_or(u64::MAX),
    };
    if spin > MAX_SPIN_COUNT as u64 {
        return Err(format!("this document's protection asks for more than {MAX_SPIN_COUNT} rounds of password hashing, which is more than Word allows, so Docxtor won't check it"));
    }
    let spin = spin as u32;
    if let (Some(hash), Some(alg)) = (hash, alg) {
        let want = b64_decode(hash);
        let salt = salt.and_then(b64_decode).unwrap_or_default();
        return Ok(match (hash_password(alg, &salt, spin, password), want) {
            (Some(got), Some(want)) => got == want,
            _ => false,
        });
    }
    if let Some(legacy) = e.attr("w:unprotectPassword") {
        let Ok(v) = u32::from_str_radix(legacy.trim(), 16) else { return Ok(false) };
        let ver = legacy_verifier(password) as u32;
        return Ok(v & 0xFFFF == ver || v >> 16 == ver);
    }
    Ok(true)
}

// ---------------------------------------------------------------------------
// Writing the protection
// ---------------------------------------------------------------------------

/// What Restrict editing sets. A field left out stays as it is.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ProtectionSettings {
    /// `none`, `readOnly`, `comments`, `trackedChanges` or `forms`.
    pub edit: Option<String>,
    /// Limit formatting to the styles not marked locked.
    pub formatting: Option<bool>,
    /// Switch protection on or off. Starting needs the password when one is wanted; stopping needs it when the
    /// document has one (use `stop` for that).
    pub enforce: Option<bool>,
    pub password: Option<String>,
    /// 16 random bytes in base64, for the password hash. The caller supplies them so the op can be replayed.
    pub salt: Option<String>,
    /// The ids of the styles people may apply while formatting is limited; every other style is locked.
    pub allowed_styles: Option<Vec<String>>,
}

/// Write `w:documentProtection` in `settings`.
pub fn apply(root: &mut Element, s: &ProtectionSettings) -> Result<(), String> {
    let mut e = root.child("w:documentProtection").cloned().unwrap_or_else(|| Element::new("w:documentProtection"));
    if let Some(edit) = &s.edit {
        if !matches!(edit.as_str(), "none" | "readOnly" | "comments" | "trackedChanges" | "forms") {
            return Err(format!("unknown protection '{edit}'"));
        }
        e.set_attr("w:edit", edit);
    }
    if let Some(f) = s.formatting {
        if f {
            e.set_attr("w:formatting", "1");
        } else {
            e.remove_attr("w:formatting");
        }
    }
    match s.enforce {
        Some(true) => {
            if e.attr("w:edit").is_none() {
                e.set_attr("w:edit", "none");
            }
            e.set_attr("w:enforcement", "1");
            for a in ["w:cryptProviderType", "w:cryptAlgorithmClass", "w:cryptAlgorithmType", "w:cryptAlgorithmSid", "w:cryptSpinCount", "w:hash", "w:salt", "w:algorithmName", "w:hashValue", "w:saltValue", "w:spinCount", "w:unprotectPassword"] {
                e.remove_attr(a);
            }
            if let Some(pw) = s.password.as_deref().filter(|p| !p.is_empty()) {
                let salt = s.salt.as_deref().and_then(b64_decode).filter(|b| !b.is_empty()).ok_or("a password needs a salt")?;
                let h = hash_password("SHA-512", &salt, 100_000, pw).expect("SHA-512 is known");
                e.set_attr("w:cryptProviderType", "rsaAES");
                e.set_attr("w:cryptAlgorithmClass", "hash");
                e.set_attr("w:cryptAlgorithmType", "typeAny");
                e.set_attr("w:cryptAlgorithmSid", "14");
                e.set_attr("w:cryptSpinCount", "100000");
                e.set_attr("w:hash", b64_encode(&h));
                e.set_attr("w:salt", b64_encode(&salt));
            }
        }
        Some(false) => {
            e.set_attr("w:enforcement", "0");
            for a in ["w:cryptProviderType", "w:cryptAlgorithmClass", "w:cryptAlgorithmType", "w:cryptAlgorithmSid", "w:cryptSpinCount", "w:hash", "w:salt", "w:algorithmName", "w:hashValue", "w:saltValue", "w:spinCount", "w:unprotectPassword"] {
                e.remove_attr(a);
            }
        }
        None => {}
    }
    crate::props::put_ordered(root, e, crate::document::SETTINGS_ORDER);
    Ok(())
}

// ---------------------------------------------------------------------------
// Mark as final
// ---------------------------------------------------------------------------

/// Whether the custom properties say the document is marked as final.
pub fn is_marked_final(custom: Option<&XmlDoc>) -> bool {
    let Some(d) = custom else { return false };
    d.root.elements().any(|p| p.attr("name") == Some("_MarkAsFinal") && p.find("vt:bool").is_some_and(|b| matches!(b.text().trim(), "true" | "1")))
}

pub fn new_custom_doc() -> XmlDoc {
    // `XmlDoc::new` has the declaration; `decl` holds what follows `<?xml `, so setting the whole declaration here wrote it twice.
    XmlDoc::new(Element::new("Properties").with_attr("xmlns", NS_CUSTOM).with_attr("xmlns:vt", NS_VT))
}

/// Set or remove `_MarkAsFinal`.
pub fn set_marked_final(doc: &mut XmlDoc, on: bool) {
    doc.root.children.retain(|n| !matches!(n, Node::Element(p) if p.attr("name") == Some("_MarkAsFinal")));
    if on {
        let pid = doc.root.elements().filter_map(|p| p.attr("pid")).filter_map(|v| v.parse::<u32>().ok()).max().unwrap_or(1) + 1;
        let prop = Element::new("property")
            .with_attr("fmtid", FMTID)
            .with_attr("pid", pid.to_string())
            .with_attr("name", "_MarkAsFinal")
            .with_child(Element::new("vt:bool").with_text("true"));
        doc.root.children.push(Node::Element(prop));
    }
}

/// Protection ops. The wire form is `{"op": "protection", "action": "...", ...}`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum ProtectionOp {
    /// Set the restrictions and start or stop enforcing them.
    Set { settings: ProtectionSettings },
    /// Stop protection. The password is checked when the document has one.
    Stop {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        password: Option<String>,
    },
    /// Mark the document as final, or take the mark off.
    MarkFinal { on: bool },
    /// Make the text between two positions an editable range, or take the range at a position away.
    EditableRange {
        from: Pos,
        to: Pos,
        /// `everyone` (the default), or one of Word's groups: `administrators`, `contributors`, `editors`,
        /// `owners`, `current`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        group: Option<String>,
        #[serde(default)]
        remove: bool,
    },
}

impl ProtectionOp {
    /// The flags of the op (see `OpClass`): turning protection on or off and marking final change settings, not the body.
    pub fn class(&self) -> crate::ops::OpClass {
        use crate::ops::OpClass as C;
        match self {
            ProtectionOp::Set { .. } | ProtectionOp::Stop { .. } | ProtectionOp::MarkFinal { .. } => C::BODY.leaves_body(),
            ProtectionOp::EditableRange { .. } => C::BODY,
        }
    }

    pub fn anchor(&self) -> Option<crate::model::NodeId> {
        match self {
            ProtectionOp::EditableRange { from, .. } => Some(from.p.0),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::xml;

    #[test]
    fn the_hash_is_the_one_word_checks() {
        let salt = [1u8, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16];
        let mut root = xml::parse(b"<w:settings xmlns:w=\"w\"/>").unwrap().root;
        let s = ProtectionSettings { edit: Some("readOnly".into()), enforce: Some(true), password: Some("secret".into()), salt: Some(b64_encode(&salt)), ..Default::default() };
        apply(&mut root, &s).unwrap();
        let e = root.child("w:documentProtection").unwrap();
        assert_eq!(e.attr("w:cryptAlgorithmSid"), Some("14"));
        assert_eq!(e.attr("w:cryptSpinCount"), Some("100000"));
        assert_eq!(check_password(e, "secret"), Ok(true));
        assert_eq!(check_password(e, "Secret"), Ok(false));
        assert!(read(Some(&XmlDoc::new(root.clone()))).has_password);
    }

    #[test]
    fn the_legacy_verifier_is_known() {
        // The 16-bit verifier of "password" (Excel's well-known sheet protection hash is 83AF).
        assert_eq!(legacy_verifier("password"), 0x83AF);
        let mut e = Element::new("w:documentProtection");
        e.set_attr("w:unprotectPassword", "000083AF");
        assert_eq!(check_password(&e, "password"), Ok(true));
        assert_eq!(check_password(&e, "passwor"), Ok(false));
    }

    #[test]
    fn a_spin_count_above_the_limit_is_refused_at_once() {
        let t = std::time::Instant::now();
        for spin in ["10000001", "4000000000", "99999999999999999999", "lots"] {
            let mut e = Element::new("w:documentProtection");
            e.set_attr("w:algorithmName", "SHA-512");
            e.set_attr("w:hashValue", "AAAA");
            e.set_attr("w:saltValue", "AAAA");
            e.set_attr("w:spinCount", spin);
            assert!(check_password(&e, "x").is_err(), "{spin}");
        }
        assert!(hash_password("SHA-512", b"s", MAX_SPIN_COUNT + 1, "x").is_none());
        assert!(t.elapsed().as_millis() < 500);
    }

    #[test]
    fn base64_round_trips() {
        for n in 0..10usize {
            let data: Vec<u8> = (0..n as u8).collect();
            assert_eq!(b64_decode(&b64_encode(&data)).unwrap(), data);
        }
    }
}
