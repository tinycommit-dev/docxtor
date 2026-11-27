"""Makes the three password-protected test files in this folder, with software other than Docxtor.

    python3 fixtures/encrypted/make_vectors.py <plain.docx>

encrypted-agile.docx is msoffcrypto-tool's own Agile encryption (AES-256, SHA-512). The other two are written here
from MS-OFFCRYPTO: Agile with AES-128 and SHA-1 (the descriptor XML on one line, as some writers save it) and
Standard encryption (AES-128, ECB, the key from SHA-1). msoffcrypto-tool's container writer holds all three, and
msoffcrypto-tool decrypts each one afterwards to check it. The password is PASSWORD; the Rust and browser tests use it.
Needs msoffcrypto-tool (pip install msoffcrypto-tool).
"""

import hashlib
import io
import os
import re
import secrets
import struct
import sys

import msoffcrypto
import msoffcrypto.method.ecma376_agile as agile
from cryptography.hazmat.primitives.ciphers import Cipher, algorithms, modes
from msoffcrypto.format.ooxml import OOXMLFile
from msoffcrypto.method.container.ecma376_encrypted import ECMA376Encrypted

PASSWORD = "docxtor-test-pw"
HERE = os.path.dirname(os.path.abspath(__file__))


def agile_default(plain):
    out = io.BytesIO()
    OOXMLFile(io.BytesIO(plain)).encrypt(PASSWORD, out)
    return out.getvalue()


def agile_aes128_sha1(plain):
    def params(self):
        self.cipherName, self.hashName = "AES", "SHA1"
        self.saltSize, self.blockSize, self.keyBits, self.hashSize = 16, 16, 128, 20
        self.saltValue = None

    descriptor = agile.ECMA376AgileEncryptionInfo.toEncryptionDescriptor
    saved = agile.ECMA376AgileCipherParams.__init__
    cbc = agile._encrypt_aes_cbc
    agile.ECMA376AgileCipherParams.__init__ = params
    # A SHA-1 hash is 20 bytes: MS-OFFCRYPTO 2.3.4.14 pads the HMAC key and value to the block size before encrypting
    agile._encrypt_aes_cbc = lambda data, key, iv: cbc(data + b"\0" * (-len(data) % 16), key, iv)
    agile.ECMA376AgileEncryptionInfo.toEncryptionDescriptor = lambda self: re.sub(
        r">\s+<", "><", re.sub(r"\s+/>", "/>", re.sub(r"\s+", " ", descriptor(self)))
    ).replace("?><", "?>\n<", 1)
    try:
        return agile.ECMA376Agile.encrypt(PASSWORD, io.BytesIO(plain))
    finally:
        agile.ECMA376AgileCipherParams.__init__ = saved
        agile.ECMA376AgileEncryptionInfo.toEncryptionDescriptor = descriptor
        agile._encrypt_aes_cbc = cbc


def aes_ecb(key, data):
    enc = Cipher(algorithms.AES(key), modes.ECB()).encryptor()
    return enc.update(data) + enc.finalize()


def standard(plain):
    # MS-OFFCRYPTO 2.3.4.7: H0 = SHA1(salt + password), 50,000 rounds, then the block number; 2.3.4.5 for the layout.
    salt = secrets.token_bytes(16)
    h = hashlib.sha1(salt + PASSWORD.encode("utf-16-le")).digest()
    for i in range(50000):
        h = hashlib.sha1(struct.pack("<I", i) + h).digest()
    h = hashlib.sha1(h + struct.pack("<I", 0)).digest()
    # 64 bytes of 0x36 (then 0x5C) with the hash XORed into the start
    x1 = hashlib.sha1(bytes(0x36 ^ b for b in h) + b"\x36" * 44).digest()
    x2 = hashlib.sha1(bytes(0x5C ^ b for b in h) + b"\x5c" * 44).digest()
    key = (x1 + x2)[:16]

    verifier = secrets.token_bytes(16)
    verifier_hash = hashlib.sha1(verifier).digest()
    csp = "Microsoft Enhanced RSA and AES Cryptographic Provider\0".encode("utf-16-le")
    flags = 0x24  # fCryptoAPI | fAES
    header = struct.pack("<IIIIIIII", flags, 0, 0x660E, 0x8004, 128, 0x18, 0, 0) + csp
    info = struct.pack("<HHI", 3, 2, flags) + struct.pack("<I", len(header)) + header
    info += struct.pack("<I", 16) + salt + aes_ecb(key, verifier)
    info += struct.pack("<I", 20) + aes_ecb(key, verifier_hash.ljust(32, b"\0"))

    padded = plain + b"\0" * (-len(plain) % 16)
    package = struct.pack("<Q", len(plain)) + aes_ecb(key, padded)
    out = io.BytesIO()
    ECMA376Encrypted(package, info).write_to(out)
    return out.getvalue()


def main():
    plain = open(sys.argv[1], "rb").read()
    for name, make in [
        ("encrypted-agile.docx", agile_default),
        ("encrypted-agile-aes128-sha1.docx", agile_aes128_sha1),
        ("encrypted-standard.docx", standard),
    ]:
        data = make(plain)
        check = msoffcrypto.OfficeFile(io.BytesIO(data))
        check.load_key(password=PASSWORD)
        back = io.BytesIO()
        check.decrypt(back)
        assert back.getvalue()[: len(plain)] == plain, name
        open(os.path.join(HERE, name), "wb").write(data)
        print(name, len(data), "bytes")


if __name__ == "__main__":
    main()
