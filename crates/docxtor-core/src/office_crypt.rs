//! Password-protected Office files (MS-OFFCRYPTO).
//!
//! Word saves a protected .docx as a compound file (see `cfb`) holding `EncryptionInfo` (how it was
//! encrypted and a check for the password) and `EncryptedPackage` (the real .docx zip, encrypted). This
//! module reads both ways Word writes them, Agile Encryption (Word 2010 and later: AES with SHA-1, SHA-256,
//! SHA-384 or SHA-512, a spun password hash, the file's integrity checked with an HMAC) and Standard
//! Encryption (Word 2007: AES-ECB with SHA-1), and writes Agile Encryption the way current Word does
//! (AES-256, SHA-512, 100000 rounds).
//!
//! Randomness is passed in, so the same code runs in the browser (`crypto.getRandomValues`) and natively.

use crate::cfb::{self, CfbReader};
pub use crate::protection::MAX_SPIN_COUNT;
use aes::cipher::{consts::U16, Array, BlockCipherDecrypt, BlockCipherEncrypt, KeyInit};
use aes::{Aes128, Aes192, Aes256};
use hmac::{Hmac, Mac};
use sha1::{Digest, Sha1};
use sha2::{Sha256, Sha384, Sha512};

/// Why a file could not be decrypted.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CryptError {
    #[error("this file is not password protected")]
    NotEncrypted,
    #[error("that password isn't right")]
    WrongPassword,
    /// The password is right but the data does not match its integrity check: the file was changed or damaged.
    #[error("the file is damaged or was changed after it was protected")]
    Integrity,
    #[error("the protected file is damaged: {0}")]
    Corrupt(String),
    #[error("this kind of protection is not supported: {0}")]
    Unsupported(String),
}

impl CryptError {
    /// A short stable name for hosts that tell the errors apart.
    pub fn code(&self) -> &'static str {
        match self {
            CryptError::NotEncrypted => "not_encrypted",
            CryptError::WrongPassword => "wrong_password",
            CryptError::Integrity => "integrity",
            CryptError::Corrupt(_) => "corrupt",
            CryptError::Unsupported(_) => "unsupported",
        }
    }
}

impl From<cfb::CfbError> for CryptError {
    fn from(e: cfb::CfbError) -> Self {
        CryptError::Corrupt(e.to_string())
    }
}

fn corrupt(s: &str) -> CryptError {
    CryptError::Corrupt(s.to_string())
}

const SEGMENT: usize = 4096;
const VERIFIER_INPUT_BLOCK: [u8; 8] = [0xfe, 0xa7, 0xd2, 0x76, 0x3b, 0x4b, 0x9e, 0x79];
const VERIFIER_VALUE_BLOCK: [u8; 8] = [0xd7, 0xaa, 0x0f, 0x6d, 0x30, 0x61, 0x34, 0x4e];
const KEY_VALUE_BLOCK: [u8; 8] = [0x14, 0x6e, 0x0b, 0xe7, 0xab, 0xac, 0xd0, 0xd6];
const HMAC_KEY_BLOCK: [u8; 8] = [0x5f, 0xb2, 0xad, 0x01, 0x0c, 0xb9, 0xe1, 0xf6];
const HMAC_VALUE_BLOCK: [u8; 8] = [0xa0, 0x67, 0x7f, 0x02, 0xb2, 0x2c, 0x84, 0x33];

// ---------------------------------------------------------------------------------------------
// Primitives

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Hash {
    Sha1,
    Sha256,
    Sha384,
    Sha512,
}

impl Hash {
    fn from_name(n: &str) -> Result<Hash, CryptError> {
        match n {
            "SHA1" | "SHA-1" => Ok(Hash::Sha1),
            "SHA256" | "SHA-256" => Ok(Hash::Sha256),
            "SHA384" | "SHA-384" => Ok(Hash::Sha384),
            "SHA512" | "SHA-512" => Ok(Hash::Sha512),
            other => Err(CryptError::Unsupported(format!("the {other} hash"))),
        }
    }

    fn name(self) -> &'static str {
        match self {
            Hash::Sha1 => "SHA1",
            Hash::Sha256 => "SHA256",
            Hash::Sha384 => "SHA384",
            Hash::Sha512 => "SHA512",
        }
    }

    fn len(self) -> usize {
        match self {
            Hash::Sha1 => 20,
            Hash::Sha256 => 32,
            Hash::Sha384 => 48,
            Hash::Sha512 => 64,
        }
    }

    fn digest(self, parts: &[&[u8]]) -> Vec<u8> {
        fn run<D: Digest>(parts: &[&[u8]]) -> Vec<u8> {
            let mut d = D::new();
            for p in parts {
                d.update(p);
            }
            d.finalize().to_vec()
        }
        match self {
            Hash::Sha1 => run::<Sha1>(parts),
            Hash::Sha256 => run::<Sha256>(parts),
            Hash::Sha384 => run::<Sha384>(parts),
            Hash::Sha512 => run::<Sha512>(parts),
        }
    }

    fn hmac(self, key: &[u8], data: &[u8]) -> Vec<u8> {
        fn run<M: Mac + hmac::digest::KeyInit>(key: &[u8], data: &[u8]) -> Vec<u8> {
            let mut m = <M as hmac::digest::KeyInit>::new_from_slice(key).expect("HMAC takes any key length");
            m.update(data);
            m.finalize().into_bytes().to_vec()
        }
        match self {
            Hash::Sha1 => run::<Hmac<Sha1>>(key, data),
            Hash::Sha256 => run::<Hmac<Sha256>>(key, data),
            Hash::Sha384 => run::<Hmac<Sha384>>(key, data),
            Hash::Sha512 => run::<Hmac<Sha512>>(key, data),
        }
    }
}

enum Aes {
    K128(Aes128),
    K192(Aes192),
    K256(Aes256),
}

impl Aes {
    fn new(key: &[u8]) -> Result<Aes, CryptError> {
        let bad = |_| corrupt("the key has the wrong length");
        Ok(match key.len() {
            16 => Aes::K128(Aes128::new_from_slice(key).map_err(bad)?),
            24 => Aes::K192(Aes192::new_from_slice(key).map_err(bad)?),
            32 => Aes::K256(Aes256::new_from_slice(key).map_err(bad)?),
            _ => return Err(corrupt("the key has the wrong length")),
        })
    }

    fn enc(&self, block: &mut [u8]) {
        let b: &mut Array<u8, U16> = block.try_into().expect("an AES block is 16 bytes");
        match self {
            Aes::K128(c) => c.encrypt_block(b),
            Aes::K192(c) => c.encrypt_block(b),
            Aes::K256(c) => c.encrypt_block(b),
        }
    }

