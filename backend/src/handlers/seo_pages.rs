use axum::{
    extract::Path,
    http::{HeaderMap, StatusCode},
    response::Html,
};

/// Check if the User-Agent belongs to a known bot/crawler
pub fn is_bot(user_agent: &str) -> bool {
    let ua = user_agent.to_lowercase();
    let bot_patterns = [
        "googlebot",
        "bingbot",
        "gptbot",
        "chatgpt-user",
        "claudebot",
        "anthropic-ai",
        "perplexitybot",
        "twitterbot",
        "facebookexternalhit",
        "linkedinbot",
        "applebot",
        "applebot-extended",
        "google-extended",
        "amazonbot",
        "cohere-ai",
        "meta-externalagent",
        "bytespider",
        "youbot",
        "slurp",
        "duckduckbot",
        "baiduspider",
        "yandexbot",
        "sogou",
        "ia_archiver",
        "semrushbot",
        "ahrefsbot",
        "dotbot",
        "rogerbot",
        "screaming frog",
        "mj12bot",
    ];
    bot_patterns.iter().any(|p| ua.contains(p))
}

struct SeoPage {
    title: &'static str,
    description: &'static str,
    canonical: &'static str,
    og_type: &'static str,
    og_image: &'static str,
    json_ld: String,
    body_content: String,
    lang: &'static str,
}

fn render_page(page: SeoPage) -> Html<String> {
    let html = format!(
        r#"<!DOCTYPE html>
<html lang="{lang}">
<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{title}</title>
    <meta name="description" content="{description}">
    <meta name="robots" content="index, follow">
    <link rel="canonical" href="{canonical}">
    <meta property="og:title" content="{title}">
    <meta property="og:description" content="{description}">
    <meta property="og:url" content="{canonical}">
    <meta property="og:type" content="{og_type}">
    <meta property="og:image" content="{og_image}">
    <meta property="og:site_name" content="Lightfriend">
    <meta property="og:locale" content="en_US">
    <meta name="twitter:card" content="summary_large_image">
    <meta name="twitter:title" content="{title}">
    <meta name="twitter:description" content="{description}">
    <meta name="twitter:image" content="{og_image}">
    <link rel="icon" type="image/png" href="/assets/fav.png">
    {json_ld}
</head>
<body>
    <nav><a href="/">Lightfriend</a> | <a href="/pricing">Pricing</a> | <a href="/faq">FAQ</a> | <a href="/blog">Blog</a></nav>
    <main>
    {body_content}
    </main>
    <footer>
        <p>&copy; 2024-2026 Lightfriend. <a href="/terms">Terms</a> | <a href="/privacy">Privacy</a> | <a href="/supported-countries">Supported Countries</a></p>
        <p>Open source: <a href="https://github.com/ahtavarasmus/lightfriend">GitHub</a> | Contact: <a href="mailto:support@lightfriend.ai">support@lightfriend.ai</a></p>
    </footer>

</body>
</html>"#,
        lang = page.lang,
        title = page.title,
        description = page.description,
        canonical = page.canonical,
        og_type = page.og_type,
        og_image = page.og_image,
        json_ld = page.json_ld,
        body_content = page.body_content,
    );
    Html(html)
}

fn org_json_ld() -> String {
    r#"<script type="application/ld+json">
    {"@context":"https://schema.org","@type":"Organization","name":"Lightfriend","url":"https://lightfriend.ai","logo":"https://lightfriend.ai/assets/fav.png","description":"AI assistant that makes dumbphones smart. Access messaging apps, email, calendar, and more via SMS and voice calls.","sameAs":["https://x.com/ahtavarasm_us","https://github.com/ahtavarasmus/lightfriend"],"contactPoint":{"@type":"ContactPoint","email":"rasmus@ahtava.com","contactType":"customer support"}}
    </script>"#.to_string()
}

fn software_json_ld() -> String {
    r#"<script type="application/ld+json">
    {"@context":"https://schema.org","@type":"SoftwareApplication","name":"Lightfriend","operatingSystem":"Any phone with SMS/calling capability","applicationCategory":"CommunicationApplication","description":"AI assistant for dumbphone users. Access WhatsApp, email, calendar, and web search via SMS and voice calls without needing a smartphone.","url":"https://lightfriend.ai","offers":{"@type":"AggregateOffer","url":"https://lightfriend.ai/pricing","priceCurrency":"USD","lowPrice":"19.00","highPrice":"29.00","offerCount":"2"},"featureList":["Voice calling interface","SMS chat interface","WhatsApp integration","Telegram integration","Signal integration","Email integration","Google Calendar","Web search via AI","Weather forecasts","Turn-by-turn directions","Photo analysis and translation","QR code scanning","Tesla vehicle control","24/7 critical message monitoring","Morning, day, and evening digests","Priority sender notifications"]}
    </script>"#.to_string()
}

const OG_IMAGE: &str =
    "https://lightfriend.ai/assets/boy_holding_dumbphone_in_crowded_place.png?v=1";

// ─── Landing Page ───

pub async fn landing() -> Html<String> {
    render_page(SeoPage {
        title: "Lightfriend: The Best AI Assistant for Dumbphones \u{2013} WhatsApp, Telegram, Signal, Email & More",
        description: "AI assistant for dumbphones like Light Phone 3, Nokia flip phones, and other minimalist phones. Access WhatsApp, Telegram, Signal, email, calendar, AI search, and GPS via SMS/voice. Enhance your digital detox without unwanted isolation.",
        canonical: "https://lightfriend.ai",
        og_type: "website",
        og_image: OG_IMAGE,
        json_ld: format!("{}\n    {}", software_json_ld(), org_json_ld()),
        lang: "en",
        body_content: r#"
    <h1>Lightfriend: AI Assistant for Dumbphones</h1>
    <p>Access WhatsApp, Telegram, Signal, email, calendar, web search, and more via SMS and voice calls &mdash; no apps, no smartphone required.</p>
    <h2>How It Works</h2>
    <ol>
        <li>Sign up at lightfriend.ai and get a dedicated phone number</li>
        <li>Text or call that number from any phone</li>
        <li>Lightfriend AI processes your request and responds via SMS or voice</li>
        <li>Connect messaging apps, email, and calendar for proactive monitoring</li>
    </ol>
    <h2>Features</h2>
    <ul>
        <li><strong>WhatsApp Integration</strong> &mdash; Send, receive, and monitor WhatsApp messages via SMS</li>
        <li><strong>Telegram Integration</strong> &mdash; Full send/receive/monitor support for Telegram</li>
        <li><strong>Signal Integration</strong> &mdash; Secure Signal messaging on any phone</li>
        <li><strong>Email</strong> &mdash; Gmail and Outlook access via SMS</li>
        <li><strong>Google Calendar</strong> &mdash; View events, create new ones, get reminders</li>
        <li><strong>AI Web Search</strong> &mdash; Internet search via SMS powered by AI</li>
        <li><strong>GPS Directions</strong> &mdash; Turn-by-turn navigation via text</li>
        <li><strong>Tesla Control</strong> &mdash; Lock/unlock, climate, battery check via SMS</li>
        <li><strong>Voice AI</strong> &mdash; Call to interact by voice</li>
        <li><strong>QR Code Scanner</strong> &mdash; Send a photo of a QR code, get the content</li>
        <li><strong>Photo Analysis</strong> &mdash; AI analysis of photos sent via MMS</li>
        <li><strong>Smart Home</strong> &mdash; Control Home Assistant via MCP integration</li>
    </ul>
    <h2>Compatible Phones</h2>
    <p>Works with any phone that can send SMS: Light Phone 2 &amp; 3, Nokia flip phones, any basic phone, any flip phone. Even old smartphones used as dumbphones.</p>
    <h2>Pricing</h2>
    <p>Plans start at $19/month (US/CA) or &euro;29/month (EU). <a href="/pricing">See full pricing</a>.</p>
    <p><a href="/register">Sign up now</a> | <a href="/faq">Learn more</a></p>
    "#.to_string(),
    })
}

// ─── Pricing Page ───

pub async fn pricing() -> Html<String> {
    let json_ld = r#"<script type="application/ld+json">
    {"@context":"https://schema.org","@type":"Product","name":"Lightfriend AI Assistant","description":"AI assistant for dumbphones - access WhatsApp, email, calendar via SMS","brand":{"@type":"Brand","name":"Lightfriend"},"offers":[{"@type":"Offer","name":"Assistant Plan","price":"19.00","priceCurrency":"USD","description":"SMS chat, voice calls, web search, messaging apps, daily digests, MCP support","url":"https://lightfriend.ai/pricing"},{"@type":"Offer","name":"Autopilot Plan","price":"29.00","priceCurrency":"USD","description":"Everything in Assistant plus 24/7 monitoring, priority alerts, event monitoring","url":"https://lightfriend.ai/pricing"}]}
    </script>"#;
    render_page(SeoPage {
        title: "Pricing \u{2013} Lightfriend AI Assistant for Dumbphones",
        description: "Lightfriend pricing plans starting at $19/month. SMS, voice calls, WhatsApp, Telegram, Signal, email, calendar, and more. Available in 40+ countries.",
        canonical: "https://lightfriend.ai/pricing",
        og_type: "website",
        og_image: OG_IMAGE,
        json_ld: json_ld.to_string(),
        lang: "en",
        body_content: r#"
    <h1>Lightfriend Pricing</h1>
    <h2>Assistant Plan &mdash; $19/month (US/CA) or &euro;29/month (EU)</h2>
    <ul>
        <li>SMS chat interface with AI assistant</li>
        <li>Voice calling interface</li>
        <li>Web search, weather, directions</li>
        <li>Connected app access (WhatsApp, Telegram, Signal, email, calendar)</li>
        <li>Scheduled daily digests (morning, day, evening summaries)</li>
        <li>MCP server support for custom integrations</li>
    </ul>
    <h2>Autopilot Plan &mdash; $29/month (US/CA) or &euro;49/month (EU)</h2>
    <ul>
        <li>Everything in Assistant plan</li>
        <li>24/7 critical message monitoring with instant SMS alerts</li>
        <li>Priority sender notifications</li>
        <li>Temporary event monitoring</li>
        <li>Priority support</li>
    </ul>
    <h2>BYOT (Bring Your Own Twilio) &mdash; $19/month</h2>
    <p>Users in unsupported countries can bring their own Twilio phone number. Pay Twilio directly for SMS costs at local rates.</p>
    <p>Available in 40+ countries. <a href="/supported-countries">See all supported countries</a>.</p>
    <p><a href="/register">Sign up now</a></p>
    "#.to_string(),
    })
}

// ─── FAQ Page ───

pub async fn faq() -> Html<String> {
    let json_ld = r#"<script type="application/ld+json">
    {"@context":"https://schema.org","@type":"FAQPage","mainEntity":[{"@type":"Question","name":"What is Lightfriend?","acceptedAnswer":{"@type":"Answer","text":"Lightfriend is an AI assistant for dumbphones that lets you access WhatsApp, Telegram, Signal, email, calendar, web search, GPS directions, and more via SMS and voice calls. No smartphone or apps required."}},{"@type":"Question","name":"Which phones work with Lightfriend?","acceptedAnswer":{"@type":"Answer","text":"Any phone that can send SMS and make calls. This includes Light Phone 2 and 3, Nokia flip phones, any basic or feature phone, and even old smartphones used as dumbphones."}},{"@type":"Question","name":"How much does Lightfriend cost?","acceptedAnswer":{"@type":"Answer","text":"The Assistant plan starts at $19/month (US/CA) or €29/month (EU). The Autopilot plan with 24/7 monitoring is $29/month (US/CA) or €49/month (EU)."}},{"@type":"Question","name":"Which countries are supported?","acceptedAnswer":{"@type":"Answer","text":"Full service with local phone numbers in US, Canada, UK, Finland, Netherlands, and Australia. Notification-only service in 30+ European and Asia-Pacific countries. Other countries can use Bring Your Own Twilio number."}},{"@type":"Question","name":"How does Lightfriend protect my data?","acceptedAnswer":{"@type":"Answer","text":"No call recordings. Optional encrypted message storage. All credentials encrypted with AES-256-GCM. Data never sold or shared. Open source code available for self-hosting."}},{"@type":"Question","name":"Can I try Lightfriend before signing up?","acceptedAnswer":{"@type":"Answer","text":"Yes! The FAQ page at lightfriend.ai/faq includes an interactive demo chat where you can try the AI assistant."}}]}
    </script>"#;
    render_page(SeoPage {
        title: "FAQ \u{2013} Lightfriend AI Assistant for Dumbphones",
        description: "Frequently asked questions about Lightfriend, the AI assistant for dumbphones. Learn how it works, which phones are supported, pricing, and privacy.",
        canonical: "https://lightfriend.ai/faq",
        og_type: "website",
        og_image: OG_IMAGE,
        json_ld: json_ld.to_string(),
        lang: "en",
        body_content: r#"
    <h1>Frequently Asked Questions</h1>
    <h2>What is Lightfriend?</h2>
    <p>Lightfriend is an AI assistant for dumbphones that lets you access WhatsApp, Telegram, Signal, email, calendar, web search, GPS directions, and more via SMS and voice calls. No smartphone or apps required.</p>
    <h2>How does it work?</h2>
    <p>Sign up, get a dedicated phone number, then text or call that number from any phone. Lightfriend AI processes your request and responds via SMS or voice. Connect messaging apps and email for proactive monitoring.</p>
    <h2>Which phones work with Lightfriend?</h2>
    <p>Any phone that can send SMS and make calls: Light Phone 2 &amp; 3, Nokia flip phones, any basic/feature phone, even old smartphones used as dumbphones.</p>
    <h2>How much does it cost?</h2>
    <p>Assistant plan: $19/month (US/CA) or &euro;29/month (EU). Autopilot plan: $29/month (US/CA) or &euro;49/month (EU). <a href="/pricing">Full pricing details</a>.</p>
    <h2>Which countries are supported?</h2>
    <p>Full service in US, Canada, UK, Finland, Netherlands, Australia. Notification-only in 30+ European and Asia-Pacific countries. <a href="/supported-countries">Full list</a>.</p>
    <h2>How does Lightfriend protect my data?</h2>
    <p>No call recordings. Optional encrypted message storage. AES-256-GCM encryption. Data never sold or shared. Open source for self-hosting.</p>
    <h2>Can I try it before signing up?</h2>
    <p>Yes! This page includes an interactive demo chat. <a href="/register">Or sign up to get started</a>.</p>
    "#.to_string(),
    })
}

// ─── Blog Index ───

