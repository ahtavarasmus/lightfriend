use reqwest::Client;
use serde::{Serialize, Deserialize};
use chrono::Utc;

#[derive(Serialize, Deserialize)]
struct IngestionEvent {
    id: String,
    timestamp: String,
    #[serde(rename = "type")]
    r#type: String,
    body: TraceBody,
}

#[derive(Serialize, Deserialize)]
struct TraceBody {
    id: String,
    timestamp: String,
    name: String,
    #[serde(rename = "userId")]
    user_id: Option<String>,
    input: String,
    output: String,
    #[serde(rename = "sessionId")]
    session_id: Option<String>,
    metadata: Option<String>,
    tags: Vec<String>,
}

#[derive(Serialize, Deserialize)]
struct IngestionPayload {
    batch: Vec<IngestionEvent>,
}

pub async fn send_langfuse_trace(
    trace_id: String,
    user_id: Option<String>,
    input: String,
    output: String,
    session_id: Option<String>,
    processing_time_ms: u128,
    is_error: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::new();
    let timestamp = Utc::now().to_rfc3339();

    let public_key = std::env::var("LANGFUSE_PUBLIC_KEY").expect("LANGFUSE_PUBLIC_KEY not set");
    let secret_key= std::env::var("LANGFUSE_SECRET_KEY").expect("LANGFUSE_SECRET_KEY not set");

    let event = IngestionEvent {
        id: uuid::Uuid::new_v4().to_string(),
        timestamp: timestamp.clone(),
        r#type: "trace-create".to_string(),
        body: TraceBody {
            id: trace_id.clone(),
            timestamp,
            name: "incoming_sms_response".to_string(),
            user_id,
            input,
            output,
            session_id,
            metadata: Some(format!("processing_time_ms: {}", processing_time_ms)),
            tags: if is_error {
                vec!["error".to_string()]
            } else {
                vec!["success".to_string()]
            },
        },
    };
    let payload = IngestionPayload {
        batch: vec![event],
    };

    let response = client
        .post("https://cloud.langfuse.com/api/public/ingestion")
        .basic_auth(public_key, Some(secret_key))
        .json(&payload) // Batch with a single event
        .send()
        .await?;

    if response.status().is_success() {
        println!("Trace sent to Langfuse successfully");
    } else {
        eprintln!("Failed to send trace: {:?}", response.text().await?);
    }

    Ok(())
}
