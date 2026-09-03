use crate::utils::seo::{use_seo, SeoMeta};
use crate::Route;
use yew::prelude::*;
use yew_router::components::Link;

#[function_component(SignalOnDumbphone)]
pub fn signal_on_dumbphone() -> Html {
    use_seo(SeoMeta {
        title: "Signal on a Dumbphone or Flip Phone: What Works | Lightfriend",
        description: "Most feature phones cannot run Signal natively. Compare a separate supported device with Lightfriend's selected Signal workflows over ordinary SMS.",
        canonical: "https://lightfriend.ai/signal-on-dumbphone",
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
                <h1>{"Signal on a Dumbphone or Flip Phone"}</h1>
                <p>{"What works natively, what stays on a supported Signal device, and how selected text workflows can reach an ordinary SMS phone."}</p>
            </section>
            <section class="blog-content">
                <h2>{"The Short Answer"}</h2>
                <p>{"Most true feature phones cannot run the official Signal app. Lightfriend does not install Signal on the handset. It can provide selected Signal text workflows through ordinary SMS after the account is connected separately."}</p>
                <p>{"That route is intentionally narrower than the native app, and the final carrier-SMS leg is not Signal end-to-end encryption. Keep a supported setup and recovery device available."}</p>
                <p><a href="/blog/signal-without-smartphone">{"Read the detailed Signal options and privacy tradeoffs"}</a>{"."}</p>

                <h2>{"How It Works"}</h2>
                <p>{"Lightfriend acts as a bridge between Signal and your dumbphone's SMS:"}</p>
                <ol>
                    <li>{"Connect your Signal account through Lightfriend's web dashboard (one-time setup)"}</li>
                    <li>{"Lightfriend monitors your Signal conversations 24/7"}</li>
                    <li>{"Important messages are forwarded to your dumbphone as SMS"}</li>
                    <li>{"Reply by texting back - your response is sent through Signal to the recipient"}</li>
                    <li>{"Text anytime to check messages, send to contacts, or get a summary"}</li>
                </ol>

                <h2>{"Privacy: Dumbphone + Signal + Encrypted Enclave"}</h2>
                <p>{"People who use Signal care about privacy. So does Lightfriend."}</p>
                <p>{"Your Signal messages are relayed through Lightfriend's production application inside a hardware-isolated AWS Nitro Enclave. Stored application data is encrypted, and key release is handled by an independently operated service that evaluates enclave attestation."}</p>
                <p>{"This isn't a promise - it's a cryptographic proof. Anyone can verify what code is running inside the enclave at any time. The entire codebase is open source."}</p>
                <p>{"For the technical details: "}<Link<Route> to={Route::Trustless}>{"Review Lightfriend's privacy architecture"}</Link<Route>>{"."}</p>
                <p>{"The SMS leg between Lightfriend and your phone is standard carrier SMS (not encrypted end-to-end). If you need the full Signal encryption chain to stay intact, self-hosting Lightfriend is an option - the code is open source under AGPLv3."}</p>

                <h2>{"What You Can Do"}</h2>
                <ul>
                    <li><strong>{"Send messages"}</strong>{" - Text Lightfriend and it sends your message via Signal to the right person"}</li>
                    <li><strong>{"Receive messages"}</strong>{" - Important Signal messages arrive as SMS on your dumbphone"}</li>
                    <li><strong>{"Group conversations"}</strong>{" - Monitor Signal groups and get summaries"}</li>
                    <li><strong>{"On-demand check"}</strong>{" - Text anytime to see your recent Signal messages"}</li>
                    <li><strong>{"Scheduled digests"}</strong>{" - Get regular summaries of everything you missed"}</li>
                    <li><strong>{"Custom rules"}</strong>{" - Build WHEN/IF/THEN automations, like \"when my boss texts, always notify me immediately\""}</li>
                    <li><strong>{"Voice interface"}</strong>{" - Call Lightfriend to hear and dictate messages"}</li>
                </ul>

                <h2>{"Smart Filtering"}</h2>
                <p>{"You switched to a dumbphone to escape constant notifications. Lightfriend doesn't undo that by forwarding every message."}</p>
                <p>{"You build custom rules using WHEN/IF/THEN blocks. For example: when your best friend texts something urgent, notify immediately. When a group chat debates lunch plans, batch it into a digest. You control exactly what interrupts you and what waits."}</p>

                <h2>{"Compatible Phones"}</h2>
                <p>{"The intended route works with phones that have unrestricted two-way SMS and a supported Lightfriend number route:"}</p>
                <ul>
                    <li>{"Light Phone 2, Light Phone 3, and Light Flip"}</li>
                    <li>{"Nokia flip phones (2780, 2660, 2760)"}</li>
                    <li>{"Punkt MP02 (even though its native Signal client was discontinued)"}</li>
                    <li>{"Mudita Pure"}</li>
                    <li>{"Any basic phone, candy bar phone, or feature phone"}</li>
                </ul>

                <h2>{"Setup Guide"}</h2>
                <ol>
                    <li><strong>{"Sign up"}</strong>{" at lightfriend.ai from any computer or phone with a browser"}</li>
                    <li><strong>{"Connect Signal"}</strong>{" through the dashboard (takes a few minutes)"}</li>
                    <li><strong>{"Configure"}</strong>{" your notification preferences and custom rules"}</li>
                    <li><strong>{"Save Lightfriend's number"}</strong>{" in your dumbphone contacts"}</li>
                    <li><strong>{"Start texting"}</strong>{" - send and receive Signal messages via SMS"}</li>
                </ol>

                <h2>{"Signal on Dumbphone: With vs Without Lightfriend"}</h2>
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
                            <td>{"Send Signal messages"}</td>
                            <td>{"Not possible"}</td>
                            <td>{"Yes, via SMS or voice"}</td>
                        </tr>
                        <tr>
                            <td>{"Receive Signal messages"}</td>
                            <td>{"Not possible"}</td>
                            <td>{"Forwarded as SMS"}</td>
                        </tr>
                        <tr>
                            <td>{"Group chats"}</td>
                            <td>{"Not possible"}</td>
                            <td>{"Summaries and monitoring"}</td>
                        </tr>
                        <tr>
                            <td>{"Smart filtering"}</td>
                            <td>{"N/A"}</td>
                            <td>{"AI filters by importance"}</td>
                        </tr>
                        <tr>
                            <td>{"Server-side privacy"}</td>
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

                <h2>{"Frequently Asked Questions"}</h2>
                <p><strong>{"Q: Is this as secure as using Signal directly?"}</strong></p>
                <p>{"The Signal-to-Lightfriend leg uses Signal's encryption. Lightfriend processes the relayed message inside a hardware-isolated enclave. The final SMS leg to your phone uses standard carrier SMS. You can also self-host Lightfriend."}</p>
                <p><strong>{"Q: Do I need a smartphone for setup?"}</strong></p>
                <p>{"You need access to your Signal account for the initial connection. After that, everything works through your dumbphone's SMS."}</p>
                <p><strong>{"Q: What about Signal's disappearing messages?"}</strong></p>
                <p>{"Lightfriend respects Signal's disappearing message settings. Forwarded SMS summaries follow your configured message retention preferences."}</p>
                <p><strong>{"Q: Can I use Signal and Telegram and WhatsApp together?"}</strong></p>
                <p>{"Yes. Connect all three (plus email and calendar) and manage everything from one SMS number on your dumbphone."}</p>

                <h2>{"Also Works With"}</h2>
                <p>{"Lightfriend bridges all major messaging platforms to SMS:"}</p>
                <ul>
                    <li><Link<Route> to={Route::LightPhone3WhatsappGuide}>{"WhatsApp on dumbphone"}</Link<Route>></li>
                    <li><Link<Route> to={Route::TelegramOnDumbphone}>{"Telegram on dumbphone"}</Link<Route>></li>
                    <li>{"Email (any provider)"}</li>
                </ul>

                <div class="blog-cta">
                    <h3>{"Use Signal without carrying a smartphone"}</h3>
                    <p>{"Then verify your country route, setup device, privacy tradeoff, and ordinary SMS compatibility."}</p>
                    <a href="/#plans" data-fast-goal="blog_pricing_click" data-fast-goal-content-slug="signal-on-dumbphone" data-fast-goal-content-cluster="messaging">{"See plans"}</a>
                </div>
            </section>
            <style>
                {include_str!("blog_styles.css")}
            </style>
        </div>
    }
}
