use yew::prelude::*;
use crate::utils::seo::{use_seo, SeoMeta};

#[function_component(ForLightPhone)]
pub fn for_light_phone() -> Html {
    use_seo(SeoMeta {
        title: "Lightfriend for Light Phone \u{2013} The Perfect Companion | Lightfriend",
        description: "Lightfriend is the perfect companion for Light Phone 2 and Light Phone 3. Get WhatsApp, Telegram, email, and AI search via SMS.",
        canonical: "https://lightfriend.ai/for/light-phone",
        og_type: "website",
    });

    html! {
        <div class="audience-page">
            <div class="audience-hero">
                <h1>{"Lightfriend for Light Phone"}</h1>
                <p class="audience-subtitle">{"The perfect companion for your Light Phone 2 or Light Phone 3."}</p>
            </div>
            <div class="audience-content">
                <section>
                    <h2>{"Why Light Phone + Lightfriend?"}</h2>
                    <p>{"The Light Phone is beautifully minimal. Lightfriend fills in the gaps without adding screen time or distractions. Everything works through SMS \u{2014} no apps to install."}</p>
                    <ul>
                        <li>{"Works with both Light Phone 2 (e-ink) and Light Phone 3"}</li>
                        <li>{"No app installation needed \u{2014} pure SMS and voice"}</li>
                        <li>{"Keeps the minimalist experience intact"}</li>
                        <li>{"Adds functionality without adding distraction"}</li>
                    </ul>
                </section>
                <section>
                    <h2>{"Features Lightfriend Adds"}</h2>
                    <p>{"Everything arrives as a text message or voice call:"}</p>
                    <ul>
                        <li>{"WhatsApp, Telegram, and Signal messaging"}</li>
                        <li>{"Email \u{2014} receive, read, and reply"}</li>
                        <li>{"Calendar reminders and daily briefings"}</li>
                        <li>{"AI web search \u{2014} ask any question via text"}</li>
                        <li>{"GPS navigation via voice directions"}</li>
                        <li>{"Weather, news summaries, and more"}</li>
                    </ul>
                </section>
                <section>
                    <h2>{"Getting Started"}</h2>
                    <ol>
                        <li>{"Sign up for Lightfriend from your computer"}</li>
                        <li>{"Add your Light Phone number"}</li>
                        <li>{"Connect your messaging and email accounts"}</li>
                        <li>{"Start texting Lightfriend \u{2014} it just works"}</li>
                    </ol>
                </section>
                <section class="audience-cta">
                    <a href="/register" class="cta-button">{"Get Started"}</a>
                    <a href="/pricing" class="cta-link">{"View pricing \u{2192}"}</a>
                </section>
            </div>
        </div>
    }
}
