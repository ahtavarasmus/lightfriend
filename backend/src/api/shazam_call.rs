use axum::{
    extract::ws::{WebSocket, WebSocketUpgrade},
    response::IntoResponse,

    extract::Path,
};
use futures::{sink::SinkExt, stream::StreamExt};
use std::collections::HashMap;

use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};
use reqwest::Client as HttpClient;
use base64::{Engine as _, engine::general_purpose};
use serde_json::{Value, from_str};
use crate::repositories::user_repository::UserRepository;
use crate::repositories::user_conversations::UserConversations;
use hound::{WavWriter, WavSpec};
use std::io::Cursor;
use tracing::{info, error};

pub type CallSessions = Arc<Mutex<HashMap<String, broadcast::Sender<Vec<u8>>>>>;
pub type UserCallMap = Arc<Mutex<HashMap<String, String>>>; // callSid -> user_id

pub struct ShazamState {
    pub sessions: CallSessions,
    pub user_calls: UserCallMap,
    pub user_repository: Arc<UserRepository>,
    pub user_conversations: Arc<UserConversations>,
}

impl ShazamState {
    pub fn new(user_repository: Arc<UserRepository>, user_conversations: Arc<UserConversations>) -> Self {
        Self {
            sessions: Arc::new(Mutex::new(HashMap::new())),
            user_calls: Arc::new(Mutex::new(HashMap::new())),
            user_repository,
            user_conversations,
        }
    }
}



// Handler to start a call for a userwhisper

pub async fn start_call_for_user(
    Path(user_id): Path<String>,
    axum::extract::State(state): axum::extract::State<Arc<crate::AppState>>,
) -> impl IntoResponse {
    let account_sid = std::env::var("TWILIO_ACCOUNT_SID").expect("TWILIO_ACCOUNT_SID must be set");
    let auth_token = std::env::var("TWILIO_AUTH_TOKEN").expect("TWILIO_AUTH_TOKEN must be set");
    
    let user = match state.user_repository.find_by_id(user_id.parse().unwrap()) {
        Ok(Some(user)) => user,
        _ => return "User not found".to_string(),
    };

    let to_number = user.phone_number;
    if to_number.is_empty() {
        return "User not found".to_string();
    }

    // Choose the appropriate from number based on the destination
    let from_number = if to_number.starts_with("+1") {
        std::env::var("SHAZAM_PHONE_NUMBER").expect("SHAZAM_PHONE_NUMBER must be set")
    } else {
        std::env::var("SHAZAM_EUROPE_PHONE_NUMBER").expect("SHAZAM_EUROPE_PHONE_NUMBER must be set")
    };

    let server_url = std::env::var("SERVER_URL").expect("SERVER_URL must be set");
    let twiml_url = format!("{}/api/twiml", server_url);

    // Create HTTP client
    let http_client = HttpClient::new();

    // Twilio API endpoint
    let url = format!(
        "https://api.twilio.com/2010-04-01/Accounts/{}/Calls.json",
        account_sid
    );

    // Form data for the POST request
    let params = [
        ("To", to_number.as_str()),
        ("From", from_number.as_str()),
        ("Url", twiml_url.as_str()),
    ];

    // Make the request with Basic Auth
    let response = http_client
        .post(&url)
        .basic_auth(&account_sid, Some(&auth_token))
        .form(&params)
        .send()
        .await;

    match response {
        Ok(resp) if resp.status().is_success() => {
            let json: Value = resp.json().await.unwrap_or_default();
            let call_sid = json["sid"].as_str().unwrap_or("").to_string();
            let mut user_calls_lock = state.user_calls.lock().await;
            user_calls_lock.insert(call_sid.clone(), user_id.clone());
            info!("Call initiated for user {}: {}", user_id, call_sid);
            format!("Call initiated for user {}: {}", user_id, call_sid)
        }
        Ok(resp) => {
            let error_text = resp.text().await.unwrap_or_default();
            info!("Error initiating call for user {}: {}", user_id, error_text);
            format!("Error initiating call: {}", error_text)
        }
        Err(e) => {
            info!("Error initiating call for user {}: {:?}", user_id, e);
            format!("Error initiating call: {:?}", e)
        }
    }
}


