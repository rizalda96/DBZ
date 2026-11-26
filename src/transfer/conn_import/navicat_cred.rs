//! Navicat 12.1+ passwords (the registry's `Pwd_2`): AES-256-GCM whose key comes from
//! the `navicat_cred` entry in Windows Credential Manager. That key is tied to the
//! machine and the Windows user, so these decode only where Navicat itself is installed.

use aes_gcm::aead::Aead;
use aes_gcm::{Aes256Gcm, KeyInit, Nonce};

use super::from_hex;

const NONCE_LEN: usize = 12;
const TAG_LEN: usize = 16;
const KEY_LEN: usize = 32;

/// The derived AES key for this machine, read once by the caller and reused.
pub(super) fn key() -> Option<[u8; KEY_LEN]> {
    derive_key(&credential_blob()?)
}

/// Decrypt one hex-encoded `Pwd_2` with an already-read key.
pub(super) fn v3_decrypt_hex(key: &[u8; KEY_LEN], hex: &str) -> Option<String> {
    v3_decrypt(key, &from_hex(hex)?)
}

/// Navicat overwrites four bytes of the raw credential at a fixed spot ("2025" mapped
/// through a 1-based table); the rest is the key as stored.
fn derive_key(cred: &[u8]) -> Option<[u8; KEY_LEN]> {
    let mut key: [u8; KEY_LEN] = cred.try_into().ok()?;
    key[25..29].copy_from_slice(&[3, 1, 3, 6]);
    Some(key)
}

/// `nonce (12) | ciphertext | tag (16)` → the password, or None if the tag doesn't
/// verify (wrong machine) or the plaintext isn't valid UTF-8.
fn v3_decrypt(key: &[u8; KEY_LEN], data: &[u8]) -> Option<String> {
    if data.len() < NONCE_LEN + TAG_LEN {
        return None;
    }
    // aes-gcm expects the tag appended to the ciphertext, which is already the layout here.
    let (nonce, body) = data.split_at(NONCE_LEN);
    let cipher = Aes256Gcm::new_from_slice(key).ok()?;
    let padded = cipher.decrypt(Nonce::from_slice(nonce), body).ok()?;
    unpad(&padded)
}

/// The plaintext is a little-endian u16 length, the password, then random padding.
fn unpad(padded: &[u8]) -> Option<String> {
    let len = usize::from(u16::from_le_bytes([*padded.first()?, *padded.get(1)?]));
    String::from_utf8(padded.get(2..2 + len)?.to_vec()).ok()
}

/// The `navicat_cred` blob (64 hex characters) decoded to its 32 bytes.
fn credential_blob() -> Option<Vec<u8>> {
    from_hex(std::str::from_utf8(&read_credential("navicat_cred")?).ok()?)
}

#[cfg(windows)]
fn read_credential(name: &str) -> Option<Vec<u8>> {
    use windows_sys::Win32::Security::Credentials::{CredFree, CredReadW, CREDENTIALW, CRED_TYPE_GENERIC};
    let wide: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
    let mut cred: *mut CREDENTIALW = std::ptr::null_mut();
    // SAFETY: `wide` is a valid NUL-terminated UTF-16 string that outlives the call.
    // On success CredReadW gives one allocation we hand back to CredFree; the blob is
    // read only while that pointer is live, and copied out before it is freed.
    unsafe {
        if CredReadW(wide.as_ptr(), CRED_TYPE_GENERIC, 0, &mut cred) == 0 {
            return None;
        }
        let c = &*cred;
        let blob = (!c.CredentialBlob.is_null()).then(|| std::slice::from_raw_parts(c.CredentialBlob, c.CredentialBlobSize as usize).to_vec());
        CredFree(cred.cast());
        blob
    }
}

#[cfg(not(windows))]
fn read_credential(_name: &str) -> Option<Vec<u8>> {
    None
}

#[cfg(test)]
mod tests {
    use super::{derive_key, v3_decrypt};

    // The published v3 vector: this credential and ciphertext decrypt to "dummy123456".
    const CRED: [u8; 32] = [
        0xf2, 0xbf, 0x93, 0xf1, 0x4d, 0x48, 0x7e, 0x3c, 0x24, 0x04, 0xee, 0xf3, 0x87, 0x7b, 0x5a, 0x4c, 0x6d, 0x1e, 0x2d, 0x03, 0x34, 0x2a, 0xa5, 0x12, 0x83, 0xa6, 0x78, 0x62,
        0x97, 0xb2, 0x65, 0x10,
    ];

    fn hex(s: &str) -> Vec<u8> {
        (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap()).collect()
    }

    #[test]
    fn known_v3_vector_decrypts() {
        let key = derive_key(&CRED).expect("32-byte credential");
        let data = hex("930868109524B44DBB875A8712647C50986810B391314EFD20ABE412629B8CCA49A0A59711BBA793479D433A");
        assert_eq!(v3_decrypt(&key, &data).as_deref(), Some("dummy123456"));
    }

    #[test]
    fn a_wrong_key_fails_the_tag_rather_than_returning_junk() {
        let mut bad = CRED;
        bad[0] ^= 1;
        let key = derive_key(&bad).expect("32-byte credential");
        let data = hex("930868109524B44DBB875A8712647C50986810B391314EFD20ABE412629B8CCA49A0A59711BBA793479D433A");
        assert_eq!(v3_decrypt(&key, &data), None);
    }
}
