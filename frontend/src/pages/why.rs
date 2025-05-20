use yew::prelude::*;

#[function_component(Why)]
pub fn why() -> Html {
    html! {
        <div class="why-page">
            <div class="why-background"></div>
            <section class="why-hero">
                <h1>{"Why Choose a Dumbphone?"}</h1>
                <p>{"Your willpower shouldn't be spent fighting an endless stream of notifications and addictive apps. Tech companies employ thousands of experts specifically to capture and hold your attention. Instead of exhausting yourself resisting these forces, why not simply step away? Choosing a dumbphone isn't about rejecting technology - you'll still have access to powerful tools like AI when you need them. It's about consciously choosing where your attention goes. It's about being the main character in your own life story, not a spectator scrolling through others' highlights."}</p>
            </section>

            <section class="why-section alternate">
                <div class="why-content">
                    <div class="why-text">
                        <h2>{"The Window of Realization"}</h2>
                        <p>
                            {"Like Squidward on his window, many of us find ourselves looking out at life from behind our screens. We see others living, connecting, and experiencing the world firsthand, while we remain observers, separated by a digital barrier."}
                        </p>
                        <p>
                            {"This isn't just about missing out – it's about recognizing that our smartphones, while promising connection, often create the very isolation they claim to solve."}
                        </p>
                    </div>
                    <div class="why-image">
                        <img src="/assets/squidwardlookingwindow.png" alt="Squidward looking through window metaphor" />
                    </div>
                </div>
            </section>

            <section class="why-section">
                <div class="why-content">
                    <div class="why-image">
                        <img src="/assets/kid_draws_mom.jpg" alt="Child drawing mother on phone" />
                    </div>
                    <div class="why-text">
                        <h2>{"Through Their Eyes"}</h2>
                        <p>
                            {"A child's drawing tells a thousand words. When asked to draw their parents, more and more children depict them with phones in hand – a powerful reflection of how our digital habits affect those around us."}
                        </p>
                        <p>
                            {"This isn't the legacy we want to leave. It's not the presence we want to embody. Our children deserve parents who are present, engaged, and available – not just physically, but mentally and emotionally too."}
                        </p>
                    </div>
                </div>
            </section>

            <section class="why-section alternate">
                <div class="why-content">
                    <div class="why-image">
                        <img src="/assets/boredom.png" alt="Illustration of creative boredom" />
                    </div>
                    <div class="why-text">
                        <h2>{"The Lost Art of Boredom"}</h2>
                        <p>
                            {"Remember when being bored meant letting your mind wander, leading to unexpected bursts of creativity and self-discovery? Today's smartphones have eliminated these precious moments of 'empty time' – replacing them with endless scrolling and constant stimulation."}
                        </p>
                        <p>
                            {"But boredom isn't your enemy – it's the canvas for creativity, the spark for innovation, and the space where your best ideas are born. When you're constantly entertained, you lose those moments of reflection that lead to personal growth and creative breakthroughs."}
                        </p>
                        <p>
                            {"By choosing a dumbphone, you're not just accepting boredom – you're embracing it as a catalyst for imagination and self-discovery. Let your mind wander again. Who knows what amazing ideas you might discover?"}
                        </p>
                    </div>
                </div>
            </section>

            <section class="why-section">
                <div class="why-content">
                    <div class="why-text">
                        <h2>{"Embrace the Unknown"}</h2>
                        <p>
                            {"The best stories often come from unexpected moments and unplanned adventures. Whether it's finding a hidden café while wandering without GPS, striking up a conversation without the safety net of your phone, or simply being present enough to notice life's little surprises."}
                        </p>
                        <p>
                            {"Modern smartphones can make life more predictable and 'safer', but at what cost? When everything is planned, reviewed, and optimized, we lose the charm of spontaneity and the thrill of the unknown. Sometimes, the most memorable experiences come from those moments when we step away from our digital safety nets and let life surprise us."}
                        </p>
                        <p>
                            {"Your next adventure, friendship, or favorite memory might be waiting just around the corner – but you'll miss it if you're too busy following the perfect route on your screen."}
                        </p>
                    </div>
                    <div class="why-image">
                        <img src="/assets/no_stories_to_tell.png" alt="No stories to tell metaphor" />
                    </div>
                </div>
            </section>

            <style>
                {r#"
.why-page {
    padding-top: 74px;
    min-height: 100vh;
    color: #ffffff;
    position: relative;
    background: transparent;
}

.why-background {
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

.why-background::after {
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

.why-hero {
    text-align: center;
    padding: 6rem 2rem;
    background: rgba(26, 26, 26, 0.75);
    backdrop-filter: blur(5px);
    margin-top: 2rem;
    border: 1px solid rgba(30, 144, 255, 0.1);
    margin-bottom: 2rem;
}

                .why-hero h1 {
                    font-size: 3.5rem;
                    margin-bottom: 1.5rem;
                    background: linear-gradient(45deg, #fff, #7EB2FF);
                    -webkit-background-clip: text;
                    -webkit-text-fill-color: transparent;
                }

                .why-hero p {
                    font-size: 1.2rem;
                    color: #999;
                    max-width: 600px;
                    margin: 0 auto;
                }

.why-section {
    padding: 6rem 2rem;
    position: relative;
    background: rgba(26, 26, 26, 0.85);
    backdrop-filter: blur(10px);
    margin: 2rem 0;
    border: 1px solid rgba(30, 144, 255, 0.1);
}

                .why-section.alternate {
                    background: rgba(30, 144, 255, 0.05);
                }

                .why-content {
                    max-width: 1200px;
                    margin: 0 auto;
                    display: flex;
                    align-items: center;
                    gap: 4rem;
                }

                .why-text {
                    flex: 1;
                }

                .why-text h2 {
                    font-size: 2.5rem;
                    margin-bottom: 1.5rem;
                    background: linear-gradient(45deg, #fff, #7EB2FF);
                    -webkit-background-clip: text;
                    -webkit-text-fill-color: transparent;
                }

                .why-text p {
                    color: #999;
                    font-size: 1.1rem;
                    line-height: 1.8;
                    margin-bottom: 1.5rem;
                }

                .why-image {
                    flex: 1;
                    display: flex;
                    justify-content: center;
                    align-items: center;
                }

                .why-image img {
                    max-width: 100%;
                    height: auto;
                    border-radius: 12px;
                    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.3);
                    transition: transform 0.3s ease;
                }

                .why-image img:hover {
                    transform: scale(1.02);
                }

                .alternate .why-content {
                    flex-direction: row-reverse;
                }

                @media (max-width: 768px) {
                    .why-hero {
                        padding: 4rem 1rem;
                    }

                    .why-hero h1 {
                        font-size: 2.5rem;
                    }

                    .why-section {
                        padding: 4rem 1rem;
                    }

                    .why-content {
                        flex-direction: column-reverse !important;
                        gap: 2rem;
                    }

                    .why-text h2 {
                        font-size: 2rem;
                    }

                    .why-text p {
                        font-size: 1rem;
                    }
                }
                "#}
            </style>
        </div>
    }
}

