use yew::prelude::*;
use crate::utils::seo::{use_seo, SeoMeta};

#[function_component(ForNokia)]
pub fn for_nokia() -> Html {
    use_seo(SeoMeta {
        title: "Lightfriend for Nokia \u{2013} AI Assistant for Nokia Flip Phones | Lightfriend",
        description: "Use Lightfriend with Nokia 2780 Flip, Nokia 2760 Flip, Nokia 225, and other Nokia feature phones. Get WhatsApp, email, and AI via SMS.",
        canonical: "https://lightfriend.ai/for/nokia",
        og_type: "website",
    });

    html! {
        <div class="audience-page">
            <div class="audience-hero">
                <h1>{"Lightfriend for Nokia"}</h1>
                <p class="audience-subtitle">{"Turn any Nokia flip phone into a smart communication device."}</p>
            </div>
            <div class="audience-content">
                <section>
                    <h2>{"Compatible Nokia Models"}</h2>
                    <p>{"Lightfriend works with any Nokia phone that can send and receive SMS:"}</p>
                    <ul>
                        <li>{"Nokia 2780 Flip"}</li>
                        <li>{"Nokia 2760 Flip"}</li>
                        <li>{"Nokia 225 4G"}</li>
                        <li>{"Nokia 105 / Nokia 110"}</li>
                        <li>{"Any other Nokia feature phone with SMS support"}</li>
                    </ul>
                </section>
                <section>
                    <h2>{"What Lightfriend Adds to Your Nokia"}</h2>
                    <p>{"Your Nokia handles calls and texts. Lightfriend handles everything else:"}</p>
                    <ul>
                        <li>{"WhatsApp, Telegram, and Signal \u{2014} sent and received as SMS"}</li>
                        <li>{"Email access \u{2014} read and reply by text"}</li>
                        <li>{"Calendar reminders delivered via SMS"}</li>
                        <li>{"AI web search \u{2014} text a question, get an answer"}</li>
                        <li>{"GPS directions via voice call"}</li>
                    </ul>
                </section>
                <section>
                    <h2>{"Setup in 5 Minutes"}</h2>
                    <ol>
                        <li>{"Sign up at lightfriend.ai from any computer or tablet"}</li>
                        <li>{"Enter your Nokia\u{2019}s phone number"}</li>
                        <li>{"Connect WhatsApp, email, or other services"}</li>
                        <li>{"Text Lightfriend from your Nokia to start"}</li>
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