pub async fn blog() -> Html<String> {
    render_page(SeoPage {
        title: "Blog \u{2013} Lightfriend: Dumbphone Tips, Guides & Digital Wellness",
        description: "Guides, tips, and stories about dumbphone living, digital detox, and getting the most from your minimalist phone with Lightfriend AI assistant.",
        canonical: "https://lightfriend.ai/blog",
        og_type: "website",
        og_image: OG_IMAGE,
        json_ld: org_json_ld(),
        lang: "en",
        body_content: r#"
    <h1>Lightfriend Blog</h1>
    <h2>Guides &amp; Articles</h2>
    <ul>
        <li><a href="/light-phone-3-whatsapp-guide">Light Phone 3 WhatsApp Guide</a> &mdash; How to add WhatsApp functionality to a Light Phone 3 using Lightfriend</li>
        <li><a href="/how-to-switch-to-dumbphone">How to Switch to a Dumbphone</a> &mdash; Complete guide covering 2FA, messaging, navigation, and more</li>
        <li><a href="/how-to-read-more-accidentally">How to Read More Accidentally</a> &mdash; How removing smartphone distractions leads to reading more</li>
        <li><a href="/blog/best-dumbphones-2026">Best Dumbphones in 2026: Complete Buyer's Guide</a></li>
        <li><a href="/blog/adhd-and-smartphones">ADHD and Smartphones: Why Dumbphones Are the Ultimate ADHD Tool</a></li>
        <li><a href="/blog/whatsapp-without-smartphone">How to Use WhatsApp Without a Smartphone</a></li>
        <li><a href="/blog/digital-detox-guide">Digital Detox Guide: Everything You Need to Know</a></li>
        <li><a href="/blog/tesla-sms-control">Tesla Control via SMS: Manage Your Tesla Without a Smartphone</a></li>
        <li><a href="/blog/lightfriend-vs-beeper">Lightfriend vs Beeper vs Bridge Apps</a></li>
        <li><a href="/blog/best-ai-assistants-2026">Best AI Assistants in 2026: Complete Comparison</a></li>
        <li><a href="/blog/email-on-dumbphone">How to Get Email on a Dumbphone</a></li>
        <li><a href="/blog/home-assistant-sms">Home Assistant via SMS: Control Your Smart Home from Any Phone</a></li>
        <li><a href="/blog/scan-qr-without-smartphone">How to Scan QR Codes Without a Smartphone</a></li>
        <li><a href="/blog/best-phone-for-adhd-2026">Best Phone for ADHD in 2026</a></li>
        <li><a href="/blog/telegram-signal-without-smartphone">How to Use Telegram and Signal Without a Smartphone</a></li>
    </ul>
    "#.to_string(),
    })
}

// ─── Updates/Changelog ───

pub async fn updates() -> Html<String> {
    render_page(SeoPage {
        title: "Updates \u{2013} Lightfriend Changelog",
        description: "Recent feature updates, improvements, and changes to Lightfriend AI assistant for dumbphones.",
        canonical: "https://lightfriend.ai/updates",
        og_type: "website",
        og_image: OG_IMAGE,
        json_ld: org_json_ld(),
        lang: "en",
        body_content: r#"
    <h1>Lightfriend Updates &amp; Changelog</h1>
    <p>See what's new in Lightfriend. We regularly add new features, integrations, and improvements.</p>
    <p><a href="/">Back to homepage</a> | <a href="/pricing">View pricing</a></p>
    "#.to_string(),
    })
}

// ─── Supported Countries ───

pub async fn supported_countries() -> Html<String> {
    render_page(SeoPage {
        title: "Supported Countries \u{2013} Lightfriend AI Assistant",
        description: "Lightfriend is available in 40+ countries. Full service with local numbers in US, Canada, UK, Finland, Netherlands, Australia. Notification-only in 30+ more countries.",
        canonical: "https://lightfriend.ai/supported-countries",
        og_type: "website",
        og_image: OG_IMAGE,
        json_ld: org_json_ld(),
        lang: "en",
        body_content: r#"
    <h1>Supported Countries</h1>
    <h2>Full Service (Local Phone Number Provided)</h2>
    <p>United States, Canada, United Kingdom, Finland, Netherlands, Australia</p>
    <h2>Notification-Only (Receive SMS from US/UK Number)</h2>
    <p>Germany, France, Spain, Italy, Portugal, Belgium, Austria, Switzerland, Poland, Czech Republic, Sweden, Denmark, Norway, Ireland, New Zealand, Greece, Hungary, Romania, Slovakia, Bulgaria, Croatia, Slovenia, Lithuania, Latvia, Estonia, Luxembourg, Malta, Cyprus, Iceland, Japan, South Korea, Singapore, Hong Kong, Taiwan, Israel</p>
    <h2>Other Countries</h2>
    <p>Use <a href="/bring-own-number">Bring Your Own Twilio number</a> or Android phone SMS bridge.</p>
    <p><a href="/register">Sign up now</a> | <a href="/pricing">View pricing</a></p>
    "#.to_string(),
    })
}

// ─── Bring Your Own Number ───

pub async fn bring_own_number() -> Html<String> {
    render_page(SeoPage {
        title: "Bring Your Own Number \u{2013} Lightfriend",
        description: "Use your own Twilio phone number with Lightfriend in any country. Pay Twilio directly for SMS costs at local rates.",
        canonical: "https://lightfriend.ai/bring-own-number",
        og_type: "website",
        og_image: OG_IMAGE,
        json_ld: org_json_ld(),
        lang: "en",
        body_content: r#"
    <h1>Bring Your Own Number (BYOT)</h1>
    <p>Use Lightfriend in any country by bringing your own Twilio phone number. You pay Twilio directly for SMS costs at local rates, plus the Lightfriend subscription ($19/month).</p>
    <h2>How It Works</h2>
    <ol>
        <li>Create a Twilio account and purchase a phone number in your country</li>
        <li>Sign up for Lightfriend and select the BYOT plan</li>
        <li>Enter your Twilio credentials in the Lightfriend dashboard</li>
        <li>Start texting your Twilio number to use Lightfriend</li>
    </ol>
    <p><a href="/register">Sign up now</a> | <a href="/supported-countries">See supported countries</a></p>
    "#.to_string(),
    })
}

// ─── Blog Posts ───

pub async fn light_phone_3_whatsapp() -> Html<String> {
    let json_ld = r#"<script type="application/ld+json">
    {"@context":"https://schema.org","@type":"BlogPosting","headline":"Light Phone 3 WhatsApp Guide: How to Get WhatsApp on Light Phone 3","description":"Step-by-step guide to add WhatsApp messaging to your Light Phone 3 using Lightfriend AI assistant. Send and receive WhatsApp messages via SMS.","url":"https://lightfriend.ai/light-phone-3-whatsapp-guide","datePublished":"2025-08-13","dateModified":"2026-03-03","author":{"@type":"Organization","name":"Lightfriend"},"publisher":{"@type":"Organization","name":"Lightfriend","logo":{"@type":"ImageObject","url":"https://lightfriend.ai/assets/fav.png"}}}
    </script>"#;
    render_page(SeoPage {
        title: "Light Phone 3 WhatsApp Guide \u{2013} How to Get WhatsApp on Light Phone 3",
        description: "Step-by-step guide to add WhatsApp messaging to your Light Phone 3 using Lightfriend. Send and receive WhatsApp messages via SMS without installing any apps.",
        canonical: "https://lightfriend.ai/light-phone-3-whatsapp-guide",
        og_type: "article",
        og_image: OG_IMAGE,
        json_ld: json_ld.to_string(),
        lang: "en",
        body_content: r#"
    <h1>Light Phone 3 WhatsApp Guide</h1>
    <p>The Light Phone 3 is a beautiful minimalist phone, but it doesn't support WhatsApp natively. With Lightfriend, you can send and receive WhatsApp messages via SMS &mdash; no apps needed.</p>
    <h2>How to Add WhatsApp to Light Phone 3</h2>
    <ol>
        <li>Sign up at <a href="/register">lightfriend.ai</a></li>
        <li>Connect your WhatsApp account through the dashboard</li>
        <li>Get a dedicated Lightfriend phone number</li>
        <li>Text that number from your Light Phone 3 to send WhatsApp messages</li>
        <li>Receive WhatsApp notifications as SMS on your Light Phone 3</li>
    </ol>
    <h2>What You Can Do</h2>
    <ul>
        <li>Send messages to any WhatsApp contact</li>
        <li>Receive and read incoming WhatsApp messages</li>
        <li>Get notifications for important messages</li>
        <li>Send messages to WhatsApp groups</li>
    </ul>
    <p>Works with the Autopilot plan ($29/month) for 24/7 monitoring, or the Assistant plan ($19/month) with scheduled digests.</p>
    <p><a href="/register">Get started</a> | <a href="/features/whatsapp-dumbphone">Learn more about WhatsApp on dumbphones</a></p>
    "#.to_string(),
    })
}

pub async fn switch_to_dumbphone() -> Html<String> {
    let json_ld = r#"<script type="application/ld+json">
    {"@context":"https://schema.org","@type":"BlogPosting","headline":"How to Switch to a Dumbphone: The Complete Guide","description":"Everything you need to know about switching from a smartphone to a dumbphone. Covers 2FA, messaging apps, navigation, email, and more.","url":"https://lightfriend.ai/how-to-switch-to-dumbphone","datePublished":"2025-08-19","dateModified":"2026-03-03","author":{"@type":"Organization","name":"Lightfriend"},"publisher":{"@type":"Organization","name":"Lightfriend","logo":{"@type":"ImageObject","url":"https://lightfriend.ai/assets/fav.png"}}}
    </script>"#;
    render_page(SeoPage {
        title: "How to Switch to a Dumbphone: The Complete Guide (2026)",
        description: "Everything you need to know about switching from a smartphone to a dumbphone. Covers 2FA authentication, messaging apps, navigation, email, calendar, and setting up your computer.",
        canonical: "https://lightfriend.ai/how-to-switch-to-dumbphone",
        og_type: "article",
        og_image: OG_IMAGE,
        json_ld: json_ld.to_string(),
        lang: "en",
        body_content: r#"
    <h1>How to Switch to a Dumbphone: The Complete Guide</h1>
    <p>Switching from a smartphone to a dumbphone is one of the best decisions you can make for your mental health, focus, and overall wellbeing. But it comes with challenges. This guide covers everything.</p>
    <h2>Before You Switch</h2>
    <ul>
        <li>Move 2FA to hardware keys or SMS-based codes</li>
        <li>Set up messaging alternatives (Lightfriend handles WhatsApp, Telegram, Signal via SMS)</li>
        <li>Configure email forwarding or use Lightfriend for email access</li>
        <li>Download offline maps or use Lightfriend for GPS directions via SMS</li>
    </ul>
    <h2>Choosing Your Dumbphone</h2>
    <p>Popular choices: Light Phone 2/3, Nokia 2780/2760 Flip, Punkt MP02, CAT B35. All work with Lightfriend.</p>
    <h2>Setting Up Your Computer</h2>
    <p>Use your computer for tasks that need a screen: banking, booking travel, video calls. Block distracting websites with tools like Cold Turkey or Freedom.</p>
    <p><a href="/register">Set up Lightfriend</a> to keep all your messaging and productivity tools accessible from your dumbphone.</p>
    "#.to_string(),
    })
}

pub async fn read_more_accidentally() -> Html<String> {
    let json_ld = r#"<script type="application/ld+json">
    {"@context":"https://schema.org","@type":"BlogPosting","headline":"How to Read More Accidentally","description":"How removing smartphone distractions naturally leads to reading more books. Tips for blocking digital escapism while keeping productivity tools.","url":"https://lightfriend.ai/how-to-read-more-accidentally","datePublished":"2025-08-21","dateModified":"2026-03-03","author":{"@type":"Organization","name":"Lightfriend"},"publisher":{"@type":"Organization","name":"Lightfriend","logo":{"@type":"ImageObject","url":"https://lightfriend.ai/assets/fav.png"}}}
    </script>"#;
    render_page(SeoPage {
        title: "How to Read More Accidentally \u{2013} Dumbphone Reading Tips",
        description: "How removing smartphone distractions naturally leads to reading more books. Tips for blocking digital escapism on computers while keeping productivity tools.",
        canonical: "https://lightfriend.ai/how-to-read-more-accidentally",
        og_type: "article",
        og_image: OG_IMAGE,
        json_ld: json_ld.to_string(),
        lang: "en",
        body_content: r#"
    <h1>How to Read More Accidentally</h1>
    <p>When you switch from a smartphone to a dumbphone, something unexpected happens: you start reading more. Not because you planned to, but because the constant pull of social media, news apps, and infinite scrolling is simply gone.</p>
    <h2>Why Dumbphones Make You Read More</h2>
    <p>Smartphones fill every idle moment with content designed to keep you scrolling. Remove that, and your brain naturally seeks out deeper content &mdash; books, long articles, physical newspapers.</p>
    <h2>Tips for Maximizing This Effect</h2>
    <ul>
        <li>Keep a book with you at all times</li>
        <li>Block distracting websites on your computer</li>
        <li>Use Lightfriend for essential digital tasks so your dumbphone stays distraction-free</li>
        <li>Visit your local library regularly</li>
    </ul>
    "#.to_string(),
    })
}

// ─── Terms & Privacy ───

pub async fn terms() -> Html<String> {
    render_page(SeoPage {
        title: "Terms of Service \u{2013} Lightfriend",
        description: "Lightfriend terms of service and conditions of use.",
        canonical: "https://lightfriend.ai/terms",
        og_type: "website",
        og_image: OG_IMAGE,
        json_ld: org_json_ld(),
        lang: "en",
        body_content: r#"
    <h1>Terms of Service</h1>
    <p>By using Lightfriend, you agree to these terms. Lightfriend is an AI assistant service that provides access to messaging apps, email, calendar, and other digital services via SMS and voice calls.</p>
    <p>For the full terms, please visit this page in a browser: <a href="https://lightfriend.ai/terms">lightfriend.ai/terms</a></p>
    "#.to_string(),
    })
}

pub async fn privacy() -> Html<String> {
    render_page(SeoPage {
        title: "Privacy Policy \u{2013} Lightfriend",
        description: "Lightfriend privacy policy. How we handle your data, encryption practices, and privacy commitments.",
        canonical: "https://lightfriend.ai/privacy",
        og_type: "website",
        og_image: OG_IMAGE,
        json_ld: org_json_ld(),
        lang: "en",
        body_content: r#"
    <h1>Privacy Policy</h1>
    <p>Lightfriend takes your privacy seriously. Key points:</p>
    <ul>
        <li>No call recordings</li>
        <li>AES-256-GCM encryption for all stored data</li>
        <li>Data never sold or shared with third parties</li>
        <li>Optional encrypted message storage (up to 10 recent exchanges)</li>
        <li>Open source code available for self-hosting</li>
    </ul>
    <p>For the full privacy policy, please visit: <a href="https://lightfriend.ai/privacy">lightfriend.ai/privacy</a></p>
    "#.to_string(),
    })
}

