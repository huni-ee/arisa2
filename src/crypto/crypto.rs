use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use base64::{Engine, engine::general_purpose::STANDARD};
use openssl::{
    hash::{MessageDigest, hash},
    symm::{Cipher, decrypt},
};

type KakaoKey = [u8; 32];

const PREFIX: [&str; 32] = [
    "",
    "",
    "12",
    "24",
    "18",
    "30",
    "36",
    "12",
    "48",
    "7",
    "35",
    "40",
    "17",
    "23",
    "29",
    "isabel",
    "kale",
    "sulli",
    "van",
    "merry",
    "kyle",
    "james",
    "maddux",
    "tony",
    "hayden",
    "paul",
    "elijah",
    "dorothy",
    "sally",
    "bran",
    "extr.ursra",
    "veil",
];
const KDF_PASSWORD: [u8; 34] = [
    0, 22, 0, 8, 0, 9, 0, 111, 0, 2, 0, 23, 0, 43, 0, 8, 0, 33, 0, 33, 0, 10, 0, 16, 0, 3, 0, 3, 0,
    7, 0, 6, 0, 0,
];
const KAKAO_IV: [u8; 16] = [
    15, 8, 1, 0, 25, 71, 37, 220, 21, 245, 23, 224, 225, 21, 12, 53,
];

#[derive(Clone)]
pub struct Decryptor {
    current_user_id: i64,
    keys: Arc<Mutex<HashMap<Vec<u8>, KakaoKey>>>,
}

impl Decryptor {
    pub fn new(current_user_id: i64) -> Self {
        Self {
            current_user_id,
            keys: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn decrypt(&self, ciphertext: &str, enc: u32) -> String {
        self.decrypt_for_user(ciphertext, enc, self.current_user_id)
    }

    pub fn decrypt_for_user(&self, ciphertext: &str, enc: u32, user_id: i64) -> String {
        self.try_decrypt(ciphertext, enc, user_id)
            .unwrap_or_else(|_| ciphertext.to_string())
    }

    fn try_decrypt(
        &self,
        ciphertext: &str,
        enc: u32,
        user_id: i64,
    ) -> Result<String, Box<dyn std::error::Error>> {
        if ciphertext.is_empty() || ciphertext == "{}" {
            return Err("invalid ciphertext".into());
        }
        let ciphertext = STANDARD.decode(ciphertext)?;
        if ciphertext.is_empty() {
            return Err("empty ciphertext".into());
        }
        let key = self.key(enc, user_id)?;
        let plaintext = decrypt(Cipher::aes_256_cbc(), &key, Some(&KAKAO_IV), &ciphertext)?;
        Ok(String::from_utf8_lossy(&plaintext).into_owned())
    }

    fn key(&self, enc: u32, user_id: i64) -> Result<KakaoKey, Box<dyn std::error::Error>> {
        let prefix = PREFIX
            .get(enc as usize)
            .ok_or_else(|| format!("invalid enc value: {enc}"))?;
        let mut salt = format!("{prefix}{user_id}").into_bytes();
        salt.resize(16, 0);

        let mut keys = self.keys.lock().map_err(|_| "decryptor lock poisoned")?;
        if let Some(key) = keys.get(&salt) {
            return Ok(*key);
        }

        let key = derive_key(&salt)?;
        keys.insert(salt, key);
        Ok(key)
    }
}

fn derive_key(salt: &[u8]) -> Result<KakaoKey, openssl::error::ErrorStack> {
    let bytes = pkcs12_sha1_kdf(&KDF_PASSWORD, salt, 1, 2, 32)?;
    Ok(bytes.try_into().expect("32-byte derived key"))
}

// RFC 7292 Appendix B, PKCS#12 KDF using SHA-1.
fn pkcs12_sha1_kdf(
    password: &[u8],
    salt: &[u8],
    id: u8,
    iterations: u32,
    key_length: usize,
) -> Result<Vec<u8>, openssl::error::ErrorStack> {
    const HASH_LENGTH: usize = 20;
    const BLOCK_LENGTH: usize = 64;

    fn pad(data: &[u8]) -> Vec<u8> {
        if data.is_empty() {
            return Vec::new();
        }
        let length = data.len().div_ceil(BLOCK_LENGTH) * BLOCK_LENGTH;
        (0..length).map(|index| data[index % data.len()]).collect()
    }

    let diversifier = vec![id; BLOCK_LENGTH];
    let mut input: Vec<u8> = pad(salt).into_iter().chain(pad(password)).collect();
    let blocks = key_length.div_ceil(HASH_LENGTH);
    let mut result = Vec::with_capacity(blocks * HASH_LENGTH);

    for _ in 0..blocks {
        let mut digest = hash(
            MessageDigest::sha1(),
            &[diversifier.as_slice(), input.as_slice()].concat(),
        )?;
        for _ in 1..iterations {
            digest = hash(MessageDigest::sha1(), &digest)?;
        }

        let repeated: Vec<u8> = (0..BLOCK_LENGTH)
            .map(|index| digest[index % HASH_LENGTH])
            .collect();
        for block in input.chunks_exact_mut(BLOCK_LENGTH) {
            let mut carry = 1u16;
            for index in (0..BLOCK_LENGTH).rev() {
                let sum = u16::from(block[index]) + u16::from(repeated[index]) + carry;
                block[index] = sum as u8;
                carry = sum >> 8;
            }
        }
        result.extend_from_slice(&digest);
    }

    result.truncate(key_length);
    Ok(result)
}
