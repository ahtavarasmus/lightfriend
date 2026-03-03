use yew::prelude::*;
use crate::utils::seo::{use_seo, SeoMeta};

#[function_component(ForBusiness)]
pub fn for_business() -> Html {
    use_seo(SeoMeta {
        title: "Business Dumbphone \u{2013} Stay Productive Without Distractions | Lightfriend",
        description: "Use a dumbphone for work without losing email, calendar, or messaging. Lightfriend delivers business essentials via SMS.",
        canonical: "https://lightfriend.ai/for/business",
        og_type: "website",
    });

    html! {
        <div class="audience-page">
            <div class="audience-hero">
                <h1>{"Business Dumbphone"}</h1>
                <p class="audience-subtitle">{"Stay productive at work. Leave the distractions behind."}</p>
            </div>
            <div class="audience-content">
                <section>
                    <h2>{"Why Professionals Choose Dumbphones"}</h2>
                    <p>{"Smartphones kill productivity. Notifications, social media, and endless apps fragment your attention throughout the workday. A dumbphone with Lightfriend gives you focus while keeping you reachable."}</p>
                    <ul>
                        <li>{"Deep work becomes possible \u{2014} no app notifications"}</li>
                        <li>{"Meetings stay focused \u{2014} no phones to check"}</li>
                        <li>{"Client calls still come through \u{2014} it\u{2019}s still a phone"}</li>
                        <li>{"Email and calendar stay accessible via SMS"}</li>
                    </ul>
                </section>
                <section>
                    <h2>{"Business Features via SMS"}</h2>
                    <p>{"Lightfriend delivers the tools professionals need:"}</p>
                    <ul>
                        <li>{"Email \u{2014} receive, read, and reply to work emails"}</li>
                        <li>{"Calendar \u{2014} meeting reminders and daily schedule"}</li>
                        <li>{"WhatsApp and Telegram \u{2014} stay in team and client chats"}</li>
                        <li>{"AI search \u{2014} quick answers without opening a browser"}</li>
                        <li>{"GPS navigation \u{2014} get to meetings on time"}</li>
                    </ul>
                </section>
                <section>
                    <h2>{"Get Set Up for Work"}</h2>
                    <ol>
                        <li>{"Choose a dumbphone \u{2014} Light Phone 3 for style, Nokia for durability"}</li>
                        <li>{"Sign up for Lightfriend Autopilot ($29/month)"}</li>
                        <li>{"Connect your work email and calendar"}</li>
                        <li>{"Add WhatsApp or Telegram for team messaging"}</li>
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