// ─── Features Index ───

pub async fn features_index() -> Html<String> {
    render_page(SeoPage {
        title: "Features \u{2013} Lightfriend AI Assistant for Dumbphones",
        description: "All Lightfriend features: WhatsApp, Telegram, Signal, email, calendar, Tesla control, AI search, GPS, voice AI, smart home, QR scanning, and digital wellness.",
        canonical: "https://lightfriend.ai/features",
        og_type: "website",
        og_image: OG_IMAGE,
        json_ld: String::new(),
        lang: "en",
        body_content: r#"
    <h1>Features</h1>
    <p>Everything your dumbphone can do with Lightfriend</p>
    <ul>
        <li><a href="/features/whatsapp-dumbphone">WhatsApp</a> &mdash; Send &amp; receive WhatsApp messages via SMS</li>
        <li><a href="/features/telegram-dumbphone">Telegram</a> &mdash; Access Telegram from any phone</li>
        <li><a href="/features/signal-dumbphone">Signal</a> &mdash; Secure messaging on your dumbphone</li>
        <li><a href="/features/email-sms">Email</a> &mdash; Gmail &amp; Outlook via SMS</li>
        <li><a href="/features/calendar-sms">Calendar</a> &mdash; Google Calendar reminders &amp; events</li>
        <li><a href="/features/tesla-sms">Tesla Control</a> &mdash; Lock, unlock, climate, battery via SMS</li>
        <li><a href="/features/ai-search-sms">AI Search</a> &mdash; Web search via text message</li>
        <li><a href="/features/gps-directions-sms">GPS Directions</a> &mdash; Turn-by-turn navigation via SMS</li>
        <li><a href="/features/voice-ai">Voice AI</a> &mdash; Call an AI assistant</li>
        <li><a href="/features/autopilot">Autopilot</a> &mdash; 24/7 message monitoring</li>
        <li><a href="/features/smart-home-sms">Smart Home</a> &mdash; Control Home Assistant via SMS</li>
        <li><a href="/features/qr-scanner">QR Scanner</a> &mdash; Decode QR codes via MMS</li>
        <li><a href="/features/wellness">Wellness</a> &mdash; Digital wellness &amp; screen time tools</li>
    </ul>
    "#.to_string(),
    })
}

// ─── Feature Pages ───

pub async fn feature_whatsapp_dumbphone() -> Html<String> {
    render_page(SeoPage {
        title: "WhatsApp on Dumbphone \u{2013} Use WhatsApp Without a Smartphone | Lightfriend",
        description: "Use WhatsApp on any dumbphone or flip phone via SMS. Send, receive, and monitor WhatsApp messages from Light Phone, Nokia, or any basic phone. No apps needed.",
        canonical: "https://lightfriend.ai/features/whatsapp-dumbphone",
        og_type: "website",
        og_image: OG_IMAGE,
        json_ld: r#"<script type="application/ld+json">
    {"@context":"https://schema.org","@type":"WebPage","name":"WhatsApp on Dumbphone","description":"Use WhatsApp on any dumbphone via SMS with Lightfriend","url":"https://lightfriend.ai/features/whatsapp-dumbphone","breadcrumb":{"@type":"BreadcrumbList","itemListElement":[{"@type":"ListItem","position":1,"name":"Home","item":"https://lightfriend.ai"},{"@type":"ListItem","position":2,"name":"Features","item":"https://lightfriend.ai/features"},{"@type":"ListItem","position":3,"name":"WhatsApp on Dumbphone"}]}}
    </script>"#.to_string(),
        lang: "en",
        body_content: r#"
    <h1>WhatsApp on Dumbphone: Use WhatsApp Without a Smartphone</h1>
    <p>WhatsApp is the world's most popular messaging app with over 2 billion users. But what if you use a dumbphone? With Lightfriend, you can send and receive WhatsApp messages via SMS from any phone &mdash; no smartphone, no apps, no internet required.</p>
    <h2>How WhatsApp Works on a Dumbphone</h2>
    <p>Lightfriend acts as a bridge between WhatsApp and SMS. When someone sends you a WhatsApp message, Lightfriend forwards it to your phone as a text message. When you want to reply, you simply text Lightfriend and it sends your message via WhatsApp.</p>
    <h2>Features</h2>
    <ul>
        <li>Send messages to any WhatsApp contact</li>
        <li>Receive incoming WhatsApp messages as SMS</li>
        <li>Get notifications for important or urgent messages</li>
        <li>Access WhatsApp groups</li>
        <li>24/7 monitoring with the Autopilot plan</li>
        <li>Daily digest summaries of all WhatsApp activity</li>
    </ul>
    <h2>Compatible Phones</h2>
    <p>Works with Light Phone 2 &amp; 3, Nokia flip phones (2780, 2760), any basic phone, any flip phone, any phone that can send SMS.</p>
    <h2>How to Get Started</h2>
    <ol>
        <li><a href="/register">Sign up for Lightfriend</a> ($19/month)</li>
        <li>Connect your WhatsApp account through the dashboard</li>
        <li>Start sending and receiving WhatsApp messages via SMS</li>
    </ol>
    <h2>FAQ</h2>
    <h3>Can I use WhatsApp on a Light Phone 3?</h3>
    <p>Yes! Lightfriend is the best way to get WhatsApp on a Light Phone 3. <a href="/light-phone-3-whatsapp-guide">See our complete Light Phone 3 WhatsApp guide</a>.</p>
    <h3>Do I need to keep a smartphone for WhatsApp?</h3>
    <p>No. Lightfriend handles the WhatsApp connection for you. You only need your dumbphone.</p>
    <h3>Can I use WhatsApp on a Nokia flip phone?</h3>
    <p>Yes! Any Nokia flip phone or feature phone that can send SMS works with Lightfriend for WhatsApp access.</p>
    <p><a href="/register">Get WhatsApp on your dumbphone now</a> | <a href="/pricing">View pricing</a></p>
    "#.to_string(),
    })
}

pub async fn feature_telegram_dumbphone() -> Html<String> {
    render_page(SeoPage {
        title: "Telegram on Dumbphone \u{2013} Use Telegram Without a Smartphone | Lightfriend",
        description: "Access Telegram from any dumbphone via SMS. Send, receive, and monitor Telegram messages from Light Phone, Nokia, or any basic phone with Lightfriend.",
        canonical: "https://lightfriend.ai/features/telegram-dumbphone",
        og_type: "website",
        og_image: OG_IMAGE,
        json_ld: String::new(),
        lang: "en",
        body_content: r#"
    <h1>Telegram on Dumbphone: Use Telegram Without a Smartphone</h1>
    <p>Telegram is a popular secure messaging app, but it requires a smartphone or computer. With Lightfriend, you can access Telegram from any phone that can send SMS &mdash; including dumbphones, flip phones, and feature phones.</p>
    <h2>How It Works</h2>
    <p>Lightfriend bridges Telegram and SMS. Incoming Telegram messages are forwarded as text messages. You reply by texting Lightfriend, which sends your message via Telegram.</p>
    <h2>Features</h2>
    <ul>
        <li>Send and receive Telegram messages via SMS</li>
        <li>Access Telegram groups and channels</li>
        <li>Get notifications for important messages</li>
        <li>24/7 monitoring or scheduled digests</li>
    </ul>
    <p><a href="/register">Get Telegram on your dumbphone</a> | <a href="/pricing">View pricing</a></p>
    "#.to_string(),
    })
}

pub async fn feature_signal_dumbphone() -> Html<String> {
    render_page(SeoPage {
        title: "Signal on Dumbphone \u{2013} Use Signal Without a Smartphone | Lightfriend",
        description: "Access Signal messenger from any dumbphone via SMS. Secure messaging on Light Phone, Nokia, or any basic phone with Lightfriend AI assistant.",
        canonical: "https://lightfriend.ai/features/signal-dumbphone",
        og_type: "website",
        og_image: OG_IMAGE,
        json_ld: String::new(),
        lang: "en",
        body_content: r#"
    <h1>Signal on Dumbphone: Use Signal Without a Smartphone</h1>
    <p>Signal is known for its privacy-first approach to messaging. With Lightfriend, you can use Signal from any dumbphone via SMS, maintaining secure communication without needing a smartphone.</p>
    <h2>Features</h2>
    <ul>
        <li>Send and receive Signal messages via SMS</li>
        <li>Maintain your secure messaging connections</li>
        <li>Get notifications for important messages</li>
        <li>Works with any phone that can send SMS</li>
    </ul>
    <p><a href="/register">Get Signal on your dumbphone</a> | <a href="/pricing">View pricing</a></p>
    "#.to_string(),
    })
}

pub async fn feature_email_sms() -> Html<String> {
    render_page(SeoPage {
        title: "Email on Dumbphone \u{2013} Read & Send Email via SMS | Lightfriend",
        description: "Access Gmail and Outlook email from any dumbphone via SMS. Read, reply, and compose emails from a flip phone or basic phone with Lightfriend.",
        canonical: "https://lightfriend.ai/features/email-sms",
        og_type: "website",
        og_image: OG_IMAGE,
        json_ld: String::new(),
        lang: "en",
        body_content: r#"
    <h1>Email via SMS: Read and Send Email on a Dumbphone</h1>
    <p>Email is essential for work and personal communication, but dumbphones don't have email apps. Lightfriend gives you full email access via SMS &mdash; read incoming emails, compose replies, and get alerts for important messages.</p>
    <h2>Supported Email Services</h2>
    <ul>
        <li>Gmail</li>
        <li>Microsoft Outlook</li>
    </ul>
    <h2>What You Can Do</h2>
    <ul>
        <li>Read incoming emails as text messages</li>
        <li>Reply to emails via SMS</li>
        <li>Compose and send new emails</li>
        <li>Get alerts for important or urgent emails</li>
        <li>Daily email digest summaries</li>
    </ul>
    <p><a href="/register">Get email on your dumbphone</a> | <a href="/pricing">View pricing</a></p>
    "#.to_string(),
    })
}

pub async fn feature_calendar_sms() -> Html<String> {
    render_page(SeoPage {
        title: "Google Calendar on Dumbphone \u{2013} Calendar via SMS | Lightfriend",
        description: "Access Google Calendar from any dumbphone via SMS. View events, create appointments, and get reminders on your flip phone or basic phone.",
        canonical: "https://lightfriend.ai/features/calendar-sms",
        og_type: "website",
        og_image: OG_IMAGE,
        json_ld: String::new(),
        lang: "en",
        body_content: r#"
    <h1>Google Calendar on Dumbphone: Calendar via SMS</h1>
    <p>Never miss an appointment again. Lightfriend connects to your Google Calendar and lets you manage your schedule entirely via SMS from any phone.</p>
    <h2>Features</h2>
    <ul>
        <li>View upcoming events via text message</li>
        <li>Create new calendar events by texting</li>
        <li>Receive SMS reminders before meetings</li>
        <li>Daily schedule summaries</li>
    </ul>
    <p><a href="/register">Get calendar on your dumbphone</a> | <a href="/pricing">View pricing</a></p>
    "#.to_string(),
    })
}

pub async fn feature_tesla_sms() -> Html<String> {
    render_page(SeoPage {
        title: "Tesla SMS Control \u{2013} Control Your Tesla from a Dumbphone | Lightfriend",
        description: "Control your Tesla from any phone via SMS. Lock/unlock, climate control, battery check, and more. No Tesla app or smartphone needed.",
        canonical: "https://lightfriend.ai/features/tesla-sms",
        og_type: "website",
        og_image: OG_IMAGE,
        json_ld: String::new(),
        lang: "en",
        body_content: r#"
    <h1>Tesla SMS Control: Manage Your Tesla from Any Phone</h1>
    <p>Tesla owners don't need a smartphone to control their vehicle. With Lightfriend, you can manage your Tesla entirely via text messages from any phone.</p>
    <h2>What You Can Do</h2>
    <ul>
        <li>Lock and unlock your Tesla</li>
        <li>Start and stop climate control</li>
        <li>Check battery level and range</li>
        <li>Open and close the trunk/frunk</li>
        <li>Start and stop charging</li>
        <li>Flash lights and honk horn</li>
    </ul>
    <h2>How It Works</h2>
    <p>Connect your Tesla account through the Lightfriend dashboard, then text commands like "lock my Tesla" or "what's my battery level?" to your Lightfriend number.</p>
    <p><a href="/register">Control your Tesla via SMS</a> | <a href="/pricing">View pricing</a></p>
    "#.to_string(),
    })
}

pub async fn feature_ai_search_sms() -> Html<String> {
    render_page(SeoPage {
        title: "AI Search on Dumbphone \u{2013} Web Search via SMS | Lightfriend",
        description: "Search the internet from any dumbphone via SMS. AI-powered web search delivers concise answers as text messages. No browser needed.",
        canonical: "https://lightfriend.ai/features/ai-search-sms",
        og_type: "website",
        og_image: OG_IMAGE,
        json_ld: String::new(),
        lang: "en",
        body_content: r#"
    <h1>AI Search via SMS: Web Search on a Dumbphone</h1>
    <p>Need to look something up but only have a dumbphone? Text your question to Lightfriend and get AI-powered web search results as concise SMS responses. Powered by advanced AI, you get accurate answers without needing a browser.</p>
    <h2>Examples</h2>
    <ul>
        <li>"What time does Target close today?"</li>
        <li>"What's the capital of Mongolia?"</li>
        <li>"Best Italian restaurant near me"</li>
        <li>"Current weather in New York"</li>
    </ul>
    <p><a href="/register">Get AI search on your dumbphone</a> | <a href="/pricing">View pricing</a></p>
    "#.to_string(),
    })
}

pub async fn feature_gps_directions_sms() -> Html<String> {
    render_page(SeoPage {
        title: "GPS on Dumbphone \u{2013} Directions via SMS | Lightfriend",
        description: "Get turn-by-turn GPS directions on any dumbphone via SMS. Google Maps navigation delivered as text messages. No GPS app needed.",
        canonical: "https://lightfriend.ai/features/gps-directions-sms",
        og_type: "website",
        og_image: OG_IMAGE,
        json_ld: String::new(),
        lang: "en",
        body_content: r#"
    <h1>GPS Directions via SMS: Navigation on Any Phone</h1>
    <p>Dumbphones don't have GPS apps, but you still need directions. Lightfriend sends you turn-by-turn navigation instructions via text message, powered by Google Maps.</p>
    <h2>How It Works</h2>
    <p>Text something like "directions from Times Square to Central Park" and receive step-by-step navigation instructions as SMS messages.</p>
    <h2>Features</h2>
    <ul>
        <li>Turn-by-turn directions via text</li>
        <li>Walking, driving, and transit directions</li>
        <li>Estimated travel time and distance</li>
        <li>Powered by Google Maps</li>
    </ul>
    <p><a href="/register">Get GPS on your dumbphone</a> | <a href="/pricing">View pricing</a></p>
    "#.to_string(),
    })
}

