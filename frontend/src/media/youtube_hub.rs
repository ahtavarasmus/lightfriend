use yew::prelude::*;
use web_sys::HtmlInputElement;
use wasm_bindgen_futures::spawn_local;
use serde::{Deserialize, Serialize};
use crate::utils::api::Api;

// Platform detection for multi-platform video support
#[derive(Debug, Clone, PartialEq)]
pub enum MediaPlatform {
    YouTube,
    TikTok,
    Instagram,
    Twitter,
    Reddit,
    Spotify,
    Rumble,
    Streamable,
    Bluesky,
    Unknown, // Treat as YouTube search
}

fn detect_platform(input: &str) -> MediaPlatform {
    let input = input.trim().to_lowercase();
    if input.contains("youtube.com") || input.contains("youtu.be") {
        MediaPlatform::YouTube
    } else if input.contains("tiktok.com") || input.contains("vm.tiktok.com") {
        MediaPlatform::TikTok
    } else if input.contains("instagram.com/reel") || input.contains("instagram.com/reels") || input.contains("instagram.com/p/") {
        MediaPlatform::Instagram
    } else if input.contains("twitter.com") || input.contains("x.com") || input.contains("t.co/") {
        MediaPlatform::Twitter
    } else if input.contains("reddit.com") || input.contains("redd.it") {
        MediaPlatform::Reddit
    } else if input.contains("spotify.com") || input.starts_with("spotify:") {
        MediaPlatform::Spotify
    } else if input.contains("rumble.com") {
        MediaPlatform::Rumble
    } else if input.contains("streamable.com") {
        MediaPlatform::Streamable
    } else if input.contains("bsky.app") {
        MediaPlatform::Bluesky
    } else {
        MediaPlatform::Unknown
    }
}

