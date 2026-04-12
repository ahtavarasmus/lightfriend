use super::content::BlogPost;
use super::linking;
use super::schema;
use std::collections::HashMap;

const INLINE_CSS: &str = r#"
*, *::before, *::after { box-sizing: border-box; margin: 0; padding: 0; }
@font-face { font-family: 'Inter'; font-weight: 400; font-style: normal; font-display: swap; src: url('/assets/inter-400-latin.woff2') format('woff2'); }
@font-face { font-family: 'Inter'; font-weight: 600; font-style: normal; font-display: swap; src: url('/assets/inter-600-latin.woff2') format('woff2'); }
@font-face { font-family: 'Inter'; font-weight: 700; font-style: normal; font-display: swap; src: url('/assets/inter-700-latin.woff2') format('woff2'); }
:root {
    --surface-card: rgba(0, 0, 0, 0.3);
    --surface-card-hover: rgba(0, 0, 0, 0.4);
    --surface-subtle: rgba(255, 255, 255, 0.03);
    --border-card: rgba(255, 255, 255, 0.12);
    --border-card-hover: rgba(255, 255, 255, 0.25);
    --text-heading: #ffffff;
    --text-body: #bbbbbb;
    --text-muted: #888888;
    --text-dim: #666666;
}
body {
    font-family: 'Inter', -apple-system, BlinkMacSystemFont, sans-serif;
    background: #1a1a1a;
    color: #ffffff;
    line-height: 1.6;
    -webkit-font-smoothing: antialiased;
}
a { color: #7EB2FF; text-decoration: none; }
a:hover { text-decoration: underline; }
.top-nav {
    position: fixed; top: 0; left: 0; right: 0;
    z-index: 1002; padding: 1rem 0;
    background: rgba(26, 26, 26, 0.9); backdrop-filter: blur(10px);
    border-bottom: 1px solid rgba(255, 255, 255, 0.06);
}
.nav-content {
    max-width: 100%; margin: 0; padding: 0 3rem;
    display: flex; justify-content: space-between; align-items: center;
}
.nav-logo {
    color: white; text-decoration: none; font-size: 1.5rem; font-weight: 600;
    background: linear-gradient(45deg, #fff, #7EB2FF);
    -webkit-background-clip: text; -webkit-text-fill-color: transparent;
}
.nav-logo:hover { opacity: 0.8; text-decoration: none; }
.nav-right { display: flex; align-items: center; gap: 0.75rem; }
.nav-link {
    color: rgba(255, 255, 255, 0.75); text-decoration: none;
    padding: 0.75rem 1.5rem; border-radius: 8px; font-size: 0.9rem;
    background: rgba(180, 180, 180, 0.12); border: 1px solid rgba(200, 200, 200, 0.25);
    transition: all 0.3s ease;
}
.nav-link:hover { background: rgba(200, 200, 200, 0.2); color: #fff; text-decoration: none; transform: translateY(-2px); }
.blog-page { padding-top: 74px; min-height: 100vh; position: relative; }
.blog-hero {
    text-align: center; padding: 6rem 2rem;
    background: var(--surface-card); margin-top: 2rem;
    border: 1px solid var(--border-card); margin-bottom: 2rem;
}
.blog-hero h1 {
    font-size: 3.5rem; margin-bottom: 1.5rem;
    background: linear-gradient(45deg, #fff, #7EB2FF);
    -webkit-background-clip: text; -webkit-text-fill-color: transparent;
}
.blog-meta { color: var(--text-dim); font-size: 0.9rem; }
.blog-content { max-width: 800px; margin: 0 auto; padding: 2rem; }
.blog-content h2 {
    font-size: 2rem; margin: 3rem 0 1rem;
    background: linear-gradient(45deg, #fff, #7EB2FF);
    -webkit-background-clip: text; -webkit-text-fill-color: transparent;
}
.blog-content h3 { font-size: 1.4rem; margin: 2rem 0 0.75rem; color: #fff; }
.blog-content p { color: var(--text-body); line-height: 1.6; margin-bottom: 1.5rem; }
.blog-content ul, .blog-content ol { color: var(--text-body); padding-left: 1.5rem; margin-bottom: 1.5rem; }
.blog-content li { margin-bottom: 0.75rem; }
.blog-content table {
    width: 100%; border-collapse: collapse; margin: 2rem 0; color: #ddd;
}
.blog-content th, .blog-content td { padding: 1rem; border: 1px solid var(--border-card); text-align: left; }
.blog-content th { background: rgba(0, 0, 0, 0.5); color: #7EB2FF; }
.blog-content strong { color: #fff; }
.blog-content code { background: rgba(255,255,255,0.08); padding: 0.2rem 0.4rem; border-radius: 4px; font-size: 0.9em; }
.blog-cta {
    text-align: center; margin: 4rem auto 2rem; padding: 2rem; max-width: 800px;
    background: var(--surface-subtle); border: 1px solid var(--border-card); border-radius: 12px;
}
.blog-cta h3 {
    font-size: 2rem; margin-bottom: 1.5rem;
    background: linear-gradient(45deg, #fff, #7EB2FF);
    -webkit-background-clip: text; -webkit-text-fill-color: transparent;
}
.blog-cta p { color: #999; margin-top: 1rem; }
.hero-cta {
    display: inline-block;
    background: linear-gradient(45deg, #7EB2FF, #4169E1); color: white;
    border: none; padding: 1rem 2.5rem; border-radius: 8px;
    font-size: 1.1rem; cursor: pointer; transition: all 0.3s ease; text-decoration: none;
}
.hero-cta:hover { transform: translateY(-2px); box-shadow: 0 4px 20px rgba(126, 178, 255, 0.4); text-decoration: none; }
.faq-section { max-width: 800px; margin: 2rem auto; padding: 0 2rem; }
.faq-section h2 {
    font-size: 2rem; margin-bottom: 1.5rem;
    background: linear-gradient(45deg, #fff, #7EB2FF);
    -webkit-background-clip: text; -webkit-text-fill-color: transparent;
}
.faq-item { margin-bottom: 1.5rem; padding: 1.5rem; background: var(--surface-card); border: 1px solid var(--border-card); border-radius: 12px; }
.faq-item h3 { color: #fff; font-size: 1.1rem; margin-bottom: 0.5rem; }
.faq-item p { color: var(--text-body); margin: 0; }
.related-posts { max-width: 800px; margin: 3rem auto; padding: 0 2rem; }
.related-posts h3 {
    font-size: 1.5rem; margin-bottom: 1.5rem;
    background: linear-gradient(45deg, #fff, #7EB2FF);
    -webkit-background-clip: text; -webkit-text-fill-color: transparent;
}
.related-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(250px, 1fr)); gap: 1rem; }
.related-card {
    display: block; padding: 1.5rem; background: var(--surface-card);
    border: 1px solid var(--border-card); border-radius: 12px;
    transition: all 0.3s ease; text-decoration: none;
}
.related-card:hover { border-color: var(--border-card-hover); transform: translateY(-3px); text-decoration: none; }
.related-card h4 { color: #fff; font-size: 1rem; margin-bottom: 0.5rem; }
.related-card p { color: var(--text-muted); font-size: 0.9rem; margin: 0; }
.hub-breadcrumb { max-width: 800px; margin: 1rem auto; padding: 0 2rem; }
.hub-breadcrumb a { color: var(--text-muted); font-size: 0.9rem; }
.blog-footer {
    max-width: 800px; margin: 4rem auto 0; padding: 2rem;
    border-top: 1px solid var(--border-card); text-align: center;
}
.footer-links { display: flex; justify-content: center; gap: 1.5rem; margin-bottom: 1rem; flex-wrap: wrap; }
.footer-links a { color: var(--text-muted); font-size: 0.9rem; }
.blog-footer p { color: var(--text-dim); font-size: 0.85rem; }
/* Blog index */
.blog-list-section { max-width: 800px; margin: 0 auto; padding: 2rem; }
.blog-post-preview {
    background: var(--surface-card); border: 1px solid var(--border-card);
    border-radius: 12px; margin-bottom: 2rem; overflow: hidden; transition: all 0.3s ease;
}
.blog-post-preview:hover { border-color: var(--border-card-hover); transform: translateY(-5px); }
.blog-post-preview a { text-decoration: none; color: inherit; display: block; padding: 1.5rem; }
.blog-post-preview a:hover { text-decoration: none; }
.blog-post-preview h2 {
    font-size: 1.5rem; margin-bottom: 0.5rem;
    background: linear-gradient(45deg, #fff, #7EB2FF);
    -webkit-background-clip: text; -webkit-text-fill-color: transparent;
}
.blog-post-preview p { color: var(--text-body); margin: 0.5rem 0; }
.blog-date { color: var(--text-dim); font-size: 0.85rem; }
.cluster-section { margin-bottom: 3rem; }
.cluster-section h2 {
    font-size: 1.8rem; margin-bottom: 1.5rem;
    background: linear-gradient(45deg, #fff, #7EB2FF);
    -webkit-background-clip: text; -webkit-text-fill-color: transparent;
}
@media (max-width: 768px) {
    .nav-content { padding: 0 1rem; }
    .blog-hero { padding: 4rem 1rem; }
    .blog-hero h1 { font-size: 2.5rem; }
    .blog-content { padding: 1rem; }
    .blog-content h2 { font-size: 1.6rem; }
    .related-grid { grid-template-columns: 1fr; }
    .nav-link { padding: 0.5rem 1rem; font-size: 0.85rem; }
}
"#;

const ANALYTICS: &str = r#"
    <script defer data-website-id="68c9baf36c40f6f0060e0d5a" data-domain="lightfriend.ai" src="https://datafa.st/js/script.js"></script>
    <script async src="https://www.googletagmanager.com/gtag/js?id=G-G812WMEHC6"></script>
    <script>window.dataLayer=window.dataLayer||[];function gtag(){dataLayer.push(arguments);}gtag('js',new Date());gtag('config','G-G812WMEHC6');</script>
"#;

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn format_date(date: &str) -> String {
    // "2026-04-12" -> "April 12, 2026"
    let months = [
        "January",
        "February",
        "March",
        "April",
        "May",
        "June",
        "July",
        "August",
        "September",
        "October",
        "November",
        "December",
    ];
    let parts: Vec<&str> = date.split('-').collect();
    if parts.len() != 3 {
        return date.to_string();
    }
    let month_idx: usize = parts[1].parse::<usize>().unwrap_or(1).saturating_sub(1);
    let day: u32 = parts[2].parse().unwrap_or(1);
    let year = parts[0];
    let month_name = months.get(month_idx).unwrap_or(&"");
    format!("{} {}, {}", month_name, day, year)
}

pub fn render_blog_post(post: &BlogPost, related: &[BlogPost]) -> String {
    let schema_json_ld = schema::generate_schema(post);
    let faq_html = linking::render_faq_section(post);
    let related_html = linking::render_related_section(related);
    let hub_link = linking::render_hub_link(post);
    let keywords_meta = if post.frontmatter.keywords.is_empty() {
        String::new()
    } else {
        format!(
            r#"    <meta name="keywords" content="{}">"#,
            html_escape(&post.frontmatter.keywords.join(", "))
        )
    };
    let ai_summary_meta = match &post.frontmatter.ai_summary {
        Some(s) => format!(
            r#"    <meta name="ai-summary" content="{}">"#,
            html_escape(s)
        ),
        None => String::new(),
    };

    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0, maximum-scale=5.0">
    <title>{title} - Lightfriend</title>
    <meta name="description" content="{description}">
{keywords_meta}
{ai_summary_meta}
    <link rel="canonical" href="https://lightfriend.ai/blog/{slug}">
    <meta property="og:title" content="{title}">
    <meta property="og:description" content="{description}">
    <meta property="og:url" content="https://lightfriend.ai/blog/{slug}">
    <meta property="og:type" content="article">
    <meta property="og:site_name" content="Lightfriend">
    <meta name="twitter:card" content="summary">
    <meta name="twitter:title" content="{title}">
    <meta name="twitter:description" content="{description}">
    {schema_json_ld}
    <link rel="icon" type="image/png" href="/assets/fav.png">
{analytics}
    <style>{css}</style>
</head>
<body>
    <nav class="top-nav">
        <div class="nav-content">
            <div class="nav-left">
                <a href="/" class="nav-logo">lightfriend</a>
            </div>
            <div class="nav-right">
                <a href="/blog" class="nav-link">Blog</a>
                <a href="/pricing" class="nav-link">Pricing</a>
                <a href="/login" class="nav-link">Login</a>
            </div>
        </div>
    </nav>
    <div class="blog-page">
        {hub_link}
        <article class="blog-content" itemscope itemtype="https://schema.org/Article">
            <header class="blog-hero">
                <h1 itemprop="headline">{title}</h1>
                <p class="blog-meta">
                    <time datetime="{date}" itemprop="datePublished">{formatted_date}</time>
                </p>
            </header>
            <div itemprop="articleBody">
                {rendered_html}
            </div>
        </article>
        {faq_html}
        {related_html}
        <div class="blog-cta">
            <h3>Works with any phone that can text</h3>
            <a href="/pricing" class="hero-cta">Get Started</a>
            <p>No smartphone needed. No app to install.</p>
        </div>
    </div>
    <footer class="blog-footer">
        <div class="footer-links">
            <a href="/">Home</a>
            <a href="/blog">Blog</a>
            <a href="/pricing">Pricing</a>
            <a href="/terms">Terms</a>
            <a href="/privacy">Privacy</a>
            <a href="https://github.com/ahtavarasmus/lightfriend" target="_blank" rel="noopener noreferrer">GitHub</a>
        </div>
        <p>Lightfriend - Open source under AGPLv3</p>
    </footer>
</body>
</html>"#,
        title = html_escape(&post.frontmatter.title),
        description = html_escape(&post.frontmatter.description),
        slug = post.frontmatter.slug,
        date = post.frontmatter.date,
        formatted_date = format_date(&post.frontmatter.date),
        rendered_html = post.rendered_html,
        schema_json_ld = schema_json_ld,
        keywords_meta = keywords_meta,
        ai_summary_meta = ai_summary_meta,
        hub_link = hub_link,
        faq_html = faq_html,
        related_html = related_html,
        css = INLINE_CSS,
        analytics = ANALYTICS,
    )
}

pub fn render_blog_index(
    slugs: &[String],
    posts: &HashMap<String, BlogPost>,
    clusters: &HashMap<String, Vec<String>>,
) -> String {
    let cluster_names = [
        ("messaging", "Messaging Without a Smartphone"),
        ("ai-assistant", "AI Assistant Guides"),
        ("privacy", "Privacy and Security"),
        ("minimalism", "Digital Minimalism"),
        ("problems", "Solve Everyday Problems"),
        ("comparisons", "Comparisons"),
    ];

    // Existing hand-coded posts (link to their current top-level routes)
    let existing_posts = vec![
        ("/prompt-injection-safe", "Why Lightfriend Can't Be Prompt Injected", "Most AI assistants are powerful enough to be dangerous. Lightfriend is read-only by default.", "April 10, 2026"),
        ("/telegram-on-dumbphone", "How to Use Telegram on a Dumbphone", "Send and receive Telegram messages from any basic phone via SMS.", "April 10, 2026"),
        ("/signal-on-dumbphone", "How to Use Signal on a Dumbphone", "Use Signal encrypted messaging on any flip phone or basic phone.", "April 10, 2026"),
        ("/how-to-read-more-accidentally", "How to Read Books Accidentally", "How to Read More Without Willpower", "August 21, 2025"),
        ("/how-to-switch-to-dumbphone", "How to Switch to a Dumbphone", "All the things you need to consider when joining the dumbphone revolution.", "August 19, 2025"),
        ("/light-phone-3-whatsapp-guide", "Light Phone 3 WhatsApp Guide", "Add WhatsApp functionality to your Light Phone 3 without compromising its minimalist design.", "August 13, 2025"),
    ];

    let mut existing_cards = String::new();
    for (path, title, desc, date) in &existing_posts {
        existing_cards.push_str(&format!(
            r#"<div class="blog-post-preview"><a href="{}"><h2>{}</h2><p>{}</p><span class="blog-date">{}</span></a></div>
"#,
            path,
            html_escape(title),
            html_escape(desc),
            date
        ));
    }

    // Cluster sections with programmatic posts
    let mut cluster_sections = String::new();
    for (cluster_id, cluster_title) in &cluster_names {
        if let Some(cluster_slugs) = clusters.get(*cluster_id) {
            if cluster_slugs.is_empty() {
                continue;
            }
            let mut section_cards = String::new();
            // Show hubs first, then recent posts, cap at 6 per cluster on the index
            let mut sorted_slugs = cluster_slugs.clone();
            sorted_slugs.sort_by(|a, b| {
                let a_hub = posts.get(a).map_or(false, |p| p.frontmatter.cluster_hub);
                let b_hub = posts.get(b).map_or(false, |p| p.frontmatter.cluster_hub);
                b_hub.cmp(&a_hub).then_with(|| {
                    let da = posts.get(a).map(|p| &p.frontmatter.date);
                    let db = posts.get(b).map(|p| &p.frontmatter.date);
                    db.cmp(&da)
                })
            });
            for slug in sorted_slugs.iter().take(6) {
                if let Some(post) = posts.get(slug) {
                    section_cards.push_str(&format!(
                        r#"<div class="blog-post-preview"><a href="/blog/{}"><h2>{}</h2><p>{}</p><span class="blog-date">{}</span></a></div>
"#,
                        post.frontmatter.slug,
                        html_escape(&post.frontmatter.title),
                        html_escape(&post.frontmatter.description),
                        format_date(&post.frontmatter.date)
                    ));
                }
            }
            if !section_cards.is_empty() {
                cluster_sections.push_str(&format!(
                    r#"<div class="cluster-section"><h2>{}</h2>{}</div>"#,
                    html_escape(cluster_title),
                    section_cards
                ));
            }
        }
    }

    // Recent programmatic posts (latest 10 across all clusters)
    let mut recent_cards = String::new();
    let recent_count = slugs.len().min(10);
    for slug in slugs.iter().take(recent_count) {
        if let Some(post) = posts.get(slug) {
            recent_cards.push_str(&format!(
                r#"<div class="blog-post-preview"><a href="/blog/{}"><h2>{}</h2><p>{}</p><span class="blog-date">{}</span></a></div>
"#,
                post.frontmatter.slug,
                html_escape(&post.frontmatter.title),
                html_escape(&post.frontmatter.description),
                format_date(&post.frontmatter.date)
            ));
        }
    }

    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0, maximum-scale=5.0">
    <title>Blog - Lightfriend Guides</title>
    <meta name="description" content="Guides and insights on using messaging apps without a smartphone, AI assistants via SMS, digital minimalism, and verifiable privacy.">
    <link rel="canonical" href="https://lightfriend.ai/blog">
    <meta property="og:title" content="Blog - Lightfriend">
    <meta property="og:description" content="Guides on messaging without a smartphone, AI via SMS, and digital minimalism.">
    <meta property="og:url" content="https://lightfriend.ai/blog">
    <meta property="og:type" content="website">
    <meta property="og:site_name" content="Lightfriend">
    <link rel="icon" type="image/png" href="/assets/fav.png">
{analytics}
    <style>{css}</style>
</head>
<body>
    <nav class="top-nav">
        <div class="nav-content">
            <div class="nav-left">
                <a href="/" class="nav-logo">lightfriend</a>
            </div>
            <div class="nav-right">
                <a href="/blog" class="nav-link">Blog</a>
                <a href="/pricing" class="nav-link">Pricing</a>
                <a href="/login" class="nav-link">Login</a>
            </div>
        </div>
    </nav>
    <div class="blog-page">
        <header class="blog-hero">
            <h1>Blog</h1>
            <p class="blog-meta">Guides, insights, and answers for people who want to stay connected without a smartphone.</p>
        </header>
        <section class="blog-list-section">
            <div class="cluster-section">
                <h2>Featured</h2>
                {existing_cards}
            </div>
            {cluster_sections}
        </section>
        <div class="blog-cta">
            <h3>Works with any phone that can text</h3>
            <a href="/pricing" class="hero-cta">Get Started</a>
            <p>No smartphone needed. No app to install.</p>
        </div>
    </div>
    <footer class="blog-footer">
        <div class="footer-links">
            <a href="/">Home</a>
            <a href="/blog">Blog</a>
            <a href="/pricing">Pricing</a>
            <a href="/terms">Terms</a>
            <a href="/privacy">Privacy</a>
            <a href="https://github.com/ahtavarasmus/lightfriend" target="_blank" rel="noopener noreferrer">GitHub</a>
        </div>
        <p>Lightfriend - Open source under AGPLv3</p>
    </footer>
</body>
</html>"#,
        existing_cards = existing_cards,
        cluster_sections = cluster_sections,
        css = INLINE_CSS,
        analytics = ANALYTICS,
    )
}
