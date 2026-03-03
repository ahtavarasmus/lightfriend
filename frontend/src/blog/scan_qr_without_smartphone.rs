use yew::prelude::*;
use yew_router::components::Link;
use crate::Route;
use crate::utils::seo::{use_seo, SeoMeta};

#[function_component(ScanQrWithoutSmartphone)]
pub fn scan_qr_without_smartphone() -> Html {
    use_seo(SeoMeta {
        title: "How to Scan QR Codes Without a Smartphone",
        description: "Solutions for scanning QR codes when you use a dumbphone. Lightfriend MMS method, browser extensions, and workarounds for restaurant menus and payments.",
        canonical: "https://lightfriend.ai/blog/scan-qr-without-smartphone",
        og_type: "article",
    });
    {
        use_effect_with_deps(
            move |_| {
                if let Some(window) = web_sys::window() {
                    window.scroll_to_with_x_and_y(0.0, 0.0);
                }
                || ()
            },
            (),
        );
    }
    html! {
        <div class="blog-page">
            <div class="blog-background"></div>
            <section class="blog-hero">
                <h1>{"How to Scan QR Codes Without a Smartphone"}</h1>
                <p>{"QR codes are everywhere. Here is how to handle them with a dumbphone."}</p>
            </section>
            <section class="blog-content">
                <h2>{"The QR Code Problem"}</h2>
                <p>{"QR codes have exploded since 2020. Restaurant menus, parking meters, event tickets, Wi-Fi logins, payment systems, and product information all rely on QR codes. For smartphone users, scanning is effortless. For dumbphone users, it can feel like the world has moved on without you."}</p>
                <p>{"But there are solutions. Here is every method available to handle QR codes when you do not have a smartphone camera app."}</p>

                <h2>{"Method 1: Lightfriend MMS Photo Method"}</h2>
                <p>{"If your dumbphone has a camera (many modern ones do, including the Light Phone 3 and Nokia flip phones), take a photo of the QR code and send it via MMS to your Lightfriend number. Lightfriend\u{2019}s AI reads the QR code, decodes it, and texts you back the URL or content."}</p>
                <ol>
                    <li>{"Take a photo of the QR code with your dumbphone\u{2019}s camera."}</li>
                    <li>{"Send the photo as an MMS to your Lightfriend number."}</li>
                    <li>{"Lightfriend decodes the QR code and replies with the URL or content."}</li>
                    <li>{"If it is a URL, Lightfriend can also summarize the linked page for you."}</li>
                </ol>

                <h2>{"Method 2: Ask for Alternatives"}</h2>
                <p>{"Most QR codes have a non-QR alternative. At restaurants, ask for a paper menu \u{2013} they are legally required to have one in many jurisdictions. For parking, look for a machine or phone number option. For payments, ask if they accept cash or card. Do not be shy about asking; you are not the only person without a smartphone scanner handy."}</p>

                <h2>{"Method 3: Manual URL Entry"}</h2>
                <p>{"Many QR codes have the URL printed nearby in small text. If you are near a computer or your dumbphone has a basic browser, you can type the URL manually. Not elegant, but it works."}</p>

                <h2>{"Method 4: Laptop or Tablet"}</h2>
                <p>{"If you carry a laptop, you can use webcam-based QR code reader websites. Open your browser, navigate to a QR reader site, allow camera access, and point your webcam at the code. This is impractical for quick scans but works in a pinch."}</p>

                <h2>{"Common QR Code Scenarios"}</h2>
                <ul>
                    <li><strong>{"Restaurant menus:"}</strong>{" Ask for a physical menu. Restaurants are always required to have them."}</li>
                    <li><strong>{"Parking meters:"}</strong>{" Look for a coin slot, card reader, or phone number on the meter."}</li>
                    <li><strong>{"Event tickets:"}</strong>{" Print your ticket at home or ask the venue for a printed copy."}</li>
                    <li><strong>{"Wi-Fi login:"}</strong>{" Ask staff for the password and enter it manually."}</li>
                    <li><strong>{"Product info:"}</strong>{" Search for the product name on your computer later, or snap a photo and send to Lightfriend."}</li>
                    <li><strong>{"Payments:"}</strong>{" Use cash or card instead."}</li>
                </ul>

                <h2>{"The Reality"}</h2>
                <p>{"QR codes are an inconvenience for dumbphone users, not a dealbreaker. With Lightfriend\u{2019}s MMS QR decoding and a willingness to ask for alternatives, you can navigate a QR-code world just fine. The minor friction is a small price for the massive benefits of a distraction-free life."}</p>

                <div class="blog-cta">
                    <h3>{"Handle QR Codes from Any Phone"}</h3>
                    <Link<Route> to={Route::Pricing} classes="forward-link">
                        <button class="hero-cta">{"Get Started with Lightfriend"}</button>
                    </Link<Route>>
                </div>
            </section>
            <style>
                {r#"
                .blog-page { padding-top: 74px; min-height: 100vh; color: #ffffff; position: relative; background: transparent; }
                .blog-background { position: fixed; top: 0; left: 0; width: 100%; height: 100vh; background-image: url('/assets/field_asthetic_not.webp'); background-size: cover; background-position: center; background-repeat: no-repeat; opacity: 1; z-index: -2; pointer-events: none; }
                .blog-background::after { content: ''; position: absolute; bottom: 0; left: 0; width: 100%; height: 50%; background: linear-gradient(to bottom, rgba(26, 26, 26, 0) 0%, rgba(26, 26, 26, 1) 100%); }
                .blog-hero { text-align: center; padding: 6rem 2rem; background: rgba(26, 26, 26, 0.75); backdrop-filter: blur(5px); margin-top: 2rem; border: 1px solid rgba(30, 144, 255, 0.1); margin-bottom: 2rem; }
                .blog-hero h1 { font-size: 3.5rem; margin-bottom: 1.5rem; background: linear-gradient(45deg, #fff, #7EB2FF); -webkit-background-clip: text; -webkit-text-fill-color: transparent; }
                .blog-hero p { font-size: 1.2rem; color: #999; max-width: 600px; margin: 0 auto; }
                .blog-content { max-width: 800px; margin: 0 auto; padding: 2rem; }
                .blog-content h2 { font-size: 2.5rem; margin: 3rem 0 1rem; background: linear-gradient(45deg, #fff, #7EB2FF); -webkit-background-clip: text; -webkit-text-fill-color: transparent; }
                .blog-content p { color: #999; line-height: 1.6; margin-bottom: 1.5rem; }
                .blog-content ul, .blog-content ol { color: #999; padding-left: 1.5rem; margin-bottom: 1.5rem; }
                .blog-content li { margin-bottom: 0.75rem; }
                .blog-content a { color: #7EB2FF; text-decoration: none; border-bottom: 1px solid rgba(126, 178, 255, 0.3); transition: all 0.3s ease; font-weight: 500; }
                .blog-content a:hover { color: #ffffff; border-bottom-color: #7EB2FF; text-shadow: 0 0 5px rgba(126, 178, 255, 0.5); }
                .blog-cta { text-align: center; margin: 4rem 0 2rem; padding: 2rem; background: rgba(30, 144, 255, 0.1); border-radius: 12px; }
                .blog-cta h3 { font-size: 2rem; margin-bottom: 1.5rem; background: linear-gradient(45deg, #fff, #7EB2FF); -webkit-background-clip: text; -webkit-text-fill-color: transparent; }
                .hero-cta { background: linear-gradient(45deg, #7EB2FF, #4169E1); color: white; border: none; padding: 1rem 2.5rem; border-radius: 8px; font-size: 1.1rem; cursor: pointer; transition: all 0.3s ease; }
                .hero-cta:hover { transform: translateY(-2px); box-shadow: 0 4px 20px rgba(126, 178, 255, 0.4); }
                @media (max-width: 768px) { .blog-hero { padding: 4rem 1rem; } .blog-hero h1 { font-size: 2.5rem; } .blog-content { padding: 1rem; } .blog-content h2 { font-size: 2rem; } }
                "#}
            </style>
        </div>
    }
}
