use gloo_net::http::Request;
use wasm_bindgen_futures::spawn_local;
use web_sys::{HtmlTextAreaElement, window, MouseEvent};
use yew::prelude::*;
use crate::config;

#[derive(Properties, PartialEq)]
pub struct Props {
    pub on_update: Callback<()>,
    pub keywords: Vec<String>,
    pub priority_senders: Vec<String>,
    pub waiting_checks: Vec<String>,
    pub threshold: i32,
}

#[function_component(WhatsappGeneralChecks)]
pub fn whatsapp_general_checks(props: &Props) -> Html {
    let default_checks = "
    Step 1: Check for Urgency Indicators
    - Look for words like 'urgent', 'immediate', 'asap', 'deadline', 'important', 'emergency'
    - Check for time-sensitive phrases like 'by tomorrow', 'end of day', 'as soon as possible', 'right now'
    - Look for multiple exclamation marks or all-caps words that might indicate urgency
    - Check for repeated messages or follow-ups indicating urgency

    Step 2: Analyze Sender Importance
    - Check if it's from family members, close friends, or emergency contacts
    - Look for messages from work colleagues, managers, or supervisors
    - Consider if it's from clients or important business partners
    - Assess if it's from service providers (doctors, lawyers, etc.)

    Step 3: Assess Content Significance
    - Look for action items or direct requests that need immediate response
    - Check for mentions of meetings, appointments, or time-sensitive events
    - Identify emergency situations or health-related concerns
    - Look for financial matters, payments, or important transactions
    - Check for travel-related information or changes

    Step 4: Consider Context and Timing
    - Consider if it's outside normal hours (late night/early morning might indicate urgency)
    - Check if it's a reply to something you sent recently
    - Look for group messages where you're specifically mentioned
    - Consider if it's breaking a long silence in conversation

    Step 5: Evaluate Personal Impact
    - Assess if immediate action or response is required
    - Consider if delaying response could have negative consequences
    - Look for personal emergencies or family matters
    - Check for work-critical communications
    - Identify if it contains sensitive or confidential information
    ".trim();

    let checks = use_state(|| default_checks.to_string());
    let is_editing = use_state(|| false);
    let error_message = use_state(String::default);

    // Use the props data instead of sample data
    let waiting_checks = &props.waiting_checks;
    let priority_senders = &props.priority_senders;
    let keywords = &props.keywords;
    let threshold = props.threshold;

    // Format the full prompt with existing variables
    let full_prompt = format!(
        "You are an intelligent WhatsApp message filter designed to determine if a message is important enough to notify the user via SMS. \
        Your evaluation process has two main parts:\n\n\
        PART 1 - SPECIFIC FILTERS CHECK:\n\
        First, check if the message matches any user-defined 'waiting checks', priority senders, or keywords. These are absolute filters \
        that should trigger a notification if matched:\n\
        - Waiting Checks: {}\n\
        - Priority Senders: {}\n\
        - Keywords: {}\n\n\
        PART 2 - GENERAL IMPORTANCE ANALYSIS:\n\
        If no specific filters are matched, evaluate the message's importance using these general criteria:\n\
        {}\n\n\
        Based on all checks, assign an importance score from 0 (not important) to 10 (extremely important). \
        If the score meets or exceeds the user's threshold ({}), recommend sending an SMS notification.",
        waiting_checks.join(", "),
        priority_senders.join(", "),
        keywords.join(", "),
        *checks,
        threshold
    );

    // Fetch current checks on component mount
    {
        let checks = checks.clone();
        let error_message = error_message.clone();
        
        use_effect_with_deps(move |_| {
            spawn_local(async move {
                if let Some(token) = window()
                    .and_then(|w| w.local_storage().ok())
                    .flatten()
                    .and_then(|storage| storage.get_item("token").ok())
                    .flatten()
                {
                    let response = Request::get(&format!("{}/api/profile/whatsapp-general-checks", config::get_backend_url()))
                        .header("Authorization", &format!("Bearer {}", token))
                        .send()
                        .await;

                    match response {
                        Ok(resp) => {
                            if let Ok(data) = resp.json::<serde_json::Value>().await {
                                if let Some(checks_str) = data["checks"].as_str() {
                                    checks.set(checks_str.to_string());
                                }
                            }
                        }
                        Err(e) => error_message.set(format!("Failed to fetch checks: {}", e)),
                    }
                } else {
                    error_message.set("Not authenticated".to_string());
                }
            });
            
            || ()
        }, ());
    }

    let on_edit = {
        let is_editing = is_editing.clone();
        Callback::from(move |_: MouseEvent| {
            is_editing.set(true);
        })
    };
        let on_save = {
        let checks = checks.clone();
        let is_editing = is_editing.clone();
        let error_message = error_message.clone();
        let on_update = props.on_update.clone();
        
        Callback::from(move |_| {
            let checks_value = checks.clone();
            let error_message = error_message.clone();
            let is_editing = is_editing.clone();
            let on_update = on_update.clone();

            spawn_local(async move {
                if let Some(token) = window()
                    .and_then(|w| w.local_storage().ok())
                    .flatten()
                    .and_then(|storage| storage.get_item("token").ok())
                    .flatten()
                {
                    let request_body = serde_json::json!({
                        "checks": (*checks_value).clone()
                    });

                    let response = Request::post(&format!("{}/api/profile/whatsapp-general-checks", config::get_backend_url()))
                        .header("Authorization", &format!("Bearer {}", token))
                        .json(&request_body)
                        .expect("Failed to build request")
                        .send()
                        .await;

                    match response {
                        Ok(_) => {
                            is_editing.set(false);
                            error_message.set(String::new());
                            on_update.emit(());
                        }
                        Err(e) => error_message.set(format!("Failed to save: {}", e)),
                    }
                } else {
                    error_message.set("Not authenticated".to_string());
                }
            });
        })
    };

    // Keep track of the checks before editing started
    let temp_checks = use_state(String::default);

    let on_edit_start = {
        let is_editing = is_editing.clone();
        let checks = checks.clone();
        let temp_checks = temp_checks.clone();
        Callback::from(move |_: MouseEvent| {
            temp_checks.set((*checks).clone());
            is_editing.set(true);
        })
    };

    let on_cancel = {
        let is_editing = is_editing.clone();
        let checks = checks.clone();
        let temp_checks = temp_checks.clone();
        Callback::from(move |_| {
            checks.set((*temp_checks).clone());
            is_editing.set(false);
        })
    };

    let on_reset = {
        let checks = checks.clone();
        Callback::from(move |_: MouseEvent| {
            checks.set(default_checks.to_string());
        })
    };

    let on_change = {
        let checks = checks.clone();
        Callback::from(move |e: Event| {
            let target = e.target_dyn_into::<HtmlTextAreaElement>();
            if let Some(input) = target {
                checks.set(input.value());
            }
        })
    };

html! {
        <div class="whatsapp-checks-container">
            <div class="filter-section">
                <h3>{"General Importance Analysis"}</h3>
                <p class="description">{"Configure how WhatsApp messages are analyzed for general importance"}</p>
                
                if *is_editing {
                    <div class="edit-section">
                        <textarea
                            class="checks-textarea"
                            value={(*checks).clone()}
                            onchange={on_change}
                            placeholder="Enter your custom WhatsApp message analysis steps..."
                        />
                        <div class="button-group">
                            <button 
                                onclick={on_save}
                                class="save-button"
                            >
                                {"Save Changes"}
                            </button>
                            <button 
                                onclick={on_cancel}
                                class="cancel-button"
                            >
                                {"Cancel"}
                            </button>
                            <button 
                                onclick={on_reset}
                                class="reset-button"
                            >
                                {"Reset to Default"}
                            </button>
                        </div>
                    </div>
                } else {
                    <div class="view-section">
                        <div class="button-group">
                            <button 
                                onclick={on_edit_start}
                                class="edit-button"
                            >
                                {"Customize Analysis Checks"}
                            </button>
                        </div>
                    </div>
                }

                if !(*error_message).is_empty() {
                    <div class="error-message">
                        {(*error_message).clone()}
                    </div>
                }
            </div>

            <div class="filter-section prompt-showcase">
                <h3>{"Current Configuration Preview"}</h3>
                <p class="description">{"This is how your WhatsApp messages will be analyzed for importance"}</p>
                <pre class="prompt-content">
                    {full_prompt}
                </pre>
            </div>

            <style>
                {r#"
                .whatsapp-checks-container {
                    padding: 0;
                    margin: 0;
                    color: #fff;
                    display: flex;
                    flex-direction: column;
                }

                .whatsapp-checks-header {
                    margin-bottom: 2rem;
                    text-align: left;
                }

                .whatsapp-checks-header h3 {
                    color: #7EB2FF;
                    font-size: 1.5rem;
                    margin-bottom: 0.5rem;
                }

                .description {
                    color: rgba(255, 255, 255, 0.7);
                    font-size: 1rem;
                }

                .edit-section {
                    display: flex;
                    flex-direction: column;
                    gap: 1rem;
                    margin-top: 1rem;
                }

                .checks-textarea {
                    width: 100%;
                    min-height: 300px;
                    padding: 1rem;
                    background: rgba(30, 144, 255, 0.05);
                    border: 1px solid rgba(30, 144, 255, 0.1);
                    border-radius: 8px;
                    color: #fff;
                    font-size: 0.95rem;
                    line-height: 1.5;
                    resize: vertical;
                    font-family: monospace;
                }

                .checks-textarea:focus {
                    outline: none;
                    border-color: rgba(30, 144, 255, 0.3);
                    background: rgba(30, 144, 255, 0.08);
                    box-shadow: 0 0 15px rgba(30, 144, 255, 0.1);
                }

                .prompt-showcase {
                    background: rgba(30, 30, 30, 0.5);
                }

                .prompt-showcase pre {
                    background: rgba(30, 144, 255, 0.05);
                    border: 1px solid rgba(30, 144, 255, 0.1);
                    border-radius: 8px;
                    padding: 1.5rem;
                    margin-top: 1rem;
                    overflow-x: auto;
                    font-family: monospace;
                    font-size: 0.9rem;
                    line-height: 1.6;
                    color: #fff;
                }

                .checks-textarea {
                    width: 100%;
                    min-height: 300px;
                    padding: 1rem;
                    background: rgba(30, 144, 255, 0.05);
                    border: 1px solid rgba(30, 144, 255, 0.1);
                    border-radius: 8px;
                    color: #fff;
                    font-size: 0.95rem;
                    line-height: 1.5;
                    resize: vertical;
                    transition: all 0.3s ease;
                }

                .checks-textarea:focus {
                    outline: none;
                    border-color: rgba(30, 144, 255, 0.3);
                    background: rgba(30, 144, 255, 0.08);
                    box-shadow: 0 0 15px rgba(30, 144, 255, 0.1);
                }

                .button-group {
                    display: flex;
                    gap: 1rem;
                    margin-top: 1rem;
                }

                .save-button, .cancel-button, .edit-button, .reset-button {
                    padding: 0.8rem 1.5rem;
                    border: none;
                    border-radius: 8px;
                    font-size: 0.95rem;
                    cursor: pointer;
                    transition: all 0.3s ease;
                    text-transform: uppercase;
                    letter-spacing: 0.5px;
                }

                .save-button {
                    background: linear-gradient(45deg, #1E90FF, #4169E1);
                    color: white;
                }

                .save-button:hover {
                    transform: translateY(-2px);
                    box-shadow: 0 4px 20px rgba(30, 144, 255, 0.3);
                }

                .cancel-button {
                    background: rgba(255, 255, 255, 0.1);
                    color: #fff;
                }

                .cancel-button:hover {
                    background: rgba(255, 255, 255, 0.15);
                }

                .edit-button {
                    background: linear-gradient(45deg, #1E90FF, #4169E1);
                    color: white;
                    margin-bottom: 2rem;
                }

                .edit-button:hover {
                    transform: translateY(-2px);
                    box-shadow: 0 4px 20px rgba(30, 144, 255, 0.3);
                }

                .reset-button {
                    background: rgba(255, 99, 71, 0.1);
                    color: #ff6347;
                    border: 1px solid rgba(255, 99, 71, 0.3);
                }

                .reset-button:hover {
                    background: rgba(255, 99, 71, 0.2);
                }

                .error-message {
                    margin-top: 1rem;
                    padding: 1rem;
                    background: rgba(255, 71, 87, 0.1);
                    border: 1px solid rgba(255, 71, 87, 0.2);
                    border-radius: 8px;
                    color: #ff4757;
                    font-size: 0.9rem;
                }

                .prompt-showcase {
                    margin-top: 3rem;
                    padding-top: 2rem;
                    border-top: 1px solid rgba(30, 144, 255, 0.1);
                }

                .prompt-header {
                    margin-bottom: 1.5rem;
                }

                .prompt-header h4 {
                    color: #7EB2FF;
                    font-size: 1.2rem;
                    margin-bottom: 0.5rem;
                }

                .prompt-header p {
                    color: rgba(255, 255, 255, 0.7);
                    font-size: 0.9rem;
                }

                .prompt-content {
                    background: rgba(30, 144, 255, 0.05);
                    border: 1px solid rgba(30, 144, 255, 0.1);
                    border-radius: 8px;
                    padding: 1.5rem;
                    color: #fff;
                    font-size: 0.9rem;
                    line-height: 1.6;
                    white-space: pre-wrap;
                    overflow-x: auto;
                }

                @media (max-width: 768px) {
                    .whatsapp-checks-container {
                        padding: 0;
                        margin: 0;
                    }

                    .button-group {
                        flex-direction: column;
                    }

                    .save-button, .cancel-button, .edit-button {
                        width: 100%;
                    }

                    .prompt-content {
                        font-size: 0.85rem;
                        padding: 1rem;
                    }
                }
                "#}
            </style>
        </div>
    }
}

