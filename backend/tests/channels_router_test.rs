use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use backend::channels::router::ChannelRouter;
use backend::channels::traits::{ChannelError, ChannelMessageId, MediaRef, MessageChannel};
use backend::models::user_models::User;

/// Minimal test channel that records every send call and returns a stub id.
struct RecordingChannel {
    id: &'static str,
    sends: AtomicUsize,
}

impl RecordingChannel {
    fn new(id: &'static str) -> Self {
        Self {
            id,
            sends: AtomicUsize::new(0),
        }
    }

    fn send_count(&self) -> usize {
        self.sends.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl MessageChannel for RecordingChannel {
    fn id(&self) -> &'static str {
        self.id
    }

    async fn send(
        &self,
        _user: &User,
        _address: &str,
        _body: &str,
        _media: Option<MediaRef>,
    ) -> Result<ChannelMessageId, ChannelError> {
        self.sends.fetch_add(1, Ordering::SeqCst);
        Ok(ChannelMessageId(format!(
            "{}-{}",
            self.id,
            self.send_count()
        )))
    }
}

fn user_with_phone(phone: &str) -> User {
    user_with(phone, None)
}

fn user_with(phone: &str, preferred: Option<&str>) -> User {
    User {
        id: 1,
        email: "test@example.com".to_string(),
        password_hash: String::new(),
        phone_number: phone.to_string(),
        nickname: None,
        time_to_live: None,
        credits: 0.0,
        preferred_number: None,
        charge_when_under: false,
        charge_back_to: None,
        stripe_customer_id: None,
        stripe_payment_method_id: None,
        stripe_checkout_session_id: None,
        sub_tier: None,
        credits_left: 0.0,
        last_credits_notification: None,
        next_billing_date_timestamp: None,
        magic_token: None,
        refresh_token_hash: None,
        refresh_token_compromised: false,
        magic_token_expires_at: None,
        plan_type: None,
        own_twilio_enabled: false,
        matrix_e2ee_enabled: false,
        preferred_sms_provider: preferred.map(|s| s.to_string()),
        accountability_friend_phone: None,
        accountability_friend_name: None,
        accountability_enabled: false,
        included_usage_window_start_timestamp: None,
        included_usage_window_end_timestamp: None,
    }
}

#[tokio::test]
#[serial_test::serial]
async fn us_user_routes_to_twilio_when_registered() {
    // Default policy: US users go to twilio. Telnyx is the documented
    // backup channel but only used when twilio is not registered.
    let twilio = Arc::new(RecordingChannel::new("twilio"));
    let telnyx = Arc::new(RecordingChannel::new("telnyx"));
    let mut router = ChannelRouter::new();
    router.register(twilio.clone());
    router.register(telnyx.clone());

    let user = user_with_phone("+12025551234");
    let result = router.send_to_user(&user, "hello", None).await.unwrap();

    assert_eq!(result.as_str(), "twilio-1");
    assert_eq!(twilio.send_count(), 1);
    assert_eq!(telnyx.send_count(), 0);
}

#[tokio::test]
#[serial_test::serial]
async fn us_user_falls_back_to_telnyx_when_no_twilio() {
    // Twilio creds not configured → US users fall back to telnyx
    // (the documented backup channel).
    let telnyx = Arc::new(RecordingChannel::new("telnyx"));
    let mut router = ChannelRouter::new();
    router.register(telnyx.clone());

    let user = user_with_phone("+12025551234");
    let result = router.send_to_user(&user, "hello", None).await.unwrap();

    assert_eq!(result.as_str(), "telnyx-1");
    assert_eq!(telnyx.send_count(), 1);
}

#[tokio::test]
#[serial_test::serial]
async fn us_user_uses_twilio_when_only_twilio_registered() {
    let twilio = Arc::new(RecordingChannel::new("twilio"));
    let mut router = ChannelRouter::new();
    router.register(twilio.clone());

    let user = user_with_phone("+12025551234");
    let result = router.send_to_user(&user, "hello", None).await.unwrap();

    assert_eq!(result.as_str(), "twilio-1");
    assert_eq!(twilio.send_count(), 1);
}

#[tokio::test]
#[serial_test::serial]
async fn non_us_user_always_uses_twilio_even_with_telnyx_registered() {
    let twilio = Arc::new(RecordingChannel::new("twilio"));
    let telnyx = Arc::new(RecordingChannel::new("telnyx"));
    let mut router = ChannelRouter::new();
    router.register(twilio.clone());
    router.register(telnyx.clone());

    // Finnish number
    let user = user_with_phone("+358401234567");
    let result = router.send_to_user(&user, "hello", None).await.unwrap();

    assert_eq!(result.as_str(), "twilio-1");
    assert_eq!(twilio.send_count(), 1);
    assert_eq!(telnyx.send_count(), 0);
}

#[tokio::test]
#[serial_test::serial]
async fn twilio_preferred_over_telnyx_and_sinch_for_us() {
    // All three channels registered → twilio wins for US users.
    // Sinch and telnyx are reachable only via explicit per-user pin.
    let twilio = Arc::new(RecordingChannel::new("twilio"));
    let telnyx = Arc::new(RecordingChannel::new("telnyx"));
    let sinch = Arc::new(RecordingChannel::new("sinch"));
    let mut router = ChannelRouter::new();
    router.register(twilio.clone());
    router.register(telnyx.clone());
    router.register(sinch.clone());

    let user = user_with_phone("+12025551234");
    router.send_to_user(&user, "hello", None).await.unwrap();

    assert_eq!(twilio.send_count(), 1);
    assert_eq!(telnyx.send_count(), 0);
    assert_eq!(sinch.send_count(), 0);
}

#[tokio::test]
#[serial_test::serial]
async fn preferred_provider_overrides_country_routing() {
    // Finnish phone but pinned to sinch (admin verification path)
    let twilio = Arc::new(RecordingChannel::new("twilio"));
    let sinch = Arc::new(RecordingChannel::new("sinch"));
    let mut router = ChannelRouter::new();
    router.register(twilio.clone());
    router.register(sinch.clone());

    let user = user_with("+358401234567", Some("sinch"));
    router.send_to_user(&user, "hello", None).await.unwrap();

    assert_eq!(sinch.send_count(), 1);
    assert_eq!(twilio.send_count(), 0);
}

#[tokio::test]
#[serial_test::serial]
async fn preferred_provider_falls_back_when_unregistered() {
    // User pinned to sinch but Sinch isn't registered → falls back to
    // country-based routing. Finnish user → twilio.
    let twilio = Arc::new(RecordingChannel::new("twilio"));
    let mut router = ChannelRouter::new();
    router.register(twilio.clone());

    let user = user_with("+358401234567", Some("sinch"));
    router.send_to_user(&user, "hello", None).await.unwrap();

    assert_eq!(twilio.send_count(), 1);
}

#[tokio::test]
#[serial_test::serial]
async fn preferred_provider_unknown_value_falls_back() {
    // Garbage preference value (e.g. operator typo) → fall back to
    // country-based routing rather than failing the send.
    let twilio = Arc::new(RecordingChannel::new("twilio"));
    let mut router = ChannelRouter::new();
    router.register(twilio.clone());

    let user = user_with("+358401234567", Some("nonsense"));
    router.send_to_user(&user, "hello", None).await.unwrap();

    assert_eq!(twilio.send_count(), 1);
}

#[tokio::test]
#[serial_test::serial]
async fn preferred_provider_can_pin_us_user_to_twilio() {
    // US user with explicit twilio preference → twilio, not sinch/telnyx.
    let twilio = Arc::new(RecordingChannel::new("twilio"));
    let telnyx = Arc::new(RecordingChannel::new("telnyx"));
    let sinch = Arc::new(RecordingChannel::new("sinch"));
    let mut router = ChannelRouter::new();
    router.register(twilio.clone());
    router.register(telnyx.clone());
    router.register(sinch.clone());

    let user = user_with("+12025551234", Some("twilio"));
    router.send_to_user(&user, "hello", None).await.unwrap();

    assert_eq!(twilio.send_count(), 1);
    assert_eq!(telnyx.send_count(), 0);
    assert_eq!(sinch.send_count(), 0);
}

#[tokio::test]
#[serial_test::serial]
async fn missing_channel_returns_not_configured() {
    let router = ChannelRouter::new();
    let user = user_with_phone("+12025551234");
    let err = router.send_to_user(&user, "hello", None).await.unwrap_err();
    assert!(matches!(err, ChannelError::NotConfigured(_)));
}

#[tokio::test]
#[serial_test::serial]
async fn empty_body_with_no_media_is_refused_before_dispatch() {
    let twilio = Arc::new(RecordingChannel::new("twilio"));
    let mut router = ChannelRouter::new();
    router.register(twilio.clone());

    let user = user_with_phone("+12025551234");
    let err = router.send_to_user(&user, "   ", None).await.unwrap_err();
    assert!(matches!(err, ChannelError::SendFailed(_)));
    // Channel must NOT be invoked when body is empty
    assert_eq!(twilio.send_count(), 0);
}

#[tokio::test]
#[serial_test::serial]
async fn dev_mode_skips_dispatch_and_returns_stub_id() {
    let twilio = Arc::new(RecordingChannel::new("twilio"));
    let mut router = ChannelRouter::new();
    router.register(twilio.clone());

    let original_env = std::env::var("ENVIRONMENT").ok();
    std::env::set_var("ENVIRONMENT", "development");

    let user = user_with_phone("+12025551234");
    let result = router.send_to_user(&user, "hello in dev", None).await;

    // Restore env before asserting so a panic doesn't leak state into other tests
    match original_env {
        Some(val) => std::env::set_var("ENVIRONMENT", val),
        None => std::env::remove_var("ENVIRONMENT"),
    }

    let id = result.expect("dev mode should still return Ok with a stub id");
    assert_eq!(id.as_str(), "dev_not_sending");
    assert_eq!(
        twilio.send_count(),
        0,
        "channel must not be dispatched in dev mode"
    );
}

#[tokio::test]
#[serial_test::serial]
async fn url_filter_runs_before_channel_dispatch() {
    use std::sync::Mutex;

    /// Channel that captures the body it receives so we can assert the
    /// router's preprocessing actually ran.
    struct CapturingChannel {
        last_body: Mutex<String>,
    }
    #[async_trait]
    impl MessageChannel for CapturingChannel {
        fn id(&self) -> &'static str {
            "twilio"
        }
        async fn send(
            &self,
            _user: &User,
            _address: &str,
            body: &str,
            _media: Option<MediaRef>,
        ) -> Result<ChannelMessageId, ChannelError> {
            *self.last_body.lock().unwrap() = body.to_string();
            Ok(ChannelMessageId("captured".to_string()))
        }
    }