    fn dec(&self, block: &mut [u8]) {
        let b: &mut Array<u8, U16> = block.try_into().expect("an AES block is 16 bytes");
        match self {
            Aes::K128(c) => c.decrypt_block(b),
            Aes::K192(c) => c.decrypt_block(b),
            Aes::K256(c) => c.decrypt_block(b),
        }
    }

    /// CBC without padding; `data` must be a whole number of blocks.
    fn cbc_decrypt(&self, iv: &[u8], data: &[u8]) -> Vec<u8> {
        let mut out = data.to_vec();
        let mut prev = [0u8; 16];
        prev.copy_from_slice(&iv[..16]);
        for chunk in out.chunks_exact_mut(16) {
            let mut cur = [0u8; 16];
            cur.copy_from_slice(chunk);
            self.dec(chunk);
            for (c, p) in chunk.iter_mut().zip(prev.iter()) {
                *c ^= p;
            }
            prev = cur;
        }
        out
    }

    /// CBC without padding; `data` is padded with zeros to a whole number of blocks.
    fn cbc_encrypt(&self, iv: &[u8], data: &[u8]) -> Vec<u8> {
        let mut out = data.to_vec();
        out.resize(data.len().next_multiple_of(16), 0);
        let mut prev = [0u8; 16];
        prev.copy_from_slice(&iv[..16]);
        for chunk in out.chunks_exact_mut(16) {
            for (c, p) in chunk.iter_mut().zip(prev.iter()) {
                *c ^= p;
            }
            self.enc(chunk);
            prev.copy_from_slice(chunk);
        }
        out
    }

    fn ecb_decrypt(&self, data: &[u8]) -> Vec<u8> {
        let mut out = data.to_vec();
        for chunk in out.chunks_exact_mut(16) {
            self.dec(chunk);
        }
        out
    }
}

/// `v` cut, or padded with 0x36, to `n` bytes.
fn fit(mut v: Vec<u8>, n: usize) -> Vec<u8> {
    v.resize(n, 0x36);
    v
}

fn utf16le(s: &str) -> Vec<u8> {
    s.encode_utf16().flat_map(|u| u.to_le_bytes()).collect()
}

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn b64_encode(data: &[u8]) -> String {
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

fn b64_decode(s: &str) -> Result<Vec<u8>, CryptError> {
    let mut out = Vec::with_capacity(s.len() / 4 * 3);
    let (mut acc, mut bits) = (0u32, 0);
    for c in s.bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            b'=' | b' ' | b'\r' | b'\n' | b'\t' => continue,
            _ => return Err(corrupt("a value is not valid base64")),
        };
        acc = acc << 6 | v as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
        }
    }
    Ok(out)
}

fn le32(n: u32) -> [u8; 4] {
    n.to_le_bytes()
}

// ---------------------------------------------------------------------------------------------
// Agile Encryption

/// The settings and key material of an Agile Encryption file. Parsed from `EncryptionInfo`, or made fresh.
#[derive(Clone)]
struct AgileKeys {
    // keyData: how the package is encrypted.
    data_hash: Hash,
    data_block: usize,
    data_salt: Vec<u8>,
    // The password key encryptor.
    spin: u32,
    kdf_hash: Hash,
    kdf_block: usize,
    key_bits: usize,
    kdf_salt: Vec<u8>,
    enc_verifier_input: Vec<u8>,
    enc_verifier_hash: Vec<u8>,
    enc_key_value: Vec<u8>,
    /// The key the package is encrypted with.
    secret: Vec<u8>,
    /// The spun password hash, kept after `unlock` so a session need not run the rounds again.
    spun: Vec<u8>,
}

struct AgileInfo {
    keys: AgileKeys,
    enc_hmac_key: Vec<u8>,
    enc_hmac_value: Vec<u8>,
    has_integrity: bool,
    verifier_salt_size: usize,
}

fn attr_num(e: &crate::xml::Element, name: &str) -> Result<usize, CryptError> {
    e.attr(name).and_then(|v| v.parse().ok()).ok_or_else(|| corrupt(&format!("the {name} setting is missing")))
}

fn attr_str<'a>(e: &'a crate::xml::Element, name: &str) -> Result<&'a str, CryptError> {
    e.attr(name).ok_or_else(|| corrupt(&format!("the {name} setting is missing")))
}

fn check_cipher(e: &crate::xml::Element) -> Result<(), CryptError> {
    if attr_str(e, "cipherAlgorithm")? != "AES" {
        return Err(CryptError::Unsupported(format!("the {} cipher", attr_str(e, "cipherAlgorithm")?)));
    }
    if attr_str(e, "cipherChaining")? != "ChainingModeCBC" {
        return Err(CryptError::Unsupported(format!("the {} chaining mode", attr_str(e, "cipherChaining")?)));
    }
    if attr_num(e, "blockSize")? != 16 {
        return Err(CryptError::Unsupported("a block size other than 16".into()));
    }
    Ok(())
}

fn parse_agile(xml: &[u8]) -> Result<(AgileInfo, Vec<u8>), CryptError> {
    let doc = crate::xml::parse(xml).map_err(|_| corrupt("the encryption settings are not XML"))?;
    let root = &doc.root;
    if root.local() != "encryption" {
        return Err(corrupt("the encryption settings have no encryption element"));
    }
    let find = |name: &str| root.find(name).or_else(|| find_local(root, name));
    let kd = find("keyData").ok_or_else(|| corrupt("keyData is missing"))?;
    check_cipher(kd)?;
    let data_hash = Hash::from_name(attr_str(kd, "hashAlgorithm")?)?;
    let data_salt = b64_decode(attr_str(kd, "saltValue")?)?;
    let data_block = attr_num(kd, "blockSize")?;
    let ek = find_local(root, "encryptedKey").ok_or_else(|| CryptError::Unsupported("no password key encryptor (certificates are not supported)".into()))?;
    check_cipher(ek)?;
    let key_bits = attr_num(ek, "keyBits")?;
    if !matches!(key_bits, 128 | 192 | 256) {
        return Err(CryptError::Unsupported(format!("{key_bits}-bit keys")));
    }
    let kdf_hash = Hash::from_name(attr_str(ek, "hashAlgorithm")?)?;
    let keys = AgileKeys {
        data_hash,
        data_block,
        data_salt,
        spin: spin_count(attr_num(ek, "spinCount")?)?,
        kdf_hash,
        kdf_block: attr_num(ek, "blockSize")?,
        key_bits,
        kdf_salt: b64_decode(attr_str(ek, "saltValue")?)?,
        enc_verifier_input: b64_decode(attr_str(ek, "encryptedVerifierHashInput")?)?,
        enc_verifier_hash: b64_decode(attr_str(ek, "encryptedVerifierHashValue")?)?,
        enc_key_value: b64_decode(attr_str(ek, "encryptedKeyValue")?)?,
        secret: Vec::new(),
        spun: Vec::new(),
    };
    let di = find_local(root, "dataIntegrity");
    let (enc_hmac_key, enc_hmac_value, has_integrity) = match di {
        Some(d) => (b64_decode(attr_str(d, "encryptedHmacKey")?)?, b64_decode(attr_str(d, "encryptedHmacValue")?)?, true),
        None => (Vec::new(), Vec::new(), false),
    };
    let salt_size = attr_num(ek, "saltSize")?;
    Ok((AgileInfo { keys, enc_hmac_key, enc_hmac_value, has_integrity, verifier_salt_size: salt_size }, Vec::new()))
}

