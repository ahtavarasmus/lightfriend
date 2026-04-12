use crate::utils::seo::{use_seo, SeoMeta};
use crate::Route;
use yew::prelude::*;
use yew_router::components::Link;

#[function_component(PromptInjectionSafe)]
pub fn prompt_injection_safe() -> Html {
    use_seo(SeoMeta {
        title: "Why Lightfriend Can't Be Prompt Injected - AI Assistant Without Write Access",
        description: "Lightfriend is an AI assistant with no write permissions by default. It can only read your messages and notify you. Actions require your explicit approval. This makes prompt injection a non-issue.",
        canonical: "https://lightfriend.ai/prompt-injection-safe",
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
                <h1>{"Why Lightfriend Can't Be Prompt Injected"}</h1>
                <p>{"Most AI assistants are powerful enough to be dangerous. Lightfriend is powerful enough to be useful."}</p>
            </section>
            <section class="blog-content">
                <h2>{"The Problem With Autonomous AI Assistants"}</h2>
                <p>{"AI assistants are getting more capable. You can now connect your email, messaging apps, calendar, and files to an AI agent that acts on your behalf - reading your messages, drafting replies, scheduling meetings, sending emails."}</p>
                <p>{"The problem: if the AI can do all of that autonomously, so can anyone who tricks it."}</p>
                <p>{"Prompt injection is when a malicious message - hidden in an email, a chat message, or a webpage - hijacks the AI into doing something the user never intended. If your AI assistant has write access to your email, a prompt injection can make it forward sensitive messages to an attacker. If it has access to your messaging apps and can send messages autonomously, a crafted message in a group chat can make it send your private data to someone else."}</p>
                <p>{"This isn't theoretical. It's the reason many security-conscious people don't trust AI assistants with their real data, even when the AI itself is useful."}</p>

                <h2>{"Lightfriend's Approach: Read-Only by Default"}</h2>
                <p>{"Lightfriend takes a fundamentally different approach. By default, the AI has no write permissions. It can read your messages across WhatsApp, Telegram, Signal, and email. It can analyze them, understand context, and figure out what's important. But it cannot act on your behalf."}</p>
                <p>{"The only thing Lightfriend can do without your permission is send you an SMS notification. That's it. The worst case scenario of a prompt injection against Lightfriend is: you get an unnecessary text message to your own phone."}</p>
                <p>{"Compare that to the worst case with a fully autonomous AI assistant: your private messages forwarded to strangers, emails sent in your name, meetings scheduled you didn't want, data exfiltrated to unknown endpoints."}</p>

                <h2>{"Not a Limitation. A Design Decision."}</h2>
                <p>{"This isn't Lightfriend being less capable. It's Lightfriend being more thoughtful about how capability should work."}</p>
                <p>{"Think about how a real-life assistant works. A good assistant doesn't act on your behalf and pretend to know what you want. They handle incoming information, figure out what's important, bring it to your attention, and wait for your decision. \"Your partner called, sounds urgent - want me to call them back?\" Not: \"Your partner called, I already replied for you.\""}</p>
                <p>{"That's exactly how Lightfriend works:"}</p>
                <ol>
                    <li><strong>{"Observe"}</strong>{" - Lightfriend reads your connected messaging apps and email, learning who matters to you, how you communicate, what's routine and what's unusual"}</li>
                    <li><strong>{"Analyze"}</strong>{" - When something seems important, Lightfriend figures out why and what you might want to do about it"}</li>
                    <li><strong>{"Notify"}</strong>{" - You get an SMS with the context you need to make a decision"}</li>
                    <li><strong>{"Wait"}</strong>{" - You decide what to do. If you want Lightfriend to reply, you tell it what to say. Only then does it execute"}</li>
                </ol>
                <p>{"Every action that affects the outside world - sending a message, replying to an email - is gated behind your explicit approval. This gate is enforced by deterministic code, not by trusting the AI to do the right thing."}</p>

                <h2>{"Why This Matters More Than You Think"}</h2>
                <p>{"Most AI security discussions focus on whether the AI model is aligned or whether the prompt is well-crafted. Those matter, but they're the wrong layer to solve the problem at."}</p>
                <p>{"Prompt injection works because AI models are fundamentally susceptible to it - they can't reliably distinguish instructions from data. No amount of prompt engineering fully fixes this. The real fix is architectural: don't give the AI the ability to do damage in the first place."}</p>
                <p>{"Lightfriend's architecture means:"}</p>
                <ul>
                    <li>{"A malicious message in a Telegram group can't make Lightfriend leak your private chats - it has no way to send data anywhere except as an SMS to you"}</li>
                    <li>{"A crafted email can't trick Lightfriend into replying with sensitive information - it can't reply to anything without you saying so"}</li>
                    <li>{"A phishing message can't hijack your assistant into performing actions - there are no autonomous actions to hijack"}</li>
                </ul>
                <p>{"You don't need to trust the AI model to be robust against prompt injection. You just need to trust the code that controls what the AI is allowed to do. And that code is open source."}</p>

                <h2>{"What About Custom Rules?"}</h2>
                <p>{"Lightfriend does support autonomous actions through custom rules. Users can create WHEN/IF/THEN automation blocks - for example, \"WHEN a message arrives from my partner, IF it mentions the kids, THEN forward it to me immediately.\""}</p>
                <p>{"Rules are off by default. When active, they execute specific, user-defined actions - not open-ended AI decisions. An attacker would need to know the exact rules a user has configured to even attempt to exploit them, and the actions are constrained to what the rule specifies."}</p>
                <p>{"This is the right tradeoff: most of what people want an assistant to do doesn't require full autonomy. For the cases that do, rules give you targeted automation with a clear, auditable scope."}</p>

                <h2>{"The Enclave Layer"}</h2>
                <p>{"Architecture alone doesn't solve everything. Even a read-only assistant handles sensitive data - your messages, your contacts, your communication patterns. If the server is compromised, that data is exposed."}</p>
                <p>{"That's why Lightfriend runs inside an AWS Nitro Enclave - a hardware-isolated computing environment that nobody can access, not even the developer. Your data is encrypted with keys that only exist inside the enclave. The code running inside is cryptographically attested and verifiable by anyone."}</p>
                <p>{"So you get two layers of protection:"}</p>
                <ol>
                    <li><strong>{"Architectural"}</strong>{" - The AI can't act on your behalf without permission (prevents prompt injection damage)"}</li>
                    <li><strong>{"Cryptographic"}</strong>{" - Nobody can access your data even if they wanted to (prevents data breaches)"}</li>
                </ol>
                <p>{"For the full technical explanation: "}<Link<Route> to={Route::Trustless}>{"How Lightfriend keeps your data private"}</Link<Route>>{"."}</p>

                <h2>{"Summary"}</h2>
                <p>{"Most AI assistants are designed to be as autonomous as possible. Lightfriend is designed to be as useful as possible without being dangerous. It reads everything, understands context, notices what matters - then tells you about it and waits for your call. Actions are gated behind your explicit approval, enforced by code, not by hoping the AI won't get tricked."}</p>
                <p>{"This means you can connect your real messaging apps and email to Lightfriend without worrying about what happens when - not if - someone tries to manipulate the AI."}</p>

                <div class="blog-cta">
                    <h3>{"An AI Assistant You Can Trust With Your Real Data"}</h3>
                    <Link<Route> to={Route::Pricing} classes="forward-link">
                        <button class="hero-cta">{"See Plans"}</button>
                    </Link<Route>>
                    <p>{"Read-only by default. Open source. Hardware-encrypted."}</p>
                </div>
            </section>
            <style>
                {include_str!("blog_styles.css")}
            </style>
        </div>
    }
}