    let chan = Arc::new(CapturingChannel {
        last_body: Mutex::new(String::new()),
    });
    let mut router = ChannelRouter::new();
    router.register(chan.clone());

    let user = user_with_phone("+12025551234");
    router
        .send_to_user(
            &user,
            "Email user(at)gmail.com about https://bit.ly/abc",
            None,
        )
        .await
        .unwrap();

    let received = chan.last_body.lock().unwrap().clone();
    // Defang re-fanged + shortener replaced
    assert_eq!(received, "Email user@gmail.com about [link]");
}

#[test]
fn pick_channel_returns_correct_id() {
    let twilio = Arc::new(RecordingChannel::new("twilio"));
    let telnyx = Arc::new(RecordingChannel::new("telnyx"));
    let mut router = ChannelRouter::new();
    router.register(twilio);
    router.register(telnyx);

    assert_eq!(
        router.pick_channel_for(&user_with_phone("+12025551234")),
        "twilio"
    );
    assert_eq!(
        router.pick_channel_for(&user_with_phone("+358401234567")),
        "twilio"
    );
    assert_eq!(
        router.pick_channel_for(&user_with_phone("+447911123456")),
        "twilio"
    );
}

#[test]
fn pick_channel_us_falls_back_to_telnyx_when_twilio_missing() {
    // If twilio isn't registered (env not set) but telnyx is, US users
    // route to telnyx. Telnyx stays as the documented US backup.
    let telnyx = Arc::new(RecordingChannel::new("telnyx"));
    let mut router = ChannelRouter::new();
    router.register(telnyx);

    assert_eq!(
        router.pick_channel_for(&user_with_phone("+12025551234")),
        "telnyx"
    );
}

