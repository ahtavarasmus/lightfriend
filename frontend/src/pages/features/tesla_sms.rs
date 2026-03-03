use yew::prelude::*;
use crate::utils::seo::{use_seo, SeoMeta};

#[function_component(TeslaSms)]
pub fn tesla_sms() -> Html {
    use_seo(SeoMeta {
        title: "Tesla SMS Control – Control Your Tesla from Any Phone | Lightfriend",
        description: "Control your Tesla from any dumbphone via SMS. Lock, unlock, check battery, and control climate from Light Phone, Nokia, or any basic phone.",
        canonical: "https://lightfriend.ai/features/tesla-sms",
        og_type: "website",
    });

    html! {
        <div class="feature-page">
            <div class="feature-hero">
                <h1>{"Tesla SMS Control"}</h1>
                <p class="feature-subtitle">{"Control your Tesla from any phone — lock, unlock, and monitor via SMS"}</p>
            </div>
            <div class="feature-content">
                <section>
                    <h2>{"How It Works"}</h2>
                    <p>{"The Tesla app requires a smartphone, but with Lightfriend you can control your Tesla via SMS from any phone. Lock and unlock doors, check battery level, pre-condition climate, and more — all through text messages."}</p>
                    <p>{"Lightfriend connects to the Tesla API and translates your SMS commands into vehicle actions. Text a command like \"unlock my car\" or \"what's my battery level\" and get instant responses."}</p>
                </section>
                <section>
                    <h2>{"Features"}</h2>
                    <ul>
                        <li>{"Lock and unlock doors via SMS"}</li>
                        <li>{"Check battery level and range"}</li>
                        <li>{"Pre-condition climate control"}</li>
                        <li>{"Check vehicle location"}</li>
                        <li>{"Open and close trunk"}</li>
                        <li>{"Natural language commands"}</li>
                    </ul>
                </section>
                <section>
                    <h2>{"Compatible Phones"}</h2>
                    <p>{"Works with Light Phone 2 & 3, Nokia flip phones, any basic phone, any flip phone — any phone that can send SMS."}</p>
                </section>
                <section class="feature-cta">
                    <a href="/register" class="cta-button">{"Control Your Tesla via SMS"}</a>
                    <a href="/pricing" class="cta-link">{"View pricing →"}</a>
                </section>
            </div>
        </div>
    }
}
