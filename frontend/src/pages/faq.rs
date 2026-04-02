use crate::utils::seo::{use_seo, SeoMeta};
use crate::Route;
use gloo_timers::callback::Timeout;
use wasm_bindgen::prelude::*;
use web_sys::MouseEvent;
use yew::prelude::*;
use yew::{Children, Properties};
use yew_router::prelude::Link;

#[derive(Clone, PartialEq)]
struct ChatMessage {
    text: String,
    is_user: bool,
}

#[derive(Properties, PartialEq)]
struct FaqItemProps {
    question: String,
    id: String,
    children: Children,
}

#[function_component(FaqItem)]
fn faq_item(props: &FaqItemProps) -> Html {
    let is_open = use_state(|| false);

    // Check URL hash on mount and when hash changes
    {
        let is_open = is_open.clone();
        let id = props.id.clone();

        use_effect_with_deps(
            move |_| {
                let check_hash = move || {
                    if let Some(window) = web_sys::window() {
                        if let Ok(location) = window.location().hash() {
                            if location == format!("#{}", id) {
                                is_open.set(true);
                                // Add a small delay to ensure the content is expanded before scrolling
                                let window_clone = window.clone();
                                let id_clone = id.clone();
                                let timeout = Timeout::new(100, move || {
                                    if let Some(element) = window_clone
                                        .document()
                                        .and_then(|doc| doc.get_element_by_id(&id_clone))
                                    {
                                        element.scroll_into_view_with_bool(true);
                                    }
                                });
                                timeout.forget();
                            }
                        }
                    }
                };

                // Check hash immediately
                check_hash();

                // Set up hash change listener
                let window = web_sys::window().unwrap();
                let callback = Closure::wrap(Box::new(move || {
                    check_hash();
                }) as Box<dyn FnMut()>);

                window
                    .add_event_listener_with_callback(
                        "hashchange",
                        callback.as_ref().unchecked_ref(),
                    )
                    .unwrap();
                callback.forget();

                || ()
            },
            (),
        );
    }

    let toggle = {
        let is_open = is_open.clone();
        Callback::from(move |e: MouseEvent| {
            e.prevent_default();
            is_open.set(!*is_open);
        })
    };
    html! {
        <div id={props.id.clone()} class={classes!("faq-item", if *is_open { "open" } else { "" })}>
            <div class="faq-question-container">
                <button class="faq-question" onclick={toggle}>
                    <span class="question-text">{&props.question}</span>
                    <span class="toggle-icon">{if *is_open { "−" } else { "+" }}</span>
                </button>
                /*
                <button class="copy-link-button" onclick={copy_link} title="Copy link to this question">
                    <span class="link-icon">{"🔗"}</span>
                </button>
                */
            </div>
            <div class="faq-answer">
                { for props.children.iter() }
            </div>
        </div>
    }
}