pub async fn feature_voice_ai() -> Html<String> {
    render_page(SeoPage {
        title: "Voice AI Assistant for Dumbphone \u{2013} Call an AI | Lightfriend",
        description: "Call an AI voice assistant from any phone. Get answers, search the web, manage messages, and more by voice. No smartphone needed.",
        canonical: "https://lightfriend.ai/features/voice-ai",
        og_type: "website",
        og_image: OG_IMAGE,
        json_ld: String::new(),
        lang: "en",
        body_content: r#"
    <h1>Voice AI Assistant: Call an AI from Any Phone</h1>
    <p>Don't want to type? Call your Lightfriend number and interact with an AI assistant by voice. Ask questions, manage messages, search the web, and more &mdash; all by phone call.</p>
    <h2>What You Can Do by Voice</h2>
    <ul>
        <li>Ask questions and get AI-powered answers</li>
        <li>Search the web by voice</li>
        <li>Send and receive messages</li>
        <li>Check your calendar and create events</li>
        <li>Get weather forecasts and directions</li>
    </ul>
    <p>Also available as web calls from the Lightfriend dashboard.</p>
    <p><a href="/register">Get started with voice AI</a> | <a href="/pricing">View pricing</a></p>
    "#.to_string(),
    })
}

pub async fn feature_autopilot() -> Html<String> {
    render_page(SeoPage {
        title: "Autopilot: Proactive AI Monitoring \u{2013} Lightfriend",
        description: "24/7 AI monitoring of your messages, email, and calendar. Get instant SMS alerts for urgent messages. Never miss important communication on your dumbphone.",
        canonical: "https://lightfriend.ai/features/autopilot",
        og_type: "website",
        og_image: OG_IMAGE,
        json_ld: String::new(),
        lang: "en",
        body_content: r#"
    <h1>Autopilot: Proactive AI Message Monitoring</h1>
    <p>Lightfriend's Autopilot plan monitors your connected messaging apps and email 24/7, alerting you instantly via SMS when something urgent arrives. Never miss an important message on your dumbphone.</p>
    <h2>Features</h2>
    <ul>
        <li>24/7 critical message monitoring across all connected apps</li>
        <li>Instant SMS alerts for urgent messages</li>
        <li>Priority sender notifications &mdash; get alerts from specific contacts</li>
        <li>Temporary event monitoring (e.g., "alert me when John replies")</li>
        <li>Morning, daytime, and evening digest summaries</li>
    </ul>
    <p>$29/month (US/CA) or &euro;49/month (EU). <a href="/register">Get Autopilot</a> | <a href="/pricing">View all plans</a></p>
    "#.to_string(),
    })
}

pub async fn feature_smart_home_sms() -> Html<String> {
    render_page(SeoPage {
        title: "Smart Home via SMS \u{2013} Control Home Assistant from Any Phone | Lightfriend",
        description: "Control your smart home from any phone via SMS. Home Assistant integration through MCP. Lights, thermostat, locks, and more without a smartphone.",
        canonical: "https://lightfriend.ai/features/smart-home-sms",
        og_type: "website",
        og_image: OG_IMAGE,
        json_ld: String::new(),
        lang: "en",
        body_content: r#"
    <h1>Smart Home via SMS: Control Home Assistant from Any Phone</h1>
    <p>Lightfriend connects to Home Assistant and other smart home platforms via MCP (Model Context Protocol) servers. Control your lights, thermostat, locks, and more by simply sending a text message.</p>
    <h2>What You Can Control</h2>
    <ul>
        <li>Lights &mdash; turn on/off, adjust brightness</li>
        <li>Thermostat &mdash; set temperature, change modes</li>
        <li>Locks &mdash; lock/unlock doors</li>
        <li>Garage doors &mdash; open/close</li>
        <li>Any device supported by your Home Assistant setup</li>
    </ul>
    <h2>How It Works</h2>
    <p>Add your Home Assistant MCP server URL in the Lightfriend dashboard. Then text commands like "turn off the living room lights" or "set thermostat to 72" to your Lightfriend number.</p>
    <p><a href="/register">Control your smart home via SMS</a> | <a href="/pricing">View pricing</a></p>
    "#.to_string(),
    })
}

pub async fn feature_qr_scanner() -> Html<String> {
    render_page(SeoPage {
        title: "QR Code Scanner for Dumbphone \u{2013} Scan QR Codes Without a Smartphone | Lightfriend",
        description: "Scan QR codes from any dumbphone. Send a photo of a QR code via MMS and get the decoded URL or content as a text message.",
        canonical: "https://lightfriend.ai/features/qr-scanner",
        og_type: "website",
        og_image: OG_IMAGE,
        json_ld: String::new(),
        lang: "en",
        body_content: r#"
    <h1>QR Code Scanner for Dumbphone</h1>
    <p>QR codes are everywhere &mdash; restaurant menus, tickets, payments, website links. But dumbphones don't have QR scanners. With Lightfriend, take a photo of any QR code and send it via MMS to get the decoded content as a text message.</p>
    <h2>How It Works</h2>
    <ol>
        <li>Take a photo of the QR code with your phone's camera</li>
        <li>Send the photo via MMS to your Lightfriend number</li>
        <li>Receive the decoded URL or content as a text message</li>
    </ol>
    <p>Available in US, Canada, and Australia (requires MMS support).</p>
    <p><a href="/register">Get QR scanning on your dumbphone</a> | <a href="/pricing">View pricing</a></p>
    "#.to_string(),
    })
}

pub async fn feature_wellness() -> Html<String> {
    render_page(SeoPage {
        title: "Digital Wellness \u{2013} Screen Time Reduction & Dumbphone Mode | Lightfriend",
        description: "Digital wellness tools to reduce screen time. Dumbphone mode, daily check-ins, notification calming, and wellbeing tracking.",
        canonical: "https://lightfriend.ai/features/wellness",
        og_type: "website",
        og_image: OG_IMAGE,
        json_ld: String::new(),
        lang: "en",
        body_content: r#"
    <h1>Digital Wellness: Screen Time Reduction Tools</h1>
    <p>Lightfriend helps you reduce screen time and build healthier digital habits. Our wellness features are designed for dumbphone users and anyone looking to reduce smartphone dependence.</p>
    <h2>Features</h2>
    <ul>
        <li><strong>Dumbphone Mode</strong> &mdash; Track your smartphone-free days</li>
        <li><strong>Daily Check-ins</strong> &mdash; Reflect on your digital wellness</li>
        <li><strong>Notification Calmer</strong> &mdash; Reduce notification overload</li>
        <li><strong>Wellbeing Points</strong> &mdash; Gamify your digital detox journey</li>
        <li><strong>Stats &amp; Tracking</strong> &mdash; Monitor your progress over time</li>
    </ul>
    <p><a href="/register">Start your digital wellness journey</a> | <a href="/pricing">View pricing</a></p>
    "#.to_string(),
    })
}

// ─── Audience Pages ───

pub async fn for_adhd() -> Html<String> {
    render_page(SeoPage {
        title: "Best Phone for ADHD \u{2013} Dumbphone + AI Assistant | Lightfriend",
        description: "The best phone solution for ADHD. A dumbphone with Lightfriend removes distractions while keeping essential communication. Reduce impulse scrolling, improve focus.",
        canonical: "https://lightfriend.ai/for/adhd",
        og_type: "website",
        og_image: OG_IMAGE,
        json_ld: String::new(),
        lang: "en",
        body_content: r#"
    <h1>Best Phone for ADHD: Dumbphone + Lightfriend</h1>
    <p>If you have ADHD, your smartphone is working against you. The constant notifications, infinite scrolling, and app switching are designed to capture attention &mdash; the exact thing ADHD makes hard to manage. A dumbphone eliminates these triggers entirely.</p>
    <h2>Why a Dumbphone Helps with ADHD</h2>
    <ul>
        <li><strong>No infinite scrolling</strong> &mdash; Social media feeds, news apps, and video recommendations are gone</li>
        <li><strong>No impulse checking</strong> &mdash; Without a touchscreen full of apps, there's nothing to compulsively check</li>
        <li><strong>Reduced context switching</strong> &mdash; A dumbphone does calls and texts, period</li>
        <li><strong>Better sleep</strong> &mdash; No blue light rabbit holes before bed</li>
        <li><strong>Improved focus</strong> &mdash; Your phone stops being a source of distraction</li>
    </ul>
    <h2>But What About Messaging, Email, Calendar?</h2>
    <p>This is where Lightfriend comes in. You keep the benefits of a dumbphone while Lightfriend handles the digital services you actually need:</p>
    <ul>
        <li>WhatsApp, Telegram, and Signal messages via SMS</li>
        <li>Email access via text</li>
        <li>Calendar reminders sent to your phone</li>
        <li>AI web search when you need to look something up</li>
    </ul>
    <h2>Recommended Setup for ADHD</h2>
    <ol>
        <li>Get a Light Phone 3 or Nokia flip phone</li>
        <li>Sign up for Lightfriend Autopilot ($29/month) for 24/7 message monitoring</li>
        <li>Connect your messaging apps and email</li>
        <li>Use your computer for tasks that require a screen, with website blockers installed</li>
    </ol>
    <p><a href="/register">Get started</a> | <a href="/pricing">View pricing</a> | <a href="/how-to-switch-to-dumbphone">Complete dumbphone switch guide</a></p>
    "#.to_string(),
    })
}

pub async fn for_digital_detox() -> Html<String> {
    render_page(SeoPage {
        title: "Digital Detox Without Losing Messaging \u{2013} Lightfriend",
        description: "Do a digital detox without losing access to WhatsApp, email, and calendar. Lightfriend keeps you connected via SMS while you break free from smartphone addiction.",
        canonical: "https://lightfriend.ai/for/digital-detox",
        og_type: "website",
        og_image: OG_IMAGE,
        json_ld: String::new(),
        lang: "en",
        body_content: r#"
    <h1>Digital Detox: Stay Connected Without a Smartphone</h1>
    <p>A digital detox doesn't mean cutting yourself off from the world. With Lightfriend, you can switch to a dumbphone and still receive important messages, emails, and calendar reminders via SMS.</p>
    <h2>The Problem with Cold-Turkey Digital Detox</h2>
    <p>Most digital detoxes fail because going completely offline means missing important messages, appointments, and communication. Lightfriend solves this by bridging essential digital services to SMS.</p>
    <h2>What You Keep During Your Detox</h2>
    <ul>
        <li>WhatsApp, Telegram, and Signal messages</li>
        <li>Email alerts for important messages</li>
        <li>Calendar reminders</li>
        <li>Web search when you need it</li>
        <li>GPS directions</li>
    </ul>
    <h2>What You Lose (The Good Part)</h2>
    <ul>
        <li>Social media addiction</li>
        <li>Infinite scrolling</li>
        <li>Notification overload</li>
        <li>Impulse app checking</li>
        <li>Blue light before bed</li>
    </ul>
    <p><a href="/register">Start your detox with Lightfriend</a> | <a href="/features/wellness">Wellness tools</a></p>
    "#.to_string(),
    })
}

pub async fn for_light_phone() -> Html<String> {
    render_page(SeoPage {
        title: "Lightfriend for Light Phone \u{2013} The Perfect Light Phone Companion",
        description: "The perfect companion app for Light Phone 2 and Light Phone 3. Add WhatsApp, email, calendar, and more to your Light Phone via SMS.",
        canonical: "https://lightfriend.ai/for/light-phone",
        og_type: "website",
        og_image: OG_IMAGE,
        json_ld: String::new(),
        lang: "en",
        body_content: r#"
    <h1>Lightfriend for Light Phone: The Perfect Companion</h1>
    <p>The Light Phone is a beautifully designed minimalist phone. Lightfriend is its perfect companion &mdash; adding WhatsApp, email, calendar, and AI features without compromising the Light Phone experience.</p>
    <h2>What Lightfriend Adds to Your Light Phone</h2>
    <ul>
        <li>WhatsApp, Telegram, and Signal messaging via SMS</li>
        <li>Email access (Gmail, Outlook)</li>
        <li>Google Calendar with reminders</li>
        <li>AI-powered web search</li>
        <li>GPS directions via text</li>
        <li>QR code scanning (Light Phone 3 with MMS)</li>
        <li>Tesla vehicle control</li>
        <li>Voice AI assistant</li>
    </ul>
    <h2>Works with Both Light Phone 2 and Light Phone 3</h2>
    <p>Both models support SMS and calling, which is all you need for Lightfriend. The Light Phone 3 also supports MMS for photo analysis and QR code scanning.</p>
    <p><a href="/register">Get Lightfriend for your Light Phone</a> | <a href="/light-phone-3-whatsapp-guide">LP3 WhatsApp guide</a></p>
    "#.to_string(),
    })
}

pub async fn for_nokia() -> Html<String> {
    render_page(SeoPage {
        title: "Lightfriend for Nokia \u{2013} Add Smart Features to Nokia Flip Phones",
        description: "Add WhatsApp, email, calendar, and AI to your Nokia flip phone. Works with Nokia 2780, 2760, and all Nokia feature phones via SMS.",
        canonical: "https://lightfriend.ai/for/nokia",
        og_type: "website",
        og_image: OG_IMAGE,
        json_ld: String::new(),
        lang: "en",
        body_content: r#"
    <h1>Lightfriend for Nokia: Smart Features on Your Nokia Flip Phone</h1>
    <p>Nokia flip phones are reliable, affordable, and have great battery life. With Lightfriend, you can add WhatsApp, email, calendar, and AI features to any Nokia phone via SMS.</p>
    <h2>Compatible Nokia Phones</h2>
    <ul>
        <li>Nokia 2780 Flip</li>
        <li>Nokia 2760 Flip</li>
        <li>Nokia 225</li>
        <li>Nokia 110</li>
        <li>Any Nokia phone with SMS capability</li>
    </ul>
    <h2>What You Get</h2>
    <ul>
        <li>WhatsApp, Telegram, Signal via SMS</li>
        <li>Email access</li>
        <li>Calendar reminders</li>
        <li>Web search and weather</li>
        <li>GPS directions</li>
        <li>AI voice assistant via phone calls</li>
    </ul>
    <p><a href="/register">Get Lightfriend for your Nokia</a> | <a href="/pricing">View pricing</a></p>
    "#.to_string(),
    })
}

