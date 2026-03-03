use yew::prelude::*;
use crate::utils::seo::{use_seo, SeoMeta};

#[function_component(AiSearchSms)]
pub fn ai_search_sms() -> Html {
    use_seo(SeoMeta {
        title: "AI Search via SMS – Search the Web from Any Phone | Lightfriend",
        description: "Search the web with AI from any dumbphone via SMS. Get instant, AI-powered answers from Light Phone, Nokia, or any basic phone.",
        canonical: "https://lightfriend.ai/features/ai-search-sms",
        og_type: "website",
    });

    html! {
        <div class="feature-page">
            <div class="feature-hero">
                <h1>{"AI Search via SMS"}</h1>
                <p class="feature-subtitle">{"Search the web with AI from any phone — get instant answers via SMS"}</p>
            </div>
            <div class="feature-content">
                <section>
                    <h2>{"How It Works"}</h2>
                    <p>{"Web search usually requires a browser and internet connection. With Lightfriend, you can search the web using AI from any phone via SMS. Ask any question and get a concise, AI-powered answer delivered as a text message."}</p>
                    <p>{"Lightfriend uses advanced AI to search the web, synthesize results, and deliver a clear answer. No need to scroll through pages of results — just text your question and get the answer."}</p>
                </section>
                <section>
                    <h2>{"Features"}</h2>
                    <ul>
                        <li>{"AI-powered web search via SMS"}</li>
                        <li>{"Concise, synthesized answers"}</li>
                        <li>{"Real-time information and news"}</li>
                        <li>{"Weather, sports scores, and facts"}</li>
                        <li>{"Restaurant and business lookups"}</li>
                        <li>{"Natural language questions"}</li>
                    </ul>
                </section>
                <section>
                    <h2>{"Compatible Phones"}</h2>
                    <p>{"Works with Light Phone 2 & 3, Nokia flip phones, any basic phone, any flip phone — any phone that can send SMS."}</p>
                </section>
                <section class="feature-cta">
                    <a href="/register" class="cta-button">{"Get AI Search on Your Phone"}</a>
                    <a href="/pricing" class="cta-link">{"View pricing →"}</a>
                </section>
            </div>
        </div>
    }
}
