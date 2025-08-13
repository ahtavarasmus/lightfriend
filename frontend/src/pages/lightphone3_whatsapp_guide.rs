use yew::prelude::*;
use yew_router::components::Link;
use crate::Route;

#[function_component(LightPhone3WhatsappGuide)]
pub fn light_phone_3_whatsapp_guide() -> Html {
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
                <h1>{"Light Phone 3 WhatsApp Guide: Enhance Your Minimalist Phone with Lightfriend"}</h1>
                <p>{"Discover how to add WhatsApp functionality to your Light Phone 3 without compromising its minimalist design. Stay connected via SMS and voice while maintaining digital detox benefits."}</p>
            </section>
            <section class="blog-content">
                <h2>{"Introduction: Why Add WhatsApp to Light Phone 3?"}</h2>
                <p>{"The Light Phone 3 is a premium minimalist phone designed for digital detox, but many users miss essential messaging apps like WhatsApp. With Lightfriend's AI assistant, you can access WhatsApp on your Light Phone 3 via simple SMS or voice calls - no apps required."}</p>
                <img src="/assets/light-phone-3-whatsapp-integration.webp" alt="Light Phone 3 with WhatsApp via Lightfriend AI" loading="lazy" class="blog-image" />

                <h2>{"How Lightfriend Works with Light Phone 3"}</h2>
                <p>{"Lightfriend acts as your smart companion, bridging the gap between your minimalist phone and digital services:"}</p>
                <ul>
                    <li>{"Send and receive WhatsApp messages via SMS/voice through a single phone number"}</li>
                    <li>{"Get notifications for critical messages and important chats"}</li>
                    <li>{"Monitor contacts and respond hands-free"}</li>
                    <li>{"Setup temporary notifications for certain one time events you are waiting for"}</li>
                    <li>{"Setup scheduled daily digests that tell you about messages you might have missed"}</li>
                </ul>

                <h2>{"Benefits for Light Phone 3 Users"}</h2>
                <p>{"Enhance your digital minimalism without sacrifices:"}</p>
                <ul>
                    <li>{"Maintain Light Phone's distraction-free experience"}</li>
                    <li>{"Access WhatsApp without making a phone with app store"}</li>
                    <li>{"AI-powered monitoring for urgent messages only"}</li>
                    <li>{"Seamless integration with Light Phone's voice and text capabilities"}</li>
                </ul>

                <h2>{"Step-by-Step Setup Guide"}</h2>
                <ol>
                    <li>{"Sign up for Lightfriend and connect your WhatsApp account via the web dashboard"}</li>
                    <li>{"Set up priority contacts and notification preferences"}</li>
                    <li>{"Add Lightfriend's number to your Light Phone 3 contacts"}</li>
                    <li>{"Test by sending 'Check WhatsApp' via SMS or voice call"}</li>
                    <li>{"Customize AI monitoring for your needs"}</li>
                </ol>

                <h2>{"Comparison: Light Phone 3 With vs Without Lightfriend"}</h2>
                <table class="comparison-table">
                    <thead>
                        <tr>
                            <th>{"Feature"}</th>
                            <th>{"Light Phone 3 Alone"}</th>
                            <th>{"With Lightfriend"}</th>
                        </tr>
                    </thead>
                    <tbody>
                        <tr>
                            <td>{"WhatsApp Access"}</td>
                            <td>{"No"}</td>
                            <td>{"Full send/receive via SMS/Voice"}</td>
                        </tr>
                        <tr>
                            <td>{"Message Notifications"}</td>
                            <td>{"None"}</td>
                            <td>{"AI-filtered important alerts"}</td>
                        </tr>
                        <tr>
                            <td>{"Chat Monitoring"}</td>
                            <td>{"No"}</td>
                            <td>{"Yes, with summaries"}</td>
                        </tr>
                        <tr>
                            <td>{"Battery Impact"}</td>
                            <td>{"Minimal"}</td>
                            <td>{"No additional drain"}</td>
                        </tr>
                        <tr>
                            <td>{"Minimalist Integrity"}</td>
                            <td>{"High"}</td>
                            <td>{"Maintained - no apps added"}</td>
                        </tr>
                    </tbody>
                </table>

                <h2>{"Common Questions"}</h2>
                <p>{"Q: Does this work with Light Phone 3's international versions? A: Yes, as long as SMS/voice is available."}</p>
                <p>{"Q: How private is my WhatsApp data? A: Your data’s safe with Lightfriend. We run on a secure EU server with no logging of your chats, searches, or personal info. All credentials are encrypted, and optional conversation history gets deleted automatically as you go—my server would fill up fast otherwise. Messaging app chats (like WhatsApp) are temporary too: they’re only accessible for 2 days after receiving them, then gone. I’m a solo dev, not some data-hungry corp. The code’s open-source on GitHub, so anyone can check it’s legit. It’s a hosted app, so some trust is needed, but you own your data and can delete it anytime, no questions asked."}</p>

                <div class="blog-cta">
                    <h3>{"Ready to Add WhatsApp to Your Light Phone 3?"}</h3>
                    <Link<Route> to={Route::Pricing} classes="forward-link">
                        <button class="hero-cta">{"Get Started with Lightfriend"}</button>
                    </Link<Route>>
                    <p>{"Join 100+ users enhancing their minimalist phones today!"}</p>
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
                    background: rgba(26, 26, 26, 0.75);
                    backdrop-filter: blur(5px);
                    margin-top: 2rem;
                    border: 1px solid rgba(30, 144, 255, 0.1);
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
                    color: #999;
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
                    color: #999;
                    line-height: 1.6;
                    margin-bottom: 1.5rem;
                }
                .blog-content ul, .blog-content ol {
                    color: #999;
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
                    border: 1px solid rgba(126, 178, 255, 0.2);
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
                    background: rgba(30, 144, 255, 0.1);
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
