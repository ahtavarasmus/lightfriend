use crate::api::tinfoil_client::{ChatMessage, TinfoilVoiceClient};
use crate::context::ContextBuilder;
use crate::handlers::auth_middleware::AuthUser;
use crate::AppState;
use crate::UserCoreOps;
use axum::extract::ws::{Message, WebSocket};
use axum::extract::{State, WebSocketUpgrade};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use serde::Deserialize;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::mpsc;

// ---------------------------------------------------------------------------
// Audio utilities
// ---------------------------------------------------------------------------

/// ITU-T G.711 mu-law decode table (256 entries)
fn mulaw_decode(encoded: u8) -> i16 {
    // Invert all bits
    let mu = !encoded;
    let sign = mu & 0x80;
    let exponent = (mu >> 4) & 0x07;
    let mantissa = mu & 0x0F;
    // Reconstruct magnitude
    let mut magnitude = ((mantissa as i32) << 1) | 1;
    magnitude = (magnitude << (exponent + 2)) - 0x21; // bias removal
                                                      // Clamp to valid i16 range
    let magnitude = magnitude.clamp(0, i16::MAX as i32);
    if sign != 0 {
        -(magnitude as i16)
    } else {
        magnitude as i16
    }
}

/// ITU-T G.711 mu-law encode
fn mulaw_encode(sample: i16) -> u8 {
    const BIAS: i16 = 0x84;
    const CLIP: i16 = 32635;

    let sign = if sample < 0 { 0x80u8 } else { 0x00u8 };
    let mut magnitude = if sample < 0 {
        (-sample).min(CLIP)
    } else {
        sample.min(CLIP)
    };
    magnitude += BIAS;

    // Find the exponent (segment)
    let mut exponent: u8 = 7;
    let mut mask = 0x4000i16;
    while exponent > 0 {
        if magnitude & mask != 0 {
            break;
        }
        exponent -= 1;
        mask >>= 1;
    }

    let mantissa = ((magnitude >> (exponent + 3)) & 0x0F) as u8;
    let byte = !(sign | (exponent << 4) | mantissa);
    byte
}

/// Encode raw PCM samples as a WAV file (16-bit mono).
fn encode_wav_16bit_mono(samples: &[i16], sample_rate: u32) -> Vec<u8> {
    let data_len = (samples.len() * 2) as u32;
    let file_len = 36 + data_len;

    let mut buf = Vec::with_capacity(44 + data_len as usize);
    // RIFF header
    buf.extend_from_slice(b"RIFF");
    buf.extend_from_slice(&file_len.to_le_bytes());
    buf.extend_from_slice(b"WAVE");
    // fmt chunk
    buf.extend_from_slice(b"fmt ");
    buf.extend_from_slice(&16u32.to_le_bytes()); // chunk size
    buf.extend_from_slice(&1u16.to_le_bytes()); // PCM format
    buf.extend_from_slice(&1u16.to_le_bytes()); // mono
    buf.extend_from_slice(&sample_rate.to_le_bytes());
    buf.extend_from_slice(&(sample_rate * 2).to_le_bytes()); // byte rate
    buf.extend_from_slice(&2u16.to_le_bytes()); // block align
    buf.extend_from_slice(&16u16.to_le_bytes()); // bits per sample
                                                 // data chunk
    buf.extend_from_slice(b"data");
    buf.extend_from_slice(&data_len.to_le_bytes());
    for &s in samples {
        buf.extend_from_slice(&s.to_le_bytes());
    }
    buf
}

/// Strip WAV header and return raw PCM bytes.
/// Searches for the "data" chunk and returns everything after the chunk size.
fn strip_wav_header(wav: &[u8]) -> &[u8] {
    // Search for "data" marker
    for i in 0..wav.len().saturating_sub(8) {
        if &wav[i..i + 4] == b"data" {
            let data_start = i + 8; // skip "data" + 4-byte size
            if data_start <= wav.len() {
                return &wav[data_start..];
            }
        }
    }
    // Fallback: skip standard 44-byte header
    if wav.len() > 44 {
        &wav[44..]
    } else {
        &[]
    }
}

/// Resample audio using rubato's FftFixedInOut.
/// `from_rate` and `to_rate` must be valid integer ratios.
fn resample(samples: &[i16], from_rate: usize, to_rate: usize) -> Vec<i16> {
    use rubato::{FftFixedInOut, Resampler};

    if from_rate == to_rate || samples.is_empty() {
        return samples.to_vec();
    }

    let resampler = match FftFixedInOut::<f32>::new(from_rate, to_rate, 1024, 1) {
        Ok(r) => r,
        Err(e) => {
            tracing::error!(
                "Failed to create resampler {}->{}Hz: {}",
                from_rate,
                to_rate,
                e
            );
            return samples.to_vec();
        }
    };

    let chunk_size = resampler.input_frames_max();
    let mut resampler = resampler;

    // Convert i16 to f32
    let float_samples: Vec<f32> = samples.iter().map(|&s| s as f32 / 32768.0).collect();

    let mut output = Vec::with_capacity(samples.len() * to_rate / from_rate + 1024);

    // Process in chunks
    let mut pos = 0;
    while pos + chunk_size <= float_samples.len() {
        let chunk = vec![float_samples[pos..pos + chunk_size].to_vec()];
        match resampler.process(&chunk, None) {
            Ok(result) => {
                if let Some(ch) = result.first() {
                    for &s in ch {
                        output.push((s * 32767.0).round().clamp(-32768.0, 32767.0) as i16);
                    }
                }
            }
            Err(e) => {
                tracing::error!("Resampling error: {}", e);
                break;
            }
        }
        pos += chunk_size;
    }

    // Handle remaining samples by zero-padding to chunk_size
    if pos < float_samples.len() {
        let mut last_chunk = float_samples[pos..].to_vec();
        let original_len = last_chunk.len();
        last_chunk.resize(chunk_size, 0.0);
        let chunk = vec![last_chunk];
        if let Ok(result) = resampler.process(&chunk, None) {
            if let Some(ch) = result.first() {
                // Only take proportional output
                let expected = original_len * to_rate / from_rate;
                let take = expected.min(ch.len());
                for &s in &ch[..take] {
                    output.push((s * 32767.0).round().clamp(-32768.0, 32767.0) as i16);
                }
            }
        }
    }

    output
}