/// A spin count from the file, refused when it is above what Word allows (`MAX_SPIN_COUNT`): the password hash
/// would otherwise run for hours the moment someone types a password.
fn spin_count(n: usize) -> Result<u32, CryptError> {
    match u32::try_from(n) {
        Ok(n) if n <= MAX_SPIN_COUNT => Ok(n),
        _ => Err(CryptError::Unsupported(format!("a spin count of {n} (the most Word allows is {MAX_SPIN_COUNT})"))),
    }
}

fn find_local<'a>(e: &'a crate::xml::Element, local: &str) -> Option<&'a crate::xml::Element> {
    if e.local() == local {
        return Some(e);
    }
    e.elements().find_map(|c| find_local(c, local))
}

#[cfg(test)]
thread_local! {
    /// How many times this thread has run the password rounds (tests check a save does not).
    static SPUN: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// The spun password hash (MS-OFFCRYPTO 2.3.4.11): H0 = H(salt + password), then `spin` rounds.
fn spun_hash(hash: Hash, salt: &[u8], password: &str, spin: u32) -> Vec<u8> {
    #[cfg(test)]
    SPUN.with(|c| c.set(c.get() + 1));
    let mut h = hash.digest(&[salt, &utf16le(password)]);
    for i in 0..spin {
        h = hash.digest(&[&le32(i), &h]);
    }
    h
}

fn derived_key(hash: Hash, spun: &[u8], block: &[u8; 8], key_len: usize) -> Vec<u8> {
    fit(hash.digest(&[spun, block]), key_len)
}

impl AgileKeys {
    /// Check the password and get the key the package is encrypted with.
    fn unlock(&mut self, password: &str, verifier_salt_size: usize) -> Result<(), CryptError> {
        let spun = spun_hash(self.kdf_hash, &self.kdf_salt, password, self.spin);
        let klen = self.key_bits / 8;
        let iv = fit(self.kdf_salt.clone(), self.kdf_block);
        let bad_len = |v: &[u8]| v.is_empty() || !v.len().is_multiple_of(16);
        if bad_len(&self.enc_verifier_input) || bad_len(&self.enc_verifier_hash) || bad_len(&self.enc_key_value) {
            return Err(corrupt("an encrypted value has the wrong length"));
        }
        let k1 = Aes::new(&derived_key(self.kdf_hash, &spun, &VERIFIER_INPUT_BLOCK, klen))?;
        let input = k1.cbc_decrypt(&iv, &self.enc_verifier_input);
        let input = &input[..verifier_salt_size.min(input.len())];
        let k2 = Aes::new(&derived_key(self.kdf_hash, &spun, &VERIFIER_VALUE_BLOCK, klen))?;
        let hashed = k2.cbc_decrypt(&iv, &self.enc_verifier_hash);
        let want = self.kdf_hash.digest(&[input]);
        if hashed.len() < want.len() || hashed[..want.len()] != want[..] {
            return Err(CryptError::WrongPassword);
        }
        let k3 = Aes::new(&derived_key(self.kdf_hash, &spun, &KEY_VALUE_BLOCK, klen))?;
        let mut secret = k3.cbc_decrypt(&iv, &self.enc_key_value);
        if secret.len() < klen {
            return Err(corrupt("the encrypted key is too short"));
        }
        secret.truncate(klen);
        self.secret = secret;
        self.spun = spun;
        Ok(())
    }

    fn segment_iv(&self, index: u32) -> Vec<u8> {
        fit(self.data_hash.digest(&[&self.data_salt, &le32(index)]), self.data_block)
    }

    fn integrity_iv(&self, block: &[u8; 8]) -> Vec<u8> {
        fit(self.data_hash.digest(&[&self.data_salt, block]), self.data_block)
    }
}

fn agile_decrypt(info_xml: &[u8], package: &[u8], password: &str) -> Result<(Vec<u8>, Settings), CryptError> {
    let (mut info, _) = parse_agile(info_xml)?;
    info.keys.unlock(password, info.verifier_salt_size)?;
    let keys = &info.keys;
    let aes = Aes::new(&keys.secret)?;
    if package.len() < 8 {
        return Err(corrupt("the encrypted package is cut short"));
    }
    if info.has_integrity {
        let hlen = keys.data_hash.len();
        let hmac_key = aes.cbc_decrypt(&keys.integrity_iv(&HMAC_KEY_BLOCK), &info.enc_hmac_key);
        let hmac_value = aes.cbc_decrypt(&keys.integrity_iv(&HMAC_VALUE_BLOCK), &info.enc_hmac_value);
        if hmac_key.len() < hlen || hmac_value.len() < hlen {
            return Err(corrupt("the integrity values are cut short"));
        }
        let actual = keys.data_hash.hmac(&hmac_key[..hlen], package);
        if actual != hmac_value[..hlen] {
            return Err(CryptError::Integrity);
        }
    }
    let mut size_bytes = [0u8; 8];
    size_bytes.copy_from_slice(&package[..8]);
    let size = u64::from_le_bytes(size_bytes) as usize;
    let body = &package[8..];
    let mut out = Vec::with_capacity(body.len());
    for (i, chunk) in body.chunks(SEGMENT).enumerate() {
        if !chunk.len().is_multiple_of(16) {
            return Err(corrupt("the encrypted package is not a whole number of blocks"));
        }
        out.extend_from_slice(&aes.cbc_decrypt(&keys.segment_iv(i as u32), chunk));
    }
    if size > out.len() {
        return Err(corrupt("the encrypted package is shorter than it says"));
    }
    out.truncate(size);
    Ok((out, Settings::of(&info.keys)))
}

fn agile_encrypt_package(keys: &AgileKeys, zip: &[u8], rng: &mut dyn FnMut(&mut [u8])) -> (Vec<u8>, String) {
    let aes = Aes::new(&keys.secret).expect("the session key has a valid length");
    let mut package = Vec::with_capacity(8 + zip.len() + 16);
    package.extend_from_slice(&(zip.len() as u64).to_le_bytes());
    for (i, chunk) in zip.chunks(SEGMENT).enumerate() {
        package.extend_from_slice(&aes.cbc_encrypt(&keys.segment_iv(i as u32), chunk));
    }
    if zip.is_empty() {
        // An empty package still has no segments; nothing to add.
    }
    // The integrity check: a fresh HMAC key for each save, over the whole stream.
    let hlen = keys.data_hash.len();
    let mut hmac_key = vec![0u8; hlen];
    rng(&mut hmac_key);
    let value = keys.data_hash.hmac(&hmac_key, &package);
    let enc_key = aes.cbc_encrypt(&keys.integrity_iv(&HMAC_KEY_BLOCK), &hmac_key);
    let enc_value = aes.cbc_encrypt(&keys.integrity_iv(&HMAC_VALUE_BLOCK), &value);

    let mut xml = String::new();
    xml.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\r\n");
    xml.push_str("<encryption xmlns=\"http://schemas.microsoft.com/office/2006/encryption\" xmlns:p=\"http://schemas.microsoft.com/office/2006/keyEncryptor/password\" xmlns:c=\"http://schemas.microsoft.com/office/2006/keyEncryptor/certificate\">");
    xml.push_str(&format!(
        "<keyData saltSize=\"{}\" blockSize=\"{}\" keyBits=\"{}\" hashSize=\"{}\" cipherAlgorithm=\"AES\" cipherChaining=\"ChainingModeCBC\" hashAlgorithm=\"{}\" saltValue=\"{}\"/>",
        keys.data_salt.len(),
        keys.data_block,
        keys.secret.len() * 8,
        hlen,
        keys.data_hash.name(),
        b64_encode(&keys.data_salt)
    ));
    xml.push_str(&format!("<dataIntegrity encryptedHmacKey=\"{}\" encryptedHmacValue=\"{}\"/>", b64_encode(&enc_key), b64_encode(&enc_value)));
    xml.push_str("<keyEncryptors><keyEncryptor uri=\"http://schemas.microsoft.com/office/2006/keyEncryptor/password\">");
    xml.push_str(&format!(
        "<p:encryptedKey spinCount=\"{}\" saltSize=\"{}\" blockSize=\"{}\" keyBits=\"{}\" hashSize=\"{}\" cipherAlgorithm=\"AES\" cipherChaining=\"ChainingModeCBC\" hashAlgorithm=\"{}\" saltValue=\"{}\" encryptedVerifierHashInput=\"{}\" encryptedVerifierHashValue=\"{}\" encryptedKeyValue=\"{}\"/>",
        keys.spin,
        keys.kdf_salt.len(),
        keys.kdf_block,
        keys.key_bits,
        keys.kdf_hash.len(),
        keys.kdf_hash.name(),
        b64_encode(&keys.kdf_salt),
        b64_encode(&keys.enc_verifier_input),
        b64_encode(&keys.enc_verifier_hash),
        b64_encode(&keys.enc_key_value)
    ));
    xml.push_str("</keyEncryptor></keyEncryptors></encryption>");
    (package, xml)
}

/// The password key of a session: the salt it was spun with and the result of the 100000 rounds. This is the one
/// expensive step, so a session does it once. It is never written to storage.
#[derive(Clone)]
struct Kdf {
    salt: Vec<u8>,
    spun: Vec<u8>,
}

/// The choices of an Agile file that a save keeps: which hashes, how long the key is, how big the salts are and
/// how many rounds, and the spun password key. The package key, the data salt, the verifier and the integrity key
/// are never kept: every save makes its own.
#[derive(Clone)]
struct Settings {
    data_hash: Hash,
    kdf_hash: Hash,
    key_bits: usize,
    salt_len: usize,
    spin: u32,
    kdf: Option<Kdf>,
}

impl Settings {
    /// Word's current settings: AES-256, SHA-512, 100000 rounds.
    fn word() -> Settings {
        Settings { data_hash: Hash::Sha512, kdf_hash: Hash::Sha512, key_bits: 256, salt_len: 16, spin: 100_000, kdf: None }
    }

    fn of(k: &AgileKeys) -> Settings {
        // Salts stay within what one `encrypt` has random bytes for; the rounds stay in a range a save can afford.
        let spin = k.spin.clamp(100_000, 1_000_000);
        let salt_len = k.kdf_salt.len().clamp(16, 32);
        // The file's own spun key carries over only when saving would write exactly what it was spun with.
        let kdf = (spin == k.spin && salt_len == k.kdf_salt.len() && !k.spun.is_empty()).then(|| Kdf { salt: k.kdf_salt.clone(), spun: k.spun.clone() });
        Settings { data_hash: k.data_hash, kdf_hash: k.kdf_hash, key_bits: k.key_bits, salt_len, spin, kdf }
    }
}

/// Fresh Agile keys: a new data salt, a new verifier and a new package key for every save, so no two files share a
/// package key or a data IV. The password key (salt and 100000 rounds) is made on the first call and kept in `set`:
/// MS-OFFCRYPTO allows any saltValue for the key encryptor and Office only needs the password to unlock it, and
/// everything encrypted under that key (verifier, its hash, the package key) is fresh random each time.
fn fresh_keys(password: &str, set: &mut Settings, rng: &mut dyn FnMut(&mut [u8])) -> AgileKeys {
    let (hash, kdf_hash) = (set.data_hash, set.kdf_hash);
    let klen = set.key_bits / 8;
    let mut data_salt = vec![0u8; set.salt_len];
    let mut verifier = vec![0u8; set.salt_len];
    let mut secret = vec![0u8; klen];
    rng(&mut data_salt);
    rng(&mut verifier);
    rng(&mut secret);
    if set.kdf.is_none() {
        let mut salt = vec![0u8; set.salt_len];
        rng(&mut salt);
        let spun = spun_hash(kdf_hash, &salt, password, set.spin);
        set.kdf = Some(Kdf { salt, spun });
    }
    let Kdf { salt: kdf_salt, spun } = set.kdf.clone().expect("the password key was just made");
    let iv = fit(kdf_salt.clone(), 16);
    let enc = |block: &[u8; 8], data: &[u8]| {
        let key = Aes::new(&derived_key(kdf_hash, &spun, block, klen)).expect("a valid key length");
        key.cbc_encrypt(&iv, data)
    };
    let enc_verifier_input = enc(&VERIFIER_INPUT_BLOCK, &verifier);
    let enc_verifier_hash = enc(&VERIFIER_VALUE_BLOCK, &kdf_hash.digest(&[&verifier]));
    let enc_key_value = enc(&KEY_VALUE_BLOCK, &secret);
    AgileKeys {
        data_hash: hash,
        data_block: 16,
        data_salt,
        spin: set.spin,
        kdf_hash,
        kdf_block: 16,
        key_bits: set.key_bits,
        kdf_salt,
        enc_verifier_input,
        enc_verifier_hash,
        enc_key_value,
        secret,
        spun,
    }
}

// ---------------------------------------------------------------------------------------------
// Standard Encryption (Word 2007)

fn u32_at(b: &[u8], i: usize) -> Result<u32, CryptError> {
    b.get(i..i + 4).map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]])).ok_or_else(|| corrupt("the encryption header is cut short"))
}

