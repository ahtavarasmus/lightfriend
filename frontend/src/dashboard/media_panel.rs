use yew::prelude::*;
use serde::{Deserialize, Serialize};
use web_sys::HtmlInputElement;
use wasm_bindgen_futures::spawn_local;
use serde_json::json;
use crate::utils::api::Api;

/// Media platforms supported for link detection
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum MediaPlatform {
    YouTube,
    TikTok,
    Instagram,
    Twitter,
    Vimeo,
    Rumble,
    Dailymotion,
    Reddit,
    Streamable,
    Spotify,
    Unknown,
}

/// A detected or searched media item
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MediaItem {
    pub platform: String,
    pub video_id: String,
    pub title: String,
    pub thumbnail: String,
    pub duration: Option<String>,
    pub channel: Option<String>,
    /// Original URL (if available) for backend resolution
    #[serde(default)]
    pub original_url: Option<String>,
}

impl MediaItem {
    /// Get the embed URL for this media item
    pub fn embed_url(&self) -> String {
        match self.platform.as_str() {
            "youtube" => format!("https://www.youtube.com/embed/{}?autoplay=0", self.video_id),
            "tiktok" => format!("https://www.tiktok.com/embed/v2/{}", self.video_id),
            "instagram" => format!("https://www.instagram.com/p/{}/embed/", self.video_id),
            "twitter" => format!("https://platform.twitter.com/embed/Tweet.html?id={}", self.video_id),
            "vimeo" => format!("https://player.vimeo.com/video/{}", self.video_id),
            "rumble" => format!("https://rumble.com/embed/{}/", self.video_id),
            "dailymotion" => format!("https://www.dailymotion.com/embed/video/{}", self.video_id),
            "reddit" => format!("https://www.redditmedia.com/mediaembed/{}?responsive=true", self.video_id),
            "streamable" => format!("https://streamable.com/e/{}", self.video_id),
            "spotify" => format!("https://open.spotify.com/embed/track/{}?theme=0", self.video_id),
            _ => String::new(),
        }
    }

    /// Get thumbnail URL for this media item (platform-specific)
    pub fn default_thumbnail(&self) -> String {
        match self.platform.as_str() {
            "youtube" => format!("https://img.youtube.com/vi/{}/mqdefault.jpg", self.video_id),
            "vimeo" => String::new(), // Vimeo requires API call for thumbnails
            _ => String::new(),
        }
    }
}

/// Detect platform from a URL
pub fn detect_platform(input: &str) -> MediaPlatform {
    let input = input.trim().to_lowercase();
    if input.contains("youtube.com") || input.contains("youtu.be") {
        MediaPlatform::YouTube
    } else if input.contains("tiktok.com") || input.contains("vm.tiktok.com") {
        MediaPlatform::TikTok
    } else if input.contains("instagram.com/reel") || input.contains("instagram.com/p/") {
        MediaPlatform::Instagram
    } else if input.contains("twitter.com") || input.contains("x.com") || input.contains("t.co/") {
        MediaPlatform::Twitter
    } else if input.contains("vimeo.com") {
        MediaPlatform::Vimeo
    } else if input.contains("rumble.com") {
        MediaPlatform::Rumble
    } else if input.contains("dailymotion.com") || input.contains("dai.ly") {
        MediaPlatform::Dailymotion
    } else if input.contains("reddit.com") || input.contains("redd.it") {
        MediaPlatform::Reddit
    } else if input.contains("streamable.com") {
        MediaPlatform::Streamable
    } else if input.contains("spotify.com") || input.contains("spotify:") {
        MediaPlatform::Spotify
    } else {
        MediaPlatform::Unknown
    }
}

/// Extract video ID from a YouTube URL
pub fn extract_youtube_video_id(url: &str) -> Option<String> {
    // Handle youtu.be/VIDEO_ID
    if url.contains("youtu.be/") {
        return url
            .split("youtu.be/")
            .nth(1)
            .map(|s| s.split(&['?', '&', '#'][..]).next().unwrap_or(s).to_string());
    }
    // Handle youtube.com/watch?v=VIDEO_ID
    if url.contains("v=") {
        return url
            .split("v=")
            .nth(1)
            .map(|s| s.split(&['?', '&', '#'][..]).next().unwrap_or(s).to_string());
    }
    // Handle youtube.com/embed/VIDEO_ID
    if url.contains("/embed/") {
        return url
            .split("/embed/")
            .nth(1)
            .map(|s| s.split(&['?', '&', '#'][..]).next().unwrap_or(s).to_string());
    }
    // Handle youtube.com/shorts/VIDEO_ID
    if url.contains("/shorts/") {
        return url
            .split("/shorts/")
            .nth(1)
            .map(|s| s.split(&['?', '&', '#'][..]).next().unwrap_or(s).to_string());
    }
    None
}

/// Extract video ID from a TikTok URL
/// Formats: tiktok.com/@user/video/VIDEO_ID, vm.tiktok.com/VIDEO_ID
pub fn extract_tiktok_video_id(url: &str) -> Option<String> {
    // Handle tiktok.com/@user/video/VIDEO_ID
    if url.contains("/video/") {
        return url
            .split("/video/")
            .nth(1)
            .map(|s| s.split(&['?', '&', '#', '/'][..]).next().unwrap_or(s).to_string());
    }
    // Handle vm.tiktok.com/CODE (short URL)
    if url.contains("vm.tiktok.com/") {
        return url
            .split("vm.tiktok.com/")
            .nth(1)
            .map(|s| s.split(&['?', '&', '#', '/'][..]).next().unwrap_or(s).to_string());
    }
    None
}

