use yew::prelude::*;
use crate::utils::seo::{use_seo, SeoMeta};

#[function_component(FiFeatureTesla)]
pub fn fi_feature_tesla() -> Html {
    use_seo(SeoMeta {
        title: "Tesla-ohjaus Tekstiviestill\u{00e4} \u{2013} Ohjaa Teslaa Ilman \u{00c4}lypuhelinta | Lightfriend",
        description: "Ohjaa Teslaasi millä tahansa puhelimella tekstiviestill\u{00e4}. Lukitse, avaa, tarkista akku ja s\u{00e4}\u{00e4}d\u{00e4} ilmastointia Light Phonella, Nokialla tai mill\u{00e4} tahansa peruspuhelimella.",
        canonical: "https://lightfriend.ai/fi/features/tesla-sms",
        og_type: "website",
    });

    html! {
        <div class="feature-page">
            <div class="feature-hero">
                <h1>{"Tesla-ohjaus tekstiviestill\u{00e4}"}</h1>
                <p class="feature-subtitle">{"Ohjaa Teslaasi mill\u{00e4} tahansa puhelimella \u{2014} lukitse, avaa ja seuraa tekstiviestill\u{00e4}"}</p>
            </div>
            <div class="feature-content">
                <section>
                    <h2>{"Miten se toimii?"}</h2>
                    <p>{"Tesla-sovellus vaatii \u{00e4}lypuhelimen, mutta Lightfriendin avulla voit ohjata Teslaasi tekstiviestill\u{00e4} mill\u{00e4} tahansa puhelimella. Lukitse ja avaa ovet, tarkista akun varaus, esis\u{00e4}\u{00e4}d\u{00e4} ilmastointi ja paljon muuta \u{2014} kaikki tekstiviestill\u{00e4}."}</p>
                    <p>{"Lightfriend yhdist\u{00e4}\u{00e4} Tesla-rajapintaan ja muuntaa tekstiviestikomentosi auton toiminnoiksi. L\u{00e4}het\u{00e4} komento kuten \"avaa auto\" tai \"paljonko akkua on j\u{00e4}ljell\u{00e4}\" ja saat vastauksen heti."}</p>
                </section>
                <section>
                    <h2>{"Ominaisuudet"}</h2>
                    <ul>
                        <li>{"Lukitse ja avaa ovet tekstiviestill\u{00e4}"}</li>
                        <li>{"Tarkista akun varaus ja toimintamatka"}</li>
                        <li>{"Esis\u{00e4}\u{00e4}d\u{00e4} ilmastointi"}</li>
                        <li>{"Tarkista auton sijainti"}</li>
                        <li>{"Avaa ja sulje tavaratila"}</li>
                        <li>{"Luonnollisen kielen komennot"}</li>
                    </ul>
                </section>
                <section>
                    <h2>{"Yhteensopivat puhelimet"}</h2>
                    <p>{"Toimii Light Phone 2:n ja 3:n, Nokia-simpukoiden ja mink\u{00e4} tahansa peruspuhelimen kanssa \u{2014} riitt\u{00e4}\u{00e4}, ett\u{00e4} puhelin tukee SMS:n l\u{00e4}hett\u{00e4}mist\u{00e4}."}</p>
                </section>
                <section class="feature-cta">
                    <a href="/register" class="cta-button">{"Ohjaa Teslaasi tekstiviestill\u{00e4}"}</a>
                    <a href="/fi/pricing" class="cta-link">{"Katso hinnoittelu \u{2192}"}</a>
                </section>
            </div>
        </div>
    }
}
