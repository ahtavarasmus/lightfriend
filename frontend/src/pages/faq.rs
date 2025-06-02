use yew::prelude::*;
use web_sys::MouseEvent;
use yew::{Children, Properties};

#[derive(Properties, PartialEq)]
struct FaqItemProps {
    question: String,
    children: Children,
}

#[function_component(FaqItem)]
fn faq_item(props: &FaqItemProps) -> Html {
    let is_open = use_state(|| false);
    
    let toggle = {
        let is_open = is_open.clone();
        Callback::from(move |e: MouseEvent| {
            e.prevent_default();
            is_open.set(!*is_open);
        })
    };

    html! {
        <div class={classes!("faq-item", if *is_open { "open" } else { "" })}>
            <button class="faq-question" onclick={toggle}>
                <span class="question-text">{&props.question}</span>
                <span class="toggle-icon">{if *is_open { "−" } else { "+" }}</span>
            </button>
            <div class="faq-answer">
                { for props.children.iter() }
            </div>
        </div>
    }
}

#[function_component(Faq)]
pub fn faq() -> Html {
    html! {
        <div class="faq-page">
            <div class="faq-background"></div>
            <section class="faq-hero">
                <h1>{"Frequently Asked Questions"}</h1>
                <p>{"Everything you need to know about going light with LightFriend"}</p>
            </section>

            <section class="faq-section">
                <h2>{"Why Go Light?"}</h2>
                
                <FaqItem question="Why choose a dumbphone?">
                    <p>
                        {"Your time is precious - why waste it fighting an endless battle against notifications and addictive apps? While tech giants deploy armies of experts to hijack your focus, there's a simpler path: stepping away. Choosing a dumbphone isn't about going backwards, you'll still have cutting edge AI at your fingertips. It's about taking back control of your attention and living life as its protagonist rather than watching it pass by through a screen."}
                    </p>
                    <img src="/assets/squidwardlookingwindow.png" alt="Squidward looking through window metaphor" class="faq-image" />
                    <p>
                        {"Like Squidward on his window, many of us find ourselves looking out at life from behind our screens. We see others living, connecting, and experiencing the world firsthand, while we remain observers, separated by a digital barrier."}
                    </p>
                </FaqItem>

                <FaqItem question="What about the impact on relationships?">
                    <img src="/assets/kid_draws_mom.jpg" alt="Child drawing mother on phone" class="faq-image" />
                    <p>
                        {"A child's drawing tells a thousand words. When asked to draw their parents, more and more children depict them with phones in hand – a powerful reflection of how our digital habits affect those around us."}
                    </p>
                    <p>
                        {"This isn't the legacy we want to leave. It's not the presence we want to embody. Our children deserve parents who are present, engaged, and available – not just physically, but mentally and emotionally too."}
                    </p>
                </FaqItem>

                <FaqItem question="What's the value of boredom?">
                    <img src="/assets/boredom.png" alt="Illustration of creative boredom" class="faq-image" />
                    <p>
                        {"Remember when being bored meant letting your mind wander, leading to unexpected bursts of creativity and self-discovery? Today's smartphones have eliminated these precious moments of 'empty time' - replacing them with endless scrolling and constant stimulation."}
                    </p>
                    <p>
                        {"Boredom isn't your enemy – it's the canvas for creativity, the spark for innovation, and the space where your best ideas are born. When you're constantly entertained, you lose those moments of reflection that lead to personal growth and creative breakthroughs."}
                    </p>
                </FaqItem>

                <h2>{"Practical Solutions"}</h2>

                <FaqItem question="How do I handle 2FA authentication?">
                    <h3>{"'Step Two' mac app"}</h3>
                    <p>{"It is very fast and simple. It's free for certain number of accounts and then small one time payment for unlimited."}</p>
                    <img src="/assets/StepTwo.png" alt="Step Two app" class="faq-image" />
                    
                    <h3>{"Yubikey"}</h3>
                    <p>{"Can be used inplace of authenticator apps."}</p>
                    <img src="/assets/Yubikey.png" alt="Yubikey" class="faq-image" />
                    
                    <h3>{"Physical Code Calculator Device"}</h3>
                    <p>{"Most banks have it and it's used for bank login."}</p>
                    <img src="/assets/nordea_code_calc.png" alt="Nordea code calculator" class="faq-image" />
                </FaqItem>

                <FaqItem question="How do I handle commuting and navigation?">
                    <h3>{"Airport"}</h3>
                    <p>{"Get printed boarding passes and use computer to check flight times. With some airlines you can also get gate changes texted to you."}</p>
                    
                    <h3>{"Bus"}</h3>
                    <p>{"If you use bus in your home town, ask for physical keycard which can be loaded with credits."}</p>
                    
                    <h3>{"Taxi"}</h3>
                    <p>{"In US, Canada and UK there is "}<a href="https://www.tremp.me/">{"Tremp."}</a></p>
                    
                    <h3>{"Maps"}</h3>
                    <p>{"Physical paper map, maps on your computer or get a phone that has maps like "}<a href="https://www.thelightphone.com/">{"the Light Phone."}</a>{" Honestly, maps is a hard one and you will get lost a ton, but they make great adventures:)."}</p>
                </FaqItem>

                <h2>{"Privacy & Security"}</h2>

                <FaqItem question="How does LightFriend protect my data?">
                    <p>{"We keep your data minimal and secure:"}</p>
                    <ul>
                        <li><strong>{"Calls:"}</strong>{" No recordings. Just anonymous metrics to improve service."}</li>
                        <li><strong>{"Messages:"}</strong>{" Sensitive info redacted, stored securely with Twilio, fetched only when needed."}</li>
                    </ul>
                    <p class="context-example">{"Example redaction:"}</p>
                    <pre class="redaction-example">
                        {"Original: \"Check if John Smith sent the $5000 invoice\"\nStored: \"Check if [NAME_REDACTED] sent the [CONTENT_REDACTED]\""}
                    </pre>
                </FaqItem>

                <FaqItem question="What tools can help me stay focused?">
                    <ul>
                        <li><a href="https://getcoldturkey.com/">{"Cold Turkey App Blocker"}</a>{" is great for website and computer app blocking. It is very strong so be careful though not to lock yourself out of your computer:D"}</li>
                        <li>{"Amazon kindle has small simple text based browser, which can be used for reading website blogs."}</li>
                        <li>{"If you want to watch some youtube videos on your computer, there's "}<a href="https://freetubeapp.io/">{"FreeTube"}</a>{" app that only has subscription feed(it has recommended videos also but they are not personalized)"}</li>
                    </ul>
                </FaqItem>
            </section>

            <style>
                {r#"
                .faq-page {
                    padding-top: 74px;
                    min-height: 100vh;
                    color: #ffffff;
                    position: relative;
                    background: transparent;
                }

                .faq-background {
                    position: fixed;
                    top: 0;
                    left: 0;
                    width: 100%;
                    height: 100vh;
                    background-image: url('/assets/bicycle_field.png');
                    background-size: cover;
                    background-position: center;
                    background-repeat: no-repeat;
                    opacity: 1;
                    z-index: -2;
                    pointer-events: none;
                }

                .faq-background::after {
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

                .faq-hero {
                    text-align: center;
                    padding: 6rem 2rem;
                    background: rgba(26, 26, 26, 0.75);
                    backdrop-filter: blur(5px);
                    margin-top: 2rem;
                    border: 1px solid rgba(30, 144, 255, 0.1);
                    margin-bottom: 2rem;
                }

                .faq-hero h1 {
                    font-size: 3.5rem;
                    margin-bottom: 1.5rem;
                    background: linear-gradient(45deg, #fff, #7EB2FF);
                    -webkit-background-clip: text;
                    -webkit-text-fill-color: transparent;
                }

                .faq-hero p {
                    font-size: 1.2rem;
                    color: #999;
                    max-width: 600px;
                    margin: 0 auto;
                }

                .faq-section {
                    max-width: 800px;
                    margin: 0 auto;
                    padding: 2rem;
                }

                .faq-section h2 {
                    font-size: 2.5rem;
                    margin: 3rem 0 2rem;
                    background: linear-gradient(45deg, #fff, #7EB2FF);
                    -webkit-background-clip: text;
                    -webkit-text-fill-color: transparent;
                }

                .faq-item {
                    background: rgba(26, 26, 26, 0.85);
                    backdrop-filter: blur(10px);
                    border: 1px solid rgba(30, 144, 255, 0.1);
                    border-radius: 12px;
                    margin-bottom: 1rem;
                    overflow: hidden;
                    transition: all 0.3s ease;
                }

                .faq-item:hover {
                    border-color: rgba(30, 144, 255, 0.3);
                }

                .faq-question {
                    width: 100%;
                    padding: 1.5rem;
                    background: none;
                    border: none;
                    color: #fff;
                    font-size: 1.2rem;
                    text-align: left;
                    cursor: pointer;
                    display: flex;
                    justify-content: space-between;
                    align-items: center;
                    transition: all 0.3s ease;
                }

                .faq-question:hover {
                    color: #7EB2FF;
                }

                .toggle-icon {
                    font-size: 1.5rem;
                    color: #7EB2FF;
                    transition: transform 0.3s ease;
                }

                .faq-item.open .toggle-icon {
                    transform: rotate(180deg);
                }

                .faq-answer {
                    max-height: 0;
                    overflow: hidden;
                    transition: max-height 0.5s ease;
                    padding: 0 1.5rem;
                }

                .faq-item.open .faq-answer {
                    max-height: 2000px;
                    padding: 0 1.5rem 1.5rem;
                }

                .faq-answer p {
                    color: #999;
                    line-height: 1.6;
                    margin-bottom: 1rem;
                }

                .faq-image {
                    max-width: 100%;
                    height: auto;
                    border-radius: 12px;
                    margin: 1.5rem 0;
                    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.3);
                    transition: transform 0.3s ease;
                }

                .faq-image:hover {
                    transform: scale(1.02);
                }

                .faq-answer h3 {
                    color: #7EB2FF;
                    font-size: 1.3rem;
                    margin: 1.5rem 0 1rem;
                }

                .faq-answer ul {
                    list-style: none;
                    padding: 0;
                    margin: 1rem 0;
                }

                .faq-answer li {
                    color: #999;
                    padding: 0.5rem 0;
                    padding-left: 1.5rem;
                    position: relative;
                }

                .faq-answer li::before {
                    content: '•';
                    position: absolute;
                    left: 0.5rem;
                    color: #1E90FF;
                }

                .faq-answer a {
                    color: #1E90FF;
                    text-decoration: none;
                    transition: color 0.3s ease;
                }

                .faq-answer a:hover {
                    color: #7EB2FF;
                }

                .redaction-example {
                    background: rgba(0, 0, 0, 0.3);
                    padding: 1rem;
                    border-radius: 8px;
                    font-family: monospace;
                    font-size: 0.9rem;
                    color: #999;
                    white-space: pre-wrap;
                    overflow-x: auto;
                }

                @media (max-width: 768px) {
                    .faq-hero {
                        padding: 4rem 1rem;
                    }

                    .faq-hero h1 {
                        font-size: 2.5rem;
                    }

                    .faq-section {
                        padding: 1rem;
                    }

                    .faq-section h2 {
                        font-size: 2rem;
                    }

                    .faq-question {
                        font-size: 1.1rem;
                        padding: 1rem;
                    }

                    .faq-answer {
                        padding: 0 1rem;
                    }

                    .faq-item.open .faq-answer {
                        padding: 0 1rem 1rem;
                    }
                }
                "#}
            </style>
        </div>
    }
}

