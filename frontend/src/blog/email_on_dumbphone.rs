use yew::prelude::*;
use yew_router::components::Link;
use crate::Route;
use crate::utils::seo::{use_seo, SeoMeta};

#[function_component(EmailOnDumbphone)]
pub fn email_on_dumbphone() -> Html {
    use_seo(SeoMeta {
        title: "How to Get Email on a Dumbphone (Gmail & Outlook)",
        description: "Complete guide to accessing Gmail, Outlook, and other email on a dumbphone. Email forwarding, Lightfriend SMS bridge, and more methods.",
        canonical: "https://lightfriend.ai/blog/email-on-dumbphone",
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
                <h1>{"How to Get Email on a Dumbphone (Gmail & Outlook)"}</h1>
                <p>{"Never miss an important email, even without a smartphone."}</p>
            </section>
            <section class="blog-content">
                <h2>{"Email Is Still Essential"}</h2>
                <p>{"Email remains the backbone of professional communication. Bills, appointments, work messages, shipping notifications \u{2013} it all arrives via email. When you switch to a dumbphone, losing email access is one of the biggest concerns. Here are all the ways to solve it."}</p>

                <h2>{"Method 1: Email Forwarding to SMS"}</h2>
                <p>{"Some carriers offer email-to-SMS gateways where emails sent to a specific address get forwarded as text messages. However, this is unreliable, heavily truncates messages, and does not support replies."}</p>
                <ul>
                    <li><strong>{"Pros:"}</strong>{" Free, no setup required beyond knowing your carrier\u{2019}s gateway address."}</li>
                    <li><strong>{"Cons:"}</strong>{" Unreliable delivery, severe character limits, no reply capability, no attachments, most carriers have deprecated this."}</li>
                </ul>

                <h2>{"Method 2: Check Email on Computer Only"}</h2>
                <p>{"The simplest approach: only check email when you are at your computer. Set specific times (morning, lunch, evening) and batch-process your inbox."}</p>
                <ul>
                    <li><strong>{"Pros:"}</strong>{" Free, encourages healthy email habits, reduces compulsive checking."}</li>
                    <li><strong>{"Cons:"}</strong>{" You miss time-sensitive emails when away from your computer, not practical for everyone."}</li>
                </ul>

                <h2>{"Method 3: Lightfriend Email Bridge"}</h2>
                <p>{"Lightfriend connects to your Gmail, Outlook, or any IMAP email account and intelligently forwards important emails to your dumbphone via SMS. It uses AI to summarize long emails into concise text messages and lets you reply via SMS."}</p>
                <ol>
                    <li>{"Sign up for Lightfriend and connect your email account (Gmail, Outlook, or custom IMAP)."}</li>
                    <li>{"Configure notification preferences: all emails, important only, or specific senders/labels."}</li>
                    <li>{"Receive email summaries via SMS on your dumbphone."}</li>
                    <li>{"Reply by texting back \u{2013} Lightfriend sends your reply from your email address."}</li>
                    <li>{"For detailed emails, text \"read full\" to get the complete message in parts."}</li>
                </ol>

                <h2>{"Method 4: KaiOS Email Apps"}</h2>
                <p>{"Some KaiOS phones have basic email clients in the KaiStore. The Gmail app on KaiOS provides a stripped-down inbox view."}</p>
                <ul>
                    <li><strong>{"Pros:"}</strong>{" Direct email access on the phone, free."}</li>
                    <li><strong>{"Cons:"}</strong>{" Very slow, tiny screen makes reading painful, limited to KaiOS phones, app may lose support."}</li>
                </ul>

                <h2>{"Recommended Setup"}</h2>
                <p>{"For most dumbphone users, the best approach combines methods: use "}<strong>{"Lightfriend"}</strong>{" for real-time important email notifications on your dumbphone, and check your full inbox on your computer during set times. This gives you peace of mind that urgent emails reach you immediately, while keeping your distraction-free lifestyle intact."}</p>

                <div class="blog-cta">
                    <h3>{"Get Email on Any Phone"}</h3>
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
