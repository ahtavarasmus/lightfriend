use yew::prelude::*;
use crate::utils::seo::{use_seo, SeoMeta};

#[function_component(SmartHomeSms)]
pub fn smart_home_sms() -> Html {
    use_seo(SeoMeta {
        title: "Smart Home via SMS – Control Home Assistant from Any Phone | Lightfriend",
        description: "Control your smart home from any dumbphone via SMS. Manage Home Assistant lights, locks, thermostats, and more from Light Phone, Nokia, or any basic phone.",
        canonical: "https://lightfriend.ai/features/smart-home-sms",
        og_type: "website",
    });

    html! {
        <div class="feature-page">
            <div class="feature-hero">
                <h1>{"Smart Home via SMS"}</h1>
                <p class="feature-subtitle">{"Control your smart home from any phone — manage Home Assistant via SMS"}</p>
            </div>
            <div class="feature-content">
                <section>
                    <h2>{"How It Works"}</h2>
                    <p>{"Smart home control usually requires a smartphone app or voice assistant. With Lightfriend, you can control your Home Assistant setup via SMS from any phone — turn on lights, lock doors, adjust thermostats, and more."}</p>
                    <p>{"Lightfriend integrates with Home Assistant through MCP (Model Context Protocol). Text natural language commands like \"turn off living room lights\" and Lightfriend translates them into smart home actions."}</p>
                </section>
                <section>
                    <h2>{"Features"}</h2>
                    <ul>
                        <li>{"Control lights, switches, and dimmers"}</li>
                        <li>{"Lock and unlock smart locks"}</li>
                        <li>{"Adjust thermostats and climate control"}</li>
                        <li>{"Check sensor readings and device status"}</li>
                        <li>{"Run Home Assistant automations and scenes"}</li>
                        <li>{"Natural language commands via SMS"}</li>
                    </ul>
                </section>
                <section>
                    <h2>{"Compatible Phones"}</h2>
                    <p>{"Works with Light Phone 2 & 3, Nokia flip phones, any basic phone, any flip phone — any phone that can send SMS."}</p>
                </section>
                <section class="feature-cta">
                    <a href="/register" class="cta-button">{"Control Your Smart Home via SMS"}</a>
                    <a href="/pricing" class="cta-link">{"View pricing →"}</a>
                </section>
            </div>
        </div>
    }
}