pub async fn for_parents() -> Html<String> {
    render_page(SeoPage {
        title: "Safe First Phone for Kids \u{2013} Dumbphone + Lightfriend for Parents",
        description: "The safest first phone for kids. A dumbphone with Lightfriend gives children basic communication without social media, addictive apps, or inappropriate content.",
        canonical: "https://lightfriend.ai/for/parents",
        og_type: "website",
        og_image: OG_IMAGE,
        json_ld: String::new(),
        lang: "en",
        body_content: r#"
    <h1>Safe First Phone for Kids: Dumbphone + Lightfriend</h1>
    <p>Giving your child a smartphone means exposing them to social media, addictive apps, cyberbullying, and inappropriate content. A dumbphone with Lightfriend provides safe, basic communication without the risks.</p>
    <h2>Why a Dumbphone Is Safer</h2>
    <ul>
        <li>No social media or addictive apps</li>
        <li>No web browser for inappropriate content</li>
        <li>No app store or in-app purchases</li>
        <li>Long battery life</li>
        <li>Durable and affordable</li>
    </ul>
    <h2>What Lightfriend Adds</h2>
    <ul>
        <li>WhatsApp access so your child can message friends</li>
        <li>Email access for school communications</li>
        <li>Calendar reminders for activities and homework</li>
        <li>Emergency web search</li>
        <li>GPS directions when needed</li>
    </ul>
    <p><a href="/register">Get a safe phone setup for your child</a> | <a href="/pricing">View pricing</a></p>
    "#.to_string(),
    })
}

pub async fn for_business() -> Html<String> {
    render_page(SeoPage {
        title: "Business Dumbphone \u{2013} Professional Minimalist Phone Setup | Lightfriend",
        description: "Use a dumbphone for business. Stay productive with email, calendar, messaging, and AI search via SMS. Eliminate distractions during work hours.",
        canonical: "https://lightfriend.ai/for/business",
        og_type: "website",
        og_image: OG_IMAGE,
        json_ld: String::new(),
        lang: "en",
        body_content: r#"
    <h1>Business Dumbphone: Professional Minimalist Phone Setup</h1>
    <p>More professionals are switching to dumbphones during work hours to eliminate distractions. With Lightfriend, you maintain access to critical business tools without the smartphone temptation.</p>
    <h2>Business Features</h2>
    <ul>
        <li>Email access (Gmail, Outlook) for urgent business correspondence</li>
        <li>Google Calendar for meetings and deadlines</li>
        <li>WhatsApp/Telegram for team communication</li>
        <li>AI web search for quick lookups</li>
        <li>24/7 monitoring for critical messages (Autopilot plan)</li>
    </ul>
    <h2>Why Professionals Choose Dumbphones</h2>
    <ul>
        <li>Deep work without smartphone interruptions</li>
        <li>Better meetings (no phone distraction)</li>
        <li>Improved client focus</li>
        <li>Signal to colleagues that you value undistracted attention</li>
    </ul>
    <p><a href="/register">Set up your business dumbphone</a> | <a href="/pricing">View pricing</a></p>
    "#.to_string(),
    })
}

// ─── Finnish Pages ───

pub async fn fi_landing() -> Html<String> {
    render_page(SeoPage {
        title: "Lightfriend: Tekoälyavustaja Tyhmäpuhelimille \u{2013} WhatsApp, Telegram, Sähköposti SMS:llä",
        description: "Tekoälyavustaja tyhmäpuhelimille kuten Light Phone 3 ja Nokia-simpukat. Käytä WhatsAppia, Telegramia, Signalia, sähköpostia ja kalenteria tekstiviestillä.",
        canonical: "https://lightfriend.ai/fi",
        og_type: "website",
        og_image: OG_IMAGE,
        json_ld: org_json_ld(),
        lang: "fi",
        body_content: r#"
    <h1>Lightfriend: Tekoälyavustaja Tyhmäpuhelimille</h1>
    <p>Käytä WhatsAppia, Telegramia, Signalia, sähköpostia, kalenteria ja hakua tekstiviestillä &mdash; ilman sovelluksia tai älypuhelinta.</p>
    <h2>Näin se toimii</h2>
    <ol>
        <li>Rekisteröidy osoitteessa lightfriend.ai ja saat oman puhelinnumeron</li>
        <li>Lähetä tekstiviesti tai soita numeroon millä tahansa puhelimella</li>
        <li>Lightfriend-tekoäly käsittelee pyyntösi ja vastaa tekstiviestillä tai puheella</li>
        <li>Yhdistä viestisovellukset, sähköposti ja kalenteri automaattista seurantaa varten</li>
    </ol>
    <h2>Ominaisuudet</h2>
    <ul>
        <li><strong>WhatsApp-integraatio</strong> &mdash; Lähetä ja vastaanota WhatsApp-viestejä tekstiviestillä</li>
        <li><strong>Telegram-integraatio</strong> &mdash; Telegram-viestit SMS:llä</li>
        <li><strong>Signal-integraatio</strong> &mdash; Turvallinen viestintä ilman älypuhelinta</li>
        <li><strong>Sähköposti</strong> &mdash; Gmail ja Outlook tekstiviestillä</li>
        <li><strong>Google-kalenteri</strong> &mdash; Tapahtumat ja muistutukset</li>
        <li><strong>Tekoälyhaku</strong> &mdash; Internethaku tekstiviestillä</li>
        <li><strong>GPS-navigointi</strong> &mdash; Ajo-ohjeet tekstiviestillä</li>
        <li><strong>Tesla-ohjaus</strong> &mdash; Lukitus, ilmastointi, akku SMS:llä</li>
    </ul>
    <h2>Hinnoittelu</h2>
    <p>Alkaen 29 &euro;/kk (Assistant) tai 49 &euro;/kk (Autopilot). <a href="/pricing">Katso hinnoittelu</a>.</p>
    <p><a href="/register">Rekisteröidy nyt</a></p>
    "#.to_string(),
    })
}

pub async fn fi_feature_whatsapp() -> Html<String> {
    render_page(SeoPage {
        title: "WhatsApp Ilman Älypuhelinta \u{2013} WhatsApp Tyhmäpuhelimella | Lightfriend",
        description: "Käytä WhatsAppia ilman älypuhelinta. Lähetä ja vastaanota WhatsApp-viestejä tekstiviestillä Light Phonella, Nokialla tai millä tahansa puhelimella.",
        canonical: "https://lightfriend.ai/fi/features/whatsapp-dumbphone",
        og_type: "website",
        og_image: OG_IMAGE,
        json_ld: String::new(),
        lang: "fi",
        body_content: r#"
    <h1>WhatsApp Ilman Älypuhelinta</h1>
    <p>WhatsApp on maailman suosituin viestisovellus, mutta se vaatii normaalisti älypuhelimen. Lightfriendin avulla voit lähettää ja vastaanottaa WhatsApp-viestejä tekstiviestillä mistä tahansa puhelimesta.</p>
    <h2>Näin se toimii</h2>
    <p>Lightfriend toimii siltana WhatsAppin ja SMS:n välillä. Kun joku lähettää sinulle WhatsApp-viestin, Lightfriend välittää sen puhelimeesi tekstiviestinä. Kun haluat vastata, lähetä tekstiviesti Lightfriendille.</p>
    <h2>Ominaisuudet</h2>
    <ul>
        <li>Lähetä viestejä WhatsApp-kontakteille</li>
        <li>Vastaanota WhatsApp-viestit tekstiviesteinä</li>
        <li>Ilmoitukset tärkeistä viesteistä</li>
        <li>WhatsApp-ryhmien tuki</li>
        <li>24/7 seuranta Autopilot-tilauksella</li>
    </ul>
    <p><a href="/register">Aloita nyt</a> | <a href="/pricing">Hinnoittelu</a></p>
    "#.to_string(),
    })
}

pub async fn fi_feature_telegram() -> Html<String> {
    render_page(SeoPage {
        title: "Telegram Ilman Älypuhelinta \u{2013} Telegram Tyhmäpuhelimella | Lightfriend",
        description: "Käytä Telegramia ilman älypuhelinta. Lähetä ja vastaanota Telegram-viestejä tekstiviestillä mistä tahansa puhelimesta.",
        canonical: "https://lightfriend.ai/fi/features/telegram-dumbphone",
        og_type: "website",
        og_image: OG_IMAGE,
        json_ld: String::new(),
        lang: "fi",
        body_content: r#"
    <h1>Telegram Ilman Älypuhelinta</h1>
    <p>Telegram on suosittu viestisovellus, mutta se vaatii älypuhelimen tai tietokoneen. Lightfriendin avulla voit käyttää Telegramia tekstiviestillä mistä tahansa puhelimesta.</p>
    <h2>Ominaisuudet</h2>
    <ul>
        <li>Lähetä ja vastaanota Telegram-viestejä SMS:llä</li>
        <li>Telegram-ryhmät ja -kanavat</li>
        <li>Ilmoitukset tärkeistä viesteistä</li>
        <li>Päivittäiset yhteenvedot</li>
    </ul>
    <p><a href="/register">Aloita nyt</a> | <a href="/pricing">Hinnoittelu</a></p>
    "#.to_string(),
    })
}

pub async fn fi_feature_signal() -> Html<String> {
    render_page(SeoPage {
        title: "Signal Ilman Älypuhelinta \u{2013} Signal Tyhmäpuhelimella | Lightfriend",
        description: "Käytä Signalia ilman älypuhelinta. Turvallinen viestintä tekstiviestillä mistä tahansa puhelimesta Lightfriendin avulla.",
        canonical: "https://lightfriend.ai/fi/features/signal-dumbphone",
        og_type: "website",
        og_image: OG_IMAGE,
        json_ld: String::new(),
        lang: "fi",
        body_content: r#"
    <h1>Signal Ilman Älypuhelinta</h1>
    <p>Signal on tunnettu yksityisyydestään. Lightfriendin avulla voit käyttää Signalia tekstiviestillä ilman älypuhelinta.</p>
    <h2>Ominaisuudet</h2>
    <ul>
        <li>Lähetä ja vastaanota Signal-viestejä SMS:llä</li>
        <li>Turvallinen viestintä</li>
        <li>Ilmoitukset tärkeistä viesteistä</li>
    </ul>
    <p><a href="/register">Aloita nyt</a> | <a href="/pricing">Hinnoittelu</a></p>
    "#.to_string(),
    })
}

pub async fn fi_feature_email() -> Html<String> {
    render_page(SeoPage {
        title: "Sähköposti Tyhmäpuhelimella \u{2013} Gmail ja Outlook SMS:llä | Lightfriend",
        description: "Lue ja lähetä sähköpostia tyhmäpuhelimella. Gmail ja Outlook tekstiviestillä Lightfriendin avulla.",
        canonical: "https://lightfriend.ai/fi/features/email-sms",
        og_type: "website",
        og_image: OG_IMAGE,
        json_ld: String::new(),
        lang: "fi",
        body_content: r#"
    <h1>Sähköposti Tyhmäpuhelimella</h1>
    <p>Sähköposti on välttämätön, mutta tyhmäpuhelimissa ei ole sähköpostisovelluksia. Lightfriend antaa sinulle sähköpostipääsyn tekstiviestillä.</p>
    <h2>Tuetut palvelut</h2>
    <ul><li>Gmail</li><li>Microsoft Outlook</li></ul>
    <h2>Mitä voit tehdä</h2>
    <ul>
        <li>Lue saapuvat sähköpostit tekstiviesteinä</li>
        <li>Vastaa sähköposteihin SMS:llä</li>
        <li>Kirjoita ja lähetä uusia sähköposteja</li>
        <li>Hälytykset tärkeistä viesteistä</li>
    </ul>
    <p><a href="/register">Aloita nyt</a> | <a href="/pricing">Hinnoittelu</a></p>
    "#.to_string(),
    })
}

pub async fn fi_feature_calendar() -> Html<String> {
    render_page(SeoPage {
        title: "Kalenteri Tyhmäpuhelimella \u{2013} Google-kalenteri SMS:llä | Lightfriend",
        description: "Google-kalenteri tyhmäpuhelimella. Tapahtumat, muistutukset ja uusien tapahtumien luonti tekstiviestillä.",
        canonical: "https://lightfriend.ai/fi/features/calendar-sms",
        og_type: "website",
        og_image: OG_IMAGE,
        json_ld: String::new(),
        lang: "fi",
        body_content: r#"
    <h1>Kalenteri Tyhmäpuhelimella</h1>
    <p>Älä missaa tapaamisia. Lightfriend yhdistää Google-kalenteriisi ja hallitsee aikatauluasi tekstiviestillä.</p>
    <h2>Ominaisuudet</h2>
    <ul>
        <li>Näe tulevat tapahtumat tekstiviestillä</li>
        <li>Luo uusia tapahtumia</li>
        <li>SMS-muistutukset ennen tapaamisia</li>
        <li>Päivittäiset aikatauluyhteenvedot</li>
    </ul>
    <p><a href="/register">Aloita nyt</a> | <a href="/pricing">Hinnoittelu</a></p>
    "#.to_string(),
    })
}

pub async fn fi_feature_tesla() -> Html<String> {
    render_page(SeoPage {
        title: "Tesla-ohjaus Tekstiviestillä \u{2013} Ohjaa Teslaa Ilman Älypuhelinta | Lightfriend",
        description: "Ohjaa Teslaasi tekstiviestillä. Lukitus, ilmastointi, akun tarkistus ja muuta ilman Tesla-sovellusta.",
        canonical: "https://lightfriend.ai/fi/features/tesla-sms",
        og_type: "website",
        og_image: OG_IMAGE,
        json_ld: String::new(),
        lang: "fi",
        body_content: r#"
    <h1>Tesla-ohjaus Tekstiviestillä</h1>
    <p>Tesla-omistajat eivät tarvitse älypuhelinta autonsa ohjaamiseen. Lightfriendin avulla voit hallita Teslaasi tekstiviesteillä.</p>
    <h2>Mitä voit tehdä</h2>
    <ul>
        <li>Lukitse ja avaa Tesla</li>
        <li>Käynnistä ja sammuta ilmastointi</li>
        <li>Tarkista akun taso ja toimintamatka</li>
        <li>Avaa tavaratila</li>
        <li>Käynnistä ja lopeta lataus</li>
    </ul>
    <p><a href="/register">Aloita nyt</a> | <a href="/pricing">Hinnoittelu</a></p>
    "#.to_string(),
    })
}

