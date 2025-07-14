use yew::prelude::*;
use web_sys::MouseEvent;

#[derive(Properties, PartialEq)]
pub struct ServerSelfHostInstructionsProps {
    #[prop_or_default]
    pub is_logged_in: bool,
    #[prop_or_default]
    pub sub_tier: Option<String>,
}

#[function_component(ServerSelfHostInstructions)]
pub fn server_self_host_instructions(props: &ServerSelfHostInstructionsProps) -> Html {
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
            <section class="instructions-section">
                <div class="instruction-block overview-block">
                    <div class="instruction-content">
                        <h2>{"Setting Up Your Server with DigitalOcean and Cloudron"}</h2>
                        <p>{"Follow these steps to set up your own server for Lightfriend. We'll guide you through purchasing a DigitalOcean droplet with Cloudron pre-installed."}</p>
                    </div>
                </div>

                <div class="instruction-block">
                    <div class="instruction-content">
                        <h2>{"Step 1: Create a DigitalOcean Account"}</h2>
                        <ul>
                            <li>{"Go to DigitalOcean's website (digitalocean.com)"}</li>
                            <li>{"Click 'Sign Up' and create an account"}</li>
                            <li>{"Add a payment method to your account"}</li>
                        </ul>
                    </div>
                    <div class="instruction-image">
                        <img 
                            src="/assets/do-signup.png" 
                            alt="DigitalOcean Signup Page" 
                            loading="lazy"
                            onclick={let open_modal = open_modal.clone(); let src = "/assets/do-signup.png".to_string(); 
                                Callback::from(move |_| open_modal.emit(src.clone()))}
                            style="cursor: pointer;"
                        />
                    </div>
                </div>

                <div class="instruction-block">
                    <div class="instruction-content">
                        <h2>{"Step 2: Create a Droplet with Cloudron"}</h2>
                        <ul>
                            <li>{"1. From your DigitalOcean dashboard, click 'Create' → 'Droplets'"}</li>
                            <li>{"2. Under 'Choose an image', select the 'Marketplace' tab"}</li>
                            <li>{"3. Search for 'Cloudron' and select it"}</li>
                            <li>{"4. Choose a plan: Basic plan with 2GB RAM / 1 CPU ($12/month) is sufficient"}</li>
                            <li>{"5. Select a datacenter region closest to you"}</li>
                            <li>{"6. Add an SSH key or choose a password"}</li>
                            <li>{"7. Click 'Create Droplet'"}</li>
                        </ul>
                    </div>
                    <div class="instruction-image">
                        <img 
                            src="/assets/do-droplet.png" 
                            alt="DigitalOcean Droplet Creation" 
                            loading="lazy"
                            onclick={let open_modal = open_modal.clone(); let src = "/assets/do-droplet.png".to_string(); 
                                Callback::from(move |_| open_modal.emit(src.clone()))}
                            style="cursor: pointer;"
                        />
                    </div>
                </div>

                <div class="instruction-block">
                    <div class="instruction-content">
                        <h2>{"Step 3: Configure Your Server"}</h2>
                        <ul>
                            <li>{"1. Note down your server's IP address from the DigitalOcean dashboard"}</li>
                            <li>{"2. Wait about 5-10 minutes for Cloudron to finish installing"}</li>
                            <li>{"3. Visit your server's IP address in a browser (https://your-ip-address)"}</li>
                            <li>{"4. Follow Cloudron's initial setup wizard"}</li>
                        </ul>
                        {
                            if props.is_logged_in && props.sub_tier.as_deref() == Some("tier 3") {
                                html! {
                                    <div class="input-field">
                                        <label for="server-ip">{"Your Server's IP Address:"}</label>
                                        <div class="input-with-button">
                                            <input type="text" id="server-ip" placeholder="Enter your server's IP address" />
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
                            src="/assets/cloudron-setup.png" 
                            alt="Cloudron Setup Page" 
                            loading="lazy"
                            onclick={let open_modal = open_modal.clone(); let src = "/assets/cloudron-setup.png".to_string(); 
                                Callback::from(move |_| open_modal.emit(src.clone()))}
                            style="cursor: pointer;"
                        />
                    </div>
                </div>

                <div class="instruction-block">
                    <div class="instruction-content">
                        <h2>{"Expected Costs"}</h2>
                        <ul>
                            <li>{"DigitalOcean Droplet: $12/month (2GB RAM / 1 CPU)"}</li>
                            <li>{"Cloudron License: Free for 2 apps"}</li>
                            <li>{"Total: ~$12/month"}</li>
                        </ul>
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
                    flex: 1.2;
                    display: flex;
                    justify-content: center;
                    align-items: center;
                }

                .instruction-image img {
                    max-width: 110%;
                    height: auto;
                    border-radius: 12px;
                    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.3);
                    transition: transform 0.3s ease;
                }

                .instruction-image img:hover {
                    transform: scale(1.02);
                }

                .input-field {
                    margin-top: 1.5rem;
                }

                .input-field label {
                    display: block;
                    margin-bottom: 0.5rem;
                    color: #7EB2FF;
                }

                .input-with-button {
                    display: flex;
                    gap: 0.5rem;
                }

                .input-with-button input {
                    flex: 1;
                    padding: 0.75rem;
                    border: 1px solid rgba(30, 144, 255, 0.3);
                    border-radius: 6px;
                    background: rgba(26, 26, 26, 0.5);
                    color: #fff;
                    font-size: 1rem;
                    transition: all 0.3s ease;
                }

                .input-with-button input:focus {
                    outline: none;
                    border-color: rgba(30, 144, 255, 0.8);
                    box-shadow: 0 0 0 2px rgba(30, 144, 255, 0.2);
                }

                .input-with-button input::placeholder {
                    color: rgba(255, 255, 255, 0.3);
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

                    .instruction-content h2 {
                        font-size: 1.75rem;
                    }

                    .instructions-section {
                        padding: 1rem;
                    }
                }
                "#}
            </style>
        </div>
    }
}