/// Extract video ID from a Vimeo URL
/// Formats: vimeo.com/VIDEO_ID, player.vimeo.com/video/VIDEO_ID
pub fn extract_vimeo_video_id(url: &str) -> Option<String> {
    // Handle player.vimeo.com/video/VIDEO_ID
    if url.contains("/video/") {
        return url
            .split("/video/")
            .nth(1)
            .map(|s| s.split(&['?', '&', '#', '/'][..]).next().unwrap_or(s).to_string());
    }
    // Handle vimeo.com/VIDEO_ID
    if url.contains("vimeo.com/") {
        return url
            .split("vimeo.com/")
            .nth(1)
            .map(|s| s.split(&['?', '&', '#', '/'][..]).next().unwrap_or(s).to_string())
            .filter(|s| s.chars().all(|c| c.is_ascii_digit()));
    }
    None
}

/// Extract video ID from a Rumble URL
/// Formats: rumble.com/VIDEO_ID-title.html, rumble.com/embed/VIDEO_ID
pub fn extract_rumble_video_id(url: &str) -> Option<String> {
    // Handle rumble.com/embed/VIDEO_ID
    if url.contains("/embed/") {
        return url
            .split("/embed/")
            .nth(1)
            .map(|s| s.split(&['?', '&', '#', '/'][..]).next().unwrap_or(s).to_string());
    }
    // Handle rumble.com/VIDEO_ID-title.html
    if url.contains("rumble.com/") {
        return url
            .split("rumble.com/")
            .nth(1)
            .and_then(|s| {
                let part = s.split('-').next()?;
                if part.is_empty() || part == "embed" { None } else { Some(part.to_string()) }
            });
    }
    None
}

/// Extract video ID from a Dailymotion URL
/// Formats: dailymotion.com/video/VIDEO_ID, dai.ly/VIDEO_ID
pub fn extract_dailymotion_video_id(url: &str) -> Option<String> {
    // Handle dailymotion.com/video/VIDEO_ID
    if url.contains("/video/") {
        return url
            .split("/video/")
            .nth(1)
            .map(|s| s.split(&['?', '&', '#', '/'][..]).next().unwrap_or(s).to_string());
    }
    // Handle dai.ly/VIDEO_ID
    if url.contains("dai.ly/") {
        return url
            .split("dai.ly/")
            .nth(1)
            .map(|s| s.split(&['?', '&', '#', '/'][..]).next().unwrap_or(s).to_string());
    }
    None
}

/// Extract video/reel ID from an Instagram URL
/// Formats: instagram.com/reel/CODE/, instagram.com/p/CODE/
pub fn extract_instagram_video_id(url: &str) -> Option<String> {
    // Handle instagram.com/reel/CODE/ or /reels/CODE/
    if url.contains("/reel/") || url.contains("/reels/") {
        let split_key = if url.contains("/reel/") { "/reel/" } else { "/reels/" };
        return url
            .split(split_key)
            .nth(1)
            .map(|s| s.split(&['?', '&', '#', '/'][..]).next().unwrap_or(s).to_string());
    }
    // Handle instagram.com/p/CODE/
    if url.contains("/p/") {
        return url
            .split("/p/")
            .nth(1)
            .map(|s| s.split(&['?', '&', '#', '/'][..]).next().unwrap_or(s).to_string());
    }
    None
}

/// Extract tweet ID from a Twitter/X URL
/// Formats: twitter.com/user/status/TWEET_ID, x.com/user/status/TWEET_ID
pub fn extract_twitter_video_id(url: &str) -> Option<String> {
    // Handle twitter.com/user/status/TWEET_ID or x.com/user/status/TWEET_ID
    if url.contains("/status/") {
        return url
            .split("/status/")
            .nth(1)
            .map(|s| s.split(&['?', '&', '#', '/'][..]).next().unwrap_or(s).to_string());
    }
    None
}

/// Extract post ID from a Reddit URL
/// Formats: reddit.com/r/subreddit/comments/POST_ID/...
pub fn extract_reddit_video_id(url: &str) -> Option<String> {
    // Handle reddit.com/r/subreddit/comments/POST_ID/
    if url.contains("/comments/") {
        return url
            .split("/comments/")
            .nth(1)
            .map(|s| s.split(&['?', '&', '#', '/'][..]).next().unwrap_or(s).to_string());
    }
    // Handle redd.it/POST_ID
    if url.contains("redd.it/") {
        return url
            .split("redd.it/")
            .nth(1)
            .map(|s| s.split(&['?', '&', '#', '/'][..]).next().unwrap_or(s).to_string());
    }
    None
}

/// Extract video ID from a Streamable URL
/// Formats: streamable.com/VIDEO_ID, streamable.com/e/VIDEO_ID
pub fn extract_streamable_video_id(url: &str) -> Option<String> {
    // Handle streamable.com/e/VIDEO_ID (embed)
    if url.contains("/e/") {
        return url
            .split("/e/")
            .nth(1)
            .map(|s| s.split(&['?', '&', '#', '/'][..]).next().unwrap_or(s).to_string());
    }
    // Handle streamable.com/VIDEO_ID
    if url.contains("streamable.com/") {
        return url
            .split("streamable.com/")
            .nth(1)
            .map(|s| s.split(&['?', '&', '#', '/'][..]).next().unwrap_or(s).to_string())
            .filter(|s| !s.is_empty() && s != "e");
    }
    None
}

