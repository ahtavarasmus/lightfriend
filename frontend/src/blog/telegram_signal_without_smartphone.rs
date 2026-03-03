use yew::prelude::*;
use yew_router::components::Link;
use crate::Route;
use crate::utils::seo::{use_seo, SeoMeta};

#[function_component(TelegramSignalWithoutSmartphone)]
pub fn telegram_signal_without_smartphone() -> Html {
    use_seo(SeoMeta {
        title: "How to Use Telegram and Signal Without a Smartphone",
        description: "Complete guide to using Telegram and Signal without a smartphone. Desktop apps, Lightfriend SMS bridge, and workarounds for dumbphone users.",
        canonical: "https://lightfriend.ai/blog/telegram-signal-without-smartphone",
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
                <h1>{"How to Use Telegram and Signal Without a Smartphone"}</h1>
                <p>{"Keep your secure and private messaging without carrying a smartphone."}</p>
            </section>
            <section class="blog-content">
                <h2>{"Telegram and Signal: Privacy Favorites"}</h2>
                <p>{"Telegram and Signal are the go-to messaging apps for privacy-conscious users. Telegram offers large groups, channels, and bots, while Signal provides the gold standard in end-to-end encryption. Both are popular among tech-savvy users \u{2013} exactly the kind of people who might want to switch to a dumbphone. But can you use them without a smartphone?"}</p>

                <h2>{"Telegram Without a Smartphone"}</h2>
                <h2>{"Desktop App"}</h2>
                <p>{"Telegram is the most dumbphone-friendly messaging app because it has a fully independent desktop client. Unlike WhatsApp, Telegram Desktop does not require a phone to stay connected."}</p>
                <ol>
                    <li>{"Download Telegram Desktop from desktop.telegram.org."}</li>
                    <li>{"Log in with your phone number \u{2013} you will receive an SMS code (works on dumbphones)."}</li>
                    <li>{"Use Telegram on your computer independently. No phone needed to stay logged in."}</li>
                </ol>
                <p>{"This makes Telegram the easiest platform to keep using after switching to a dumbphone."}</p>

                <h2>{"Lightfriend SMS Bridge for Telegram"}</h2>
                <p>{"For receiving Telegram messages on your dumbphone while away from your computer, Lightfriend bridges Telegram to SMS."}</p>
                <ul>
                    <li>{"Connect your Telegram account in the Lightfriend dashboard."}</li>
                    <li>{"Incoming Telegram messages are forwarded as SMS to your dumbphone."}</li>
                    <li>{"Reply via SMS and your message is sent back through Telegram."}</li>
                    <li>{"Choose which chats to forward: all, favorites only, or specific contacts."}</li>
                </ul>

                <h2>{"Signal Without a Smartphone"}</h2>
                <h2>{"Signal Desktop"}</h2>
                <p>{"Signal Desktop works as a linked device. You need a smartphone to set it up initially, but after linking, it works independently for messaging."}</p>
                <ol>
                    <li>{"Borrow a smartphone temporarily and install Signal."}</li>
                    <li>{"Register your dumbphone number with Signal (verification SMS goes to your dumbphone)."}</li>
                    <li>{"Link Signal Desktop by scanning the QR code."}</li>
                    <li>{"Return the borrowed smartphone. Signal Desktop stays linked."}</li>
                </ol>
                <p>{"Note: Signal Desktop needs to be re-linked if it is inactive for an extended period. You may need to borrow a smartphone again for re-linking."}</p>

                <h2>{"Lightfriend SMS Bridge for Signal"}</h2>
                <p>{"Lightfriend can also bridge Signal messages to SMS, similar to the Telegram bridge. This gives you Signal access on your dumbphone without any smartphone or computer nearby."}</p>
                <ul>
                    <li>{"Connect Signal in the Lightfriend dashboard."}</li>
                    <li>{"Receive and reply to Signal messages via SMS."}</li>
                    <li>{"Your messages are still encrypted between Lightfriend and Signal\u{2019}s servers."}</li>
                </ul>

                <h2>{"Best Strategy"}</h2>
                <p>{"Use "}<strong>{"Telegram Desktop"}</strong>{" and "}<strong>{"Signal Desktop"}</strong>{" when at your computer, and "}<strong>{"Lightfriend"}</strong>{" to bridge both to SMS when you are on the go. This gives you full access to both platforms at your desk, and essential messaging access from your dumbphone anywhere."}</p>
                <p>{"Telegram is significantly easier to maintain without a smartphone thanks to its independent desktop client. Signal requires more occasional maintenance but is absolutely doable with the borrowed-phone method and Lightfriend bridging."}</p>

                <div class="blog-cta">
                    <h3>{"Keep Telegram and Signal on Any Phone"}</h3>
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