pub async fn fi_for_adhd() -> Html<String> {
    render_page(SeoPage {
        title: "Paras Puhelin ADHD:lle \u{2013} Tyhmäpuhelin + Tekoälyavustaja | Lightfriend",
        description: "Paras puhelinratkaisu ADHD:lle. Tyhmäpuhelin ja Lightfriend poistavat häiriötekijät ja säilyttävät olennaiset viestintävälineet.",
        canonical: "https://lightfriend.ai/fi/for/adhd",
        og_type: "website",
        og_image: OG_IMAGE,
        json_ld: String::new(),
        lang: "fi",
        body_content: r#"
    <h1>Paras Puhelin ADHD:lle</h1>
    <p>Jos sinulla on ADHD, älypuhelimesi toimii sinua vastaan. Jatkuvat ilmoitukset, loputon selaaminen ja sovellusten vaihto on suunniteltu vangitsemaan huomiosi. Tyhmäpuhelin poistaa nämä häiriötekijät kokonaan.</p>
    <h2>Miksi tyhmäpuhelin auttaa ADHD:ssä</h2>
    <ul>
        <li><strong>Ei loputonta selaamista</strong> &mdash; Sosiaalinen media ja uutissyötteet ovat poissa</li>
        <li><strong>Ei impulsiivista tarkistamista</strong> &mdash; Ilman sovellusten täyttämää kosketusnäyttöä ei ole mitään pakonomaisesti tarkistettavaa</li>
        <li><strong>Vähemmän kontekstin vaihtoa</strong> &mdash; Puhelin tekee puhelut ja tekstiviestit</li>
        <li><strong>Parempi uni</strong> &mdash; Ei sinistä valoa ennen nukkumaanmenoa</li>
    </ul>
    <h2>Entä viestit, sähköposti, kalenteri?</h2>
    <p>Lightfriend hoitaa digitaaliset palvelut joita tarvitset: WhatsApp, Telegram, Signal, sähköposti, kalenteri ja haku tekstiviestillä.</p>
    <p><a href="/register">Aloita nyt</a> | <a href="/pricing">Hinnoittelu</a></p>
    "#.to_string(),
    })
}

pub async fn fi_for_digital_detox() -> Html<String> {
    render_page(SeoPage {
        title: "Digidetox Ilman Eristäytymistä \u{2013} Lightfriend",
        description: "Tee digidetox menettämättä pääsyä WhatsAppiin, sähköpostiin ja kalenteriin. Lightfriend pitää sinut yhteydessä tekstiviestillä.",
        canonical: "https://lightfriend.ai/fi/for/digital-detox",
        og_type: "website",
        og_image: OG_IMAGE,
        json_ld: String::new(),
        lang: "fi",
        body_content: r#"
    <h1>Digidetox: Pysy Yhteydessä Ilman Älypuhelinta</h1>
    <p>Digidetox ei tarkoita itsesi eristämistä maailmasta. Lightfriendin avulla voit siirtyä tyhmäpuhelimeen ja silti vastaanottaa tärkeät viestit, sähköpostit ja kalenterimuistutukset.</p>
    <h2>Mitä säilytät detoksin aikana</h2>
    <ul>
        <li>WhatsApp-, Telegram- ja Signal-viestit</li>
        <li>Sähköpostihälytykset</li>
        <li>Kalenterimuistutukset</li>
        <li>Nettihaku tarvittaessa</li>
    </ul>
    <h2>Mistä pääset eroon (hyvä juttu)</h2>
    <ul>
        <li>Sosiaalisen median riippuvuus</li>
        <li>Loputon selaaminen</li>
        <li>Ilmoitustulva</li>
        <li>Impulsiivinen puhelimen tarkistaminen</li>
    </ul>
    <p><a href="/register">Aloita digidetox</a> | <a href="/features/wellness">Hyvinvointityökalut</a></p>
    "#.to_string(),
    })
}

pub async fn fi_for_light_phone() -> Html<String> {
    render_page(SeoPage {
        title: "Lightfriend Light Phonelle \u{2013} Täydellinen Light Phone -kumppani",
        description: "Täydellinen kumppanisovellus Light Phone 2:lle ja Light Phone 3:lle. Lisää WhatsApp, sähköposti ja kalenteri Light Phoneesi.",
        canonical: "https://lightfriend.ai/fi/for/light-phone",
        og_type: "website",
        og_image: OG_IMAGE,
        json_ld: String::new(),
        lang: "fi",
        body_content: r#"
    <h1>Lightfriend Light Phonelle</h1>
    <p>Light Phone on kauniisti suunniteltu minimalistinen puhelin. Lightfriend on sen täydellinen kumppani &mdash; lisää WhatsApp, sähköposti, kalenteri ja tekoälyominaisuudet ilman Light Phone -kokemuksen heikentämistä.</p>
    <h2>Mitä Lightfriend lisää Light Phoneesi</h2>
    <ul>
        <li>WhatsApp, Telegram ja Signal SMS:llä</li>
        <li>Sähköposti (Gmail, Outlook)</li>
        <li>Google-kalenteri muistutuksineen</li>
        <li>Tekoälyhaku</li>
        <li>GPS-navigointi</li>
        <li>Tesla-ohjaus</li>
        <li>Äänitekoälyavustaja</li>
    </ul>
    <p><a href="/register">Hanki Lightfriend Light Phoneellesi</a></p>
    "#.to_string(),
    })
}

pub async fn fi_for_nokia() -> Html<String> {
    render_page(SeoPage {
        title: "Lightfriend Nokia-puhelimille \u{2013} Älyominaisuudet Nokia-simpukkaan",
        description: "Lisää WhatsApp, sähköposti ja kalenteri Nokia-simpukkapuhelimeesi. Toimii Nokia 2780, 2760 ja kaikkien Nokia-puhelimien kanssa.",
        canonical: "https://lightfriend.ai/fi/for/nokia",
        og_type: "website",
        og_image: OG_IMAGE,
        json_ld: String::new(),
        lang: "fi",
        body_content: r#"
    <h1>Lightfriend Nokia-puhelimille</h1>
    <p>Nokia-simpukat ovat luotettavia, edullisia ja niillä on erinomainen akunkesto. Lightfriendin avulla voit lisätä WhatsAppin, sähköpostin ja kalenterin mihin tahansa Nokiaan.</p>
    <h2>Yhteensopivat Nokia-puhelimet</h2>
    <ul>
        <li>Nokia 2780 Flip</li>
        <li>Nokia 2760 Flip</li>
        <li>Nokia 225</li>
        <li>Mikä tahansa Nokia SMS-tuella</li>
    </ul>
    <p><a href="/register">Aloita nyt</a> | <a href="/pricing">Hinnoittelu</a></p>
    "#.to_string(),
    })
}

pub async fn fi_for_parents() -> Html<String> {
    render_page(SeoPage {
        title: "Turvallinen Ensipuhelin Lapselle \u{2013} Tyhmäpuhelin + Lightfriend",
        description: "Turvallisin ensipuhelin lapsille. Tyhmäpuhelin ja Lightfriend tarjoavat perusviestinnän ilman sosiaalista mediaa tai addiktoivia sovelluksia.",
        canonical: "https://lightfriend.ai/fi/for/parents",
        og_type: "website",
        og_image: OG_IMAGE,
        json_ld: String::new(),
        lang: "fi",
        body_content: r#"
    <h1>Turvallinen Ensipuhelin Lapselle</h1>
    <p>Älypuhelimen antaminen lapselle tarkoittaa altistamista sosiaaliselle medialle, addiktoiville sovelluksille ja sopimattomalle sisällölle. Tyhmäpuhelin ja Lightfriend tarjoavat turvallisen perusviestinnän.</p>
    <h2>Miksi tyhmäpuhelin on turvallisempi</h2>
    <ul>
        <li>Ei sosiaalista mediaa</li>
        <li>Ei selainta sopimattomalle sisällölle</li>
        <li>Ei sovelluskauppaa</li>
        <li>Pitkä akunkesto</li>
        <li>Kestävä ja edullinen</li>
    </ul>
    <h2>Mitä Lightfriend lisää</h2>
    <ul>
        <li>WhatsApp-yhteys kavereihin</li>
        <li>Sähköposti kouluviestintään</li>
        <li>Kalenterimuistutukset</li>
    </ul>
    <p><a href="/register">Hanki turvallinen puhelin lapsellesi</a></p>
    "#.to_string(),
    })
}

pub async fn fi_for_business() -> Html<String> {
    render_page(SeoPage {
        title: "Tyhmäpuhelin Työkäyttöön \u{2013} Ammattilaisen Minimalistinen Puhelin | Lightfriend",
        description: "Käytä tyhmäpuhelinta työssä. Sähköposti, kalenteri ja viestit SMS:llä. Poista häiriötekijät työpäivän aikana.",
        canonical: "https://lightfriend.ai/fi/for/business",
        og_type: "website",
        og_image: OG_IMAGE,
        json_ld: String::new(),
        lang: "fi",
        body_content: r#"
    <h1>Tyhmäpuhelin Työkäyttöön</h1>
    <p>Yhä useampi ammattilainen siirtyy tyhmäpuhelimeen työaikana häiriötekijöiden poistamiseksi. Lightfriendin avulla säilytät pääsyn kriittisiin työkaluihin.</p>
    <h2>Liiketoimintaominaisuudet</h2>
    <ul>
        <li>Sähköposti (Gmail, Outlook) kiireelliseen viestintään</li>
        <li>Google-kalenteri kokouksille</li>
        <li>WhatsApp/Telegram tiimin viestintään</li>
        <li>Tekoälyhaku nopeisiin tarkistuksiin</li>
        <li>24/7 seuranta kriittisille viesteille</li>
    </ul>
    <p><a href="/register">Aloita nyt</a> | <a href="/pricing">Hinnoittelu</a></p>
    "#.to_string(),
    })
}

pub async fn fi_pricing() -> Html<String> {
    render_page(SeoPage {
        title: "Hinnoittelu \u{2013} Lightfriend Tekoälyavustaja",
        description: "Lightfriend-hinnoittelu alkaen 29 €/kk. SMS, puhelut, WhatsApp, Telegram, Signal, sähköposti, kalenteri ja paljon muuta.",
        canonical: "https://lightfriend.ai/fi/pricing",
        og_type: "website",
        og_image: OG_IMAGE,
        json_ld: String::new(),
        lang: "fi",
        body_content: r#"
    <h1>Lightfriend Hinnoittelu</h1>
    <h2>Assistant &mdash; 29 &euro;/kk</h2>
    <ul>
        <li>SMS-chat tekoälyavustajan kanssa</li>
        <li>Puhelut tekoälyn kanssa</li>
        <li>Nettihaku, sää, navigointi</li>
        <li>WhatsApp, Telegram, Signal, sähköposti, kalenteri</li>
        <li>Päivittäiset yhteenvedot</li>
    </ul>
    <h2>Autopilot &mdash; 49 &euro;/kk</h2>
    <ul>
        <li>Kaikki Assistant-tilauksessa</li>
        <li>24/7 viestien seuranta</li>
        <li>Välittömät hälytykset tärkeistä viesteistä</li>
        <li>Prioriteettilähettäjien ilmoitukset</li>
    </ul>
    <p>Saatavilla yli 40 maassa. <a href="/supported-countries">Katso tuetut maat</a>.</p>
    <p><a href="/register">Rekisteröidy nyt</a></p>
    "#.to_string(),
    })
}

pub async fn fi_faq() -> Html<String> {
    render_page(SeoPage {
        title: "UKK \u{2013} Lightfriend Tekoälyavustaja Tyhmäpuhelimille",
        description: "Usein kysytyt kysymykset Lightfriendistä. Miten se toimii, tuetut puhelimet, hinnoittelu ja yksityisyys.",
        canonical: "https://lightfriend.ai/fi/faq",
        og_type: "website",
        og_image: OG_IMAGE,
        json_ld: String::new(),
        lang: "fi",
        body_content: r#"
    <h1>Usein Kysytyt Kysymykset</h1>
    <h2>Mikä on Lightfriend?</h2>
    <p>Lightfriend on tekoälyavustaja tyhmäpuhelimille. Sen avulla pääset käyttämään WhatsAppia, Telegramia, Signalia, sähköpostia, kalenteria, nettihakua ja paljon muuta tekstiviestillä ja puheluilla.</p>
    <h2>Mitkä puhelimet toimivat?</h2>
    <p>Mikä tahansa puhelin joka voi lähettää tekstiviestejä: Light Phone 2 ja 3, Nokia-simpukat, mikä tahansa peruspuhelin.</p>
    <h2>Paljonko se maksaa?</h2>
    <p>Assistant: 29 &euro;/kk. Autopilot: 49 &euro;/kk. <a href="/pricing">Täydet hintatiedot</a>.</p>
    <h2>Missä maissa palvelu toimii?</h2>
    <p>Täysi palvelu Suomessa, USA:ssa, Kanadassa, Britanniassa, Hollannissa ja Australiassa. Ilmoituspalvelu 30+ maassa. <a href="/supported-countries">Täysi lista</a>.</p>
    <p><a href="/register">Rekisteröidy nyt</a></p>
    "#.to_string(),
    })
}

// ─── Blog Posts ───

pub async fn best_dumbphones_2026() -> Html<String> {
    let json_ld = r#"<script type="application/ld+json">
    {"@context":"https://schema.org","@type":"BlogPosting","headline":"Best Dumbphones in 2026: Complete Buyer's Guide","description":"Comprehensive guide to the best dumbphones and minimalist phones in 2026, including Light Phone 3, Nokia 2780, Punkt MP02, and more.","url":"https://lightfriend.ai/blog/best-dumbphones-2026","datePublished":"2026-03-03","author":{"@type":"Organization","name":"Lightfriend"},"publisher":{"@type":"Organization","name":"Lightfriend","logo":{"@type":"ImageObject","url":"https://lightfriend.ai/assets/fav.png"}}}
    </script>"#;
    render_page(SeoPage {
        title: "Best Dumbphones in 2026: Complete Buyer\u{2019}s Guide",
        description: "Comprehensive guide to the best dumbphones and minimalist phones in 2026, including Light Phone 3, Nokia 2780, Punkt MP02, and more.",
        canonical: "https://lightfriend.ai/blog/best-dumbphones-2026",
        og_type: "article",
        og_image: OG_IMAGE,
        json_ld: json_ld.to_string(),
        lang: "en",
        body_content: r#"
    <h1>Best Dumbphones in 2026: Complete Buyer's Guide</h1>
    <p>The dumbphone market has exploded in 2026. Whether you want a full digital detox or just a distraction-free phone, here are the best options available right now.</p>
    <h2>Top Picks</h2>
    <ul>
        <li><strong>Light Phone 3</strong> &mdash; Premium e-ink minimalist phone with limited app support. Best for committed minimalists.</li>
        <li><strong>Nokia 2780 Flip</strong> &mdash; Reliable KaiOS flip phone with basic apps. Best value option.</li>
        <li><strong>Punkt MP02</strong> &mdash; Swiss-designed 4G feature phone with Signal support. Best for design enthusiasts.</li>
        <li><strong>CAT B35</strong> &mdash; Rugged KaiOS phone built for tough environments. Best for outdoor use.</li>
        <li><strong>Nokia 2760 Flip</strong> &mdash; Budget-friendly flip phone with solid call and text performance.</li>
    </ul>
    <p>All of these phones work with <a href="/">Lightfriend</a>, which adds WhatsApp, Telegram, Signal, email, calendar, and AI search via SMS. No apps needed &mdash; just text your Lightfriend number.</p>
    <p><a href="/register">Try Lightfriend free</a> | <a href="/pricing">See pricing</a></p>
    "#.to_string(),
    })
}

