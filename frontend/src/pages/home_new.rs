use yew::prelude::*;
use yew_router::prelude::*;
use crate::Route;

#[function_component(HomeNew)]
pub fn home_new() -> Html {
    html! {
        <div class="landing">
            <section class="hero">
                <div class="hero__title">
                    <div class="stack stack--fade">
                        <span>{"Break"}</span>
                        <span>{"Free"}</span>
                        <span>{"Without"}</span>
                        <span>{"Vanishing"}</span>
                    </div>
                </div>

                <div class="hero__image">
                    <div class="image__background">
                        <div class="media-box">
                            <img src="/assets/nokia_hand.png" alt="Nokia phone in hand" />
                        </div>
                    </div>

                    <div class="hero__masks">
                        <div class="hero__not">
                            <div class="flash">
                                <div class="flash__item">{"( stay connected )"}</div>
                            </div>
                        </div>

                        <div class="image__overlay">
                            <img src="/assets/nokia_hand.png" alt="Nokia phone overlay" />
                            <div class="hero__text-mobile">
                                <span>{" "}</span>
                                <span>{"Free"}</span>
                                <span>{" "}</span>
                            </div>
                        </div>
                    </div>
                </div>

                <div class="hero__content">
                    <div class="content__text">
                        <p>
                            {"The average person spends 4.5 hours a day on social media. LightFriend's universal SMS and voice interface connects your digital life to any dumbphone."}
                        </p>
                    </div>

                    <div class="content__continue">
                        <Link<Route> to={Route::Register} classes="cta-button">
                            <span>{"Start Using Now"}</span>
                            <i class="arrow">{"→"}</i>
                        </Link<Route>>
                    </div>
                </div>
            </section>

            <style>
                {r#"
                .landing {
                    min-height: 100vh;
                    background: #1a1a1a;
                    color: #ffffff;
                    overflow-x: hidden;
                }

                .hero {
                    position: relative;
                    padding: 6rem 2rem;
                    display: flex;
                    flex-direction: column;
                    align-items: center;
                }

                .hero__title {
                    text-align: center;
                    margin-bottom: 4rem;
                }

                .stack {
                    display: flex;
                    flex-direction: column;
                    gap: 0.5rem;
                }

                .stack span {
                    font-size: 4.5rem;
                    font-weight: 700;
                    background: linear-gradient(45deg, #fff, #7EB2FF);
                    -webkit-background-clip: text;
                    -webkit-text-fill-color: transparent;
                    opacity: 0;
                    animation: fadeIn 0.5s forwards;
                }

                .stack span:nth-child(1) { animation-delay: 0.1s; }
                .stack span:nth-child(2) { animation-delay: 0.3s; }
                .stack span:nth-child(3) { animation-delay: 0.5s; }
                .stack span:nth-child(4) { animation-delay: 0.7s; }

                @keyframes fadeIn {
                    from { 
                        opacity: 0;
                        transform: translateY(20px);
                    }
                    to { 
                        opacity: 1;
                        transform: translateY(0);
                    }
                }

                .hero__image {
                    position: relative;
                    width: 100%;
                    max-width: 500px;
                    margin: 2rem 0;
                }

                .image__background {
                    position: relative;
                    overflow: hidden;
                    border-radius: 24px;
                }

                .media-box img {
                    width: 100%;
                    height: auto;
                    transition: transform 0.3s ease;
                }

                .hero__masks {
                    position: absolute;
                    top: 0;
                    left: 0;
                    width: 100%;
                    height: 100%;
                    pointer-events: none;
                }

                .hero__not {
                    position: absolute;
                    top: 2rem;
                    left: 50%;
                    transform: translateX(-50%);
                    color: #7EB2FF;
                    font-size: 1.2rem;
                }

                .flash {
                    overflow: hidden;
                }

                .flash__item {
                    animation: slideIn 0.5s forwards;
                    transform: translateY(100%);
                }

                @keyframes slideIn {
                    to {
                        transform: translateY(0);
                    }
                }

                .hero__content {
                    text-align: center;
                    max-width: 600px;
                    margin: 4rem auto 0;
                }

                .content__text {
                    color: #999;
                    font-size: 1.2rem;
                    line-height: 1.6;
                    margin-bottom: 2rem;
                }

                .cta-button {
                    display: inline-flex;
                    align-items: center;
                    gap: 0.5rem;
                    padding: 1rem 2rem;
                    background: linear-gradient(45deg, #1E90FF, #4169E1);
                    color: white;
                    text-decoration: none;
                    border-radius: 8px;
                    font-size: 1.1rem;
                    transition: all 0.3s ease;
                }

                .cta-button:hover {
                    transform: translateY(-2px);
                    box-shadow: 0 4px 20px rgba(30, 144, 255, 0.3);
                }

                .arrow {
                    transition: transform 0.3s ease;
                }

                .cta-button:hover .arrow {
                    transform: translateX(5px);
                }

                @media (max-width: 768px) {
                    .hero {
                        padding: 4rem 1rem;
                    }

                    .stack span {
                        font-size: 3rem;
                    }

                    .content__text {
                        font-size: 1rem;
                    }
                }
                "#}
            </style>
        </div>
    }
}