// TikTok types
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TikTokResolveRequest {
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct TikTokEmbedResponse {
    pub video_id: String,
    pub embed_url: String,
    pub title: String,
    pub author: String,
    pub thumbnail_url: Option<String>,
}

// Instagram types
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InstagramResolveRequest {
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct InstagramEmbedResponse {
    pub reel_id: String,
    pub embed_url: String,
}

// Twitter/X types
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TwitterResolveRequest {
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct TwitterEmbedResponse {
    pub tweet_id: String,
    pub embed_url: String,
    pub author: Option<String>,
}

// Reddit types
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RedditResolveRequest {
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct RedditEmbedResponse {
    pub post_id: String,
    pub embed_url: String,
    pub subreddit: Option<String>,
}

// Spotify types
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpotifyResolveRequest {
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct SpotifyEmbedResponse {
    pub content_id: String,
    pub content_type: String,
    pub embed_url: String,
}

// Rumble types
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RumbleResolveRequest {
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct RumbleEmbedResponse {
    pub video_id: String,
    pub embed_url: String,
}

// Streamable types
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StreamableResolveRequest {
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct StreamableEmbedResponse {
    pub video_id: String,
    pub embed_url: String,
}

// Bluesky types
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BlueskyResolveRequest {
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct BlueskyEmbedResponse {
    pub post_id: String,
    pub handle: String,
    pub embed_url: String,
}

// Unified selected media state for all platforms
#[derive(Debug, Clone, PartialEq)]
pub enum SelectedMedia {
    YouTube(VideoDetailsResponse),
    TikTok(TikTokEmbedResponse),
    Instagram(InstagramEmbedResponse),
    Twitter(TwitterEmbedResponse),
    Reddit(RedditEmbedResponse),
    Spotify(SpotifyEmbedResponse),
    Rumble(RumbleEmbedResponse),
    Streamable(StreamableEmbedResponse),
    Bluesky(BlueskyEmbedResponse),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Video {
    pub id: String,
    pub title: String,
    pub channel: String,
    pub channel_id: String,
    pub thumbnail: String,
    pub duration: String,
    pub published_at: String,
    pub view_count: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct SubscriptionFeedResponse {
    pub videos: Vec<Video>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct SearchResponse {
    pub videos: Vec<Video>,
    pub channels: Option<Vec<Channel>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Channel {
    pub id: String,
    pub title: String,
    pub description: String,
    pub thumbnail: String,
    pub subscriber_count: String,
    pub is_subscribed: bool,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct VideoDetailsResponse {
    pub video: Video,
    pub embed_url: String,
}

// Comment types
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

#[derive(Debug, Clone, PartialEq)]
pub enum SearchType {
    Videos,
    Channels,
    All,
}

// YouTube Video Player Component with Comments
#[derive(Properties, PartialEq)]
pub struct YouTubeVideoPlayerProps {
    pub video_details: VideoDetailsResponse,
    pub can_comment: bool,
    pub on_close: Callback<MouseEvent>,
}

#[function_component(YouTubeVideoPlayer)]
pub fn youtube_video_player(props: &YouTubeVideoPlayerProps) -> Html {
    let comments = use_state(|| Vec::<Comment>::new());
    let loading_comments = use_state(|| false);
    let comment_text = use_state(|| String::new());
    let posting_comment = use_state(|| false);
    let comment_error = use_state(|| None::<String>);
    let show_comment_modal = use_state(|| false);
    let liked = use_state(|| false);
    let liking = use_state(|| false);

    // Fetch comments on mount - only if user has comment permissions
    // (YouTube API requires youtube.force-ssl scope even for reading comments)
    {
        let video_id = props.video_details.video.id.clone();
        let comments = comments.clone();
        let loading_comments = loading_comments.clone();
        let can_comment = props.can_comment;
        use_effect_with_deps(move |_| {
            if can_comment {
                let comments = comments.clone();
                let loading_comments = loading_comments.clone();
                loading_comments.set(true);
                spawn_local(async move {
                    match Api::get(&format!("/api/youtube/video/{}/comments", video_id)).send().await {
                        Ok(response) => {
                            if response.ok() {
                                if let Ok(data) = response.json::<CommentsResponse>().await {
                                    comments.set(data.comments);
                                }
                            }
                        }
                        Err(_) => {}
                    }
                    loading_comments.set(false);
                });
            }
            || ()
        }, (props.video_details.video.id.clone(), props.can_comment));
    }

    let on_comment_input = {
        let comment_text = comment_text.clone();
        Callback::from(move |e: InputEvent| {
            let input: HtmlInputElement = e.target_unchecked_into();
            comment_text.set(input.value());
        })
    };

    let on_comment_submit = {
        let video_id = props.video_details.video.id.clone();
        let comment_text = comment_text.clone();
        let comments = comments.clone();
        let posting_comment = posting_comment.clone();
        let comment_error = comment_error.clone();
        let show_comment_modal = show_comment_modal.clone();
        let can_comment = props.can_comment;
        Callback::from(move |e: SubmitEvent| {
            e.prevent_default();
            let text = (*comment_text).clone();
            if text.trim().is_empty() {
                return;
            }

            if !can_comment {
                show_comment_modal.set(true);
                return;
            }

            let video_id = video_id.clone();
            let comment_text = comment_text.clone();
            let comments = comments.clone();
            let posting_comment = posting_comment.clone();
            let comment_error = comment_error.clone();
            posting_comment.set(true);
            spawn_local(async move {
                let request = CommentRequest { text: text.clone() };
                match Api::post(&format!("/api/youtube/video/{}/comments", video_id))
                    .json(&request)
                    .unwrap()
                    .send()
                    .await
                {
                    Ok(response) => {
                        if response.ok() {
                            if let Ok(new_comment) = response.json::<Comment>().await {
                                // Add new comment to the top
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

    let close_comment_modal = {
        let show_comment_modal = show_comment_modal.clone();
        Callback::from(move |_: MouseEvent| {
            show_comment_modal.set(false);
        })
    };

    let on_like_click = {
        let video_id = props.video_details.video.id.clone();
        let liked = liked.clone();
        let liking = liking.clone();
        let can_comment = props.can_comment;
        let show_comment_modal = show_comment_modal.clone();
        Callback::from(move |_: MouseEvent| {
            if !can_comment {
                show_comment_modal.set(true);
                return;
            }
            let video_id = video_id.clone();
            let liked = liked.clone();
            let liking = liking.clone();
            let current_liked = *liked;
            liking.set(true);
            spawn_local(async move {
                let rating = if current_liked { "none" } else { "like" };
                let body = serde_json::json!({ "rating": rating });
                match Api::post(&format!("/api/youtube/video/{}/rate", video_id))
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

    let on_upgrade_click = {
        let video_id = props.video_details.video.id.clone();
        Callback::from(move |_: MouseEvent| {
            let video_id = video_id.clone();
            spawn_local(async move {
                match Api::get("/api/auth/youtube/upgrade").send().await {
                    Ok(response) => {
                        if response.ok() {
                            if let Ok(data) = response.json::<serde_json::Value>().await {
                                if let Some(auth_url) = data["auth_url"].as_str() {
                                    if let Some(window) = web_sys::window() {
                                        // Store video ID to return to after OAuth
                                        if let Ok(Some(storage)) = window.local_storage() {
                                            let _ = storage.set_item("youtube_return_video", &video_id);
                                        }
                                        let _ = window.location().set_href(auth_url);
                                    }
                                }
                            }
                        }
                    }
                    Err(_) => {}
                }
            });
        })
    };

    let video_details = &props.video_details;
    let on_close = props.on_close.clone();

    html! {
        <div class="video-player-container">
            <div class="video-player-header">
                <button onclick={on_close} class="back-button">
                    {"← Back"}
                </button>
                <span class="platform-badge youtube">{"YouTube"}</span>
            </div>
            <div class="video-embed-wrapper youtube-aspect">
                <iframe
                    src={video_details.embed_url.clone()}
                    title={video_details.video.title.clone()}
                    frameborder="0"
                    allow="accelerometer; autoplay; clipboard-write; encrypted-media; gyroscope; picture-in-picture; fullscreen"
                    allowfullscreen=true
                    class="video-embed"
                />
            </div>
            <div class="video-details">
                <h2 class="video-player-title">{&video_details.video.title}</h2>
                <div class="video-player-meta">
                    <span class="channel-name">{&video_details.video.channel}</span>
                    <span class="meta-separator">{"•"}</span>
                    <span>{&video_details.video.view_count}</span>
                    if !video_details.video.duration.is_empty() {
                        <>
                            <span class="meta-separator">{"•"}</span>
                            <span>{&video_details.video.duration}</span>
                        </>
                    }
                </div>
                <div class="video-actions">
                    <button
                        onclick={on_like_click}
                        class={classes!("like-button", if *liked { "liked" } else { "" })}
                        disabled={*liking}
                    >
                        if *liking {
                            {"..."}
                        } else if *liked {
                            {"👍 Liked"}
                        } else {
                            {"👍 Like"}
                        }
                    </button>
                </div>
            </div>
            <p class="intentional-note">{"No related videos • No autoplay • No recommendations"}</p>

            // Comments Section
            <div class="comments-section">
                <h3 class="comments-header">{"Comments"}</h3>

                if props.can_comment {
                    // User has permissions - show comment form and list
                    <>
                        // Comment input
                        <form onsubmit={on_comment_submit} class="comment-form">
                            <input
                                type="text"
                                placeholder="Add a comment..."
                                value={(*comment_text).clone()}
                                oninput={on_comment_input}
                                class="comment-input"
                                disabled={*posting_comment}
                            />
                            <button type="submit" class="comment-submit-btn" disabled={*posting_comment || comment_text.is_empty()}>
                                if *posting_comment {
                                    {"..."}
                                } else {
                                    {"Post"}
                                }
                            </button>
                        </form>

                        // Comment error
                        if let Some(err) = (*comment_error).as_ref() {
                            <div class="comment-error">{err}</div>
                        }

                        // Comments list
                        if *loading_comments {
                            <div class="comments-loading">{"Loading comments..."}</div>
                        } else if comments.is_empty() {
                            <div class="no-comments">{"No comments yet"}</div>
                        } else {
                            <div class="comments-list">
                                { for (*comments).iter().map(|comment| render_comment(comment)) }
                            </div>
                        }
                    </>
                } else {
                    // User doesn't have permissions - show upgrade prompt
                    <div class="comments-upgrade-prompt">
                        <p>{"YouTube requires additional permissions to view and post comments."}</p>
                        <button class="upgrade-comments-btn" onclick={
                            let show_comment_modal = show_comment_modal.clone();
                            Callback::from(move |_: MouseEvent| show_comment_modal.set(true))
                        }>
                            {"Enable Comments"}
                        </button>
                    </div>
                }
            </div>

            // Comment permission modal
            if *show_comment_modal {
                <div class="modal-overlay" onclick={close_comment_modal.clone()}>
                    <div class="upgrade-modal" onclick={Callback::from(|e: MouseEvent| e.stop_propagation())}>
                        <h3>{"Enable Comments"}</h3>
                        <p class="modal-description">
                            {"YouTube's API doesn't allow reading comments with basic permissions, so elevated access is required."}
                        </p>
                        <p class="modal-warning">
                            {"This will only be used to read comments and post your own comments if you choose to. You'll be redirected back to this video after granting permission."}
                        </p>
                        <div class="modal-buttons">
                            <button class="modal-button primary" onclick={on_upgrade_click}>
                                {"Grant Permission"}
                            </button>
                            <button class="modal-button cancel" onclick={close_comment_modal}>
                                {"Cancel"}
                            </button>
                        </div>
                    </div>
                </div>
            }
        </div>
    }
}

fn render_comment(comment: &Comment) -> Html {
    let time_ago = format_comment_time(&comment.published_at);
    html! {
        <div class="comment-item">
            <img src={comment.author_profile_image.clone()} alt="" class="comment-avatar" />
            <div class="comment-content">
                <div class="comment-header">
                    <span class="comment-author">{&comment.author}</span>
                    <span class="comment-time">{time_ago}</span>
                </div>
                <p class="comment-text">{&comment.text}</p>
                <div class="comment-actions">
                    <span class="comment-likes">
                        {"👍 "}{comment.like_count}
                    </span>
                    if comment.reply_count > 0 {
                        <span class="comment-replies">
                            {format!("{} replies", comment.reply_count)}
                        </span>
                    }
                </div>
            </div>
        </div>
    }
}

fn format_comment_time(published_at: &str) -> String {
    if published_at.is_empty() {
        return String::new();
    }
    // Simple date extraction
    if let Some(date_part) = published_at.split('T').next() {
        let parts: Vec<&str> = date_part.split('-').collect();
        if parts.len() == 3 {
            return format!("{}/{}/{}", parts[1], parts[2], parts[0]);
        }
    }
    published_at.to_string()
}

#[derive(Properties, PartialEq)]
pub struct YouTubeHubProps {
    #[prop_or(false)]
    pub youtube_connected: bool,
    #[prop_or(false)]
    pub can_subscribe: bool,
}

#[function_component(YouTubeHub)]
pub fn youtube_hub(props: &YouTubeHubProps) -> Html {
    let search_query = use_state(|| String::new());
    let videos = use_state(|| Vec::<Video>::new());
    let channels = use_state(|| Vec::<Channel>::new());
    let loading = use_state(|| false);
    let error = use_state(|| None::<String>);
    let showing_search = use_state(|| false);
    let selected_media = use_state(|| None::<SelectedMedia>);
    let search_type = use_state(|| SearchType::All);
    let subscribing_channel = use_state(|| None::<String>);
    let show_upgrade_modal = use_state(|| false);
    let pending_channel_id = use_state(|| None::<String>);
    let auto_downgrade_after_subscribe = use_state(|| false);
    let success_message = use_state(|| None::<String>);
    let viewing_channel = use_state(|| None::<(String, String)>); // (channel_id, channel_name)

    // Check localStorage on mount for pending subscribe after OAuth redirect
    {
        let error = error.clone();
        let success_message = success_message.clone();
        let can_subscribe = props.can_subscribe;
        use_effect_with_deps(move |_| {
            if can_subscribe {
                // Check if we returned from OAuth upgrade and need to subscribe
                if let Some(window) = web_sys::window() {
                    if let Ok(Some(storage)) = window.local_storage() {
                        // Check for pending subscribe
                        if let Ok(Some(channel_id)) = storage.get_item("youtube_pending_subscribe") {
                            // Clear the storage items
                            let _ = storage.remove_item("youtube_pending_subscribe");
                            let should_downgrade = storage.get_item("youtube_auto_downgrade")
                                .ok()
                                .flatten()
                                .map(|v| v == "true")
                                .unwrap_or(false);
                            let _ = storage.remove_item("youtube_auto_downgrade");

                            let error = error.clone();
                            let success_message = success_message.clone();
                            spawn_local(async move {
                                // Subscribe to the channel
                                let body = serde_json::json!({ "channel_id": channel_id });
                                match Api::post("/api/youtube/subscribe")
                                    .json(&body)
                                    .unwrap()
                                    .send()
                                    .await
                                {
                                    Ok(response) => {
                                        if response.ok() {
                                            // Now auto-downgrade if requested
                                            if should_downgrade {
                                                if let Ok(resp) = Api::get("/api/auth/youtube/downgrade").send().await {
                                                    if resp.ok() {
                                                        if let Ok(data) = resp.json::<serde_json::Value>().await {
                                                            if let Some(auth_url) = data["auth_url"].as_str() {
                                                                // Store success message to show after downgrade
                                                                if let Some(window) = web_sys::window() {
                                                                    if let Ok(Some(storage)) = window.local_storage() {
                                                                        let _ = storage.set_item("youtube_subscribe_success", "true");
                                                                    }
                                                                    let _ = window.location().set_href(auth_url);
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            } else {
                                                success_message.set(Some("Subscribed successfully!".to_string()));
                                            }
                                        } else {
                                            if let Ok(data) = response.json::<serde_json::Value>().await {
                                                let err = data["error"].as_str().unwrap_or("Failed to subscribe").to_string();
                                                error.set(Some(err));
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        error.set(Some(format!("Network error: {}", e)));
                                    }
                                }
                            });
                        }
                        // Check for success after downgrade redirect
                        else if let Ok(Some(_)) = storage.get_item("youtube_subscribe_success") {
                            let _ = storage.remove_item("youtube_subscribe_success");
                            success_message.set(Some("Subscribed and permissions revoked!".to_string()));
                        }
                    }
                }
            }
            || ()
        }, can_subscribe);
    }

    // Check localStorage for video to return to after comments OAuth upgrade
    {
        let selected_media = selected_media.clone();
        let loading = loading.clone();
        let can_subscribe = props.can_subscribe;
        use_effect_with_deps(move |_| {
            if can_subscribe {
                if let Some(window) = web_sys::window() {
                    if let Ok(Some(storage)) = window.local_storage() {
                        if let Ok(Some(video_id)) = storage.get_item("youtube_return_video") {
                            let _ = storage.remove_item("youtube_return_video");
                            let selected_media = selected_media.clone();
                            let loading = loading.clone();
                            loading.set(true);
                            spawn_local(async move {
                                match Api::get(&format!("/api/youtube/video/{}", video_id)).send().await {
                                    Ok(response) => {
                                        if response.ok() {
                                            if let Ok(data) = response.json::<VideoDetailsResponse>().await {
                                                selected_media.set(Some(SelectedMedia::YouTube(data)));
                                            }
                                        }
                                    }
                                    Err(_) => {}
                                }
                                loading.set(false);
                            });
                        }
                    }
                }
            }
            || ()
        }, can_subscribe);
    }

    // Fetch subscription feed on mount when connected
    {
        let videos = videos.clone();
        let loading = loading.clone();
        let error = error.clone();
        let youtube_connected = props.youtube_connected;
        use_effect_with_deps(move |_| {
            if youtube_connected {
                loading.set(true);
                spawn_local(async move {
                    match Api::get("/api/youtube/subscriptions").send().await {
                        Ok(response) => {
                            if response.ok() {
                                match response.json::<SubscriptionFeedResponse>().await {
                                    Ok(data) => {
                                        videos.set(data.videos);
                                        error.set(None);
                                    }
                                    Err(e) => {
                                        error.set(Some(format!("Failed to parse response: {}", e)));
                                    }
                                }
                            } else {
                                match response.json::<serde_json::Value>().await {
                                    Ok(data) => {
                                        let err = data["error"].as_str().unwrap_or("Failed to fetch subscriptions").to_string();
                                        error.set(Some(err));
                                    }
                                    Err(_) => {
                                        error.set(Some("Failed to fetch subscriptions".to_string()));
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            error.set(Some(format!("Network error: {}", e)));
                        }
                    }
                    loading.set(false);
                });
            }
            || ()
        }, props.youtube_connected);
    }

    let on_search_input = {
        let search_query = search_query.clone();
        Callback::from(move |e: InputEvent| {
            let input: HtmlInputElement = e.target_unchecked_into();
            search_query.set(input.value());
        })
    };

    let on_search_submit = {
        let search_query = search_query.clone();
        let videos = videos.clone();
        let channels = channels.clone();
        let loading = loading.clone();
        let error = error.clone();
        let showing_search = showing_search.clone();
        let selected_media = selected_media.clone();
        let search_type = search_type.clone();
        Callback::from(move |e: SubmitEvent| {
            e.prevent_default();
            let query = (*search_query).clone();
            if query.is_empty() {
                return;
            }

            // Detect platform from URL
            let platform = detect_platform(&query);

            match platform {
                MediaPlatform::YouTube => {
                    // YouTube URL - fetch video details directly
                    let loading = loading.clone();
                    let error = error.clone();
                    let selected_media = selected_media.clone();
                    let query = query.clone();
                    loading.set(true);
                    spawn_local(async move {
                        let encoded_query = js_sys::encode_uri_component(&query);
                        match Api::get(&format!("/api/youtube/video/{}", encoded_query)).send().await {
                            Ok(response) => {
                                if response.ok() {
                                    match response.json::<VideoDetailsResponse>().await {
                                        Ok(data) => {
                                            selected_media.set(Some(SelectedMedia::YouTube(data)));
                                            error.set(None);
                                        }
                                        Err(e) => {
                                            error.set(Some(format!("Failed to parse video: {}", e)));
                                        }
                                    }
                                } else {
                                    match response.json::<serde_json::Value>().await {
                                        Ok(data) => {
                                            let err = data["error"].as_str().unwrap_or("Failed to fetch video").to_string();
                                            error.set(Some(err));
                                        }
                                        Err(_) => {
                                            error.set(Some("Failed to fetch video".to_string()));
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                error.set(Some(format!("Network error: {}", e)));
                            }
                        }
                        loading.set(false);
                    });
                }
                MediaPlatform::TikTok => {
                    // TikTok URL - resolve via backend
                    let loading = loading.clone();
                    let error = error.clone();
                    let selected_media = selected_media.clone();
                    let query = query.clone();
                    loading.set(true);
                    spawn_local(async move {
                        let request = TikTokResolveRequest { url: query };
                        match Api::post("/api/tiktok/resolve").json(&request).unwrap().send().await {
                            Ok(response) => {
                                if response.ok() {
                                    match response.json::<TikTokEmbedResponse>().await {
                                        Ok(data) => {
                                            selected_media.set(Some(SelectedMedia::TikTok(data)));
                                            error.set(None);
                                        }
                                        Err(e) => {
                                            error.set(Some(format!("Failed to parse TikTok video: {}", e)));
                                        }
                                    }
                                } else {
                                    match response.json::<serde_json::Value>().await {
                                        Ok(data) => {
                                            let err = data["error"].as_str().unwrap_or("Failed to fetch TikTok video").to_string();
                                            error.set(Some(err));
                                        }
                                        Err(_) => {
                                            error.set(Some("Failed to fetch TikTok video".to_string()));
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                error.set(Some(format!("Network error: {}", e)));
                            }
                        }
                        loading.set(false);
                    });
                }
                MediaPlatform::Instagram => {
                    // Instagram URL - resolve via backend
                    let loading = loading.clone();
                    let error = error.clone();
                    let selected_media = selected_media.clone();
                    let query = query.clone();
                    loading.set(true);
                    spawn_local(async move {
                        let request = InstagramResolveRequest { url: query };
                        match Api::post("/api/instagram/resolve").json(&request).unwrap().send().await {
                            Ok(response) => {
                                if response.ok() {
                                    match response.json::<InstagramEmbedResponse>().await {
                                        Ok(data) => {
                                            selected_media.set(Some(SelectedMedia::Instagram(data)));
                                            error.set(None);
                                        }
                                        Err(e) => {
                                            error.set(Some(format!("Failed to parse Instagram reel: {}", e)));
                                        }
                                    }
                                } else {
                                    match response.json::<serde_json::Value>().await {
                                        Ok(data) => {
                                            let err = data["error"].as_str().unwrap_or("Failed to fetch Instagram reel").to_string();
                                            error.set(Some(err));
                                        }
                                        Err(_) => {
                                            error.set(Some("Failed to fetch Instagram reel".to_string()));
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                error.set(Some(format!("Network error: {}", e)));
                            }
                        }
                        loading.set(false);
                    });
                }
                MediaPlatform::Twitter => {
                    // Twitter/X video URL
                    let loading = loading.clone();
                    let error = error.clone();
                    let selected_media = selected_media.clone();
                    let query = query.clone();
                    loading.set(true);
                    spawn_local(async move {
                        let request = TwitterResolveRequest { url: query };
                        match Api::post("/api/twitter/resolve")
                            .json(&request)
                            .unwrap()
                            .send()
                            .await
                        {
                            Ok(response) => {
                                if response.ok() {
                                    match response.json::<TwitterEmbedResponse>().await {
                                        Ok(data) => {
                                            selected_media.set(Some(SelectedMedia::Twitter(data)));
                                            error.set(None);
                                        }
                                        Err(e) => {
                                            error.set(Some(format!("Failed to parse Twitter post: {}", e)));
                                        }
                                    }
                                } else {
                                    match response.json::<serde_json::Value>().await {
                                        Ok(data) => {
                                            let err = data["error"].as_str().unwrap_or("Failed to fetch Twitter post").to_string();
                                            error.set(Some(err));
                                        }
                                        Err(_) => {
                                            error.set(Some("Failed to fetch Twitter post".to_string()));
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                error.set(Some(format!("Network error: {}", e)));
                            }
                        }
                        loading.set(false);
                    });
                }
                MediaPlatform::Reddit => {
                    let loading = loading.clone();
                    let error = error.clone();
                    let selected_media = selected_media.clone();
                    let query = query.clone();
                    loading.set(true);
                    spawn_local(async move {
                        let request = RedditResolveRequest { url: query };
                        match Api::post("/api/reddit/resolve")
                            .json(&request)
                            .unwrap()
                            .send()
                            .await
                        {
                            Ok(response) => {
                                if response.ok() {
                                    match response.json::<RedditEmbedResponse>().await {
                                        Ok(data) => {
                                            selected_media.set(Some(SelectedMedia::Reddit(data)));
                                            error.set(None);
                                        }
                                        Err(e) => {
                                            error.set(Some(format!("Failed to parse Reddit post: {}", e)));
                                        }
                                    }
                                } else {
                                    match response.json::<serde_json::Value>().await {
                                        Ok(data) => {
                                            let err = data["error"].as_str().unwrap_or("Failed to fetch Reddit post").to_string();
                                            error.set(Some(err));
                                        }
                                        Err(_) => {
                                            error.set(Some("Failed to fetch Reddit post".to_string()));
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                error.set(Some(format!("Network error: {}", e)));
                            }
                        }
                        loading.set(false);
                    });
                }
                MediaPlatform::Spotify => {
                    let loading = loading.clone();
                    let error = error.clone();
                    let selected_media = selected_media.clone();
                    let query = query.clone();
                    loading.set(true);
                    spawn_local(async move {
                        let request = SpotifyResolveRequest { url: query };
                        match Api::post("/api/spotify/resolve")
                            .json(&request)
                            .unwrap()
                            .send()
                            .await
                        {
                            Ok(response) => {
                                if response.ok() {
                                    match response.json::<SpotifyEmbedResponse>().await {
                                        Ok(data) => {
                                            selected_media.set(Some(SelectedMedia::Spotify(data)));
                                            error.set(None);
                                        }
                                        Err(e) => {
                                            error.set(Some(format!("Failed to parse Spotify content: {}", e)));
                                        }
                                    }
                                } else {
                                    match response.json::<serde_json::Value>().await {
                                        Ok(data) => {
                                            let err = data["error"].as_str().unwrap_or("Failed to fetch Spotify content").to_string();
                                            error.set(Some(err));
                                        }
                                        Err(_) => {
                                            error.set(Some("Failed to fetch Spotify content".to_string()));
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                error.set(Some(format!("Network error: {}", e)));
                            }
                        }
                        loading.set(false);
                    });
                }
                MediaPlatform::Rumble => {
                    let loading = loading.clone();
                    let error = error.clone();
                    let selected_media = selected_media.clone();
                    let query = query.clone();
                    loading.set(true);
                    spawn_local(async move {
                        let request = RumbleResolveRequest { url: query };
                        match Api::post("/api/rumble/resolve")
                            .json(&request)
                            .unwrap()
                            .send()
                            .await
                        {
                            Ok(response) => {
                                if response.ok() {
                                    match response.json::<RumbleEmbedResponse>().await {
                                        Ok(data) => {
                                            selected_media.set(Some(SelectedMedia::Rumble(data)));
                                            error.set(None);
                                        }
                                        Err(e) => {
                                            error.set(Some(format!("Failed to parse Rumble video: {}", e)));
                                        }
                                    }
                                } else {
                                    match response.json::<serde_json::Value>().await {
                                        Ok(data) => {
                                            let err = data["error"].as_str().unwrap_or("Failed to fetch Rumble video").to_string();
                                            error.set(Some(err));
                                        }
                                        Err(_) => {
                                            error.set(Some("Failed to fetch Rumble video".to_string()));
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                error.set(Some(format!("Network error: {}", e)));
                            }
                        }
                        loading.set(false);
                    });
                }
                MediaPlatform::Streamable => {
                    let loading = loading.clone();
                    let error = error.clone();
                    let selected_media = selected_media.clone();
                    let query = query.clone();
                    loading.set(true);
                    spawn_local(async move {
                        let request = StreamableResolveRequest { url: query };
                        match Api::post("/api/streamable/resolve")
                            .json(&request)
                            .unwrap()
                            .send()
                            .await
                        {
                            Ok(response) => {
                                if response.ok() {
                                    match response.json::<StreamableEmbedResponse>().await {
                                        Ok(data) => {
                                            selected_media.set(Some(SelectedMedia::Streamable(data)));
                                            error.set(None);
                                        }
                                        Err(e) => {
                                            error.set(Some(format!("Failed to parse Streamable video: {}", e)));
                                        }
                                    }
                                } else {
                                    match response.json::<serde_json::Value>().await {
                                        Ok(data) => {
                                            let err = data["error"].as_str().unwrap_or("Failed to fetch Streamable video").to_string();
                                            error.set(Some(err));
                                        }
                                        Err(_) => {
                                            error.set(Some("Failed to fetch Streamable video".to_string()));
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                error.set(Some(format!("Network error: {}", e)));
                            }
                        }
                        loading.set(false);
                    });
                }
                MediaPlatform::Bluesky => {
                    let loading = loading.clone();
                    let error = error.clone();
                    let selected_media = selected_media.clone();
                    let query = query.clone();
                    loading.set(true);
                    spawn_local(async move {
                        let request = BlueskyResolveRequest { url: query };
                        match Api::post("/api/bluesky/resolve")
                            .json(&request)
                            .unwrap()
                            .send()
                            .await
                        {
                            Ok(response) => {
                                if response.ok() {
                                    match response.json::<BlueskyEmbedResponse>().await {
                                        Ok(data) => {
                                            selected_media.set(Some(SelectedMedia::Bluesky(data)));
                                            error.set(None);
                                        }
                                        Err(e) => {
                                            error.set(Some(format!("Failed to parse Bluesky post: {}", e)));
                                        }
                                    }
                                } else {
                                    match response.json::<serde_json::Value>().await {
                                        Ok(data) => {
                                            let err = data["error"].as_str().unwrap_or("Failed to fetch Bluesky post").to_string();
                                            error.set(Some(err));
                                        }
                                        Err(_) => {
                                            error.set(Some("Failed to fetch Bluesky post".to_string()));
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                error.set(Some(format!("Network error: {}", e)));
                            }
                        }
                        loading.set(false);
                    });
                }
                MediaPlatform::Unknown => {
                    // Treat as YouTube search
                    let videos = videos.clone();
                    let channels = channels.clone();
                    let loading = loading.clone();
                    let error = error.clone();
                    let showing_search = showing_search.clone();
                    let query = query.clone();
                    let type_param = match *search_type {
                        SearchType::Videos => "video",
                        SearchType::Channels => "channel",
                        SearchType::All => "all",
                    };
                    loading.set(true);
                    showing_search.set(true);
                    spawn_local(async move {
                        let encoded_query = js_sys::encode_uri_component(&query);
                        match Api::get(&format!("/api/youtube/search?q={}&type={}", encoded_query, type_param)).send().await {
                            Ok(response) => {
                                if response.ok() {
                                    match response.json::<SearchResponse>().await {
                                        Ok(data) => {
                                            videos.set(data.videos);
                                            channels.set(data.channels.unwrap_or_default());
                                            error.set(None);
                                        }
                                        Err(e) => {
                                            error.set(Some(format!("Failed to parse results: {}", e)));
                                        }
                                    }
                                } else {
                                    match response.json::<serde_json::Value>().await {
                                        Ok(data) => {
                                            let err = data["error"].as_str().unwrap_or("Search failed").to_string();
                                            error.set(Some(err));
                                        }
                                        Err(_) => {
                                            error.set(Some("Search failed".to_string()));
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                error.set(Some(format!("Network error: {}", e)));
                            }
                        }
                        loading.set(false);
                    });
                }
            }
        })
    };

    let clear_search = {
        let search_query = search_query.clone();
        let showing_search = showing_search.clone();
        let videos = videos.clone();
        let channels = channels.clone();
        let loading = loading.clone();
        let error = error.clone();
        let selected_media = selected_media.clone();
        let youtube_connected = props.youtube_connected;
        Callback::from(move |_: MouseEvent| {
            search_query.set(String::new());
            showing_search.set(false);
            selected_media.set(None);
            channels.set(Vec::new());

            // Refetch subscriptions
            if youtube_connected {
                let videos = videos.clone();
                let loading = loading.clone();
                let error = error.clone();
                loading.set(true);
                spawn_local(async move {
                    match Api::get("/api/youtube/subscriptions").send().await {
                        Ok(response) => {
                            if response.ok() {
                                if let Ok(data) = response.json::<SubscriptionFeedResponse>().await {
                                    videos.set(data.videos);
                                    error.set(None);
                                }
                            }
                        }
                        Err(_) => {}
                    }
                    loading.set(false);
                });
            }
        })
    };

    // Subscribe action - checks if user has write permissions first
    let on_subscribe = {
        let channels = channels.clone();
        let subscribing_channel = subscribing_channel.clone();
        let error = error.clone();
        let show_upgrade_modal = show_upgrade_modal.clone();
        let pending_channel_id = pending_channel_id.clone();
        let can_subscribe = props.can_subscribe;
        Callback::from(move |channel_id: String| {
            if !can_subscribe {
                // Show upgrade modal instead of subscribing
                pending_channel_id.set(Some(channel_id));
                show_upgrade_modal.set(true);
                return;
            }

            let channels = channels.clone();
            let subscribing_channel = subscribing_channel.clone();
            let error = error.clone();
            let channel_id_clone = channel_id.clone();
            subscribing_channel.set(Some(channel_id.clone()));
            spawn_local(async move {
                let body = serde_json::json!({ "channel_id": channel_id_clone });
                match Api::post("/api/youtube/subscribe")
                    .json(&body)
                    .unwrap()
                    .send()
                    .await
                {
                    Ok(response) => {
                        if response.ok() {
                            // Update the channel's is_subscribed status
                            let mut updated_channels = (*channels).clone();
                            for ch in &mut updated_channels {
                                if ch.id == channel_id_clone {
                                    ch.is_subscribed = true;
                                }
                            }
                            channels.set(updated_channels);
                            error.set(None);
                        } else {
                            if let Ok(data) = response.json::<serde_json::Value>().await {
                                let err = data["error"].as_str().unwrap_or("Failed to subscribe").to_string();
                                error.set(Some(err));
                            }
                        }
                    }
                    Err(e) => {
                        error.set(Some(format!("Network error: {}", e)));
                    }
                }
                subscribing_channel.set(None);
            });
        })
    };

    // Unsubscribe action - also checks permissions
    let on_unsubscribe = {
        let channels = channels.clone();
        let videos = videos.clone();
        let subscribing_channel = subscribing_channel.clone();
        let error = error.clone();
        let show_upgrade_modal = show_upgrade_modal.clone();
        let pending_channel_id = pending_channel_id.clone();
        let can_subscribe = props.can_subscribe;
        Callback::from(move |channel_id: String| {
            if !can_subscribe {
                // Show upgrade modal
                pending_channel_id.set(Some(channel_id));
                show_upgrade_modal.set(true);
                return;
            }

            // Immediately remove videos from this channel (optimistic update)
            let updated_videos: Vec<Video> = (*videos).iter()
                .filter(|v| v.channel_id != channel_id)
                .cloned()
                .collect();
            videos.set(updated_videos);

            let channels = channels.clone();
            let subscribing_channel = subscribing_channel.clone();
            let error = error.clone();
            let channel_id_clone = channel_id.clone();
            subscribing_channel.set(Some(channel_id.clone()));
            spawn_local(async move {
                match Api::delete(&format!("/api/youtube/unsubscribe/{}", channel_id_clone))
                    .send()
                    .await
                {
                    Ok(response) => {
                        if response.ok() {
                            // Update the channel's is_subscribed status
                            let mut updated_channels = (*channels).clone();
                            for ch in &mut updated_channels {
                                if ch.id == channel_id_clone {
                                    ch.is_subscribed = false;
                                }
                            }
                            channels.set(updated_channels);
                            error.set(None);
                        } else {
                            if let Ok(data) = response.json::<serde_json::Value>().await {
                                let err = data["error"].as_str().unwrap_or("Failed to unsubscribe").to_string();
                                error.set(Some(err));
                            }
                        }
                    }
                    Err(e) => {
                        error.set(Some(format!("Network error: {}", e)));
                    }
                }
                subscribing_channel.set(None);
            });
        })
    };

    let on_video_click = {
        let selected_media = selected_media.clone();
        let loading = loading.clone();
        let error = error.clone();
        Callback::from(move |video_id: String| {
            let selected_media = selected_media.clone();
            let loading = loading.clone();
            let error = error.clone();
            loading.set(true);
            spawn_local(async move {
                match Api::get(&format!("/api/youtube/video/{}", video_id)).send().await {
                    Ok(response) => {
                        if response.ok() {
                            match response.json::<VideoDetailsResponse>().await {
                                Ok(data) => {
                                    selected_media.set(Some(SelectedMedia::YouTube(data)));
                                    error.set(None);
                                }
                                Err(e) => {
                                    error.set(Some(format!("Failed to load video: {}", e)));
                                }
                            }
                        } else {
                            error.set(Some("Failed to load video".to_string()));
                        }
                    }
                    Err(e) => {
                        error.set(Some(format!("Network error: {}", e)));
                    }
                }
                loading.set(false);
            });
        })
    };

    let close_media = {
        let selected_media = selected_media.clone();
        Callback::from(move |_: MouseEvent| {
            selected_media.set(None);
        })
    };

    // Channel click handler - loads videos from that channel
    let on_channel_click = {
        let videos = videos.clone();
        let channels = channels.clone();
        let loading = loading.clone();
        let error = error.clone();
        let viewing_channel = viewing_channel.clone();
        Callback::from(move |channel_info: (String, String)| {
            let (channel_id, channel_name) = channel_info;
            let videos = videos.clone();
            let channels = channels.clone();
            let loading = loading.clone();
            let error = error.clone();
            let viewing_channel = viewing_channel.clone();

            viewing_channel.set(Some((channel_id.clone(), channel_name)));
            channels.set(Vec::new()); // Clear channels when viewing channel videos
            loading.set(true);

            spawn_local(async move {
                // Search for videos from this channel (empty query returns recent uploads)
                let url = format!("/api/youtube/search?q=&type=video&channel_id={}", channel_id);
                match Api::get(&url).send().await {
                    Ok(response) => {
                        if response.ok() {
                            match response.json::<SearchResponse>().await {
                                Ok(data) => {
                                    videos.set(data.videos);
                                    error.set(None);
                                }
                                Err(e) => {
                                    error.set(Some(format!("Failed to load channel videos: {}", e)));
                                }
                            }
                        } else {
                            error.set(Some("Failed to load channel videos".to_string()));
                        }
                    }
                    Err(e) => {
                        error.set(Some(format!("Network error: {}", e)));
                    }
                }
                loading.set(false);
            });
        })
    };

    // Back to search results from channel view
    let on_back_to_search = {
        let viewing_channel = viewing_channel.clone();
        let search_query = search_query.clone();
        let videos = videos.clone();
        let channels = channels.clone();
        let loading = loading.clone();
        let error = error.clone();
        Callback::from(move |_: MouseEvent| {
            let query = (*search_query).clone();
            viewing_channel.set(None);

            if query.is_empty() {
                // Just clear and go back to empty state
                videos.set(Vec::new());
                channels.set(Vec::new());
                return;
            }

            // Re-run the original search
            let videos = videos.clone();
            let channels = channels.clone();
            let loading = loading.clone();
            let error = error.clone();
            loading.set(true);

            spawn_local(async move {
                let encoded_query = js_sys::encode_uri_component(&query);
                match Api::get(&format!("/api/youtube/search?q={}&type=all", encoded_query)).send().await {
                    Ok(response) => {
                        if response.ok() {
                            match response.json::<SearchResponse>().await {
                                Ok(data) => {
                                    videos.set(data.videos);
                                    channels.set(data.channels.unwrap_or_default());
                                    error.set(None);
                                }
                                Err(e) => {
                                    error.set(Some(format!("Failed to load results: {}", e)));
                                }
                            }
                        }
                    }
                    Err(e) => {
                        error.set(Some(format!("Network error: {}", e)));
                    }
                }
                loading.set(false);
            });
        })
    };

    // Not connected state - show prompt but allow TikTok/Instagram URLs
    if !props.youtube_connected {
        return html! {
            <div class="media-hub-container">
                <div class="media-hub-header">
                    <span class="header-title">{"Media"}</span>
                </div>

                <p class="media-description">
                    {"Watch videos without algorithmic recommendations, autoplay, or endless feeds. Paste a URL or search intentionally."}
                </p>

                <div class="youtube-not-connected-banner">
                    <img src="https://upload.wikimedia.org/wikipedia/commons/0/09/YouTube_full-color_icon_%282017%29.svg" alt="YouTube" width="32" height="32"/>
                    <div class="banner-text">
                        <span class="banner-title">{"Connect YouTube for search & subscriptions"}</span>
                        <span class="banner-subtitle">{"TikTok & Instagram URLs work without connecting"}</span>
                    </div>
                    <a href="/" class="connect-button-small">{"Connect"}</a>
                </div>

                // Search bar - works for TikTok/Instagram even when YouTube not connected
                <form onsubmit={on_search_submit.clone()} class="search-form">
                    <div class="search-input-wrapper">
                        <input
                            type="text"
                            placeholder="Paste video URL or search YouTube..."
                            value={(*search_query).clone()}
                            oninput={on_search_input.clone()}
                            class="search-input"
                        />
                        {
                            if !search_query.is_empty() {
                                html! {
                                    <button type="button" onclick={clear_search.clone()} class="clear-button">
                                        {"×"}
                                    </button>
                                }
                            } else { html! {} }
                        }
                    </div>
                    <button type="submit" class="search-button">
                        {"Go"}
                    </button>
                </form>

                // Error message
                {
                    if let Some(err) = (*error).as_ref() {
                        html! {
                            <div class="error-message">{err}</div>
                        }
                    } else {
                        html! {}
                    }
                }

                // Show selected TikTok/Instagram video even when YouTube not connected
                {
                    if let Some(media) = (*selected_media).as_ref() {
                        match media {
                            SelectedMedia::YouTube(video_details) => html! {
                                <YouTubeVideoPlayer
                                    video_details={video_details.clone()}
                                    can_comment={props.can_subscribe}
                                    on_close={close_media.clone()}
                                />
                            },
                            _ => render_media_player(media, close_media.clone())
                        }
                    } else {
                        html! {}
                    }
                }

                <style>{get_styles()}</style>
            </div>
        };
    }

    // Media player modal (YouTube, TikTok, or Instagram)
    if let Some(media) = (*selected_media).as_ref() {
        let can_comment = props.can_subscribe;
        return html! {
            <div class="media-hub-container">
                {
                    match media {
                        SelectedMedia::YouTube(video_details) => html! {
                            <YouTubeVideoPlayer
                                video_details={video_details.clone()}
                                can_comment={can_comment}
                                on_close={close_media.clone()}
                            />
                        },
                        _ => render_media_player(media, close_media.clone())
                    }
                }
                <style>{get_styles()}</style>
            </div>
        };
    }

    // Search type filter callbacks - also trigger search if query exists
    let is_all = matches!(*search_type, SearchType::All);
    let is_videos = matches!(*search_type, SearchType::Videos);
    let is_channels = matches!(*search_type, SearchType::Channels);

    // Helper function to perform search with given type
    let perform_search = {
        let search_query = search_query.clone();
        let videos = videos.clone();
        let channels = channels.clone();
        let loading = loading.clone();
        let error = error.clone();
        let showing_search = showing_search.clone();
        move |type_param: &'static str| {
            let query = (*search_query).clone();
            if query.is_empty() {
                return;
            }
            let videos = videos.clone();
            let channels = channels.clone();
            let loading = loading.clone();
            let error = error.clone();
            let showing_search = showing_search.clone();
            loading.set(true);
            showing_search.set(true);
            spawn_local(async move {
                let encoded_query = js_sys::encode_uri_component(&query);
                match Api::get(&format!("/api/youtube/search?q={}&type={}", encoded_query, type_param)).send().await {
                    Ok(response) => {
                        if response.ok() {
                            match response.json::<SearchResponse>().await {
                                Ok(data) => {
                                    videos.set(data.videos);
                                    channels.set(data.channels.unwrap_or_default());
                                    error.set(None);
                                }
                                Err(e) => {
                                    error.set(Some(format!("Failed to parse results: {}", e)));
                                }
                            }
                        } else {
                            match response.json::<serde_json::Value>().await {
                                Ok(data) => {
                                    let err = data["error"].as_str().unwrap_or("Search failed").to_string();
                                    error.set(Some(err));
                                }
                                Err(_) => {
                                    error.set(Some("Search failed".to_string()));
                                }
                            }
                        }
                    }
                    Err(e) => {
                        error.set(Some(format!("Network error: {}", e)));
                    }
                }
                loading.set(false);
            });
        }
    };

    let set_all = {
        let search_type = search_type.clone();
        let perform_search = perform_search.clone();
        let showing_search = showing_search.clone();
        Callback::from(move |_: MouseEvent| {
            search_type.set(SearchType::All);
            if *showing_search {
                perform_search("all");
            }
        })
    };
    let set_videos = {
        let search_type = search_type.clone();
        let perform_search = perform_search.clone();
        let showing_search = showing_search.clone();
        Callback::from(move |_: MouseEvent| {
            search_type.set(SearchType::Videos);
            if *showing_search {
                perform_search("video");
            }
        })
    };
    let set_channels = {
        let search_type = search_type.clone();
        let perform_search = perform_search.clone();
        let showing_search = showing_search.clone();
        Callback::from(move |_: MouseEvent| {
            search_type.set(SearchType::Channels);
            if *showing_search {
                perform_search("channel");
            }
        })
    };

    // Modal callbacks
    let close_upgrade_modal = {
        let show_upgrade_modal = show_upgrade_modal.clone();
        let pending_channel_id = pending_channel_id.clone();
        Callback::from(move |_: MouseEvent| {
            show_upgrade_modal.set(false);
            pending_channel_id.set(None);
        })
    };

    // Subscribe once then auto-downgrade
    let on_subscribe_once_click = {
        let loading = loading.clone();
        let auto_downgrade_after_subscribe = auto_downgrade_after_subscribe.clone();
        let pending_channel_id = pending_channel_id.clone();
        Callback::from(move |_: MouseEvent| {
            let loading = loading.clone();
            let auto_downgrade_after_subscribe = auto_downgrade_after_subscribe.clone();
            let pending_channel_id = pending_channel_id.clone();
            loading.set(true);
            auto_downgrade_after_subscribe.set(true);
            spawn_local(async move {
                match Api::get("/api/auth/youtube/upgrade").send().await {
                    Ok(response) => {
                        if response.ok() {
                            if let Ok(data) = response.json::<serde_json::Value>().await {
                                if let Some(auth_url) = data["auth_url"].as_str() {
                                    // Store auto-downgrade flag AND channel ID in localStorage before redirect
                                    if let Some(window) = web_sys::window() {
                                        if let Ok(Some(storage)) = window.local_storage() {
                                            let _ = storage.set_item("youtube_auto_downgrade", "true");
                                            // Store the channel to subscribe to
                                            if let Some(channel_id) = (*pending_channel_id).as_ref() {
                                                let _ = storage.set_item("youtube_pending_subscribe", channel_id);
                                            }
                                        }
                                        let _ = window.location().set_href(auth_url);
                                    }
                                }
                            }
                        }
                    }
                    Err(_) => {}
                }
                loading.set(false);
            });
        })
    };

    let pending_channel_for_youtube = (*pending_channel_id).clone();

    // Connected state - main view
    html! {
        <div class="media-hub-container">
            <div class="media-hub-header">
                <span class="header-title">{"Media"}</span>
                <span class="connected-badge youtube-badge">{"YouTube Connected"}</span>
            </div>

            <p class="media-description">
                {"Watch videos without algorithmic recommendations, autoplay, or endless feeds. Paste a URL or search intentionally."}
            </p>

            // Search type filter
            <div class="search-type-filter">
                <button
                    type="button"
                    class={classes!("filter-button", is_all.then_some("active"))}
                    onclick={set_all}
                >
                    {"All"}
                </button>
                <button
                    type="button"
                    class={classes!("filter-button", is_videos.then_some("active"))}
                    onclick={set_videos}
                >
                    {"Videos"}
                </button>
                <button
                    type="button"
                    class={classes!("filter-button", is_channels.then_some("active"))}
                    onclick={set_channels}
                >
                    {"Channels"}
                </button>
            </div>

            // Search bar
            <form onsubmit={on_search_submit} class="search-form">
                <div class="search-input-wrapper">
                    <input
                        type="text"
                        placeholder="Paste URL (YouTube, TikTok, Instagram, X) or search YouTube..."
                        value={(*search_query).clone()}
                        oninput={on_search_input}
                        class="search-input"
                    />
                    {
                        if !search_query.is_empty() || *showing_search {
                            html! {
                                <button type="button" onclick={clear_search} class="clear-button">
                                    {"×"}
                                </button>
                            }
                        } else { html! {} }
                    }
                </div>
                <button type="submit" class="search-button">
                    {"Search"}
                </button>
            </form>

            // Error message
            {
                if let Some(err) = (*error).as_ref() {
                    html! {
                        <div class="error-message">{err}</div>
                    }
                } else {
                    html! {}
                }
            }

            // Success message
            {
                if let Some(msg) = (*success_message).as_ref() {
                    html! {
                        <div class="success-message">{msg}</div>
                    }
                } else {
                    html! {}
                }
            }

            // Content
            {render_content(
                &videos,
                &channels,
                *loading,
                *showing_search,
                &subscribing_channel,
                &on_subscribe,
                &on_unsubscribe,
                &on_video_click,
                &on_channel_click,
                &*viewing_channel,
                &on_back_to_search,
                props.can_subscribe,
            )}

            // Upgrade scope modal
            if *show_upgrade_modal {
                <div class="modal-overlay" onclick={close_upgrade_modal.clone()}>
                    <div class="upgrade-modal" onclick={Callback::from(|e: MouseEvent| e.stop_propagation())}>
                        <h3>{"Subscribe to Channel"}</h3>
                        <p class="modal-description">
                            {"To subscribe to channels through Lightfriend, you need to grant additional permissions. This allows Lightfriend to manage your YouTube subscriptions."}
                        </p>
                        <p class="modal-warning">
                            {"Note: This grants broader access to your YouTube account (required by YouTube's API). Lightfriend only uses it for subscribing/unsubscribing."}
                        </p>
                        <div class="modal-buttons">
                            <button class="modal-button primary" onclick={on_subscribe_once_click.clone()}>
                                {"Grant Permission & Subscribe"}
                                <span class="button-subtitle">{"(permission auto-revokes after subscribing)"}</span>
                            </button>
                            if let Some(channel_id) = pending_channel_for_youtube.as_ref() {
                                <a
                                    href={format!("https://www.youtube.com/channel/{}?sub_confirmation=1", channel_id)}
                                    target="_blank"
                                    rel="noopener noreferrer"
                                    class="modal-button secondary"
                                >
                                    {"Subscribe on YouTube instead"}
                                </a>
                            }
                            <button class="modal-button cancel" onclick={close_upgrade_modal}>
                                {"Cancel"}
                            </button>
                        </div>
                    </div>
                </div>
            }

            <style>{get_styles()}</style>
        </div>
    }
}

/// Renders the appropriate media player based on the selected media type
/// Renders TikTok and Instagram media players (YouTube is handled by YouTubeVideoPlayer component)
fn render_media_player(media: &SelectedMedia, close_callback: Callback<MouseEvent>) -> Html {
    match media {
        SelectedMedia::YouTube(_) => {
            // YouTube is now handled by the YouTubeVideoPlayer component
            html! { <></> }
        }
        SelectedMedia::TikTok(tiktok_data) => {
            html! {
                <div class="video-player-container">
                    <div class="video-player-header">
                        <button onclick={close_callback} class="back-button">
                            {"← Back"}
                        </button>
                        <span class="platform-badge tiktok">{"TikTok"}</span>
                    </div>
                    <div class="video-embed-wrapper vertical-aspect">
                        <iframe
                            src={tiktok_data.embed_url.clone()}
                            title={tiktok_data.title.clone()}
                            frameborder="0"
                            allow="accelerometer; autoplay; clipboard-write; encrypted-media; gyroscope; picture-in-picture; fullscreen"
                            allowfullscreen=true
                            class="video-embed"
                        />
                    </div>
                    <div class="video-details">
                        <h2 class="video-player-title">{&tiktok_data.title}</h2>
                        <div class="video-player-meta">
                            <span class="channel-name">{format!("@{}", &tiktok_data.author)}</span>
                        </div>
                    </div>
                    <p class="intentional-note">{"One video only • No For You feed • No endless scroll"}</p>
                </div>
            }
        }
        SelectedMedia::Instagram(instagram_data) => {
            let embed_url = instagram_data.embed_url.clone();
            html! {
                <div class="video-player-container">
                    <div class="video-player-header">
                        <button onclick={close_callback} class="back-button">
                            {"← Back"}
                        </button>
                        <span class="platform-badge instagram">{"Instagram"}</span>
                    </div>
                    <div class="video-embed-wrapper vertical-aspect">
                        <iframe
                            id="instagram-embed-iframe"
                            src={embed_url}
                            title="Instagram Reel"
                            frameborder="0"
                            allow="accelerometer; autoplay; clipboard-write; encrypted-media; gyroscope; picture-in-picture; fullscreen"
                            allowfullscreen=true
                            class="video-embed"
                        />
                    </div>
                    <p class="intentional-note">{"Single reel • No Explore page • No suggestions"}</p>
                    <div class="platform-disclaimer instagram-disclaimer">
                        <span class="disclaimer-icon">{"⚠"}</span>
                        <span>{"Instagram often blocks embedded playback to force you into their app. If the video won't play, that's Instagram being Instagram - there's not much we can do about it."}</span>
                    </div>
                </div>
            }
        }
        SelectedMedia::Twitter(twitter_data) => {
            let embed_url = twitter_data.embed_url.clone();
            let author_display = twitter_data.author.clone().unwrap_or_else(|| "Post".to_string());
            html! {
                <div class="video-player-container">
                    <div class="video-player-header">
                        <button onclick={close_callback} class="back-button">
                            {"← Back"}
                        </button>
                        <span class="platform-badge twitter">{"X / Twitter"}</span>
                    </div>
                    <div class="video-embed-wrapper twitter-aspect">
                        <iframe
                            src={embed_url}
                            title={format!("Tweet by {}", author_display)}
                            frameborder="0"
                            allow="accelerometer; autoplay; clipboard-write; encrypted-media; gyroscope; picture-in-picture; fullscreen"
                            allowfullscreen=true
                            class="video-embed twitter-embed"
                        />
                    </div>
                    <p class="intentional-note">{"Single post • No timeline • No algorithm"}</p>
                </div>
            }
        }
        SelectedMedia::Reddit(reddit_data) => {
            let embed_url = reddit_data.embed_url.clone();
            let subreddit_display = reddit_data.subreddit.clone().unwrap_or_else(|| "post".to_string());
            html! {
                <div class="video-player-container">
                    <div class="video-player-header">
                        <button onclick={close_callback} class="back-button">
                            {"← Back"}
                        </button>
                        <span class="platform-badge reddit">{"Reddit"}</span>
                    </div>
                    <div class="video-embed-wrapper reddit-aspect">
                        <iframe
                            src={embed_url}
                            title={format!("Reddit r/{}", subreddit_display)}
                            frameborder="0"
                            allow="accelerometer; autoplay; clipboard-write; encrypted-media; gyroscope; picture-in-picture; fullscreen"
                            allowfullscreen=true
                            class="video-embed reddit-embed"
                        />
                    </div>
                    <p class="intentional-note">{"Single post • No infinite scroll • No rabbit holes"}</p>
                </div>
            }
        }
        SelectedMedia::Spotify(spotify_data) => {
            let embed_url = spotify_data.embed_url.clone();
            let content_type = spotify_data.content_type.clone();
            html! {
                <div class="video-player-container">
                    <div class="video-player-header">
                        <button onclick={close_callback} class="back-button">
                            {"← Back"}
                        </button>
                        <span class="platform-badge spotify">{"Spotify"}</span>
                    </div>
                    <div class="video-embed-wrapper spotify-aspect">
                        <iframe
                            src={embed_url}
                            title={format!("Spotify {}", content_type)}
                            frameborder="0"
                            allow="autoplay; clipboard-write; encrypted-media; fullscreen; picture-in-picture"
                            allowfullscreen=true
                            class="video-embed spotify-embed"
                        />
                    </div>
                    <p class="intentional-note">{"Just this content • No autoplay queue • No Discovery Weekly rabbit hole"}</p>
                </div>
            }
        }
        SelectedMedia::Rumble(rumble_data) => {
            let embed_url = rumble_data.embed_url.clone();
            html! {
                <div class="video-player-container">
                    <div class="video-player-header">
                        <button onclick={close_callback} class="back-button">
                            {"← Back"}
                        </button>
                        <span class="platform-badge rumble">{"Rumble"}</span>
                    </div>
                    <div class="video-embed-wrapper youtube-aspect">
                        <iframe
                            src={embed_url}
                            title="Rumble Video"
                            frameborder="0"
                            allow="accelerometer; autoplay; clipboard-write; encrypted-media; gyroscope; picture-in-picture; fullscreen"
                            allowfullscreen=true
                            class="video-embed"
                        />
                    </div>
                    <p class="intentional-note">{"One video • No recommendations • No endless feed"}</p>
                </div>
            }
        }
        SelectedMedia::Streamable(streamable_data) => {
            let embed_url = streamable_data.embed_url.clone();
            html! {
                <div class="video-player-container">
                    <div class="video-player-header">
                        <button onclick={close_callback} class="back-button">
                            {"← Back"}
                        </button>
                        <span class="platform-badge streamable">{"Streamable"}</span>
                    </div>
                    <div class="video-embed-wrapper youtube-aspect">
                        <iframe
                            src={embed_url}
                            title="Streamable Video"
                            frameborder="0"
                            allow="accelerometer; autoplay; clipboard-write; encrypted-media; gyroscope; picture-in-picture; fullscreen"
                            allowfullscreen=true
                            class="video-embed"
                        />
                    </div>
                    <p class="intentional-note">{"Just this clip • No suggestions"}</p>
                </div>
            }
        }
        SelectedMedia::Bluesky(bluesky_data) => {
            let embed_url = bluesky_data.embed_url.clone();
            let handle = bluesky_data.handle.clone();
            html! {
                <div class="video-player-container">
                    <div class="video-player-header">
                        <button onclick={close_callback} class="back-button">
                            {"← Back"}
                        </button>
                        <span class="platform-badge bluesky">{"Bluesky"}</span>
                    </div>
                    <div class="video-embed-wrapper bluesky-aspect">
                        <iframe
                            src={embed_url}
                            title={format!("Post by @{}", handle)}
                            frameborder="0"
                            allowfullscreen=true
                            class="video-embed bluesky-embed"
                        />
                    </div>
                    <p class="intentional-note">{"Single post • No timeline • No algorithm"}</p>
                </div>
            }
        }
    }
}

fn render_video_card(
    video: &Video,
    onclick: Callback<MouseEvent>,
    can_unsubscribe: bool,
    is_unsubscribing: bool,
    on_unsubscribe: Callback<MouseEvent>,
) -> Html {
    let time_ago = format_time_ago(&video.published_at);

    html! {
        <div class="video-card" onclick={onclick}>
            <div class="video-thumbnail">
                if !video.thumbnail.is_empty() {
                    <img src={video.thumbnail.clone()} alt={video.title.clone()} />
                } else {
                    <div class="thumbnail-placeholder">
                        {"▶"}
                    </div>
                }
                if !video.duration.is_empty() {
                    <span class="video-duration">{&video.duration}</span>
                }
            </div>
            <div class="video-info">
                <h4 class="video-title">{&video.title}</h4>
                <div class="video-meta">
                    <span class="channel-name-wrapper">
                        <span class="channel-name">{&video.channel}</span>
                        if can_unsubscribe {
                            <button
                                class="unsubscribe-hover-btn"
                                onclick={on_unsubscribe}
                                disabled={is_unsubscribing}
                            >
                                if is_unsubscribing {
                                    {"..."}
                                } else {
                                    {"Unsubscribe"}
                                }
                            </button>
                        }
                    </span>
                    if !video.view_count.is_empty() {
                        <>
                            <span class="meta-separator">{"•"}</span>
                            <span class="video-views">{&video.view_count}</span>
                        </>
                    }
                    <span class="meta-separator">{"•"}</span>
                    <span class="video-time">{time_ago}</span>
                </div>
            </div>
        </div>
    }
}

fn render_channel_card(
    channel: &Channel,
    is_subscribing: bool,
    on_action: Callback<MouseEvent>,
    on_channel_click: Callback<MouseEvent>,
) -> Html {
    html! {
        <div class="channel-card">
            <div class="channel-clickable" onclick={on_channel_click}>
                <div class="channel-avatar">
                    if !channel.thumbnail.is_empty() {
                        <img src={channel.thumbnail.clone()} alt={channel.title.clone()} />
                    } else {
                        <div class="avatar-placeholder">
                            {channel.title.chars().next().unwrap_or('?')}
                        </div>
                    }
                </div>
                <div class="channel-info">
                    <h4 class="channel-title">{&channel.title}</h4>
                    <div class="channel-meta">
                        <span class="subscriber-count">{&channel.subscriber_count}</span>
                    </div>
                    if !channel.description.is_empty() {
                        <p class="channel-description">{&channel.description}</p>
                    }
                </div>
            </div>
            <button
                class={classes!(
                    "subscribe-button",
                    channel.is_subscribed.then_some("subscribed")
                )}
                onclick={on_action}
                disabled={is_subscribing}
            >
                if is_subscribing {
                    {"..."}
                } else if channel.is_subscribed {
                    {"Subscribed"}
                } else {
                    {"Subscribe"}
                }
            </button>
        </div>
    }
}

fn render_content(
    videos: &UseStateHandle<Vec<Video>>,
    channels: &UseStateHandle<Vec<Channel>>,
    loading: bool,
    showing_search: bool,
    subscribing_channel: &UseStateHandle<Option<String>>,
    on_subscribe: &Callback<String>,
    on_unsubscribe: &Callback<String>,
    on_video_click: &Callback<String>,
    on_channel_click: &Callback<(String, String)>,
    viewing_channel: &Option<(String, String)>,
    on_back_to_search: &Callback<MouseEvent>,
    can_subscribe: bool,
) -> Html {
    let has_channels = !channels.is_empty();
    let has_videos = !videos.is_empty();

    if loading && !has_videos && !has_channels {
        return html! {
            <div class="loading-state">
                <div class="loading-spinner"></div>
                <p>{"Loading..."}</p>
            </div>
        };
    }

    if !has_videos && !has_channels {
        return html! {
            <div class="empty-state">
                <p>{"No results found"}</p>
            </div>
        };
    }

    let channel_cards: Vec<Html> = (*channels).iter().map(|channel| {
        let channel_id = channel.id.clone();
        let channel_title = channel.title.clone();
        let is_subscribed = channel.is_subscribed;
        let is_subscribing = (*subscribing_channel).as_ref() == Some(&channel.id);
        let on_sub = on_subscribe.clone();
        let on_unsub = on_unsubscribe.clone();
        let on_ch_click = on_channel_click.clone();
        let channel_id_for_callback = channel_id.clone();
        let channel_id_for_click = channel_id.clone();
        render_channel_card(
            channel,
            is_subscribing,
            if is_subscribed {
                Callback::from(move |_: MouseEvent| {
                    on_unsub.emit(channel_id_for_callback.clone());
                })
            } else {
                Callback::from(move |_: MouseEvent| {
                    on_sub.emit(channel_id.clone());
                })
            },
            Callback::from(move |_: MouseEvent| {
                on_ch_click.emit((channel_id_for_click.clone(), channel_title.clone()));
            }),
        )
    }).collect();

    let video_cards: Vec<Html> = (*videos).iter().map(|video| {
        let video_id = video.id.clone();
        let channel_id = video.channel_id.clone();
        let on_click = on_video_click.clone();
        let on_unsub = on_unsubscribe.clone();
        let is_unsubscribing = (*subscribing_channel).as_ref() == Some(&video.channel_id);
        render_video_card(
            video,
            Callback::from(move |_: MouseEvent| {
                on_click.emit(video_id.clone());
            }),
            can_subscribe,
            is_unsubscribing,
            Callback::from(move |e: MouseEvent| {
                e.stop_propagation();
                on_unsub.emit(channel_id.clone());
            }),
        )
    }).collect();

    let is_viewing_channel = viewing_channel.is_some();

    html! {
        <div class="video-feed">
            // Back button when viewing a channel
            if let Some((_, channel_name)) = viewing_channel {
                <div class="channel-view-header">
                    <button class="back-to-search-btn" onclick={on_back_to_search}>
                        {"← Back to search"}
                    </button>
                    <h2 class="viewing-channel-name">{channel_name}</h2>
                </div>
            }

            // Channels section (only in search results, not when viewing a channel)
            if has_channels && showing_search && !is_viewing_channel {
                <div class="channels-section">
                    <div class="section-header">
                        <h3>{"Channels"}</h3>
                        <span class="result-count">{format!("{} channels", channels.len())}</span>
                    </div>
                    <div class="channel-list">
                        { for channel_cards.into_iter() }
                    </div>
                </div>
            }

            // Videos section
            if has_videos {
                <div class="videos-section">
                    <div class="section-header">
                        <h3>
                            if is_viewing_channel {
                                {"Channel Videos"}
                            } else if showing_search {
                                {"Videos"}
                            } else {
                                {"Recent from Subscriptions"}
                            }
                        </h3>
                        <span class="result-count">{format!("{} videos", videos.len())}</span>
                    </div>
                    <div class="video-list">
                        { for video_cards.into_iter() }
                    </div>
                </div>
            }

            <p class="intentional-note">
                if is_viewing_channel {
                    {"Showing recent uploads • No related videos • No autoplay"}
                } else if showing_search {
                    {"Limited results • No related videos • No autoplay"}
                } else {
                    {"Chronological order • No recommendations • No infinite scroll"}
                }
            </p>
        </div>
    }
}

fn format_time_ago(published_at: &str) -> String {
    // Parse ISO 8601 date and calculate time ago
    // For simplicity, just show the date if parsing fails
    if published_at.is_empty() {
        return String::new();
    }

    // Try to parse the date (format: 2024-01-15T12:00:00Z)
    if let Some(date_part) = published_at.split('T').next() {
        let parts: Vec<&str> = date_part.split('-').collect();
        if parts.len() == 3 {
            return format!("{}/{}/{}", parts[1], parts[2], parts[0]);
        }
    }

    published_at.to_string()
}

fn get_styles() -> &'static str {
    r#"
        /* Media Hub Container - unified for all platforms */
        .media-hub-container {
            background: rgba(30, 30, 30, 0.7);
            border: 1px solid rgba(255, 255, 255, 0.1);
            border-radius: 16px;
            padding: 1.5rem;
            backdrop-filter: blur(10px);
        }
        .media-hub-header {
            display: flex;
            align-items: center;
            gap: 12px;
            margin-bottom: 0.5rem;
        }
        .media-description {
            color: #888;
            font-size: 0.9rem;
            margin: 0 0 1.5rem 0;
            line-height: 1.4;
        }
        /* Platform badges */
        .platform-badge {
            padding: 4px 12px;
            border-radius: 12px;
            font-size: 0.8rem;
            font-weight: 500;
        }
        .platform-badge.youtube {
            background: rgba(255, 0, 0, 0.15);
            color: #FF6B6B;
        }
        .platform-badge.tiktok {
            background: rgba(0, 0, 0, 0.7);
            color: #69C9D0;
        }
        .platform-badge.instagram {
            background: linear-gradient(45deg, rgba(131, 58, 180, 0.2), rgba(253, 29, 29, 0.2));
            color: #E1306C;
        }
        .platform-badge.twitter {
            background: rgba(29, 155, 240, 0.2);
            color: #1D9BF0;
        }
        .platform-badge.reddit {
            background: rgba(255, 69, 0, 0.2);
            color: #FF4500;
        }
        .platform-badge.spotify {
            background: rgba(30, 215, 96, 0.2);
            color: #1ED760;
        }
        .platform-badge.rumble {
            background: rgba(133, 195, 30, 0.2);
            color: #85C31E;
        }
        .platform-badge.streamable {
            background: rgba(10, 139, 210, 0.2);
            color: #0A8BD2;
        }
        .platform-badge.bluesky {
            background: rgba(0, 133, 255, 0.2);
            color: #0085FF;
        }
        /* Video embed aspect ratios */
        .video-embed-wrapper.youtube-aspect {
            padding-bottom: 56.25%; /* 16:9 aspect ratio */
        }
        .video-embed-wrapper.vertical-aspect {
            max-width: 350px;
            margin: 0 auto;
            padding-bottom: min(177.78%, 622px); /* 9:16 aspect ratio, capped */
        }
        .video-embed-wrapper.twitter-aspect {
            position: relative;
            max-width: 550px;
            margin: 0 auto;
            height: 80vh;
            max-height: 800px;
            padding-bottom: 0;
        }
        .video-embed.twitter-embed {
            position: absolute;
            top: 0;
            left: 0;
            width: 100%;
            height: 100%;
            border: none;
            border-radius: 12px;
        }
        .video-embed-wrapper.reddit-aspect {
            position: relative;
            max-width: 640px;
            margin: 0 auto;
            height: 80vh;
            max-height: 800px;
            padding-bottom: 0;
        }
        .video-embed.reddit-embed {
            position: absolute;
            top: 0;
            left: 0;
            width: 100%;
            height: 100%;
            border: none;
            border-radius: 12px;
        }
        .video-embed-wrapper.spotify-aspect {
            position: relative;
            max-width: 400px;
            margin: 0 auto;
            height: 352px;
            padding-bottom: 0;
        }
        .video-embed.spotify-embed {
            width: 100%;
            height: 100%;
            border: none;
            border-radius: 12px;
        }
        .video-embed-wrapper.bluesky-aspect {
            position: relative;
            max-width: 550px;
            margin: 0 auto;
            height: 70vh;
            max-height: 700px;
            padding-bottom: 0;
        }
        .video-embed.bluesky-embed {
            position: absolute;
            top: 0;
            left: 0;
            width: 100%;
            height: 100%;
            border: none;
            border-radius: 12px;
        }
        /* Legacy container for compatibility */
        .youtube-hub-container {
            background: rgba(30, 30, 30, 0.7);
            border: 1px solid rgba(255, 0, 0, 0.1);
            border-radius: 16px;
            padding: 1.5rem;
            backdrop-filter: blur(10px);
        }
        .youtube-not-connected-banner {
            display: flex;
            align-items: center;
            gap: 12px;
            padding: 1rem;
            background: rgba(255, 0, 0, 0.05);
            border: 1px solid rgba(255, 0, 0, 0.15);
            border-radius: 12px;
        }
        .banner-text {
            flex: 1;
            display: flex;
            flex-direction: column;
        }
        .banner-title {
            color: #fff;
            font-weight: 500;
        }
        .banner-subtitle {
            color: #999;
            font-size: 0.85rem;
        }
        .connect-button-small {
            padding: 8px 16px;
            background: #FF0000;
            color: white;
            border-radius: 8px;
            text-decoration: none;
            font-size: 0.9rem;
            transition: all 0.2s;
        }
        .connect-button-small:hover {
            background: #CC0000;
        }
        .youtube-header {
            display: flex;
            align-items: center;
            gap: 12px;
            margin-bottom: 1.5rem;
        }
        .header-title {
            color: #fff;
            font-size: 1.2rem;
            font-weight: 500;
        }
        .connected-badge {
            margin-left: auto;
            padding: 4px 12px;
            background: rgba(105, 240, 174, 0.15);
            color: #69f0ae;
            border-radius: 12px;
            font-size: 0.8rem;
        }
        .connected-badge.youtube-badge {
            background: rgba(255, 0, 0, 0.15);
            color: #FF6B6B;
        }
        .search-form {
            display: flex;
            gap: 8px;
            margin-bottom: 1.5rem;
        }
        .search-input-wrapper {
            position: relative;
            flex: 1;
            display: flex;
        }
        .search-input {
            flex: 1;
            padding: 12px 40px 12px 16px;
            background: rgba(0, 0, 0, 0.3);
            border: 1px solid rgba(255, 255, 255, 0.1);
            border-radius: 24px;
            color: #fff;
            font-size: 1rem;
            outline: none;
            transition: border-color 0.2s;
        }
        .search-input:focus {
            border-color: rgba(255, 0, 0, 0.4);
        }
        .search-input::placeholder {
            color: #666;
        }
        .search-input:disabled {
            opacity: 0.6;
        }
        .clear-button {
            position: absolute;
            right: 12px;
            top: 50%;
            transform: translateY(-50%);
            background: rgba(255, 255, 255, 0.1);
            border: none;
            color: #999;
            font-size: 1.2rem;
            cursor: pointer;
            padding: 4px 10px;
            border-radius: 50%;
            line-height: 1;
        }
        .clear-button:hover {
            background: rgba(255, 255, 255, 0.2);
            color: #fff;
        }
        .search-button {
            padding: 12px 20px;
            background: rgba(255, 0, 0, 0.15);
            border: 1px solid rgba(255, 0, 0, 0.3);
            border-radius: 24px;
            color: #FF6B6B;
            cursor: pointer;
            transition: all 0.2s;
            min-width: 80px;
        }
        .search-button:hover:not(:disabled) {
            background: rgba(255, 0, 0, 0.25);
        }
        .search-button:disabled {
            opacity: 0.6;
            cursor: not-allowed;
        }
        .error-message {
            background: rgba(255, 0, 0, 0.1);
            border: 1px solid rgba(255, 0, 0, 0.3);
            color: #ff6b6b;
            padding: 12px;
            border-radius: 8px;
            margin-bottom: 1rem;
        }
        .success-message {
            background: rgba(105, 240, 174, 0.1);
            border: 1px solid rgba(105, 240, 174, 0.3);
            color: #69f0ae;
            padding: 12px;
            border-radius: 8px;
            margin-bottom: 1rem;
        }
        .loading-state, .empty-state {
            text-align: center;
            padding: 3rem;
            color: #999;
        }
        .loading-spinner {
            width: 40px;
            height: 40px;
            border: 3px solid rgba(255, 0, 0, 0.1);
            border-top-color: #ff0000;
            border-radius: 50%;
            animation: spin 1s linear infinite;
            margin: 0 auto 1rem;
        }
        @keyframes spin {
            to { transform: rotate(360deg); }
        }
        .section-header {
            display: flex;
            align-items: center;
            justify-content: space-between;
            margin-bottom: 1rem;
        }
        .section-header h3 {
            color: #fff;
            font-size: 1rem;
            font-weight: 500;
            margin: 0;
        }
        .result-count {
            color: #666;
            font-size: 0.85rem;
        }
        .video-list {
            display: flex;
            flex-direction: column;
            gap: 1rem;
        }
        .video-card {
            display: flex;
            gap: 12px;
            padding: 12px;
            background: rgba(0, 0, 0, 0.2);
            border: 1px solid rgba(255, 255, 255, 0.05);
            border-radius: 12px;
            cursor: pointer;
            transition: all 0.2s;
        }
        .video-card:hover {
            background: rgba(0, 0, 0, 0.3);
            border-color: rgba(255, 0, 0, 0.2);
        }
        .video-thumbnail {
            position: relative;
            width: 160px;
            min-width: 160px;
            height: 90px;
            background: rgba(0, 0, 0, 0.4);
            border-radius: 8px;
            overflow: hidden;
            display: flex;
            align-items: center;
            justify-content: center;
        }
        .video-thumbnail img {
            width: 100%;
            height: 100%;
            object-fit: cover;
        }
        .thumbnail-placeholder {
            color: #666;
            font-size: 2rem;
        }
        .video-duration {
            position: absolute;
            bottom: 4px;
            right: 4px;
            padding: 2px 6px;
            background: rgba(0, 0, 0, 0.8);
            color: #fff;
            font-size: 0.75rem;
            border-radius: 4px;
        }
        .video-info {
            flex: 1;
            min-width: 0;
            display: flex;
            flex-direction: column;
            justify-content: center;
        }
        .video-title {
            color: #fff;
            font-size: 0.95rem;
            font-weight: 500;
            margin: 0 0 8px 0;
            line-height: 1.3;
            overflow: hidden;
            text-overflow: ellipsis;
            display: -webkit-box;
            -webkit-line-clamp: 2;
            -webkit-box-orient: vertical;
        }
        .video-meta {
            display: flex;
            align-items: center;
            gap: 6px;
            flex-wrap: wrap;
            color: #999;
            font-size: 0.8rem;
        }
        .channel-name {
            color: #aaa;
        }
        .meta-separator {
            color: #666;
        }
        .intentional-note {
            text-align: center;
            color: #666;
            font-size: 0.85rem;
            font-style: italic;
            margin-top: 1.5rem;
            padding-top: 1rem;
            border-top: 1px solid rgba(255, 255, 255, 0.05);
        }
        /* Video Player */
        .video-player-container {
            display: flex;
            flex-direction: column;
            gap: 1rem;
        }
        .video-player-header {
            display: flex;
            align-items: center;
        }
        .back-button {
            background: rgba(255, 255, 255, 0.1);
            border: none;
            color: #fff;
            padding: 8px 16px;
            border-radius: 8px;
            cursor: pointer;
            font-size: 0.9rem;
            transition: all 0.2s;
        }
        .back-button:hover {
            background: rgba(255, 255, 255, 0.2);
        }
        .reload-button {
            margin-left: auto;
            background: rgba(225, 48, 108, 0.15);
            border: 1px solid rgba(225, 48, 108, 0.3);
            color: #E1306C;
            padding: 8px 16px;
            border-radius: 8px;
            cursor: pointer;
            font-size: 0.9rem;
            transition: all 0.2s;
        }
        .reload-button:hover {
            background: rgba(225, 48, 108, 0.25);
        }
        /* Platform Disclaimer */
        .platform-disclaimer {
            display: flex;
            align-items: flex-start;
            gap: 8px;
            padding: 12px;
            border-radius: 8px;
            font-size: 0.85rem;
            line-height: 1.4;
            margin-top: 1rem;
        }
        .platform-disclaimer.instagram-disclaimer {
            background: rgba(225, 48, 108, 0.08);
            border: 1px solid rgba(225, 48, 108, 0.2);
            color: #ccc;
        }
        .disclaimer-icon {
            color: #E1306C;
            font-size: 1rem;
            flex-shrink: 0;
        }
        .video-embed-wrapper {
            position: relative;
            width: 100%;
            padding-bottom: 56.25%; /* 16:9 aspect ratio */
            background: #000;
            border-radius: 12px;
            overflow: hidden;
        }
        .video-embed {
            position: absolute;
            top: 0;
            left: 0;
            width: 100%;
            height: 100%;
        }
        .video-details {
            padding: 1rem 0;
        }
        .video-player-title {
            color: #fff;
            font-size: 1.2rem;
            font-weight: 500;
            margin: 0 0 8px 0;
            line-height: 1.4;
        }
        .video-player-meta {
            display: flex;
            align-items: center;
            gap: 8px;
            color: #999;
            font-size: 0.9rem;
        }
        /* Search Type Filter */
        .search-type-filter {
            display: flex;
            gap: 8px;
            margin-bottom: 1rem;
        }
        .filter-button {
            padding: 8px 16px;
            background: rgba(255, 255, 255, 0.05);
            border: 1px solid rgba(255, 255, 255, 0.1);
            border-radius: 20px;
            color: #999;
            cursor: pointer;
            transition: all 0.2s;
            font-size: 0.9rem;
        }
        .filter-button:hover {
            background: rgba(255, 255, 255, 0.1);
            color: #fff;
        }
        .filter-button.active {
            background: rgba(255, 0, 0, 0.15);
            border-color: rgba(255, 0, 0, 0.4);
            color: #FF6B6B;
        }
        /* Channels Section */
        .channels-section {
            margin-bottom: 2rem;
        }
        .channel-list {
            display: flex;
            flex-direction: column;
            gap: 1rem;
        }
        .channel-card {
            display: flex;
            gap: 12px;
            padding: 12px;
            background: rgba(0, 0, 0, 0.2);
            border: 1px solid rgba(255, 255, 255, 0.05);
            border-radius: 12px;
            align-items: center;
        }
        .channel-clickable {
            display: flex;
            gap: 12px;
            align-items: center;
            flex: 1;
            cursor: pointer;
            transition: opacity 0.2s;
        }
        .channel-clickable:hover {
            opacity: 0.8;
        }
        .channel-view-header {
            display: flex;
            align-items: center;
            gap: 1rem;
            margin-bottom: 1rem;
            padding-bottom: 1rem;
            border-bottom: 1px solid rgba(255, 255, 255, 0.1);
        }
        .back-to-search-btn {
            background: transparent;
            border: 1px solid rgba(255, 255, 255, 0.2);
            color: #aaa;
            padding: 8px 16px;
            border-radius: 8px;
            cursor: pointer;
            font-size: 0.9rem;
            transition: all 0.2s;
            white-space: nowrap;
        }
        .back-to-search-btn:hover {
            background: rgba(255, 255, 255, 0.1);
            color: #fff;
        }
        .viewing-channel-name {
            color: #fff;
            font-size: 1.25rem;
            margin: 0;
            overflow: hidden;
            text-overflow: ellipsis;
            white-space: nowrap;
        }
        .channel-avatar {
            width: 64px;
            min-width: 64px;
            height: 64px;
            border-radius: 50%;
            overflow: hidden;
            background: rgba(0, 0, 0, 0.4);
            display: flex;
            align-items: center;
            justify-content: center;
        }
        .channel-avatar img {
            width: 100%;
            height: 100%;
            object-fit: cover;
        }
        .avatar-placeholder {
            color: #fff;
            font-size: 1.5rem;
            font-weight: 500;
        }
        .channel-info {
            flex: 1;
            min-width: 0;
        }
        .channel-title {
            color: #fff;
            font-size: 1rem;
            font-weight: 500;
            margin: 0 0 4px 0;
        }
        .channel-meta {
            color: #999;
            font-size: 0.85rem;
            margin-bottom: 4px;
        }
        .subscriber-count {
            color: #aaa;
        }
        .channel-description {
            color: #888;
            font-size: 0.8rem;
            margin: 0;
            overflow: hidden;
            text-overflow: ellipsis;
            display: -webkit-box;
            -webkit-line-clamp: 2;
            -webkit-box-orient: vertical;
        }
        .subscribe-button {
            padding: 8px 20px;
            background: #FF0000;
            border: none;
            border-radius: 20px;
            color: white;
            cursor: pointer;
            transition: all 0.2s;
            font-size: 0.9rem;
            font-weight: 500;
            white-space: nowrap;
        }
        .subscribe-button:hover:not(:disabled) {
            background: #CC0000;
        }
        .subscribe-button:disabled {
            opacity: 0.6;
            cursor: not-allowed;
        }
        .subscribe-button.subscribed {
            background: rgba(255, 255, 255, 0.1);
            color: #aaa;
            border: 1px solid rgba(255, 255, 255, 0.2);
        }
        .subscribe-button.subscribed:hover:not(:disabled) {
            background: rgba(255, 0, 0, 0.1);
            border-color: rgba(255, 0, 0, 0.3);
            color: #FF6B6B;
        }
        .videos-section {
            margin-top: 0;
        }
        @media (max-width: 480px) {
            .video-thumbnail {
                width: 120px;
                min-width: 120px;
                height: 68px;
            }
            .video-title {
                font-size: 0.9rem;
            }
            .youtube-not-connected-banner {
                flex-direction: column;
                text-align: center;
            }
            .banner-text {
                align-items: center;
            }
            .channel-card {
                flex-wrap: wrap;
            }
            .channel-info {
                flex-basis: calc(100% - 76px);
            }
            .subscribe-button {
                width: 100%;
                margin-top: 8px;
            }
            .search-type-filter {
                flex-wrap: wrap;
            }
            .video-embed-wrapper.vertical-aspect {
                max-width: 100%;
            }
        }
        /* Upgrade Modal */
        .modal-overlay {
            position: fixed;
            top: 0;
            left: 0;
            right: 0;
            bottom: 0;
            background: rgba(0, 0, 0, 0.8);
            display: flex;
            align-items: center;
            justify-content: center;
            z-index: 1000;
            padding: 1rem;
        }
        .upgrade-modal {
            background: #1a1a1a;
            border: 1px solid rgba(255, 255, 255, 0.1);
            border-radius: 16px;
            padding: 1.5rem;
            max-width: 450px;
            width: 100%;
        }
        .upgrade-modal h3 {
            color: #fff;
            margin: 0 0 1rem 0;
            font-size: 1.2rem;
        }
        .modal-description {
            color: #ccc;
            font-size: 0.95rem;
            line-height: 1.5;
            margin: 0 0 1rem 0;
        }
        .modal-warning {
            color: #f59e0b;
            font-size: 0.85rem;
            line-height: 1.4;
            margin: 0 0 1.5rem 0;
            padding: 0.75rem;
            background: rgba(245, 158, 11, 0.1);
            border-radius: 8px;
            border: 1px solid rgba(245, 158, 11, 0.2);
        }
        .modal-buttons {
            display: flex;
            flex-direction: column;
            gap: 0.75rem;
        }
        .modal-button {
            padding: 12px 20px;
            border-radius: 8px;
            cursor: pointer;
            font-size: 0.95rem;
            text-align: center;
            text-decoration: none;
            transition: all 0.2s;
            border: none;
        }
        .modal-button.primary {
            background: #FF0000;
            color: white;
            display: flex;
            flex-direction: column;
            align-items: center;
            gap: 2px;
        }
        .modal-button.primary:hover {
            background: #CC0000;
        }
        .modal-button.primary .button-subtitle {
            color: rgba(255, 255, 255, 0.7);
        }
        .modal-button.secondary-action {
            background: rgba(255, 255, 255, 0.05);
            color: #ccc;
            border: 1px solid rgba(255, 255, 255, 0.15);
            display: flex;
            flex-direction: column;
            align-items: center;
            gap: 2px;
        }
        .modal-button.secondary-action:hover {
            background: rgba(255, 255, 255, 0.1);
            color: #fff;
        }
        .button-subtitle {
            font-size: 0.75rem;
            color: #888;
            font-weight: normal;
        }
        .modal-button.secondary {
            background: rgba(255, 255, 255, 0.1);
            color: #fff;
            border: 1px solid rgba(255, 255, 255, 0.2);
        }
        .modal-button.secondary:hover {
            background: rgba(255, 255, 255, 0.15);
        }
        .modal-button.cancel {
            background: transparent;
            color: #999;
        }
        .modal-button.cancel:hover {
            color: #fff;
        }
        /* Comments Section */
        .comments-section {
            margin-top: 1.5rem;
            padding-top: 1.5rem;
            border-top: 1px solid rgba(255, 255, 255, 0.1);
        }
        .comments-header {
            color: #fff;
            font-size: 1rem;
            font-weight: 500;
            margin: 0 0 1rem 0;
        }
        .comment-form {
            display: flex;
            gap: 8px;
            margin-bottom: 1rem;
        }
        .comment-input {
            flex: 1;
            padding: 10px 14px;
            background: rgba(0, 0, 0, 0.3);
            border: 1px solid rgba(255, 255, 255, 0.1);
            border-radius: 20px;
            color: #fff;
            font-size: 0.9rem;
            outline: none;
            transition: border-color 0.2s;
        }
        .comment-input:focus {
            border-color: rgba(255, 0, 0, 0.4);
        }
        .comment-input::placeholder {
            color: #666;
        }
        .comment-submit-btn {
            padding: 10px 18px;
            background: rgba(255, 0, 0, 0.15);
            border: 1px solid rgba(255, 0, 0, 0.3);
            border-radius: 20px;
            color: #FF6B6B;
            cursor: pointer;
            transition: all 0.2s;
            font-size: 0.9rem;
        }
        .comment-submit-btn:hover:not(:disabled) {
            background: rgba(255, 0, 0, 0.25);
        }
        .comment-submit-btn:disabled {
            opacity: 0.5;
            cursor: not-allowed;
        }
        .comment-error {
            background: rgba(255, 0, 0, 0.1);
            border: 1px solid rgba(255, 0, 0, 0.3);
            color: #ff6b6b;
            padding: 8px 12px;
            border-radius: 8px;
            margin-bottom: 1rem;
            font-size: 0.85rem;
        }
        .comments-loading, .no-comments {
            text-align: center;
            color: #666;
            font-size: 0.9rem;
            padding: 2rem 0;
        }
        .comments-list {
            display: flex;
            flex-direction: column;
            gap: 1rem;
            max-height: 400px;
            overflow-y: auto;
        }
        .comment-item {
            display: flex;
            gap: 12px;
            padding: 12px;
            background: rgba(0, 0, 0, 0.2);
            border-radius: 12px;
        }
        .comment-avatar {
            width: 40px;
            height: 40px;
            border-radius: 50%;
            object-fit: cover;
            flex-shrink: 0;
        }
        .comment-content {
            flex: 1;
            min-width: 0;
        }
        .comment-header {
            display: flex;
            align-items: center;
            gap: 8px;
            margin-bottom: 4px;
        }
        .comment-author {
            color: #fff;
            font-weight: 500;
            font-size: 0.9rem;
        }
        .comment-time {
            color: #666;
            font-size: 0.8rem;
        }
        .comment-text {
            color: #ccc;
            font-size: 0.9rem;
            line-height: 1.4;
            margin: 0 0 8px 0;
            word-wrap: break-word;
        }
        .comment-actions {
            display: flex;
            gap: 16px;
            color: #666;
            font-size: 0.8rem;
        }
        .comment-likes {
            display: flex;
            align-items: center;
            gap: 4px;
        }
        .comment-replies {
            color: #69C9D0;
        }
        .comments-upgrade-prompt {
            text-align: center;
            padding: 2rem 1rem;
            background: rgba(0, 0, 0, 0.2);
            border-radius: 12px;
        }
        .comments-upgrade-prompt p {
            color: #999;
            margin: 0 0 1rem 0;
            font-size: 0.9rem;
        }
        .upgrade-comments-btn {
            padding: 10px 20px;
            background: rgba(255, 0, 0, 0.15);
            border: 1px solid rgba(255, 0, 0, 0.3);
            border-radius: 20px;
            color: #FF6B6B;
            cursor: pointer;
            transition: all 0.2s;
            font-size: 0.9rem;
        }
        .upgrade-comments-btn:hover {
            background: rgba(255, 0, 0, 0.25);
        }
        /* Like Button */
        .video-actions {
            margin-top: 12px;
        }
        .like-button {
            padding: 8px 16px;
            background: rgba(255, 255, 255, 0.05);
            border: 1px solid rgba(255, 255, 255, 0.1);
            border-radius: 20px;
            color: #ccc;
            cursor: pointer;
            transition: all 0.2s;
            font-size: 0.9rem;
        }
        .like-button:hover:not(:disabled) {
            background: rgba(255, 255, 255, 0.1);
            color: #fff;
        }
        .like-button:disabled {
            opacity: 0.5;
            cursor: not-allowed;
        }
        .like-button.liked {
            background: rgba(255, 0, 0, 0.15);
            border-color: rgba(255, 0, 0, 0.3);
            color: #FF6B6B;
        }
        .like-button.liked:hover:not(:disabled) {
            background: rgba(255, 0, 0, 0.25);
        }
        /* Unsubscribe hover button */
        .channel-name-wrapper {
            position: relative;
            display: inline-flex;
            align-items: center;
            gap: 8px;
        }
        .unsubscribe-hover-btn {
            display: none;
            padding: 2px 8px;
            background: rgba(255, 0, 0, 0.15);
            border: 1px solid rgba(255, 0, 0, 0.3);
            border-radius: 12px;
            color: #FF6B6B;
            cursor: pointer;
            font-size: 0.75rem;
            white-space: nowrap;
            transition: all 0.2s;
        }
        .channel-name-wrapper:hover .unsubscribe-hover-btn {
            display: inline-block;
        }
        .unsubscribe-hover-btn:hover:not(:disabled) {
            background: rgba(255, 0, 0, 0.25);
        }
        .unsubscribe-hover-btn:disabled {
            opacity: 0.5;
            cursor: not-allowed;
        }
        /* Link Preview Cards (for platforms that block embedding) */
        .link-preview-card {
            background: rgba(40, 40, 40, 0.95);
            border-radius: 16px;
            padding: 2rem;
            max-width: 500px;
            margin: 2rem auto;
            text-align: center;
            border: 1px solid rgba(255, 255, 255, 0.1);
        }
        .link-preview-card.reddit-card {
            border-color: rgba(255, 69, 0, 0.3);
        }
        .link-preview-card.rumble-card {
            border-color: rgba(133, 195, 30, 0.3);
        }
        .link-preview-card.bluesky-card {
            border-color: rgba(0, 133, 255, 0.3);
        }
        .link-preview-subreddit {
            color: #FF4500;
            font-size: 0.9rem;
            font-weight: 500;
            margin-bottom: 0.5rem;
        }
        .link-preview-title {
            color: #fff;
            font-size: 1.2rem;
            font-weight: 600;
            margin: 0 0 1rem 0;
            line-height: 1.4;
        }
        .link-preview-author {
            color: #888;
            font-size: 0.85rem;
            margin: 0 0 1.5rem 0;
        }
        .link-preview-handle {
            color: #0085FF;
            font-size: 0.9rem;
            font-weight: 500;
            margin-bottom: 1.5rem;
        }
        .open-external-button {
            display: inline-block;
            padding: 12px 24px;
            border-radius: 8px;
            font-weight: 500;
            text-decoration: none;
            transition: all 0.2s;
        }
        .open-external-button.reddit-button {
            background: rgba(255, 69, 0, 0.2);
            color: #FF4500;
            border: 1px solid rgba(255, 69, 0, 0.4);
        }
        .open-external-button.reddit-button:hover {
            background: rgba(255, 69, 0, 0.3);
        }
        .open-external-button.rumble-button {
            background: rgba(133, 195, 30, 0.2);
            color: #85C31E;
            border: 1px solid rgba(133, 195, 30, 0.4);
        }
        .open-external-button.rumble-button:hover {
            background: rgba(133, 195, 30, 0.3);
        }
        .open-external-button.bluesky-button {
            background: rgba(0, 133, 255, 0.2);
            color: #0085FF;
            border: 1px solid rgba(0, 133, 255, 0.4);
        }
        .open-external-button.bluesky-button:hover {
            background: rgba(0, 133, 255, 0.3);
        }
    "#
}
