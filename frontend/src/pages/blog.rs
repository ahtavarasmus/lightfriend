use yew::prelude::*;

#[function_component(Blog)]
pub fn blog() -> Html {
    html! {
        <div class="blog-container">
            <article class="blog-post">
                <header class="blog-header">
                    <h1>{"The other stuff"}</h1>
                    <div class="blog-meta">
                        <span class="blog-date">{"March 26, 2024"}</span>
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
    }
}