// ---------------------------------------------------------------------------
// Energy-based VAD
// ---------------------------------------------------------------------------

fn compute_rms(samples: &[i16]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let sum: f64 = samples.iter().map(|&s| (s as f64) * (s as f64)).sum();
    (sum / samples.len() as f64).sqrt() as f32
}

const SPEECH_RMS_THRESHOLD: f32 = 500.0;
const SILENCE_DURATION_MS: u64 = 800;
const MIN_SPEECH_DURATION_MS: u64 = 500;

// ---------------------------------------------------------------------------
// Whisper hallucination filter
// ---------------------------------------------------------------------------

/// Common Whisper hallucinations on silence/noise.
const HALLUCINATION_BLOCKLIST: &[&str] = &[
    "you",
    "thank you",
    "thank you.",
    "thanks",
    "thanks.",
    "thanks for watching",
    "thanks for watching!",
    "thanks for watching.",
    "thank you for watching",
    "bye",
    "bye.",
    "bye bye",
    "goodbye",
    "goodbye.",
    "okay",
    "okay.",
    "hmm",
    "hmm.",
    "huh",
    "uh",
    "um",
    "ah",
    "oh",
    "so",
    "yeah",
    "yes",
    "no",
    "please subscribe",
    "subscribe",
    "like and subscribe",
    "",
    "...",
    "you.",
    "*thud*",
    "*silence*",
    "(silence)",
    "the end",
    "the end.",
];

fn is_hallucination(transcript: &str) -> bool {
    let t = transcript.trim().to_lowercase();
    // Check blocklist
    if HALLUCINATION_BLOCKLIST.contains(&t.as_str()) {
        return true;
    }
    // Single character or just punctuation
    if t.len() <= 2 {
        return true;
    }
    // Only punctuation/symbols
    if t.chars().all(|c| !c.is_alphanumeric()) {
        return true;
    }
    false
}

// ---------------------------------------------------------------------------
// Twilio WebSocket protocol types
// ---------------------------------------------------------------------------

#[derive(Deserialize, Debug)]
struct TwilioWsMessage {
    event: String,
    start: Option<TwilioStart>,
    media: Option<TwilioMedia>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct TwilioStart {
    stream_sid: String,
    call_sid: String,
    custom_parameters: Option<serde_json::Value>,
}

#[derive(Deserialize, Debug)]
struct TwilioMedia {
    payload: String,
}

// ---------------------------------------------------------------------------
// Call session state
// ---------------------------------------------------------------------------

#[derive(Debug, PartialEq)]
enum CallState {
    Listening,
    Processing,
    Speaking,
}

#[derive(Debug, PartialEq, Clone, Copy)]
enum TransportMode {
    /// Twilio media stream: mulaw, 8kHz, base64 JSON messages
    Twilio,
    /// Browser WebSocket: raw PCM i16 at 16kHz in, 24kHz out
    WebCall,
}

struct CallSession {
    state: CallState,
    transport: TransportMode,
    stream_sid: String,
    user_id: i32,
    // Audio buffers
    speech_audio: Vec<i16>,
    is_speaking: bool,
    silence_start: Option<Instant>,
    speech_start: Option<Instant>,
    // Conversation
    history: Vec<ChatMessage>,
    system_prompt: String,
    tinfoil: TinfoilVoiceClient,
    tts_voice: String,
    voice_model: String,
    // Tool calling
    app_state: Arc<AppState>,
    user: crate::models::user_models::User,
    tool_defs_json: Vec<serde_json::Value>,
    // Timing & billing
    call_start: Instant,
    mark_counter: u32,
    is_outbound: bool,
}

// ---------------------------------------------------------------------------
// TwiML endpoint: POST /api/voice/incoming
// ---------------------------------------------------------------------------

/// Called by Twilio when a call arrives. Returns TwiML to connect to our WebSocket.
pub async fn voice_incoming(State(state): State<Arc<AppState>>, body: String) -> Response {
    // Parse Twilio form params
    let params: std::collections::HashMap<String, String> =
        url::form_urlencoded::parse(body.as_bytes())
            .into_owned()
            .collect();

    let caller = params.get("From").cloned().unwrap_or_default();
    tracing::info!("Voice incoming from: {}", caller);

    // Look up user by phone
    let user = match state.user_core.find_by_phone_number(&caller) {
        Ok(Some(u)) => u,
        Ok(None) => {
            tracing::warn!("Voice call from unknown number: {}", caller);
            return twiml_say("Sorry, this number is not registered with Lightfriend.");
        }
        Err(e) => {
            tracing::error!("DB error looking up caller {}: {}", caller, e);
            return twiml_say("An error occurred. Please try again later.");
        }
    };

    // Check credits
    if let Err(e) = crate::utils::usage::check_user_credits(&state, &user, "voice", None).await {
        tracing::warn!("User {} insufficient credits: {}", user.id, e);
        return twiml_say("You don't have enough credits for a voice call. Please add credits on the Lightfriend website.");
    }

    // Build WebSocket URL
    let server_url =
        std::env::var("SERVER_URL").unwrap_or_else(|_| "https://localhost:3000".to_string());
    let ws_url = server_url
        .replace("https://", "wss://")
        .replace("http://", "ws://");

    let twiml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Response>
  <Connect>
    <Stream url="{}/api/voice/ws">
      <Parameter name="user_id" value="{}" />
    </Stream>
  </Connect>
</Response>"#,
        ws_url, user.id
    );