// ===== Provider-routes fallback tests =====
//
// A channel that fails on demand so we can simulate "primary carrier
// dropped this message" and assert that the router falls through to
// the next provider with the [Lightfriend backup] prefix.
struct FailingChannel {
    id: &'static str,
    fail_until_attempt: AtomicUsize,
    attempts: AtomicUsize,
    last_body: std::sync::Mutex<String>,
}

impl FailingChannel {
    fn new(id: &'static str, fail_n_times: usize) -> Self {
        Self {
            id,
            fail_until_attempt: AtomicUsize::new(fail_n_times),
            attempts: AtomicUsize::new(0),
            last_body: std::sync::Mutex::new(String::new()),
        }
    }
}

#[async_trait]
impl MessageChannel for FailingChannel {
    fn id(&self) -> &'static str {
        self.id
    }
    async fn send(
        &self,
        _user: &User,
        _address: &str,
        body: &str,
        _media: Option<MediaRef>,
    ) -> Result<ChannelMessageId, ChannelError> {
        let attempt = self.attempts.fetch_add(1, Ordering::SeqCst) + 1;
        *self.last_body.lock().unwrap() = body.to_string();
        if attempt <= self.fail_until_attempt.load(Ordering::SeqCst) {
            Err(ChannelError::SendFailed(format!(
                "simulated fail {}",
                attempt
            )))
        } else {
            Ok(ChannelMessageId(format!("{}-{}", self.id, attempt)))
        }
    }
}

