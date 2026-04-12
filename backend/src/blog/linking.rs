use super::content::BlogPost;

pub fn render_related_section(related: &[BlogPost]) -> String {
    if related.is_empty() {
        return String::new();
    }
    let cards: Vec<String> = related
        .iter()
        .map(|p| {
            format!(
                r#"<a href="/blog/{}" class="related-card">
  <h4>{}</h4>
  <p>{}</p>
</a>"#,
                p.frontmatter.slug,
                html_escape(&p.frontmatter.title),
                html_escape(&truncate(&p.frontmatter.description, 120))
            )
        })
        .collect();

    format!(
        r#"<section class="related-posts">
  <h3>Related Guides</h3>
  <div class="related-grid">
    {}
  </div>
</section>"#,
        cards.join("\n    ")
    )
}

pub fn render_hub_link(post: &BlogPost) -> String {
    match &post.frontmatter.hub_slug {
        Some(hub) => format!(
            r#"<nav class="hub-breadcrumb">
  <a href="/blog/{}">Back to guide overview</a>
</nav>"#,
            hub
        ),
        None => String::new(),
    }
}

pub fn render_faq_section(post: &BlogPost) -> String {
    if post.frontmatter.faqs.is_empty() {
        return String::new();
    }
    let items: Vec<String> = post
        .frontmatter
        .faqs
        .iter()
        .map(|faq| {
            format!(
                r#"<div class="faq-item">
  <h3>{}</h3>
  <p>{}</p>
</div>"#,
                html_escape(&faq.q),
                html_escape(&faq.a)
            )
        })
        .collect();

    format!(
        r#"<section class="faq-section">
  <h2>Frequently Asked Questions</h2>
  {}
</section>"#,
        items.join("\n  ")
    )
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    let truncated: String = s.chars().take(max).collect();
    if let Some(last_space) = truncated.rfind(' ') {
        format!("{}...", &truncated[..last_space])
    } else {
        format!("{}...", truncated)
    }
}
