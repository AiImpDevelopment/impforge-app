// SPDX-License-Identifier: MIT
//! Behavioral tests for crypto_lite Phase 1 type skeleton.

use impforge_app_lib::crypto_lite::{EncryptedBlob, KeyHandle};

#[test]
fn encrypted_blob_roundtrips_through_json() {
    let original = EncryptedBlob {
        ciphertext: vec![0xDE, 0xAD, 0xBE, 0xEF],
        nonce: [0u8; 12],
        kdf_salt: [1u8; 16],
    };
    let j = serde_json::to_string(&original).expect("serialize EncryptedBlob");
    let back: EncryptedBlob = serde_json::from_str(&j).expect("deserialize EncryptedBlob");
    assert_eq!(original.ciphertext, back.ciphertext);
    assert_eq!(original.nonce, back.nonce);
    assert_eq!(original.kdf_salt, back.kdf_salt);
}

#[test]
fn key_handle_roundtrips_through_json() {
    let original = KeyHandle("master-key-uuid".into());
    let j = serde_json::to_string(&original).expect("serialize KeyHandle");
    let back: KeyHandle = serde_json::from_str(&j).expect("deserialize KeyHandle");
    assert_eq!(original.0, back.0);
}

#[test]
fn types_implement_required_traits() {
    fn assert_traits<T: Clone + std::fmt::Debug + serde::Serialize + for<'de> serde::Deserialize<'de>>()
    {
    }
    assert_traits::<EncryptedBlob>();
    assert_traits::<KeyHandle>();
}