fn standard_decrypt(info: &[u8], package: &[u8], password: &str) -> Result<Vec<u8>, CryptError> {
    let header_size = u32_at(info, 8)? as usize;
    let h = info.get(12..12 + header_size).ok_or_else(|| corrupt("the encryption header is cut short"))?;
    let alg = u32_at(h, 8)?;
    let alg_hash = u32_at(h, 12)?;
    let key_bits = u32_at(h, 16)? as usize;
    if !matches!(alg, 0x660E..=0x6610) {
        return Err(CryptError::Unsupported("RC4 encryption".into()));
    }
    if alg_hash != 0x8004 && alg_hash != 0 {
        return Err(CryptError::Unsupported("a hash other than SHA-1".into()));
    }
    if !matches!(key_bits, 128 | 192 | 256) {
        return Err(CryptError::Unsupported(format!("{key_bits}-bit keys")));
    }
    let v = &info[12 + header_size..];
    let salt_size = u32_at(v, 0)? as usize;
    let salt = v.get(4..4 + salt_size).ok_or_else(|| corrupt("the salt is cut short"))?;
    let enc_verifier = v.get(4 + salt_size..20 + salt_size).ok_or_else(|| corrupt("the verifier is cut short"))?;
    let enc_hash = v.get(24 + salt_size..24 + salt_size + 32).ok_or_else(|| corrupt("the verifier hash is cut short"))?;

    let mut h0 = Sha1::digest([salt, &utf16le(password)].concat()).to_vec();
    for i in 0..50_000u32 {
        h0 = Sha1::digest([&le32(i)[..], &h0].concat()).to_vec();
    }
    let hfinal = Sha1::digest([&h0[..], &le32(0)[..]].concat()).to_vec();
    let mixed = |pad: u8| {
        let mut buf = [pad; 64];
        for (b, k) in buf.iter_mut().zip(hfinal.iter()) {
            *b ^= k;
        }
        Sha1::digest(buf).to_vec()
    };
    let mut x3 = mixed(0x36);
    x3.extend(mixed(0x5c));
    let aes = Aes::new(&x3[..key_bits / 8])?;

    let verifier = aes.ecb_decrypt(enc_verifier);
    let verifier_hash = aes.ecb_decrypt(enc_hash);
    let want = Sha1::digest(&verifier);
    if verifier_hash[..20] != want[..] {
        return Err(CryptError::WrongPassword);
    }
    if package.len() < 8 {
        return Err(corrupt("the encrypted package is cut short"));
    }
    let size = u64::from_le_bytes(package[..8].try_into().expect("8 bytes")) as usize;
    let body = &package[8..];
    if !body.len().is_multiple_of(16) {
        return Err(corrupt("the encrypted package is not a whole number of blocks"));
    }
    let mut out = aes.ecb_decrypt(body);
    if size > out.len() {
        return Err(corrupt("the encrypted package is shorter than it says"));
    }
    out.truncate(size);
    Ok(out)
}

