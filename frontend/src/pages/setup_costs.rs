use yew::prelude::*;
use web_sys::Event;

#[function_component(SetupCosts)]
pub fn setup_costs() -> Html {
    let selected_country = use_state(|| "en".to_string());

    let on_country_change = {
        let selected_country = selected_country.clone();
        Callback::from(move |e: Event| {
            let input: web_sys::HtmlSelectElement = e.target_unchecked_into();
            selected_country.set(input.value());
        })
    };

    let country = (*selected_country).clone();

    // Define data based on country
    let openrouter_cost = "~1$/month".to_string();
    let openrouter_time = "~5 minutes".to_string();
    let openrouter_effort = "Low".to_string();
    let openrouter_required = "Yes".to_string();

    let server_cost = "5-8$/month".to_string();
    let server_time = "~10-20 minutes".to_string();
    let server_effort = "Medium".to_string();
    let server_required = "Yes".to_string();

    let twilio_voice_cost = match country.as_str() {
        "en" => "~2$/month (1$ number + 1$ calls for 3mins/day)".to_string(),
        "fi" => "~8.6$/month (5$ number + 3.6$ calls for 3mins/day)".to_string(),
        "gb" => "~3.5$/month (1$ number + 2.5$ calls for 3mins/day)".to_string(),
        "au" => "~9$/month (6.5$ number + 2.5$ calls for 3mins/day)".to_string(),
        "se" => "~6.0$/month (3.0$ number + 3.0$ calls for 3mins/day)".to_string(),
        "dk" => "~17.5$/month (15.0$ number + 2.5$ calls for 3mins/day)".to_string(),
        "de" => "~2.6$/month (1$ number + 1.6$ calls for 3mins/day). Requires registered German business address.".to_string(),
        _ => "~3.7$/month (2.3$ number + 1.4$ calls for 3mins/day)".to_string(),
    };
    let twilio_voice_link = match country.as_str() {
        "en" => "https://www.twilio.com/en-us/voice/pricing/us".to_string(),
        "fi" => "https://www.twilio.com/en-us/voice/pricing/fi".to_string(),
        "gb" => "https://www.twilio.com/en-us/voice/pricing/gb".to_string(),
        "au" => "https://www.twilio.com/en-us/voice/pricing/au".to_string(),
        "se" => "https://www.twilio.com/en-us/voice/pricing/se".to_string(),
        "dk" => "https://www.twilio.com/en-us/voice/pricing/dk".to_string(),
        "de" => "https://www.twilio.com/en-us/voice/pricing/de".to_string(),
        _ => "https://www.twilio.com/en-us/voice/pricing/en".to_string(),
    };
    let twilio_voice_time = "5-10 minutes".to_string();
    let twilio_voice_effort = match country.as_str() {
        "de" => "High".to_string(),
        "gb" | "au" | "se" | "dk" => "Medium".to_string(),
        _ => "Easy".to_string(),
    };
    let twilio_voice_required = "For voice features".to_string();

    let elevenlabs_cost = "15 mins free/month (50 mins for 5$/month)".to_string();
    let elevenlabs_link = "https://elevenlabs.io/pricing".to_string();
    let elevenlabs_time = "5-10 minutes".to_string();
    let elevenlabs_effort = "Low".to_string();
    let elevenlabs_required = "For voice calls".to_string();

    let twilio_msg_cost = match country.as_str() {
        "en" => "4$ one time A2P registration + ~4$/month (2$ A2P campaign + 2$ for 100 messages)".to_string(),
        "fi" => "normal 5$-25$/month (for 20-100 messages)".to_string(),
        "se" => "normal 4$-20$/month (for 20-100 messages)".to_string(),
        "de" => "Hard to setup: 15$/month number with registered business and 120 day validation time. Messages cost ~2x UK rates. Easier to use a UK number.".to_string(),
        _ => "3.5$-17.5$/month (for 20-100 messages)".to_string(),
    };
    let server_cost_link = "https://www.hostinger.com/pricing?content=vps-hosting".to_string();
    let twilio_msg_link = match country.as_str() {
        "en" => "https://www.twilio.com/en-us/sms/pricing/us".to_string(),
        "fi" => "https://www.twilio.com/en-us/sms/pricing/fi".to_string(),
        "gb" => "https://www.twilio.com/en-us/sms/pricing/gb".to_string(),
        "au" => "https://www.twilio.com/en-us/sms/pricing/au".to_string(),
        "se" => "https://www.twilio.com/en-us/sms/pricing/se".to_string(),
        "dk" => "https://www.twilio.com/en-us/sms/pricing/dk".to_string(),
        "de" => "https://www.twilio.com/en-us/sms/pricing/de".to_string(),
         _ => "https://www.twilio.com/en-us/sms/pricing/en".to_string(),
    };
    let twilio_msg_time = match country.as_str() {
        "en" => "1-3 weeks (depending on Twilio approval)".to_string(),
        "de" => "120 days".to_string(),
        _ => "None extra (from voice setup)".to_string(),
    };
    let twilio_msg_effort = match country.as_str() {
        "en" => "Medium-High".to_string(),
        "de" => "High".to_string(),
        _ => "None".to_string(),
    };
    let twilio_msg_required = "For messaging".to_string();

    html! {
        <div class="instructions-page">
            <div class="instructions-background"></div>
            <section class="instructions-section">
                <div class="instruction-block overview-block">
                    <div class="instruction-content">
                        <h2>{"Setup Costs, Times, and Requirements"}</h2>
                        <p>{"Select a country to view the estimated costs and setup details for self-hosting and integrations. Setups are ordered from minimal requirements at the top to additional features below."}</p>
                        <div class="country-selector">
                            <label for="country-select">{"Country: "}</label>
                            <select id="country-select" onchange={on_country_change}>
                                <option value="en" selected={country == "en"}>{"EN"}</option>
                                <option value="fi" selected={country == "fi"}>{"FI"}</option>
                                <option value="gb" selected={country == "gb"}>{"GB"}</option>
                                <option value="au" selected={country == "au"}>{"AU"}</option>
                                <option value="se" selected={country == "se"}>{"SE"}</option>
                                <option value="dk" selected={country == "dk"}>{"DK"}</option>
                                <option value="de" selected={country == "de"}>{"DE"}</option>
                            </select>
                        </div>
                    </div>
                </div>

                <div class="instruction-block">
                    <div class="instruction-content">
                        <table class="setup-table">
                            <thead>
                                <tr>
                                    <th>{"Service"}</th>
                                    <th>{"Approx Cost"}</th>
                                    <th>{"Setup Time"}</th>
                                    <th>{"Effort"}</th>
                                    <th>{"Required"}</th>
                                </tr>
                            </thead>
                            <tbody>
                                <tr>
                                    <td>{"Server Self-Hosting"}</td>
                                    <td><a href={server_cost_link} target="_blank" class="cost-link">{server_cost}</a></td>
                                    <td>{server_time}</td>
                                    <td>{server_effort}</td>
                                    <td>{server_required}</td>
                                </tr>
                                <tr>
                                    <td>{"AI Provider (OpenRouter)"}</td>
                                    <td>{openrouter_cost}</td>
                                    <td>{openrouter_time}</td>
                                    <td>{openrouter_effort}</td>
                                    <td>{openrouter_required}</td>
                                </tr>
                                <tr>
                                    <td>{"Twilio for Voice Calling"}</td>
                                    <td><a href={twilio_voice_link} target="_blank" class="cost-link">{twilio_voice_cost}</a></td>
                                    <td>{twilio_voice_time}</td>
                                    <td>{twilio_voice_effort}</td>
                                    <td>{twilio_voice_required}</td>
                                </tr>
                                <tr>
                                    <td>{"ElevenLabs (for Voice Calls)"}</td>
                                    <td><a href={elevenlabs_link} target="_blank" class="cost-link">{elevenlabs_cost}</a></td>
                                    <td>{elevenlabs_time}</td>
                                    <td>{elevenlabs_effort}</td>
                                    <td>{elevenlabs_required}</td>
                                </tr>
                                <tr>
                                    <td>{"Twilio for Messaging"}</td>
                                    <td><a href={twilio_msg_link} target="_blank" class="cost-link">{twilio_msg_cost}</a></td>
                                    <td>{twilio_msg_time}</td>
                                    <td>{twilio_msg_effort}</td>
                                    <td>{twilio_msg_required}</td>
                                </tr>
                            </tbody>
                        </table>
                    </div>
                </div>
            </section>

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
                }

                .instruction-content h2 {
                    font-size: 2rem;
                    margin-bottom: 1.5rem;
                    background: linear-gradient(45deg, #fff, #7EB2FF);
                    -webkit-background-clip: text;
                    -webkit-text-fill-color: transparent;
                }

                .instruction-content p {
                    color: #999;
                    line-height: 1.6;
                    margin-bottom: 1.5rem;
                }

                .country-selector {
                    display: flex;
                    align-items: center;
                    gap: 1rem;
                    margin-bottom: 2rem;
                }

                .country-selector label {
                    color: #7EB2FF;
                    font-size: 1.1rem;
                }

                .country-selector select {
                    padding: 0.75rem;
                    background: rgba(26, 26, 26, 0.5);
                    border: 1px solid rgba(30, 144, 255, 0.3);
                    color: #fff;
                    border-radius: 6px;
                    font-size: 1rem;
                    cursor: pointer;
                    transition: all 0.3s ease;
                }

                .country-selector select:focus {
                    outline: none;
                    border-color: rgba(30, 144, 255, 0.8);
                }

                .setup-table {
                    width: 100%;
                    border-collapse: separate;
                    border-spacing: 0;
                    border-radius: 8px;
                    overflow: hidden;
                }

                .setup-table th,
                .setup-table td {
                    padding: 1rem 1.5rem;
                    text-align: left;
                    border-bottom: 1px solid rgba(255, 255, 255, 0.1);
                    color: #999;
                }

                .setup-table th {
                    background: rgba(30, 144, 255, 0.1);
                    color: #fff;
                    font-weight: normal;
                }

                .setup-table tr:last-child td {
                    border-bottom: none;
                }

                .setup-table td:first-child {
                    color: #fff;
                }

                .cost-link {
                    color: #1E90FF;
                    text-decoration: none;
                }

                .cost-link:hover {
                    text-decoration: underline;
                }

                @media (max-width: 968px) {
                    .instruction-block {
                        flex-direction: column;
                        gap: 2rem;
                        padding: 2rem;
                    }

                    .setup-table th,
                    .setup-table td {
                        padding: 0.75rem 1rem;
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
