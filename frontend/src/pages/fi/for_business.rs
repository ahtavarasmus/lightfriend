use yew::prelude::*;
use crate::utils::seo::{use_seo, SeoMeta};

#[function_component(FiForBusiness)]
pub fn fi_for_business() -> Html {
    use_seo(SeoMeta {
        title: "Tyhm\u{00e4}puhelin Ty\u{00f6}k\u{00e4}ytt\u{00f6}\u{00f6}n \u{2013} Ammattilaisen Minimalistinen Puhelin | Lightfriend",
        description: "K\u{00e4}yt\u{00e4} tyhm\u{00e4}puhelinta t\u{00f6}iss\u{00e4} menett\u{00e4}m\u{00e4}tt\u{00e4} s\u{00e4}hk\u{00f6}postia, kalenteria tai viestint\u{00e4}\u{00e4}. Lightfriend toimittaa ty\u{00f6}n kannalta olennaiset palvelut tekstiviestill\u{00e4}.",
        canonical: "https://lightfriend.ai/fi/for/business",
        og_type: "website",
    });

    html! {
        <div class="audience-page">
            <div class="audience-hero">
                <h1>{"Tyhm\u{00e4}puhelin ty\u{00f6}k\u{00e4}ytt\u{00f6}\u{00f6}n"}</h1>
                <p class="audience-subtitle">{"Pysy tuottavana t\u{00f6}iss\u{00e4}. J\u{00e4}t\u{00e4} h\u{00e4}iri\u{00f6}tekij\u{00e4}t taakse."}</p>
            </div>
            <div class="audience-content">
                <section>
                    <h2>{"Miksi ammattilaiset valitsevat tyhm\u{00e4}puhelimen"}</h2>
                    <p>{"\u{00c4}lypuhelimet tappavat tuottavuuden. Ilmoitukset, sosiaalinen media ja loputtomat sovellukset pirstovat huomiosi koko ty\u{00f6}p\u{00e4}iv\u{00e4}n. Tyhm\u{00e4}puhelin Lightfriendin kanssa antaa sinulle keskittymisrauhan ja pit\u{00e4}\u{00e4} sinut silti tavoitettavissa."}</p>
                    <ul>
                        <li>{"Syv\u{00e4}ty\u{00f6} mahdollistuu \u{2014} ei sovellusilmoituksia"}</li>
                        <li>{"Palaverit pysyv\u{00e4}t keskittynein\u{00e4} \u{2014} ei puhelimia tarkistettavana"}</li>
                        <li>{"Asiakaspuhelut tulevat silti l\u{00e4}pi \u{2014} se on edelleen puhelin"}</li>
                        <li>{"S\u{00e4}hk\u{00f6}posti ja kalenteri pysyv\u{00e4}t saatavilla SMS:n kautta"}</li>
                    </ul>
                </section>
                <section>
                    <h2>{"Ty\u{00f6}ominaisuudet tekstiviestill\u{00e4}"}</h2>
                    <p>{"Lightfriend toimittaa ammattilaisten tarvitsemat ty\u{00f6}kalut:"}</p>
                    <ul>
                        <li>{"S\u{00e4}hk\u{00f6}posti \u{2014} vastaanota, lue ja vastaa ty\u{00f6}s\u{00e4}hk\u{00f6}posteihin"}</li>
                        <li>{"Kalenteri \u{2014} kokousmuistutukset ja p\u{00e4}iv\u{00e4}n aikataulu"}</li>
                        <li>{"WhatsApp ja Telegram \u{2014} pysy tiimi- ja asiakaskeskusteluissa"}</li>
                        <li>{"Teko\u{00e4}lyhaku \u{2014} nopeat vastaukset ilman selainta"}</li>
                        <li>{"GPS-navigointi \u{2014} p\u{00e4}\u{00e4}se tapaamisiin ajoissa"}</li>
                    </ul>
                </section>
                <section>
                    <h2>{"N\u{00e4}in p\u{00e4}\u{00e4}set alkuun"}</h2>
                    <ol>
                        <li>{"Valitse tyhm\u{00e4}puhelin \u{2014} Light Phone 3 tyyliin, Nokia kest\u{00e4}vyyteen"}</li>
                        <li>{"Tilaa Lightfriend Autopilot (49\u{20ac}/kk)"}</li>
                        <li>{"Yhdist\u{00e4} ty\u{00f6}s\u{00e4}hk\u{00f6}posti ja kalenteri"}</li>
                        <li>{"Lis\u{00e4}\u{00e4} WhatsApp tai Telegram tiimiviestint\u{00e4}\u{00e4} varten"}</li>
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
