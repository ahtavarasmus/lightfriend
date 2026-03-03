use yew::prelude::*;
use crate::utils::seo::{use_seo, SeoMeta};

#[function_component(ForParents)]
pub fn for_parents() -> Html {
    use_seo(SeoMeta {
        title: "Safe First Phone for Kids \u{2013} Dumbphone + AI Assistant | Lightfriend",
        description: "Give your child a safe first phone. A dumbphone with Lightfriend provides communication without social media, browsers, or app stores.",
        canonical: "https://lightfriend.ai/for/parents",
        og_type: "website",
    });

    html! {
        <div class="audience-page">
            <div class="audience-hero">
                <h1>{"Safe First Phone for Kids"}</h1>
                <p class="audience-subtitle">{"A dumbphone keeps kids safe. Lightfriend keeps them connected."}</p>
            </div>
            <div class="audience-content">
                <section>
                    <h2>{"Why a Dumbphone Is Safer"}</h2>
                    <p>{"Smartphones expose children to risks that no parental control app fully solves. A dumbphone removes these risks at the hardware level:"}</p>
                    <ul>
                        <li>{"No social media \u{2014} no cyberbullying, no comparison culture"}</li>
                        <li>{"No web browser \u{2014} no inappropriate content"}</li>
                        <li>{"No app store \u{2014} no addictive games or in-app purchases"}</li>
                        <li>{"No camera pressure \u{2014} no photo sharing or sexting risk"}</li>
                        <li>{"Calls and texts only \u{2014} the basics done right"}</li>
                    </ul>
                </section>
                <section>
                    <h2>{"What Lightfriend Adds for Kids"}</h2>
                    <p>{"Your child gets useful features without the dangers:"}</p>
                    <ul>
                        <li>{"WhatsApp and Telegram messaging with family and friends"}</li>
                        <li>{"AI-powered homework help via text"}</li>
                        <li>{"Calendar reminders for school and activities"}</li>
                        <li>{"GPS directions when they need to find their way"}</li>
                        <li>{"All communication goes through SMS \u{2014} easy to monitor"}</li>
                    </ul>
                </section>
                <section>
                    <h2>{"Recommended Setup for Families"}</h2>
                    <ol>
                        <li>{"Get a Nokia flip phone or Light Phone for your child"}</li>
                        <li>{"Sign up for Lightfriend and add their number"}</li>
                        <li>{"Connect family WhatsApp or group chats"}</li>
                        <li>{"Your child stays reachable without the risks of a smartphone"}</li>
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