// TwiML to stream audio
pub async fn twiml_handler() -> impl IntoResponse {
    let server_url = std::env::var("TWILIO_SHAZAM_SERVER_URL").expect("TWILIO_SHAZAM_SERVER_URL must be set");
    // Remove any trailing slashes from the server URL
    let clean_server_url = server_url.trim_end_matches('/');
    let twiml = format!(r#"<?xml version="1.0" encoding="UTF-8"?>
                <Response>
                <Say>I'm listening.</Say>
                <Start>
                    <Stream url="wss://{}/api/stream" track="inbound_track"/>
                </Start>
                <Pause length="60"/>
                </Response>"#, clean_server_url);
    axum::http::Response::builder()
        .header("Content-Type", "application/xml")
        .body(twiml.to_string())
        .unwrap()
}

// WebSocket handler for Twilio audio stream
pub async fn stream_handler(
    ws: WebSocketUpgrade,
    axum::extract::State(state): axum::extract::State<Arc<crate::AppState>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_stream(socket, state.sessions.clone()))
}

pub async fn handle_stream(mut socket: WebSocket, sessions: CallSessions) {
    let (tx, _) = broadcast::channel(100);
    let mut call_sid = String::new();

    println!("Twilio WebSocket connected");

    while let Some(msg) = socket.next().await {
        match msg {
            Ok(msg) => {
                if let Ok(text) = msg.to_text() {
                    if text.contains("\"start\"") {
                        let json: Value = from_str(text).unwrap();
                        call_sid = json["start"]["callSid"].as_str().unwrap_or("").to_string();
                        let mut sessions_lock = sessions.lock().await;
                        sessions_lock.insert(call_sid.clone(), tx.clone());
                        let server_url = std::env::var("TWILIO_SHAZAM_SERVER_URL").expect("TWILIO_SHAZAM_SERVER_URL must be set");
                        println!("Call started: {}. Listen at ws://{}/api/listen/{}", call_sid, server_url, call_sid);
                        println!("Waiting for audio data...");
                    } else if text.contains("\"media\"") {
                        let audio_data = extract_audio_from_twilio(text);
                        if !audio_data.is_empty() {
                            let _ = tx.send(audio_data);
                        } else {
                            println!("Failed to extract audio data from media packet");
                            println!("Media packet content: {}", text);
                        }
                    } else if text.contains("\"stop\"") {
                        let mut sessions_lock = sessions.lock().await;
                        sessions_lock.remove(&call_sid);
                        println!("Call ended: {}", call_sid);
                        break;
                    }
                }
            }
            Err(e) => eprintln!("WebSocket error: {:?}", e),
        }
    }
}

// WebSocket handler for listening to a specific call
pub async fn listen_handler(
    ws: WebSocketUpgrade,
    axum::extract::State(state): axum::extract::State<Arc<crate::AppState>>,
    Path(call_sid): Path<String>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_listen(socket, state.sessions.clone(), call_sid))
}

pub async fn handle_listen(mut socket: WebSocket, sessions: CallSessions, call_sid: String) {
    println!("Listener WebSocket connected for call: {}", call_sid);

    let rx = {
        let sessions_lock = sessions.lock().await;
        sessions_lock.get(&call_sid).map(|tx| tx.subscribe())
    };

    match rx {
        Some(mut rx) => {
            while let Ok(audio_data) = rx.recv().await {
                let msg = format!("Audio chunk received ({} bytes)", audio_data.len());
                socket.send(axum::extract::ws::Message::Text(msg.into())).await.unwrap_or_else(|e| eprintln!("WebSocket send error: {:?}", e));
            }
            println!("Listener stopped for call: {}", call_sid);
        }
        None => {
            socket.send(axum::extract::ws::Message::Text("Call not found".to_string().into())).await.unwrap();
        }
    }
}

// Process audio with Shazam and send SMS

