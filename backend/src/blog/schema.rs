use super::content::BlogPost;

pub fn generate_schema(post: &BlogPost) -> String {
    match post.frontmatter.schema_type.as_deref() {
        Some("HowTo") => generate_howto_schema(post),
        Some("FAQPage") => generate_faq_schema(post),
        _ => generate_article_schema(post),
    }
}

fn escape_json(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "")
        .replace('\t', "\\t")
}

fn generate_article_schema(post: &BlogPost) -> String {
    let mut schema = format!(
        r#"<script type="application/ld+json">
{{
  "@context": "https://schema.org",
  "@type": "Article",
  "headline": "{}",
  "description": "{}",
  "datePublished": "{}",
  "author": {{
    "@type": "Organization",
    "name": "Lightfriend",
    "url": "https://lightfriend.ai"
  }},
  "publisher": {{
    "@type": "Organization",
    "name": "Lightfriend",
    "url": "https://lightfriend.ai"
  }},
  "mainEntityOfPage": "https://lightfriend.ai/blog/{}"
}}"#,
        escape_json(&post.frontmatter.title),
        escape_json(&post.frontmatter.description),
        post.frontmatter.date,
        post.frontmatter.slug
    );

    // Append FAQ schema if FAQs exist
    if !post.frontmatter.faqs.is_empty() {
        schema.push_str("\n</script>\n");
        schema.push_str(&generate_faq_schema(post));
        return schema;
    }

    schema.push_str("\n</script>");
    schema
}

fn generate_faq_schema(post: &BlogPost) -> String {
    if post.frontmatter.faqs.is_empty() {
        return String::new();
    }
    let entities: Vec<String> = post
        .frontmatter
        .faqs
        .iter()
        .map(|faq| {
            format!(
                r#"    {{
      "@type": "Question",
      "name": "{}",
      "acceptedAnswer": {{
        "@type": "Answer",
        "text": "{}"
      }}
    }}"#,
                escape_json(&faq.q),
                escape_json(&faq.a)
            )
        })
        .collect();

    format!(
        r#"<script type="application/ld+json">
{{
  "@context": "https://schema.org",
  "@type": "FAQPage",
  "mainEntity": [
{}
  ]
}}
</script>"#,
        entities.join(",\n")
    )
}

fn generate_howto_schema(post: &BlogPost) -> String {
    let time = post
        .frontmatter
        .estimated_time
        .as_deref()
        .unwrap_or("PT10M");

    let mut schema = format!(
        r#"<script type="application/ld+json">
{{
  "@context": "https://schema.org",
  "@type": "HowTo",
  "name": "{}",
  "description": "{}",
  "totalTime": "{}"
}}
</script>"#,
        escape_json(&post.frontmatter.title),
        escape_json(&post.frontmatter.description),
        time
    );

    // Also add FAQ schema if FAQs exist
    if !post.frontmatter.faqs.is_empty() {
        schema.push('\n');
        schema.push_str(&generate_faq_schema(post));
    }

    schema
}
