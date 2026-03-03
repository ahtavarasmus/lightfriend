use yew::prelude::*;
use crate::utils::seo::{use_seo, SeoMeta};

#[function_component(FiFeatureWhatsapp)]
pub fn fi_feature_whatsapp() -> Html {
    use_seo(SeoMeta {
        title: "WhatsApp Ilman \u{00c4}lypuhelinta \u{2013} WhatsApp Tyhm\u{00e4}puhelimella | Lightfriend",
        description: "K\u{00e4}yt\u{00e4} WhatsAppia millä tahansa tyhm\u{00e4}puhelimella tai simpukkapuhelimella SMS:n kautta. L\u{00e4}het\u{00e4} ja vastaanota WhatsApp-viestej\u{00e4} Light Phonella, Nokialla tai mill\u{00e4} tahansa peruspuhelimella.",
        canonical: "https://lightfriend.ai/fi/features/whatsapp-dumbphone",
        og_type: "website",
    });

    html! {
        <div class="feature-page">
            <div class="feature-hero">
                <h1>{"WhatsApp tyhm\u{00e4}puhelimella"}</h1>
                <p class="feature-subtitle">{"K\u{00e4}yt\u{00e4} WhatsAppia ilman \u{00e4}lypuhelinta \u{2014} l\u{00e4}het\u{00e4} ja vastaanota viestej\u{00e4} tekstiviestill\u{00e4}"}</p>
            </div>
            <div class="feature-content">
                <section>
                    <h2>{"Miten se toimii?"}</h2>
                    <p>{"WhatsApp on maailman suosituin viestisovellus yli kahdella miljardilla k\u{00e4}ytt\u{00e4}j\u{00e4}ll\u{00e4}. Lightfriendin avulla voit l\u{00e4}hett\u{00e4}\u{00e4} ja vastaanottaa WhatsApp-viestej\u{00e4} tekstiviestill\u{00e4} mill\u{00e4} tahansa puhelimella \u{2014} \u{00e4}lypuhelinta ei tarvita."}</p>
                    <p>{"Lightfriend yhdist\u{00e4}\u{00e4} WhatsAppin ja SMS:n. Saapuvat WhatsApp-viestit v\u{00e4}litet\u{00e4}\u{00e4}n sinulle tekstiviestein\u{00e4}. Vastaa l\u{00e4}hett\u{00e4}m\u{00e4}ll\u{00e4} tekstiviesti Lightfriendille, ja viestisi menee perille WhatsAppissa."}</p>
                </section>
                <section>
                    <h2>{"Ominaisuudet"}</h2>
                    <ul>
                        <li>{"L\u{00e4}het\u{00e4} viestej\u{00e4} kenelle tahansa WhatsApp-kontaktille"}</li>
                        <li>{"Vastaanota saapuvat WhatsApp-viestit tekstiviestein\u{00e4}"}</li>
                        <li>{"Saa ilmoitukset t\u{00e4}rkeist\u{00e4} viesteist\u{00e4}"}</li>
                        <li>{"P\u{00e4}\u{00e4}sy WhatsApp-ryhmiin"}</li>
                        <li>{"24/7-seuranta Autopilot-tilauksella"}</li>
                        <li>{"P\u{00e4}ivitt\u{00e4}iset yhteenvedot"}</li>
                    </ul>
                </section>
                <section>
                    <h2>{"Yhteensopivat puhelimet"}</h2>
                    <p>{"Toimii Light Phone 2:n ja 3:n, Nokia-simpukoiden ja mink\u{00e4} tahansa peruspuhelimen kanssa \u{2014} riitt\u{00e4}\u{00e4}, ett\u{00e4} puhelin tukee SMS:n l\u{00e4}hett\u{00e4}mist\u{00e4}."}</p>
                </section>
                <section class="feature-cta">
                    <a href="/register" class="cta-button">{"Hanki WhatsApp tyhm\u{00e4}puhelimeesi"}</a>
                    <a href="/fi/pricing" class="cta-link">{"Katso hinnoittelu \u{2192}"}</a>
                </section>
            </div>
        </div>
    }
}
