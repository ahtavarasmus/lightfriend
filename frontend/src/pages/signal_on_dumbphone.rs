use crate::utils::seo::{use_seo, SeoMeta};
use crate::Route;
use yew::prelude::*;
use yew_router::components::Link;

#[function_component(SignalOnDumbphone)]
pub fn signal_on_dumbphone() -> Html {
    use_seo(SeoMeta {
        title: "Signal on Dumbphone - How to Use Signal on Any Flip Phone or Basic Phone",
        description: "Use Signal encrypted messaging on any dumbphone or flip phone via SMS. Lightfriend monitors your Signal chats and forwards important messages as texts. No apps needed.",
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
                <h1>{"How to Use Signal on a Dumbphone"}</h1>
                <p>{"Send and receive Signal messages from any basic phone, flip phone, or minimalist phone - without installing anything."}</p>
            </section>
            <section class="blog-content">
                <h2>{"Signal on a Dumbphone: It's Possible"}</h2>
                <p>{"Signal has no dumbphone app. The Punkt MP02 used to support it, but that was discontinued. If you've switched to a minimalist phone for privacy or to break phone addiction, you've probably accepted that Signal is off the table. It doesn't have to be."}</p>
                <p>{"Lightfriend connects to your Signal account and bridges your messages to SMS. You send and receive Signal messages as regular texts from any phone - no apps, no internet on your phone, no workarounds."}</p>

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
                <p>{"Your Signal messages are relayed through Lightfriend's servers, but those servers run inside a hardware-isolated AWS Nitro Enclave. This is a sealed computing environment - nobody can access it, not the cloud provider, not the developer. Encryption keys only exist inside the enclave and are managed by an independent attested service."}</p>
                <p>{"This isn't a promise - it's a cryptographic proof. Anyone can verify what code is running inside the enclave at any time. The entire codebase is open source."}</p>
                <p>{"For the technical details: "}<Link<Route> to={Route::Trustless}>{"How Lightfriend keeps your data private"}</Link<Route>>{"."}</p>
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
                <p>{"Any phone with SMS works:"}</p>
                <ul>
                    <li>{"Light Phone 2 and Light Phone 3"}</li>
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
                <p>{"The Signal-to-Lightfriend leg maintains Signal's encryption. The Lightfriend server runs in a hardware-isolated enclave that nobody can access. The final SMS leg to your phone uses standard carrier SMS. For most people this is more than sufficient. For maximum security, you can self-host Lightfriend."}</p>
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
                    <h3>{"Get Signal on Your Dumbphone"}</h3>
                    <Link<Route> to={Route::Pricing} classes="forward-link">
                        <button class="hero-cta">{"See Plans"}</button>
                    </Link<Route>>
                    <p>{"Works with any phone that can send a text message."}</p>
                </div>
            </section>
            <style>
                {include_str!("blog_styles.css")}
            </style>
        </div>
    }
}