// ---------------------------------------------------------------------------------------------
// The compound file around it

/// The `EncryptionInfo` and `EncryptedPackage` streams.
type Streams = (Vec<u8>, Vec<u8>);

fn streams(bytes: &[u8]) -> Result<Option<Streams>, CryptError> {
    if !cfb::is_cfb(bytes) {
        return Ok(None);
    }
    let r = CfbReader::new(bytes)?;
    let info = r.stream("EncryptionInfo")?;
    let package = r.stream("EncryptedPackage")?;
    Ok(info.zip(package))
}

/// Whether the bytes are a password-protected Office file (a compound file with `EncryptedPackage`).
pub fn is_encrypted(bytes: &[u8]) -> bool {
    matches!(streams(bytes), Ok(Some(_)))
}

/// The protected file's original package and the session to save it again with the same password.
pub fn open(bytes: &[u8], password: &str) -> Result<(Vec<u8>, CryptSession), CryptError> {
    // A file is read once, from the same budget as any other (the unlocked package is read again by `open`).
    crate::budget::Budget::default().spend_bytes(bytes.len() as u64).map_err(|e| corrupt(&e.to_string()))?;
    let (info, package) = streams(bytes)?.ok_or(CryptError::NotEncrypted)?;
    if info.len() < 8 {
        return Err(corrupt("the encryption header is cut short"));
    }
    let (major, minor) = (u16::from_le_bytes([info[0], info[1]]), u16::from_le_bytes([info[2], info[3]]));
    match (major, minor) {
        (4, 4) => {
            let (data, keys) = agile_decrypt(&info[8..], &package, password)?;
            Ok((data, CryptSession { password: password.to_string(), settings: Some(keys) }))
        }
        (2..=4, 2) => {
            let data = standard_decrypt(&info, &package, password)?;
            Ok((data, CryptSession { password: password.to_string(), settings: None }))
        }
        _ => Err(CryptError::Unsupported(format!("encryption version {major}.{minor}"))),
    }
}

/// The package (a .docx zip) inside a protected file.
pub fn decrypt(bytes: &[u8], password: &str) -> Result<Vec<u8>, CryptError> {
    open(bytes, password).map(|(data, _)| data)
}

/// A password and the settings a file was protected with. Keep one for as long as a document is open. Every save
/// makes a new data salt, a new package key, verifier and integrity check, so two saves never share a package key
/// or an IV. The password key (its salt and the 100000 rounds of hashing) is made once per session, so saving
/// again is fast. Nothing here is ever written to storage.
pub struct CryptSession {
    password: String,
    settings: Option<Settings>,
}

impl CryptSession {
    pub fn new(password: &str) -> CryptSession {
        CryptSession { password: password.to_string(), settings: None }
    }

    pub fn password(&self) -> &str {
        &self.password
    }

    /// How many bytes of randomness one `encrypt` can use at most (three salts and a key of up to 32 bytes each,
    /// 64 for the key of the integrity check, and room to spare).
    pub const RANDOM_MAX: usize = 256;

    /// A protected file holding `zip` (a .docx package): Agile Encryption in a compound file.
    pub fn encrypt(&mut self, zip: &[u8], rng: &mut dyn FnMut(&mut [u8])) -> Vec<u8> {
        let mut set = self.settings.take().unwrap_or_else(Settings::word);
        let keys = fresh_keys(&self.password, &mut set, rng);
        self.settings = Some(set);
        let (package, xml) = agile_encrypt_package(&keys, zip, rng);
        let mut info = vec![4, 0, 4, 0, 0x40, 0, 0, 0];
        info.extend_from_slice(xml.as_bytes());
        cfb::write(&[
            ("\u{6}DataSpaces/Version", dataspaces_version()),
            ("\u{6}DataSpaces/DataSpaceMap", dataspace_map()),
            ("\u{6}DataSpaces/DataSpaceInfo/StrongEncryptionDataSpace", dataspace_info()),
            ("\u{6}DataSpaces/TransformInfo/StrongEncryptionTransform/\u{6}Primary", transform_primary()),
            ("EncryptionInfo", info),
            ("EncryptedPackage", package),
        ])
    }
}

