use yew::prelude::*;
use crate::utils::seo::{use_seo, SeoMeta};

#[function_component(FiLanding)]
pub fn fi_landing() -> Html {
    use_seo(SeoMeta {
        title: "Lightfriend: Teko\u{00e4}lyavustaja Tyhm\u{00e4}puhelimille \u{2013} WhatsApp, Telegram, S\u{00e4}hk\u{00f6}posti SMS:ll\u{00e4}",
        description: "Teko\u{00e4}lyavustaja tyhm\u{00e4}puhelimille kuten Light Phone 3 ja Nokia-simpukat. K\u{00e4}yt\u{00e4} WhatsAppia, Telegramia, Signalia, s\u{00e4}hk\u{00f6}postia ja kalenteria tekstiviestill\u{00e4}.",
        canonical: "https://lightfriend.ai/fi",
        og_type: "website",
    });

    html! {
        <div class="feature-page">
            <div class="feature-hero">
                <h1>{"Lightfriend: Teko\u{00e4}lyavustaja Tyhm\u{00e4}puhelimille"}</h1>
                <p class="feature-subtitle">{"K\u{00e4}yt\u{00e4} WhatsAppia, Telegramia, s\u{00e4}hk\u{00f6}postia ja kalenteria millä tahansa puhelimella \u{2014} pelk\u{00e4}ll\u{00e4} tekstiviestill\u{00e4}."}</p>
            </div>
            <div class="feature-content">
                <section>
                    <h2>{"Miten se toimii?"}</h2>
                    <ol>
                        <li>{"Rekister\u{00f6}idy \u{2014} luo tili osoitteessa lightfriend.ai"}</li>
                        <li>{"L\u{00e4}het\u{00e4} tekstiviesti \u{2014} kirjoita viestisi ja l\u{00e4}het\u{00e4} se Lightfriend-numeroon"}</li>
                        <li>{"Teko\u{00e4}ly k\u{00e4}sittelee \u{2014} viestisi ohjataan oikeaan sovellukseen"}</li>
                        <li>{"Yhdist\u{00e4} sovellukset \u{2014} WhatsApp, Telegram, Signal, s\u{00e4}hk\u{00f6}posti, kalenteri ja muut"}</li>
                    </ol>
                </section>
                <section>
                    <h2>{"Ominaisuudet"}</h2>
                    <ul>
                        <li>{"WhatsApp, Telegram ja Signal tekstiviestill\u{00e4}"}</li>
                        <li>{"S\u{00e4}hk\u{00f6}posti \u{2014} lue ja vastaa Gmail- ja Outlook-viesteihin"}</li>
                        <li>{"Kalenterimuistutukset ja p\u{00e4}iv\u{00e4}n aikataulu"}</li>
                        <li>{"Teko\u{00e4}lyhaku \u{2014} kysy mit\u{00e4} tahansa tekstiviestill\u{00e4}"}</li>
                        <li>{"GPS-navigointi \u{00e4}\u{00e4}niohjauksella"}</li>
                        <li>{"Tesla-ohjaus tekstiviestill\u{00e4}"}</li>
                    </ul>
                </section>
                <section>
                    <h2>{"Hinnoittelu"}</h2>
                    <ul>
                        <li>{"Assistant \u{2014} 29\u{20ac}/kk: viestit, s\u{00e4}hk\u{00f6}posti, haku, kalenteri"}</li>
                        <li>{"Autopilot \u{2014} 49\u{20ac}/kk: kaikki yll\u{00e4} + jatkuva viestiseuranta ja yhteenvedot"}</li>
                    </ul>
                </section>
                <section>
                    <h2>{"Yhteensopivat puhelimet"}</h2>
                    <p>{"Toimii kaikkien puhelimien kanssa, jotka tukevat tekstiviestej\u{00e4}: Light Phone 2 ja 3, Nokia-simpukat, ja mik\u{00e4} tahansa peruspuhelin."}</p>
                </section>
                <section class="feature-cta">
                    <a href="/register" class="cta-button">{"Rekister\u{00f6}idy nyt"}</a>
                    <a href="/fi/pricing" class="cta-link">{"Katso hinnoittelu \u{2192}"}</a>
                </section>
            </div>
        </div>
    }
}
