use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone, Deserialize)]
pub struct FaqItem {
    pub q: String,
    pub a: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BlogFrontmatter {
    pub title: String,
    pub slug: String,
    pub description: String,
    pub date: String,
    #[serde(default)]
    pub cluster: String,
    #[serde(default)]
    pub cluster_hub: bool,
    #[serde(default)]
    pub keywords: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub schema_type: Option<String>,
    pub estimated_time: Option<String>,
    #[serde(default)]
    pub faqs: Vec<FaqItem>,
    #[serde(default)]
    pub related_slugs: Vec<String>,
    pub hub_slug: Option<String>,
    pub ai_summary: Option<String>,
}

#[derive(Debug, Clone)]
pub struct BlogPost {
    pub frontmatter: BlogFrontmatter,
    pub raw_markdown: String,
    pub rendered_html: String,
    pub full_page_html: String,
    pub word_count: usize,
}

pub struct BlogStore {
    pub posts: HashMap<String, BlogPost>,
    pub clusters: HashMap<String, Vec<String>>,
    pub all_slugs_sorted: Vec<String>,
    pub sitemap_xml: String,
    pub blog_index_html: String,
}

fn parse_frontmatter(content: &str) -> Result<(BlogFrontmatter, String), anyhow::Error> {
    let content = content.trim_start_matches('\u{feff}'); // strip BOM
    if !content.starts_with("---") {
        anyhow::bail!("missing frontmatter delimiter");
    }
    let after_first = &content[3..];
    let end = after_first
        .find("\n---")
        .ok_or_else(|| anyhow::anyhow!("missing closing frontmatter delimiter"))?;
    let yaml = &after_first[..end];
    let body = &after_first[end + 4..]; // skip \n---
    let fm: BlogFrontmatter = serde_yaml::from_str(yaml)?;
    Ok((fm, body.trim().to_string()))
}

fn render_markdown(md: &str) -> String {
    use pulldown_cmark::{html, Options, Parser};
    let opts =
        Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_HEADING_ATTRIBUTES;
    let parser = Parser::new_ext(md, opts);
    let mut html_output = String::new();
    html::push_html(&mut html_output, parser);
    html_output
}

fn count_words(text: &str) -> usize {
    text.split_whitespace().count()
}

impl BlogStore {
    pub fn empty() -> Self {
        Self {
            posts: HashMap::new(),
            clusters: HashMap::new(),
            all_slugs_sorted: vec![],
            sitemap_xml: Self::generate_sitemap(&HashMap::new()),
            blog_index_html: String::new(),
        }
    }

    pub fn load(content_dir: &str) -> Result<Self, anyhow::Error> {
        let mut posts = HashMap::new();
        let mut clusters: HashMap<String, Vec<String>> = HashMap::new();

        let path = Path::new(content_dir);
        if !path.exists() {
            tracing::warn!("Blog content directory not found: {}", content_dir);
            return Ok(Self {
                posts,
                clusters,
                all_slugs_sorted: vec![],
                sitemap_xml: Self::generate_sitemap(&HashMap::new()),
                blog_index_html: String::new(),
            });
        }

        Self::load_dir(path, &mut posts)?;

        // Build cluster index
        for (slug, post) in &posts {
            if !post.frontmatter.cluster.is_empty() {
                clusters
                    .entry(post.frontmatter.cluster.clone())
                    .or_default()
                    .push(slug.clone());
            }
        }

        // Sort slugs by date descending
        let mut all_slugs_sorted: Vec<String> = posts.keys().cloned().collect();
        all_slugs_sorted.sort_by(|a, b| {
            let da = &posts[a].frontmatter.date;
            let db = &posts[b].frontmatter.date;
            db.cmp(da)
        });

        // Now render full page HTML for each post (needs the store's related posts)
        for slug in &all_slugs_sorted {
            let post = posts.get(slug).unwrap();
            let related = Self::get_related_static(&posts, post);
            let full_html = super::templates::render_blog_post(post, &related);
            posts.get_mut(slug).unwrap().full_page_html = full_html;
        }

        let sitemap_xml = Self::generate_sitemap(&posts);
        let blog_index_html =
            super::templates::render_blog_index(&all_slugs_sorted, &posts, &clusters);

        Ok(Self {
            posts,
            clusters,
            all_slugs_sorted,
            sitemap_xml,
            blog_index_html,
        })
    }