/// A protected file holding `zip`, with a fresh session (this derives the key, which takes a moment).
pub fn encrypt(zip: &[u8], password: &str, rng: &mut dyn FnMut(&mut [u8])) -> Vec<u8> {
    CryptSession::new(password).encrypt(zip, rng)
}

/// Random bytes from the operating system, for native hosts (the CLI and the MCP server): `/dev/urandom` on
/// Unix and `RtlGenRandom` on Windows.
#[cfg(not(target_arch = "wasm32"))]
pub fn try_os_random(buf: &mut [u8]) -> Result<(), String> {
    #[cfg(windows)]
    {
        #[link(name = "advapi32")]
        unsafe extern "system" {
            // RtlGenRandom, exported as SystemFunction036.
            fn SystemFunction036(buffer: *mut u8, length: u32) -> u8;
        }
        for chunk in buf.chunks_mut(1 << 20) {
            // SAFETY: the pointer and length describe a live, writable slice.
            if unsafe { SystemFunction036(chunk.as_mut_ptr(), chunk.len() as u32) } == 0 {
                return Err("the system could not give random bytes".into());
            }
        }
        Ok(())
    }
    #[cfg(not(windows))]
    {
        use std::io::Read;
        std::fs::File::open("/dev/urandom").and_then(|mut f| f.read_exact(buf)).map_err(|e| format!("the system has no random source: {e}"))
    }
}

/// `try_os_random`, for callers that cannot report a failure; hosts check `try_os_random` first so it never panics.
#[cfg(not(target_arch = "wasm32"))]
pub fn os_random(buf: &mut [u8]) {
    try_os_random(buf).expect("the system has a random source");
}

/// A length-prefixed UTF-16 string padded to four bytes, as the data space streams hold them.
fn ustr(s: &str) -> Vec<u8> {
    let u = utf16le(s);
    let mut out = (u.len() as u32).to_le_bytes().to_vec();
    out.extend_from_slice(&u);
    while !out.len().is_multiple_of(4) {
        out.push(0);
    }
    out
}

fn dataspaces_version() -> Vec<u8> {
    let mut v = ustr("Microsoft.Container.DataSpaces");
    // Reader, updater and writer versions: 1.0.
    for _ in 0..3 {
        v.extend_from_slice(&[1, 0, 0, 0]);
    }
    v
}

fn dataspace_map() -> Vec<u8> {
    let mut entry = Vec::new();
    entry.extend_from_slice(&le32(1)); // one reference component
    entry.extend_from_slice(&le32(0)); // a stream
    entry.extend(ustr("EncryptedPackage"));
    entry.extend(ustr("StrongEncryptionDataSpace"));
    let mut v = Vec::new();
    v.extend_from_slice(&le32(8));
    v.extend_from_slice(&le32(1));
    v.extend_from_slice(&le32(entry.len() as u32 + 4));
    v.extend(entry);
    v
}

fn dataspace_info() -> Vec<u8> {
    let mut v = Vec::new();
    v.extend_from_slice(&le32(8));
    v.extend_from_slice(&le32(1));
    v.extend(ustr("StrongEncryptionTransform"));
    v
}

