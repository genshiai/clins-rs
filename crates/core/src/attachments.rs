// SPDX-License-Identifier: Apache-2.0
//! Attachment byte-level checks, independent of remote content fetching.
use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::Value;
use sha1::{Digest, Sha1};
pub fn check(attachment: &Value) -> Vec<String> {
    let mut errors = Vec::new();
    let Some(data) = attachment["data"].as_str() else {
        return errors;
    };
    let compact: String = data.chars().filter(|c| !c.is_ascii_whitespace()).collect();
    let Ok(bytes) = STANDARD.decode(compact) else {
        errors.push("Attachment.dataのbase64エンコードが不正です".into());
        return errors;
    };
    if let Some(size) = attachment["size"].as_u64()
        && size != bytes.len() as u64
    {
        errors.push(format!(
            "Attachment.size={size}と実際のデータ長{}が一致しません",
            bytes.len()
        ));
    }
    if let Some(hash) = attachment["hash"].as_str() {
        let compact: String = hash.chars().filter(|c| !c.is_ascii_whitespace()).collect();
        if STANDARD.decode(compact).ok().as_deref() != Some(Sha1::digest(&bytes).as_slice()) {
            errors.push("Attachment.hashがdataのSHA-1チェックサムと一致しません".into());
        }
    }
    errors
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn size_and_hash_cover_decoded_bytes_and_base64_whitespace() {
        let mut a = json!({"data":"dGVz dA==\n","size":4,"hash":"qUqP5cyxm6YcTAhz05Hph5gvu9M="});
        assert!(check(&a).is_empty());
        a["size"] = json!(5);
        assert_eq!(check(&a).len(), 1);
        a["hash"] = json!("AAAAAAAAAAAAAAAAAAAAAAAAAAA=");
        assert_eq!(check(&a).len(), 2);
        a["data"] = json!("@@@");
        assert_eq!(check(&a).len(), 1);
    }
}
