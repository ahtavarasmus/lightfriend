use yew::prelude::*;
use yew_router::components::Link;
use crate::Route;
use crate::utils::seo::{use_seo, SeoMeta};

#[function_component(FeaturesIndex)]
pub fn features_index() -> Html {
    use_seo(SeoMeta {
        title: "Features \u{2013} Lightfriend AI Assistant for Dumbphones",
        description: "All Lightfriend features: WhatsApp, Telegram, Signal, email, calendar, Tesla control, AI search, GPS, voice AI, smart home, QR scanning, and digital wellness.",
        canonical: "https://lightfriend.ai/features",
        og_type: "website",
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
        <div class="features-index-page">
            <div class="features-background"></div>
            <section class="features-hero">
                <h1>{"Features"}</h1>
                <p>{"Everything your dumbphone can do with Lightfriend"}</p>
            </section>
            <section class="features-grid">
                <Link<Route> to={Route::FeatureWhatsApp} classes="feature-card">
                    <h2>{"WhatsApp"}</h2>
                    <p>{"Send & receive WhatsApp messages via SMS"}</p>
                </Link<Route>>
                <Link<Route> to={Route::FeatureTelegram} classes="feature-card">
                    <h2>{"Telegram"}</h2>
                    <p>{"Access Telegram from any phone"}</p>
                </Link<Route>>
                <Link<Route> to={Route::FeatureSignal} classes="feature-card">
                    <h2>{"Signal"}</h2>
                    <p>{"Secure messaging on your dumbphone"}</p>
                </Link<Route>>
                <Link<Route> to={Route::FeatureEmail} classes="feature-card">
                    <h2>{"Email"}</h2>
                    <p>{"Gmail & Outlook via SMS"}</p>
                </Link<Route>>
                <Link<Route> to={Route::FeatureCalendar} classes="feature-card">
                    <h2>{"Calendar"}</h2>
                    <p>{"Google Calendar reminders & events"}</p>
                </Link<Route>>
                <Link<Route> to={Route::FeatureTesla} classes="feature-card">
                    <h2>{"Tesla Control"}</h2>
                    <p>{"Lock, unlock, climate, battery via SMS"}</p>
                </Link<Route>>
                <Link<Route> to={Route::FeatureAiSearch} classes="feature-card">
                    <h2>{"AI Search"}</h2>
                    <p>{"Web search via text message"}</p>
                </Link<Route>>
                <Link<Route> to={Route::FeatureGps} classes="feature-card">
                    <h2>{"GPS Directions"}</h2>
                    <p>{"Turn-by-turn navigation via SMS"}</p>
                </Link<Route>>
                <Link<Route> to={Route::FeatureVoiceAi} classes="feature-card">
                    <h2>{"Voice AI"}</h2>
                    <p>{"Call an AI assistant"}</p>
                </Link<Route>>
                <Link<Route> to={Route::FeatureAutopilot} classes="feature-card">
                    <h2>{"Autopilot"}</h2>
                    <p>{"24/7 message monitoring"}</p>
                </Link<Route>>
                <Link<Route> to={Route::FeatureSmartHome} classes="feature-card">
                    <h2>{"Smart Home"}</h2>
                    <p>{"Control Home Assistant via SMS"}</p>
                </Link<Route>>
                <Link<Route> to={Route::FeatureQrScanner} classes="feature-card">
                    <h2>{"QR Scanner"}</h2>
                    <p>{"Decode QR codes via MMS"}</p>
                </Link<Route>>
                <Link<Route> to={Route::FeatureWellness} classes="feature-card">
                    <h2>{"Wellness"}</h2>
                    <p>{"Digital wellness & screen time tools"}</p>
                </Link<Route>>
            </section>
            <style>
                {r#"
                .features-index-page { padding-top: 74px; min-height: 100vh; color: #fff; position: relative; background: transparent; }
                .features-background { position: fixed; top: 0; left: 0; width: 100%; height: 100vh; background: linear-gradient(135deg, #0a0a0a 0%, #1a1a2e 50%, #0a0a0a 100%); z-index: -2; pointer-events: none; }
                .features-hero { text-align: center; padding: 6rem 2rem 3rem; }
                .features-hero h1 { font-size: 3.5rem; margin-bottom: 1rem; background: linear-gradient(45deg, #fff, #7EB2FF); -webkit-background-clip: text; -webkit-text-fill-color: transparent; }
                .features-hero p { font-size: 1.2rem; color: #999; }
                .features-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(250px, 1fr)); gap: 1.5rem; max-width: 1000px; margin: 0 auto; padding: 0 2rem 4rem; }
                .feature-card { display: block; background: rgba(26, 26, 26, 0.85); backdrop-filter: blur(10px); border: 1px solid rgba(30, 144, 255, 0.15); border-radius: 12px; padding: 2rem; text-decoration: none; color: inherit; transition: all 0.3s ease; }
                .feature-card:hover { border-color: rgba(126, 178, 255, 0.4); transform: translateY(-4px); box-shadow: 0 8px 30px rgba(30, 144, 255, 0.15); }
                .feature-card h2 { font-size: 1.4rem; margin-bottom: 0.5rem; background: linear-gradient(45deg, #fff, #7EB2FF); -webkit-background-clip: text; -webkit-text-fill-color: transparent; }
                .feature-card p { color: #999; font-size: 0.95rem; margin: 0; }
                @media (max-width: 768px) { .features-hero h1 { font-size: 2.5rem; } .features-grid { grid-template-columns: 1fr; padding: 0 1rem 3rem; } }
                "#}
            </style>
        </div>
    }
}
