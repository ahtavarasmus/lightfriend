use yew::prelude::*;
use crate::utils::seo::{use_seo, SeoMeta};

#[function_component(FiPricing)]
pub fn fi_pricing() -> Html {
    use_seo(SeoMeta {
        title: "Hinnoittelu \u{2013} Lightfriend Teko\u{00e4}lyavustaja",
        description: "Lightfriend-hinnoittelu: Assistant 29\u{20ac}/kk, Autopilot 49\u{20ac}/kk, BYOT 19\u{20ac}/kk. Teko\u{00e4}lyavustaja tyhm\u{00e4}puhelimille.",
        canonical: "https://lightfriend.ai/fi/pricing",
        og_type: "website",
    });

    html! {
        <div class="feature-page">
            <div class="feature-hero">
                <h1>{"Hinnoittelu"}</h1>
                <p class="feature-subtitle">{"Valitse sinulle sopiva tilaus. Kaikki tilaukset sis\u{00e4}lt\u{00e4}v\u{00e4}t 7 p\u{00e4}iv\u{00e4}n ilmaisen kokeilujakson."}</p>
            </div>
            <div class="feature-content">
                <section>
                    <h2>{"Assistant \u{2014} 29\u{20ac}/kk"}</h2>
                    <p>{"Kaikki tarvitsemasi perusominaisuudet:"}</p>
                    <ul>
                        <li>{"WhatsApp, Telegram ja Signal tekstiviestill\u{00e4}"}</li>
                        <li>{"S\u{00e4}hk\u{00f6}postin luku ja vastaus"}</li>
                        <li>{"Google-kalenterimuistutukset"}</li>
                        <li>{"Teko\u{00e4}lyhaku tekstiviestill\u{00e4}"}</li>
                        <li>{"GPS-navigointi"}</li>
                        <li>{"Tesla-ohjaus"}</li>
                    </ul>
                </section>
                <section>
                    <h2>{"Autopilot \u{2014} 49\u{20ac}/kk"}</h2>
                    <p>{"Kaikki Assistant-ominaisuudet sek\u{00e4}:"}</p>
                    <ul>
                        <li>{"Jatkuva viestiseuranta \u{2014} t\u{00e4}rke\u{00e4}t viestit v\u{00e4}litet\u{00e4}\u{00e4}n heti"}</li>
                        <li>{"P\u{00e4}ivitt\u{00e4}iset yhteenvedot viesteist\u{00e4} ja s\u{00e4}hk\u{00f6}posteista"}</li>
                        <li>{"Automaattiset vastaukset poissaollessa"}</li>
                        <li>{"Ennakoivat kalenterimuistutukset"}</li>
                        <li>{"Lis\u{00e4}\u{00e4} viestikiinti\u{00f6}t\u{00e4}"}</li>
                    </ul>
                </section>
                <section>
                    <h2>{"BYOT (Bring Your Own Twilio) \u{2014} 19\u{20ac}/kk"}</h2>
                    <p>{"Teknisille k\u{00e4}ytt\u{00e4}jille, jotka haluavat k\u{00e4}ytt\u{00e4}\u{00e4} omaa Twilio-tili\u{00e4}\u{00e4}n:"}</p>
                    <ul>
                        <li>{"Kaikki Assistant-ominaisuudet"}</li>
                        <li>{"Maksat vain k\u{00e4}ytt\u{00e4}m\u{00e4}si tekstiviestit suoraan Twiliolle"}</li>
                        <li>{"T\u{00e4}ysi hallinta numeroon ja kustannuksiin"}</li>
                    </ul>
                </section>
                <section class="feature-cta">
                    <a href="/register" class="cta-button">{"Aloita ilmainen kokeilu"}</a>
                    <a href="/fi" class="cta-link">{"Takaisin etusivulle \u{2192}"}</a>
                </section>
            </div>
        </div>
    }
}
