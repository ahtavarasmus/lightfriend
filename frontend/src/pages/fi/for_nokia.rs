use yew::prelude::*;
use crate::utils::seo::{use_seo, SeoMeta};

#[function_component(FiForNokia)]
pub fn fi_for_nokia() -> Html {
    use_seo(SeoMeta {
        title: "Lightfriend Nokia-puhelimille \u{2013} \u{00c4}lyominaisuudet Nokia-simpukkaan",
        description: "K\u{00e4}yt\u{00e4} Lightfriendia Nokia 2780 Flip-, Nokia 2760 Flip-, Nokia 225- ja muiden Nokia-puhelimien kanssa. Saat WhatsAppin, s\u{00e4}hk\u{00f6}postin ja teko\u{00e4}lyn tekstiviestill\u{00e4}.",
        canonical: "https://lightfriend.ai/fi/for/nokia",
        og_type: "website",
    });

    html! {
        <div class="audience-page">
            <div class="audience-hero">
                <h1>{"Lightfriend Nokia-puhelimille"}</h1>
                <p class="audience-subtitle">{"Muuta mik\u{00e4} tahansa Nokia-simpukka \u{00e4}lykk\u{00e4}\u{00e4}ksi viestint\u{00e4}laitteeksi."}</p>
            </div>
            <div class="audience-content">
                <section>
                    <h2>{"Yhteensopivat Nokia-mallit"}</h2>
                    <p>{"Lightfriend toimii mink\u{00e4} tahansa Nokian kanssa, joka tukee tekstiviestej\u{00e4}:"}</p>
                    <ul>
                        <li>{"Nokia 2780 Flip"}</li>
                        <li>{"Nokia 2760 Flip"}</li>
                        <li>{"Nokia 225 4G"}</li>
                        <li>{"Nokia 105 / Nokia 110"}</li>
                        <li>{"Mik\u{00e4} tahansa muu Nokia-peruspuhelin SMS-tuella"}</li>
                    </ul>
                </section>
                <section>
                    <h2>{"Mit\u{00e4} Lightfriend lis\u{00e4}\u{00e4} Nokiaasi"}</h2>
                    <p>{"Nokiasi hoitaa puhelut ja tekstiviestit. Lightfriend hoitaa kaiken muun:"}</p>
                    <ul>
                        <li>{"WhatsApp, Telegram ja Signal \u{2014} l\u{00e4}hetet\u{00e4}\u{00e4}n ja vastaanotetaan tekstiviesteinä"}</li>
                        <li>{"S\u{00e4}hk\u{00f6}posti \u{2014} lue ja vastaa tekstiviestill\u{00e4}"}</li>
                        <li>{"Kalenterimuistutukset tekstiviestill\u{00e4}"}</li>
                        <li>{"Teko\u{00e4}lyhaku \u{2014} l\u{00e4}het\u{00e4} kysymys, saat vastauksen"}</li>
                        <li>{"GPS-reittiohje \u{00e4}\u{00e4}nipuhelulla"}</li>
                    </ul>
                </section>
                <section>
                    <h2>{"K\u{00e4}ytt\u{00f6}\u{00f6}notto 5 minuutissa"}</h2>
                    <ol>
                        <li>{"Rekister\u{00f6}idy osoitteessa lightfriend.ai tietokoneella tai tabletilla"}</li>
                        <li>{"Sy\u{00f6}t\u{00e4} Nokiasi puhelinnumero"}</li>
                        <li>{"Yhdist\u{00e4} WhatsApp, s\u{00e4}hk\u{00f6}posti tai muut palvelut"}</li>
                        <li>{"L\u{00e4}het\u{00e4} tekstiviesti Lightfriendille Nokiastasi"}</li>
                    </ol>
                </section>
                <section class="audience-cta">
                    <a href="/register" class="cta-button">{"Aloita nyt"}</a>
                    <a href="/fi/pricing" class="cta-link">{"Katso hinnoittelu \u{2192}"}</a>
                </section>
            </div>
        </div>
    }
}