/// Extract track/content ID from a Spotify URL
/// Formats: open.spotify.com/track/ID, spotify:track:ID
pub fn extract_spotify_id(url: &str) -> Option<String> {
    // Handle spotify:track:ID URI format
    if url.starts_with("spotify:") {
        let parts: Vec<&str> = url.split(':').collect();
        if parts.len() >= 3 {
            return Some(parts[2].to_string());
        }
    }
    // Handle open.spotify.com/track/ID, /episode/ID, etc.
    for content_type in &["track", "episode", "playlist", "album"] {
        let pattern = format!("/{}/", content_type);
        if url.contains(&pattern) {
            return url
                .split(&pattern)
                .nth(1)
                .map(|s| s.split(&['?', '&', '#', '/'][..]).next().unwrap_or(s).to_string());
        }
    }
    None
}

/// Generic function to extract video ID from any supported platform
pub fn extract_video_id(url: &str) -> Option<(MediaPlatform, String)> {
    let platform = detect_platform(url);
    let video_id = match platform {
        MediaPlatform::YouTube => extract_youtube_video_id(url),
        MediaPlatform::TikTok => extract_tiktok_video_id(url),
        MediaPlatform::Instagram => extract_instagram_video_id(url),
        MediaPlatform::Twitter => extract_twitter_video_id(url),
        MediaPlatform::Vimeo => extract_vimeo_video_id(url),
        MediaPlatform::Rumble => extract_rumble_video_id(url),
        MediaPlatform::Dailymotion => extract_dailymotion_video_id(url),
        MediaPlatform::Reddit => extract_reddit_video_id(url),
        MediaPlatform::Streamable => extract_streamable_video_id(url),
        MediaPlatform::Spotify => extract_spotify_id(url),
        _ => None,
    };
    video_id.map(|id| (platform, id))
}

