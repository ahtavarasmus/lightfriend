use yew::prelude::*;
use yew_router::components::Link;
use crate::Route;

#[function_component(ReadMoreAccidentallyGuide)]
pub fn read_more_accidentally_guide() -> Html {
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
                <h1>{"How to Read More Accidentally"}</h1>
                <p>{"Ditch the brainrot, and watch books become your new addiction. Who knew?"}</p>
                <img src="/assets/man_accidentally_reading_books.png" alt="Man being surprised he is reading a book" loading="lazy" class="blog-image" />
            </section>
            <section class="blog-content">
                <h2>{"Bye Brainrot, Hello Books!"}</h2>
                <p>{"Ever feel like your phone is a black hole sucking away your reading time? The trick isn't some superhuman willpower, it's removing the easy escapes. Switch to a dumbphone, and suddenly that book on your shelf looks way more interesting than staring at a blank screen. Funny how that works, right? All this time, the algorithms were the real villains."}</p>
                <p>{"Keep your computer for the essentials (work, friends, that one group chat you can't quit), but block the endless scrolls. Apps like:"}</p>
                <ul>
                    <li>
                        <a href="https://getcoldturkey.com" target="_blank">{"Cold Turkey"}</a>{" to lock distractions. Give the password to a buddy, and boom, no more doomscrolling. (Lightfriend users get 20% off Pro!)"}
                    </li>
                    <li>
                        <a href="https://freetubeapp.io" target="_blank">{"FreeTube"}</a>{" for YouTube without the rabbit holes."}
                    </li>
                    <li>
                        <a href="https://beeper.com" target="_blank">{"Beeper"}</a>{" to chat without the social media."}
                    </li>
                </ul>
                <h2>{"The Dumbphone Magic Trick"}</h2>
                <p>{"Grab a simple Nokia or something from "}<a href="https://dumbphones.org" target="_blank">{"dumbphones.org"}</a>{". No apps, no feeds, just calls and texts. Suddenly, waiting in line? Book! Bored on the bus? Book! It's hilarious how compelling books become when your only alternative is counting ceiling tiles. Options like The Light Phone or Mudita Kompakt even have hotspots for your laptop wifi needs. Retro Nokia from vintagemobile.fr? Chef's kiss."}</p>
                <h2>{"Lightfriend AI: Your Wingman"}</h2>
                <p>{"But hey, life's not all analog. Lightfriend.ai is your dumbphone's smart sidekick, text or call it to check emails, messages, or calendars. It filters out the noise, forwarding only the urgent stuff."}</p>
                <h2>{"Quick Steps to Book Bliss"}</h2>
                <ol>
                    <li>{"Pick a dumbphone (Nokia vibes welcome)."}</li>
                    <li>{"Block distractions on your computer with Cold Turkey."}</li>
                    <li>{"Sign up for Lightfriend to stay connected without the chaos."}</li>
                    <li>{"Stack some books nearby."}</li>
                    <li>{"Laugh as you accidentally read more than ever. Easy peasy!"}</li>
                </ol>
                <div class="blog-cta">
                    <h3>{"Ready to Accidentally Become a Bookworm?"}</h3>
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
