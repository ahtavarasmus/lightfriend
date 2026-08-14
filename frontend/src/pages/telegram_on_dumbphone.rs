use crate::utils::seo::{use_seo, SeoMeta};
use crate::Route;
use yew::prelude::*;
use yew_router::components::Link;

#[function_component(TelegramOnDumbphone)]
pub fn telegram_on_dumbphone() -> Html {
    use_seo(SeoMeta {
        title: "Telegram on a Dumbphone or Flip Phone: Options | Lightfriend",
        description: "Most feature phones cannot run Telegram natively. Compare a home device or computer with Lightfriend's selected Telegram workflows over ordinary SMS.",
        canonical: "https://lightfriend.ai/telegram-on-dumbphone",
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
                <h1>{"Telegram on a Dumbphone or Flip Phone"}</h1>
                <p>{"Compare a native app-capable device, planned desktop sessions, and selected Telegram workflows through ordinary SMS."}</p>
            </section>
            <section class="blog-content">
                <h2>{"The Short Answer"}</h2>
                <p>{"Most true feature phones do not run Telegram's official apps. Some Android-based minimalist phones are a separate category and may run the native app if their software policy permits it."}</p>
                <p>{"Lightfriend does not install Telegram on the dumbphone. It can provide selected text-first workflows through ordinary SMS after the account is connected separately. Authentication, recovery, rich media, and full account administration stay on a supported device or computer."}</p>
                <p><a href="/blog/telegram-without-smartphone">{"Read the detailed Telegram options and limitations"}</a>{"."}</p>

                <h2>{"How It Works"}</h2>
                <p>{"Instead of running Telegram on your phone, Lightfriend runs it for you in the cloud and relays messages via SMS:"}</p>
                <ol>
                    <li>{"You connect your Telegram account through Lightfriend's web dashboard (one-time setup on any computer)"}</li>
                    <li>{"Lightfriend monitors your Telegram chats 24/7"}</li>
                    <li>{"When someone sends you an important message, Lightfriend texts it to your dumbphone"}</li>
                    <li>{"You reply by texting back - Lightfriend sends your reply through Telegram"}</li>
                    <li>{"You can also text Lightfriend anytime to check messages, send to a specific contact, or get a summary of what you missed"}</li>
                </ol>
                <p>{"Everything happens through your dumbphone's native SMS. No apps, no internet connection on the phone, no workarounds."}</p>

                <h2>{"What You Can Do"}</h2>
                <ul>
                    <li><strong>{"Send messages"}</strong>{" - Text Lightfriend and it sends your message via Telegram to the right person"}</li>
                    <li><strong>{"Receive messages"}</strong>{" - Important messages are forwarded to you as SMS automatically"}</li>
                    <li><strong>{"Group chats"}</strong>{" - Monitor group conversations and get summaries"}</li>
                    <li><strong>{"Check on demand"}</strong>{" - Text anytime to see your recent Telegram messages"}</li>
                    <li><strong>{"Scheduled digests"}</strong>{" - Get regular summaries of everything you missed"}</li>
                    <li><strong>{"Custom rules"}</strong>{" - Build WHEN/IF/THEN automations, like \"when Mom texts, always notify me immediately\""}</li>
                    <li><strong>{"Voice"}</strong>{" - Call Lightfriend's number to hear and send messages by voice"}</li>
                </ul>

                <h2>{"Smart Filtering: Not Every Message Interrupts You"}</h2>
                <p>{"The whole point of a dumbphone is fewer interruptions. Lightfriend doesn't flood you with every Telegram notification - it uses AI to figure out what actually matters."}</p>
                <p>{"You set custom rules for what gets forwarded: a message from your partner always comes through immediately, group chat noise gets batched into digests. Rules use WHEN/IF/THEN blocks so you control exactly what interrupts you."}</p>
                <p>{"This is the key difference from just forwarding all notifications - you get a smart filter that respects your minimalist lifestyle while making sure you never miss something important."}</p>

                <h2>{"Compatible Phones"}</h2>
                <p>{"The intended route works with phones that have unrestricted two-way SMS and a supported Lightfriend number route:"}</p>
                <ul>
                    <li>{"Light Phone 2, Light Phone 3, and Light Flip"}</li>
                    <li>{"Nokia flip phones (2780 Flip, 2660 Flip, 2760 Flip)"}</li>
                    <li>{"Punkt MP02"}</li>
                    <li>{"Mudita Pure"}</li>
                    <li>{"Cat phones"}</li>
                    <li>{"Any basic phone, candy bar phone, or feature phone"}</li>
                    <li>{"Even old smartphones used as dumbphones"}</li>
                </ul>

                <h2>{"Setup Guide"}</h2>
                <ol>
                    <li><strong>{"Sign up"}</strong>{" at lightfriend.ai from any computer or phone with a browser"}</li>
                    <li><strong>{"Connect Telegram"}</strong>{" through the dashboard (takes a few minutes)"}</li>
                    <li><strong>{"Configure"}</strong>{" your notification preferences and custom rules"}</li>
                    <li><strong>{"Save Lightfriend's number"}</strong>{" in your dumbphone contacts"}</li>
                    <li><strong>{"Start texting"}</strong>{" - send and receive Telegram messages via SMS"}</li>
                </ol>

                <h2>{"Telegram on Dumbphone: With vs Without Lightfriend"}</h2>
                <table class="comparison-table">
                    <thead>
                        <tr>
                            <th>{"Feature"}</th>
                            <th>{"Dumbphone Alone"}</th>
                            <th>{"With Lightfriend"}</th>
                        </tr>
                    </thead>
                    <tbody>
                        <tr>
                            <td>{"Send Telegram messages"}</td>
                            <td>{"Not possible"}</td>
                            <td>{"Yes, via SMS or voice call"}</td>
                        </tr>
                        <tr>
                            <td>{"Receive Telegram messages"}</td>
                            <td>{"Not possible"}</td>
                            <td>{"Forwarded as SMS"}</td>
                        </tr>
                        <tr>
                            <td>{"Group chat access"}</td>
                            <td>{"Not possible"}</td>
                            <td>{"Summaries and monitoring"}</td>
                        </tr>
                        <tr>
                            <td>{"Smart notification filtering"}</td>
                            <td>{"N/A"}</td>
                            <td>{"AI filters for importance"}</td>
                        </tr>
                        <tr>
                            <td>{"Privacy"}</td>
                            <td>{"N/A"}</td>
                            <td>{"Hardware-encrypted enclave"}</td>
                        </tr>
                        <tr>
                            <td>{"Apps to install"}</td>
                            <td>{"None available"}</td>
                            <td>{"None needed"}</td>
                        </tr>
                    </tbody>
                </table>

                <h2>{"Privacy"}</h2>
                <p>{"Your Telegram messages pass through Lightfriend's production application inside an AWS Nitro Enclave. Stored application data is encrypted, the source code is public, and the enclave's signed attestation reports a code measurement that can be compared with the published build."}</p>
                <p>{"For the full technical explanation: "}<Link<Route> to={Route::Trustless}>{"Review Lightfriend's privacy architecture"}</Link<Route>>{"."}</p>

                <h2>{"Frequently Asked Questions"}</h2>
                <p><strong>{"Q: Do I need a smartphone at all?"}</strong></p>
                <p>{"You need access to your Telegram account for the initial connection (smartphone or computer). After that, everything works through your dumbphone's SMS."}</p>
                <p><strong>{"Q: What happens if I get a lot of Telegram messages?"}</strong></p>
                <p>{"Lightfriend doesn't forward everything. It uses AI to determine what's important and batches the rest into scheduled digests. You control the rules."}</p>
                <p><strong>{"Q: Can I use this with Telegram groups?"}</strong></p>
                <p>{"Yes. You can monitor groups and get summaries. You can also send messages to groups via Lightfriend."}</p>
                <p><strong>{"Q: Does this work internationally?"}</strong></p>
                <p>{"Yes. Lightfriend supports 40+ countries with local or notification-only numbers. You can also bring your own Twilio number for countries not on the list."}</p>
                <p><strong>{"Q: What about Telegram voice messages and media?"}</strong></p>
                <p>{"Media like images and voice messages on Telegram are not yet supported - you'll receive a note that media was sent. Text messages work fully."}</p>

                <h2>{"Also Works With"}</h2>
                <p>{"Lightfriend isn't just for Telegram. The same approach works for all your messaging apps:"}</p>
                <ul>
                    <li><Link<Route> to={Route::LightPhone3WhatsappGuide}>{"WhatsApp on dumbphone"}</Link<Route>></li>
                    <li><Link<Route> to={Route::SignalOnDumbphone}>{"Signal on dumbphone"}</Link<Route>></li>
                    <li>{"Email (any provider)"}</li>
                </ul>
                <p>{"Connect all of them and get one unified SMS interface for everything."}</p>

                <div class="blog-cta">
                    <h3>{"Check Telegram in Your Exit Plan"}</h3>
                    <a href="/can-i-leave-my-smartphone" class="forward-link" data-fast-goal="exit_planner_click" data-fast-goal-content-slug="telegram-on-dumbphone" data-fast-goal-content-cluster="messaging">
                        <button class="hero-cta">{"Build Your Plan"}</button>
                    </a>
                    <p>{"Then verify your country route, setup device, message volume, and ordinary SMS compatibility."}</p>
                    <a href="/#plans" data-fast-goal="blog_pricing_click" data-fast-goal-content-slug="telegram-on-dumbphone" data-fast-goal-content-cluster="messaging">{"See plans and start a trial"}</a>
                </div>
            </section>
            <style>
                {include_str!("blog_styles.css")}
            </style>
        </div>
    }
}
