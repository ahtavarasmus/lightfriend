use yew::prelude::*;
use web_sys::{MouseEvent, window};
use crate::pages::twilio_self_host_instructions::TwilioSelfHostInstructions;
use crate::pages::llm_self_host_instructions::AISelfHostInstructions;
use crate::pages::voice_self_host_instructions::VoiceSelfHostInstructions;
use crate::pages::server_self_host_instructions::ServerSelfHostInstructions;
use crate::pages::setup_costs::SetupCosts;

#[derive(Clone, PartialEq, Debug)]
pub enum InstructionPage {
    Twilio,
    AI,
    Voice,
    Server,
    SetupCosts
}

#[derive(Properties, PartialEq)]
pub struct SelfHostInstructionsProps {
    #[prop_or_default]
    pub is_logged_in: bool,
    #[prop_or_default]
    pub sub_tier: Option<String>,
    #[prop_or_default]
    pub user_id: Option<String>,
    #[prop_or_default]
    pub server_ip: Option<String>,
    #[prop_or_default]
    pub twilio_phone: Option<String>,
    #[prop_or_default]
    pub twilio_sid: Option<String>,
    #[prop_or_default]
    pub twilio_token: Option<String>,
    #[prop_or_default]
    pub textbee_api_key: Option<String>,
    #[prop_or_default]
    pub textbee_device_id: Option<String>,
    #[prop_or_default]
    pub openrouter_api_key: Option<String>,
}

