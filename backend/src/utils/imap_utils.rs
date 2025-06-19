use crate::AppState;
use std::sync::Arc;
use tracing::{info, error};
use std::env;
use openai_api_rs;
use serde_json;
use reqwest;
use serde::Deserialize;

use crate::handlers::imap_handlers::ImapEmailPreview;

use reqwest::multipart;

#[derive(Debug, Deserialize)]
struct TwilioMediaResponse {
    sid: String,
    links: TwilioMediaLinks,
}

#[derive(Debug, Deserialize)]
struct TwilioMediaLinks {
    content_direct_temporary: String,
}

pub async fn upload_media_to_twilio(
    content_type: String,
    data: Vec<u8>,
    filename: String,
) -> Result<String, Box<dyn std::error::Error>> {
    let twilio_account_sid = env::var("TWILIO_ACCOUNT_SID")
        .map_err(|_| "TWILIO_ACCOUNT_SID not set")?;
    let twilio_auth_token = env::var("TWILIO_AUTH_TOKEN")
        .map_err(|_| "TWILIO_AUTH_TOKEN not set")?;
    let twilio_chat_service = env::var("TWILIO_CHAT_SERVICE")
        .map_err(|_| "TWILIO_CHAT_SERVICE not set")?;
    
    let client = reqwest::Client::new();
    
    let url = format!(
        "https://mcs.us1.twilio.com/v1/Services/{}/Media",
        twilio_chat_service
    );

    // Create multipart form data
    let part = multipart::Part::bytes(data)
        .file_name(filename.clone())
        .mime_str(&content_type)?;
    
    let form = multipart::Form::new()
        .part("file", part);
    
    let response = client
        .post(&url)
        .basic_auth(&twilio_account_sid, Some(&twilio_auth_token))
        .multipart(form)
        .send()
        .await?;
    
    if !response.status().is_success() {
        let error_text = response.text().await?;
        return Err(format!("Failed to upload media: {}", error_text).into());
    }
    
    let media_response: TwilioMediaResponse = response.json().await?;
    Ok(media_response.links.content_direct_temporary)
}