    (
        axum::http::StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, "application/xml")],
        twiml,
    )
        .into_response()
}

/// Escape a string for safe inclusion in XML/TwiML.
fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn twiml_say(message: &str) -> Response {
    let twiml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Response>
  <Say>{}</Say>
</Response>"#,
        xml_escape(message)
    );
    (
        axum::http::StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, "application/xml")],
        twiml,
    )
        .into_response()
}

// ---------------------------------------------------------------------------
// Outbound notification call
// ---------------------------------------------------------------------------

/// Initiate an outbound voice call to a user using Twilio + our voice pipeline.
/// The call connects to our WebSocket and speaks the notification message as the greeting.
pub async fn make_notification_call(
    state: &Arc<AppState>,
    user: &crate::models::user_models::User,
    notification_message: &str,
) -> Result<String, String> {
    let server_url =
        std::env::var("SERVER_URL").unwrap_or_else(|_| "https://localhost:3000".to_string());
    let ws_url = server_url
        .replace("https://", "wss://")
        .replace("http://", "ws://");

    // XML-escape then URL-encode the greeting for safe TwiML inclusion
    let greeting_escaped = xml_escape(notification_message);
    let greeting_encoded = urlencoding::encode(&greeting_escaped);

    let twiml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Response>
  <Connect>
    <Stream url="{}/api/voice/ws">
      <Parameter name="user_id" value="{}" />
      <Parameter name="greeting" value="{}" />
    </Stream>
  </Connect>
</Response>"#,
        ws_url, user.id, greeting_encoded
    );

    // Get credentials and from number
    let credentials = state
        .twilio_message_service
        .resolve_credentials(user)
        .map_err(|e| format!("Failed to resolve Twilio credentials: {}", e))?;

    let from_number = user
        .preferred_number
        .clone()
        .ok_or_else(|| "No from number available for voice call".to_string())?;

    // Initiate the call
    use crate::api::twilio_client::TwilioClient;
    let call_sid = state
        .twilio_client
        .make_call(&credentials, &user.phone_number, &from_number, &twiml)
        .await
        .map_err(|e| format!("Twilio call failed: {}", e))?;

    tracing::info!(
        "Outbound notification call initiated for user {}: SID {}",
        user.id,
        call_sid
    );

    Ok(call_sid)
}

// ---------------------------------------------------------------------------
// Web call start: GET /api/voice/web-start (authenticated)
// ---------------------------------------------------------------------------

/// Called by the Yew frontend to start a web voice call.
/// Checks auth + credits, returns WebSocket URL for the JS client.
pub async fn voice_web_start(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let user_id = auth_user.user_id;

    let user = state
        .user_core
        .find_by_id(user_id)
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "Database error"})),
            )
        })?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": "User not found"})),
            )
        })?;

    // Check credits (web calls use voice pricing - no Twilio leg, just Tinfoil)
    if let Err(e) = crate::utils::usage::check_user_credits(&state, &user, "voice", None).await {
        return Err((StatusCode::FORBIDDEN, Json(serde_json::json!({"error": e}))));
    }

    // Generate a one-time token for WebSocket auth (expires in 30s)
    let token = uuid::Uuid::new_v4().to_string();
    let expiry = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
        + 30;
    state
        .pending_totp_logins
        .insert(format!("voice_{}", token), (user_id, expiry));

    let ws_url = format!("/api/voice/web-ws?token={}", token);

    Ok(Json(serde_json::json!({
        "ws_url": ws_url,
        "user_id": user_id,
    })))
}

// ---------------------------------------------------------------------------
// Web voice call WebSocket: GET /api/voice/web-ws?user_id=N
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct WebWsParams {
    token: String,
}

pub async fn voice_web_ws(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(params): axum::extract::Query<WebWsParams>,
    ws: WebSocketUpgrade,
) -> Response {
    let key = format!("voice_{}", params.token);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    // Validate and consume token (one-time use)
    let user_id = match state.pending_totp_logins.remove(&key) {
        Some((_, (uid, expiry))) if expiry > now => uid,
        Some(_) => {
            tracing::warn!("Expired voice WS token");
            return (StatusCode::UNAUTHORIZED, "Token expired").into_response();
        }
        None => {
            tracing::warn!("Invalid voice WS token");
            return (StatusCode::UNAUTHORIZED, "Invalid token").into_response();
        }
    };

    ws.on_upgrade(move |socket| handle_web_ws(state, socket, user_id))
}

