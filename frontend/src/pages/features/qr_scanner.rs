use yew::prelude::*;
use crate::utils::seo::{use_seo, SeoMeta};

#[function_component(QrScanner)]
pub fn qr_scanner() -> Html {
    use_seo(SeoMeta {
        title: "QR Code Scanner for Dumbphone – Scan QR Codes via MMS | Lightfriend",
        description: "Scan QR codes from any dumbphone via MMS. Take a photo of a QR code, send it to Lightfriend, and get the decoded content via SMS.",
        canonical: "https://lightfriend.ai/features/qr-scanner",
        og_type: "website",
    });

    html! {
        <div class="feature-page">
            <div class="feature-hero">
                <h1>{"QR Code Scanner for Dumbphone"}</h1>
                <p class="feature-subtitle">{"Scan QR codes from any phone — take a photo and send via MMS"}</p>
            </div>
            <div class="feature-content">
                <section>
                    <h2>{"How It Works"}</h2>
                    <p>{"QR codes are everywhere — restaurant menus, parking meters, event tickets. Most dumbphones can't scan them. With Lightfriend, take a photo of any QR code and send it as an MMS to your Lightfriend number."}</p>
                    <p>{"Lightfriend decodes the QR code and sends you the content via SMS. Whether it's a URL, text, contact info, or WiFi credentials — you'll get the decoded information as a text message."}</p>
                </section>
                <section>
                    <h2>{"Features"}</h2>
                    <ul>
                        <li>{"Scan any QR code via MMS photo"}</li>
                        <li>{"Decode URLs, text, and contact info"}</li>
                        <li>{"WiFi credential extraction"}</li>
                        <li>{"Restaurant menu and payment QR codes"}</li>
                        <li>{"AI-powered webpage summarization for URL codes"}</li>
                        <li>{"Works with any phone camera"}</li>
                    </ul>
                </section>
                <section>
                    <h2>{"Compatible Phones"}</h2>
                    <p>{"Works with Light Phone 2 & 3, Nokia flip phones, and any basic phone with a camera and MMS support."}</p>
                </section>
                <section class="feature-cta">
                    <a href="/register" class="cta-button">{"Get QR Scanning on Your Dumbphone"}</a>
                    <a href="/pricing" class="cta-link">{"View pricing →"}</a>
                </section>
            </div>
        </div>
    }
}
