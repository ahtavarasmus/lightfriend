use yew::prelude::*;
use crate::utils::seo::{use_seo, SeoMeta};

#[function_component(FiForParents)]
pub fn fi_for_parents() -> Html {
    use_seo(SeoMeta {
        title: "Turvallinen Ensipuhelin Lapselle \u{2013} Tyhm\u{00e4}puhelin + Lightfriend",
        description: "Anna lapsellesi turvallinen ensipuhelin. Tyhm\u{00e4}puhelin Lightfriendin kanssa tarjoaa viestinn\u{00e4}n ilman sosiaalista mediaa, selaimia tai sovelluskauppoja.",
        canonical: "https://lightfriend.ai/fi/for/parents",
        og_type: "website",
    });

    html! {
        <div class="audience-page">
            <div class="audience-hero">
                <h1>{"Turvallinen ensipuhelin lapselle"}</h1>
                <p class="audience-subtitle">{"Tyhm\u{00e4}puhelin pit\u{00e4}\u{00e4} lapset turvassa. Lightfriend pit\u{00e4}\u{00e4} heid\u{00e4}t yhteydess\u{00e4}."}</p>
            </div>
            <div class="audience-content">
                <section>
                    <h2>{"Miksi tyhm\u{00e4}puhelin on turvallisempi"}</h2>
                    <p>{"\u{00c4}lypuhelimet altistavat lapset riskeille, joita mik\u{00e4}\u{00e4}n lapsilukko-sovellus ei t\u{00e4}ysin ratkaise. Tyhm\u{00e4}puhelin poistaa n\u{00e4}m\u{00e4} riskit laitteen tasolla:"}</p>
                    <ul>
                        <li>{"Ei sosiaalista mediaa \u{2014} ei kiusaamista, ei vertailukulttuuria"}</li>
                        <li>{"Ei selainta \u{2014} ei sopimatonta sis\u{00e4}lt\u{00f6}\u{00e4}"}</li>
                        <li>{"Ei sovelluskauppaa \u{2014} ei koukuttavia pelej\u{00e4} tai sovellus\u{00f6}stoja"}</li>
                        <li>{"Ei kameraa painetta \u{2014} ei kuvanjakoa tai sextingriskin\u{00e4}"}</li>
                        <li>{"Vain puhelut ja viestit \u{2014} perusasiat kunnossa"}</li>
                    </ul>
                </section>
                <section>
                    <h2>{"Mit\u{00e4} Lightfriend lis\u{00e4}\u{00e4} lapselle"}</h2>
                    <p>{"Lapsesi saa hy\u{00f6}dyllisi\u{00e4} ominaisuuksia ilman vaaroja:"}</p>
                    <ul>
                        <li>{"WhatsApp- ja Telegram-viestit perheen ja kavereiden kanssa"}</li>
                        <li>{"Teko\u{00e4}lyavusteinen l\u{00e4}ksyapu tekstiviestill\u{00e4}"}</li>
                        <li>{"Kalenterimuistutukset koulua ja harrastuksia varten"}</li>
                        <li>{"GPS-reittiohje kun pit\u{00e4}\u{00e4} l\u{00f6}yt\u{00e4}\u{00e4} perille"}</li>
                        <li>{"Kaikki viestint\u{00e4} kulkee SMS:n kautta \u{2014} helppo seurata"}</li>
                    </ul>
                </section>
                <section>
                    <h2>{"Suositeltu k\u{00e4}ytt\u{00f6}\u{00f6}notto perheille"}</h2>
                    <ol>
                        <li>{"Hanki lapselle Nokia-simpukka tai Light Phone"}</li>
                        <li>{"Rekister\u{00f6}idy Lightfriendiin ja lis\u{00e4}\u{00e4} lapsen numero"}</li>
                        <li>{"Yhdist\u{00e4} perheen WhatsApp tai ryhm\u{00e4}keskustelut"}</li>
                        <li>{"Lapsesi pysyy tavoitettavissa ilman \u{00e4}lypuhelimen riskej\u{00e4}"}</li>
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