fn transform_primary() -> Vec<u8> {
    let id = ustr("{FF9A3F03-56EF-4613-BDD5-5A41C1D07246}");
    let mut v = Vec::new();
    v.extend_from_slice(&le32(8 + id.len() as u32));
    v.extend_from_slice(&le32(1));
    v.extend(id);
    v.extend(ustr("Microsoft.Container.EncryptionTransform"));
    for _ in 0..3 {
        v.extend_from_slice(&[1, 0, 0, 0]);
    }
    v.extend(ustr(""));
    v.extend_from_slice(&le32(0)); // block size
    v.extend_from_slice(&le32(0)); // cipher mode
    v.extend_from_slice(&le32(4));
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(s: &str) -> Vec<u8> {
        (0..s.len() / 2).map(|i| u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).unwrap()).collect()
    }

    /// A deterministic "random" source for tests.
    fn counter_rng() -> impl FnMut(&mut [u8]) {
        let mut n = 7u8;
        move |buf: &mut [u8]| {
            for b in buf.iter_mut() {
                n = n.wrapping_mul(73).wrapping_add(41);
                *b = n;
            }
        }
    }

    fn sample_zip() -> Vec<u8> {
        // Any bytes do for the crypto layer; use a real small package so the result also opens.
        let mut d = crate::Document::new_blank();
        d.save().unwrap()
    }

    #[test]
    fn hashes_match_published_vectors() {
        assert_eq!(Hash::Sha1.digest(&[b"abc"]), hex("a9993e364706816aba3e25717850c26c9cd0d89d"));
        assert_eq!(Hash::Sha256.digest(&[b"abc"]), hex("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"));
        assert_eq!(Hash::Sha384.digest(&[b"abc"]), hex("cb00753f45a35e8bb5a03d699ac65007272c32ab0eded1631a8b605a43ff5bed8086072ba1e7cc2358baeca134c825a7"));
        assert_eq!(
            Hash::Sha512.digest(&[b"a", b"bc"]),
            hex("ddaf35a193617abacc417349ae20413112e6fa4e89a97ea20a9eeee64b55d39a2192992a274fc1a836ba3c23a3feebbd454d4423643ce80e2a9ac94fa54ca49f")
        );
    }

    #[test]
    fn hmac_matches_rfc_4231() {
        // Test case 2: key "Jefe", data "what do ya want for nothing?".
        let key = b"Jefe";
        let data = b"what do ya want for nothing?";
        assert_eq!(Hash::Sha256.hmac(key, data), hex("5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843"));
        assert_eq!(
            Hash::Sha512.hmac(key, data),
            hex("164b7a7bfcf819e2e395fbe73b56e0a387bd64222e831fd610270cd7ea2505549758bf75c05a994a6d034f65f8f0e6fdcaeab1a34d4a6b4b636e070a38bce737")
        );
    }

    #[test]
    fn aes_matches_fips_197() {
        let key = hex("000102030405060708090a0b0c0d0e0f");
        let plain = hex("00112233445566778899aabbccddeeff");
        let mut block = plain.clone();
        Aes::new(&key).unwrap().enc(&mut block);
        assert_eq!(block, hex("69c4e0d86a7b0430d8cdb78070b4c55a"));
        let key256 = hex("000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f");
        let mut block = plain.clone();
        Aes::new(&key256).unwrap().enc(&mut block);
        assert_eq!(block, hex("8ea2b7ca516745bfeafc49904b496089"));
        Aes::new(&key256).unwrap().dec(&mut block);
        assert_eq!(block, plain);
    }

    #[test]
    fn cbc_matches_nist_sp800_38a() {
        let key = hex("2b7e151628aed2a6abf7158809cf4f3c");
        let iv = hex("000102030405060708090a0b0c0d0e0f");
        let plain = hex("6bc1bee22e409f96e93d7e117393172aae2d8a571e03ac9c9eb76fac45af8e51");
        let cipher = hex("7649abac8119b246cee98e9b12e9197d5086cb9b507219ee95db113a917678b2");
        let aes = Aes::new(&key).unwrap();
        assert_eq!(aes.cbc_encrypt(&iv, &plain), cipher);
        assert_eq!(aes.cbc_decrypt(&iv, &cipher), plain);
    }

    #[test]
    fn base64_round_trips() {
        for n in 0..10 {
            let data: Vec<u8> = (0..n).map(|i| (i as u8).wrapping_mul(37)).collect();
            assert_eq!(b64_decode(&b64_encode(&data)).unwrap(), data);
        }
        assert_eq!(b64_encode(b"Man"), "TWFu");
        assert_eq!(b64_decode("TWE=").unwrap(), b"Ma");
    }

    #[test]
    fn docxtor_encrypts_and_docxtor_decrypts() {
        let zip = sample_zip();
        let mut rng = counter_rng();
        let file = encrypt(&zip, "correct horse", &mut rng);
        assert!(cfb::is_cfb(&file));
        assert!(is_encrypted(&file));
        assert_eq!(decrypt(&file, "correct horse").unwrap(), zip);
        // The plain package is not readable in the file.
        assert!(!file.windows(4).any(|w| w == b"PK\x03\x04"));
    }

    #[test]
    fn a_wrong_password_is_told_apart() {
        let mut rng = counter_rng();
        let file = encrypt(&sample_zip(), "right", &mut rng);
        assert_eq!(decrypt(&file, "wrong").err(), Some(CryptError::WrongPassword));
        assert_eq!(decrypt(&file, "").err(), Some(CryptError::WrongPassword));
        assert_eq!(decrypt(&file, "Right").err(), Some(CryptError::WrongPassword));
    }

    #[test]
    fn every_save_has_its_own_salts_keys_and_ivs() {
        let mut rng_a = counter_rng();
        let mut session = CryptSession::new("pw");
        let one = session.encrypt(&sample_zip(), &mut rng_a);
        // A second save draws different randomness (a real source never repeats).
        let mut seed = 77u8;
        let mut rng_b = move |b: &mut [u8]| {
            for x in b.iter_mut() {
                seed = seed.wrapping_mul(13).wrapping_add(5);
                *x = seed;
            }
        };
        let two = session.encrypt(&sample_zip(), &mut rng_b);
        let info = |f: &[u8]| String::from_utf8_lossy(&CfbReader::new(f).unwrap().stream("EncryptionInfo").unwrap().unwrap()).to_string();
        let attr = |s: &str, name: &str| -> String {
            let at = s.find(&format!("{name}=\"")).unwrap() + name.len() + 2;
            s[at..at + s[at..].find('"').unwrap()].to_string()
        };
        let (i1, i2) = (info(&one), info(&two));
        // The data salt (keyData), the verifier and the encrypted package key all differ (the password salt stays
        // for the session, so the rounds run once),
        let key_data = |s: &str| s[s.find("<keyData").unwrap()..s.find("<dataIntegrity").unwrap()].to_string();
        assert_ne!(attr(&key_data(&i1), "saltValue"), attr(&key_data(&i2), "saltValue"));
        let key_part = |s: &str| s[s.find("<p:encryptedKey").unwrap()..].to_string();
        assert_ne!(attr(&key_part(&i1), "encryptedKeyValue"), attr(&key_part(&i2), "encryptedKeyValue"));
        assert_ne!(attr(&key_part(&i1), "encryptedVerifierHashInput"), attr(&key_part(&i2), "encryptedVerifierHashInput"));
        // so the same content encrypts to different bytes, from the first segment on.
        let package = |f: &[u8]| CfbReader::new(f).unwrap().stream("EncryptedPackage").unwrap().unwrap();
        assert_ne!(package(&one)[8..40], package(&two)[8..40]);
        assert_eq!(decrypt(&one, "pw").unwrap(), sample_zip());
        assert_eq!(decrypt(&two, "pw").unwrap(), sample_zip());
        // The same goes for a session that came from opening a file.
        let (_, mut again) = open(&one, "pw").unwrap();
        let three = again.encrypt(b"third", &mut rng_b);
        assert_ne!(attr(&key_part(&info(&three)), "encryptedKeyValue"), attr(&key_part(&i1), "encryptedKeyValue"));
        assert_ne!(attr(&key_data(&info(&three)), "saltValue"), attr(&key_data(&i1), "saltValue"));
        assert_eq!(decrypt(&three, "pw").unwrap(), b"third");
    }

    #[test]
    fn saving_again_does_not_run_the_password_rounds_again() {
        let attr = |s: &str, name: &str| -> String {
            let at = s.find(&format!("{name}=\"")).unwrap() + name.len() + 2;
            s[at..at + s[at..].find('"').unwrap()].to_string()
        };
        let info = |f: &[u8]| String::from_utf8_lossy(&CfbReader::new(f).unwrap().stream("EncryptionInfo").unwrap().unwrap()).to_string();
        // A xorshift stream, so no two draws repeat within the test (a one-byte generator would).
        let mut state = 0x9E37_79B9_7F4A_7C15u64;
        let mut rng = move |b: &mut [u8]| {
            for x in b.iter_mut() {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                *x = (state >> 24) as u8;
            }
        };
        let zip = sample_zip();
        let spins = || SPUN.with(|c| c.get());
        let mut session = CryptSession::new("pw");
        let (start, t) = (spins(), std::time::Instant::now());
        let first = session.encrypt(&zip, &mut rng);
        let first_time = t.elapsed();
        assert_eq!(spins() - start, 1, "the first save runs the rounds once");
        let (start, t) = (spins(), std::time::Instant::now());
        let mut files = vec![first];
        for _ in 0..3 {
            files.push(session.encrypt(&zip, &mut rng));
        }
        assert_eq!(spins() - start, 0, "later saves reuse the password key");
        // Three more saves cost far less than one first save (which hashes 100000 times).
        assert!(t.elapsed() < first_time, "{:?} vs {:?}", t.elapsed(), first_time);
        // Still new package keys, data salts and IVs every time, and every file opens with the password.
        let mut seen = std::collections::HashSet::new();
        for f in &files {
            let i = info(f);
            let (kd, kp) = (&i[i.find("<keyData").unwrap()..i.find("<dataIntegrity").unwrap()], &i[i.find("<p:encryptedKey").unwrap()..]);
            for v in [attr(kd, "saltValue"), attr(kp, "encryptedKeyValue"), attr(kp, "encryptedVerifierHashInput"), attr(&i, "encryptedHmacKey")] {
                assert!(seen.insert(v), "a value repeated between saves");
            }
            assert_eq!(decrypt(f, "pw").unwrap(), zip);
        }
        // A session opened from a file also skips the rounds once it has unlocked it.
        let (_, mut opened) = open(&files[0], "pw").unwrap();
        let start = spins();
        let again = opened.encrypt(&zip, &mut rng);
        opened.encrypt(&zip, &mut rng);
        assert_eq!(spins() - start, 0);
        assert_eq!(decrypt(&again, "pw").unwrap(), zip);
    }

    /// An agile file with the spin count of its password key replaced.
    fn with_spin(file: &[u8], spin: &str) -> Vec<u8> {
        let r = CfbReader::new(file).unwrap();
        let info = String::from_utf8(r.stream("EncryptionInfo").unwrap().unwrap()[8..].to_vec()).unwrap();
        let at = info.find("spinCount=\"").unwrap() + 11;
        let end = at + info[at..].find('"').unwrap();
        let mut out = vec![4, 0, 4, 0, 0x40, 0, 0, 0];
        out.extend_from_slice(format!("{}{spin}{}", &info[..at], &info[end..]).as_bytes());
        cfb::write(&[("EncryptionInfo", out), ("EncryptedPackage", r.stream("EncryptedPackage").unwrap().unwrap())])
    }

    #[test]
    fn a_huge_spin_count_is_refused_before_any_hashing() {
        let file = include_bytes!("../../../fixtures/encrypted/encrypted-agile.docx");
        let t = std::time::Instant::now();
        for spin in ["10000001", "4000000000", "4294967296", "18446744073709551615"] {
            let crafted = with_spin(file, spin);
            assert!(is_encrypted(&crafted));
            match open(&crafted, "any password").err() {
                Some(CryptError::Unsupported(m)) => assert!(m.contains("spin count"), "{m}"),
                other => panic!("{spin}: {other:?}"),
            }
        }
        assert!(t.elapsed().as_millis() < 500);
    }

    #[test]
    fn long_and_short_packages_round_trip() {
        let mut rng = counter_rng();
        let mut session = CryptSession::new("pw");
        for n in [0usize, 1, 15, 16, 17, 4095, 4096, 4097, 20_000] {
            let data: Vec<u8> = (0..n).map(|i| (i * 7 + 3) as u8).collect();
            let file = session.encrypt(&data, &mut rng);
            assert_eq!(decrypt(&file, "pw").unwrap(), data, "{n} bytes");
        }
    }

    #[test]
    fn changed_data_fails_the_integrity_check() {
        let mut rng = counter_rng();
        let file = encrypt(&sample_zip(), "pw", &mut rng);
        let r = CfbReader::new(&file).unwrap();
        let info = r.stream("EncryptionInfo").unwrap().unwrap();
        let mut package = r.stream("EncryptedPackage").unwrap().unwrap();
        let last = package.len() - 1;
        package[last] ^= 1;
        let tampered = cfb::write(&[("EncryptionInfo", info), ("EncryptedPackage", package)]);
        assert_eq!(decrypt(&tampered, "pw").err(), Some(CryptError::Integrity));
    }

    #[test]
    fn plain_files_are_not_encrypted() {
        let zip = sample_zip();
        assert!(!is_encrypted(&zip));
        assert_eq!(decrypt(&zip, "x").err(), Some(CryptError::NotEncrypted));
        assert!(!is_encrypted(b""));
        // A compound file without an EncryptedPackage (an old .doc) is not one of ours.
        let doc = cfb::write(&[("WordDocument", vec![1, 2, 3])]);
        assert!(!is_encrypted(&doc));
    }

    #[test]
    fn a_protected_file_opens_as_a_document() {
        let mut doc = crate::Document::new_blank();
        let bytes = doc.save().unwrap();
        let mut rng = counter_rng();
        let file = encrypt(&bytes, "open sesame", &mut rng);
        let plain = decrypt(&file, "open sesame").unwrap();
        let reopened = crate::Document::open(&plain).unwrap();
        assert!(crate::export::to_text(&reopened).trim().is_empty());
    }

    // Vectors made by other software (fixtures/encrypted/make_vectors.py): msoffcrypto-tool's Agile encryption, and
    // Agile AES-128/SHA-1 and Standard written from MS-OFFCRYPTO. All three hold the same one-paragraph document.

    const NOTE_WORD: &str = "marmalade";

    fn text_of(zip: &[u8]) -> String {
        let doc = crate::Document::open(zip).unwrap();
        crate::export::to_text(&doc)
    }

    #[test]
    fn opens_agile_encryption_made_by_another_tool() {
        let file = include_bytes!("../../../fixtures/encrypted/encrypted-agile.docx");
        assert!(is_encrypted(file));
        assert_eq!(decrypt(file, "nope").err(), Some(CryptError::WrongPassword));
        let zip = decrypt(file, "docxtor-test-pw").unwrap();
        assert!(text_of(&zip).contains(NOTE_WORD));
    }

    #[test]
    fn opens_standard_encryption_made_by_another_tool() {
        let file = include_bytes!("../../../fixtures/encrypted/encrypted-standard.docx");
        assert!(is_encrypted(file));
        assert_eq!(decrypt(file, "nope").err(), Some(CryptError::WrongPassword));
        let zip = decrypt(file, "docxtor-test-pw").unwrap();
        assert!(text_of(&zip).contains(NOTE_WORD));
        // Saving it again writes Agile Encryption.
        let (zip, mut session) = open(file, "docxtor-test-pw").unwrap();
        let mut rng = counter_rng();
        let again = session.encrypt(&zip, &mut rng);
        assert_eq!(decrypt(&again, "docxtor-test-pw").unwrap(), zip);
    }

    #[test]
    fn agile_files_with_other_settings_open() {
        let file = include_bytes!("../../../fixtures/encrypted/encrypted-agile-aes128-sha1.docx");
        let zip = decrypt(file, "docxtor-test-pw").unwrap();
        assert!(text_of(&zip).contains(NOTE_WORD));
        // And save again with the file's own settings.
        let (zip, mut session) = open(file, "docxtor-test-pw").unwrap();
        let mut rng = counter_rng();
        let again = session.encrypt(&zip, &mut rng);
        let info = CfbReader::new(&again).unwrap().stream("EncryptionInfo").unwrap().unwrap();
        let info = String::from_utf8_lossy(&info).to_string();
        assert!(info.contains("keyBits=\"128\"") && info.contains("hashAlgorithm=\"SHA1\""));
        assert_eq!(decrypt(&again, "docxtor-test-pw").unwrap(), zip);
    }
}
