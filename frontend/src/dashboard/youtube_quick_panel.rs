use yew::prelude::*;
use wasm_bindgen_futures::spawn_local;
use serde::Deserialize;
use crate::utils::api::Api;
use crate::dashboard::media_panel::MediaItem;
use web_sys::HtmlInputElement;
use gloo_timers::callback::Timeout;
use std::rc::Rc;
use std::cell::RefCell;

const YOUTUBE_PANEL_STYLES: &str = r#"
.youtube-quick-panel {
    background: rgba(30, 30, 30, 0.8);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 12px;
    padding: 0.75rem;
    margin-top: 0.5rem;
    max-height: 300px;
    overflow-y: auto;
}
.youtube-quick-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 0.5rem;
    gap: 0.5rem;
}
.youtube-header-left {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    flex: 1;
    min-width: 0;
}
.youtube-title {
    color: #fff;
    font-size: 0.85rem;
    font-weight: 500;
    display: flex;
    align-items: center;
    gap: 0.35rem;
    flex-shrink: 0;
}
.youtube-title i {
    color: #ff0000;
}
.youtube-search-box {
    flex: 1;
    min-width: 100px;
    background: rgba(255, 255, 255, 0.06);
    border: 1px solid rgba(255, 255, 255, 0.12);
    border-radius: 6px;
    padding: 0.35rem 0.5rem;
    color: #fff;
    font-size: 0.8rem;
    outline: none;
}
.youtube-search-box:focus {
    border-color: rgba(255, 0, 0, 0.4);
    background: rgba(255, 255, 255, 0.08);
}
.youtube-search-box::placeholder {
    color: #666;
}
.youtube-quick-close {
    background: transparent;
    border: none;
    color: #666;
    cursor: pointer;
    font-size: 1rem;
    padding: 0.25rem 0.5rem;
    border-radius: 4px;
    transition: all 0.2s;
    flex-shrink: 0;
}
.youtube-quick-close:hover {
    color: #999;
    background: rgba(255, 255, 255, 0.05);
}
.youtube-video-grid {
    display: grid;
    grid-template-columns: repeat(2, 1fr);
    gap: 0.5rem;
}
@media (max-width: 500px) {
    .youtube-video-grid {
        grid-template-columns: 1fr;
    }
}
.youtube-video-card {
    display: flex;
    gap: 0.5rem;
    padding: 0.5rem;
    background: rgba(255, 255, 255, 0.03);
    border-radius: 8px;
    cursor: pointer;
    transition: background 0.2s;
    text-decoration: none;
}
.youtube-video-card:hover {
    background: rgba(255, 255, 255, 0.08);
}
.youtube-thumbnail {
    width: 100px;
    height: 56px;
    border-radius: 4px;
    object-fit: cover;
    flex-shrink: 0;
    background: rgba(255, 255, 255, 0.05);
}
.youtube-video-info {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 0.15rem;
}
.youtube-video-title {
    color: #fff;
    font-size: 0.8rem;
    line-height: 1.2;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
}
.youtube-video-meta {
    color: #888;
    font-size: 0.7rem;
}
.youtube-quick-loading {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    color: #888;
    font-size: 0.85rem;
    padding: 0.5rem 0;
}
.youtube-quick-loading .spinner {
    width: 16px;
    height: 16px;
    border: 2px solid rgba(255, 0, 0, 0.3);
    border-radius: 50%;
    border-top-color: #ff0000;
    animation: yt-spin 1s linear infinite;
}
@keyframes yt-spin {
    to { transform: rotate(360deg); }
}
.youtube-quick-error {
    color: #ff6b6b;
    font-size: 0.85rem;
    padding: 0.5rem 0;
}
.youtube-preview-banner {
    display: flex;
    align-items: center;
    justify-content: space-between;
    background: rgba(255, 0, 0, 0.1);
    border: 1px solid rgba(255, 0, 0, 0.2);
    border-radius: 8px;
    padding: 0.5rem 0.75rem;
    margin-bottom: 0.75rem;
}
.youtube-preview-banner span {
    color: #888;
    font-size: 0.8rem;
}
.youtube-preview-banner a {
    color: #ff6b6b;
    text-decoration: none;
    font-size: 0.8rem;
    font-weight: 500;
}
.youtube-preview-banner a:hover {
    text-decoration: underline;
}
.youtube-quick-panel.preview .youtube-video-grid {
    opacity: 0.6;
}
.youtube-quick-panel.preview .youtube-video-card {
    cursor: default;
}
.youtube-empty-state {
    color: #888;
    font-size: 0.85rem;
    text-align: center;
    padding: 1rem;
}
"#;