pub async fn adhd_and_smartphones() -> Html<String> {
    let json_ld = r#"<script type="application/ld+json">
    {"@context":"https://schema.org","@type":"BlogPosting","headline":"ADHD and Smartphones: Why Dumbphones Are the Ultimate ADHD Tool","description":"How smartphones worsen ADHD symptoms and why switching to a dumbphone can dramatically improve focus, productivity, and mental health.","url":"https://lightfriend.ai/blog/adhd-and-smartphones","datePublished":"2026-03-03","author":{"@type":"Organization","name":"Lightfriend"},"publisher":{"@type":"Organization","name":"Lightfriend","logo":{"@type":"ImageObject","url":"https://lightfriend.ai/assets/fav.png"}}}
    </script>"#;
    render_page(SeoPage {
        title: "ADHD and Smartphones: Why Dumbphones Are the Ultimate ADHD Tool",
        description: "How smartphones worsen ADHD symptoms and why switching to a dumbphone can dramatically improve focus, productivity, and mental health.",
        canonical: "https://lightfriend.ai/blog/adhd-and-smartphones",
        og_type: "article",
        og_image: OG_IMAGE,
        json_ld: json_ld.to_string(),
        lang: "en",
        body_content: r#"
    <h1>ADHD and Smartphones: Why Dumbphones Are the Ultimate ADHD Tool</h1>
    <p>Smartphones are designed to capture attention. For people with ADHD, this is especially damaging. Infinite scroll, push notifications, and dopamine-driven app design exploit the exact executive function challenges that ADHD brains already struggle with.</p>
    <p>Switching to a dumbphone removes the most harmful triggers. No social media rabbit holes, no compulsive app-checking, no notification overload. Many ADHD adults report dramatic improvements in focus, sleep, and task completion after making the switch.</p>
    <h2>Why Dumbphones Work for ADHD</h2>
    <ul>
        <li><strong>Eliminates infinite scroll</strong> &mdash; No social media apps to get lost in</li>
        <li><strong>Reduces decision fatigue</strong> &mdash; Fewer choices means less executive function drain</li>
        <li><strong>Improves sleep</strong> &mdash; No late-night screen time from phone use</li>
        <li><strong>Boosts task completion</strong> &mdash; Fewer interruptions means you finish what you start</li>
    </ul>
    <p>Use <a href="/">Lightfriend</a> to keep essential tools like messaging, email, and calendar accessible via SMS without the distracting smartphone interface.</p>
    <p><a href="/register">Get started</a> | <a href="/for/adhd">More about Lightfriend for ADHD</a></p>
    "#.to_string(),
    })
}

pub async fn whatsapp_without_smartphone() -> Html<String> {
    let json_ld = r#"<script type="application/ld+json">
    {"@context":"https://schema.org","@type":"BlogPosting","headline":"How to Use WhatsApp Without a Smartphone (2026)","description":"Step-by-step guide to accessing WhatsApp without a smartphone. Use WhatsApp on a dumbphone, flip phone, or basic phone via SMS with Lightfriend.","url":"https://lightfriend.ai/blog/whatsapp-without-smartphone","datePublished":"2026-03-03","author":{"@type":"Organization","name":"Lightfriend"},"publisher":{"@type":"Organization","name":"Lightfriend","logo":{"@type":"ImageObject","url":"https://lightfriend.ai/assets/fav.png"}}}
    </script>"#;
    render_page(SeoPage {
        title: "How to Use WhatsApp Without a Smartphone (2026)",
        description: "Step-by-step guide to accessing WhatsApp without a smartphone. Use WhatsApp on a dumbphone, flip phone, or basic phone via SMS with Lightfriend.",
        canonical: "https://lightfriend.ai/blog/whatsapp-without-smartphone",
        og_type: "article",
        og_image: OG_IMAGE,
        json_ld: json_ld.to_string(),
        lang: "en",
        body_content: r#"
    <h1>How to Use WhatsApp Without a Smartphone (2026)</h1>
    <p>WhatsApp is the world's most popular messaging app, but it requires a smartphone to run. If you use a dumbphone, flip phone, or basic phone, you can still send and receive WhatsApp messages using Lightfriend.</p>
    <h2>How It Works</h2>
    <ul>
        <li>Sign up at <a href="/register">lightfriend.ai</a> and connect your WhatsApp account</li>
        <li>Lightfriend bridges your WhatsApp to SMS</li>
        <li>Send a text to your Lightfriend number to message any WhatsApp contact</li>
        <li>Receive WhatsApp messages as SMS on your dumbphone</li>
    </ul>
    <p>This works on any phone that can send texts: Light Phone 3, Nokia flip phones, or any basic phone. No apps, no internet connection needed on your phone.</p>
    <p><a href="/register">Start using WhatsApp on your dumbphone</a> | <a href="/features/whatsapp-dumbphone">WhatsApp feature details</a></p>
    "#.to_string(),
    })
}

pub async fn digital_detox_guide() -> Html<String> {
    let json_ld = r#"<script type="application/ld+json">
    {"@context":"https://schema.org","@type":"BlogPosting","headline":"Digital Detox Guide: Everything You Need to Know","description":"Complete guide to digital detox in 2026. Learn how to reduce screen time, switch to a dumbphone, and reclaim your attention without losing connectivity.","url":"https://lightfriend.ai/blog/digital-detox-guide","datePublished":"2026-03-03","author":{"@type":"Organization","name":"Lightfriend"},"publisher":{"@type":"Organization","name":"Lightfriend","logo":{"@type":"ImageObject","url":"https://lightfriend.ai/assets/fav.png"}}}
    </script>"#;
    render_page(SeoPage {
        title: "Digital Detox Guide: Everything You Need to Know",
        description: "Complete guide to digital detox in 2026. Learn how to reduce screen time, switch to a dumbphone, and reclaim your attention without losing connectivity.",
        canonical: "https://lightfriend.ai/blog/digital-detox-guide",
        og_type: "article",
        og_image: OG_IMAGE,
        json_ld: json_ld.to_string(),
        lang: "en",
        body_content: r#"
    <h1>Digital Detox Guide: Everything You Need to Know</h1>
    <p>A digital detox means intentionally reducing your dependence on smartphones and addictive technology. It does not mean going offline entirely &mdash; it means removing the harmful, attention-stealing parts while keeping the tools you actually need.</p>
    <p>The average person spends over 4 hours daily on their smartphone. Most of that time is spent on social media, short-form video, and compulsive checking &mdash; none of which adds real value to life.</p>
    <h2>Steps to a Successful Digital Detox</h2>
    <ul>
        <li><strong>Switch to a dumbphone</strong> &mdash; Remove the temptation entirely</li>
        <li><strong>Set up messaging bridges</strong> &mdash; Use Lightfriend to keep WhatsApp, Telegram, Signal accessible via SMS</li>
        <li><strong>Block distracting sites on your computer</strong> &mdash; Use website blockers for social media</li>
        <li><strong>Replace screen time with analog activities</strong> &mdash; Reading, exercise, hobbies</li>
    </ul>
    <p><a href="/">Lightfriend</a> makes digital detox practical by keeping you connected to essential messaging, email, and calendar without a smartphone.</p>
    <p><a href="/register">Start your detox</a> | <a href="/for/digital-detox">Lightfriend for digital detox</a></p>
    "#.to_string(),
    })
}

pub async fn tesla_sms_control() -> Html<String> {
    let json_ld = r#"<script type="application/ld+json">
    {"@context":"https://schema.org","@type":"BlogPosting","headline":"Tesla Control via SMS: Manage Your Tesla Without a Smartphone","description":"How to control your Tesla via SMS from any phone. Lock, unlock, check battery, start climate control, and more without needing the Tesla app.","url":"https://lightfriend.ai/blog/tesla-sms-control","datePublished":"2026-03-03","author":{"@type":"Organization","name":"Lightfriend"},"publisher":{"@type":"Organization","name":"Lightfriend","logo":{"@type":"ImageObject","url":"https://lightfriend.ai/assets/fav.png"}}}
    </script>"#;
    render_page(SeoPage {
        title: "Tesla Control via SMS: Manage Your Tesla Without a Smartphone",
        description: "How to control your Tesla via SMS from any phone. Lock, unlock, check battery, start climate control, and more without needing the Tesla app.",
        canonical: "https://lightfriend.ai/blog/tesla-sms-control",
        og_type: "article",
        og_image: OG_IMAGE,
        json_ld: json_ld.to_string(),
        lang: "en",
        body_content: r#"
    <h1>Tesla Control via SMS: Manage Your Tesla Without a Smartphone</h1>
    <p>Tesla owners who switch to dumbphones face a challenge: the Tesla app requires a smartphone. Lightfriend solves this by letting you control your Tesla via SMS from any phone.</p>
    <h2>What You Can Do via SMS</h2>
    <ul>
        <li><strong>Lock and unlock</strong> &mdash; Text "unlock my Tesla" to your Lightfriend number</li>
        <li><strong>Climate control</strong> &mdash; Start heating or cooling before you get in</li>
        <li><strong>Battery status</strong> &mdash; Check charge level and range</li>
        <li><strong>Location</strong> &mdash; Find where your car is parked</li>
    </ul>
    <p>Simply connect your Tesla account through the Lightfriend dashboard, then text natural language commands to control your vehicle.</p>
    <p><a href="/register">Set up Tesla SMS control</a> | <a href="/features/tesla-sms">Tesla feature details</a></p>
    "#.to_string(),
    })
}

pub async fn lightfriend_vs_beeper() -> Html<String> {
    let json_ld = r#"<script type="application/ld+json">
    {"@context":"https://schema.org","@type":"BlogPosting","headline":"Lightfriend vs Beeper vs Bridge Apps: Which Is Best for Dumbphones?","description":"Detailed comparison of Lightfriend, Beeper, and other messaging bridge solutions for dumbphone users who need access to WhatsApp, Telegram, and Signal.","url":"https://lightfriend.ai/blog/lightfriend-vs-beeper","datePublished":"2026-03-03","author":{"@type":"Organization","name":"Lightfriend"},"publisher":{"@type":"Organization","name":"Lightfriend","logo":{"@type":"ImageObject","url":"https://lightfriend.ai/assets/fav.png"}}}
    </script>"#;
    render_page(SeoPage {
        title: "Lightfriend vs Beeper vs Bridge Apps: Which Is Best?",
        description: "Detailed comparison of Lightfriend, Beeper, and other messaging bridge solutions for dumbphone users who need access to WhatsApp, Telegram, and Signal.",
        canonical: "https://lightfriend.ai/blog/lightfriend-vs-beeper",
        og_type: "article",
        og_image: OG_IMAGE,
        json_ld: json_ld.to_string(),
        lang: "en",
        body_content: r#"
    <h1>Lightfriend vs Beeper vs Bridge Apps</h1>
    <p>If you use a dumbphone and need access to messaging apps, there are several options. Here is how they compare.</p>
    <h2>Comparison</h2>
    <ul>
        <li><strong>Lightfriend</strong> &mdash; SMS-based AI assistant. Works on any phone. Bridges WhatsApp, Telegram, Signal, email, calendar, and more. Includes AI search, GPS, Tesla control, and voice calling. No app needed.</li>
        <li><strong>Beeper</strong> &mdash; Unified messaging app that requires a smartphone or computer. Combines multiple chat protocols into one interface. Not usable on dumbphones directly.</li>
        <li><strong>Self-hosted bridges (Matrix/Mautrix)</strong> &mdash; Open-source bridging via Matrix protocol. Requires technical setup and a server. Maximum flexibility but high maintenance.</li>
    </ul>
    <p>Lightfriend is the only solution designed specifically for dumbphone users. It works via SMS with no apps, no internet, and no technical setup required on the phone.</p>
    <p><a href="/register">Try Lightfriend</a> | <a href="/pricing">See pricing</a></p>
    "#.to_string(),
    })
}

pub async fn best_ai_assistants_2026() -> Html<String> {
    let json_ld = r#"<script type="application/ld+json">
    {"@context":"https://schema.org","@type":"BlogPosting","headline":"Best AI Assistants in 2026: Complete Comparison","description":"Comparison of the best AI assistants in 2026 including Lightfriend, Siri, Google Assistant, Alexa, and ChatGPT. Which works best without a smartphone?","url":"https://lightfriend.ai/blog/best-ai-assistants-2026","datePublished":"2026-03-03","author":{"@type":"Organization","name":"Lightfriend"},"publisher":{"@type":"Organization","name":"Lightfriend","logo":{"@type":"ImageObject","url":"https://lightfriend.ai/assets/fav.png"}}}
    </script>"#;
    render_page(SeoPage {
        title: "Best AI Assistants in 2026: Complete Comparison",
        description: "Comparison of the best AI assistants in 2026 including Lightfriend, Siri, Google Assistant, Alexa, and ChatGPT. Which works best without a smartphone?",
        canonical: "https://lightfriend.ai/blog/best-ai-assistants-2026",
        og_type: "article",
        og_image: OG_IMAGE,
        json_ld: json_ld.to_string(),
        lang: "en",
        body_content: r#"
    <h1>Best AI Assistants in 2026: Complete Comparison</h1>
    <p>AI assistants have become essential tools, but most require a smartphone or smart speaker. Here is how the major options compare, especially for dumbphone users.</p>
    <h2>The Options</h2>
    <ul>
        <li><strong>Lightfriend</strong> &mdash; Works via SMS and voice calls from any phone. Integrates with messaging apps, email, calendar, and more. Designed for dumbphones.</li>
        <li><strong>Siri</strong> &mdash; Built into Apple devices. Requires an iPhone, iPad, or Mac.</li>
        <li><strong>Google Assistant</strong> &mdash; Requires Android phone or Google smart speaker.</li>
        <li><strong>Amazon Alexa</strong> &mdash; Requires Echo device or smartphone app.</li>
        <li><strong>ChatGPT</strong> &mdash; Requires smartphone app or web browser. Powerful but no phone integration.</li>
    </ul>
    <p>Lightfriend is the only AI assistant that works on any phone via SMS and voice calls, making it the best choice for dumbphone and minimalist phone users.</p>
    <p><a href="/register">Try Lightfriend</a> | <a href="/pricing">See pricing</a></p>
    "#.to_string(),
    })
}