pub async fn judge_email_importance(
    state: &Arc<AppState>,
    user_id: i32,
    emails: Vec<ImapEmailPreview>,
    importance_priority: i32,
    last_activated: i32,
) -> Result<Vec<(ImapEmailPreview, serde_json::Value)>, Box<dyn std::error::Error>> {
    let mut important_emails = Vec::new();

    // Get filter activation settings
    let (keywords_active, priority_senders_active, waiting_checks_active, general_importance_active) = 
        match state.user_repository.get_email_filter_settings(user_id) {
            Ok(settings) => settings,
            Err(e) => {
                error!("Failed to get filter settings for user {}: {}", user_id, e);
                (true, true, true, true) // Default to all active on error
            }
        };

    // Only fetch active filters
    let waiting_checks = if waiting_checks_active {
        match state.user_repository.get_waiting_checks(user_id, "imap") {
            Ok(checks) => checks,
            Err(e) => {
                error!("Failed to get waiting checks for user {}: {}", user_id, e);
                Vec::new()
            }
        }
    } else {
        Vec::new()
    };

    let priority_senders = if priority_senders_active {
        match state.user_repository.get_priority_senders(user_id, "imap") {
            Ok(senders) => senders,
            Err(e) => {
                error!("Failed to get priority senders for user {}: {}", user_id, e);
                Vec::new()
            }
        }
    } else {
        Vec::new()
    };

    let keywords = if keywords_active {
        match state.user_repository.get_keywords(user_id, "imap") {
            Ok(kw) => kw,
            Err(e) => {
                error!("Failed to get keywords for user {}: {}", user_id, e);
                Vec::new()
            }
        }
    } else {
        Vec::new()
    };

    // Get user's custom general checks prompt or use default
    let general_checks_prompt = match state.user_repository.get_imap_general_checks(user_id) {
        Ok(prompt) => {
            info!("Using custom general checks prompt for user {}", user_id);
            prompt
        },
        Err(e) => {
            error!("Failed to get general checks prompt for user {}: {}", user_id, e);
            return Err(Box::new(e));
        }
    };

    let waiting_checks_formatted = waiting_checks.iter()
        .map(|wc| format!("{{id: {}, content: '{}'}}", wc.id.unwrap_or(-1), wc.content))
        .collect::<Vec<_>>()
        .join(", ");

    let system_message = format!(
        "You are an intelligent email filter designed to determine if an email is important enough to notify the user via SMS. \
        Your evaluation process has two main parts:\n\n\
        PART 1 - SPECIFIC FILTERS CHECK:\n\
        First, check if the email matches any user-defined 'waiting checks', priority senders, or keywords. These are absolute filters \
        that should trigger a notification if matched:\n\
        - Waiting Checks: {}\n\
        - Priority Senders: {}\n\
        - Keywords: {}\n\n\
        PART 2 - GENERAL IMPORTANCE ANALYSIS:\n\
        If no specific filters are matched, evaluate the email's importance using these general criteria:\n\
        {}\n\n\
        Based on all checks, assign an importance score from 0 (not important) to 10 (extremely important). \
        If the score meets or exceeds the user's threshold ({}), recommend sending an SMS notification.\n\n\
        When a waiting check matches, you MUST include its ID in the matched_waiting_check field.\n\n\
        Return a JSON object with the following structure:\n\
        {{\n\
            'should_notify': true/false,\n\
            'reason': 'explanation',\n\
            'score': number (if applicable),\n\
            'matched_waiting_check': number (the ID of the matched waiting check, if any)\n\
        }}",
        waiting_checks_formatted,
        priority_senders.iter().map(|ps| ps.sender.clone()).collect::<Vec<_>>().join(", "),
        keywords.iter().map(|k| k.keyword.clone()).collect::<Vec<_>>().join(", "),
        general_checks_prompt,
        importance_priority
    );

    let api_key = env::var("OPENROUTER_API_KEY")
        .map_err(|e| {
            error!("Failed to get OPENROUTER_API_KEY: {}", e);
            e
        })?;

    let client = openai_api_rs::v1::api::OpenAIClient::builder()
        .with_endpoint("https://openrouter.ai/api/v1")
        .with_api_key(api_key)
        .build()
        .map_err(|e| {
            error!("Failed to build OpenAI client: {}", e);
            e
        })?;

    // Process each email
    for email in emails {
        // Create email judgment log regardless of notification decision
        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i32;

        let email_timestamp = email.date
                .map(|dt| dt.timestamp() as i32)
                .unwrap_or(current_time);

        // Skip emails that are older than the last activation time
        if email_timestamp <= last_activated {
            info!("Skipping email with timestamp {} as it's older than last activation time {}", email_timestamp, last_activated);
            continue;
        }
        let from_email_str = email.from_email.clone().as_deref().unwrap_or("Unknown").to_string();
        let subject = email.subject.as_deref().unwrap_or("No subject");
        let body = email.body.as_deref().unwrap_or("No content");

        let mut skip_rest = false;
        // FAST CHECKS FIRST - Check waiting checks (exact string matching) if active
        if waiting_checks_active {
            for waiting_check in &waiting_checks {
                if body.to_lowercase().contains(&waiting_check.content.to_lowercase()) ||
                   subject.to_lowercase().contains(&waiting_check.content.to_lowercase()) {
                    info!("Fast check: Waiting check matched for user {}: '{}'", user_id, waiting_check.content);
                    
                    let evaluation = serde_json::json!({
                        "should_notify": true,
                        "reason": format!("Matched waiting check: {}", waiting_check.content),
                        "score": 10,
                        "matched_waiting_check": waiting_check.id,
                        "email_timestamp": email_timestamp,
                    });
                    
                    important_emails.push((email.clone(), evaluation));
                    skip_rest = true;
                    break;
                }
            }
        }
        if skip_rest {continue;}

        // FAST CHECKS SECOND - Check priority senders if active
        if priority_senders_active {
            for priority_sender in &priority_senders {
                if from_email_str.to_lowercase().contains(&priority_sender.sender.to_lowercase()) {
                    info!("Fast check: Priority sender matched for user {}: '{}'", user_id, priority_sender.sender);
                    
                    let evaluation = serde_json::json!({
                        "should_notify": true,
                        "reason": format!("Message from priority sender: {}", priority_sender.sender),
                        "score": 10,
                        "matched_waiting_check": null,
                        "email_timestamp": email_timestamp,
                    });
                    
                    important_emails.push((email.clone(), evaluation));
                    continue;
                }
            }
        }
        if skip_rest {continue;}

        // FAST CHECKS THIRD - Check keywords if active
        if keywords_active {
            for keyword in &keywords {
                if body.to_lowercase().contains(&keyword.keyword.to_lowercase()) ||
                   subject.to_lowercase().contains(&keyword.keyword.to_lowercase()) {
                    info!("Fast check: Keyword matched for user {}: '{}'", user_id, keyword.keyword);
                    
                    let evaluation = serde_json::json!({
                        "should_notify": true,
                        "reason": format!("Matched keyword: {}", keyword.keyword),
                        "score": 10,
                        "matched_waiting_check": null,
                        "email_timestamp": email_timestamp,
                    });
                    
                    important_emails.push((email.clone(), evaluation));
                    break;
                }
            }
        }
        if skip_rest {continue;}

        // FALLBACK TO LLM - Only if no fast checks matched and general importance is active
        if !general_importance_active {
            info!("General importance check is disabled for user {}, skipping LLM evaluation", user_id);
            continue;
        }
        info!("No fast checks matched, falling back to LLM evaluation for user {}", user_id);

        let email_content = format!(
            "From: {}\nSubject: {}\nBody: {}",
            from_email_str,
            subject,
            body
        );

        // Define the tool for email evaluation
        let mut email_eval_properties = std::collections::HashMap::new();
        email_eval_properties.insert(
            "should_notify".to_string(),
            Box::new(openai_api_rs::v1::types::JSONSchemaDefine {
                schema_type: Some(openai_api_rs::v1::types::JSONSchemaType::Boolean),
                description: Some("Whether the user should be notified about this email".to_string()),
                ..Default::default()
            }),
        );
        email_eval_properties.insert(
            "reason".to_string(),
            Box::new(openai_api_rs::v1::types::JSONSchemaDefine {
                schema_type: Some(openai_api_rs::v1::types::JSONSchemaType::String),
                description: Some("Explanation for why the user should or should not be notified".to_string()),
                ..Default::default()
            }),
        );
        email_eval_properties.insert(
            "score".to_string(),
            Box::new(openai_api_rs::v1::types::JSONSchemaDefine {
                schema_type: Some(openai_api_rs::v1::types::JSONSchemaType::Number),
                description: Some("Importance score from 0 to 10".to_string()),
                ..Default::default()
            }),
        );
        email_eval_properties.insert(
            "matched_waiting_check".to_string(),
            Box::new(openai_api_rs::v1::types::JSONSchemaDefine {
                schema_type: Some(openai_api_rs::v1::types::JSONSchemaType::Number),
                description: Some("The ID of the waiting check that was matched, if any. Must be the exact ID from the waiting checks list.".to_string()),
                ..Default::default()
            }),
        );

        let tools = vec![
            openai_api_rs::v1::chat_completion::Tool {
                r#type: openai_api_rs::v1::chat_completion::ToolType::Function,
                function: openai_api_rs::v1::types::Function {
                    name: String::from("evaluate_email"),
                    description: Some(String::from("Evaluate email importance and determine if notification is needed")),
                    parameters: openai_api_rs::v1::types::FunctionParameters {
                        schema_type: openai_api_rs::v1::types::JSONSchemaType::Object,
                        properties: Some(email_eval_properties),
                        required: Some(vec![
                            String::from("should_notify"),
                            String::from("reason"),
                            String::from("score"),
                            String::from("matched_waiting_check"),
                        ]),
                    },
                },
            },
        ];

        let messages = vec![
            openai_api_rs::v1::chat_completion::ChatCompletionMessage {
                role: openai_api_rs::v1::chat_completion::MessageRole::system,
                content: openai_api_rs::v1::chat_completion::Content::Text(system_message.clone()),
                name: None,
                tool_calls: None,
                tool_call_id: None,
            },
            openai_api_rs::v1::chat_completion::ChatCompletionMessage {
                role: openai_api_rs::v1::chat_completion::MessageRole::user,
                content: openai_api_rs::v1::chat_completion::Content::Text(email_content.clone()),
                name: None,
                tool_calls: None,
                tool_call_id: None,
            },
        ];

        let req = openai_api_rs::v1::chat_completion::ChatCompletionRequest::new(
            "meta-llama/llama-4-maverick".to_string(),
            messages,
        )
        .tools(tools)
        .tool_choice(openai_api_rs::v1::chat_completion::ToolChoiceType::Required);

        match client.chat_completion(req.clone()).await {
            Ok(response) => {
                info!("Received LLM response: {:?}", response);
                if let Some(tool_calls) = response.choices.first().and_then(|choice| choice.message.tool_calls.as_ref()) {
                    for tool_call in tool_calls {
                        if tool_call.function.name.as_deref() == Some("evaluate_email") {
                            if let Some(arguments) = &tool_call.function.arguments {
                                info!("Processing tool call arguments");
                                match serde_json::from_str::<serde_json::Value>(arguments) {
                                    Ok(mut evaluation) => {
                                        if evaluation["should_notify"].as_bool().unwrap_or(false) {
                                            // Add email timestamp to evaluation
                                            evaluation["email_timestamp"] = serde_json::json!(email_timestamp as i32);
                                            
                                            // Ensure all required fields are present with defaults if needed
                                            if evaluation.get("score").is_none() {
                                                evaluation["score"] = serde_json::json!(10); // Default score
                                            }
                                            if evaluation.get("matched_waiting_check").is_none() {
                                                evaluation["matched_waiting_check"] = serde_json::json!(null);
                                            }
                                            if evaluation.get("reason").is_none() {
                                                evaluation["reason"] = serde_json::json!("Email deemed important by AI evaluation");
                                            }
                                            
                                            important_emails.push((email.clone(), evaluation));
                                        }
                                    }
                                    Err(e) => {
                                        error!("Failed to parse tool call arguments: {}", e);
                                        error!("Raw arguments that failed to parse: {}", arguments);
                                        // Continue processing other tool calls
                                        continue;
                                    }
                                }
                            }
                        }
                    }
                } else {
                    error!("No valid tool calls found in LLM response");
                }
            }
            Err(e) => {
                error!("Failed to get LLM response: {}", e);
                // Continue with next email
                continue;
            }
        }
    }

    Ok(important_emails)
}

