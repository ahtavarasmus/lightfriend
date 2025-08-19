use yew::prelude::*;
use yew_router::components::Link;
use crate::Route;

#[function_component(SwitchToDumbphoneGuide)]
pub fn switch_to_dumbphone_guide() -> Html {
    // Scroll to top only on initial mount
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
                <h1>{"How to Switch to a Dumbphone"}</h1>
                <p>{"Learn how to transition to a dumbphone for a distraction-free life."}</p>
                <img src="/assets/lightphone2.png" alt="Light Phone 2" loading="lazy" class="blog-image" />
            </section>
            <section class="blog-content">
                <h2>{"Introduction: Your Computer is Your Best Friend"}</h2>
                <p>{"The point is not to go full jail mode. You still need two factor authentication and probably check that group chat also. We want to block the endless algorithms, not work or friends:)."}</p>
                <p>{"Apps you may want on your computer: "}</p>
                <ul>
                    <li>
                        <a href="https://beeper.com" target="_blank">{"Beeper.com"}</a>{" so you can keep chatting like you used to, but now across platforms in one app!"}
                    </li>
                    <li>
                        <a href="https://steptwo.app" target="_blank">{"Step Two App (MacOS)"}</a>{" for handling two factor authentication codes on a Macbook"}
                    </li>
                    <li>
                        <a href="https://freetubeapp.io" target="_blank">{"FreeTube"}</a>{" is a YouTube video player with only subscription feed"}
                    </li>
                    <li>
                        <a href="https://getcoldturkey.com" target="_blank">{"Cold Turkey App Blocker"}</a>{" can be set to block any website or app on your computer completely, for a specific time or however you like. The blocks can be locked with a password which you can give to a trusted person. You cannot unblock or uninstall anything without that password. You can set temporary allowances if you really need to check some blocked app for certain duration. If you bring lightfriend AI along your dumbphone journey, you get 20% discount on the Pro version."}
                    </li>
                </ul>
                <h2>{"Dumbphones and Where to Buy"}</h2>
                <p>{"Dumbphones are basic feature phones without app stores, social media, or endless notifications. Popular options include "}<a href="https://thelightphone.com" target="_blank">{"The Light Phone, "}</a><a href="https://mudita.com/products/phones/mudita-kompakt" target="_blank">{"Mudita Kompakt, "}</a><a href="https://www.punkt.ch/en/products/mp02-4g-mobile-phone" target="_blank">{"Punkt MP02"}</a>{" or if you want you can go full retro and buy old Nokia from "}<a href="https://vintagemobile.fr/en/collections/nokia" target="_blank">{"vintagemobile.fr"}</a>{". "}<a href="https://dumbphones.org" target="_blank">{"Dumbphones.org"}</a>{" is a great resource for browsing different dumbphones. Don't sweat it too much, as long as it doesn't have an app store it should be fine. My personal recommendation would be to get one that has a hotspot so you can share wifi to your computer. At least The Light Phone and Mudita Kompakt have a hotspot, but if your phone doesn't, you may want to buy a portable wifi module to bring with or just rely on public wifis depending on the country you live in."}</p>
                <h2>{"Lightfriend AI"}</h2>
                <p>{"Lightfriend.ai is your assistant that you can call or text from your dumbphone to get access to your digital life. It monitors emails, messages, and calendar, forwarding only what's urgent to your dumbphone."}</p>
                <h2>{"YubiKey and Where to Buy"}</h2>
                <p>{"YubiKey is a hardware security key for two-factor authentication (2FA) for services that require authentication apps like Microsoft Authenticator normally. It's essential for being able to login to certain services where software 2FA codes aren't allowed. Purchase from the official Yubico website (yubico.com) or authorized resellers like Amazon."}</p>
                <h2>{"Bank authentication"}</h2>
                <p>{"Many banks require authentication apps for logging in, but they usually offer physical code calculators or security token devices as alternatives. These small devices generate one-time codes for logging into your bank account. Contact your bank to request a physical authentication device - they typically provide these free of charge or for a small fee. Some banks may call these devices 'key fobs', 'code calculators', or 'security tokens'."}</p>
                <h2>{"Transportation"}</h2>
                <p>{"Transportation solutions vary by country, but there are several options available. Many cities offer physical keycards for public transit. For ride-hailing, Lightfriend will soon integrate with Uber and similar services. Alternative platforms like tremp.me offer ride-sharing possibilities. For navigation, modern dumbphones like The Light Phone and Mudita Kompakt include built-in maps. Additionally, Lightfriend can provide step-by-step directions through voice calls or SMS when you need guidance."}</p>
                <h2>{"Benefits of Switching to Dumbphone"}</h2>
                <p>{"Reclaim your time and attention:"}</p>
                <ul>
                    <li>{"Reduced distractions, more life and better mental health"}</li>
                    <li>{"Computer handles heavy lifting seamlessly"}</li>
                </ul>
                <h2>{"Step-by-Step Guide to Switching"}</h2>
                <ol>
                    <li>{"Choose and buy your dumbphone"}</li>
                    <li>{"Set up 2FA to your computer and YubiKey on yubico.com"}</li>
                    <li>{"Sign up for Beeper and connect messaging services"}</li>
                    <li>{"Sign up for Lightfriend and connect your digital life"}</li>
                    <li>{"Install Cold Turkey on your computer and configure blocks"}</li>
                    <li>{"Transfer contacts and start living!"}</li>
                </ol>
                <div class="blog-cta">
                    <h3>{"Ready to Switch to a Dumbphone?"}</h3>
                    <Link<Route> to={Route::Pricing} classes="forward-link">
                        <button class="hero-cta">{"Get Started with Lightfriend"}</button>
                    </Link<Route>>
                </div>
            </section>
            <style>
                {r#"
                .blog-page {
                    padding-top: 74px;
                    min-height: 100vh;
                    color: #ffffff;
                    position: relative;
                    background: transparent;
                }
                .blog-background {
                    position: fixed;
                    top: 0;
                    left: 0;
                    width: 100%;
                    height: 100vh;
                    background-image: url('/assets/field_asthetic_not.webp');
                    background-size: cover;
                    background-position: center;
                    background-repeat: no-repeat;
                    opacity: 1;
                    z-index: -2;
                    pointer-events: none;
                }
                .blog-background::after {
                    content: '';
                    position: absolute;
                    bottom: 0;
                    left: 0;
                    width: 100%;
                    height: 50%;
                    background: linear-gradient(
                        to bottom,
                        rgba(26, 26, 26, 0) 0%,
                        rgba(26, 26, 26, 1) 100%
                    );
                }
                .blog-hero {
                    text-align: center;
                    padding: 6rem 2rem;
                    background: rgba(26, 26, 26, 0.75);
                    backdrop-filter: blur(5px);
                    margin-top: 2rem;
                    border: 1px solid rgba(30, 144, 255, 0.1);
                    margin-bottom: 2rem;
                }
                .blog-hero h1 {
                    font-size: 3.5rem;
                    margin-bottom: 1.5rem;
                    background: linear-gradient(45deg, #fff, #7EB2FF);
                    -webkit-background-clip: text;
                    -webkit-text-fill-color: transparent;
                }
                .blog-hero p {
                    font-size: 1.2rem;
                    color: #999;
                    max-width: 600px;
                    margin: 0 auto;
                }
                .blog-content {
                    max-width: 800px;
                    margin: 0 auto;
                    padding: 2rem;
                }
                .blog-content h2 {
                    font-size: 2.5rem;
                    margin: 3rem 0 1rem;
                    background: linear-gradient(45deg, #fff, #7EB2FF);
                    -webkit-background-clip: text;
                    -webkit-text-fill-color: transparent;
                }
                .blog-content p {
                    color: #999;
                    line-height: 1.6;
                    margin-bottom: 1.5rem;
                }
                .blog-content ul, .blog-content ol {
                    color: #999;
                    padding-left: 1.5rem;
                    margin-bottom: 1.5rem;
                }
                .blog-content li {
                    margin-bottom: 0.75rem;
                }
                .blog-content a {
                    color: #7EB2FF;
                    text-decoration: none;
                    border-bottom: 1px solid rgba(126, 178, 255, 0.3);
                    transition: all 0.3s ease;
                    font-weight: 500;
                }
                .blog-content a:hover {
                    color: #ffffff;
                    border-bottom-color: #7EB2FF;
                    text-shadow: 0 0 5px rgba(126, 178, 255, 0.5);
                }
                .blog-image {
                    max-width: 100%;
                    height: auto;
                    border-radius: 12px;
                    margin: 2rem 0;
                    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.3);
                }
                .comparison-table {
                    width: 100%;
                    border-collapse: collapse;
                    margin: 2rem 0;
                    color: #ddd;
                }
                .comparison-table th, .comparison-table td {
                    padding: 1rem;
                    border: 1px solid rgba(126, 178, 255, 0.2);
                    text-align: left;
                }
                .comparison-table th {
                    background: rgba(0, 0, 0, 0.5);
                    color: #7EB2FF;
                }
                .blog-cta {
                    text-align: center;
                    margin: 4rem 0 2rem;
                    padding: 2rem;
                    background: rgba(30, 144, 255, 0.1);
                    border-radius: 12px;
                }
                .blog-cta h3 {
                    font-size: 2rem;
                    margin-bottom: 1.5rem;
                    background: linear-gradient(45deg, #fff, #7EB2FF);
                    -webkit-background-clip: text;
                    -webkit-text-fill-color: transparent;
                }
                .blog-cta p {
                    color: #999;
                    margin-top: 1rem;
                }
                .hero-cta {
                    background: linear-gradient(45deg, #7EB2FF, #4169E1);
                    color: white;
                    border: none;
                    padding: 1rem 2.5rem;
                    border-radius: 8px;
                    font-size: 1.1rem;
                    cursor: pointer;
                    transition: all 0.3s ease;
                }
                .hero-cta:hover {
                    transform: translateY(-2px);
                    box-shadow: 0 4px 20px rgba(126, 178, 255, 0.4);
                }
                @media (max-width: 768px) {
                    .blog-hero {
                        padding: 4rem 1rem;
                    }
                    .blog-hero h1 {
                        font-size: 2.5rem;
                    }
                    .blog-content {
                        padding: 1rem;
                    }
                    .blog-content h2 {
                        font-size: 2rem;
                    }
                    .comparison-table th, .comparison-table td {
                        padding: 0.75rem;
                        font-size: 0.9rem;
                    }
                }
                "#}
            </style>
        </div>
    }
}
