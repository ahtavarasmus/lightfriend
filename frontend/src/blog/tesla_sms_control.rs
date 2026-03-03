use yew::prelude::*;
use yew_router::components::Link;
use crate::Route;
use crate::utils::seo::{use_seo, SeoMeta};

#[function_component(TeslaSmsControl)]
pub fn tesla_sms_control() -> Html {
    use_seo(SeoMeta {
        title: "Tesla Control via SMS: Manage Your Tesla Without a Smartphone",
        description: "Control your Tesla from any phone using SMS commands through Lightfriend. Lock, unlock, climate control, and more without the Tesla app.",
        canonical: "https://lightfriend.ai/blog/tesla-sms-control",
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
                <h1>{"Tesla Control via SMS: Manage Your Tesla Without a Smartphone"}</h1>
                <p>{"Lock, unlock, and climate-control your Tesla from a dumbphone."}</p>
            </section>
            <section class="blog-content">
                <h2>{"The Problem: Tesla Loves Smartphones"}</h2>
                <p>{"Tesla designed their cars around smartphone control. The Tesla app handles everything from unlocking the car to pre-conditioning the cabin, checking charge status, and opening the frunk. For dumbphone users, this presents a real challenge \u{2013} how do you control a high-tech car from a low-tech phone?"}</p>
                <p>{"The Tesla key card works for basic lock/unlock, but you lose all remote features. That is where Lightfriend comes in."}</p>

                <h2>{"How It Works: Lightfriend + Tesla"}</h2>
                <p>{"Lightfriend connects to the Tesla API and lets you control your car via simple SMS commands or voice calls. Text a command to your Lightfriend number, and it executes on your Tesla in seconds."}</p>

                <h2>{"Available SMS Commands"}</h2>
                <ul>
                    <li><strong>{"\"Unlock my Tesla\""}</strong>{" \u{2013} Unlocks all doors."}</li>
                    <li><strong>{"\"Lock my Tesla\""}</strong>{" \u{2013} Locks all doors."}</li>
                    <li><strong>{"\"Start climate\""}</strong>{" \u{2013} Turns on climate control to your preset temperature."}</li>
                    <li><strong>{"\"Stop climate\""}</strong>{" \u{2013} Turns off climate control."}</li>
                    <li><strong>{"\"Tesla status\""}</strong>{" \u{2013} Returns battery level, range, and location."}</li>
                    <li><strong>{"\"Open frunk\""}</strong>{" \u{2013} Opens the front trunk."}</li>
                    <li><strong>{"\"Flash lights\""}</strong>{" \u{2013} Flashes headlights (useful for finding your car in a parking lot)."}</li>
                    <li><strong>{"\"Honk horn\""}</strong>{" \u{2013} Sounds the horn."}</li>
                    <li><strong>{"\"Start charging\""}</strong>{" \u{2013} Begins charging when plugged in."}</li>
                    <li><strong>{"\"Stop charging\""}</strong>{" \u{2013} Stops charging."}</li>
                </ul>

                <h2>{"Setup Guide"}</h2>
                <ol>
                    <li>{"Sign up for a Lightfriend account and choose a plan that includes Tesla integration."}</li>
                    <li>{"Connect your Tesla account through the Lightfriend dashboard by authorizing the Tesla API."}</li>
                    <li>{"Save your Lightfriend phone number in your dumbphone contacts."}</li>
                    <li>{"Start texting commands. Lightfriend\u{2019}s AI understands natural language, so you do not need exact syntax."}</li>
                </ol>

                <h2>{"Why Control Tesla from a Dumbphone?"}</h2>
                <p>{"Tesla owners who switch to dumbphones often worry about losing car control. With Lightfriend, you keep full remote control while enjoying a distraction-free phone. Pre-heat your car on cold mornings, check your charge level before a road trip, or unlock from across the parking lot \u{2013} all with a simple text message."}</p>
                <p>{"You can also use voice: call Lightfriend and say \u{201c}unlock my Tesla\u{201d} and it handles the rest. Perfect for when your hands are full."}</p>

                <div class="blog-cta">
                    <h3>{"Control Your Tesla from Any Phone"}</h3>
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
