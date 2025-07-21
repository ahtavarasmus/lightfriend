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
    let is_voice_filled = false;

    // print sub_tier here
    web_sys::console::log_1(&format!("Current subscription tier: {:?}", sub_tier).into());
    let initial_page = if is_logged_in {
        match sub_tier.as_ref().map(|s| s.as_str()) {
            Some("self_hosted") => InstructionPage::Twilio,
            _ => InstructionPage::Server,
        }
    } else {
        InstructionPage::Server
    };
    // print initial_page here
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

    html! {
        <div class="instructions-container">
            <h1 class="main-header">{"Self-Host Instructions"}</h1>
            <div class="instructions-tabs">
                <button 
                    class={classes!("tab-button", (*current_page == InstructionPage::SetupCosts).then(|| "active"))}
                    onclick={let switch_page = switch_page.clone(); 
                        Callback::from(move |_| switch_page.emit(InstructionPage::SetupCosts))}
                    title={get_title(server_applicable, true)}
                >
                    /*<img src="/assets/" alt="" class="tab-logo" />
                    */
                    <i class="fa-solid fa-tag"></i>
                    {"Setup Costs"}
                </button>
                <button 
                    class={classes!("tab-button", (*current_page == InstructionPage::Server).then(|| "active"), server_applicable.then(|| if is_server_filled { "completed" } else { "" }), (!server_applicable).then(|| "disabled"))}
                    onclick={let switch_page = switch_page.clone(); 
                        Callback::from(move |_| switch_page.emit(InstructionPage::Server))}
                    title={get_title(server_applicable, true)}
                >
                    <img src="/assets/hostinger-logo.png" alt="" class="tab-logo" />
                    {if server_applicable { if is_server_filled { "Server Setup (Ready)".to_string() } else { "Server Setup (Required)".to_string() } } else { "Server Setup".to_string() }}
                </button>
                <button 
                    class={classes!("tab-button", (*current_page == InstructionPage::Twilio).then(|| "active"), twilio_applicable.then(|| if is_twilio_filled { "completed" } else { "" }), (!twilio_applicable).then(|| "disabled"))}
                    onclick={let switch_page = switch_page.clone(); 
                        Callback::from(move |_| switch_page.emit(InstructionPage::Twilio))}
                    title={get_title(twilio_applicable, false)}
                >
                    <img src="/assets/twilio-logo.png" alt="Twilio Logo" class="tab-logo" />
                    {if twilio_applicable { if is_twilio_filled { "Twilio Setup (Ready)".to_string() } else { "Twilio Setup (Required)".to_string() } } else { "Twilio Setup".to_string() }}
                </button>
                <button 
                    class={classes!("tab-button", (*current_page == InstructionPage::AI).then(|| "active"), ai_applicable.then(|| if is_ai_filled { "completed" } else { "" }), (!ai_applicable).then(|| "disabled"))}
                    onclick={let switch_page = switch_page.clone(); 
                        Callback::from(move |_| switch_page.emit(InstructionPage::AI))}
                    title={get_title(ai_applicable, false)}
                >
                    <img src="/assets/openrouter-logo.png" alt="OpenRouter Logo" class="tab-logo" />
                    {if ai_applicable { if is_ai_filled { "OpenRouter Setup (Ready)".to_string() } else { "OpenRouter Setup (Required)".to_string() } } else { "OpenRouter Setup".to_string() }}
                </button>
                <button 
                    class={classes!("tab-button", (*current_page == InstructionPage::Voice).then(|| "active"), voice_applicable.then(|| if is_voice_filled { "completed" } else { "" }), (!voice_applicable).then(|| "disabled"))}
                    onclick={let switch_page = switch_page.clone(); 
                        Callback::from(move |_| switch_page.emit(InstructionPage::Voice))}
                    title={get_title(voice_applicable, false)}
                >
                    <img src="/assets/elevenlabs-logo.png" alt="Elevenlabs Logo" class="tab-logo" />
                    {if voice_applicable { if is_voice_filled { "ElevenLabs Setup (Ready)".to_string() } else { "ElevenLabs Setup (Optional)".to_string() } } else { "ElevenLabs Setup".to_string() }}
                </button>
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

                .main-header {
                    text-align: center;
                    font-size: 2.5rem;
                    margin: 0;
                    color: #ffffff;
                    padding: 5rem 1rem 0;
                    background: rgba(26, 26, 26, 0.85);
                    backdrop-filter: blur(10px);
                }

                .instructions-tabs {
                    display: flex;
                    justify-content: center;
                    gap: 1rem;
                    padding: 1rem;
                    margin: 0;
                    border-bottom: 1px solid rgba(30, 144, 255, 0.1);
                    background: rgba(26, 26, 26, 0.85);
                    backdrop-filter: blur(10px);
                }

                .tab-button {
                    padding: 0.75rem 1.5rem;
                    background: transparent;
                    color: #999;
                    border: 1px solid rgba(30, 144, 255, 0.1);
                    border-radius: 6px;
                    cursor: pointer;
                    font-size: 1rem;
                    transition: all 0.3s ease;
                    display: flex;
                    align-items: center;
                    gap: 0.5rem;
                }

                .tab-logo {
                    width: 28px;
                    height: 28px;
                    object-fit: contain;
                }

                /* Specific adjustment for Twilio logo to match OpenRouter size visually */
                button:first-child .tab-logo {
                    width: 32px;
                    height: 32px;
                }

                .tab-button:hover {
                    border-color: rgba(30, 144, 255, 0.3);
                    color: #fff;
                }

                .tab-button.active {
                    background: rgba(30, 144, 255, 0.1);
                    border-color: rgba(30, 144, 255, 0.5);
                    color: #fff;
                }

                .tab-button.completed {
                    border-color: rgba(50, 205, 50, 0.5);
                    color: #fff;
                }

                .tab-button.completed:hover {
                    border-color: rgba(50, 205, 50, 0.7);
                }

                .tab-button.completed.active {
                    border-color: rgba(50, 205, 50, 0.7);
                }

                .tab-button.disabled {
                    color: #666;
                    border-color: rgba(30, 144, 255, 0.05);
                    font-size: 0.9rem;
                    opacity: 0.8;
                }

                .tab-button.disabled:hover {
                    border-color: rgba(30, 144, 255, 0.1);
                    color: #777;
                }

                .tab-button.disabled.active {
                    background: rgba(30, 144, 255, 0.05);
                    border-color: rgba(30, 144, 255, 0.3);
                    color: #999;
                }

                .instructions-content {
                    flex: 1;
                    width: 100%;
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
                        padding: 0.5rem;
                    }

                    .tab-button {
                        padding: 0.5rem 1rem;
                        font-size: 0.9rem;
                    }

                    .navigation-buttons {
                        bottom: 1rem;
                        right: 1rem;
                    }
                }
                "#}
            </style>
        </div>
    }
}
