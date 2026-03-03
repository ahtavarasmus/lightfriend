use yew::prelude::*;
use crate::utils::seo::{use_seo, SeoMeta};

#[function_component(FiFeatureCalendar)]
pub fn fi_feature_calendar() -> Html {
    use_seo(SeoMeta {
        title: "Kalenteri Tyhm\u{00e4}puhelimella \u{2013} Google-kalenteri SMS:ll\u{00e4} | Lightfriend",
        description: "K\u{00e4}yt\u{00e4} Google-kalenteria millä tahansa tyhm\u{00e4}puhelimella SMS:n kautta. Saa muistutukset, tarkista aikataulu ja hallitse tapaamisia tekstiviestill\u{00e4}.",
        canonical: "https://lightfriend.ai/fi/features/calendar-sms",
        og_type: "website",
    });

    html! {
        <div class="feature-page">
            <div class="feature-hero">
                <h1>{"Google-kalenteri tyhm\u{00e4}puhelimella"}</h1>
                <p class="feature-subtitle">{"Saa kalenterimuistutukset ja tarkista aikataulusi tekstiviestill\u{00e4} \u{2014} ilman \u{00e4}lypuhelinta"}</p>
            </div>
            <div class="feature-content">
                <section>
                    <h2>{"Miten se toimii?"}</h2>
                    <p>{"Aikataulun hallinta ei vaadi \u{00e4}lypuhelinta. Lightfriendin avulla k\u{00e4}yt\u{00e4}t Google-kalenteria tekstiviestill\u{00e4} \u{2014} saat muistutukset, tarkistat tulevat tapahtumat etkä miss\u{00e4}\u{00e4} yht\u{00e4}\u{00e4}n tapaamista."}</p>
                    <p>{"Lightfriend yhdist\u{00e4}\u{00e4} Google-kalenterisi ja l\u{00e4}hett\u{00e4}\u{00e4} SMS-muistutukset ennen tapahtumia. Kysy aikataulustasi tekstiviestill\u{00e4}, niin saat heti vastauksen tulevista tapahtumistasi."}</p>
                </section>
                <section>
                    <h2>{"Ominaisuudet"}</h2>
                    <ul>
                        <li>{"Automaattiset SMS-muistutukset ennen tapahtumia"}</li>
                        <li>{"Tarkista p\u{00e4}iv\u{00e4}n ja viikon aikataulu tekstiviestill\u{00e4}"}</li>
                        <li>{"Tapahtuman tiedot: aika, paikka ja osallistujat"}</li>
                        <li>{"Aamuinen p\u{00e4}iv\u{00e4}n yhteenveto"}</li>
                        <li>{"Usean kalenterin tuki"}</li>
                        <li>{"S\u{00e4}\u{00e4}dett\u{00e4}v\u{00e4} muistutusaika"}</li>
                    </ul>
                </section>
                <section>
                    <h2>{"Yhteensopivat puhelimet"}</h2>
                    <p>{"Toimii Light Phone 2:n ja 3:n, Nokia-simpukoiden ja mink\u{00e4} tahansa peruspuhelimen kanssa \u{2014} riitt\u{00e4}\u{00e4}, ett\u{00e4} puhelin tukee SMS:n l\u{00e4}hett\u{00e4}mist\u{00e4}."}</p>
                </section>
                <section class="feature-cta">
                    <a href="/register" class="cta-button">{"Hanki kalenteri tyhm\u{00e4}puhelimeesi"}</a>
                    <a href="/fi/pricing" class="cta-link">{"Katso hinnoittelu \u{2192}"}</a>
                </section>
            </div>
        </div>
    }
}
