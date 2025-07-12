use yew::prelude::*;
use web_sys::MouseEvent;

#[derive(Properties, PartialEq)]
pub struct TwilioSelfHostInstructionsProps {
    #[prop_or_default]
    pub is_logged_in: bool,
    #[prop_or_default]
    pub sub_tier: Option<String>,
}

#[function_component(TwilioSelfHostInstructions)]
pub fn twilio_self_host_instructions(props: &TwilioSelfHostInstructionsProps) -> Html {
    let modal_visible = use_state(|| false);
    let selected_image = use_state(|| String::new());

    let close_modal = {
        let modal_visible = modal_visible.clone();
        Callback::from(move |_: MouseEvent| {
            modal_visible.set(false);
        })
    };

    let open_modal = {
        let modal_visible = modal_visible.clone();
        let selected_image = selected_image.clone();
        Callback::from(move |src: String| {
            selected_image.set(src);
            modal_visible.set(true);
        })
    };
    html! {
        <div class="instructions-page">
            <div class="instructions-background"></div>
            <section class="instructions-hero">
                <h1>{"Twilio Self-Host Instructions"}</h1>
                <p>{"Step by step guide to setup self-host with Twilio"}</p>
            </section>

            <section class="instructions-section">
                <div class="instruction-block">
                    <div class="instruction-content">
                        <h2>{"Sign up and Add Funds"}</h2>
                        <ul>
                            <li>{"Go to Twilio's website (twilio.com) and click 'Sign up'"}</li>
                            <li>{"Complete the registration process with your email and other required information"}</li>
                            <li>{"Once registered, you'll need to add funds to your account:"}</li>
                            <li>{"1. Click on 'Admin' in the top right"}</li>
                            <li>{"2. Select 'Account billing' from the dropdown"}</li>
                            <li>{"3. Click 'Add funds' on the new billing page that opens up and input desired amount (minimum usually $20)"}</li>
                            <li>{"After adding funds, your account will be ready to purchase a phone number"}</li>
                        </ul>
                    </div>
                    <div class="instruction-image">
                        <img 
                            src="/assets/billing-twilio.png" 
                            alt="Navigating to Twilio Billing Page" 
                            loading="lazy"
                            onclick={let open_modal = open_modal.clone(); let src = "/assets/billing-twilio.png".to_string(); 
                                Callback::from(move |_| open_modal.emit(src.clone()))}
                            style="cursor: pointer;"
                        />
                    </div>
                </div>

                <div class="instruction-block">
                    <div class="instruction-content">
                        <h2>{"Buy a phone number"}</h2>
                        <ul>
                            <li>{"1. On the Twilio Dashboard, click on the 'Phone Numbers' button in the left sidebar when 'Develop' is selected above."}</li>
                            <li>{"2. Click the 'Buy a number' button under the new sub menu"}</li>
                            <li>{"3. Use the country search box to select your desired country"}</li>
                            <li>{"4. (Optional) Use advanced search options to find specific number types"}</li>
                            <li>{"5. Check the capabilities column to ensure the number supports your needs (Voice, SMS, MMS, etc.)"}</li>
                            <li>{"6. Click the 'Buy' button next to your chosen number and follow the steps"}</li>
                        </ul>
                        {
                            if props.is_logged_in {
                                html! {
                                    <div class="input-field">
                                        <label for="phone-number">{"Your Twilio Phone Number:"}</label>
                                        <div class="input-with-button">
                                            <input type="text" id="phone-number" placeholder="+1234567890" />
                                            <button class="save-button">{"Save"}</button>
                                        </div>
                                    </div>
                                }
                            } else {
                                html! {}
                            }
                        }
                    </div>
                    <div class="instruction-image">
                        <img 
                            src="/assets/number-twilio.png" 
                            alt="Buy Twilio Phone Number Image" 
                            loading="lazy"
                            onclick={let open_modal = open_modal.clone(); let src = "/assets/number-twilio.png".to_string(); 
                                Callback::from(move |_| open_modal.emit(src.clone()))}
                            style="cursor: pointer;"
                        />
                    </div>
                </div>

                <div class="instruction-block">
                    <div class="instruction-content">
                        <h2>{"Finding Credentials"}</h2>
                        <ul>
                            <li>{"1. Click on the 'Account Dashboard' in the left sidebar"}</li>
                            <li>{"2. Find and copy your 'Account SID' from the dashboard"}</li>
                            <li>{"3. Reveal and copy your 'Auth Token' from the dashboard"}</li>
                        </ul>
                        {
                            if props.is_logged_in {
                                html! {
                                    <>
                                        <div class="input-field">
                                            <label for="account-sid">{"Your Account SID:"}</label>
                                            <div class="input-with-button">
                                                <input type="text" id="account-sid" placeholder="ACxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx" />
                                            </div>
                                        </div>
                                        <div class="input-field">
                                            <label for="auth-token">{"Your Auth Token:"}</label>
                                            <div class="input-with-button">
                                                <input type="password" id="auth-token" placeholder="your_auth_token_here" />
                                            </div>
                                        </div>
                                        <button class="save-button">{"Save"}</button>
                                    </>
                                }
                            } else {
                                html! {}
                            }
                        }
                    </div>
                    <div class="instruction-image">
                        <img 
                            src="/assets/creds-twilio.png" 
                            alt="Twilio Credentials Dashboard" 
                            loading="lazy"
                            onclick={let open_modal = open_modal.clone(); let src = "/assets/creds-twilio.png".to_string(); 
                                Callback::from(move |_| open_modal.emit(src.clone()))}
                            style="cursor: pointer;"
                        />
                    </div>
                </div>
            </section>

            {
                if *modal_visible {
                    html! {
                        <div class="modal-overlay" onclick={close_modal.clone()}>
                            <div class="modal-content" onclick={Callback::from(|e: MouseEvent| e.stop_propagation())}>
                                <img src={(*selected_image).clone()} alt="Large preview" />
                                <button class="modal-close" onclick={close_modal}>{"×"}</button>
                            </div>
                        </div>
                    }
                } else {
                    html! {}
                }
            }

            <style>
                {r#"
                .instructions-page {
                    padding-top: 74px;
                    min-height: 100vh;
                    color: #ffffff;
                    position: relative;
                    background: transparent;
                }

                .instructions-background {
                    position: fixed;
                    top: 0;
                    left: 0;
                    width: 100%;
                    height: 100vh;
                    background-image: url('/assets/bicycle_field.webp');
                    background-size: cover;
                    background-position: center;
                    background-repeat: no-repeat;
                    opacity: 1;
                    z-index: -2;
                    pointer-events: none;
                }

                .instructions-background::after {
                    content: '';
                    position: absolute;
                    bottom: 0;
                    left: 0;
                    width: 100%;
                    height: 50%;
                    background: linear-gradient(
                        to bottom, 
                        rgba(26, 26, 26, 0) 0%,
                        rgba(26, 26, 26, 1) 100%
                    );
                }

                .instructions-hero {
                    text-align: center;
                    padding: 6rem 2rem;
                    background: rgba(26, 26, 26, 0.75);
                    backdrop-filter: blur(5px);
                    margin-top: 2rem;
                    border: 1px solid rgba(30, 144, 255, 0.1);
                    margin-bottom: 2rem;
                }

                .instructions-hero h1 {
                    font-size: 3.5rem;
                    margin-bottom: 1.5rem;
                    background: linear-gradient(45deg, #fff, #7EB2FF);
                    -webkit-background-clip: text;
                    -webkit-text-fill-color: transparent;
                }

                .instructions-hero p {
                    font-size: 1.2rem;
                    color: #999;
                    max-width: 600px;
                    margin: 0 auto;
                }

                .instructions-section {
                    max-width: 1200px;
                    margin: 0 auto;
                    padding: 2rem;
                }

                .instruction-block {
                    display: flex;
                    align-items: center;
                    gap: 4rem;
                    margin-bottom: 4rem;
                    background: rgba(26, 26, 26, 0.85);
                    backdrop-filter: blur(10px);
                    border: 1px solid rgba(30, 144, 255, 0.1);
                    border-radius: 12px;
                    padding: 4rem;
                    transition: all 0.3s ease;
                }

                .instruction-block:hover {
                    border-color: rgba(30, 144, 255, 0.3);
                }

                .instruction-content {
                    flex: 1;
                    order: 1;
                }

                .instruction-image {
                    flex: 1;
                    order: 2;
                }

                .instruction-content h2 {
                    font-size: 2rem;
                    margin-bottom: 1.5rem;
                    background: linear-gradient(45deg, #fff, #7EB2FF);
                    -webkit-background-clip: text;
                    -webkit-text-fill-color: transparent;
                }

                .instruction-content ul {
                    list-style: none;
                    padding: 0;
                }

                .instruction-content li {
                    color: #999;
                    padding: 0.75rem 0;
                    padding-left: 1.5rem;
                    position: relative;
                    line-height: 1.6;
                }

                .instruction-content li::before {
                    content: '•';
                    position: absolute;
                    left: 0.5rem;
                    color: #1E90FF;
                }

                .instruction-image {
                    flex: 1.2;  /* Increased from 1 to 1.2 to give more space for images */
                    display: flex;
                    justify-content: center;
                    align-items: center;
                }

                .instruction-image img {
                    max-width: 110%;  /* Increased from 100% to 120% */
                    height: auto;
                    border-radius: 12px;
                    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.3);
                    transition: transform 0.3s ease;
                }

                .instruction-image img:hover {
                    transform: scale(1.02);
                }

                @media (max-width: 968px) {
                    .instruction-block {
                        flex-direction: column;
                        gap: 2rem;
                    }

                    .instruction-content {
                        order: 1;
                    }

                    .instruction-image {
                        order: 2;
                    }

                    .instructions-hero h1 {
                        font-size: 2.5rem;
                    }

                    .instruction-content h2 {
                        font-size: 1.75rem;
                    }

                    .instructions-section {
                        padding: 1rem;
                    }
                }

                .input-field {
                    margin-top: 1.5rem;
                }

                .input-field label {
                    display: block;
                    margin-bottom: 0.5rem;
                    color: #7EB2FF;
                }

                .input-field input {
                    width: 100%;
                    padding: 0.75rem;
                    border: 1px solid rgba(30, 144, 255, 0.3);
                    border-radius: 6px;
                    background: rgba(26, 26, 26, 0.5);
                    color: #fff;
                    font-size: 1rem;
                    transition: all 0.3s ease;
                }

                .input-field input:focus {
                    outline: none;
                    border-color: rgba(30, 144, 255, 0.8);
                    box-shadow: 0 0 0 2px rgba(30, 144, 255, 0.2);
                }

                .input-field input::placeholder {
                    color: rgba(255, 255, 255, 0.3);
                }

                .input-with-button {
                    display: flex;
                    gap: 0.5rem;
                }

                .input-with-button input {
                    flex: 1;
                }

                .save-button {
                    padding: 0.75rem 1.5rem;
                    background: #1E90FF;
                    color: white;
                    border: none;
                    border-radius: 6px;
                    cursor: pointer;
                    font-size: 1rem;
                    transition: all 0.3s ease;
                }

                .save-button:hover {
                    background: #1976D2;
                }

                .save-button:active {
                    transform: translateY(1px);
                }

                .modal-overlay {
                    position: fixed;
                    top: 0;
                    left: 0;
                    width: 100%;
                    height: 100%;
                    background: rgba(0, 0, 0, 0.85);
                    display: flex;
                    justify-content: center;
                    align-items: center;
                    z-index: 1000;
                    backdrop-filter: blur(5px);
                }

                .modal-content {
                    position: relative;
                    max-width: 90%;
                    max-height: 90vh;
                    border-radius: 12px;
                    overflow: hidden;
                    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.5);
                }

                .modal-content img {
                    display: block;
                    max-width: 100%;
                    max-height: 90vh;
                    object-fit: contain;
                }

                .modal-close {
                    position: absolute;
                    top: 10px;
                    right: 10px;
                    width: 40px;
                    height: 40px;
                    border-radius: 50%;
                    background: rgba(0, 0, 0, 0.5);
                    border: 2px solid rgba(255, 255, 255, 0.5);
                    color: white;
                    font-size: 24px;
                    display: flex;
                    align-items: center;
                    justify-content: center;
                    cursor: pointer;
                    transition: all 0.3s ease;
                }

                .modal-close:hover {
                    background: rgba(0, 0, 0, 0.8);
                    border-color: white;
                }
                "#}
            </style>
        </div>
    }
}