/// Browser-based voice call: raw PCM i16 at 16kHz in, 24kHz out.
/// No Twilio, no mulaw. Direct browser <-> server audio.
async fn handle_web_ws(state: Arc<AppState>, socket: WebSocket, user_id: i32) {
    use futures::stream::StreamExt;

    let (ws_tx, mut ws_rx) = socket.split();
    let (send_tx, send_rx) = mpsc::channel::<Message>(64);

    tokio::spawn(sender_loop(ws_tx, send_rx));

    let mut session = match init_session(
        &state,
        user_id,
        "web".to_string(),
        false,
        TransportMode::WebCall,
    )
    .await
    {
        Ok(s) => s,
        Err(e) => {
            tracing::error!(
                "Failed to init web call session for user {}: {}",
                user_id,
                e
            );
            let err = serde_json::json!({"type": "error", "message": e});
            let _ = send_tx.send(Message::Text(err.to_string().into())).await;
            return;
        }
    };

    // Send greeting
    let greeting = build_greeting(&state, user_id);
    if let Err(e) = send_tts_response(&mut session, &greeting, &send_tx).await {
        tracing::error!("Failed to send web call greeting: {}", e);
    }
    session.state = CallState::Listening;

    let ready = serde_json::json!({"type": "ready", "user_id": user_id});
    let _ = send_tx.send(Message::Text(ready.to_string().into())).await;

    const MAX_CALL_DURATION: std::time::Duration = std::time::Duration::from_secs(600); // 10 min

    while let Some(msg) = tokio::select! {
        msg = ws_rx.next() => msg,
        _ = tokio::time::sleep(MAX_CALL_DURATION.saturating_sub(session.call_start.elapsed())) => {
            tracing::info!("Web call max duration reached for user {}", user_id);
            let _ = send_tts_response(&mut session, "Call time limit reached. Goodbye.", &send_tx).await;
            None
        }
    } {
        let msg = match msg {
            Ok(m) => m,
            Err(e) => {
                tracing::error!("Web call WS error: {}", e);
                break;
            }
        };

        match msg {
            Message::Binary(data) => {
                if session.state != CallState::Listening {
                    continue;
                }

                let pcm_16k: Vec<i16> = data
                    .chunks_exact(2)
                    .map(|c| i16::from_le_bytes([c[0], c[1]]))
                    .collect();

                let rms = compute_rms(&pcm_16k);

                if rms > SPEECH_RMS_THRESHOLD {
                    if !session.is_speaking {
                        session.is_speaking = true;
                        session.speech_start = Some(Instant::now());
                        session.silence_start = None;

                        let evt = serde_json::json!({"type": "vad", "speaking": true});
                        let _ = send_tx.send(Message::Text(evt.to_string().into())).await;
                    }
                    session.speech_audio.extend_from_slice(&pcm_16k);
                    session.silence_start = None;
                } else if session.is_speaking {
                    session.speech_audio.extend_from_slice(&pcm_16k);

                    if session.silence_start.is_none() {
                        session.silence_start = Some(Instant::now());
                    }

                    if let Some(silence_start) = session.silence_start {
                        if silence_start.elapsed().as_millis() as u64 >= SILENCE_DURATION_MS {
                            let speech_duration = session
                                .speech_start
                                .map(|s| s.elapsed().as_millis() as u64)
                                .unwrap_or(0);

                            if speech_duration >= MIN_SPEECH_DURATION_MS
                                && !session.speech_audio.is_empty()
                            {
                                let speech = std::mem::take(&mut session.speech_audio);
                                session.is_speaking = false;
                                session.silence_start = None;
                                session.speech_start = None;
                                session.state = CallState::Processing;

                                let evt = serde_json::json!({
                                    "type": "vad",
                                    "speaking": false,
                                    "processing": true,
                                    "speech_ms": speech_duration,
                                });
                                let _ = send_tx.send(Message::Text(evt.to_string().into())).await;

                                if let Err(e) =
                                    process_utterance(&mut session, &speech, &send_tx).await
                                {
                                    tracing::error!("Web call process_utterance error: {}", e);
                                    let _ = send_tts_response(
                                        &mut session,
                                        "Sorry, something went wrong. Please try again.",
                                        &send_tx,
                                    )
                                    .await;
                                }

                                session.state = CallState::Listening;
                            } else {
                                session.speech_audio.clear();
                                session.is_speaking = false;
                                session.silence_start = None;
                                session.speech_start = None;
                            }
                        }
                    }
                }
            }
            Message::Text(text) => {
                if let Ok(cmd) = serde_json::from_str::<serde_json::Value>(&text) {
                    if cmd.get("type").and_then(|t| t.as_str()) == Some("ping") {
                        let pong = serde_json::json!({"type": "pong"});
                        let _ = send_tx.send(Message::Text(pong.to_string().into())).await;
                    }
                }
            }
            Message::Close(_) => break,
            _ => {}
        }
    }

    // Deduct credits (Tinfoil only, no Twilio leg for web calls)
    let duration_secs = session.call_start.elapsed().as_secs() as i32;
    tracing::info!(
        "Web call ended for user {}. Duration: {}s",
        user_id,
        duration_secs
    );
    if let Err(e) =
        crate::utils::usage::deduct_user_credits(&state, user_id, "voice", Some(duration_secs))
    {
        tracing::error!(
            "Failed to deduct credits for web call user {}: {}",
            user_id,
            e
        );
    }

    tracing::info!("Web voice call closed for user {}", user_id);
}

