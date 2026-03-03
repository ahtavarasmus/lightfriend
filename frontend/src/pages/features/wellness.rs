use yew::prelude::*;
use crate::utils::seo::{use_seo, SeoMeta};

#[function_component(Wellness)]
pub fn wellness() -> Html {
    use_seo(SeoMeta {
        title: "Digital Wellness – Dumbphone Mode & Wellbeing Tracking | Lightfriend",
        description: "Digital wellness features for dumbphone users. Dumbphone mode, daily check-ins, wellbeing tracking, and mindful technology use with Lightfriend.",
        canonical: "https://lightfriend.ai/features/wellness",
        og_type: "website",
    });

    html! {
        <div class="feature-page">
            <div class="feature-hero">
                <h1>{"Digital Wellness"}</h1>
                <p class="feature-subtitle">{"Mindful technology use — dumbphone mode, check-ins, and wellbeing tracking"}</p>
            </div>
            <div class="feature-content">
                <section>
                    <h2>{"How It Works"}</h2>
                    <p>{"Using a dumbphone is a choice for digital wellness. Lightfriend enhances that choice with features designed to support your wellbeing — without pulling you back into smartphone habits."}</p>
                    <p>{"Enable dumbphone mode to limit distractions, set up daily check-ins for mindful reflection, and track your wellbeing over time. Lightfriend helps you stay connected to what matters while maintaining healthy boundaries with technology."}</p>
                </section>
                <section>
                    <h2>{"Features"}</h2>
                    <ul>
                        <li>{"Dumbphone mode — limit notifications to essentials"}</li>
                        <li>{"Daily wellness check-ins via SMS"}</li>
                        <li>{"Mood and wellbeing tracking"}</li>
                        <li>{"Customizable quiet hours"}</li>
                        <li>{"Weekly wellness summaries"}</li>
                        <li>{"Mindful message batching"}</li>
                    </ul>
                </section>
                <section>
                    <h2>{"Compatible Phones"}</h2>
                    <p>{"Works with Light Phone 2 & 3, Nokia flip phones, any basic phone, any flip phone — any phone that can send SMS."}</p>
                </section>
                <section class="feature-cta">
                    <a href="/register" class="cta-button">{"Start Your Wellness Journey"}</a>
                    <a href="/pricing" class="cta-link">{"View pricing →"}</a>
                </section>
            </div>
        </div>
    }
}
