use crate::utils::seo::{use_seo, SeoMeta};
use yew::prelude::*;

#[function_component(SwitchToDumbphoneGuide)]
pub fn switch_to_dumbphone_guide() -> Html {
    use_seo(SeoMeta {
        title: "How to Switch to a Dumbphone Without Losing Essential Apps",
        description: "A practical dumbphone switching checklist for WhatsApp, 2FA, banking, maps, payments, transport, and account recovery.",
        canonical: "https://lightfriend.ai/how-to-switch-to-dumbphone",
        og_type: "article",
    });
    // Scroll to top only on initial mount
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
                <h1>{"How to Switch to a Dumbphone Without Losing What Matters"}</h1>
                <p>{"A practical plan for WhatsApp, 2FA, banking, maps, payments, transport, and account recovery."}</p>
                <img src="/assets/lightphone2.png" alt="Light Phone 2" loading="lazy" class="blog-image" />
            </section>
            <section class="blog-content">
                <p><strong>{"The safest switch is gradual: keep the old smartphone secured at home, move everyday calls and texts to the dumbphone, and replace one essential dependency at a time."}</strong>{" Do not erase, sell, or trade in the smartphone until the new setup has worked for several weeks."}</p>

                <h2>{"1. Inventory what your smartphone actually does"}</h2>
                <p>{"Review the last seven days and sort each important use into three groups: must work while you are away, can wait until you are home, or needs a tested replacement. Calls and SMS usually belong on the dumbphone. Routine WhatsApp and email can often wait. Banking approval, authentication, tickets, and navigation need a deliberate plan."}</p>
                <table class="comparison-table">
                    <thead><tr><th>{"Need"}</th><th>{"Possible dumbphone plan"}</th><th>{"Verify before switching"}</th></tr></thead>
                    <tbody>
                        <tr><td>{"WhatsApp and group chats"}</td><td>{"Computer checks, a home smartphone, or selected access through Lightfriend"}</td><td>{"Pairing, recovery, urgent-contact fallback"}</td></tr>
                        <tr><td>{"2FA and passkeys"}</td><td>{"Recovery codes, security key, computer passkey, or a supported authenticator"}</td><td>{"Every critical account individually"}</td></tr>
                        <tr><td>{"Banking and payments"}</td><td>{"Computer banking, physical token, SMS approval, and payment cards"}</td><td>{"Your bank's exact policy"}</td></tr>
                        <tr><td>{"Maps and transport"}</td><td>{"Built-in directions, printed routes, travel card, or taxi number"}</td><td>{"One ordinary trip and one unfamiliar route"}</td></tr>
                    </tbody>
                </table>

                <h2>{"2. Choose the smallest phone that fits"}</h2>
                <p>{"Decide whether you want a true feature phone, a purpose-built minimalist phone, or an Android-based phone with fewer distractions. A phone that runs the official WhatsApp app is still an app-capable phone; that may be the right compromise, but it is not the same intervention as removing the app platform from your pocket."}</p>
                <p>{"Check carrier compatibility, calling, SMS, hotspot support, maps, accessibility, and battery behavior for the exact model and region. Light Phone III owners should review Light's current "}<a href="https://support.thelightphone.com/hc/en-us/articles/360031128671-Tool-Availability-Status" target="_blank" rel="noopener noreferrer">{"official tool list"}</a>{" rather than relying on an old review."}</p>

                <h2>{"3. Keep your computer useful, not addictive"}</h2>
                <p>{"A computer is a good home for tasks that deserve a deliberate session: long email, account administration, rich group chats, travel planning, and document work. Remove automatic launches and notifications, then choose specific times to check communication."}</p>
                <p>{"Optional tools include:"}</p>
                <ul>
                    <li>
                        <a href="https://beeper.com" target="_blank" rel="noopener noreferrer">{"Beeper"}</a>{" or the providers' official desktop apps for scheduled messaging sessions"}
                    </li>
                    <li>
                        <a href="https://getcoldturkey.com" target="_blank" rel="noopener noreferrer">{"Cold Turkey"}</a>{" or another website blocker to protect focused work periods"}
                    </li>
                </ul>

                <h2>{"4. Build account-recovery and 2FA fallbacks"}</h2>
                <p>{"For every important account, identify whether it uses an authenticator code, push approval, SMS, passkey, or hardware security key. Store recovery codes securely and add a second supported sign-in method before changing devices. A hardware key is useful where supported, but it does not replace every authenticator or banking app."}</p>

                <h2>{"5. Make a banking and payments plan"}</h2>
                <p>{"Banks vary by institution and country. Sign in from the computer you plan to use, verify the lost-device recovery process, and ask the bank about web approval, SMS, or physical tokens. Carry a payment card and keep the bank's official support number. Never wipe the only device capable of approving a new login."}</p>

                <h2>{"6. Decide how WhatsApp will work"}</h2>
                <p>{"You can check WhatsApp at home, use the official app on an Android-based minimalist phone, leave the account entirely, or use a text-first bridge for selected access. The "}<a href="/blog/best-dumbphone-whatsapp-setup-2026">{"dumbphone WhatsApp comparison"}</a>{" explains the differences. Light Phone III does not currently list a native WhatsApp tool."}</p>
                <p>{"Lightfriend can let you ask about connected messages, reply, receive focused alerts, and request summaries through SMS and voice. It does not reproduce every provider feature. Initial pairing and later linked-device maintenance can still require the native app on a separate supported device."}</p>

                <h2>{"7. Test the transition before making it permanent"}</h2>
                <ol>
                    <li>{"Leave the smartphone at home for one ordinary weekend"}</li>
                    <li>{"Test calls, SMS, voicemail, and your emergency-contact instructions"}</li>
                    <li>{"Take one normal journey and make one real payment"}</li>
                    <li>{"Sign in to one protected account using the planned 2FA fallback"}</li>
                    <li>{"Record every unplanned return to the smartphone"}</li>
                    <li>{"Fix the real gaps, then repeat for a full week"}</li>
                </ol>

                <h2>{"Detailed setup guides"}</h2>
                <ul>
                    <li><a href="/blog/smartphone-at-home-dumbphone">{"Keep your smartphone at home and carry a dumbphone"}</a></li>
                    <li><a href="/light-phone-3-whatsapp-guide">{"Compare WhatsApp options for Light Phone III"}</a></li>
                    <li><a href="/blog/digital-detox-with-whatsapp">{"Keep urgent WhatsApp messages during a digital detox"}</a></li>
                    <li><a href="/blog/ai-email-on-dumbphone">{"Manage selected email from a dumbphone"}</a></li>
                </ul>
                <div class="blog-cta">
                    <h3>{"Ready to Switch to a Dumbphone?"}</h3>
                    <a href="/#plans" class="forward-link" data-fast-goal="blog_pricing_click" data-fast-goal-content-slug="how-to-switch-to-dumbphone" data-fast-goal-content-cluster="minimalism">
                        <button class="hero-cta">{"See Lightfriend Plans"}</button>
                    </a>
                    <p>{"Use calls and SMS as the small interface to selected messages, email, reminders, and AI assistance."}</p>
                </div>
            </section>
            <style>
                {r#"
                .blog-page {
                    padding-top: 74px;
                    min-height: 100vh;
                    color: #ffffff;
                    position: relative;
                    background: transparent;
                }
                .blog-background {
                    position: fixed;
                    top: 0;
                    left: 0;
                    width: 100%;
                    height: 100vh;
                    background-image: url('/assets/field_asthetic_not.webp');
                    background-size: cover;
                    background-position: center;
                    background-repeat: no-repeat;
                    opacity: 1;
                    z-index: -2;
                    pointer-events: none;
                }
                .blog-background::after {
                    content: '';
                    position: absolute;
                    bottom: 0;
                    left: 0;
                    width: 100%;
                    height: 50%;
                    background: linear-gradient(
                        to bottom,
                        rgba(26, 26, 26, 0) 0%,
                        rgba(26, 26, 26, 1) 100%
                    );
                }
                .blog-hero {
                    text-align: center;
                    padding: 6rem 2rem;
                    background: var(--surface-card);
                    margin-top: 2rem;
                    border: 1px solid var(--border-card);
                    margin-bottom: 2rem;
                }
                .blog-hero h1 {
                    font-size: 3.5rem;
                    margin-bottom: 1.5rem;
                    background: linear-gradient(45deg, #fff, #7EB2FF);
                    -webkit-background-clip: text;
                    -webkit-text-fill-color: transparent;
                }
                .blog-hero p {
                    font-size: 1.2rem;
                    color: var(--text-body);
                    max-width: 600px;
                    margin: 0 auto;
                }
                .blog-content {
                    max-width: 800px;
                    margin: 0 auto;
                    padding: 2rem;
                }
                .blog-content h2 {
                    font-size: 2.5rem;
                    margin: 3rem 0 1rem;
                    background: linear-gradient(45deg, #fff, #7EB2FF);
                    -webkit-background-clip: text;
                    -webkit-text-fill-color: transparent;
                }
                .blog-content p {
                    color: var(--text-body);
                    line-height: 1.6;
                    margin-bottom: 1.5rem;
                }
                .blog-content ul, .blog-content ol {
                    color: var(--text-body);
                    padding-left: 1.5rem;
                    margin-bottom: 1.5rem;
                }
                .blog-content li {
                    margin-bottom: 0.75rem;
                }
                .blog-content a {
                    color: #7EB2FF;
                    text-decoration: none;
                    border-bottom: 1px solid rgba(126, 178, 255, 0.3);
                    transition: all 0.3s ease;
                    font-weight: 500;
                }
                .blog-content a:hover {
                    color: #ffffff;
                    border-bottom-color: #7EB2FF;
                    text-shadow: 0 0 5px rgba(126, 178, 255, 0.5);
                }
                .blog-image {
                    max-width: 100%;
                    height: auto;
                    border-radius: 12px;
                    margin: 2rem 0;
                    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.3);
                }
                .comparison-table {
                    width: 100%;
                    border-collapse: collapse;
                    margin: 2rem 0;
                    color: #ddd;
                }
                .comparison-table th, .comparison-table td {
                    padding: 1rem;
                    border: 1px solid var(--border-card);
                    text-align: left;
                }
                .comparison-table th {
                    background: rgba(0, 0, 0, 0.5);
                    color: #7EB2FF;
                }
                .blog-cta {
                    text-align: center;
                    margin: 4rem 0 2rem;
                    padding: 2rem;
                    background: var(--surface-subtle);
                    border: 1px solid var(--border-card);
                    border-radius: 12px;
                }
                .blog-cta h3 {
                    font-size: 2rem;
                    margin-bottom: 1.5rem;
                    background: linear-gradient(45deg, #fff, #7EB2FF);
                    -webkit-background-clip: text;
                    -webkit-text-fill-color: transparent;
                }
                .blog-cta p {
                    color: #999;
                    margin-top: 1rem;
                }
                .hero-cta {
                    background: linear-gradient(45deg, #7EB2FF, #4169E1);
                    color: white;
                    border: none;
                    padding: 1rem 2.5rem;
                    border-radius: 8px;
                    font-size: 1.1rem;
                    cursor: pointer;
                    transition: all 0.3s ease;
                }
                .hero-cta:hover {
                    transform: translateY(-2px);
                    box-shadow: 0 4px 20px rgba(126, 178, 255, 0.4);
                }
                @media (max-width: 768px) {
                    .blog-hero {
                        padding: 4rem 1rem;
                    }
                    .blog-hero h1 {
                        font-size: 2.5rem;
                    }
                    .blog-content {
                        padding: 1rem;
                    }
                    .blog-content h2 {
                        font-size: 2rem;
                    }
                    .comparison-table th, .comparison-table td {
                        padding: 0.75rem;
                        font-size: 0.9rem;
                    }
                }
                "#}
            </style>
        </div>
    }
}