#[function_component(Faq)]
pub fn faq() -> Html {
    use_seo(SeoMeta {
        title: "FAQ \u{2013} Lightfriend AI Assistant for Dumbphones",
        description: "Frequently asked questions about Lightfriend. Learn how to use WhatsApp, Telegram, Signal, and email on your dumbphone via SMS and voice calls.",
        canonical: "https://lightfriend.ai/faq",
        og_type: "website",
    });

    let chat_messages = use_state(|| Vec::<ChatMessage>::new());
    let is_typing = use_state(|| false);
    let current_demo_index = use_state(|| 0);

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

    // Inject FAQPage structured data for SEO
    {
        use_effect_with_deps(
            move |_| {
                let cleanup_id = "faq-schema-ld";
                if let Some(document) = web_sys::window().and_then(|w| w.document()) {
                    let script = document.create_element("script").unwrap();
                    script.set_attribute("type", "application/ld+json").ok();
                    script.set_attribute("id", cleanup_id).ok();
                    script.set_text_content(Some(r#"{
  "@context": "https://schema.org",
  "@type": "FAQPage",
  "mainEntity": [
    {
      "@type": "Question",
      "name": "What problem does Lightfriend solve?",
      "acceptedAnswer": {
        "@type": "Answer",
        "text": "Lightfriend bridges the gap between minimalist dumbphones and essential digital services. It gives you access to WhatsApp, Telegram, Signal, email, web search, and more via simple SMS and voice calls - no apps or smartphone needed."
      }
    },
    {
      "@type": "Question",
      "name": "Which countries are supported?",
      "acceptedAnswer": {
        "@type": "Answer",
        "text": "Lightfriend offers full service in the US, Canada, UK, Finland, Netherlands, and Australia with local phone numbers. Notification-only mode is available in 30+ countries across Europe and Asia-Pacific. Other countries can use Bring Your Own Twilio number."
      }
    },
    {
      "@type": "Question",
      "name": "Will I be charged extra for replying to messages?",
      "acceptedAnswer": {
        "@type": "Answer",
        "text": "If you're in a country without a local Lightfriend number, replying may incur international SMS rates from your carrier. You can choose which country's number to use in settings to minimize costs. Most value comes from receiving notifications which doesn't require replies."
      }
    },
    {
      "@type": "Question",
      "name": "Can I try the service before signing up?",
      "acceptedAnswer": {
        "@type": "Answer",
        "text": "Yes! The FAQ page includes an interactive demo chat where you can try common requests like checking WhatsApp messages, weather, emails, web search, photo translation, and QR code scanning."
      }
    },
    {
      "@type": "Question",
      "name": "Why choose a dumbphone?",
      "acceptedAnswer": {
        "@type": "Answer",
        "text": "Choosing a dumbphone lets you take back control of your attention. Instead of fighting addictive apps and notifications designed to hijack your focus, you can step away while still having AI-powered access to essential services."
      }
    },
    {
      "@type": "Question",
      "name": "Where can I buy a dumbphone?",
      "acceptedAnswer": {
        "@type": "Answer",
        "text": "Lightfriend works with any basic phone. Start with whatever simple phone you have. For recommendations, visit dumbphones.org. Popular choices include the Light Phone 2 and 3, which offer hotspot, navigation, and camera features."
      }
    },
    {
      "@type": "Question",
      "name": "How do I handle 2FA authentication without a smartphone?",
      "acceptedAnswer": {
        "@type": "Answer",
        "text": "Options include the Step Two Mac app for authenticator codes, YubiKey hardware security keys, and physical code calculator devices from banks. These replace smartphone-based 2FA apps."
      }
    },
    {
      "@type": "Question",
      "name": "How does LightFriend protect my data?",
      "acceptedAnswer": {
        "@type": "Answer",
        "text": "Data is kept minimal, secure, and private. No call recordings. Messages can be optionally stored (encrypted at rest) with up to 10 recent exchanges for context. All sensitive credentials are encrypted. Data is never sold or shared with third parties. The code is open source for self-hosting."
      }
    }
  ]
}"#));
                    if let Some(head) = document.head() {
                        head.append_child(&script).ok();
                    }
                }
                move || {
                    if let Some(document) = web_sys::window().and_then(|w| w.document()) {
                        if let Ok(Some(el)) = document.query_selector(&format!("#{}", cleanup_id)) {
                            el.remove();
                        }
                    }
                }
            },
            (),
        );
    }

    // Define demo conversations
    let demo_conversations = [
        ("Check my WhatsApp messages", "You have 3 new WhatsApp messages:\n\n📱 Mom: \"Don't forget dinner at 7pm\"\n📱 Sarah: \"Great job on the presentation!\"\n📱 Work Group: \"Meeting moved to 3pm tomorrow\""),
        ("What's the weather like?", "🌤️ Today in your location:\n\nTemperature: 72°F (22°C)\nConditions: Partly cloudy\nChance of rain: 20%\n\nPerfect weather for a walk!"),
        ("Any important emails?", "📧 You have 2 important emails:\n\n• From: boss@company.com\n  Subject: \"Q4 Budget Review - Action Required\"\n  Received: 2 hours ago\n\n• From: bank@yourbank.com\n  Subject: \"Account Statement Ready\"\n  Received: 1 hour ago"),
        ("When is the next Light Phone 3 estimated delivery date?", "📱 Light Phone 3 has estimated delivery date in August 2025 if you order today"),
        ("📸 [Image sent] Can you translate this menu?", "I see a Japanese restaurant menu in the image. Here's the translation:\n\n🍜 Ramen Section:\n1. Tonkotsu Ramen - $12\n   Rich pork bone broth\n2. Miso Ramen - $11\n   Traditional fermented soybean paste soup\n3. Shoyu Ramen - $11\n   Classic soy sauce based broth\n\n🍱 Side Dishes:\n• Gyoza (6 pcs) - $6\n• Edamame - $4\n• Karaage (Japanese fried chicken) - $7\n\nWould you like me to explain any of these dishes in more detail?"),
        ("📸 [Image sent of QR code]", "The QR code contains the URL: https://lightfriend.ai (you can now open this on your computer, or if you have a light phone the link gets send automatically to your email)"),
    ];

    let start_demo = {
        let chat_messages = chat_messages.clone();
        let is_typing = is_typing.clone();
        let current_demo_index = current_demo_index.clone();

        Callback::from(move |question_index: usize| {
            if question_index >= demo_conversations.len() {
                return;
            }

            let chat_messages = chat_messages.clone();
            let is_typing = is_typing.clone();
            let current_demo_index = current_demo_index.clone();
            let question = demo_conversations[question_index].0.to_string();
            let answer = demo_conversations[question_index].1.to_string();

            // Clear previous messages and add user message
            let user_message = ChatMessage {
                text: question.clone(),
                is_user: true,
            };
            chat_messages.set(vec![user_message]);
            current_demo_index.set(question_index);

            // Show typing indicator
            is_typing.set(true);

            // Simulate AI response delay
            let timeout = Timeout::new(1500, move || {
                is_typing.set(false);
                let ai_message = ChatMessage {
                    text: answer,
                    is_user: false,
                };
                chat_messages.set(vec![
                    ChatMessage {
                        text: question,
                        is_user: true,
                    },
                    ai_message,
                ]);
            });
            timeout.forget();
        })
    };
    html! {
        <div class="faq-page">
            <div class="faq-background"></div>
            <section class="faq-hero">
                <h1>{"Frequently Asked Questions"}</h1>
                <p>{"Everything you need to know about going light with lightfriend"}</p>
            </section>

            <section class="faq-section">

                <h2>{"Getting Started"}</h2>

                <FaqItem
                    question="What problem does lightfriend solve?"
                    id="lightfriend-solution"
                >
                    <div class="phone-comparison">
                        <div class="comparison-column">
                            <h3>{"Phones with App Store"}</h3>
                            <div class="pros-cons-container">
                                <div class="pros">
                                    <h4>{"Pros"}</h4>
                                    <ul>
                                        <li>{"Messaging apps"}</li>
                                        <li>{"Calendar"}</li>
                                        <li>{"Email"}</li>
                                    </ul>
                                </div>
                                <div class="cons">
                                    <h4>{"Cons"}</h4>
                                    <ul>
                                        <li>{"Highly addictive"}</li>
                                        <li>{"Mental health impact"}</li>
                                        <li>{"Time waste"}</li>
                                        <li>{"Constant distractions"}</li>
                                        <li>{"Sleep disruption"}</li>
                                    </ul>
                                </div>
                            </div>
                        </div>

                        <div class="comparison-column">
                            <h3>{"Phones without App Store"}</h3>
                            <div class="pros-cons-container">
                                <div class="pros">
                                    <h4>{"Pros"}</h4>
                                    <ul>
                                        <li>{"No addiction"}</li>
                                        <li>{"More presence"}</li>
                                        <li>{"Better sleep"}</li>
                                    </ul>
                                </div>
                                <div class="cons">
                                    <h4>{"Cons"}</h4>
                                    <ul>
                                        <li>{"No messaging apps"}</li>
                                        <li>{"No email"}</li>
                                        <li>{"No Qr code reader"}</li>
                                        <li>{"Usually no calendar"}</li>
                                        <li>{"Can feel disconnected"}</li>
                                    </ul>
                                </div>
                            </div>
                        </div>

                        <div class="solution-column">
                            <h3>{"Phone without App Store + LightFriend"}</h3>
                            <div class="solution-benefits">
                                <ul>
                                    <li>{"✨ Access essential communication channels through voice and text"}</li>
                                    <li>{"✨ Internet search without scrolling"}</li>
                                    <li>{"✨ Better focus and mental health"}</li>
                                    <li>{"✨ Stay connected on your terms"}</li>
                                </ul>
                            </div>
                        </div>
                    </div>
                </FaqItem>

                <FaqItem
                    question="Which countries are supported?"
                    id="supported-countries"
                >
                    <p>{"Lightfriend can be made to work globally, but all features may not work everywhere."}</p>
                    <h3>{"Full Service Countries"}</h3>
                    <p>{"For people in the US and Canada, lightfriend phone number and messages are included."}</p>
                    <p>{"If you live in Finland, UK, Netherlands or Australia, lightfriend provides a phone number, but messages have to be bought separately beforehand."}</p>
                    <h3>{"Notification-Only Countries"}</h3>
                    <p>{"If you live in Germany, France, Spain, Italy, Portugal, Belgium, Austria, Switzerland, Poland, Czech Republic, Sweden, Denmark, Norway, Ireland, or New Zealand, you can use lightfriend in notification-only mode. This means you'll receive messages from a US number and can reply to it, but you won't get your own local phone number. Message costs are based on Twilio's international rates for your country."}</p>
                    <h3>{"Other Countries"}</h3>
                    <p>{"Elsewhere you will have to bring your own Twilio number or if you have an extra android phone with extra phone plan laying around, you can use it to send and receive sms messages through it without extra costs. See your country's Twilio pricing and regulations from "}<a href="/bring-own-number">{"here"}</a> {" or ask about the service availability in your country by emailing "}<a href="mailto:rasmus@lightfriend.ai">{"rasmus@lightfriend.ai."}</a></p>

                </FaqItem>

                <FaqItem
                    question="Will I be charged extra for replying to messages?"
                    id="international-sms-rates"
                >
                    <p>{"If you're in a country without a local Lightfriend number (see above), you'll receive messages from a foreign number - typically US or UK. While receiving messages works the same everywhere, replying may incur international SMS rates from your mobile carrier."}</p>
                    <h3>{"Reducing Costs"}</h3>
                    <ul>
                        <li>{"You can choose which country's number to use in your settings - options include US, Canada, UK, Finland, Netherlands, and Australia. For example, if you're in Europe, a UK number may be cheaper to text than a US number."}</li>
                        <li>{"Most of Lightfriend's value comes from receiving notifications - message summaries and alerts - which doesn't require you to reply."}</li>
                    </ul>
                    <h3>{"Check Your Carrier's Rates"}</h3>
                    <p>{"International SMS rates vary by carrier. Contact your mobile provider to understand what you'd pay to text a US or UK number, then choose the cheaper option in your Lightfriend settings."}</p>
                </FaqItem>

                <FaqItem
                    question="Can I try the service before signing up?"
                    id="try-service"
                >
                    <p>{"Yes! Try our demo chat below to see how LightFriend responds to common requests:"}</p>

                    <div class="demo-chat-container">
                        <div class="phone-demo">
                            <div class="phone-screen">
                                <div class="phone-header">
                                    <div class="phone-status">
                                        <span>{"9:41"}</span>
                                        <span>{"100%"}</span>
                                    </div>
                                    <div class="chat-header">
                                        <span class="contact-name">{"lightfriend"}</span>
                                    </div>
                                </div>
                                <div class="chat-messages">
                                    {if chat_messages.is_empty() {
                                        html! {
                                            <div class="welcome-message">
                                                <p>{"Try a demo message below 👇"}</p>
                                            </div>
                                        }
                                    } else {
                                        html! {
                                            <>
                                                {for chat_messages.iter().map(|msg| {
                                                    let class = if msg.is_user { "user" } else { "ai" };
                                                    html! {
                                                        <div class={if msg.is_user { "user-message" } else { "ai-message" }}>
                                                            <div class={format!("message-bubble {}", class)}>
                                                                {&msg.text}
                                                            </div>
                                                        </div>
                                                    }
                                                })}
                                                {if *is_typing {
                                                    html! {
                                                        <div class="ai-message">
                                                            <div class="message-bubble typing">
                                                                <div class="typing-indicator">
                                                                    <span></span>
                                                                    <span></span>
                                                                    <span></span>
                                                                </div>
                                                            </div>
                                                        </div>
                                                    }
                                                } else {
                                                    html! {}
                                                }}
                                            </>
                                        }
                                    }}
                                </div>
                            </div>
                        </div>
                        <div class="demo-controls">
                            <h3>{"Try these examples:"}</h3>
                            <div class="demo-questions">
                                {for (0..demo_conversations.len()).map(|i| {
                                    let start_demo = start_demo.clone();
                                    let onclick = Callback::from(move |_| start_demo.emit(i));
                                    html! {
                                        <button class="demo-question" onclick={onclick}>
                                            {demo_conversations[i].0}
                                        </button>
                                    }
                                })}
                            </div>
                        </div>
                    </div>
                </FaqItem>

                <h2>{"How It Works"}</h2>

                <FaqItem
                    question="What can lightfriend actually do?"
                    id="what-can-it-do"
                >
                    <p>{"Lightfriend bridges your dumbphone to the modern world via SMS and voice calls. Here's what you get:"}</p>
                    <ul>
                        <li><strong>{"Messaging bridges:"}</strong>{" Connect WhatsApp, Telegram, and Signal. Receive and reply to messages from any basic phone."}</li>
                        <li><strong>{"Email:"}</strong>{" Read and respond to emails without an app."}</li>
                        <li><strong>{"Critical notifications:"}</strong>{" AI screens your incoming messages and only alerts you about genuinely urgent or important things."}</li>
                        <li><strong>{"Smart digests:"}</strong>{" Instead of constant pings, get a summary of what happened while you were away - delivered at times that suit you."}</li>
                        <li><strong>{"Tracked items:"}</strong>{" AI automatically detects deliveries, invoices, deadlines, and commitments from your messages and tracks them for you."}</li>
                        <li><strong>{"Rule builder:"}</strong>{" Create your own automation rules - 'if this happens, do that'. Keyword triggers, AI conditions, scheduled actions, tool calls."}</li>
                        <li><strong>{"Web search:"}</strong>{" Ask any question and get a concise answer, no scrolling."}</li>
                        <li><strong>{"Image understanding:"}</strong>{" Send a photo of a menu, sign, or QR code and get it translated or read back to you."}</li>
                        <li><strong>{"Voice calls:"}</strong>{" For urgent notifications, lightfriend can call you instead of texting."}</li>
                        <li><strong>{"Learns over time:"}</strong>{" The system builds context about your life - who matters to you, what's urgent, what can wait. It gets better at filtering and prioritizing the longer you use it."}</li>
                        <li><strong>{"Tesla integration:"}</strong>{" Lock, unlock, climate control, and check your Tesla's status via SMS."}</li>
                        <li><strong>{"MCP server support:"}</strong>{" Connect external tools and services via the Model Context Protocol. Extend what lightfriend can do with custom integrations."}</li>
                        <li><strong>{"Minimalist YouTube:"}</strong>{" A built-in player with no algorithm, no recommendations, no rabbit holes. Just search and play."}</li>
                    </ul>
                </FaqItem>

                <FaqItem
                    question="How do critical notifications work?"
                    id="critical-notifications"
                >
                    <p>{"When a message arrives on WhatsApp, Telegram, Signal, or email, the AI evaluates whether it needs your immediate attention. If it's urgent - a server going down, a family emergency, a time-sensitive request - you get notified right away via SMS or even a phone call."}</p>
                    <p>{"Non-urgent messages get batched into your digest instead of interrupting your day. You stay in control of what counts as critical."}</p>
                </FaqItem>

                <FaqItem
                    question="What are tracked items?"
                    id="tracked-items"
                >
                    <p>{"When someone says 'I'll send you the invoice by Friday' or you get a shipping notification, the AI automatically creates a tracked item with a due date. These show up on your dashboard so nothing falls through the cracks."}</p>
                    <p>{"Tracked items include delivery updates, payment deadlines, follow-up commitments, booking confirmations - anything concrete with a timeline that you'd want to remember."}</p>
                </FaqItem>

                <FaqItem
                    question="What's the rule builder?"
                    id="rule-builder"
                >
                    <p>{"The rule builder lets you create custom automations. Each rule has three parts:"}</p>
                    <ul>
                        <li><strong>{"WHEN:"}</strong>{" A trigger - either a schedule (daily at 9am) or an event (new message arrives)."}</li>
                        <li><strong>{"IF:"}</strong>{" A condition - always, keyword match, or an AI-evaluated condition you write in plain English."}</li>
                        <li><strong>{"THEN:"}</strong>{" An action - send an SMS/call notification, send a chat message, or trigger a tool."}</li>
                    </ul>
                    <p>{"Example: 'Every day at 9am, summarize my unread messages and text me.' Or: 'When a WhatsApp message mentions a meeting, notify me immediately.'"}</p>
                </FaqItem>

                <h2>{"Why Go Light?"}</h2>


                <FaqItem
                    question="Why choose a dumbphone?"
                    id="why-dumbphone"
                >
                    <p>
                        {"Your time is precious - why waste it fighting an endless battle against notifications and addictive apps? While tech giants deploy armies of experts to hijack your focus, there's a simpler path: stepping away. Choosing a dumbphone isn't about going backwards, you'll still have cutting edge AI at your fingertips. It's about taking back control of your attention and living life as its protagonist rather than watching it pass by through a screen."}
                    </p>
                    <img src="/assets/squidwardlookingwindow.webp" loading="lazy" alt="Squidward looking through window metaphor" class="faq-image" />
                    <p>
                        {"Like Squidward on his window, many of us find ourselves looking out at life from behind our screens. We see others living, connecting, and experiencing the world firsthand, while we remain observers, separated by a digital barrier."}
                    </p>
                </FaqItem>

                <FaqItem
                    question="What about the impact on relationships?"
                    id="relationships-impact"
                >
                    <img src="/assets/kid_draws_mom.jpg" loading="lazy"  alt="Child drawing mother on phone" class="faq-image" />
                    <p>
                        {"A child's drawing tells a thousand words. When asked to draw their parents, more and more children depict them with phones in hand – a powerful reflection of how our digital habits affect those around us."}
                    </p>
                    <p>
                        {"This isn't the legacy we want to leave. It's not the presence we want to embody. Our children deserve parents who are present, engaged, and available – not just physically, but mentally and emotionally too."}
                    </p>
                </FaqItem>

                <FaqItem
                    question="What's the value of boredom?"
                    id="value-of-boredom"
                >
                    <img src="/assets/boredom.webp" loading="lazy" alt="Illustration of creative boredom" class="faq-image" />
                    <p>
                        {"Remember when being bored meant letting your mind wander, leading to unexpected bursts of creativity and self-discovery? Today's smartphones have eliminated these precious moments of 'empty time' - replacing them with endless scrolling and constant stimulation."}
                    </p>
                    <p>
                        {"Boredom isn't your enemy – it's the canvas for creativity, the spark for innovation, and the space where your best ideas are born. When you're constantly entertained, you lose those moments of reflection that lead to personal growth and creative breakthroughs."}
                    </p>
                </FaqItem>

                <h2>{"Practical Solutions"}</h2>

                <FaqItem
                    question="Where can I buy a dumbphone?"
                    id="buy-dumbphone"
                >
                    <h3>{"Start with what you have"}</h3>
                    <p>
                        {"Lightfriend service is phone-agnostic - it works with any basic phone capable of calling and texting. We strongly recommend starting with whatever simple phone you already have, even if it's an old flip phone in your drawer."}
                    </p>

                    <h3>{"Ready to commit?"}</h3>
                    <p>
                        {"If you've tried the minimalist phone life and want to continue, "}<a href="https://dumbphones.org">{"dumbphones.org"}</a>{" is an excellent resource for comparing different models based on your needs."}
                    </p>

                    <h3>{"The Light Phone Option"}</h3>
                    <p>
                        {"While not necessary for using LightFriend, the "}<a href="https://www.thelightphone.com">{"Light Phone 2 and 3"}</a>{" are popular choices among our users. They offer features like:"}
                    </p>
                    <ul>
                        <li>{"Hotspot capability for sharing internet to your computer"}</li>
                        <li>{"Built-in navigation maps"}</li>
                        <li>{"Camera (Light Phone 3 only) for QR codes and translations"}</li>
                    </ul>
                </FaqItem>

                <FaqItem
                    question="How do I handle 2FA authentication?"
                    id="handle-2fa"
                >
                    <h3>{"'Step Two' mac app"}</h3>
                    <p>{"It is very fast and simple. It's free for certain number of accounts and then small one time payment for unlimited."}</p>
                    <img src="/assets/StepTwo.png" loading="lazy" alt="Step Two app" class="faq-image" />

                    <h3>{"Yubikey"}</h3>
                    <p>{"Can be used inplace of authenticator apps."}</p>
                    <img src="/assets/Yubikey.png" alt="Yubikey" loading="lazy" class="faq-image" />

                    <h3>{"Physical Code Calculator Device"}</h3>
                    <p>{"Most banks have it and it's used for bank login."}</p>
                    <img src="/assets/nordea_code_calc.png" loading="lazy" alt="Nordea code calculator" class="faq-image" />
                </FaqItem>

                <FaqItem
                    question="How do I handle commuting and navigation?"
                    id="commuting-navigation"
                >
                    <h3>{"Airport"}</h3>
                    <p>{"Get printed boarding passes and use computer to check flight times. With some airlines you can also get gate changes texted to you."}</p>

                    <h3>{"Bus"}</h3>
                    <p>{"If you use bus in your home town, ask for physical keycard which can be loaded with credits."}</p>

                    <h3>{"Taxi & Ridesharing"}</h3>
                    <p>{"In US, Canada and UK there is "}<a href="https://www.tremp.me/">{"Tremp."}</a></p>

                    <h3>{"Maps"}</h3>
                    <p>{"Options include physical paper map, maps on your computer, or get a phone that has maps like "}<a href="https://www.thelightphone.com/">{"the Light Phone."}</a>{" While you might still get lost occasionally, that's part of the adventure:)."}</p>
                </FaqItem>

                <FaqItem
                    question="What tools can help me stay focused?"
                    id="focus-tools"
                >
                    <ul>
                        <li><a href="https://getcoldturkey.com/">{"Cold Turkey App Blocker"}</a>{" is great for website and computer app blocking. It is very strong so be careful though not to lock yourself out of your computer:D"}</li>
                        <li>{"Amazon kindle has small simple text based browser, which can be used for reading website blogs."}</li>
                        <li>{"If you want to watch some youtube videos on your computer, there's "}<a href="https://freetubeapp.io/">{"FreeTube"}</a>{" app that only has subscription feed(it has recommended videos also but they are not personalized)"}</li>
                    </ul>
                </FaqItem>

                <h2>{"Privacy & Security"}</h2>
                <FaqItem
                    question="How does LightFriend protect my data?"
                    id="data-protection"
                    >
                    <p>{"Lightfriend runs inside an AWS Nitro Enclave - a hardware-isolated environment where even the server operator cannot access your data while it's being processed. This is verifiable, not just a promise."}</p>
                    <ul>
                        <li><strong>{"Nitro Enclave:"}</strong>{" Your data is processed inside a cryptographically attested enclave. No SSH access, no debugging ports, no way for anyone (including the developer) to peek inside while it's running. See the "}<Link<Route> to={Route::TrustChain}>{"Trust Chain"}</Link<Route>>{" page for the full verification."}</li>
                        <li><strong>{"Encryption:"}</strong>{" All data is encrypted at rest (AES-256-GCM). Backups are encrypted before leaving the enclave. Credentials and sensitive fields use per-field encryption."}</li>
                        <li><strong>{"Calls:"}</strong>{" No recordings. Just anonymous metrics to improve service."}</li>
                        <li><strong>{"Messages:"}</strong>{" Your messages never exist as plain text outside the enclave. They are processed, stored, and backed up entirely within the hardware-isolated environment. See "}<Link<Route> to={Route::Trustless}>{"Verifiably Private"}</Link<Route>>{" and "}<Link<Route> to={Route::TrustChain}>{"Trust Chain"}</Link<Route>>{" for the full architecture."}</li>
                        <li><strong>{"Open source:"}</strong>{" The entire codebase is open source (AGPLv3) on GitHub. You can verify what runs inside the enclave, or self-host it."}</li>
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
                    background-image: url('/assets/rain.gif');
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
                    margin-top: 2rem;
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

                .faq-question-container {
                    display: flex;
                    align-items: center;
                    width: 100%;
                }

                .copy-link-button {
                    background: none;
                    border: none;
                    color: #666;
                    padding: 8px;
                    cursor: pointer;
                    opacity: 0;
                    transition: opacity 0.3s ease;
                }

                .faq-question-container:hover .copy-link-button {
                    opacity: 1;
                }

                .copy-link-button:hover {
                    color: #7EB2FF;
                }

                .link-icon {
                    font-size: 1rem;
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
                    max-height: 3000px;
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

                /* Demo Chat Styling */
                .demo-chat-container {
                    display: flex;
                    flex-direction: column;
                    align-items: center;
                    gap: 2rem;
                    margin: 2rem 0;
                    padding: 1rem;
                }

                .phone-demo {
                    width: 280px;
                    height: 480px;
                    background: #2a2a2a;
                    border-radius: 20px;
                    position: relative;
                    padding: 40px 20px;
                    box-shadow: 0 25px 50px -12px rgba(0, 0, 0, 0.5);
                    border: 2px solid #444;
                    overflow: hidden;
                }

                .phone-demo::before {
                    content: '';
                    position: absolute;
                    top: 15px;
                    left: 50%;
                    transform: translateX(-50%);
                    width: 50px;
                    height: 5px;
                    background: #444;
                    border-radius: 3px;
                }

                .phone-screen {
                    background: #001a1a;
                    height: 280px;
                    width: 240px;
                    border-radius: 5px;
                    overflow: hidden;
                    display: flex;
                    flex-direction: column;
                    border: 3px solid #444;
                    box-shadow: inset 0 0 10px rgba(0, 255, 255, 0.1);
                }

                .phone-header {
                    background: rgba(0, 0, 0, 0.9);
                    padding: 10px;
                    border-bottom: 1px solid #333;
                }

                .phone-status {
                    display: flex;
                    justify-content: space-between;
                    color: #fff;
                    font-size: 0.8rem;
                    margin-bottom: 10px;
                }

                .chat-header {
                    display: flex;
                    flex-direction: column;
                    align-items: center;
                    padding: 5px 0;
                }

                .contact-name {
                    color: #fff;
                    font-weight: bold;
                    font-size: 1.1rem;
                }

                .contact-status {
                    color: #666;
                    font-size: 0.8rem;
                }

                .chat-messages {
                    flex: 1;
                    overflow-y: auto;
                    padding: 15px;
                    display: flex;
                    flex-direction: column;
                    gap: 10px;
                    background: #000;
                }

                .message-bubble img {
                    max-width: 100%;
                    border-radius: 8px;
                    margin: 5px 0;
                }

                .chat-messages::-webkit-scrollbar {
                    width: 6px;
                }

                .chat-messages::-webkit-scrollbar-track {
                    background: transparent;
                }

                .chat-messages::-webkit-scrollbar-thumb {
                    background: #333;
                    border-radius: 3px;
                }

                .welcome-message {
                    text-align: center;
                    color: #666;
                    padding: 20px;
                }

                .message-bubble {
                    max-width: 80%;
                    padding: 10px 15px;
                    border-radius: 15px;
                    font-size: 0.9rem;
                    line-height: 1.4;
                    white-space: pre-wrap;
                }

                .user-message {
                    align-self: flex-end;
                }

                .ai-message {
                    align-self: flex-start;
                }

                .user .message-bubble {
                    background: #1E90FF;
                    color: white;
                    border-bottom-right-radius: 5px;
                }

                .ai .message-bubble {
                    background: #333;
                    color: white;
                    border-bottom-left-radius: 5px;
                }

                .typing .message-bubble {
                    background: #333;
                    padding: 15px;
                }

                .typing-indicator {
                    display: flex;
                    gap: 4px;
                }

                .typing-indicator span {
                    width: 8px;
                    height: 8px;
                    background: #666;
                    border-radius: 50%;
                    animation: typing 1s infinite;
                }

                .typing-indicator span:nth-child(2) {
                    animation-delay: 0.2s;
                }

                .typing-indicator span:nth-child(3) {
                    animation-delay: 0.4s;
                }

                @keyframes typing {
                    0%, 100% { transform: translateY(0); }
                    50% { transform: translateY(-5px); }
                }

                .demo-controls {
                    width: 100%;
                    max-width: 400px;
                }

                .demo-controls h3 {
                    text-align: center;
                    margin-bottom: 1rem;
                    color: #fff;
                }

                .demo-questions {
                    display: grid;
                    gap: 10px;
                }

                .demo-question {
                    background: rgba(30, 144, 255, 0.1);
                    border: 1px solid rgba(30, 144, 255, 0.3);
                    color: #fff;
                    padding: 12px;
                    border-radius: 8px;
                    cursor: pointer;
                    transition: all 0.3s ease;
                    text-align: left;
                }

                .demo-question:hover {
                    background: rgba(30, 144, 255, 0.2);
                    border-color: rgba(30, 144, 255, 0.5);
                }

                /* Phone Comparison Styling */
                .phone-comparison {
                    display: grid;
                    grid-template-columns: 1fr;
                    gap: 2rem;
                    margin: 2rem 0;
                    width: 100%;
                }

                .comparison-column {
                    background: rgba(0, 0, 0, 0.3);
                    border-radius: 12px;
                    padding: 1.5rem;
                    border: 1px solid rgba(30, 144, 255, 0.2);
                }

                .comparison-column h3 {
                    text-align: center;
                    margin-bottom: 1.5rem;
                    color: #fff;
                }

                .pros-cons-container {
                    display: grid;
                    gap: 1.5rem;
                }

                .pros, .cons {
                    padding: 1rem;
                    border-radius: 8px;
                }

                .pros h4 {
                    color: #4CAF50;
                    margin-bottom: 0.5rem;
                }

                .cons h4 {
                    color: #f44336;
                    margin-bottom: 0.5rem;
                }

                .pros ul, .cons ul {
                    list-style: none;
                    padding: 0;
                    margin: 0;
                }

                .pros li, .cons li {
                    padding: 0.5rem 0;
                    color: #999;
                    position: relative;
                    padding-left: 1.5rem;
                }

                .pros li::before {
                    content: '✓';
                    color: #4CAF50;
                    position: absolute;
                    left: 0;
                }

                .cons li::before {
                    content: '×';
                    color: #f44336;
                    position: absolute;
                    left: 0;
                }

                .solution-column {
                    background: linear-gradient(145deg, rgba(30, 144, 255, 0.1), rgba(30, 144, 255, 0.2));
                    border-radius: 12px;
                    padding: 1.5rem;
                    border: 1px solid rgba(30, 144, 255, 0.3);
                    justify-self: center;
                    width: 100%;
                    max-width: 600px;
                }

                .solution-column h3 {
                    text-align: center;
                    margin-bottom: 1.5rem;
                    color: #7EB2FF;
                }

                .solution-benefits ul {
                    list-style: none !important;
                    padding: 0;
                    margin: 0;
                }

                .solution-benefits li {
                    padding: 0.75rem 0;
                    list-style: none !important;
                    color: #fff;
                    text-align: left;
                }

                .notification-demo-container {
                    position: relative;
                    margin-top: 1rem;
                }

                @media (min-width: 768px) {
                    .phone-comparison {
                        grid-template-columns: 1fr 1fr;
                    }

                    .solution-column {
                        grid-column: 1 / -1;
                        margin-top: 1rem;
                    }
                }

                @media (max-width: 768px) {
                    .demo-chat-container {
                        padding: 0;
                    }

                    .phone-demo {
                        width: 100%;
                        max-width: 280px;
                        height: 480px;
                    }

                    .phone-screen {
                        width: calc(100% - 40px);
                        height: 280px;
                    }

                    .demo-controls {
                        padding: 0 1rem;
                    }
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