pub async fn process_audio_with_shazam(state: Arc<ShazamState>) {
    let http_client = HttpClient::new();
    let account_sid = std::env::var("TWILIO_ACCOUNT_SID").expect("TWILIO_ACCOUNT_SID must be set");
    let auth_token = std::env::var("TWILIO_AUTH_TOKEN").expect("TWILIO_AUTH_TOKEN must be set");

    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
        let mut sessions_lock = state.sessions.lock().await;
        for (call_sid, tx) in sessions_lock.iter_mut() {
            let mut rx = tx.subscribe();
            let mut audio_buffer = Vec::new();

            let end_time = tokio::time::Instant::now() + tokio::time::Duration::from_secs(10);
            let mut packets_received = 0;
            
            while tokio::time::Instant::now() < end_time {
                if let Ok(audio_data) = rx.try_recv() {
                    audio_buffer.extend(audio_data);
                    packets_received += 1;
                }
                tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
            }

            if !audio_buffer.is_empty() {
                println!("\n=== Processing Audio ===");
                println!("Received {} packets, total {} bytes of audio for call {}", 
                    packets_received, audio_buffer.len(), call_sid);
                
                if audio_buffer.len() > 5120 {
                    let song_name = identify_with_shazam(&http_client, &audio_buffer).await;
                    println!("\n=== Shazam Result ===");
                    println!("Call {}: Identified '{}'", call_sid, song_name);

                    let to_number = {
                        let user_calls_lock = state.user_calls.lock().await;
                        if let Some(user_id) = user_calls_lock.get(call_sid) {
                            if let Ok(Some(user)) = state.user_repository.find_by_id(user_id.parse().unwrap()) {
                                user.phone_number
                            } else {
                                String::new()
                            }
                        } else {
                            String::new()
                        }
                    };

                    if !to_number.is_empty() && !song_name.starts_with("Error") && 
                       !song_name.starts_with("Failed") && !song_name.contains("Unknown song") {
                        println!("\n=== Song Identification Success ===");
                        println!("Successfully identified: {}", song_name);


                        match state.user_repository.find_by_phone_number(&to_number) {
                            Ok(Some(user)) => {
                                match crate::twilio_sms::send_shazam_answer_to_user(state.clone(), user.id, &song_name).await {
                                    Ok(_) => {
                                        println!("Successfully sent Shazam result to user {}: {}", user.id, song_name);
                                    }
                                    Err(e) => {
                                        eprintln!("Failed to send Shazam result to user {}: {}", user.id, e);
                                    }
                                }
                            }
                            Ok(None) => {
                                eprintln!("User not found for phone number: {}", to_number);
                            }
                            Err(e) => {
                                eprintln!("Error finding user by phone number {}: {}", to_number, e);
                            }
                        }
                    }
                }
            }
        }
    }
}

use std::env;

