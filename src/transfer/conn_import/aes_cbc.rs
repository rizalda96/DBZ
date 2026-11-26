//! AES-128-CBC with PKCS#7 padding, as Navicat 12+ and DBeaver store passwords.

use aes::cipher::generic_array::GenericArray;
use aes::cipher::{BlockDecrypt, KeyInit};
use aes::Aes128;

const BLOCK: usize = 16;

/// The plain bytes, or None when `data` is not whole blocks or its padding is wrong
/// (a wrong key or a different cipher).
pub(super) fn decrypt(key: &[u8; BLOCK], iv: &[u8; BLOCK], data: &[u8]) -> Option<Vec<u8>> {
    if data.is_empty() || data.len() % BLOCK != 0 {
        return None;
    }
    let aes = Aes128::new(GenericArray::from_slice(key));
    let mut prev = *iv;
    let mut out = Vec::with_capacity(data.len());
    for block in data.chunks_exact(BLOCK) {
        let mut plain = GenericArray::clone_from_slice(block);
        aes.decrypt_block(&mut plain);
        out.extend(plain.iter().zip(prev).map(|(p, c)| p ^ c));
        prev.copy_from_slice(block);
    }
    unpad(out)
}

fn unpad(mut data: Vec<u8>) -> Option<Vec<u8>> {
    let n = usize::from(*data.last()?);
    let body = data.len().checked_sub(n)?;
    if n == 0 || n > BLOCK || data[body..].iter().any(|b| usize::from(*b) != n) {
        return None;
    }
    data.truncate(body);
    Some(data)
}

/// The other direction, for building test data.
#[cfg(test)]
pub(super) fn encrypt(key: &[u8; BLOCK], iv: &[u8; BLOCK], plain: &[u8]) -> Vec<u8> {
    use aes::cipher::BlockEncrypt;
    let aes = Aes128::new(GenericArray::from_slice(key));
    let pad = BLOCK - plain.len() % BLOCK;
    let mut data = plain.to_vec();
    data.extend(std::iter::repeat(pad as u8).take(pad));
    let mut prev = *iv;
    let mut out = Vec::with_capacity(data.len());
    for block in data.chunks_exact(BLOCK) {
        let mut b = GenericArray::clone_from_slice(block);
        b.iter_mut().zip(prev).for_each(|(x, p)| *x ^= p);
        aes.encrypt_block(&mut b);
        prev.copy_from_slice(&b);
        out.extend_from_slice(&b);
    }
    out
}