pub async fn email_on_dumbphone() -> Html<String> {
    let json_ld = r#"<script type="application/ld+json">
    {"@context":"https://schema.org","@type":"BlogPosting","headline":"How to Get Email on a Dumbphone","description":"Guide to accessing Gmail, Outlook, and other email on a dumbphone or flip phone. Read and send emails via SMS using Lightfriend.","url":"https://lightfriend.ai/blog/email-on-dumbphone","datePublished":"2026-03-03","author":{"@type":"Organization","name":"Lightfriend"},"publisher":{"@type":"Organization","name":"Lightfriend","logo":{"@type":"ImageObject","url":"https://lightfriend.ai/assets/fav.png"}}}
    </script>"#;
    render_page(SeoPage {
        title: "How to Get Email on a Dumbphone",
        description: "Guide to accessing Gmail, Outlook, and other email on a dumbphone or flip phone. Read and send emails via SMS using Lightfriend.",
        canonical: "https://lightfriend.ai/blog/email-on-dumbphone",
        og_type: "article",
        og_image: OG_IMAGE,
        json_ld: json_ld.to_string(),
        lang: "en",
        body_content: r#"
    <h1>How to Get Email on a Dumbphone</h1>
    <p>Email is one of the biggest challenges when switching to a dumbphone. Most basic phones have no email client, and even KaiOS phones have limited email support. Lightfriend bridges this gap by letting you access email via SMS.</p>
    <h2>How Lightfriend Email Works</h2>
    <ul>
        <li>Connect your Gmail or Outlook account through the Lightfriend dashboard</li>
        <li>Get email summaries delivered as SMS digests</li>
        <li>Text your Lightfriend number to read specific emails</li>
        <li>Reply to emails by texting back</li>
        <li>Get instant SMS alerts for important emails with Autopilot plan</li>
    </ul>
    <p>No apps or internet connection needed on your phone. Works on any phone that can send SMS.</p>
    <p><a href="/register">Set up email on your dumbphone</a> | <a href="/features/email-sms">Email feature details</a></p>
    "#.to_string(),
    })
}

pub async fn home_assistant_sms() -> Html<String> {
    let json_ld = r#"<script type="application/ld+json">
    {"@context":"https://schema.org","@type":"BlogPosting","headline":"Home Assistant via SMS: Control Your Smart Home from Any Phone","description":"How to control Home Assistant smart home devices via SMS from any phone. Toggle lights, check sensors, run automations without a smartphone.","url":"https://lightfriend.ai/blog/home-assistant-sms","datePublished":"2026-03-03","author":{"@type":"Organization","name":"Lightfriend"},"publisher":{"@type":"Organization","name":"Lightfriend","logo":{"@type":"ImageObject","url":"https://lightfriend.ai/assets/fav.png"}}}
    </script>"#;
    render_page(SeoPage {
        title: "Home Assistant via SMS: Control Your Smart Home from Any Phone",
        description: "How to control Home Assistant smart home devices via SMS from any phone. Toggle lights, check sensors, run automations without a smartphone.",
        canonical: "https://lightfriend.ai/blog/home-assistant-sms",
        og_type: "article",
        og_image: OG_IMAGE,
        json_ld: json_ld.to_string(),
        lang: "en",
        body_content: r#"
    <h1>Home Assistant via SMS: Control Your Smart Home from Any Phone</h1>
    <p>Home Assistant is a powerful open-source smart home platform, but it typically requires a smartphone app or web browser. With Lightfriend's MCP integration, you can control your entire smart home via SMS.</p>
    <h2>What You Can Control</h2>
    <ul>
        <li><strong>Lights</strong> &mdash; Turn on/off, set brightness, change colors</li>
        <li><strong>Thermostat</strong> &mdash; Adjust temperature, check current settings</li>
        <li><strong>Sensors</strong> &mdash; Check temperature, humidity, motion status</li>
        <li><strong>Automations</strong> &mdash; Trigger scenes and automations by name</li>
        <li><strong>Locks and garage doors</strong> &mdash; Lock/unlock, open/close</li>
    </ul>
    <p>Set up your Home Assistant MCP server in the Lightfriend dashboard, then text natural language commands like "turn off living room lights" to your Lightfriend number.</p>
    <p><a href="/register">Set up smart home SMS control</a> | <a href="/features/smart-home-sms">Smart home feature details</a></p>
    "#.to_string(),
    })
}

pub async fn scan_qr_without_smartphone() -> Html<String> {
    let json_ld = r#"<script type="application/ld+json">
    {"@context":"https://schema.org","@type":"BlogPosting","headline":"How to Scan QR Codes Without a Smartphone","description":"Guide to scanning QR codes without a smartphone. Use MMS photo messaging to scan QR codes from a dumbphone or flip phone with Lightfriend.","url":"https://lightfriend.ai/blog/scan-qr-without-smartphone","datePublished":"2026-03-03","author":{"@type":"Organization","name":"Lightfriend"},"publisher":{"@type":"Organization","name":"Lightfriend","logo":{"@type":"ImageObject","url":"https://lightfriend.ai/assets/fav.png"}}}
    </script>"#;
    render_page(SeoPage {
        title: "How to Scan QR Codes Without a Smartphone",
        description: "Guide to scanning QR codes without a smartphone. Use MMS photo messaging to scan QR codes from a dumbphone or flip phone with Lightfriend.",
        canonical: "https://lightfriend.ai/blog/scan-qr-without-smartphone",
        og_type: "article",
        og_image: OG_IMAGE,
        json_ld: json_ld.to_string(),
        lang: "en",
        body_content: r#"
    <h1>How to Scan QR Codes Without a Smartphone</h1>
    <p>QR codes are everywhere &mdash; restaurant menus, parking meters, event tickets, Wi-Fi login. Dumbphone users often feel stuck when confronted with a QR code. Lightfriend solves this.</p>
    <h2>How to Scan QR Codes from a Dumbphone</h2>
    <ul>
        <li>Take a photo of the QR code with your phone's camera</li>
        <li>Send the photo as an MMS to your Lightfriend number</li>
        <li>Lightfriend's AI reads the QR code and sends back the content via SMS</li>
        <li>Works with URLs, Wi-Fi codes, contact cards, and more</li>
    </ul>
    <p>Any phone with a camera and MMS support can scan QR codes this way. No apps or internet connection needed on the phone itself.</p>
    <p><a href="/register">Get QR scanning on your dumbphone</a> | <a href="/features/qr-scanner">QR scanner feature details</a></p>
    "#.to_string(),
    })
}

pub async fn best_phone_adhd_2026() -> Html<String> {
    let json_ld = r#"<script type="application/ld+json">
    {"@context":"https://schema.org","@type":"BlogPosting","headline":"Best Phone for ADHD in 2026","description":"Guide to choosing the best phone for ADHD in 2026. Dumbphones, minimalist phones, and phone setups that help manage ADHD symptoms.","url":"https://lightfriend.ai/blog/best-phone-for-adhd-2026","datePublished":"2026-03-03","author":{"@type":"Organization","name":"Lightfriend"},"publisher":{"@type":"Organization","name":"Lightfriend","logo":{"@type":"ImageObject","url":"https://lightfriend.ai/assets/fav.png"}}}
    </script>"#;
    render_page(SeoPage {
        title: "Best Phone for ADHD in 2026",
        description: "Guide to choosing the best phone for ADHD in 2026. Dumbphones, minimalist phones, and phone setups that help manage ADHD symptoms.",
        canonical: "https://lightfriend.ai/blog/best-phone-for-adhd-2026",
        og_type: "article",
        og_image: OG_IMAGE,
        json_ld: json_ld.to_string(),
        lang: "en",
        body_content: r#"
    <h1>Best Phone for ADHD in 2026</h1>
    <p>For people with ADHD, the wrong phone can sabotage focus, sleep, and productivity. The right phone removes distractions while keeping you connected. Here are the best options in 2026.</p>
    <h2>Top Picks for ADHD</h2>
    <ul>
        <li><strong>Light Phone 3</strong> &mdash; E-ink display eliminates visual dopamine triggers. Minimal apps. Best overall for ADHD.</li>
        <li><strong>Nokia 2780 Flip</strong> &mdash; Physical flip mechanism creates a barrier to mindless use. Affordable.</li>
        <li><strong>Punkt MP02</strong> &mdash; No apps at all. Pure calls and texts. Best for severe ADHD distraction issues.</li>
    </ul>
    <p>Pair any of these with <a href="/">Lightfriend</a> to keep essential tools (messaging, email, calendar) accessible via SMS without the distracting smartphone interface.</p>
    <p><a href="/register">Get started with Lightfriend</a> | <a href="/blog/adhd-and-smartphones">ADHD and smartphones</a></p>
    "#.to_string(),
    })
}

pub async fn telegram_signal_without_smartphone() -> Html<String> {
    let json_ld = r#"<script type="application/ld+json">
    {"@context":"https://schema.org","@type":"BlogPosting","headline":"How to Use Telegram and Signal Without a Smartphone","description":"Guide to using Telegram and Signal on a dumbphone or flip phone. Send and receive encrypted messages via SMS without a smartphone.","url":"https://lightfriend.ai/blog/telegram-signal-without-smartphone","datePublished":"2026-03-03","author":{"@type":"Organization","name":"Lightfriend"},"publisher":{"@type":"Organization","name":"Lightfriend","logo":{"@type":"ImageObject","url":"https://lightfriend.ai/assets/fav.png"}}}
    </script>"#;
    render_page(SeoPage {
        title: "How to Use Telegram and Signal Without a Smartphone",
        description: "Guide to using Telegram and Signal on a dumbphone or flip phone. Send and receive encrypted messages via SMS without a smartphone.",
        canonical: "https://lightfriend.ai/blog/telegram-signal-without-smartphone",
        og_type: "article",
        og_image: OG_IMAGE,
        json_ld: json_ld.to_string(),
        lang: "en",
        body_content: r#"
    <h1>How to Use Telegram and Signal Without a Smartphone</h1>
    <p>Telegram and Signal are popular messaging apps, but both require a smartphone to run. If you have switched to a dumbphone, Lightfriend lets you keep using both via SMS.</p>
    <h2>Telegram on a Dumbphone</h2>
    <ul>
        <li>Connect your Telegram account through the Lightfriend dashboard</li>
        <li>Send and receive Telegram messages via SMS</li>
        <li>Get digest summaries of Telegram chats</li>
        <li>Works with groups and individual conversations</li>
    </ul>
    <h2>Signal on a Dumbphone</h2>
    <ul>
        <li>Connect Signal through the Lightfriend dashboard</li>
        <li>Send and receive Signal messages via SMS</li>
        <li>Maintain your secure messaging without a smartphone</li>
    </ul>
    <p>Both integrations work on any phone with SMS. No apps or internet needed on your phone.</p>
    <p><a href="/register">Start using Telegram and Signal on your dumbphone</a> | <a href="/features/telegram-dumbphone">Telegram details</a> | <a href="/features/signal-dumbphone">Signal details</a></p>
    "#.to_string(),
    })
}

// ─── Catch-all for blog posts by slug ───

pub async fn blog_page(Path(slug): Path<String>) -> Result<Html<String>, StatusCode> {
    match slug.as_str() {
        "best-dumbphones-2026" => Ok(best_dumbphones_2026().await),
        "adhd-and-smartphones" => Ok(adhd_and_smartphones().await),
        "whatsapp-without-smartphone" => Ok(whatsapp_without_smartphone().await),
        "digital-detox-guide" => Ok(digital_detox_guide().await),
        "tesla-sms-control" => Ok(tesla_sms_control().await),
        "lightfriend-vs-beeper" => Ok(lightfriend_vs_beeper().await),
        "best-ai-assistants-2026" => Ok(best_ai_assistants_2026().await),
        "email-on-dumbphone" => Ok(email_on_dumbphone().await),
        "home-assistant-sms" => Ok(home_assistant_sms().await),
        "scan-qr-without-smartphone" => Ok(scan_qr_without_smartphone().await),
        "best-phone-for-adhd-2026" => Ok(best_phone_adhd_2026().await),
        "telegram-signal-without-smartphone" => Ok(telegram_signal_without_smartphone().await),
        _ => Err(StatusCode::NOT_FOUND),
    }
}

// ─── Catch-all for feature pages by path ───

pub async fn feature_page(Path(feature): Path<String>) -> Result<Html<String>, StatusCode> {
    match feature.as_str() {
        "whatsapp-dumbphone" => Ok(feature_whatsapp_dumbphone().await),
        "telegram-dumbphone" => Ok(feature_telegram_dumbphone().await),
        "signal-dumbphone" => Ok(feature_signal_dumbphone().await),
        "email-sms" => Ok(feature_email_sms().await),
        "calendar-sms" => Ok(feature_calendar_sms().await),
        "tesla-sms" => Ok(feature_tesla_sms().await),
        "ai-search-sms" => Ok(feature_ai_search_sms().await),
        "gps-directions-sms" => Ok(feature_gps_directions_sms().await),
        "voice-ai" => Ok(feature_voice_ai().await),
        "autopilot" => Ok(feature_autopilot().await),
        "smart-home-sms" => Ok(feature_smart_home_sms().await),
        "qr-scanner" => Ok(feature_qr_scanner().await),
        "wellness" => Ok(feature_wellness().await),
        _ => Err(StatusCode::NOT_FOUND),
    }
}

pub async fn for_page(Path(audience): Path<String>) -> Result<Html<String>, StatusCode> {
    match audience.as_str() {
        "adhd" => Ok(for_adhd().await),
        "digital-detox" => Ok(for_digital_detox().await),
        "light-phone" => Ok(for_light_phone().await),
        "nokia" => Ok(for_nokia().await),
        "parents" => Ok(for_parents().await),
        "business" => Ok(for_business().await),
        _ => Err(StatusCode::NOT_FOUND),
    }
}

pub async fn fi_feature_page(Path(feature): Path<String>) -> Result<Html<String>, StatusCode> {
    match feature.as_str() {
        "whatsapp-dumbphone" => Ok(fi_feature_whatsapp().await),
        "telegram-dumbphone" => Ok(fi_feature_telegram().await),
        "signal-dumbphone" => Ok(fi_feature_signal().await),
        "email-sms" => Ok(fi_feature_email().await),
        "calendar-sms" => Ok(fi_feature_calendar().await),
        "tesla-sms" => Ok(fi_feature_tesla().await),
        _ => Err(StatusCode::NOT_FOUND),
    }
}

pub async fn fi_for_page(Path(audience): Path<String>) -> Result<Html<String>, StatusCode> {
    match audience.as_str() {
        "adhd" => Ok(fi_for_adhd().await),
        "digital-detox" => Ok(fi_for_digital_detox().await),
        "light-phone" => Ok(fi_for_light_phone().await),
        "nokia" => Ok(fi_for_nokia().await),
        "parents" => Ok(fi_for_parents().await),
        "business" => Ok(fi_for_business().await),
        _ => Err(StatusCode::NOT_FOUND),
    }
}

/// Check if the request is from a bot and return the appropriate pre-rendered page
pub fn check_bot(headers: &HeaderMap) -> bool {
    headers
        .get("user-agent")
        .and_then(|v| v.to_str().ok())
        .map(is_bot)
        .unwrap_or(false)
}