// ---------------------------------------------------------------------------
// WebSocket handler: GET /api/voice/ws
// ---------------------------------------------------------------------------

pub async fn voice_ws(State(state): State<Arc<AppState>>, ws: WebSocketUpgrade) -> Response {
    ws.on_upgrade(move |socket| handle_voice_ws(state, socket))
}

async fn handle_voice_ws(state: Arc<AppState>, socket: WebSocket) {
    use futures::stream::StreamExt;

    let (ws_tx, mut ws_rx) = socket.split();

    // Channel for sending messages back through the WebSocket
    let (send_tx, send_rx) = mpsc::channel::<Message>(64);

    // Sender task: drains send_rx -> ws_tx
    tokio::spawn(sender_loop(ws_tx, send_rx));

    let mut session: Option<CallSession> = None;

    while let Some(msg) = ws_rx.next().await {
        let msg = match msg {
            Ok(m) => m,
            Err(e) => {
                tracing::error!("WebSocket receive error: {}", e);
                break;
            }
        };

        let text = match msg {
            Message::Text(t) => t,
            Message::Close(_) => break,
            _ => continue,
        };

        let twilio_msg: TwilioWsMessage = match serde_json::from_str(&text) {
            Ok(m) => m,
            Err(e) => {
                tracing::warn!("Failed to parse Twilio WS message: {}", e);
                continue;
            }
        };

        match twilio_msg.event.as_str() {
            "connected" => {
                tracing::info!("Twilio media stream connected");
            }

            "start" => {
                if let Some(start) = twilio_msg.start {
                    tracing::info!(
                        "Stream started: streamSid={}, callSid={}",
                        start.stream_sid,
                        start.call_sid
                    );

                    let user_id = start
                        .custom_parameters
                        .as_ref()
                        .and_then(|p| p.get("user_id"))
                        .and_then(|v| v.as_str())
                        .and_then(|s| s.parse::<i32>().ok())
                        .unwrap_or(0);

                    if user_id == 0 {
                        tracing::error!("No valid user_id in stream start parameters");
                        break;
                    }

                    // Check for custom greeting (outbound notification calls)
                    let custom_greeting = start
                        .custom_parameters
                        .as_ref()
                        .and_then(|p| p.get("greeting"))
                        .and_then(|v| v.as_str())
                        .map(|s| urlencoding::decode(s).unwrap_or_default().to_string());

                    let is_outbound = custom_greeting.is_some();

                    match init_session(
                        &state,
                        user_id,
                        start.stream_sid.clone(),
                        is_outbound,
                        TransportMode::Twilio,
                    )
                    .await
                    {
                        Ok(s) => {
                            // Spawn greeting TTS on a separate task so we don't
                            // block the WS receive loop (TTS HTTP takes ~2s).
                            let greeting =
                                custom_greeting.unwrap_or_else(|| build_greeting(&state, user_id));
                            let stream_sid = s.stream_sid.clone();
                            let tinfoil = s.tinfoil.clone();
                            let tts_voice = s.tts_voice.clone();
                            let send_tx_clone = send_tx.clone();
                            tokio::spawn(async move {
                                tracing::info!("TTS requesting for: \"{}\"", greeting);
                                match tinfoil.text_to_speech(&greeting, &tts_voice).await {
                                    Ok(tts_wav) => {
                                        tracing::info!("TTS returned {} bytes", tts_wav.len());
                                        let pcm_bytes = strip_wav_header(&tts_wav);
                                        if pcm_bytes.is_empty() {
                                            tracing::warn!("TTS returned empty audio");
                                            return;
                                        }
                                        let pcm_samples: Vec<i16> = pcm_bytes
                                            .chunks_exact(2)
                                            .map(|c| i16::from_le_bytes([c[0], c[1]]))
                                            .collect();
                                        let pcm_8k = resample(&pcm_samples, 24000, 8000);
                                        let mulaw_bytes: Vec<u8> =
                                            pcm_8k.iter().map(|&s| mulaw_encode(s)).collect();
                                        for chunk in mulaw_bytes.chunks(160) {
                                            let payload = BASE64.encode(chunk);
                                            let msg = serde_json::json!({
                                                "event": "media",
                                                "streamSid": stream_sid,
                                                "media": { "payload": payload }
                                            });
                                            let _ = send_tx_clone
                                                .send(Message::Text(msg.to_string().into()))
                                                .await;
                                        }
                                        tracing::info!(
                                            "Greeting audio queued ({} bytes mulaw)",
                                            mulaw_bytes.len()
                                        );
                                    }
                                    Err(e) => {
                                        tracing::error!("Failed to generate greeting TTS: {}", e);
                                    }
                                }
                            });
                            session = Some(s);
                        }
                        Err(e) => {
                            tracing::error!("Failed to init session for user {}: {}", user_id, e);
                            break;
                        }
                    }
                }
            }

            "media" => {
                if let (Some(ref mut sess), Some(media)) = (&mut session, twilio_msg.media) {
                    if sess.state == CallState::Processing {
                        continue;
                    }

                    // Decode base64 -> mulaw -> PCM i16 at 8kHz
                    let mulaw_bytes = match BASE64.decode(&media.payload) {
                        Ok(b) => b,
                        Err(_) => continue,
                    };

                    let pcm_8k: Vec<i16> = mulaw_bytes.iter().map(|&b| mulaw_decode(b)).collect();

                    // Resample 8kHz -> 16kHz for STT
                    let pcm_16k = resample(&pcm_8k, 8000, 16000);

                    // Energy-based VAD
                    let rms = compute_rms(&pcm_16k);

                    if rms > SPEECH_RMS_THRESHOLD {
                        if !sess.is_speaking {
                            sess.is_speaking = true;
                            sess.speech_start = Some(Instant::now());
                            sess.silence_start = None;
                            tracing::debug!("Speech detected (RMS={:.0})", rms);
                        }
                        sess.speech_audio.extend_from_slice(&pcm_16k);
                        sess.silence_start = None;
                    } else if sess.is_speaking {
                        // Still accumulate audio during brief silence
                        sess.speech_audio.extend_from_slice(&pcm_16k);

                        if sess.silence_start.is_none() {
                            sess.silence_start = Some(Instant::now());
                        }

                        if let Some(silence_start) = sess.silence_start {
                            if silence_start.elapsed().as_millis() as u64 >= SILENCE_DURATION_MS {
                                // End of speech detected
                                let speech_duration = sess
                                    .speech_start
                                    .map(|s| s.elapsed().as_millis() as u64)
                                    .unwrap_or(0);

                                if speech_duration >= MIN_SPEECH_DURATION_MS
                                    && !sess.speech_audio.is_empty()
                                {
                                    tracing::info!(
                                        "End of speech detected ({}ms, {} samples)",
                                        speech_duration,
                                        sess.speech_audio.len()
                                    );

                                    // Take the speech audio and process it
                                    let speech = std::mem::take(&mut sess.speech_audio);
                                    sess.is_speaking = false;
                                    sess.silence_start = None;
                                    sess.speech_start = None;
                                    sess.state = CallState::Processing;

                                    if let Err(e) = process_utterance(sess, &speech, &send_tx).await
                                    {
                                        tracing::error!("process_utterance error: {}", e);
                                        // Speak the error so the user isn't left hanging
                                        let _ = send_tts_response(
                                            sess,
                                            "Sorry, something went wrong. Please try again.",
                                            &send_tx,
                                        )
                                        .await;
                                    }

                                    sess.state = CallState::Listening;
                                } else {
                                    // Too short, discard
                                    sess.speech_audio.clear();
                                    sess.is_speaking = false;
                                    sess.silence_start = None;
                                    sess.speech_start = None;
                                }
                            }
                        }
                    }
                }
            }

            "mark" => {
                if let Some(ref mut sess) = session {
                    if sess.state == CallState::Speaking {
                        sess.state = CallState::Listening;
                        tracing::debug!("Playback complete, resuming listening");
                    }
                }
            }

            "stop" => {
                tracing::info!("Stream stopped");
                if let Some(ref sess) = session {
                    let duration_secs = sess.call_start.elapsed().as_secs() as i32;
                    let event_type = if sess.is_outbound {
                        "noti_call"
                    } else {
                        "voice"
                    };
                    tracing::info!(
                        "Call ended for user {}. Duration: {}s, type: {}",
                        sess.user_id,
                        duration_secs,
                        event_type
                    );

                    // Deduct credits (uses outbound vs inbound pricing)
                    if let Err(e) = crate::utils::usage::deduct_user_credits(
                        &state,
                        sess.user_id,
                        event_type,
                        Some(duration_secs),
                    ) {
                        tracing::error!(
                            "Failed to deduct credits for user {}: {}",
                            sess.user_id,
                            e
                        );
                    }
                }
                break;
            }

            other => {
                tracing::debug!("Unhandled Twilio event: {}", other);
            }
        }
    }

    tracing::info!("Voice WebSocket connection closed");
}

