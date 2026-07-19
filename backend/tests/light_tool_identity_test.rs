use std::collections::HashSet;

use backend::services::light_tool_identity::{
    generate_device_token, hash_device_token, hash_installation_id, LightToolIdentityError,
    DEVICE_TOKEN_PREFIX,
};

#[test]
fn installation_ids_are_validated_and_canonicalized_before_hashing() {
    let canonical = "550e8400-e29b-41d4-a716-446655440000";
    let uppercase = "550E8400-E29B-41D4-A716-446655440000";

    let canonical_hash = hash_installation_id(canonical).unwrap();
    let uppercase_hash = hash_installation_id(uppercase).unwrap();

    assert_eq!(canonical_hash, uppercase_hash);
    assert_eq!(canonical_hash.len(), 64);
    assert!(canonical_hash.bytes().all(|byte| byte.is_ascii_hexdigit()));
    assert_eq!(
        hash_installation_id("not-a-uuid"),
        Err(LightToolIdentityError::InvalidInstallationId)
    );
}

#[test]
fn generated_device_tokens_are_well_formed_and_unique() {
    let mut tokens = HashSet::new();

    for _ in 0..100 {
        let generated = generate_device_token();

        assert!(generated.raw.starts_with(DEVICE_TOKEN_PREFIX));
        assert_eq!(generated.raw.len(), DEVICE_TOKEN_PREFIX.len() + 64);
        assert_eq!(hash_device_token(&generated.raw).unwrap(), generated.hash);
        assert!(tokens.insert(generated.raw));
    }
}

#[test]
fn malformed_device_tokens_are_rejected_before_hashing() {
    for malformed in [
        "",
        "wrong_0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        "lft_too-short",
        "lft_0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF",
        "lft_gggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggg",
    ] {
        assert_eq!(
            hash_device_token(malformed),
            Err(LightToolIdentityError::InvalidDeviceToken)
        );
    }
}
