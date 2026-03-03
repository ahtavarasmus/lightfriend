use yew::prelude::*;
use crate::utils::seo::{use_seo, SeoMeta};

#[function_component(FiForAdhd)]
pub fn fi_for_adhd() -> Html {
    use_seo(SeoMeta {
        title: "Paras Puhelin ADHD:lle \u{2013} Tyhm\u{00e4}puhelin + Teko\u{00e4}lyavustaja | Lightfriend",
        description: "Paras puhelinratkaisu ADHD:lle. Tyhm\u{00e4}puhelin Lightfriendin kanssa poistaa h\u{00e4}iri\u{00f6}tekij\u{00e4}t ja pit\u{00e4}\u{00e4} t\u{00e4}rke\u{00e4}n viestinn\u{00e4}n toiminnassa.",
        canonical: "https://lightfriend.ai/fi/for/adhd",
        og_type: "website",
    });

    html! {
        <div class="audience-page">
            <div class="audience-hero">
                <h1>{"Paras puhelin ADHD:lle"}</h1>
                <p class="audience-subtitle">{"Tyhm\u{00e4}puhelin poistaa h\u{00e4}iri\u{00f6}tekij\u{00e4}t. Lightfriend pit\u{00e4}\u{00e4} sinut yhteydess\u{00e4}."}</p>
            </div>
            <div class="audience-content">
                <section>
                    <h2>{"Miksi tyhm\u{00e4}puhelin auttaa ADHD:ss\u{00e4}?"}</h2>
                    <p>{"\u{00c4}lypuhelimet jatkuvine ilmoituksineen, loputtomalla selailulla ja sovellusten v\u{00e4}lisell\u{00e4} vaihtamisella on suunniteltu kaappaamaan huomio \u{2014} juuri se asia, jota ADHD:n kanssa on vaikea hallita. Tyhm\u{00e4}puhelin poistaa n\u{00e4}m\u{00e4} \u{00e4}rsykkeet kokonaan."}</p>
                    <ul>
                        <li>{"Ei loputonta selailua \u{2014} sosiaalinen media ja uutissy\u{00f6}tteet ovat poissa"}</li>
                        <li>{"Ei impulsiivista tarkistamista \u{2014} ei ole mit\u{00e4}\u{00e4}n pakonomaisesti tarkistettavaa"}</li>
                        <li>{"V\u{00e4}hemm\u{00e4}n kontekstin vaihtoa \u{2014} vain puhelut ja tekstiviestit"}</li>
                        <li>{"Parempi uni \u{2014} ei sinisen valon kaninkuoppia"}</li>
                        <li>{"Parempi keskittyminen \u{2014} puhelimesi lakkaa olemasta h\u{00e4}iri\u{00f6}tekij\u{00e4}"}</li>
                    </ul>
                </section>
                <section>
                    <h2>{"Ent\u{00e4} viestit, s\u{00e4}hk\u{00f6}posti ja kalenteri?"}</h2>
                    <p>{"Lightfriend hoitaa tarvitsemasi digipalvelut:"}</p>
                    <ul>
                        <li>{"WhatsApp, Telegram ja Signal tekstiviestill\u{00e4}"}</li>
                        <li>{"S\u{00e4}hk\u{00f6}postin luku ja vastaus"}</li>
                        <li>{"Kalenterimuistutukset"}</li>
                        <li>{"Teko\u{00e4}lyhaku"}</li>
                    </ul>
                </section>
                <section>
                    <h2>{"Suositeltu k\u{00e4}ytt\u{00f6}\u{00f6}notto"}</h2>
                    <ol>
                        <li>{"Hanki Light Phone 3 tai Nokia-simpukka"}</li>
                        <li>{"Tilaa Lightfriend Autopilot (49\u{20ac}/kk)"}</li>
                        <li>{"Yhdist\u{00e4} viestisovellukset ja s\u{00e4}hk\u{00f6}posti"}</li>
                        <li>{"K\u{00e4}yt\u{00e4} tietokonetta ruututeht\u{00e4}viin sivustonestojen kanssa"}</li>
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
