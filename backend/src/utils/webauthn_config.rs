use webauthn_rs::prelude::*;
use std::sync::OnceLock;
use url::Url;

static WEBAUTHN: OnceLock<Webauthn> = OnceLock::new();

/// Get the global WebAuthn instance
pub fn get_webauthn() -> &'static Webauthn {
    WEBAUTHN.get_or_init(|| {
        let rp_id = std::env::var("WEBAUTHN_RP_ID")
            .unwrap_or_else(|_| "localhost".to_string());

        let rp_origin = std::env::var("WEBAUTHN_RP_ORIGIN")
            .unwrap_or_else(|_| "http://localhost:8080".to_string());

        let rp_origin_url = Url::parse(&rp_origin)
            .expect("WEBAUTHN_RP_ORIGIN must be a valid URL");

        let builder = WebauthnBuilder::new(&rp_id, &rp_origin_url)
            .expect("Failed to create WebAuthn builder")
            .rp_name("Lightfriend");

        builder.build().expect("Failed to build WebAuthn")
    })
}