#[tokio::test]
#[serial_test::serial]
async fn route_drives_provider_order() {
    // Admin set US to telnyx-first, twilio-fallback. Router must honor it
    // even though the hardcoded default would pick twilio.
    let twilio = Arc::new(RecordingChannel::new("twilio"));
    let telnyx = Arc::new(RecordingChannel::new("telnyx"));
    let mut router = ChannelRouter::new();
    router.register(twilio.clone());
    router.register(telnyx.clone());
    router.set_route("US", vec!["telnyx".to_string(), "twilio".to_string()]);

    let user = user_with_phone("+12025551234");
    let result = router.send_to_user(&user, "hello", None).await.unwrap();

    assert_eq!(result.as_str(), "telnyx-1");
    assert_eq!(telnyx.send_count(), 1);
    assert_eq!(twilio.send_count(), 0);
}

#[tokio::test]
#[serial_test::serial]
async fn falls_through_to_next_provider_on_error() {
    // Twilio fails the first attempt. Telnyx should be tried with the
    // [Lightfriend backup] prefix and the message reaches the user.
    let failing_twilio = Arc::new(FailingChannel::new("twilio", 1));
    let telnyx = Arc::new(RecordingChannel::new("telnyx"));
    let mut router = ChannelRouter::new();
    router.register(failing_twilio.clone());
    router.register(telnyx.clone());
    router.set_route("US", vec!["twilio".to_string(), "telnyx".to_string()]);

    let user = user_with_phone("+12025551234");
    let result = router.send_to_user(&user, "hello", None).await.unwrap();

    assert_eq!(result.as_str(), "telnyx-1");
    assert_eq!(failing_twilio.attempts.load(Ordering::SeqCst), 1);
    assert_eq!(telnyx.send_count(), 1);
}

#[tokio::test]
#[serial_test::serial]
async fn fallback_body_has_lightfriend_backup_prefix() {
    // Capture body each provider receives. First attempt unchanged, second
    // attempt prefixed so the recipient knows it's still legit Lightfriend.
    use backend::channels::router::FALLBACK_PREFIX;

    let failing_twilio = Arc::new(FailingChannel::new("twilio", 1));
    let telnyx = Arc::new(FailingChannel::new("telnyx", 0));
    let mut router = ChannelRouter::new();
    router.register(failing_twilio.clone());
    router.register(telnyx.clone());
    router.set_route("US", vec!["twilio".to_string(), "telnyx".to_string()]);

    let user = user_with_phone("+12025551234");
    router.send_to_user(&user, "hello", None).await.unwrap();

    let twilio_body = failing_twilio.last_body.lock().unwrap().clone();
    let telnyx_body = telnyx.last_body.lock().unwrap().clone();
    assert_eq!(twilio_body, "hello");
    assert_eq!(telnyx_body, format!("{}hello", FALLBACK_PREFIX));
}

#[tokio::test]
#[serial_test::serial]
async fn all_providers_failing_returns_last_error() {
    // Every provider in the chain errors. send_to_user surfaces the last
    // error rather than silently succeeding.
    let twilio = Arc::new(FailingChannel::new("twilio", 999));
    let telnyx = Arc::new(FailingChannel::new("telnyx", 999));
    let mut router = ChannelRouter::new();
    router.register(twilio.clone());
    router.register(telnyx.clone());
    router.set_route("US", vec!["twilio".to_string(), "telnyx".to_string()]);

    let user = user_with_phone("+12025551234");
    let err = router.send_to_user(&user, "hello", None).await.unwrap_err();

    assert!(matches!(err, ChannelError::SendFailed(_)));
    assert_eq!(twilio.attempts.load(Ordering::SeqCst), 1);
    assert_eq!(telnyx.attempts.load(Ordering::SeqCst), 1);
}

#[tokio::test]
#[serial_test::serial]
async fn clear_route_reverts_to_default() {
    let twilio = Arc::new(RecordingChannel::new("twilio"));
    let telnyx = Arc::new(RecordingChannel::new("telnyx"));
    let mut router = ChannelRouter::new();
    router.register(twilio.clone());
    router.register(telnyx.clone());

    router.set_route("US", vec!["telnyx".to_string()]);
    router.clear_route("US");

    let user = user_with_phone("+12025551234");
    router.send_to_user(&user, "hello", None).await.unwrap();

    // Cleared → default picks twilio.
    assert_eq!(twilio.send_count(), 1);
    assert_eq!(telnyx.send_count(), 0);
}
