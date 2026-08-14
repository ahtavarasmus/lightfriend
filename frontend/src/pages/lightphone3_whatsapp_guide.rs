use crate::utils::seo::{use_seo, SeoMeta};
use crate::Route;
use yew::prelude::*;
use yew_router::components::Link;

#[function_component(LightPhone3WhatsappGuide)]
pub fn light_phone_3_whatsapp_guide() -> Html {
    use_seo(SeoMeta {
        title: "Can You Use WhatsApp on Light Phone III? Options Compared",
        description: "Light Phone III does not currently include a native WhatsApp tool. Compare the practical alternatives, including desktop access and Lightfriend's text-first SMS bridge.",
        canonical: "https://lightfriend.ai/light-phone-3-whatsapp-guide",
        og_type: "article",
    });
    // Scroll to top only on initial mount
    {
        use_effect_with_deps(
            move |_| {
                if let Some(window) = web_sys::window() {
                    window.scroll_to_with_x_and_y(0.0, 0.0);
                }
                || ()
            },
            (),
        );
    }

    html! {
        <div class="blog-page">
            <div class="blog-background"></div>
            <section class="blog-hero">
                <h1>{"Can You Use WhatsApp on Light Phone III?"}</h1>
                <p>{"There is no native WhatsApp tool on Light Phone III today. Here are the practical ways to stay reachable without pretending they reproduce the full app."}</p>
            </section>
            <section class="blog-content">
                <p><strong>{"Short answer: Light Phone III does not currently run a native WhatsApp tool."}</strong>{" Lightfriend is a separate service that can give the phone selected, text-first access to a connected WhatsApp account through ordinary SMS and voice calls. It does not install WhatsApp on LightOS or reproduce every WhatsApp feature."}</p>
                <img src="/assets/light-phone-3-whatsapp-integration.webp" alt="Light Phone 3 with WhatsApp via Lightfriend AI" loading="lazy" class="blog-image" />

                <h2>{"What Light Phone officially supports"}</h2>
                <p>{"Light's current "}<a href="https://support.thelightphone.com/hc/en-us/articles/360031128671-Tool-Availability-Status" target="_blank" rel="noopener noreferrer">{"tool availability page"}</a>{" lists tools such as Directions, Calendar, Notes, Weather, and Authenticator for Light Phone III, but not WhatsApp. Light says third-party messaging tools would require deeper collaboration with those platforms and does not provide a timeline."}</p>

                <h2>{"Your three practical options"}</h2>
                <table class="comparison-table">
                    <thead>
                        <tr>
                            <th>{"Option"}</th>
                            <th>{"Works while carrying only Light Phone III"}</th>
                            <th>{"Main tradeoff"}</th>
                        </tr>
                    </thead>
                    <tbody>
                        <tr>
                            <td>{"Calls and SMS only"}</td>
                            <td>{"Yes"}</td>
                            <td>{"Your contacts must change how they reach you"}</td>
                        </tr>
                        <tr>
                            <td>{"WhatsApp Web/Desktop"}</td>
                            <td>{"No"}</td>
                            <td>{"The complete interface is available only at a computer"}</td>
                        </tr>
                        <tr>
                            <td>{"Lightfriend over SMS"}</td>
                            <td>{"Yes, for selected text access"}</td>
                            <td>{"Not a full clone of the WhatsApp app"}</td>
                        </tr>
                    </tbody>
                </table>

                <h2>{"How Lightfriend works with Light Phone III"}</h2>
                <p>{"Lightfriend connects a supported WhatsApp account to a smaller conversational interface. From Light Phone III you can text or call Lightfriend to ask about recent messages, search for a sender or topic, reply through the connected account, and configure focused alerts or digests."}</p>
                <ul>
                    <li>{"Ask what a person or group recently said"}</li>
                    <li>{"Send a text reply through the connected account"}</li>
                    <li>{"Receive focused alerts from important contacts"}</li>
                    <li>{"Create a temporary alert for an event you are waiting for"}</li>
                    <li>{"Receive scheduled summaries instead of a live notification feed"}</li>
                </ul>

                <h2>{"Setup: what you still need"}</h2>
                <ol>
                    <li>{"Keep a supported device with the native WhatsApp app available for account setup and recovery"}</li>
                    <li>{"Sign up for Lightfriend and pair the WhatsApp account from the web dashboard"}</li>
                    <li>{"Add Lightfriend's number to your Light Phone 3 contacts"}</li>
                    <li>{"Test one direct message, one group conversation, and one reply"}</li>
                    <li>{"Configure which contacts may interrupt you and when routine messages should be summarized"}</li>
                </ol>
                <p>{"You do not need to carry the setup device every day. Keep it secured, updated, and available because linked-device sessions can require periodic native-app activity or re-pairing. Lightfriend can send a reminder before the expected inactivity window, but it cannot override WhatsApp's policies."}</p>

                <h2>{"What works and what does not"}</h2>
                <table class="comparison-table">
                    <thead>
                        <tr>
                            <th>{"Feature"}</th>
                            <th>{"Lightfriend on Light Phone III"}</th>
                        </tr>
                    </thead>
                    <tbody>
                        <tr>
                            <td>{"Retrieve and reply to text messages"}</td>
                            <td>{"Yes"}</td>
                        </tr>
                        <tr>
                            <td>{"Focused alerts and summaries"}</td>
                            <td>{"Yes"}</td>
                        </tr>
                        <tr>
                            <td>{"Group-chat context"}</td>
                            <td>{"Text-first access; not the full group interface"}</td>
                        </tr>
                        <tr>
                            <td>{"WhatsApp voice or video calls"}</td>
                            <td>{"No"}</td>
                        </tr>
                        <tr>
                            <td>{"Status, stickers, reactions, and live location"}</td>
                            <td>{"No native app experience"}</td>
                        </tr>
                    </tbody>
                </table>

                <h2>{"Privacy and reliability tradeoffs"}</h2>
                <p>{"Lightfriend's production application runs in a hardware-isolated enclave. Stored application data is encrypted, the codebase is open source, and the running enclave exposes a signed code measurement that can be checked against the published build."}</p>
                <p>{"The final SMS leg still passes through your cellular carrier and is not end-to-end encrypted. Delivery also depends on WhatsApp, the bridge connection, Lightfriend, and your carrier. Use a direct call or SMS fallback for emergencies."}</p>

                <h2>{"Continue planning your setup"}</h2>
                <ul>
                    <li><a href="/can-i-leave-my-smartphone" data-fast-goal="exit_planner_click">{"Build a personalized smartphone-exit plan"}</a></li>
                    <li><a href="/whatsapp-on-dumbphone">{"Compare WhatsApp options for any dumbphone or flip phone"}</a></li>
                    <li><a href="/blog/best-dumbphone-whatsapp-setup-2026">{"Compare four dumbphone WhatsApp setups"}</a></li>
                    <li><a href="/how-to-switch-to-dumbphone">{"Use the complete dumbphone switching checklist"}</a></li>
                    <li><a href="/blog/digital-detox-with-whatsapp">{"Plan a digital detox with urgent-message fallbacks"}</a></li>
                    <li><Link<Route> to={Route::TelegramOnDumbphone}>{"Telegram on dumbphone"}</Link<Route>></li>
                    <li><Link<Route> to={Route::SignalOnDumbphone}>{"Signal on dumbphone"}</Link<Route>></li>
                </ul>

                <div class="blog-cta">
                    <h3>{"Can you leave your smartphone without losing the people who use WhatsApp?"}</h3>
                    <a href="/can-i-leave-my-smartphone" class="forward-link" data-fast-goal="exit_planner_click">
                        <button class="hero-cta">{"Build My Exit Plan"}</button>
                    </a>
                    <p>{"Text-first access for phones that can call and send SMS. Initial connected-service setup still uses the provider's supported device."}</p>
                    <a href="/#plans" data-fast-goal="blog_pricing_click" data-fast-goal-content-slug="light-phone-3-whatsapp-guide" data-fast-goal-content-cluster="messaging">{"See plans and start a trial"}</a>
                </div>
            </section>
            <style>
                {r#"
                .blog-page {
                    padding-top: 74px;
                    min-height: 100vh;
                    color: #ffffff;
                    position: relative;
                    background: transparent;
                }
                .blog-background {
                    position: fixed;
                    top: 0;
                    left: 0;
                    width: 100%;
                    height: 100vh;
                    background-image: url('/assets/field_asthetic_not.webp');
                    background-size: cover;
                    background-position: center;
                    background-repeat: no-repeat;
                    opacity: 1;
                    z-index: -2;
                    pointer-events: none;
                }
                .blog-background::after {
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
                .blog-hero {
                    text-align: center;
                    padding: 6rem 2rem;
                    background: var(--surface-card);
                    margin-top: 2rem;
                    border: 1px solid var(--border-card);
                    margin-bottom: 2rem;
                }
                .blog-hero h1 {
                    font-size: 3.5rem;
                    margin-bottom: 1.5rem;
                    background: linear-gradient(45deg, #fff, #7EB2FF);
                    -webkit-background-clip: text;
                    -webkit-text-fill-color: transparent;
                }
                .blog-hero p {
                    font-size: 1.2rem;
                    color: var(--text-body);
                    max-width: 600px;
                    margin: 0 auto;
                }
                .blog-content {
                    max-width: 800px;
                    margin: 0 auto;
                    padding: 2rem;
                }
                .blog-content h2 {
                    font-size: 2.5rem;
                    margin: 3rem 0 1rem;
                    background: linear-gradient(45deg, #fff, #7EB2FF);
                    -webkit-background-clip: text;
                    -webkit-text-fill-color: transparent;
                }
                .blog-content p {
                    color: var(--text-body);
                    line-height: 1.6;
                    margin-bottom: 1.5rem;
                }
                .blog-content ul, .blog-content ol {
                    color: var(--text-body);
                    padding-left: 1.5rem;
                    margin-bottom: 1.5rem;
                }
                .blog-content li {
                    margin-bottom: 0.75rem;
                }
                .blog-image {
                    max-width: 100%;
                    height: auto;
                    border-radius: 12px;
                    margin: 2rem 0;
                    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.3);
                }
                .comparison-table {
                    width: 100%;
                    border-collapse: collapse;
                    margin: 2rem 0;
                    color: #ddd;
                }
                .comparison-table th, .comparison-table td {
                    padding: 1rem;
                    border: 1px solid var(--border-card);
                    text-align: left;
                }
                .comparison-table th {
                    background: rgba(0, 0, 0, 0.5);
                    color: #7EB2FF;
                }
                .blog-cta {
                    text-align: center;
                    margin: 4rem 0 2rem;
                    padding: 2rem;
                    background: var(--surface-subtle);
                    border: 1px solid var(--border-card);
                    border-radius: 12px;
                }
                .blog-cta h3 {
                    font-size: 2rem;
                    margin-bottom: 1.5rem;
                    background: linear-gradient(45deg, #fff, #7EB2FF);
                    -webkit-background-clip: text;
                    -webkit-text-fill-color: transparent;
                }
                .blog-cta p {
                    color: #999;
                    margin-top: 1rem;
                }
                .hero-cta {
                    background: linear-gradient(45deg, #7EB2FF, #4169E1);
                    color: white;
                    border: none;
                    padding: 1rem 2.5rem;
                    border-radius: 8px;
                    font-size: 1.1rem;
                    cursor: pointer;
                    transition: all 0.3s ease;
                }
                .hero-cta:hover {
                    transform: translateY(-2px);
                    box-shadow: 0 4px 20px rgba(126, 178, 255, 0.4);
                }
                @media (max-width: 768px) {
                    .blog-hero {
                        padding: 4rem 1rem;
                    }
                    .blog-hero h1 {
                        font-size: 2.5rem;
                    }
                    .blog-content {
                        padding: 1rem;
                    }
                    .blog-content h2 {
                        font-size: 2rem;
                    }
                    .comparison-table th, .comparison-table td {
                        padding: 0.75rem;
                        font-size: 0.9rem;
                    }
                }
                "#}
            </style>
        </div>
    }
}