fn convert_mulaw_to_wav(mulaw_data: &[u8]) -> Vec<u8> {
    let mut wav_data = Vec::new();
    let spec = WavSpec {
        channels: 1,
        sample_rate: 8000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    
    let mut writer = WavWriter::new(Cursor::new(&mut wav_data), spec).unwrap();
    
    for &byte in mulaw_data {
        // Convert mu-law to 16-bit PCM
        let sample = mulaw_to_pcm(byte);
        writer.write_sample(sample).unwrap();
    }
    writer.finalize().unwrap();
    
    wav_data
}

fn mulaw_to_pcm(mulaw: u8) -> i16 {
    // Mu-law to PCM conversion table
    const MULAW_DECODE: [i16; 256] = [
        -32124, -31100, -30076, -29052, -28028, -27004, -25980, -24956,
        -23932, -22908, -21884, -20860, -19836, -18812, -17788, -16764,
        -15996, -15484, -14972, -14460, -13948, -13436, -12924, -12412,
        -11900, -11388, -10876, -10364, -9852, -9340, -8828, -8316,
        -7932, -7676, -7420, -7164, -6908, -6652, -6396, -6140,
        -5884, -5628, -5372, -5116, -4860, -4604, -4348, -4092,
        -3900, -3772, -3644, -3516, -3388, -3260, -3132, -3004,
        -2876, -2748, -2620, -2492, -2364, -2236, -2108, -1980,
        -1884, -1820, -1756, -1692, -1628, -1564, -1500, -1436,
        -1372, -1308, -1244, -1180, -1116, -1052, -988, -924,
        -876, -844, -812, -780, -748, -716, -684, -652,
        -620, -588, -556, -524, -492, -460, -428, -396,
        -372, -356, -340, -324, -308, -292, -276, -260,
        -244, -228, -212, -196, -180, -164, -148, -132,
        -120, -112, -104, -96, -88, -80, -72, -64,
        -56, -48, -40, -32, -24, -16, -8, 0,
        32124, 31100, 30076, 29052, 28028, 27004, 25980, 24956,
        23932, 22908, 21884, 20860, 19836, 18812, 17788, 16764,
        15996, 15484, 14972, 14460, 13948, 13436, 12924, 12412,
        11900, 11388, 10876, 10364, 9852, 9340, 8828, 8316,
        7932, 7676, 7420, 7164, 6908, 6652, 6396, 6140,
        5884, 5628, 5372, 5116, 4860, 4604, 4348, 4092,
        3900, 3772, 3644, 3516, 3388, 3260, 3132, 3004,
        2876, 2748, 2620, 2492, 2364, 2236, 2108, 1980,
        1884, 1820, 1756, 1692, 1628, 1564, 1500, 1436,
        1372, 1308, 1244, 1180, 1116, 1052, 988, 924,
        876, 844, 812, 780, 748, 716, 684, 652,
        620, 588, 556, 524, 492, 460, 428, 396,
        372, 356, 340, 324, 308, 292, 276, 260,
        244, 228, 212, 196, 180, 164, 148, 132,
        120, 112, 104, 96, 88, 80, 72, 64,
        56, 48, 40, 32, 24, 16, 8, 0
    ];
    MULAW_DECODE[mulaw as usize]
}

pub async fn identify_with_shazam(client: &HttpClient, audio: &[u8]) -> String {
    println!("\n=== Shazam API Request ===");
    println!("Converting {} bytes of mu-law audio to WAV", audio.len());
    
    // Convert mu-law to WAV
    let wav_data = convert_mulaw_to_wav(audio);
    println!("Converted to {} bytes of WAV data", wav_data.len());
    
    let api_key = env::var("SHAZAM_API_KEY").expect("SHAZAM_API_KEY must be set");
    println!("Using Shazam API key: {}...", &api_key[..10]);
    
    // Create a form with the WAV file
    let form = reqwest::multipart::Form::new()
        .part("file", 
            reqwest::multipart::Part::bytes(wav_data)
                .file_name("audio.wav")
                .mime_str("audio/wav")
                .expect("Failed to set mime type")
        );

    let response = client
        .post("https://shazam-core.p.rapidapi.com/v1/tracks/recognize")
        .header("X-RapidAPI-Key", api_key)
        .header("X-RapidAPI-Host", "shazam-core.p.rapidapi.com")
        .multipart(form)
        .send()
        .await;

    match response {
        Ok(resp) => {
            println!("Shazam API response status: {}", resp.status());
            if resp.status().is_success() {
                match resp.text().await {
                    Ok(text) => {
                        println!("Raw Shazam response: {}", text);
                        match serde_json::from_str::<Value>(&text) {
                            Ok(json) => {
                                println!("Parsed Shazam response: {}", serde_json::to_string_pretty(&json).unwrap_or_default());
                                if let Some(track) = json.get("track") {
                                    if let (Some(title), Some(artist)) = (track.get("title"), track.get("subtitle")) {
                                        let song_info = format!("{} by {}", 
                                            title.as_str().unwrap_or("Unknown"),
                                            artist.as_str().unwrap_or("Unknown Artist")
                                        );
                                        println!("Identified song: {}", song_info);
                                        song_info
                                    } else {
                                        "Unknown song".to_string()
                                    }
                                } else {
                                    println!("No track information found in response");
                                    "No track information found".to_string()
                                }
                            },
                            Err(e) => {
                                println!("Failed to parse Shazam response: {:?}", e);
                                "Error parsing Shazam response".to_string()
                            }
                        }
                    },
                    Err(e) => {
                        println!("Failed to get response text: {:?}", e);
                        "Error reading Shazam response".to_string()
                    }
                }
            } else {
                println!("Shazam API error status: {}", resp.status());
                format!("API error: {}", resp.status())
            }
        }
        Err(e) => {
            println!("Shazam API request failed: {:?}", e);
            "Failed to contact Shazam API".to_string()
        }
    }
}


fn extract_audio_from_twilio(payload: &str) -> Vec<u8> {
    match from_str::<Value>(payload) {
        Ok(json) => {
            match json["media"]["payload"].as_str() {
                Some(base64_audio) => {
                    match general_purpose::STANDARD.decode(base64_audio) {
                        Ok(audio_data) => audio_data,
                        Err(e) => {
                            println!("Failed to decode base64 audio: {:?}", e);
                            vec![]
                        }
                    }
                }
                None => {
                    println!("No payload found in media packet");
                    vec![]
                }
            }
        }
        Err(e) => {
            println!("Failed to parse JSON payload: {:?}", e);
            vec![]
        }
    }
}
