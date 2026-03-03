use yew::prelude::*;
use crate::utils::seo::{use_seo, SeoMeta};

#[function_component(FiForDigitalDetox)]
pub fn fi_for_digital_detox() -> Html {
    use_seo(SeoMeta {
        title: "Digidetox Ilman Erist\u{00e4}ytymist\u{00e4} \u{2013} Lightfriend",
        description: "Tee digidetox menett\u{00e4}m\u{00e4}tt\u{00e4} viestint\u{00e4}\u{00e4}, s\u{00e4}hk\u{00f6}postia tai kalenteria. Lightfriend yhdist\u{00e4}\u{00e4} tyhm\u{00e4}puhelimesi tarvitsemiisi palveluihin.",
        canonical: "https://lightfriend.ai/fi/for/digital-detox",
        og_type: "website",
    });

    html! {
        <div class="audience-page">
            <div class="audience-hero">
                <h1>{"Digidetox ilman erist\u{00e4}ytymist\u{00e4}"}</h1>
                <p class="audience-subtitle">{"Luovu \u{00e4}lypuhelimesta. S\u{00e4}ilyt\u{00e4} viestint\u{00e4}."}</p>
            </div>
            <div class="audience-content">
                <section>
                    <h2>{"Mit\u{00e4} s\u{00e4}ilyt\u{00e4}t"}</h2>
                    <p>{"Tyhm\u{00e4}puhelimeen vaihtaminen ei tarkoita erist\u{00e4}ytymist\u{00e4}. Lightfriend v\u{00e4}litt\u{00e4}\u{00e4} olennaiset puhelimeesi tekstiviestill\u{00e4}:"}</p>
                    <ul>
                        <li>{"WhatsApp-, Telegram- ja Signal-viestit"}</li>
                        <li>{"S\u{00e4}hk\u{00f6}posti \u{2014} lue ja vastaa tekstiviestill\u{00e4}"}</li>
                        <li>{"Kalenterimuistutukset ja tapahtumayhteenvedot"}</li>
                        <li>{"Teko\u{00e4}lyhaku kun tarvitset vastauksia"}</li>
                        <li>{"GPS-navigointi \u{00e4}\u{00e4}nell\u{00e4} tai tekstiviestill\u{00e4}"}</li>
                    </ul>
                </section>
                <section>
                    <h2>{"Mist\u{00e4} luovut"}</h2>
                    <p>{"Asiat jotka veiv\u{00e4}t aikaasi ja huomiotasi:"}</p>
                    <ul>
                        <li>{"Sosiaalisen median sy\u{00f6}tteet ja loputon selailu"}</li>
                        <li>{"Push-ilmoitukset kymmenist\u{00e4} sovelluksista"}</li>
                        <li>{"Pakonomainen ruudun tarkistaminen"}</li>
                        <li>{"Y\u{00f6}llinen doomscrollailu"}</li>
                        <li>{"Sovellusten aiheuttama ahdistus ja FOMO"}</li>
                    </ul>
                </section>
                <section>
                    <h2>{"N\u{00e4}in aloitat detoxin"}</h2>
                    <ol>
                        <li>{"Valitse tyhm\u{00e4}puhelin \u{2014} Light Phone 3, Nokia-simpukka tai mik\u{00e4} tahansa peruspuhelin"}</li>
                        <li>{"Rekister\u{00f6}idy Lightfriendiin ja yhdist\u{00e4} viestisovelluksesi"}</li>
                        <li>{"Laita \u{00e4}lypuhelimesi laatikonpohjalle"}</li>
                        <li>{"Nauti rauhasta \u{2014} saat silti kaikki t\u{00e4}rke\u{00e4}t viestit"}</li>
                    </ol>
                </section>
                <section class="audience-cta">
                    <a href="/register" class="cta-button">{"Aloita detox"}</a>
                    <a href="/fi/pricing" class="cta-link">{"Katso hinnoittelu \u{2192}"}</a>
                </section>
            </div>
        </div>
    }
}
