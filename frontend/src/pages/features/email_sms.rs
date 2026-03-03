use yew::prelude::*;
use crate::utils::seo::{use_seo, SeoMeta};

#[function_component(EmailSms)]
pub fn email_sms() -> Html {
    use_seo(SeoMeta {
        title: "Email via SMS – Access Gmail & Outlook on Dumbphone | Lightfriend",
        description: "Access Gmail and Outlook email from any dumbphone via SMS. Read, reply, and get email notifications on Light Phone, Nokia, or any basic phone.",
        canonical: "https://lightfriend.ai/features/email-sms",
        og_type: "website",
    });

    html! {
        <div class="feature-page">
            <div class="feature-hero">
                <h1>{"Email via SMS"}</h1>
                <p class="feature-subtitle">{"Access Gmail and Outlook from any phone — read and reply to emails via SMS"}</p>
            </div>
            <div class="feature-content">
                <section>
                    <h2>{"How It Works"}</h2>
                    <p>{"Email is essential, but it usually requires a smartphone or computer. With Lightfriend, you can read and reply to Gmail and Outlook emails via SMS from any phone — no internet connection needed."}</p>
                    <p>{"Lightfriend monitors your inbox and forwards important emails as text messages. Reply by texting Lightfriend, and your response is sent as an email from your account."}</p>
                </section>
                <section>
                    <h2>{"Features"}</h2>
                    <ul>
                        <li>{"Read incoming emails as SMS"}</li>
                        <li>{"Reply to emails via text message"}</li>
                        <li>{"Gmail and Outlook support"}</li>
                        <li>{"AI-powered email summaries"}</li>
                        <li>{"Priority filtering for important emails"}</li>
                        <li>{"24/7 monitoring with Autopilot plan"}</li>
                    </ul>
                </section>
                <section>
                    <h2>{"Compatible Phones"}</h2>
                    <p>{"Works with Light Phone 2 & 3, Nokia flip phones, any basic phone, any flip phone — any phone that can send SMS."}</p>
                </section>
                <section class="feature-cta">
                    <a href="/register" class="cta-button">{"Get Email on Your Dumbphone"}</a>
                    <a href="/pricing" class="cta-link">{"View pricing →"}</a>
                </section>
            </div>
        </div>
    }
}