    fn load_dir(dir: &Path, posts: &mut HashMap<String, BlogPost>) -> Result<(), anyhow::Error> {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                // Skip _data directory
                if path
                    .file_name()
                    .map_or(false, |n| n.to_str() == Some("_data"))
                {
                    continue;
                }
                Self::load_dir(&path, posts)?;
            } else if path.extension().map_or(false, |e| e == "md") {
                match Self::load_post(&path) {
                    Ok(post) => {
                        posts.insert(post.frontmatter.slug.clone(), post);
                    }
                    Err(e) => {
                        tracing::warn!("Failed to load blog post {}: {}", path.display(), e);
                    }
                }
            }
        }
        Ok(())
    }

    fn load_post(path: &Path) -> Result<BlogPost, anyhow::Error> {
        let content = std::fs::read_to_string(path)?;
        let (frontmatter, raw_markdown) = parse_frontmatter(&content)?;
        let rendered_html = render_markdown(&raw_markdown);
        let word_count = count_words(&raw_markdown);

        Ok(BlogPost {
            frontmatter,
            raw_markdown,
            rendered_html,
            full_page_html: String::new(), // filled in after all posts loaded
            word_count,
        })
    }

    pub fn get_post(&self, slug: &str) -> Option<&BlogPost> {
        self.posts.get(slug)
    }

    pub fn get_related_posts(&self, slug: &str, limit: usize) -> Vec<&BlogPost> {
        let post = match self.posts.get(slug) {
            Some(p) => p,
            None => return vec![],
        };
        let mut related = Vec::new();
        // First: explicitly listed related slugs
        for rs in &post.frontmatter.related_slugs {
            if let Some(p) = self.posts.get(rs) {
                related.push(p);
                if related.len() >= limit {
                    return related;
                }
            }
        }
        // Then: same cluster, sorted by date
        if let Some(cluster_slugs) = self.clusters.get(&post.frontmatter.cluster) {
            for cs in cluster_slugs {
                if cs != slug && !post.frontmatter.related_slugs.contains(cs) {
                    if let Some(p) = self.posts.get(cs) {
                        related.push(p);
                        if related.len() >= limit {
                            return related;
                        }
                    }
                }
            }
        }
        related
    }

    /// Static version for use during loading (before BlogStore is constructed)
    fn get_related_static(posts: &HashMap<String, BlogPost>, post: &BlogPost) -> Vec<BlogPost> {
        let mut related = Vec::new();
        for rs in &post.frontmatter.related_slugs {
            if let Some(p) = posts.get(rs) {
                related.push(p.clone());
                if related.len() >= 5 {
                    return related;
                }
            }
        }
        related
    }

    fn generate_sitemap(posts: &HashMap<String, BlogPost>) -> String {
        let mut urls = String::new();

        // Static pages
        let static_pages = [
            ("/", "1.0", "weekly"),
            ("/pricing", "0.9", "weekly"),
            ("/faq", "0.7", "monthly"),
            ("/terms", "0.4", "monthly"),
            ("/privacy", "0.4", "monthly"),
            ("/trustless", "0.8", "monthly"),
            ("/trust-chain", "0.7", "monthly"),
            ("/blog", "0.8", "daily"),
            // Existing hand-coded blog posts
            ("/signal-on-dumbphone", "0.8", "monthly"),
            ("/telegram-on-dumbphone", "0.8", "monthly"),
            ("/prompt-injection-safe", "0.8", "monthly"),
            ("/light-phone-3-whatsapp-guide", "0.8", "monthly"),
            ("/how-to-switch-to-dumbphone", "0.8", "monthly"),
            ("/how-to-read-more-accidentally", "0.7", "monthly"),
        ];

        for (path, priority, freq) in &static_pages {
            urls.push_str(&format!(
                "  <url>\n    <loc>https://lightfriend.ai{}</loc>\n    <changefreq>{}</changefreq>\n    <priority>{}</priority>\n  </url>\n",
                path, freq, priority
            ));
        }

        // Blog posts sorted by date
        let mut sorted: Vec<&BlogPost> = posts.values().collect();
        sorted.sort_by(|a, b| b.frontmatter.date.cmp(&a.frontmatter.date));

        for post in sorted {
            let priority = if post.frontmatter.cluster_hub {
                "0.8"
            } else {
                "0.6"
            };
            urls.push_str(&format!(
                "  <url>\n    <loc>https://lightfriend.ai/blog/{}</loc>\n    <lastmod>{}</lastmod>\n    <changefreq>monthly</changefreq>\n    <priority>{}</priority>\n  </url>\n",
                post.frontmatter.slug, post.frontmatter.date, priority
            ));
        }

        format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">\n{}</urlset>",
            urls
        )
    }
}
