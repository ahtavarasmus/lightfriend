use yew::prelude::*;
use web_sys::{MouseEvent, window};
use crate::pages::twilio_self_host_instructions::TwilioSelfHostInstructions;
use crate::pages::llm_self_host_instructions::AISelfHostInstructions;

#[derive(Clone, PartialEq)]
pub enum InstructionPage {
    Twilio,
    AI,
    // Add more pages here as needed
    // Example: Database,
    // Example: Environment,
}

#[derive(Properties, PartialEq)]
pub struct SelfHostInstructionsProps {
    #[prop_or_default]
    pub is_logged_in: bool,
    #[prop_or_default]
    pub sub_tier: Option<String>,
}

#[function_component(SelfHostInstructions)]
pub fn self_host_instructions(props: &SelfHostInstructionsProps) -> Html {
    let current_page = use_state(|| InstructionPage::Twilio);
    
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
            current_page.set(InstructionPage::AI);
            if let Some(window) = window() {
                let _ = window.scroll_to_with_x_and_y(0.0, 0.0);
            }
        })
    };

    html! {
        <div class="instructions-container">
            <h1 class="main-header">{"Self-Host Instructions"}</h1>
            <div class="instructions-tabs">
                <button 
                    class={classes!("tab-button", (*current_page == InstructionPage::Twilio).then(|| "active"))}
                    onclick={let switch_page = switch_page.clone(); 
                        Callback::from(move |_| switch_page.emit(InstructionPage::Twilio))}
                >
                    <img src="/assets/twilio-logo.png" alt="Twilio Logo" class="tab-logo" />
                    {"Twilio Setup"}
                </button>
                <button 
                    class={classes!("tab-button", (*current_page == InstructionPage::AI).then(|| "active"))}
                    onclick={let switch_page = switch_page.clone(); 
                        Callback::from(move |_| switch_page.emit(InstructionPage::AI))}
                >
                    <img src="/assets/openrouter-logo.png" alt="OpenRouter Logo" class="tab-logo" />
                    {"OpenRouter Setup"}
                </button>
            </div>

            <div class="instructions-content">
                {
                    match *current_page {
                        InstructionPage::Twilio => html! {
                            <TwilioSelfHostInstructions 
                                is_logged_in={props.is_logged_in}
                                sub_tier={props.sub_tier.clone()}
                            />
                        },
                        InstructionPage::AI => html! {
                            <AISelfHostInstructions 
                                is_logged_in={props.is_logged_in}
                                sub_tier={props.sub_tier.clone()}
                            />
                        },
                        // Add more matches here as more pages are added
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

