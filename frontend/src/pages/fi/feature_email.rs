use yew::prelude::*;
use crate::utils::seo::{use_seo, SeoMeta};

#[function_component(FiFeatureEmail)]
pub fn fi_feature_email() -> Html {
    use_seo(SeoMeta {
        title: "S\u{00e4}hk\u{00f6}posti Tyhm\u{00e4}puhelimella \u{2013} Gmail ja Outlook SMS:ll\u{00e4} | Lightfriend",
        description: "Lue ja vastaa Gmail- ja Outlook-s\u{00e4}hk\u{00f6}posteihin millä tahansa tyhm\u{00e4}puhelimella tekstiviestill\u{00e4}. S\u{00e4}hk\u{00f6}posti ilman \u{00e4}lypuhelinta Lightfriendin avulla.",
        canonical: "https://lightfriend.ai/fi/features/email-sms",
        og_type: "website",
    });

    html! {
        <div class="feature-page">
            <div class="feature-hero">
                <h1>{"S\u{00e4}hk\u{00f6}posti tekstiviestill\u{00e4}"}</h1>
                <p class="feature-subtitle">{"Lue ja vastaa Gmail- ja Outlook-viesteihin mill\u{00e4} tahansa puhelimella \u{2014} SMS:n kautta"}</p>
            </div>
            <div class="feature-content">
                <section>
                    <h2>{"Miten se toimii?"}</h2>
                    <p>{"S\u{00e4}hk\u{00f6}posti on v\u{00e4}ltt\u{00e4}m\u{00e4}t\u{00f6}n, mutta yleens\u{00e4} se vaatii \u{00e4}lypuhelimen tai tietokoneen. Lightfriendin avulla voit lukea ja vastata Gmail- ja Outlook-s\u{00e4}hk\u{00f6}posteihin tekstiviestill\u{00e4} mill\u{00e4} tahansa puhelimella \u{2014} internetyhteyttä ei tarvita."}</p>
                    <p>{"Lightfriend seuraa postilaatikkoasi ja v\u{00e4}litt\u{00e4}\u{00e4} t\u{00e4}rke\u{00e4}t s\u{00e4}hk\u{00f6}postit tekstiviesteinä. Vastaa l\u{00e4}hett\u{00e4}m\u{00e4}ll\u{00e4} tekstiviesti, ja vastauksesi l\u{00e4}hetet\u{00e4}\u{00e4}n s\u{00e4}hk\u{00f6}postina omasta tililt\u{00e4}si."}</p>
                </section>
                <section>
                    <h2>{"Ominaisuudet"}</h2>
                    <ul>
                        <li>{"Lue saapuvat s\u{00e4}hk\u{00f6}postit tekstiviesteinä"}</li>
                        <li>{"Vastaa s\u{00e4}hk\u{00f6}posteihin tekstiviestill\u{00e4}"}</li>
                        <li>{"Gmail- ja Outlook-tuki"}</li>
                        <li>{"Teko\u{00e4}lyn tuottamat s\u{00e4}hk\u{00f6}postiyhteenvedot"}</li>
                        <li>{"T\u{00e4}rkeiden viestien priorisointi"}</li>
                        <li>{"24/7-seuranta Autopilot-tilauksella"}</li>
                    </ul>
                </section>
                <section>
                    <h2>{"Yhteensopivat puhelimet"}</h2>
                    <p>{"Toimii Light Phone 2:n ja 3:n, Nokia-simpukoiden ja mink\u{00e4} tahansa peruspuhelimen kanssa \u{2014} riitt\u{00e4}\u{00e4}, ett\u{00e4} puhelin tukee SMS:n l\u{00e4}hett\u{00e4}mist\u{00e4}."}</p>
                </section>
                <section class="feature-cta">
                    <a href="/register" class="cta-button">{"Hanki s\u{00e4}hk\u{00f6}posti tyhm\u{00e4}puhelimeesi"}</a>
                    <a href="/fi/pricing" class="cta-link">{"Katso hinnoittelu \u{2192}"}</a>
                </section>
            </div>
        </div>
    }
}
