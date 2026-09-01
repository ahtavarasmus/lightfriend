#[test]
fn production_default_does_not_enable_application_debug_logs() {
    let source = include_str!("../src/main.rs");

    assert!(source.contains("EnvFilter::new(\"info\")"));
    assert!(!source.contains("info,lightfriend=debug"));
}

#[test]
fn known_sensitive_payload_log_sinks_do_not_return() {
    let checks = [
        (
            "ai_config.rs",
            include_str!("../src/ai_config.rs"),
            &[
                "MODEL REASONING",
                "response {}",
                "{}: {}\", status, text",
                "Streaming error: {}",
            ] as &[&str],
        ),
        (
            "system_behaviors.rs",
            include_str!("../src/proactive/system_behaviors.rs"),
            &["preview={:?}", "created.description"],
        ),
        (
            "tool_call_utils/bridge.rs",
            include_str!("../src/tool_call_utils/bridge.rs"),
            &[
                "raw_args={}",
                "display_name={:?}",
                "recipient={}",
                "contact={}",
                "chat_name={}",
                "room_id={:?}",
                "chat_id={:?}",
            ],
        ),
        (
            "matrix_auth.rs",
            include_str!("../src/utils/matrix_auth.rs"),
            &["room {}: {:?}"],
        ),
        (
            "profile_handlers.rs",
            include_str!("../src/handlers/profile_handlers.rs"),
            &["response message (first 500 chars)"],
        ),
        (
            "utils/bridge.rs",
            include_str!("../src/utils/bridge.rs"),
            &[
                "management room message: {:?}",
                "content={:?}",
                "list-logins body",
                "sending {:?}",
            ],
        ),
        (
            "telegram_auth.rs",
            include_str!("../src/handlers/telegram_auth.rs"),
            &["last_bot_body", "bot_msg#{} body={:?}"],
        ),
        (
            "voice_pipeline.rs",
            include_str!("../src/api/voice_pipeline.rs"),
            &["error event: {}", "event: {} / {}"],
        ),
    ];

    for (file, source, forbidden_patterns) in checks {
        for pattern in forbidden_patterns {
            assert!(
                !source.contains(pattern),
                "{file} contains forbidden sensitive log pattern: {pattern}"
            );
        }
    }
}

#[test]
fn admin_log_viewer_does_not_expose_third_party_service_logs() {
    let source = include_str!("../src/handlers/admin_handlers.rs");

    for forbidden_program in [
        "\"telegram\" | \"mautrix-telegram\" => \"telegram\"",
        "\"whatsapp\" | \"mautrix-whatsapp\" => \"whatsapp\"",
        "\"signal\" | \"mautrix-signal\" => \"signal\"",
        "\"tuwunel\" => \"tuwunel\"",
    ] {
        assert!(
            !source.contains(forbidden_program),
            "admin log viewer exposes third-party log mapping: {forbidden_program}"
        );
    }

    assert!(source.contains("\"lightfriend\" => \"lightfriend\""));
}