#[derive(Clone, PartialEq, Deserialize, Debug)]
pub struct YouTubeVideo {
    pub id: String,
    pub title: String,
    pub channel: Option<String>,
    pub thumbnail: Option<String>,
    pub duration: Option<String>,
    pub published_at: Option<String>,
    pub view_count: Option<String>,
}

#[derive(Clone, PartialEq, Deserialize, Debug)]
pub struct YouTubeVideosResponse {
    pub videos: Vec<YouTubeVideo>,
}

#[derive(Properties, Clone, PartialEq)]
pub struct YouTubeQuickPanelProps {
    pub on_close: Callback<()>,
    pub on_video_select: Callback<MediaItem>,
}

#[function_component(YouTubeQuickPanel)]
pub fn youtube_quick_panel(props: &YouTubeQuickPanelProps) -> Html {
    let connected = use_state(|| None::<bool>);
    let videos = use_state(|| Vec::<YouTubeVideo>::new());
    let loading = use_state(|| true);
    let error = use_state(|| None::<String>);
    let search_query = use_state(|| String::new());
    let search_input_ref = use_node_ref();

    // Debounce timer
    let debounce_handle: UseStateHandle<Option<Rc<RefCell<Option<Timeout>>>>> = use_state(|| None);

    // Check connection status and fetch subscription feed on mount
    {
        let connected = connected.clone();
        let videos = videos.clone();
        let loading = loading.clone();
        let error = error.clone();

        use_effect_with_deps(
            move |_| {
                spawn_local(async move {
                    // Check YouTube connection status
                    match Api::get("/api/auth/youtube/status").send().await {
                        Ok(response) => {
                            if response.ok() {
                                if let Ok(data) = response.json::<serde_json::Value>().await {
                                    let has_youtube = data["connected"].as_bool().unwrap_or(false);
                                    connected.set(Some(has_youtube));

                                    if has_youtube {
                                        // Fetch subscription feed
                                        match Api::get("/api/youtube/subscriptions").send().await {
                                            Ok(resp) => {
                                                if resp.ok() {
                                                    if let Ok(data) = resp.json::<YouTubeVideosResponse>().await {
                                                        videos.set(data.videos);
                                                    } else {
                                                        error.set(Some("Failed to parse videos".to_string()));
                                                    }
                                                } else {
                                                    if let Ok(err_data) = resp.json::<serde_json::Value>().await {
                                                        let msg = err_data["error"].as_str().unwrap_or("Failed to fetch videos");
                                                        error.set(Some(msg.to_string()));
                                                    } else {
                                                        error.set(Some("Failed to fetch videos".to_string()));
                                                    }
                                                }
                                            }
                                            Err(e) => {
                                                error.set(Some(format!("Network error: {}", e)));
                                            }
                                        }
                                    }
                                }
                            } else {
                                connected.set(Some(false));
                            }
                        }
                        Err(e) => {
                            error.set(Some(format!("Network error: {}", e)));
                            connected.set(Some(false));
                        }
                    }
                    loading.set(false);
                });
                || ()
            },
            (),
        );
    }

    // Search function
    let do_search = {
        let videos = videos.clone();
        let loading = loading.clone();
        let error = error.clone();
        let connected = connected.clone();

        Callback::from(move |query: String| {
            let videos = videos.clone();
            let loading = loading.clone();
            let error = error.clone();
            let connected = connected.clone();

            // Only search if connected
            if !matches!(*connected, Some(true)) {
                return;
            }

            loading.set(true);
            error.set(None);

            spawn_local(async move {
                let endpoint = if query.trim().is_empty() {
                    "/api/youtube/subscriptions".to_string()
                } else {
                    format!("/api/youtube/search?q={}", urlencoding::encode(&query))
                };

                match Api::get(&endpoint).send().await {
                    Ok(resp) => {
                        if resp.ok() {
                            if let Ok(data) = resp.json::<YouTubeVideosResponse>().await {
                                videos.set(data.videos);
                            } else {
                                error.set(Some("Failed to parse videos".to_string()));
                            }
                        } else {
                            if let Ok(err_data) = resp.json::<serde_json::Value>().await {
                                let msg = err_data["error"].as_str().unwrap_or("Search failed");
                                error.set(Some(msg.to_string()));
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

    // Handle search input with debounce
    let on_search_input = {
        let search_query = search_query.clone();
        let debounce_handle = debounce_handle.clone();
        let do_search = do_search.clone();

        Callback::from(move |e: InputEvent| {
            let input: HtmlInputElement = e.target_unchecked_into();
            let value = input.value();
            search_query.set(value.clone());

            // Cancel previous debounce timer
            if let Some(handle_rc) = (*debounce_handle).as_ref() {
                if let Some(handle) = handle_rc.borrow_mut().take() {
                    drop(handle);
                }
            }

            // Set new debounce timer
            let do_search = do_search.clone();
            let handle_rc = Rc::new(RefCell::new(None::<Timeout>));
            let handle_rc_inner = handle_rc.clone();
            let timeout = Timeout::new(300, move || {
                do_search.emit(value);
                *handle_rc_inner.borrow_mut() = None;
            });
            *handle_rc.borrow_mut() = Some(timeout);
            debounce_handle.set(Some(handle_rc));
        })
    };

    let on_close = {
        let on_close = props.on_close.clone();
        Callback::from(move |_: MouseEvent| {
            on_close.emit(());
        })
    };

    // Preview mode when not connected
    let is_preview = matches!(*connected, Some(false));

    // Mock videos for preview
    let preview_videos = vec![
        YouTubeVideo {
            id: "preview1".to_string(),
            title: "How to Get Started with...".to_string(),
            channel: Some("Tech Channel".to_string()),
            thumbnail: None,
            duration: Some("12:34".to_string()),
            published_at: None,
            view_count: Some("1.2M views".to_string()),
        },
        YouTubeVideo {
            id: "preview2".to_string(),
            title: "Top 10 Tips for Better...".to_string(),
            channel: Some("Tutorial Hub".to_string()),
            thumbnail: None,
            duration: Some("8:45".to_string()),
            published_at: None,
            view_count: Some("890K views".to_string()),
        },
        YouTubeVideo {
            id: "preview3".to_string(),
            title: "The Complete Guide to...".to_string(),
            channel: Some("Learn Daily".to_string()),
            thumbnail: None,
            duration: Some("25:10".to_string()),
            published_at: None,
            view_count: Some("2.3M views".to_string()),
        },
        YouTubeVideo {
            id: "preview4".to_string(),
            title: "Why You Should Try...".to_string(),
            channel: Some("Lifestyle Pro".to_string()),
            thumbnail: None,
            duration: Some("6:22".to_string()),
            published_at: None,
            view_count: Some("456K views".to_string()),
        },
    ];

    let display_videos = if is_preview {
        preview_videos
    } else {
        (*videos).clone()
    };

    html! {
        <>
            <style>{YOUTUBE_PANEL_STYLES}</style>
            <div class={classes!("youtube-quick-panel", is_preview.then(|| "preview"))}>
                {
                    if *loading && (*connected).is_none() {
                        html! {
                            <div class="youtube-quick-loading">
                                <div class="spinner"></div>
                                <span>{"Loading YouTube..."}</span>
                            </div>
                        }
                    } else {
                        html! {
                            <>
                                // Preview banner when not connected
                                {
                                    if is_preview {
                                        html! {
                                            <div class="youtube-preview-banner">
                                                <span>{"Connect YouTube to see your subscriptions"}</span>
                                                <a href="/?settings=capabilities">{"Connect"}</a>
                                            </div>
                                        }
                                    } else {
                                        html! {}
                                    }
                                }
                                <div class="youtube-quick-header">
                                    <div class="youtube-header-left">
                                        <span class="youtube-title">
                                            <i class="fab fa-youtube"></i>
                                            {"YouTube"}
                                        </span>
                                        {
                                            if !is_preview {
                                                html! {
                                                    <input
                                                        type="text"
                                                        class="youtube-search-box"
                                                        ref={search_input_ref}
                                                        placeholder="Search videos..."
                                                        value={(*search_query).clone()}
                                                        oninput={on_search_input}
                                                    />
                                                }
                                            } else {
                                                html! {}
                                            }
                                        }
                                    </div>
                                    <button class="youtube-quick-close" onclick={on_close}>{"x"}</button>
                                </div>
                                {
                                    if let Some(err) = (*error).as_ref() {
                                        html! { <div class="youtube-quick-error">{err}</div> }
                                    } else if *loading {
                                        html! {
                                            <div class="youtube-quick-loading">
                                                <div class="spinner"></div>
                                                <span>{"Searching..."}</span>
                                            </div>
                                        }
                                    } else if display_videos.is_empty() {
                                        html! {
                                            <div class="youtube-empty-state">
                                                {"No videos found"}
                                            </div>
                                        }
                                    } else {
                                        html! {
                                            <div class="youtube-video-grid">
                                                {
                                                    display_videos.iter().map(|video| {
                                                        let thumbnail_url = video.thumbnail.clone()
                                                            .unwrap_or_else(|| format!("https://img.youtube.com/vi/{}/mqdefault.jpg", video.id));
                                                        let meta = match (&video.channel, &video.duration) {
                                                            (Some(ch), Some(dur)) => format!("{} - {}", ch, dur),
                                                            (Some(ch), None) => ch.clone(),
                                                            (None, Some(dur)) => dur.clone(),
                                                            (None, None) => String::new(),
                                                        };

                                                        if is_preview {
                                                            html! {
                                                                <div class="youtube-video-card">
                                                                    <div
                                                                        class="youtube-thumbnail"
                                                                        style="background: linear-gradient(135deg, #333, #222);"
                                                                    />
                                                                    <div class="youtube-video-info">
                                                                        <div class="youtube-video-title">{&video.title}</div>
                                                                        <div class="youtube-video-meta">{meta}</div>
                                                                    </div>
                                                                </div>
                                                            }
                                                        } else {
                                                            let on_click = {
                                                                let on_video_select = props.on_video_select.clone();
                                                                let media_item = MediaItem {
                                                                    platform: "youtube".to_string(),
                                                                    video_id: video.id.clone(),
                                                                    title: video.title.clone(),
                                                                    thumbnail: thumbnail_url.clone(),
                                                                    duration: video.duration.clone(),
                                                                    channel: video.channel.clone(),
                                                                    original_url: None,
                                                                };
                                                                Callback::from(move |_: MouseEvent| {
                                                                    on_video_select.emit(media_item.clone());
                                                                })
                                                            };
                                                            html! {
                                                                <div
                                                                    class="youtube-video-card"
                                                                    onclick={on_click}
                                                                >
                                                                    <img
                                                                        class="youtube-thumbnail"
                                                                        src={thumbnail_url}
                                                                        alt={video.title.clone()}
                                                                        loading="lazy"
                                                                    />
                                                                    <div class="youtube-video-info">
                                                                        <div class="youtube-video-title">{&video.title}</div>
                                                                        <div class="youtube-video-meta">{meta}</div>
                                                                    </div>
                                                                </div>
                                                            }
                                                        }
                                                    }).collect::<Html>()
                                                }
                                            </div>
                                        }
                                    }
                                }
                            </>
                        }
                    }
                }
            </div>
        </>
    }
}
