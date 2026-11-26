//! Navicat's two password encodings, both written as hex: AES-128-CBC with fixed key
//! and IV (Navicat 12 and later, used in exported `.ncx` files), and Blowfish with a
//! home-made chaining (Navicat 11, still used in the Windows registry).

use aes::cipher::generic_array::GenericArray;
use aes::cipher::{BlockDecrypt, BlockEncrypt, KeyInit};
use blowfish::Blowfish;
use sha1::{Digest, Sha1};

use super::{aes_cbc, from_hex};

const AES_KEY: &[u8; 16] = b"libcckeylibcckey";
const AES_IV: &[u8; 16] = b"libcciv libcciv ";
/// The Blowfish key is the SHA-1 of this string.
const BLOWFISH_SEED: &[u8] = b"3DC5CA39";
const BF_BLOCK: usize = 8;

/// The password behind Navicat's hex text. AES is tried first because a wrong guess is
/// caught by its padding; Blowfish decodes anything.
pub(super) fn decrypt(hex: &str) -> String {
    let Some(data) = from_hex(hex).filter(|d| !d.is_empty()) else { return String::new() };
    let aes = aes_cbc::decrypt(AES_KEY, AES_IV, &data).and_then(|p| String::from_utf8(p).ok());
    aes.unwrap_or_else(|| String::from_utf8_lossy(&blowfish_decrypt(&data)).into_owned())
}

fn cipher() -> Blowfish {
    Blowfish::new_from_slice(&Sha1::digest(BLOWFISH_SEED)).expect("a 20-byte key is within Blowfish's 4..56 byte range")
}

fn encrypt_block(bf: &Blowfish, block: &mut [u8; BF_BLOCK]) {
    bf.encrypt_block(GenericArray::from_mut_slice(block));
}

fn xor_into(target: &mut [u8; BF_BLOCK], other: &[u8]) {
    target.iter_mut().zip(other).for_each(|(t, o)| *t ^= o);
}

/// The chain value starts as the encrypted all-ones block. Each whole block is
/// Blowfish-decrypted and XORed with it, then the ciphertext is folded into it; a short
/// tail is XORed with the chain value encrypted once more.
fn blowfish_decrypt(data: &[u8]) -> Vec<u8> {
    let bf = cipher();
    let mut chain = [0xFF; BF_BLOCK];
    encrypt_block(&bf, &mut chain);
    let mut out = Vec::with_capacity(data.len());
    let mut blocks = data.chunks_exact(BF_BLOCK);
    for block in &mut blocks {
        let mut plain = GenericArray::clone_from_slice(block);
        bf.decrypt_block(&mut plain);
        out.extend(plain.iter().zip(chain).map(|(p, c)| p ^ c));
        xor_into(&mut chain, block);
    }
    let tail = blocks.remainder();
    if !tail.is_empty() {
        encrypt_block(&bf, &mut chain);
        out.extend(tail.iter().zip(chain).map(|(t, c)| t ^ c));
    }
    out
}

/// The other direction, for building test data.
#[cfg(test)]
pub(super) fn blowfish_encrypt(plain: &[u8]) -> Vec<u8> {
    let bf = cipher();
    let mut chain = [0xFF; BF_BLOCK];
    encrypt_block(&bf, &mut chain);
    let mut out = Vec::with_capacity(plain.len());
    let mut blocks = plain.chunks_exact(BF_BLOCK);
    for block in &mut blocks {
        let mut b = chain;
        xor_into(&mut b, block);
        encrypt_block(&bf, &mut b);
        xor_into(&mut chain, &b);
        out.extend_from_slice(&b);
    }
    let tail = blocks.remainder();
    if !tail.is_empty() {
        encrypt_block(&bf, &mut chain);
        out.extend(tail.iter().zip(chain).map(|(t, c)| t ^ c));
    }
    out
}

#[cfg(test)]
pub(super) fn aes_encrypt(plain: &[u8]) -> Vec<u8> {
    aes_cbc::encrypt(AES_KEY, AES_IV, plain)
}