/// Sender task: forwards messages from mpsc channel to WebSocket sink.
async fn sender_loop(
    mut ws_tx: futures::stream::SplitSink<WebSocket, Message>,
    mut send_rx: mpsc::Receiver<Message>,
) {
    use futures::SinkExt;

    while let Some(msg) = send_rx.recv().await {
        if ws_tx.send(msg).await.is_err() {
            break;
        }
    }
}

// ---------------------------------------------------------------------------
// Session initialization
// ---------------------------------------------------------------------------

async fn init_session(
    state: &Arc<AppState>,
    user_id: i32,
    stream_sid: String,
    is_outbound: bool,
    transport: TransportMode,
) -> Result<CallSession, String> {
    let ctx = ContextBuilder::for_user(state, user_id)
        .with_user_context()
        .build()
        .await
        .map_err(|e| format!("context build error: {}", e))?;

    // Build system prompt and tools via shared agent_core
    let system_prompt =
        crate::agent_core::build_system_prompt(&ctx, crate::agent_core::ChannelMode::Voice);
    let all_tools = crate::agent_core::build_tools(state, user_id, true).await;
    let tool_defs_json: Vec<serde_json::Value> = all_tools
        .iter()
        .filter_map(|t| serde_json::to_value(t).ok())
        .collect();

    let tinfoil = TinfoilVoiceClient::new(&state.ai_config);

    // Voice model from centralized config
    let voice_model = state
        .ai_config
        .model(crate::AiProvider::Tinfoil, crate::ModelPurpose::Voice)
        .to_string();

    // Determine TTS voice based on language setting
    let language = ctx
        .user_settings
        .as_ref()
        .map(|s| s.agent_language.as_str())
        .unwrap_or("en");

    let tts_voice = match language {
        "fi" => "serena".to_string(),
        "de" => "serena".to_string(),
        _ => "aiden".to_string(),
    };

    let user = ctx.user.clone();

    Ok(CallSession {
        state: CallState::Listening,
        transport,
        stream_sid,
        user_id,
        speech_audio: Vec::new(),
        is_speaking: false,
        silence_start: None,
        speech_start: None,
        history: Vec::new(),
        system_prompt,
        tinfoil,
        tts_voice,
        voice_model,
        app_state: Arc::clone(state),
        user,
        tool_defs_json,
        call_start: Instant::now(),
        mark_counter: 0,
        is_outbound,
    })
}

