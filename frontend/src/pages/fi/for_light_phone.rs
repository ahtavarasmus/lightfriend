use yew::prelude::*;
use crate::utils::seo::{use_seo, SeoMeta};

#[function_component(FiForLightPhone)]
pub fn fi_for_light_phone() -> Html {
    use_seo(SeoMeta {
        title: "Lightfriend Light Phonelle \u{2013} T\u{00e4}ydellinen Light Phone -kumppani",
        description: "Lightfriend on t\u{00e4}ydellinen kumppani Light Phone 2:lle ja Light Phone 3:lle. Saat WhatsAppin, Telegramin, s\u{00e4}hk\u{00f6}postin ja teko\u{00e4}lyhaun tekstiviestill\u{00e4}.",
        canonical: "https://lightfriend.ai/fi/for/light-phone",
        og_type: "website",
    });

    html! {
        <div class="audience-page">
            <div class="audience-hero">
                <h1>{"Lightfriend Light Phonelle"}</h1>
                <p class="audience-subtitle">{"T\u{00e4}ydellinen kumppani Light Phone 2:lle tai Light Phone 3:lle."}</p>
            </div>
            <div class="audience-content">
                <section>
                    <h2>{"Miksi Light Phone + Lightfriend?"}</h2>
                    <p>{"Light Phone on kauniisti minimalistinen. Lightfriend t\u{00e4}ytt\u{00e4}\u{00e4} puuttuvat palat lis\u{00e4}\u{00e4}m\u{00e4}tt\u{00e4} ruutuaikaa tai h\u{00e4}iri\u{00f6}tekij\u{00f6}it\u{00e4}. Kaikki toimii tekstiviestill\u{00e4} \u{2014} sovelluksia ei tarvitse asentaa."}</p>
                    <ul>
                        <li>{"Toimii sek\u{00e4} Light Phone 2:n (e-ink) ett\u{00e4} Light Phone 3:n kanssa"}</li>
                        <li>{"Ei sovelluksia asennettavaksi \u{2014} pelkk\u{00e4} SMS ja \u{00e4}\u{00e4}ni"}</li>
                        <li>{"S\u{00e4}ilytt\u{00e4}\u{00e4} minimalistisen k\u{00e4}ytt\u{00f6}kokemuksen"}</li>
                        <li>{"Lis\u{00e4}\u{00e4} toiminnallisuutta ilman h\u{00e4}iri\u{00f6}it\u{00e4}"}</li>
                    </ul>
                </section>
                <section>
                    <h2>{"Ominaisuudet Lightfriendin kautta"}</h2>
                    <p>{"Kaikki saapuu tekstiviestinä tai puheluna:"}</p>
                    <ul>
                        <li>{"WhatsApp-, Telegram- ja Signal-viestit"}</li>
                        <li>{"S\u{00e4}hk\u{00f6}posti \u{2014} vastaanota, lue ja vastaa"}</li>
                        <li>{"Kalenterimuistutukset ja p\u{00e4}iv\u{00e4}n katsaukset"}</li>
                        <li>{"Teko\u{00e4}lyhaku \u{2014} kysy mit\u{00e4} tahansa tekstiviestill\u{00e4}"}</li>
                        <li>{"GPS-navigointi \u{00e4}\u{00e4}niohjauksella"}</li>
                        <li>{"S\u{00e4}\u{00e4}, uutisyhteenvedot ja muuta"}</li>
                    </ul>
                </section>
                <section>
                    <h2>{"N\u{00e4}in p\u{00e4}\u{00e4}set alkuun"}</h2>
                    <ol>
                        <li>{"Rekister\u{00f6}idy Lightfriendiin tietokoneella"}</li>
                        <li>{"Lis\u{00e4}\u{00e4} Light Phonesi numero"}</li>
                        <li>{"Yhdist\u{00e4} viestisovellukset ja s\u{00e4}hk\u{00f6}postitilisi"}</li>
                        <li>{"Aloita viestiminen Lightfriendille \u{2014} se toimii heti"}</li>
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
