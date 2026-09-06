use std::error::Error;

use openssl::{hash::MessageDigest, pkcs5};
use prost::Message;

mod preferences {
    include!(concat!(env!("OUT_DIR"), "/datastore.preferences.rs"));
}

pub fn kdf(salt_number: i64) -> Result<String, Box<dyn Error>> {
    let password: [u8; 16] = [
        4, 15, 81, 123, 77, 5, 23, 99, 2, 111, 10, 31, 54, 29, 109, 97,
    ];
    let mut salt = format!("se{salt_number}ed").into_bytes();
    salt.resize(16, 0);
    let mut key = vec![0; 32];
    pkcs5::pbkdf2_hmac(&password, &salt, 4096, MessageDigest::sha256(), &mut key)?;
    Ok(key.iter().map(|byte| format!("{byte:02x}")).collect())
}

pub fn extract_db_seed(data: &[u8]) -> Option<i64> {
    let map = preferences::PreferenceMap::decode(data).ok()?;
    match map
        .preferences
        .get("userDbPassPhraseSalt")?
        .value
        .as_ref()?
    {
        preferences::value::Value::Long(seed) => Some(*seed),
        _ => None,
    }
}
