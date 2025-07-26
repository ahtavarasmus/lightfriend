use yew::prelude::*;
use crate::auth::connect::Connect;
use wasm_bindgen::prelude::*;
use web_sys::{Element, HtmlElement, HtmlElement as HtmlElementTrait};
use yew_router::prelude::*;
use crate::Route;
use yew_router::components::Link;
use crate::config;
use web_sys::{window, HtmlInputElement};
use gloo_net::http::Request;
use serde::{Deserialize, Serialize};
use serde_json::json;
use wasm_bindgen_futures::spawn_local;
use gloo_timers::callback::Timeout;

#[function_component(Landing)]
pub fn landing() -> Html {
    let current_phone_word = use_state(|| 0);

    // Scroll to top only on initial mount
    {
        use_effect_with_deps(
            move |_| {
                if let Some(window) = web_sys::window() {
                    window.scroll_to_with_x_and_y(0.0, 0.0);
                }
                || ()
            },
            (), // Empty dependencies array means this effect runs only once on mount
        );
    }

    html! {
        <div class="landing-page">
        <header class="hero">
                <div class="hero-background"></div>
                <div class="hero-content">
                    <div class="hero-header">
                        <p class="hero-subtitle">
                            {"Ditch Smartphone Addiction Without Willpower, Even If Isolation Scares You"}
                        </p>
                    </div>
                    <div class="hero-cta-group">
                        <Link<Route> to={Route::Pricing} classes="forward-link">
                            <button class="hero-cta">{"Get Started"}</button>
                        </Link<Route>>
                        <a href="/faq#try-service" class="faq-link">
                            {"Try demo first"}
                        </a>
                    </div>
                </div>
        </header>        

        <div class="transition-spacer">
            <h2 class="spacer-headline">{"How Lightfriend Delivers"}</h2>
        </div>

        <section class="story-section">
            <div class="story-grid">
                <div class="story-item">
                    <img src="/assets/lightfriend-robot-scene-1.png" alt="Tired of constant distractions" loading="lazy" />
                    <p>{"Trapped by the Scroll?"}</p>
                </div>
                <div class="story-item">
                    <img src="/assets/lightfriend-robot-scene-2.png" alt="Let robot handle monitoring" loading="lazy" />
                    <p>{"Let your lightfriend monitor apps."}</p>
                </div>
                <div class="story-item">
                    <img src="/assets/lightfriend-robot-scene-3.png" alt="Live life freely" loading="lazy" />
                    <p>{"Get notified when it matters"}</p>
                </div>
            </div>
        </section>

            <div class="filter-concept">
                <div class="filter-content">
                    <div class="filter-text">
                        <h2>{"Finally switch to a dumbphone"}</h2>
                        <p>{"Lightfriend lets you know when it's important and answers your questions anytime. Works between calling and SMS seamlessly without needing internet connection."}</p>

                    <div class="section-intro">
                            <Link<Route> to={Route::Pricing} classes="forward-link">
                                <button class="hero-cta">{"Try with 7-Day Detox Now"}</button>
                            </Link<Route>>
                    </div>
                    </div>
                    <div class="filter-image">
                        <img src="/assets/lightfriend-filter.png" alt="Lightfriend filtering concept" loading="lazy" />
                    </div>
                </div>
            </div>

            // Add mobile-only intro content first
            <div class="section-header">
            </div>

        <section class="how-it-works">
            <h2>{"Offload to Your Lightfriend"}</h2>
            <p>{"Stay focused while your robot handles monitoring."}</p>
            <div class="steps-grid">
                <div class="step">
                    <h3>{"Connect Apps"}</h3>
                    <p>{"Link your services securely."}</p>
                </div>
                <div class="step">
                    <h3>{"Set Priorities"}</h3>
                    <p>{"Choose what matters."}</p>
                </div>
                <div class="step">
                    <h3>{"Live Present"}</h3>
                    <p>{"Focus on life, get alerts for important stuff."}</p>
                </div>
            </div>
        </section>

        <footer class="footer-cta">
            <div class="footer-content">
                <h2>{"Ready for Digital Peace?"}</h2>
                <p class="subtitle">{"Let your personal robot monitor messages and notify only what matters."}</p>
                <Link<Route> to={Route::Pricing} classes="forward-link">
                    <button class="hero-cta">{"Start Today"}</button>
                </Link<Route>>
                <p class="disclaimer">{"Works with smartphones and basic phones. Customize to your needs."}</p>
                <div class="development-links">
                    <p>{"Source code on "}
                        <a href="https://github.com/ahtavarasmus/lightfriend" target="_blank" rel="noopener noreferrer">{"GitHub"}</a>
                    </p>
                    <p>{"Follow us at "}
                        <a href="https://pacepeek.com/ahtavarasmus" target="_blank" rel="noopener noreferrer">{"pacepeek.com/ahtavarasmus"}</a>
                        {" and "}
                        <a href="https://x.com/rasmuscodes" target="_blank" rel="noopener noreferrer">{"x.com/rasmuscodes"}</a>
                    </p>
                                <div class="legal-links">
                                    <Link<Route> to={Route::Terms}>{"Terms & Conditions"}</Link<Route>>
                                    {" | "}
                                    <Link<Route> to={Route::Privacy}>{"Privacy Policy"}</Link<Route>>
                                </div>
                </div>
            </div>
        </footer>
            <style>
                {r#"
    .cta-image-container {
        max-width: 300px;
        margin: 0 auto;
        display: flex;
        flex-direction: column;
        align-items: center;
        gap: 1rem;
        position: relative;
        padding: 0 2rem;
    }

    .filter-concept {
        padding: 4rem 2rem;
        margin: 0 auto;
        max-width: 1200px;
        position: relative;
        z-index: 2;
    }

    .filter-content {
        display: flex;
        align-items: center;
        gap: 4rem;
        background: rgba(30, 30, 30, 0.8);
        border: 1px solid rgba(30, 144, 255, 0.15);
        border-radius: 24px;
        padding: 3rem;
        transition: transform 0.3s ease, box-shadow 0.3s ease;
    }

    .filter-content:hover {
        transform: translateY(-5px);
        box-shadow: 0 8px 32px rgba(30, 144, 255, 0.15);
    }

    .filter-text {
        flex: 1;
    }

    .filter-text h2 {
        font-size: 2.5rem;
        margin-bottom: 1.5rem;
        background: linear-gradient(45deg, #fff, #7EB2FF);
        -webkit-background-clip: text;
        -webkit-text-fill-color: transparent;
    }

    .filter-image {
        flex: 1;
        display: flex;
        justify-content: center;
        align-items: center;
    }

    .filter-image img {
        max-width: 100%;
        height: auto;
        border-radius: 12px;
        box-shadow: 0 8px 32px rgba(0, 0, 0, 0.3);
    }

    @media (max-width: 768px) {
        .filter-concept {
            padding: 2rem 1rem;
        }

        .filter-content {
            flex-direction: column;
            padding: 2rem;
            gap: 2rem;
            text-align: center;
        }

        .filter-text h2 {
            font-size: 2rem;
        }
    }

    .landing-page {
        position: relative;
        min-height: 100vh;
        background-color: transparent;
        color: #ffffff;
        font-family: system-ui, -apple-system, sans-serif;
        margin: 0 auto;
        width: 100%;
        overflow-x: hidden;
        box-sizing: border-box;
        z-index: 0;
    }

    .main-features {
        max-width: 1200px;
        margin: 0 auto;
        padding: 0 2rem;
        position: relative;
        z-index: 3;
        background: transparent;
        opacity: 1;
        pointer-events: auto;
        margin-top: -30vh;
    }

    .feature-block {
        display: flex;
        align-items: center;
        gap: 4rem;
        background: rgba(30, 30, 30, 0.8);
        border: 1px solid rgba(30, 144, 255, 0.15);
        border-radius: 24px;
        padding: 3rem;
        z-index: 3;
        transition: transform 1.8s cubic-bezier(0.4, 0, 0.2, 1), 
                    border-color 1.8s ease, 
                    box-shadow 1.8s ease;
        overflow: hidden;
        position: relative;
        margin: 10%;
        margin-bottom: 180vh;
    }

    .feature-block:hover {
        transform: translateY(-5px) scale(1.02);
        box-shadow: 0 8px 32px rgba(30, 144, 255, 0.15);
        border-color: rgba(30, 144, 255, 0.3);
    }

    .feature-image {
        flex: 1;
        display: flex;
        justify-content: center;
        align-items: center;
    }

    .feature-image img {
        max-width: 100%;
        height: auto;
        border-radius: 12px;
        box-shadow: 0 8px 32px rgba(0, 0, 0, 0.3);
    }

    .demo-link-container {
        margin-top: 2rem;
        display: flex;
        justify-content: center;
    }

    .demo-link {
        display: inline-flex;
        align-items: center;
        gap: 0.5rem;
        padding: 0.8rem 1.5rem;
        background: linear-gradient(
            45deg,
            #7EB2FF,
            #4169E1
        );
        color: white;
        text-decoration: none;
        border-radius: 8px;
        font-size: 1rem;
        transition: all 0.3s ease;
    }

    .demo-link:hover {
        transform: translateY(-2px);
        box-shadow: 0 4px 20px rgba(30, 144, 255, 0.3);
    }

    @media (max-width: 1024px) {
        .feature-block {
            flex-direction: column;
            padding: 2rem;
            gap: 2rem;
            margin-bottom: 50vh;
        }

        .feature-image {
            order: -1;
        }
    }

    @media (max-width: 768px) {
        .landing-page {
            padding: 0;
        }

        .hero {
            padding: 2rem 1rem;
            padding-top: 100px;
        }

        .hero-subtitle {
            font-size: 1rem;
            padding: 0 1rem;
        }

        .how-it-works {
            padding: 0 3rem;
        }

        .how-it-works h2 {
            font-size: 1.75rem;
        }

        .steps-grid {
            grid-template-columns: 1fr;
            gap: 1.5rem;
            padding: 0 1rem;
        }

        .footer-cta {
            padding: 3rem 1rem;
        }

        .footer-cta h2 {
            font-size: 2rem;
        }

        .footer-cta .subtitle {
            font-size: 1rem;
        }

        .footer-content {
            padding: 0 1rem;
        }

        .development-links {
            padding: 0 1rem;
        }
    }

    .how-it-works {
        padding: 2rem 2rem;
        text-align: center;
        position: relative;
        z-index: 1;
        margin-top: 0;
    }

    .how-it-works::before {
        content: '';
        position: absolute;
        top: 0;
        left: 0;
        right: 0;
        bottom: 0;
        background: linear-gradient(
            to bottom,
            rgba(26, 26, 26, 0),
            rgba(26, 26, 26, 0.7),
            rgba(26, 26, 26, 0.9)
        );
        z-index: -1;
        pointer-events: none;
    }

    .how-it-works * {
        pointer-events: auto;
    }

    .how-it-works h2 {
        font-size: 3rem;
        margin-bottom: 1rem;
        text-shadow: 0 0 20px rgba(30, 144, 255, 0.2);
    }

    .how-it-works > p {
        color: #7EB2FF;
        margin-bottom: 4rem;
        font-size: 1.2rem;
    }

    .steps-grid {
        display: grid;
        grid-template-columns: repeat(3, 1fr);
        gap: 2rem;
        margin-top: 4rem;
    }

    .step {
        background: rgba(255, 255, 255, 0.03);
        border-radius: 16px;
        padding: 2.5rem;
        border: 1px solid rgba(30, 144, 255, 0.2);
        backdrop-filter: blur(5px);
        transition: all 0.3s ease;
        position: relative;
        overflow: hidden;
    }

    .step::before {
        content: '';
        position: absolute;
        top: 0;
        left: 0;
        right: 0;
        height: 1px;
        background: linear-gradient(
            90deg,
            transparent,
            rgba(30, 144, 255, 0.3),
            transparent
        );
    }

    .step:hover {
        transform: translateY(-5px);
        box-shadow: 0 4px 20px rgba(30, 144, 255, 0.15);
        border-color: rgba(30, 144, 255, 0.4);
    }

    .step h3 {
        color: #1E90FF;
        font-size: 1.5rem;
        margin-bottom: 1.5rem;
        font-weight: 600;
    }

    .step p {
        color: #999;
        font-size: 1rem;
        line-height: 1.6;
    }

    .step::after {
        content: '';
        position: absolute;
        top: 1rem;
        right: 1rem;
        width: 30px;
        height: 30px;
        border-radius: 50%;
        border: 2px solid rgba(30, 144, 255, 0.3);
        display: flex;
        align-items: center;
        justify-content: center;
        font-size: 0.9rem;
        color: #1E90FF;
    }

    .step:nth-child(1)::after {
        content: '1';
    }

    .step:nth-child(2)::after {
        content: '2';
    }

    .step:nth-child(3)::after {
        content: '3';
    }

    .footer-cta {
        padding: 6rem 0;
        background: linear-gradient(
            to bottom,
            transparent,
            rgba(30, 144, 255, 0.05)
        );
        border-top: 1px solid rgba(30, 144, 255, 0.1);
        text-align: left;
        position: relative;
        z-index: 1;
        margin-top: 0;
        background: rgba(26, 26, 26, 0.9);
        pointer-events: auto;
    }

    .footer-cta::before {
        content: '';
        position: absolute;
        top: 0;
        left: 0;
        right: 0;
        bottom: 0;
        background: linear-gradient(
            to bottom,
            rgba(26, 26, 26, 0.9),
            rgba(26, 26, 26, 0.95)
        );
        z-index: -1;
        pointer-events: none;
    }

    .footer-content {
        max-width: 800px;
        margin: 0 auto;
        padding: 0 2rem;
        width: 100%;
        box-sizing: border-box;
    }

    .footer-cta h2 {
        font-size: 3.5rem;
        margin-bottom: 1.5rem;
        background: linear-gradient(45deg, #fff, #7EB2FF);
        -webkit-background-clip: text;
        -webkit-text-fill-color: transparent;
        font-weight: 700;
    }

    .footer-cta .subtitle {
        font-size: 1.2rem;
        color: #999;
        margin-bottom: 2.5rem;
        line-height: 1.6;
    }

    .hero {
        height: 100vh;
        display: flex;
        flex-direction: column;
        align-items: center;
        justify-content: flex-start;
        text-align: center;
        position: relative;
        background: transparent;
        z-index: 1;
    }

    .hero-content {
        z-index: 3;
        width: 100%;
        height: 100%;
        display: flex;
        justify-content: space-around;
        padding: 40px;
        pointer-events: auto;
    }

    .hero-header {
        display: flex;
        flex-direction: column;
        justify-content: flex-end;
    }

    .hero-background {
        position: fixed;
        top: 0;
        left: 0;
        width: 100%;
        height: 100vh;
        background-image: url('/assets/boy_holding_dumbphone_in_crowded_place.webp');
        background-size: cover;
        background-position: center;
        background-repeat: no-repeat;
        opacity: 1;
        z-index: -2;
        pointer-events: none;
    }

    .hero-background::after {
        content: '';
        position: absolute;
        bottom: 0;
        left: 0;
        width: 100%;
        height: 50%;
        background: linear-gradient(to bottom, 
            rgba(26, 26, 26, 0) 0%,
            rgba(26, 26, 26, 1) 100%
        );
    }

    @media (max-width: 700px) {
        .hero-background {
            background-position: 70% center;
        }
    }

    .hero-subtitle {
        position: relative;
        font-size: 1.3rem;
        font-weight: 300;
        letter-spacing: 0.02em;
        max-width: 600px;
        margin: 0 auto 3rem;
        line-height: 1.8;
        font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Oxygen, Ubuntu, Cantarell, sans-serif;
        text-align: left;
        background: linear-gradient(45deg, #fff, #7EB2FF);
        -webkit-background-clip: text;
        -webkit-text-fill-color: transparent;
        text-shadow: none;
    }

    @media (max-width: 768px) {
        .hero-content {
            padding: 20px;
            flex-direction: column;
            justify-content: flex-end;
        }

        .hero-subtitle {
            font-size: 1.1rem;
            line-height: 1.6;
            margin-bottom: 2rem;
        }
    }

    .hero-cta {
        background: linear-gradient(
            45deg,
            #7EB2FF,
            #4169E1
        );
        color: white;
        border: none;
        padding: 1rem 2.5rem;
        border-radius: 8px;
        font-size: 1.1rem;
        cursor: pointer;
        transition: transform 1.5s cubic-bezier(0.4, 0, 0.2, 1),
                    box-shadow 1.5s ease,
                    background 0.3s ease;
        display: inline-flex;
        align-items: center;
        gap: 0.5rem;
        position: relative;
        overflow: hidden;
        margin: 2rem 0 3rem 0;
        border: 1px solid rgba(255, 255, 255, 0.2);
        backdrop-filter: blur(5px);
    }

    @media (min-width: 769px) {
        .hero-cta {
            margin: 3rem 0 3rem 0;
        }
    }

    .hero-cta::before {
        content: '';
        position: absolute;
        top: 0;
        left: 0;
        width: 100%;
        height: 100%;
        background: linear-gradient(
            45deg,
            transparent,
            rgba(255, 255, 255, 0.1),
            transparent
        );
        transform: translateX(-100%);
        transition: transform 0.6s;
    }

    .hero-cta::after {
        content: '→';
    }

    .hero-cta:hover::before {
        transform: translateX(100%);
    }

    .hero-cta:hover {
        transform: translateY(-2px);
        box-shadow: 0 4px 20px rgba(126, 178, 255, 0.4);
        background: linear-gradient(
            45deg,
            #90c2ff,
            #5479f1
        );
    }

    .hero-cta-group {
        display: flex;
        flex-direction: row;
        align-items: center;
        gap: 1rem;
    }

    .faq-link {
        color: #7EB2FF;
        text-decoration: none;
        font-size: 1rem;
        transition: all 0.3s ease;
        position: relative;
        padding: 0.5rem 1rem;
    }

    .faq-link::after {
        content: '';
        position: absolute;
        width: 100%;
        height: 1px;
        bottom: -2px;
        left: 0;
        background: linear-gradient(90deg, #1E90FF, #4169E1);
        transform: scaleX(0);
        transform-origin: bottom right;
        transition: transform 0.3s ease;
    }

    .faq-link:hover {
        color: #90c2ff;
        text-shadow: 0 0 8px rgba(30, 144, 255, 0.3);
    }

    .faq-link:hover::after {
        transform: scaleX(1);
        transform-origin: bottom left;
    }

    @media (max-width: 768px) {
        .hero-cta-group {
            gap: 0.75rem;
        }
    }

    .section-header {
        text-align: center;
    }

    .section-intro {
        max-width: 600px;
        margin: 0 auto;
        text-align: center;
        padding: 2rem;
        border-radius: 16px;
        display: flex;
        flex-direction: column;
        align-items: center;
        justify-content: center;
    }

    .section-intro .hero-cta {
        margin: 1rem auto;
        display: block;
    }

    .legal-links {
        margin-top: 1rem;
    }

    .legal-links a {
        color: #666;
        text-decoration: none;
        transition: color 0.3s ease;
    }

    .legal-links a:hover {
        color: #7EB2FF;
    }

    @media (max-width: 768px) {
        .section-intro {
            padding: 1.5rem;
            margin-top: 2rem;
        }

        .section-intro {
            padding: 1.5rem;
            margin-top: 2rem;
        }
    }

    .development-links {
        margin-top: 2rem;
        font-size: 0.9rem;
        color: #666;
    }

    .development-links p {
        margin: 0.5rem 0;
    }

    .development-links a {
        color: #007bff;
        text-decoration: none;
        position: relative;
        padding: 0 2px;
        transition: all 0.3s ease;
    }

    .development-links a::after {
        content: '';
        position: absolute;
        width: 100%;
        height: 1px;
        bottom: -2px;
        left: 0;
        background: linear-gradient(90deg, #1E90FF, #4169E1);
        transform: scaleX(0);
        transform-origin: bottom right;
        transition: transform 0.3s ease;
    }

    .development-links a:hover {
        color: #7EB2FF;
        text-shadow: 0 0 8px rgba(30, 144, 255, 0.3);
    }

    .development-links a:hover::after {
        transform: scaleX(1);
        transform-origin: bottom left;
    }

    .story-section {
        padding: 4rem 2rem;
        max-width: 1200px;
        margin: 0 auto;
        text-align: center;
        position: relative;
        z-index: 2;
    }

    .story-grid {
        display: grid;
        grid-template-columns: repeat(3, 1fr);
        gap: 2rem;
    }

    .story-item {
        background: rgba(30, 30, 30, 0.8);
        border: 1px solid rgba(30, 144, 255, 0.15);
        border-radius: 24px;
        padding: 1.5rem;
        display: flex;
        flex-direction: column;
        align-items: center;
        transition: transform 0.3s ease, box-shadow 0.3s ease;
    }

    .story-item:hover {
        transform: translateY(-5px);
        box-shadow: 0 8px 32px rgba(30, 144, 255, 0.15);
    }

    .story-item img {
        max-width: 100%;
        height: auto;
        border-radius: 12px;
        margin-bottom: 1rem;
    }

    .story-item p {
        color: #ddd;
        font-size: 1.4rem;
        font-weight: 500;
        margin: 0;
        text-shadow: 0 1px 2px rgba(0, 0, 0, 0.5);
    }

    .transition-spacer {
        padding: 2rem 0;
        text-align: center;
        max-width: 1200px;
        margin: 0 auto;
    }

    .spacer-headline {
        font-size: 2rem;
        background: linear-gradient(45deg, #fff, #7EB2FF);
        -webkit-background-clip: text;
        -webkit-text-fill-color: transparent;
        margin: 0;
    }

    @media (max-width: 768px) {
        .story-section {
            padding: 2rem 1rem;
        }

        .story-grid {
            grid-template-columns: 1fr;
        }

        .story-item p {
            font-size: 1.2rem;
        }

        .spacer-headline {
            font-size: 1.75rem;
        }
    }
                   
                "#}
            </style>
        </div>
    }
}