fn build_greeting(state: &Arc<AppState>, user_id: i32) -> String {
    let nickname = state
        .user_core
        .find_by_id(user_id)
        .ok()
        .flatten()
        .and_then(|u| u.nickname)
        .unwrap_or_default();

    if nickname.is_empty() {
        "Hello!".to_string()
    } else {
        format!("Hello {}!", nickname)
    }
}

// ---------------------------------------------------------------------------
// STT -> LLM -> TTS pipeline
// ---------------------------------------------------------------------------

async fn process_utterance(
    session: &mut CallSession,
    speech_samples: &[i16],
    send_tx: &mpsc::Sender<Message>,
) -> Result<(), String> {
    use crate::api::tinfoil_client::CompletionResult;

    // 1. Encode speech as WAV at 16kHz
    let wav = encode_wav_16bit_mono(speech_samples, 16000);

    // 2. Transcribe
    let transcript = session.tinfoil.transcribe(&wav).await?;
    let transcript = transcript.trim().to_string();

    if transcript.is_empty() {
        tracing::debug!("Empty transcript, skipping");
        return Ok(());
    }

    // 3. Filter Whisper hallucinations
    if is_hallucination(&transcript) {
        tracing::debug!("Filtered hallucination: \"{}\"", transcript);
        return Ok(());
    }

    tracing::info!("STT transcript: \"{}\"", transcript);

    // 4. Immediate feedback - say "just a moment" while we process
    send_tts_response(session, "Just a moment.", send_tx).await?;

    // 5. Add user message to history
    session.history.push(ChatMessage::user(&transcript));

    // 6. Agentic loop: LLM -> maybe tool calls -> LLM -> ... -> text response
    let tools_owned = session.tool_defs_json.clone();
    let tools = if tools_owned.is_empty() {
        None
    } else {
        Some(tools_owned.as_slice())
    };

    let mut final_response = String::new();
    let max_rounds = 5;

    for round in 0..max_rounds {
        let result = session
            .tinfoil
            .chat_completion_with_tools(
                &session.history,
                &session.system_prompt,
                tools,
                &session.voice_model,
            )
            .await?;

        match result {
            CompletionResult::Text(text) => {
                tracing::info!("LLM response: \"{}\"", text);
                session.history.push(ChatMessage::assistant(&text));
                final_response = text;
                break;
            }
            CompletionResult::ToolCalls {
                content,
                tool_calls,
            } => {
                tracing::info!(
                    "LLM requested {} tool call(s) (round {}): {}",
                    tool_calls.len(),
                    round + 1,
                    tool_calls
                        .iter()
                        .map(|tc| tc.function.name.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                );

                // Add assistant message with tool calls to history
                session.history.push(ChatMessage::assistant_with_tool_calls(
                    &content,
                    tool_calls.clone(),
                ));

                // Execute each tool call with progress feedback
                for tc in &tool_calls {
                    let tool_name = tc.function.name.clone();
                    let tool_args = tc.function.arguments.clone();

                    // Spawn tool execution so we can give progress feedback
                    let state = Arc::clone(&session.app_state);
                    let user = session.user.clone();
                    let user_id = session.user_id;
                    let tool_handle = tokio::spawn(async move {
                        let user_given_info = state
                            .user_core
                            .get_user_info(user_id)
                            .ok()
                            .and_then(|i| i.info)
                            .unwrap_or_default();
                        let current_time = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs() as i32;
                        let result = crate::agent_core::dispatch_tool(
                            &state,
                            &user,
                            &tool_name,
                            &tool_args,
                            "voice-call",
                            &user_given_info,
                            current_time,
                            None,
                        )
                        .await;
                        match result {
                            crate::agent_core::ToolDispatchResult::Answer(a) => a,
                            crate::agent_core::ToolDispatchResult::AnswerWithTask {
                                answer,
                                ..
                            } => answer,
                            crate::agent_core::ToolDispatchResult::EarlyReturn {
                                response, ..
                            } => response.message,
                            crate::agent_core::ToolDispatchResult::SubscriptionRequired(m) => m,
                            crate::agent_core::ToolDispatchResult::Unknown(m) => m,
                            crate::agent_core::ToolDispatchResult::Error(e) => {
                                format!("Tool error: {}", e)
                            }
                        }
                    });

                    let started = std::time::Instant::now();
                    let mut progress_count = 0u32;
                    let mut tool_handle = tool_handle;

                    let progress_messages = [
                        "Still working on it, one moment.",
                        "Still on it, almost there.",
                        "Taking a while, hang tight.",
                    ];

                    let answer = loop {
                        tokio::select! {
                            tool_result = &mut tool_handle => {
                                break tool_result.unwrap_or_else(|e| format!("Tool error: {}", e));
                            }
                            _ = tokio::time::sleep(tokio::time::Duration::from_secs(if progress_count == 0 { 8 } else { 15 })) => {
                                let msg = progress_messages[progress_count.min(2) as usize];
                                progress_count += 1;
                                tracing::info!("Tool {} taking {:.0}s, progress #{}", tc.function.name, started.elapsed().as_secs_f32(), progress_count);
                                let _ = send_tts_response(session, msg, send_tx).await;
                            }
                        }
                    };

                    tracing::info!(
                        "Tool {} result: {} chars ({:.1}s)",
                        tc.function.name,
                        answer.len(),
                        started.elapsed().as_secs_f32()
                    );
                    session
                        .history
                        .push(ChatMessage::tool_result(&tc.id, &answer));
                }

                // If this is the last round, force a text response
                if round == max_rounds - 1 {
                    let fallback = session
                        .tinfoil
                        .chat_completion(
                            &session.history,
                            &session.system_prompt,
                            &session.voice_model,
                        )
                        .await?;
                    session.history.push(ChatMessage::assistant(&fallback));
                    final_response = fallback;
                }
            }
        }
    }

    if final_response.is_empty() {
        return Ok(());
    }

    tracing::info!("Final response to speak: \"{}\"", final_response);

    // 6. TTS and send audio
    send_tts_response(session, &final_response, send_tx).await
}

async fn send_tts_response(
    session: &mut CallSession,
    text: &str,
    send_tx: &mpsc::Sender<Message>,
) -> Result<(), String> {
    session.state = CallState::Speaking;

    // Generate TTS audio
    tracing::info!("TTS requesting for: \"{}\"", text);
    let tts_wav = session
        .tinfoil
        .text_to_speech(text, &session.tts_voice)
        .await?;

    tracing::info!("TTS returned {} bytes", tts_wav.len());

    // Strip WAV header to get raw PCM bytes
    let pcm_bytes = strip_wav_header(&tts_wav);

    if pcm_bytes.is_empty() {
        tracing::warn!("TTS returned empty audio");
        session.state = CallState::Listening;
        return Ok(());
    }

    // Parse as i16 PCM (little-endian)
    let pcm_samples: Vec<i16> = pcm_bytes
        .chunks_exact(2)
        .map(|c| i16::from_le_bytes([c[0], c[1]]))
        .collect();

    match session.transport {
        TransportMode::Twilio => {
            // Resample from 24kHz -> 8kHz for Twilio
            let pcm_8k = resample(&pcm_samples, 24000, 8000);

            // Encode as mulaw
            let mulaw_bytes: Vec<u8> = pcm_8k.iter().map(|&s| mulaw_encode(s)).collect();

            // Send in 160-byte (20ms) chunks as Twilio media events
            let stream_sid = session.stream_sid.clone();
            for chunk in mulaw_bytes.chunks(160) {
                let payload = BASE64.encode(chunk);
                let msg = serde_json::json!({
                    "event": "media",
                    "streamSid": stream_sid,
                    "media": {
                        "payload": payload
                    }
                });
                let _ = send_tx.send(Message::Text(msg.to_string().into())).await;
            }

            // Send mark to track playback completion
            session.mark_counter += 1;
            let mark_msg = serde_json::json!({
                "event": "mark",
                "streamSid": stream_sid,
                "mark": {
                    "name": format!("response_{}", session.mark_counter)
                }
            });
            let _ = send_tx
                .send(Message::Text(mark_msg.to_string().into()))
                .await;

            // Transition to Listening immediately after queuing audio.
            // We can't block waiting for the mark echo because this function
            // runs inline in the WS receive loop - blocking here drops
            // incoming media events from Twilio.
            session.state = CallState::Listening;
            tracing::info!(
                "TTS audio queued ({} bytes mulaw), now listening",
                mulaw_bytes.len()
            );
        }
        TransportMode::WebCall => {
            // Send raw PCM at 24kHz as binary frames (browser plays directly)
            let meta = serde_json::json!({
                "type": "audio_start",
                "sample_rate": 24000,
                "channels": 1,
                "samples": pcm_samples.len(),
            });
            let _ = send_tx.send(Message::Text(meta.to_string().into())).await;

            // Send PCM as binary (i16 little-endian) in ~4KB chunks
            let byte_data: Vec<u8> = pcm_samples.iter().flat_map(|&s| s.to_le_bytes()).collect();
            for chunk in byte_data.chunks(4096) {
                let _ = send_tx.send(Message::Binary(chunk.to_vec().into())).await;
            }

            let end = serde_json::json!({"type": "audio_end"});
            let _ = send_tx.send(Message::Text(end.to_string().into())).await;

            // Wait for estimated playback to avoid echo
            let playback_ms = (pcm_samples.len() as u64 * 1000) / 24000;
            tokio::time::sleep(tokio::time::Duration::from_millis(playback_ms + 300)).await;
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Debug WebSocket handler: GET /api/voice/ws-debug?user_id=N
// ---------------------------------------------------------------------------
