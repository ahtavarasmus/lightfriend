use yew::prelude::*;

#[function_component(Blog)]
pub fn blog() -> Html {
    let styles = stylist::Style::new(r#"
        .blog-container {
            max-width: 800px;
            margin: 0 auto;
            padding: 2rem;
            margin-top: 74px;
            min-height: 100vh;
            background: #1a1a1a;
            color: #e0e0e0;
        }

        .blog-post {
            background: rgba(30, 30, 30, 0.7);
            border: 1px solid rgba(30, 144, 255, 0.1);
            border-radius: 16px;
            padding: 3rem;
            backdrop-filter: blur(10px);
            box-shadow: 0 8px 32px rgba(0, 0, 0, 0.3);
        }

        .blog-header {
            margin-bottom: 3rem;
            text-align: center;
            position: relative;
        }

        .blog-header::after {
            content: '';
            position: absolute;
            bottom: -1.5rem;
            left: 50%;
            transform: translateX(-50%);
            width: 60px;
            height: 2px;
            background: linear-gradient(90deg, transparent, #1E90FF, transparent);
        }

        .blog-title {
            font-size: 3rem;
            margin-bottom: 1.5rem;
            background: linear-gradient(45deg, #fff, #7EB2FF);
            -webkit-background-clip: text;
            -webkit-text-fill-color: transparent;
            line-height: 1.2;
            font-weight: 700;
        }

        .blog-meta {
            color: #999;
            font-size: 0.9rem;
            display: flex;
            justify-content: center;
            align-items: center;
            gap: 1rem;
        }

        .blog-divider {
            color: rgba(30, 144, 255, 0.3);
        }

        .blog-content {
            color: #e0e0e0;
            line-height: 1.8;
            font-size: 1.1rem;
        }

        .blog-content h2 {
            color: #7EB2FF;
            font-size: 2rem;
            margin: 3rem 0 1.5rem;
            position: relative;
            padding-bottom: 0.5rem;
        }

        .blog-content h2::after {
            content: '';
            position: absolute;
            bottom: 0;
            left: 0;
            width: 40px;
            height: 2px;
            background: #1E90FF;
        }

        .blog-content h3 {
            color: #5a9eff;
            font-size: 1.5rem;
            margin: 2rem 0 1rem;
        }

        .blog-content p {
            margin-bottom: 1.5rem;
            color: #e0e0e0;
            line-height: 1.8;
        }

        .blog-content a {
            color: #1E90FF;
            text-decoration: none;
            position: relative;
            padding: 0 2px;
            transition: all 0.3s ease;
        }

        .blog-content a::after {
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

        .blog-content a:hover {
            color: #7EB2FF;
            text-shadow: 0 0 8px rgba(30, 144, 255, 0.3);
        }

        .blog-content a:hover::after {
            transform: scaleX(1);
            transform-origin: bottom left;
        }

        .blog-image {
            max-width: 100%;
            height: auto;
            margin: 2rem auto;
            border-radius: 12px;
            box-shadow: 0 4px 20px rgba(0, 0, 0, 0.3);
            display: block;
            transition: transform 0.3s ease;
        }

        .blog-image:hover {
            transform: scale(1.02);
        }

        .blog-content ul {
            list-style-type: none;
            padding-left: 0;
            margin: 2rem 0;
        }

        .blog-content ul li {
            position: relative;
            padding-left: 1.5rem;
            margin-bottom: 1rem;
            line-height: 1.6;
        }

        .blog-content ul li::before {
            content: '•';
            position: absolute;
            left: 0;
            color: #1E90FF;
            font-weight: bold;
        }

        @media (max-width: 768px) {
            .blog-container {
                padding: 1rem;
            }

            .blog-post {
                padding: 1.5rem;
            }

            .blog-title {
                font-size: 2rem;
            }

            .blog-content h2 {
                font-size: 1.75rem;
            }

            .blog-content h3 {
                font-size: 1.25rem;
            }

            .blog-content p {
                font-size: 1rem;
            }

            .blog-image {
                max-width: 100%;
            }
        }
    "#).expect("Failed to create style");

    html! {
        <div class={styles}>
            <div class="blog-container">
            <article class="blog-post">
                <header class="blog-header">
                    <h1 class="blog-title">{"The other stuff"}</h1>
                    <div class="blog-meta">
                        <span class="blog-date">{"March 26, 2024"}</span>
                        <span class="blog-divider">{"|"}</span>
                        <span class="blog-author">{"by Rasmus Ahtava"}</span>
                    </div>
                </header>
                
                <div class="blog-content">
                    <p>
                        {"The stuff that lightfriend won't save you on. Let me know your solutions so I could make this as comprehensive as possible:), you can reach me at "}<a href="mailto:rasmus@ahtava.com">{"rasmus@ahtava.com"}</a>
                    </p>

                    <h2>{"2FA authentication"}</h2>
                    <h3>{"'Step Two' mac app."}</h3> 
                    <p>
                        {"It is very fast and simple. It's free for certain number of accounts and then small one time payment for unlimited."}
                        <img src="/assets/StepTwo.png" alt="Step Two image" class="blog-image" />
                    </p>
                    <h3>{"Yubikey"}</h3> 
                    <p>
                         {"Can be used inplace of authenticator apps."}
                        <img src="/assets/Yubikey.png" alt="Yubikey image" class="blog-image" />
                    </p>
                    <h3>{"Physical Code Calculator Device"}</h3> 
                    <p>
                        {"Most banks have it and it's used for bank login. Photo of the Nordea one below."}
                        <img src="/assets/nordea_code_calc.png" alt="Nordea code calculator image" class="blog-image" />
                    </p>
                    <h2>{"Commuting"}</h2>
                    <h3>{"Airport"}</h3> 
                    <p>{"Get printed boarding passes and use computer to check flight times. With some airlines you can also get gate changes texted to you."}</p>
                    <h3>{"Bus"}</h3> 
                    <p>{"If you use bus in your home town, ask for physical keycard which can be loaded with credits."}</p>
                    <h3>{"Taxi"}</h3> 
                    <p>{"In US, Canada and UK there is "}<a href="https://www.tremp.me/">{"Tremp."}</a></p>
                    <h3>{"Maps"}</h3> 
                    <p>{"Physical paper map, maps on your computer or get a phone that has maps like "}<a href="https://www.thelightphone.com/">{"the Light Phone."}</a> {" Honestly, maps is a hard one and you will get lost a ton, but they make great adventures:)."}</p>

                    <h2>{"Stuff you can also try when going light"}</h2>
                    <ul>
                        <li><a href="https://getcoldturkey.com/">{"Cold Turkey App Blocker"}</a>{" is great for website and computer app blocking. It is very strong so be careful though not to lock yourself out of your computer:D"}</li>
                        <li>{"Amazon kindle has small simple text based browser, which can be used for reading website blogs."}</li>
                        <li>{"If you want to watch some youtube videos on your computer, there's "}<a href="https://freetubeapp.io/">{"FreeTube"}</a>{" app that only has subscription feed(it has recommended videos also but they are not personalized)"}</li>
                    </ul>

                </div>
            </article>
            </div>
        </div>
    }
}
