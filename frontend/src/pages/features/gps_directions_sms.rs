use yew::prelude::*;
use crate::utils::seo::{use_seo, SeoMeta};

#[function_component(GpsDirectionsSms)]
pub fn gps_directions_sms() -> Html {
    use_seo(SeoMeta {
        title: "GPS Directions via SMS – Google Maps on Dumbphone | Lightfriend",
        description: "Get GPS directions via SMS from any dumbphone. Turn-by-turn navigation from Google Maps on Light Phone, Nokia, or any basic phone.",
        canonical: "https://lightfriend.ai/features/gps-directions-sms",
        og_type: "website",
    });

    html! {
        <div class="feature-page">
            <div class="feature-hero">
                <h1>{"GPS Directions via SMS"}</h1>
                <p class="feature-subtitle">{"Get turn-by-turn directions from Google Maps via SMS — no smartphone needed"}</p>
            </div>
            <div class="feature-content">
                <section>
                    <h2>{"How It Works"}</h2>
                    <p>{"Navigation apps require a smartphone, but with Lightfriend you can get directions via SMS from any phone. Text your destination and receive step-by-step directions powered by Google Maps."}</p>
                    <p>{"Lightfriend uses the Google Maps API to calculate routes and delivers clear, text-based directions. Get distance, estimated travel time, and turn-by-turn instructions — all via SMS."}</p>
                </section>
                <section>
                    <h2>{"Features"}</h2>
                    <ul>
                        <li>{"Turn-by-turn directions via SMS"}</li>
                        <li>{"Distance and estimated travel time"}</li>
                        <li>{"Driving, walking, and transit directions"}</li>
                        <li>{"Address and business name search"}</li>
                        <li>{"Alternative route options"}</li>
                        <li>{"Powered by Google Maps"}</li>
                    </ul>
                </section>
                <section>
                    <h2>{"Compatible Phones"}</h2>
                    <p>{"Works with Light Phone 2 & 3, Nokia flip phones, any basic phone, any flip phone — any phone that can send SMS."}</p>
                </section>
                <section class="feature-cta">
                    <a href="/register" class="cta-button">{"Get Directions on Your Dumbphone"}</a>
                    <a href="/pricing" class="cta-link">{"View pricing →"}</a>
                </section>
            </div>
        </div>
    }
}