// Comment types for YouTube
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Comment {
    pub id: String,
    pub author: String,
    pub author_profile_image: String,
    pub author_channel_id: String,
    pub text: String,
    pub like_count: u64,
    pub published_at: String,
    pub reply_count: u32,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct CommentsResponse {
    pub comments: Vec<Comment>,
    pub next_page_token: Option<String>,
    pub total_results: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CommentRequest {
    pub text: String,
}

// Backend resolve response types for non-YouTube platforms
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct TikTokEmbedResponse {
    pub video_id: String,
    pub embed_url: String,
    pub title: String,
    pub author: String,
    pub thumbnail_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct RumbleEmbedResponse {
    pub video_id: String,
    pub embed_url: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct InstagramEmbedResponse {
    pub post_id: String,
    pub embed_url: String,
    pub author: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct TwitterEmbedResponse {
    pub tweet_id: String,
    pub embed_url: String,
    pub author: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct StreamableEmbedResponse {
    pub video_id: String,
    pub embed_url: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct DailymotionEmbedResponse {
    pub video_id: String,
    pub embed_url: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct VimeoEmbedResponse {
    pub video_id: String,
    pub embed_url: String,
}

// Resolved embed data that can hold any platform's response
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedEmbed {
    pub embed_url: String,
    pub is_vertical: bool,
}

fn format_comment_time(published_at: &str) -> String {
    if published_at.is_empty() {
        return String::new();
    }
    if let Some(date_part) = published_at.split('T').next() {
        let parts: Vec<&str> = date_part.split('-').collect();
        if parts.len() == 3 {
            return format!("{}/{}/{}", parts[1], parts[2], parts[0]);
        }
    }
    published_at.to_string()
}

fn render_comment(comment: &Comment) -> Html {
    let time_ago = format_comment_time(&comment.published_at);
    html! {
        <div class="mp-comment-item">
            <img src={comment.author_profile_image.clone()} alt="" class="mp-comment-avatar" />
            <div class="mp-comment-content">
                <div class="mp-comment-header">
                    <span class="mp-comment-author">{&comment.author}</span>
                    <span class="mp-comment-time">{time_ago}</span>
                </div>
                <p class="mp-comment-text">{&comment.text}</p>
                <div class="mp-comment-actions">
                    <span class="mp-comment-likes">
                        {"👍 "}{comment.like_count}
                    </span>
                    if comment.reply_count > 0 {
                        <span class="mp-comment-replies">
                            {format!("{} replies", comment.reply_count)}
                        </span>
                    }
                </div>
            </div>
        </div>
    }
}

const MEDIA_PANEL_STYLES: &str = r#"
.media-panel {
    background: rgba(20, 20, 25, 0.95);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 12px;
    margin-top: 0.75rem;
    overflow: hidden;
}
.media-panel-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 0.5rem 0.75rem;
    border-bottom: 1px solid rgba(255, 255, 255, 0.08);
    background: rgba(255, 255, 255, 0.03);
}
.media-panel-title {
    font-size: 0.8rem;
    color: #888;
    font-weight: 500;
}
.media-panel-close {
    background: none;
    border: none;
    color: #666;
    cursor: pointer;
    padding: 0.25rem;
    font-size: 1rem;
    line-height: 1;
    transition: color 0.2s;
}
.media-panel-close:hover {
    color: #fff;
}
.media-panel-content {
    padding: 0.75rem;
}
/* Single video embed view */
.media-embed-container {
    position: relative;
    width: 100%;
    padding-top: 56.25%; /* 16:9 aspect ratio for horizontal videos */
    background: #000;
    border-radius: 8px;
    overflow: hidden;
}
.media-embed-container.vertical {
    padding-top: 177.78%; /* 9:16 aspect ratio for vertical videos (TikTok, Instagram) */
    max-width: 320px;
    margin: 0 auto;
}
.media-embed-container.square {
    padding-top: 100%; /* 1:1 aspect ratio */
    max-width: 400px;
    margin: 0 auto;
}
.media-embed-container iframe {
    position: absolute;
    top: 0;
    left: 0;
    width: 100%;
    height: 100%;
    border: none;
}
.media-info {
    margin-top: 0.5rem;
}
.media-title {
    font-size: 0.9rem;
    color: #fff;
    margin: 0 0 0.25rem 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
}
.media-channel {
    font-size: 0.8rem;
    color: #888;
}
/* Grid view for search results */
.media-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(140px, 1fr));
    gap: 0.75rem;
}
.media-grid-item {
    cursor: pointer;
    border-radius: 8px;
    overflow: hidden;
    background: rgba(255, 255, 255, 0.05);
    transition: transform 0.2s, background 0.2s;
}
.media-grid-item:hover {
    transform: translateY(-2px);
    background: rgba(255, 255, 255, 0.08);
}
.media-grid-thumbnail {
    position: relative;
    width: 100%;
    padding-top: 56.25%;
    background: #1a1a1a;
}
.media-grid-thumbnail img {
    position: absolute;
    top: 0;
    left: 0;
    width: 100%;
    height: 100%;
    object-fit: cover;
}
.media-grid-duration {
    position: absolute;
    bottom: 4px;
    right: 4px;
    background: rgba(0, 0, 0, 0.8);
    color: #fff;
    font-size: 0.7rem;
    padding: 0.1rem 0.3rem;
    border-radius: 3px;
}
.media-grid-info {
    padding: 0.5rem;
}
.media-grid-title {
    font-size: 0.75rem;
    color: #ddd;
    margin: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    -webkit-box-orient: vertical;
    line-height: 1.3;
}
.media-grid-channel {
    font-size: 0.65rem;
    color: #888;
    margin-top: 0.25rem;
}
/* Link preview (detected URL) */
.media-link-preview {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    padding: 0.25rem;
}
.media-link-thumbnail {
    width: 120px;
    height: 68px;
    border-radius: 6px;
    overflow: hidden;
    flex-shrink: 0;
    background: #1a1a1a;
}
.media-link-thumbnail img {
    width: 100%;
    height: 100%;
    object-fit: cover;
}
.media-link-info {
    flex: 1;
    min-width: 0;
}
.media-link-title {
    font-size: 0.85rem;
    color: #fff;
    margin: 0 0 0.25rem 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
}
.media-link-meta {
    font-size: 0.75rem;
    color: #888;
}
.media-link-play {
    background: linear-gradient(135deg, #1E90FF, #4169E1);
    border: none;
    color: white;
    padding: 0.5rem 1rem;
    border-radius: 6px;
    cursor: pointer;
    font-size: 0.8rem;
    font-weight: 500;
    transition: box-shadow 0.2s;
}
.media-link-play:hover {
    box-shadow: 0 4px 12px rgba(30, 144, 255, 0.3);
}
/* Back button */
.media-panel-back {
    background: none;
    border: none;
    color: #888;
    cursor: pointer;
    padding: 0.25rem 0.5rem;
    font-size: 1rem;
    line-height: 1;
    transition: color 0.2s;
    margin-right: 0.5rem;
}
.media-panel-back:hover {
    color: #fff;
}
.media-panel-header-left {
    display: flex;
    align-items: center;
}
/* Like button */
.media-like-section {
    display: flex;
    align-items: center;
    gap: 1rem;
    margin-top: 0.75rem;
    padding-top: 0.75rem;
    border-top: 1px solid rgba(255, 255, 255, 0.08);
}
.media-like-btn {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    background: rgba(255, 255, 255, 0.08);
    border: 1px solid rgba(255, 255, 255, 0.1);
    color: #ccc;
    padding: 0.5rem 1rem;
    border-radius: 20px;
    cursor: pointer;
    font-size: 0.85rem;
    transition: all 0.2s;
}
.media-like-btn:hover {
    background: rgba(255, 255, 255, 0.12);
    color: #fff;
}
.media-like-btn.liked {
    background: rgba(30, 144, 255, 0.2);
    border-color: rgba(30, 144, 255, 0.4);
    color: #1E90FF;
}
.media-like-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
}
/* Comments Section */
.mp-comments-section {
    margin-top: 1rem;
    padding-top: 1rem;
    border-top: 1px solid rgba(255, 255, 255, 0.08);
}
.mp-comments-header {
    font-size: 0.9rem;
    color: #ccc;
    margin: 0 0 0.75rem 0;
    font-weight: 500;
}
.mp-comment-form {
    display: flex;
    gap: 0.5rem;
    margin-bottom: 1rem;
}
.mp-comment-input {
    flex: 1;
    background: rgba(255, 255, 255, 0.05);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 20px;
    padding: 0.5rem 1rem;
    color: #fff;
    font-size: 0.85rem;
    outline: none;
    transition: border-color 0.2s;
}
.mp-comment-input:focus {
    border-color: rgba(30, 144, 255, 0.5);
}
.mp-comment-input::placeholder {
    color: #666;
}
.mp-comment-submit-btn {
    background: linear-gradient(135deg, #1E90FF, #4169E1);
    border: none;
    color: white;
    padding: 0.5rem 1rem;
    border-radius: 20px;
    cursor: pointer;
    font-size: 0.8rem;
    font-weight: 500;
    transition: opacity 0.2s;
}
.mp-comment-submit-btn:hover:not(:disabled) {
    opacity: 0.9;
}
.mp-comment-submit-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
}
.mp-comment-error {
    color: #ff6b6b;
    font-size: 0.8rem;
    margin-bottom: 0.75rem;
    padding: 0.5rem;
    background: rgba(255, 107, 107, 0.1);
    border-radius: 6px;
}
.mp-comments-loading, .mp-no-comments {
    color: #888;
    font-size: 0.85rem;
    text-align: center;
    padding: 1rem;
}
.mp-comments-list {
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
    max-height: 300px;
    overflow-y: auto;
}
.mp-comment-item {
    display: flex;
    gap: 0.75rem;
    padding: 0.5rem;
    background: rgba(255, 255, 255, 0.03);
    border-radius: 8px;
}
.mp-comment-avatar {
    width: 32px;
    height: 32px;
    border-radius: 50%;
    flex-shrink: 0;
}
.mp-comment-content {
    flex: 1;
    min-width: 0;
}
.mp-comment-header {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    margin-bottom: 0.25rem;
}
.mp-comment-author {
    font-size: 0.8rem;
    font-weight: 500;
    color: #ddd;
}
.mp-comment-time {
    font-size: 0.7rem;
    color: #666;
}
.mp-comment-text {
    font-size: 0.85rem;
    color: #bbb;
    margin: 0 0 0.25rem 0;
    line-height: 1.4;
    word-break: break-word;
}
.mp-comment-actions {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    font-size: 0.75rem;
}
.mp-comment-likes {
    color: #888;
}
.mp-comment-replies {
    color: #666;
}
.mp-connect-prompt {
    text-align: center;
    padding: 1rem;
    color: #888;
    font-size: 0.85rem;
}
"#;

#[derive(Properties, PartialEq, Clone)]
pub struct MediaPanelProps {
    /// List of media items to display
    pub media_items: Vec<MediaItem>,
    /// Whether to show a single video playing or grid of results
    #[prop_or(false)]
    pub playing: bool,
    /// Index of currently playing video (when playing=true)
    #[prop_or(0)]
    pub playing_index: usize,
    /// Callback when panel is closed
    pub on_close: Callback<()>,
    /// Callback when a video is selected to play
    pub on_select: Callback<usize>,
    /// Callback when back button is pressed (to return to grid from playing video)
    #[prop_or_default]
    pub on_back: Option<Callback<()>>,
    /// Whether user has YouTube connected (for comments/likes)
    #[prop_or(false)]
    pub youtube_connected: bool,
}

#[function_component(MediaPanel)]
pub fn media_panel(props: &MediaPanelProps) -> Html {
    if props.media_items.is_empty() {
        return html! {};
    }

    // Comment state
    let comments = use_state(|| Vec::<Comment>::new());
    let loading_comments = use_state(|| false);
    let comment_text = use_state(|| String::new());
    let posting_comment = use_state(|| false);
    let comment_error = use_state(|| None::<String>);
    let liked = use_state(|| false);
    let liking = use_state(|| false);

    // State for resolved embed URLs (for non-YouTube platforms that need backend resolution)
    let resolved_embed = use_state(|| None::<ResolvedEmbed>);
    let resolving_embed = use_state(|| false);
    let resolve_error = use_state(|| None::<String>);

    // Get current video ID for YouTube videos
    let current_video_id = if props.playing && props.playing_index < props.media_items.len() {
        let item = &props.media_items[props.playing_index];
        if item.platform == "youtube" {
            Some(item.video_id.clone())
        } else {
            None
        }
    } else {
        None
    };

    // Fetch comments when video changes (only for YouTube and if connected)
    {
        let video_id = current_video_id.clone();
        let comments = comments.clone();
        let loading_comments = loading_comments.clone();
        let liked = liked.clone();
        let youtube_connected = props.youtube_connected;
        use_effect_with_deps(move |video_id| {
            if let Some(vid) = video_id.clone() {
                web_sys::console::log_1(&format!("MediaPanel: video_id={}, youtube_connected={}", vid, youtube_connected).into());
                if youtube_connected {
                    let comments = comments.clone();
                    let loading_comments = loading_comments.clone();
                    let liked = liked.clone();
                    loading_comments.set(true);
                    liked.set(false);
                    spawn_local(async move {
                        web_sys::console::log_1(&format!("MediaPanel: Fetching comments for {}", vid).into());
                        match Api::get(&format!("/api/youtube/video/{}/comments", vid)).send().await {
                            Ok(response) => {
                                let status = response.status();
                                web_sys::console::log_1(&format!("MediaPanel: Comments response status={}", status).into());
                                if response.ok() {
                                    match response.json::<CommentsResponse>().await {
                                        Ok(data) => {
                                            web_sys::console::log_1(&format!("MediaPanel: Got {} comments", data.comments.len()).into());
                                            comments.set(data.comments);
                                        }
                                        Err(e) => {
                                            web_sys::console::error_1(&format!("MediaPanel: Failed to parse comments: {:?}", e).into());
                                        }
                                    }
                                } else {
                                    web_sys::console::error_1(&format!("MediaPanel: Comments request failed with status {}", status).into());
                                }
                            }
                            Err(e) => {
                                web_sys::console::error_1(&format!("MediaPanel: Comments request error: {:?}", e).into());
                            }
                        }
                        loading_comments.set(false);
                    });
                } else {
                    web_sys::console::log_1(&"MediaPanel: youtube_connected is false, not fetching comments".into());
                }
            } else {
                comments.set(Vec::new());
            }
            || ()
        }, video_id);
    }

    // Resolve non-YouTube video URLs when playing starts
    {
        let playing = props.playing;
        let playing_index = props.playing_index;
        let media_items = props.media_items.clone();
        let resolved_embed = resolved_embed.clone();
        let resolving_embed = resolving_embed.clone();
        let resolve_error = resolve_error.clone();

        use_effect_with_deps(move |(playing, playing_index, items): &(bool, usize, Vec<MediaItem>)| {
            if *playing && *playing_index < items.len() {
                let item = &items[*playing_index];
                let platform = item.platform.as_str();

                // YouTube doesn't need resolution - it uses direct embed URLs
                if platform == "youtube" {
                    resolved_embed.set(None);
                    resolving_embed.set(false);
                    resolve_error.set(None);
                } else {
                    // Use original URL if available, otherwise construct from video_id
                    // Note: vimeo, dailymotion, reddit, spotify use direct embed URLs (no backend resolve needed)
                    let url_opt = if let Some(orig_url) = &item.original_url {
                        // Use the original URL for platforms that need resolution
                        match platform {
                            "tiktok" | "rumble" | "instagram" | "twitter" | "streamable" => Some(orig_url.clone()),
                            _ => None,
                        }
                    } else {
                        // Fallback: construct URL from video_id (may not work for all platforms)
                        match platform {
                            "tiktok" => Some(format!("https://www.tiktok.com/@user/video/{}", item.video_id)),
                            "rumble" => Some(format!("https://rumble.com/{}", item.video_id)),
                            "instagram" => Some(format!("https://www.instagram.com/reel/{}/", item.video_id)),
                            "twitter" => Some(format!("https://twitter.com/i/status/{}", item.video_id)),
                            "streamable" => Some(format!("https://streamable.com/{}", item.video_id)),
                            _ => None,
                        }
                    };

                    let api_endpoint = match platform {
                        "tiktok" => Some("/api/tiktok/resolve"),
                        "rumble" => Some("/api/rumble/resolve"),
                        "instagram" => Some("/api/instagram/resolve"),
                        "twitter" => Some("/api/twitter/resolve"),
                        "streamable" => Some("/api/streamable/resolve"),
                        _ => None,
                    };

                    if let (Some(url), Some(endpoint)) = (url_opt, api_endpoint) {
                        let is_vertical = platform == "tiktok" || platform == "instagram";
                        let platform_str = platform.to_string();
                        let resolved_embed = resolved_embed.clone();
                        let resolving_embed = resolving_embed.clone();
                        let resolve_error = resolve_error.clone();
                        let fallback_url = item.embed_url();

                        resolving_embed.set(true);
                        resolve_error.set(None);
                        resolved_embed.set(None);

                        spawn_local(async move {
                            web_sys::console::log_1(&format!("MediaPanel: Resolving {} URL: {}", platform_str, url).into());

                            match Api::post(endpoint)
                                .json(&json!({ "url": url }))
                                .unwrap()
                                .send()
                                .await
                            {
                                Ok(response) => {
                                    if response.ok() {
                                        // Parse based on platform - all have embed_url field
                                        match response.json::<serde_json::Value>().await {
                                            Ok(data) => {
                                                if let Some(embed_url) = data["embed_url"].as_str() {
                                                    web_sys::console::log_1(&format!("MediaPanel: Resolved embed URL: {}", embed_url).into());
                                                    resolved_embed.set(Some(ResolvedEmbed {
                                                        embed_url: embed_url.to_string(),
                                                        is_vertical,
                                                    }));
                                                } else {
                                                    web_sys::console::warn_1(&"MediaPanel: Response missing embed_url, using fallback".into());
                                                    resolved_embed.set(Some(ResolvedEmbed {
                                                        embed_url: fallback_url,
                                                        is_vertical,
                                                    }));
                                                }
                                            }
                                            Err(e) => {
                                                web_sys::console::error_1(&format!("MediaPanel: Failed to parse response: {:?}", e).into());
                                                resolved_embed.set(Some(ResolvedEmbed {
                                                    embed_url: fallback_url,
                                                    is_vertical,
                                                }));
                                            }
                                        }
                                    } else {
                                        web_sys::console::error_1(&format!("MediaPanel: Resolve request failed with status {}", response.status()).into());
                                        // Fall back to direct URL
                                        resolved_embed.set(Some(ResolvedEmbed {
                                            embed_url: fallback_url,
                                            is_vertical,
                                        }));
                                    }
                                }
                                Err(e) => {
                                    web_sys::console::error_1(&format!("MediaPanel: Resolve request error: {:?}", e).into());
                                    resolve_error.set(Some(format!("Failed to load video: {:?}", e)));
                                    // Fall back to direct URL
                                    resolved_embed.set(Some(ResolvedEmbed {
                                        embed_url: fallback_url,
                                        is_vertical,
                                    }));
                                }
                            }
                            resolving_embed.set(false);
                        });
                    } else {
                        // For unsupported platforms, use direct embed URL
                        resolved_embed.set(Some(ResolvedEmbed {
                            embed_url: item.embed_url(),
                            is_vertical: false,
                        }));
                    }
                }
            } else {
                // Not playing, clear resolved embed
                resolved_embed.set(None);
                resolving_embed.set(false);
                resolve_error.set(None);
            }
            || ()
        }, (playing, playing_index, media_items));
    }

    let on_close = {
        let callback = props.on_close.clone();
        Callback::from(move |_: MouseEvent| {
            callback.emit(());
        })
    };

    let on_back = {
        let callback = props.on_back.clone();
        Callback::from(move |_: MouseEvent| {
            if let Some(cb) = &callback {
                cb.emit(());
            }
        })
    };

    let on_comment_input = {
        let comment_text = comment_text.clone();
        Callback::from(move |e: InputEvent| {
            let input: HtmlInputElement = e.target_unchecked_into();
            comment_text.set(input.value());
        })
    };

    let on_comment_submit = {
        let video_id = current_video_id.clone();
        let comment_text = comment_text.clone();
        let comments = comments.clone();
        let posting_comment = posting_comment.clone();
        let comment_error = comment_error.clone();
        Callback::from(move |e: SubmitEvent| {
            e.prevent_default();
            let text = (*comment_text).clone();
            if text.trim().is_empty() {
                return;
            }
            let Some(vid) = video_id.clone() else { return };

            let comment_text = comment_text.clone();
            let comments = comments.clone();
            let posting_comment = posting_comment.clone();
            let comment_error = comment_error.clone();
            posting_comment.set(true);
            spawn_local(async move {
                let request = CommentRequest { text: text.clone() };
                match Api::post(&format!("/api/youtube/video/{}/comments", vid))
                    .json(&request)
                    .unwrap()
                    .send()
                    .await
                {
                    Ok(response) => {
                        if response.ok() {
                            if let Ok(new_comment) = response.json::<Comment>().await {
                                let mut updated = vec![new_comment];
                                updated.extend((*comments).clone());
                                comments.set(updated);
                                comment_text.set(String::new());
                                comment_error.set(None);
                            }
                        } else {
                            if let Ok(data) = response.json::<serde_json::Value>().await {
                                let err = data["error"].as_str().unwrap_or("Failed to post comment").to_string();
                                comment_error.set(Some(err));
                            }
                        }
                    }
                    Err(e) => {
                        comment_error.set(Some(format!("Network error: {}", e)));
                    }
                }
                posting_comment.set(false);
            });
        })
    };

    let on_like_click = {
        let video_id = current_video_id.clone();
        let liked = liked.clone();
        let liking = liking.clone();
        Callback::from(move |_: MouseEvent| {
            let Some(vid) = video_id.clone() else { return };
            let liked = liked.clone();
            let liking = liking.clone();
            let current_liked = *liked;
            liking.set(true);
            spawn_local(async move {
                let rating = if current_liked { "none" } else { "like" };
                let body = serde_json::json!({ "rating": rating });
                match Api::post(&format!("/api/youtube/video/{}/rate", vid))
                    .json(&body)
                    .unwrap()
                    .send()
                    .await
                {
                    Ok(response) => {
                        if response.ok() {
                            liked.set(!current_liked);
                        }
                    }
                    Err(_) => {}
                }
                liking.set(false);
            });
        })
    };

    // Determine panel title based on content
    let panel_title = if props.playing {
        "Now Playing"
    } else if props.media_items.len() == 1 {
        let item = &props.media_items[0];
        match item.platform.as_str() {
            "youtube" => "YouTube Video",
            "tiktok" => "TikTok Video",
            _ => "Video",
        }
    } else {
        "Search Results"
    };

    let show_back_button = props.playing && props.media_items.len() > 1 && props.on_back.is_some();

    let youtube_connected = props.youtube_connected;

    html! {
        <>
            <style>{MEDIA_PANEL_STYLES}</style>
            <div class="media-panel">
                <div class="media-panel-header">
                    <div class="media-panel-header-left">
                        if show_back_button {
                            <button class="media-panel-back" onclick={on_back} title="Back to results">
                                {"←"}
                            </button>
                        }
                        <span class="media-panel-title">{panel_title}</span>
                    </div>
                    <button class="media-panel-close" onclick={on_close} title="Close">
                        {"×"}
                    </button>
                </div>
                <div class="media-panel-content">
                    {
                        if props.playing && props.playing_index < props.media_items.len() {
                            // Show embedded video player
                            let item = &props.media_items[props.playing_index];
                            let is_youtube = item.platform == "youtube";

                            // For YouTube, use direct embed URL. For others, use resolved URL.
                            let (embed_url, is_vertical) = if is_youtube {
                                (item.embed_url(), false)
                            } else if let Some(resolved) = (*resolved_embed).as_ref() {
                                (resolved.embed_url.clone(), resolved.is_vertical)
                            } else if *resolving_embed {
                                // Still resolving - show loading
                                return html! {
                                    <div class="mp-comments-loading">{"Loading video..."}</div>
                                };
                            } else {
                                // Fallback to direct URL while resolve is pending
                                (item.embed_url(), item.platform == "tiktok" || item.platform == "instagram")
                            };

                            let container_class = if is_vertical {
                                "media-embed-container vertical"
                            } else {
                                "media-embed-container"
                            };

                            html! {
                                <>
                                    <div class={container_class}>
                                        <iframe
                                            src={embed_url}
                                            title={item.title.clone()}
                                            frameborder="0"
                                            allow="accelerometer; autoplay; clipboard-write; encrypted-media; gyroscope; picture-in-picture; fullscreen"
                                            allowfullscreen=true
                                        />
                                    </div>
                                    <div class="media-info">
                                        <h4 class="media-title">{&item.title}</h4>
                                        if let Some(channel) = &item.channel {
                                            <span class="media-channel">{channel}</span>
                                        }
                                    </div>

                                    // Like button and comments for YouTube videos
                                    if is_youtube && youtube_connected {
                                        <div class="media-like-section">
                                            <button
                                                class={classes!("media-like-btn", (*liked).then_some("liked"))}
                                                onclick={on_like_click.clone()}
                                                disabled={*liking}
                                            >
                                                if *liked { {"👍 Liked"} } else { {"👍 Like"} }
                                            </button>
                                        </div>

                                        <div class="mp-comments-section">
                                            <h3 class="mp-comments-header">{"Comments"}</h3>

                                            <form onsubmit={on_comment_submit.clone()} class="mp-comment-form">
                                                <input
                                                    type="text"
                                                    placeholder="Add a comment..."
                                                    value={(*comment_text).clone()}
                                                    oninput={on_comment_input.clone()}
                                                    class="mp-comment-input"
                                                    disabled={*posting_comment}
                                                />
                                                <button
                                                    type="submit"
                                                    class="mp-comment-submit-btn"
                                                    disabled={*posting_comment || comment_text.is_empty()}
                                                >
                                                    if *posting_comment { {"..."} } else { {"Post"} }
                                                </button>
                                            </form>

                                            if let Some(err) = (*comment_error).as_ref() {
                                                <div class="mp-comment-error">{err}</div>
                                            }

                                            if *loading_comments {
                                                <div class="mp-comments-loading">{"Loading comments..."}</div>
                                            } else if comments.is_empty() {
                                                <div class="mp-no-comments">{"No comments yet"}</div>
                                            } else {
                                                <div class="mp-comments-list">
                                                    { for (*comments).iter().map(|comment| render_comment(comment)) }
                                                </div>
                                            }
                                        </div>
                                    } else if is_youtube && !youtube_connected {
                                        <div class="mp-connect-prompt">
                                            {"Connect YouTube to like videos and view comments"}
                                        </div>
                                    }
                                </>
                            }
                        } else if props.media_items.len() == 1 {
                            // Single item - show link preview
                            let item = &props.media_items[0];
                            let on_play = {
                                let callback = props.on_select.clone();
                                Callback::from(move |_: MouseEvent| {
                                    callback.emit(0);
                                })
                            };
                            html! {
                                <div class="media-link-preview">
                                    <div class="media-link-thumbnail">
                                        <img src={item.thumbnail.clone()} alt={item.title.clone()} />
                                    </div>
                                    <div class="media-link-info">
                                        <h4 class="media-link-title">{&item.title}</h4>
                                        <span class="media-link-meta">
                                            {
                                                match &item.duration {
                                                    Some(d) => format!("{} - {}", item.platform, d),
                                                    None => item.platform.clone(),
                                                }
                                            }
                                        </span>
                                    </div>
                                    <button class="media-link-play" onclick={on_play}>
                                        {"Play"}
                                    </button>
                                </div>
                            }
                        } else {
                            // Multiple items - show grid
                            html! {
                                <div class="media-grid">
                                    {
                                        props.media_items.iter().enumerate().map(|(idx, item)| {
                                            let on_click = {
                                                let callback = props.on_select.clone();
                                                Callback::from(move |_: MouseEvent| {
                                                    callback.emit(idx);
                                                })
                                            };
                                            html! {
                                                <div class="media-grid-item" onclick={on_click}>
                                                    <div class="media-grid-thumbnail">
                                                        <img src={item.thumbnail.clone()} alt={item.title.clone()} />
                                                        if let Some(duration) = &item.duration {
                                                            <span class="media-grid-duration">{duration}</span>
                                                        }
                                                    </div>
                                                    <div class="media-grid-info">
                                                        <h5 class="media-grid-title">{&item.title}</h5>
                                                        if let Some(channel) = &item.channel {
                                                            <span class="media-grid-channel">{channel}</span>
                                                        }
                                                    </div>
                                                </div>
                                            }
                                        }).collect::<Html>()
                                    }
                                </div>
                            }
                        }
                    }
                </div>
            </div>
        </>
    }
}
