use yew::prelude::*;
use yew_router::components::Link;
use crate::Route;
use crate::utils::seo::{use_seo, SeoMeta};

#[function_component(HomeAssistantSms)]
pub fn home_assistant_sms() -> Html {
    use_seo(SeoMeta {
        title: "Home Assistant via SMS: Control Your Smart Home from Any Phone",
        description: "Control Home Assistant smart home devices via SMS using Lightfriend MCP integration. Setup guide, example commands, and automation tips.",
        canonical: "https://lightfriend.ai/blog/home-assistant-sms",
        og_type: "article",
    });
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
                <h1>{"Home Assistant via SMS: Control Your Smart Home from Any Phone"}</h1>
                <p>{"Turn lights on, adjust thermostats, and run automations with a text message."}</p>
            </section>
            <section class="blog-content">
                <h2>{"Smart Home, Dumb Phone?"}</h2>
                <p>{"Home Assistant is the most popular open-source smart home platform, managing everything from lights and thermostats to cameras, locks, and appliances. But its companion app requires a smartphone. If you have switched to a dumbphone, does that mean giving up your smart home? Not at all."}</p>

                <h2>{"How It Works: Lightfriend + MCP + Home Assistant"}</h2>
                <p>{"Lightfriend uses the Model Context Protocol (MCP) to connect directly to your Home Assistant instance. When you send a text message to Lightfriend, the AI interprets your intent and executes the appropriate Home Assistant commands. No app needed \u{2013} just SMS."}</p>
                <p>{"MCP is a standardized protocol that lets AI assistants interact with external tools and services securely. Lightfriend\u{2019}s MCP integration with Home Assistant means your AI understands your entire smart home setup."}</p>

                <h2>{"Example Commands"}</h2>
                <ul>
                    <li><strong>{"\"Turn off all lights\""}</strong>{" \u{2013} Switches off every light entity in Home Assistant."}</li>
                    <li><strong>{"\"Set living room to 22 degrees\""}</strong>{" \u{2013} Adjusts the thermostat in your living room."}</li>
                    <li><strong>{"\"Lock the front door\""}</strong>{" \u{2013} Engages your smart lock."}</li>
                    <li><strong>{"\"Is the garage door open?\""}</strong>{" \u{2013} Checks the status and reports back."}</li>
                    <li><strong>{"\"Turn on movie mode\""}</strong>{" \u{2013} Triggers a Home Assistant scene or automation."}</li>
                    <li><strong>{"\"Set bedroom lights to 30%\""}</strong>{" \u{2013} Dims specific lights."}</li>
                    <li><strong>{"\"What\u{2019}s the temperature inside?\""}</strong>{" \u{2013} Reads sensor data from your home."}</li>
                    <li><strong>{"\"Arm the alarm system\""}</strong>{" \u{2013} Activates your security system."}</li>
                </ul>
                <p>{"Commands are in natural language. You do not need to memorize exact syntax \u{2013} Lightfriend\u{2019}s AI figures out what you mean."}</p>

                <h2>{"Setup Guide"}</h2>
                <ol>
                    <li><strong>{"Expose Home Assistant externally"}</strong>{" using Nabu Casa, a reverse proxy, or a Cloudflare tunnel so Lightfriend can reach it."}</li>
                    <li><strong>{"Generate a long-lived access token"}</strong>{" in Home Assistant (Profile > Security > Long-Lived Access Tokens)."}</li>
                    <li><strong>{"Connect in Lightfriend"}</strong>{" by adding your Home Assistant URL and token in the Lightfriend dashboard under MCP connections."}</li>
                    <li><strong>{"Test it"}</strong>{" by texting a command like \"turn on the kitchen lights\" to your Lightfriend number."}</li>
                </ol>

                <h2>{"Why SMS for Smart Home?"}</h2>
                <p>{"SMS works everywhere \u{2013} no Wi-Fi needed, no app to load, no screen to navigate. Stuck in traffic and want to pre-heat the house? Text it. In bed and want to kill all the lights? Text it. The simplicity is the feature. Combined with Lightfriend\u{2019}s AI, you get a voice/text interface to your entire smart home that works from any phone on Earth."}</p>

                <div class="blog-cta">
                    <h3>{"Control Your Smart Home from Any Phone"}</h3>
                    <Link<Route> to={Route::Pricing} classes="forward-link">
                        <button class="hero-cta">{"Get Started with Lightfriend"}</button>
                    </Link<Route>>
                </div>
            </section>
            <style>
                {r#"
                .blog-page { padding-top: 74px; min-height: 100vh; color: #ffffff; position: relative; background: transparent; }
                .blog-background { position: fixed; top: 0; left: 0; width: 100%; height: 100vh; background-image: url('/assets/field_asthetic_not.webp'); background-size: cover; background-position: center; background-repeat: no-repeat; opacity: 1; z-index: -2; pointer-events: none; }
                .blog-background::after { content: ''; position: absolute; bottom: 0; left: 0; width: 100%; height: 50%; background: linear-gradient(to bottom, rgba(26, 26, 26, 0) 0%, rgba(26, 26, 26, 1) 100%); }
                .blog-hero { text-align: center; padding: 6rem 2rem; background: rgba(26, 26, 26, 0.75); backdrop-filter: blur(5px); margin-top: 2rem; border: 1px solid rgba(30, 144, 255, 0.1); margin-bottom: 2rem; }
                .blog-hero h1 { font-size: 3.5rem; margin-bottom: 1.5rem; background: linear-gradient(45deg, #fff, #7EB2FF); -webkit-background-clip: text; -webkit-text-fill-color: transparent; }
                .blog-hero p { font-size: 1.2rem; color: #999; max-width: 600px; margin: 0 auto; }
                .blog-content { max-width: 800px; margin: 0 auto; padding: 2rem; }
                .blog-content h2 { font-size: 2.5rem; margin: 3rem 0 1rem; background: linear-gradient(45deg, #fff, #7EB2FF); -webkit-background-clip: text; -webkit-text-fill-color: transparent; }
                .blog-content p { color: #999; line-height: 1.6; margin-bottom: 1.5rem; }
                .blog-content ul, .blog-content ol { color: #999; padding-left: 1.5rem; margin-bottom: 1.5rem; }
                .blog-content li { margin-bottom: 0.75rem; }
                .blog-content a { color: #7EB2FF; text-decoration: none; border-bottom: 1px solid rgba(126, 178, 255, 0.3); transition: all 0.3s ease; font-weight: 500; }
                .blog-content a:hover { color: #ffffff; border-bottom-color: #7EB2FF; text-shadow: 0 0 5px rgba(126, 178, 255, 0.5); }
                .blog-cta { text-align: center; margin: 4rem 0 2rem; padding: 2rem; background: rgba(30, 144, 255, 0.1); border-radius: 12px; }
                .blog-cta h3 { font-size: 2rem; margin-bottom: 1.5rem; background: linear-gradient(45deg, #fff, #7EB2FF); -webkit-background-clip: text; -webkit-text-fill-color: transparent; }
                .hero-cta { background: linear-gradient(45deg, #7EB2FF, #4169E1); color: white; border: none; padding: 1rem 2.5rem; border-radius: 8px; font-size: 1.1rem; cursor: pointer; transition: all 0.3s ease; }
                .hero-cta:hover { transform: translateY(-2px); box-shadow: 0 4px 20px rgba(126, 178, 255, 0.4); }
                @media (max-width: 768px) { .blog-hero { padding: 4rem 1rem; } .blog-hero h1 { font-size: 2.5rem; } .blog-content { padding: 1rem; } .blog-content h2 { font-size: 2rem; } }
                "#}
            </style>
        </div>
    }
}
