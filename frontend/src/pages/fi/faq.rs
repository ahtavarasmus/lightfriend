use yew::prelude::*;
use crate::utils::seo::{use_seo, SeoMeta};

#[function_component(FiFaq)]
pub fn fi_faq() -> Html {
    use_seo(SeoMeta {
        title: "UKK \u{2013} Lightfriend Teko\u{00e4}lyavustaja Tyhm\u{00e4}puhelimille",
        description: "Usein kysytyt kysymykset Lightfriend-teko\u{00e4}lyavustajasta tyhm\u{00e4}puhelimille. Mik\u{00e4} on Lightfriend, mitkä puhelimet toimivat ja paljonko se maksaa.",
        canonical: "https://lightfriend.ai/fi/faq",
        og_type: "website",
    });

    html! {
        <div class="feature-page">
            <div class="feature-hero">
                <h1>{"Usein kysytyt kysymykset"}</h1>
                <p class="feature-subtitle">{"Kaikki mit\u{00e4} sinun tarvitsee tiet\u{00e4}\u{00e4} Lightfriendist\u{00e4}."}</p>
            </div>
            <div class="feature-content">
                <section>
                    <h2>{"Mik\u{00e4} on Lightfriend?"}</h2>
                    <p>{"Lightfriend on teko\u{00e4}lyavustaja, joka toimii tekstiviestill\u{00e4}. Se yhdist\u{00e4}\u{00e4} tyhm\u{00e4}puhelimesi WhatsAppiin, Telegramiin, Signaliin, s\u{00e4}hk\u{00f6}postiin, kalenteriin ja muihin palveluihin \u{2014} ilman \u{00e4}lypuhelinta tai internetyhteyttä."}</p>
                </section>
                <section>
                    <h2>{"Mitk\u{00e4} puhelimet toimivat?"}</h2>
                    <p>{"Mik\u{00e4} tahansa puhelin, joka voi l\u{00e4}hett\u{00e4}\u{00e4} ja vastaanottaa tekstiviestej\u{00e4}. Suosittuja vaihtoehtoja ovat:"}</p>
                    <ul>
                        <li>{"Light Phone 2 ja Light Phone 3"}</li>
                        <li>{"Nokia-simpukat (2780 Flip, 2760 Flip, 225 4G)"}</li>
                        <li>{"Nokia 105 ja Nokia 110"}</li>
                        <li>{"Mik\u{00e4} tahansa peruspuhelin, jossa on SMS-tuki"}</li>
                    </ul>
                </section>
                <section>
                    <h2>{"Paljonko se maksaa?"}</h2>
                    <p>{"Lightfriendill\u{00e4} on kolme tilausta:"}</p>
                    <ul>
                        <li>{"Assistant \u{2014} 29\u{20ac}/kk: viestit, s\u{00e4}hk\u{00f6}posti, haku, kalenteri"}</li>
                        <li>{"Autopilot \u{2014} 49\u{20ac}/kk: kaikki edell\u{00e4} + jatkuva seuranta ja yhteenvedot"}</li>
                        <li>{"BYOT \u{2014} 19\u{20ac}/kk: omalla Twilio-tilill\u{00e4}"}</li>
                    </ul>
                    <p>{"Kaikki tilaukset sis\u{00e4}lt\u{00e4}v\u{00e4}t 7 p\u{00e4}iv\u{00e4}n ilmaisen kokeilujakson."}</p>
                </section>
                <section>
                    <h2>{"Miss\u{00e4} maissa Lightfriend toimii?"}</h2>
                    <p>{"Lightfriend toimii maailmanlaajuisesti. Paikallisia puhelinnumeroita on saatavilla Suomessa, Yhdysvalloissa, Kanadassa, Alankomaissa, Iso-Britanniassa ja Australiassa. Muissa maissa viestit l\u{00e4}hetet\u{00e4}\u{00e4}n yhdysvaltalaisesta numerosta."}</p>
                </section>
                <section>
                    <h2>{"Miten tietoni suojataan?"}</h2>
                    <p>{"Kaikki arkaluonteiset tiedot salataan AES-256-GCM-salauksella. Emme myy tai jaa tietojasi. Viestej\u{00e4} s\u{00e4}ilytet\u{00e4}\u{00e4}n vain niin kauan kuin on tarpeen niiden v\u{00e4}litt\u{00e4}miseksi, ja ne poistetaan automaattisesti."}</p>
                </section>
                <section class="feature-cta">
                    <a href="/register" class="cta-button">{"Rekister\u{00f6}idy nyt"}</a>
                    <a href="/fi" class="cta-link">{"Takaisin etusivulle \u{2192}"}</a>
                </section>
            </div>
        </div>
    }
}