#[function_component(SelfHostInstructions)]
pub fn self_host_instructions(props: &SelfHostInstructionsProps) -> Html {
    let is_logged_in = props.is_logged_in;
    let sub_tier = props.sub_tier.clone();

    let server_applicable = is_logged_in && sub_tier.as_ref().map_or(false, |t| t == "tier 3");
    let twilio_applicable = is_logged_in && sub_tier.as_ref().map_or(false, |t| t == "self_hosted");
    let ai_applicable = twilio_applicable;
    let voice_applicable = twilio_applicable;

    let is_server_filled = props.server_ip.as_deref().map_or(false, |s| !s.is_empty());
    let is_twilio_filled = [&props.twilio_phone, &props.twilio_sid, &props.twilio_token]
        .iter()
        .all(|o| o.as_deref().map_or(false, |s| !s.is_empty()));
    let is_ai_filled = props.openrouter_api_key.as_deref().map_or(false, |s| !s.is_empty());
    let is_voice_filled = false; // Update this to true based on Voice props if added

    web_sys::console::log_1(&format!("Current subscription tier: {:?}", sub_tier).into());
    let initial_page = if is_logged_in {
        match sub_tier.as_ref().map(|s| s.as_str()) {
            Some("self_hosted") => InstructionPage::Twilio,
            _ => InstructionPage::Server,
        }
    } else {
        InstructionPage::SetupCosts
    };
    web_sys::console::log_1(&format!("Initial page: {:#?}", initial_page).into());

    let current_page = use_state(|| initial_page);

    let current_page_effect = current_page.clone();
    use_effect_with_deps(
        move |(is_logged_in, sub_tier)| {
            let new_initial = if *is_logged_in {
                match sub_tier.as_ref().map(|s| s.as_str()) {
                    Some("self_hosted") => InstructionPage::Twilio,
                    _ => InstructionPage::Server,
                }
            } else {
                InstructionPage::SetupCosts
            };
            current_page_effect.set(new_initial);
            || ()
        },
        (is_logged_in, sub_tier.clone()),
    );

    let switch_page = {
        let current_page = current_page.clone();
        Callback::from(move |page: InstructionPage| {
            current_page.set(page);
            if let Some(window) = window() {
                let _ = window.scroll_to_with_x_and_y(0.0, 0.0);
            }
        })
    };

    let next_page = {
        let current_page = current_page.clone();
        Callback::from(move |_: MouseEvent| {
            let next = match *current_page {
                InstructionPage::Server => InstructionPage::SetupCosts,
                InstructionPage::SetupCosts => InstructionPage::Twilio,
                InstructionPage::Twilio => InstructionPage::AI,
                InstructionPage::AI => InstructionPage::Voice,
                InstructionPage::Voice => InstructionPage::Server,
            };
            current_page.set(next);
            if let Some(window) = window() {
                let _ = window.scroll_to_with_x_and_y(0.0, 0.0);
            }
        })
    };

    let get_title = |applicable: bool, is_server: bool| {
        if !is_logged_in {
            "Please log in to configure this setup.".to_string()
        } else {
            let tier_matches = if is_server {
                sub_tier.as_ref().map_or(false, |t| t == "tier 3")
            } else {
                sub_tier.as_ref().map_or(false, |t| t == "self_hosted")
            };
            if tier_matches {
                "".to_string()
            } else if sub_tier.is_none() || sub_tier.as_ref().map(|t| t != "tier 3" && t != "self_hosted").unwrap_or(false) {
                "Please subscribe to the self-hosted subscription to access this setup.".to_string()
            } else if is_server {
                "Fill this on your lightfriend.ai account (you are currently in the self hosted version)".to_string()
            } else {
                "Fill this on your self hosted lightfriend server (you are currently in main lightfriend.ai server)".to_string()
            }
        }
    };

    // Calculate progress for gamification
    // Assuming Text and Photo both tied to Twilio (2 points), Calling to Voice (1), AI (1), Server (1) - total 5
    let completed = (is_server_filled as u32) + (is_twilio_filled as u32 * 2) + (is_ai_filled as u32) + (is_voice_filled as u32);
    let progress = ((completed as f32 / 5.0) * 100.0) as u32;

    html! {
        <div class="instructions-container">
            <div class="instructions-tabs">
                <div class="nav-section">
                    <span class="section-label">{"Channels"}</span>
                    <div class="sub-section">
                        <span class="sub-label">{"Messaging"}</span>
                        <button 
                            class={classes!("bubble", if is_twilio_filled { "green" } else { "gray" }, (*current_page == InstructionPage::Twilio).then(|| "active"))}
                            onclick={let switch_page = switch_page.clone(); 
                                Callback::from(move |_| switch_page.emit(InstructionPage::Twilio))}
                            title={if is_twilio_filled { "Text (Ready)" } else { "Text (Required)" }}
                        >
                            {"Text"}
                        </button>
                        <button 
                            class={classes!("bubble", if is_twilio_filled { "green" } else { "gray" }, (*current_page == InstructionPage::Twilio).then(|| "active"))}
                            onclick={let switch_page = switch_page.clone(); 
                                Callback::from(move |_| switch_page.emit(InstructionPage::Twilio))}
                            title={if is_twilio_filled { "Photo (Ready)" } else { "Photo (Required)" }}
                        >
                            {"Photo"}
                        </button>
                    </div>
                    <div class="sub-section">
                        <span class="sub-label">{"Calling"}</span>
                        <button 
                            class={classes!("bubble", if is_voice_filled { "green" } else { "gray" }, (*current_page == InstructionPage::Voice).then(|| "active"))}
                            onclick={let switch_page = switch_page.clone(); 
                                Callback::from(move |_| switch_page.emit(InstructionPage::Voice))}
                            title={if is_voice_filled { "Calling (Ready)" } else { "Calling (Optional)" }}
                        >
                            {"Calling"}
                        </button>
                    </div>
                </div>
                <div class="nav-section">
                    <span class="section-label">{"Features"}</span>
                    <div class="sub-section">
                        <button 
                            class={classes!("bubble", "gray", (*current_page == InstructionPage::SetupCosts).then(|| "active"))}
                            onclick={let switch_page = switch_page.clone(); 
                                Callback::from(move |_| switch_page.emit(InstructionPage::SetupCosts))}
                            title={"Setup Costs"}
                        >
                            {"Costs"}
                        </button>
                        <button 
                            class={classes!("bubble", if is_server_filled { "green" } else { "gray" }, (*current_page == InstructionPage::Server).then(|| "active"))}
                            onclick={let switch_page = switch_page.clone(); 
                                Callback::from(move |_| switch_page.emit(InstructionPage::Server))}
                            title={if is_server_filled { "Server (Ready)" } else { "Server (Required)" }}
                        >
                            {"Server"}
                        </button>
                        <button 
                            class={classes!("bubble", if is_ai_filled { "green" } else { "gray" }, (*current_page == InstructionPage::AI).then(|| "active"))}
                            onclick={let switch_page = switch_page.clone(); 
                                Callback::from(move |_| switch_page.emit(InstructionPage::AI))}
                            title={if is_ai_filled { "AI (Ready)" } else { "AI (Required)" }}
                        >
                            {"AI"}
                        </button>
                    </div>
                </div>
                <div class="progress-container">
                    <span>{format!("Progress: {}%", progress)}</span>
                    <div class="progress-bg">
                        <div class="progress-bar" style={format!("width: {}%;", progress)}></div>
                    </div>
                </div>
            </div>

            <div class="instructions-content">
                {
                    match *current_page {
                        InstructionPage::SetupCosts => html! {
                            <SetupCosts />
                        },
                        InstructionPage::Server => html! {
                            <ServerSelfHostInstructions 
                                is_logged_in={props.is_logged_in}
                                user_id={props.user_id.clone()}
                                sub_tier={props.sub_tier.clone()}
                                server_ip={props.server_ip.clone()}
                                message={get_title(server_applicable, true)}
                            />
                        },
                        InstructionPage::Twilio => html! {
                            <TwilioSelfHostInstructions 
                                is_logged_in={props.is_logged_in}
                                sub_tier={props.sub_tier.clone()}
                                twilio_phone={props.twilio_phone.clone()}
                                twilio_sid={props.twilio_sid.clone()}
                                twilio_token={props.twilio_token.clone()}
                                message={get_title(twilio_applicable, false)}
                                textbee_api_key={props.textbee_api_key.clone()}
                                textbee_device_id={props.textbee_device_id.clone()}
                            />
                        },
                        InstructionPage::AI => html! {
                            <AISelfHostInstructions 
                                is_logged_in={props.is_logged_in}
                                sub_tier={props.sub_tier.clone()}
                                api_key={props.openrouter_api_key.clone()}
                                message={get_title(ai_applicable, false)}
                            />
                        },
                        InstructionPage::Voice => html! {
                            <VoiceSelfHostInstructions 
                                is_logged_in={props.is_logged_in}
                                sub_tier={props.sub_tier.clone()}
                                message={get_title(voice_applicable, false)}
                            />
                        },
                    }
                }
            </div>

            <div class="navigation-buttons">
                <button class="next-button" onclick={next_page}>
                    {"Next"}
                </button>
            </div>

            <style>
                {r#"
                .instructions-container {
                    width: 100%;
                    min-height: 100vh;
                    display: flex;
                    flex-direction: column;
                }

                .instructions-tabs {
                    position: fixed;
                    top: 60px;
                    left: 0;
                    width: 100%;
                    display: flex;
                    justify-content: center;
                    align-items: center;
                    gap: 2rem;
                    padding: 0.5rem;
                    background: rgba(26, 26, 26, 0.95);
                    backdrop-filter: blur(10px);
                    border-bottom: 1px solid rgba(30, 144, 255, 0.1);
                    z-index: 1000;
                    overflow-x: auto;
                }

                .nav-section {
                    display: flex;
                    flex-direction: column;
                    align-items: center;
                    gap: 0.5rem;
                    padding: 0 1rem;
                    border-right: 1px solid rgba(30, 144, 255, 0.1);
                }

                .nav-section:last-child {
                    border-right: none;
                }

                .section-label {
                    font-weight: bold;
                    color: #fff;
                    font-size: 1rem;
                }

                .sub-section {
                    display: flex;
                    align-items: center;
                    gap: 0.5rem;
                }

                .sub-label {
                    color: #ccc;
                    font-size: 0.9rem;
                    margin-right: 0.5rem;
                }

                .bubble {
                    width: 80px;
                    height: 40px;
                    border-radius: 20px;
                    background: transparent;
                    border: 2px solid #999;
                    color: #999;
                    cursor: pointer;
                    font-size: 0.9rem;
                    display: flex;
                    align-items: center;
                    justify-content: center;
                    transition: transform 0.2s, border 0.2s, background 0.2s;
                }

                .bubble:hover {
                    transform: scale(1.05);
                }

                .bubble.green {
                    background: rgba(50, 205, 50, 0.2);
                    border: 2px solid #32CD32;
                    color: #fff;
                    animation: pulse 0.5s;
                }

                .bubble.gray {
                    background: rgba(128, 128, 128, 0.1);
                    border: 2px solid #999;
                    color: #999;
                }

                .bubble.active {
                    box-shadow: 0 0 10px rgba(30, 144, 255, 0.5);
                }

                @keyframes pulse {
                    0% { transform: scale(1); }
                    50% { transform: scale(1.1); }
                    100% { transform: scale(1); }
                }

                .progress-container {
                    display: flex;
                    flex-direction: column;
                    align-items: center;
                    gap: 0.25rem;
                }

                .progress-container span {
                    color: #fff;
                    font-size: 0.9rem;
                }

                .progress-bg {
                    width: 100px;
                    height: 5px;
                    background: #333;
                    border-radius: 5px;
                    overflow: hidden;
                }

                .progress-bar {
                    height: 100%;
                    background: #32CD32;
                    transition: width 0.3s ease;
                }

                .instructions-content {
                    flex: 1;
                    width: 100%;
                    padding-top: 120px;
                }

                .navigation-buttons {
                    position: fixed;
                    bottom: 2rem;
                    right: 2rem;
                    display: flex;
                    gap: 1rem;
                    z-index: 100;
                }

                .next-button {
                    padding: 0.75rem 2rem;
                    background: #1E90FF;
                    color: white;
                    border: none;
                    border-radius: 6px;
                    cursor: pointer;
                    font-size: 1rem;
                    transition: all 0.3s ease;
                    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.3);
                }

                .next-button:hover {
                    background: #1976D2;
                    transform: translateY(-2px);
                    box-shadow: 0 6px 16px rgba(0, 0, 0, 0.4);
                }

                .next-button:active {
                    transform: translateY(0);
                }

                @media (max-width: 768px) {
                    .instructions-tabs {
                        justify-content: flex-start;
                        padding: 0.5rem 1rem;
                        gap: 1rem;
                    }

                    .nav-section {
                        align-items: flex-start;
                    }

                    .bubble {
                        width: 60px;
                        height: 30px;
                        font-size: 0.8rem;
                    }

                    .navigation-buttons {
                        bottom: 1rem;
                        right: 1rem;
                    }

                    .progress-bg {
                        width: 80px;
                    }
                }
                "#}
            </style>
        </div>
    }
}
