use base64::{Engine, engine::general_purpose::STANDARD};
use openssl::symm::{Cipher, decrypt};

use crate::proto::Credential;

const AOT_IV: [u8; 16] = [
    10, 2, 3, 252, 20, 73, 47, 218, 27, 234, 11, 236, 234, 37, 36, 54,
];
const AOT_KEY: [u8; 32] = [
    67, 109, 141, 146, 233, 119, 33, 86, 157, 228, 154, 109, 183, 13, 43, 160, 109, 180, 91, 173,
    73, 242, 107, 168, 6, 11, 74, 109, 84, 188, 176, 15,
];

pub async fn read(app_path: &str) -> Result<Credential, String> {
    let device_uuid = read_device_uuid(app_path).await?;
    let (access_token, refresh_token) = decrypt_aot_file(app_path).await?;
    Ok(Credential {
        access_token,
        refresh_token,
        device_uuid,
    })
}

async fn read_device_uuid(app_path: &str) -> Result<String, String> {
    let path = format!(
        "{}/shared_prefs/KakaoTalk.hw.perferences.xml",
        app_path.trim_end_matches('/')
    );
    let content = tokio::fs::read_to_string(&path)
        .await
        .map_err(|error| format!("failed to read {path}: {error}"))?;
    extract_xml_string(&content, "d_id")
        .ok_or_else(|| format!("failed to read {path}: missing d_id"))
}

async fn decrypt_aot_file(app_path: &str) -> Result<(String, String), String> {
    let path = format!("{}/aot", app_path.trim_end_matches('/'));
    let data = tokio::fs::read(&path)
        .await
        .map_err(|error| format!("failed to read {path}: {error}"))?;
    let encoded = read_length_prefixed_payload(&data, &path)?;
    let ciphertext = STANDARD
        .decode(encoded)
        .map_err(|error| format!("failed to decode {path}: {error}"))?;
    let plaintext = decrypt(Cipher::aes_256_cbc(), &AOT_KEY, Some(&AOT_IV), &ciphertext)
        .map_err(|error| format!("failed to decrypt {path}: {error}"))?;
    let json: serde_json::Value = serde_json::from_slice(&plaintext)
        .map_err(|error| format!("failed to parse {path}: {error}"))?;

    let access_token = json
        .get("access_token")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| format!("failed to parse {path}: missing access_token"))?
        .to_string();
    let refresh_token = json
        .get("refresh_token")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| format!("failed to parse {path}: missing refresh_token"))?
        .to_string();

    Ok((access_token, refresh_token))
}

fn read_length_prefixed_payload<'a>(data: &'a [u8], path: &str) -> Result<&'a [u8], String> {
    if data.len() < 2 {
        return Err(format!("failed to read {path}: missing length header"));
    }
    let encoded_len = u16::from_be_bytes([data[0], data[1]]) as usize;
    let encoded_end = 2 + encoded_len;
    data.get(2..encoded_end).ok_or_else(|| {
        format!(
            "failed to read {path}: length header {encoded_len} exceeds file size {}",
            data.len()
        )
    })
}

fn extract_xml_string(content: &str, name: &str) -> Option<String> {
    let start_tag = format!(r#"<string name="{name}">"#);
    content
        .split(&start_tag)
        .nth(1)
        .and_then(|value| value.split("</string>").next())
        .map(str::to_string)
}
