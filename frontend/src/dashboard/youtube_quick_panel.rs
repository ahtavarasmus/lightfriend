use crate::dashboard::media_panel::MediaItem;
use crate::utils::api::Api;
use serde::Deserialize;
use wasm_bindgen_futures::spawn_local;
use web_sys::HtmlInputElement;
use yew::prelude::*;

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
.youtube-search-btn {
    background: rgba(255, 0, 0, 0.15);
    border: 1px solid rgba(255, 0, 0, 0.25);
    border-radius: 6px;
    color: #ff6b6b;
    cursor: pointer;
    padding: 0.3rem 0.5rem;
    font-size: 0.75rem;
    flex-shrink: 0;
    transition: all 0.2s;
    display: flex;
    align-items: center;
}
.youtube-search-btn:hover {
    background: rgba(255, 0, 0, 0.25);
    color: #ff8888;
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
/* Filter pills */
.youtube-filter-row {
    display: flex;
    gap: 0.3rem;
    margin-bottom: 0.5rem;
    flex-wrap: wrap;
}
.youtube-filter-pill {
    background: rgba(255, 255, 255, 0.06);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 14px;
    padding: 0.2rem 0.55rem;
    color: #999;
    font-size: 0.7rem;
    cursor: pointer;
    transition: all 0.2s;
}
.youtube-filter-pill:hover {
    background: rgba(255, 255, 255, 0.1);
    color: #ccc;
}
.youtube-filter-pill.active {
    background: rgba(255, 0, 0, 0.15);
    border-color: rgba(255, 0, 0, 0.3);
    color: #ff6b6b;
}
/* Channel cards */
.youtube-channel-card {
    display: flex;
    gap: 0.5rem;
    padding: 0.5rem;
    background: rgba(255, 255, 255, 0.03);
    border-radius: 8px;
    cursor: pointer;
    transition: background 0.2s;
    align-items: center;
}
.youtube-channel-card:hover {
    background: rgba(255, 255, 255, 0.08);
}
.youtube-channel-avatar {
    width: 40px;
    height: 40px;
    border-radius: 50%;
    object-fit: cover;
    flex-shrink: 0;
    background: rgba(255, 255, 255, 0.05);
}
.youtube-channel-info {
    flex: 1;
    min-width: 0;
}
.youtube-channel-name {
    color: #fff;
    font-size: 0.8rem;
    font-weight: 500;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
}
.youtube-channel-desc {
    color: #888;
    font-size: 0.7rem;
    display: -webkit-box;
    -webkit-line-clamp: 1;
    -webkit-box-orient: vertical;
    overflow: hidden;
}
.youtube-channel-subs {
    color: #666;
    font-size: 0.65rem;
}
/* Breadcrumb nav for channel view */
.youtube-breadcrumb {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    margin-bottom: 0.5rem;
}
.youtube-breadcrumb-back {
    background: transparent;
    border: none;
    color: #999;
    cursor: pointer;
    font-size: 0.85rem;
    padding: 0.15rem 0.3rem;
    border-radius: 4px;
    transition: all 0.2s;
    display: flex;
    align-items: center;
}
.youtube-breadcrumb-back:hover {
    color: #fff;
    background: rgba(255, 255, 255, 0.08);
}
.youtube-breadcrumb-label {
    color: #ccc;
    font-size: 0.8rem;
    font-weight: 500;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
}
"#;

// --- Types ---

#[derive(Clone, PartialEq, Debug)]
pub enum YtView {
    Subscriptions,
    Search,
    ChannelVideos,
}

#[derive(Clone, PartialEq, Debug)]
pub enum YtFilter {
    All,
    Videos,
    Shorts,
    Channels,
}

impl YtFilter {
    fn label(&self) -> &'static str {
        match self {
            YtFilter::All => "All",
            YtFilter::Videos => "Videos",
            YtFilter::Shorts => "Shorts",
            YtFilter::Channels => "Channels",
        }
    }

    /// Returns the YouTube API `type` param
    fn search_type(&self) -> &'static str {
        match self {
            YtFilter::Channels => "channel",
            _ => "video",
        }
    }

    /// Returns the YouTube API `videoDuration` param (for Videos/Shorts filtering)
    fn video_duration(&self) -> Option<&'static str> {
        match self {
            YtFilter::Shorts => Some("short"),
            YtFilter::Videos => Some("medium"),
            _ => None,
        }
    }
}

#[derive(Clone, PartialEq, Deserialize, Debug)]
pub struct YouTubeChannel {
    pub id: String,
    pub title: String,
    pub description: String,
    pub thumbnail: String,
    pub subscriber_count: String,
}

#[derive(Clone, PartialEq, Deserialize, Debug)]
pub struct YouTubeVideo {
    pub id: String,
    pub title: String,
    pub channel: Option<String>,
    pub channel_id: Option<String>,
    pub thumbnail: Option<String>,
    pub duration: Option<String>,
    pub published_at: Option<String>,
    pub view_count: Option<String>,
}

#[derive(Clone, PartialEq, Deserialize, Debug)]
pub struct YouTubeVideosResponse {
    pub videos: Vec<YouTubeVideo>,
}

/// Search response that can include both videos and channels
#[derive(Clone, PartialEq, Deserialize, Debug)]
pub struct YouTubeSearchResponse {
    pub videos: Vec<YouTubeVideo>,
    #[serde(default)]
    pub channels: Option<Vec<YouTubeChannel>>,
}

/// Browsing state that persists across panel mount/unmount cycles
#[derive(Clone, PartialEq, Debug)]
pub struct YtBrowseState {
    pub current_view: YtView,
    pub active_filter: YtFilter,
    pub search_query: String,
    pub videos: Vec<YouTubeVideo>,
    pub channels: Vec<YouTubeChannel>,
    /// Channel being browsed (name + id)
    pub browsing_channel: Option<(String, String)>,
    /// Cached videos/channels from the previous view (for back navigation)
    pub cached_videos: Vec<YouTubeVideo>,
    pub cached_channels: Vec<YouTubeChannel>,
    pub cached_view: Option<YtView>,
}

impl Default for YtBrowseState {
    fn default() -> Self {
        Self {
            current_view: YtView::Subscriptions,
            active_filter: YtFilter::All,
            search_query: String::new(),
            videos: Vec::new(),
            channels: Vec::new(),
            browsing_channel: None,
            cached_videos: Vec::new(),
            cached_channels: Vec::new(),
            cached_view: None,
        }
    }
}

#[derive(Properties, Clone, PartialEq)]
pub struct YouTubeQuickPanelProps {
    pub on_close: Callback<()>,
    pub on_video_select: Callback<MediaItem>,
    #[prop_or_default]
    pub initial_state: Option<YtBrowseState>,
    #[prop_or_default]
    pub on_state_change: Option<Callback<YtBrowseState>>,
}

#[function_component(YouTubeQuickPanel)]
pub fn youtube_quick_panel(props: &YouTubeQuickPanelProps) -> Html {
    let connected = use_state(|| None::<bool>);
    let loading = use_state(|| true);
    let error = use_state(|| None::<String>);
    let search_input_ref = use_node_ref();

    // Initialize from initial_state if provided
    let has_initial = props.initial_state.is_some();
    let init = props.initial_state.clone().unwrap_or_default();

    let videos = use_state(|| init.videos.clone());
    let channels: UseStateHandle<Vec<YouTubeChannel>> = use_state(|| init.channels.clone());
    let search_query = use_state(|| init.search_query.clone());
    let current_view = use_state(|| init.current_view.clone());
    let active_filter = use_state(|| init.active_filter.clone());
    let browsing_channel: UseStateHandle<Option<(String, String)>> =
        use_state(|| init.browsing_channel.clone());
    let cached_videos = use_state(|| init.cached_videos.clone());
    let cached_channels: UseStateHandle<Vec<YouTubeChannel>> =
        use_state(|| init.cached_channels.clone());
    let cached_view: UseStateHandle<Option<YtView>> = use_state(|| init.cached_view.clone());

    // Helper: emit state change to parent
    let emit_state = {
        let on_state_change = props.on_state_change.clone();
        let videos = videos.clone();
        let channels = channels.clone();
        let search_query = search_query.clone();
        let current_view = current_view.clone();
        let active_filter = active_filter.clone();
        let browsing_channel = browsing_channel.clone();
        let cached_videos = cached_videos.clone();
        let cached_channels = cached_channels.clone();
        let cached_view = cached_view.clone();
        Callback::from(move |_: ()| {
            if let Some(ref cb) = on_state_change {
                cb.emit(YtBrowseState {
                    current_view: (*current_view).clone(),
                    active_filter: (*active_filter).clone(),
                    search_query: (*search_query).clone(),
                    videos: (*videos).clone(),
                    channels: (*channels).clone(),
                    browsing_channel: (*browsing_channel).clone(),
                    cached_videos: (*cached_videos).clone(),
                    cached_channels: (*cached_channels).clone(),
                    cached_view: (*cached_view).clone(),
                });
            }
        })
    };

    // Check connection status and fetch subscription feed on mount (skip if we have initial state)
    {
        let connected = connected.clone();
        let videos = videos.clone();
        let loading = loading.clone();
        let error = error.clone();
        let has_initial = has_initial;
        let emit_state = emit_state.clone();

        use_effect_with_deps(
            move |_| {
                if has_initial && !videos.is_empty() {
                    // We have cached state - skip fetch
                    loading.set(false);
                    connected.set(Some(true));
                } else {
                    spawn_local(async move {
                        // Check YouTube connection status
                        match Api::get("/api/auth/youtube/status").send().await {
                            Ok(response) => {
                                if response.ok() {
                                    if let Ok(data) = response.json::<serde_json::Value>().await {
                                        let has_youtube =
                                            data["connected"].as_bool().unwrap_or(false);
                                        connected.set(Some(has_youtube));

                                        if has_youtube {
                                            // Fetch subscription feed
                                            match Api::get("/api/youtube/subscriptions")
                                                .send()
                                                .await
                                            {
                                                Ok(resp) => {
                                                    if resp.ok() {
                                                        if let Ok(data) = resp
                                                            .json::<YouTubeVideosResponse>()
                                                            .await
                                                        {
                                                            videos.set(data.videos);
                                                            emit_state.emit(());
                                                        } else {
                                                            error.set(Some(
                                                                "Failed to parse videos"
                                                                    .to_string(),
                                                            ));
                                                        }
                                                    } else {
                                                        if let Ok(err_data) =
                                                            resp.json::<serde_json::Value>().await
                                                        {
                                                            let msg = err_data["error"]
                                                                .as_str()
                                                                .unwrap_or(
                                                                    "Failed to fetch videos",
                                                                );
                                                            error.set(Some(msg.to_string()));
                                                        } else {
                                                            error.set(Some(
                                                                "Failed to fetch videos"
                                                                    .to_string(),
                                                            ));
                                                        }
                                                    }
                                                }
                                                Err(e) => {
                                                    error
                                                        .set(Some(format!("Network error: {}", e)));
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
                }
                || ()
            },
            (),
        );
    }

    // Search/filter function - fetches videos (and optionally channels) based on query + filter
    let do_search = {
        let videos = videos.clone();
        let channels = channels.clone();
        let loading = loading.clone();
        let error = error.clone();
        let connected = connected.clone();
        let current_view = current_view.clone();
        let emit_state = emit_state.clone();

        Callback::from(move |(query, filter): (String, YtFilter)| {
            let videos = videos.clone();
            let channels = channels.clone();
            let loading = loading.clone();
            let error = error.clone();
            let connected = connected.clone();
            let current_view = current_view.clone();
            let emit_state = emit_state.clone();

            if !matches!(*connected, Some(true)) {
                return;
            }

            loading.set(true);
            error.set(None);

            spawn_local(async move {
                let endpoint = if query.trim().is_empty() {
                    // Subscription feed with optional duration filter
                    let mut url = "/api/youtube/subscriptions".to_string();
                    if let Some(dur) = filter.video_duration() {
                        url = format!("{}?video_duration={}", url, dur);
                    }
                    current_view.set(YtView::Subscriptions);
                    url
                } else {
                    // Search with type + duration
                    let mut url = format!(
                        "/api/youtube/search?q={}&type={}",
                        urlencoding::encode(&query),
                        filter.search_type(),
                    );
                    if let Some(dur) = filter.video_duration() {
                        url = format!("{}&video_duration={}", url, dur);
                    }
                    // For "All" filter, request both videos and channels
                    if matches!(filter, YtFilter::All) {
                        url = format!(
                            "/api/youtube/search?q={}&type=video,channel",
                            urlencoding::encode(&query),
                        );
                    }
                    current_view.set(YtView::Search);
                    url
                };

                match Api::get(&endpoint).send().await {
                    Ok(resp) => {
                        if resp.ok() {
                            // Try parsing as search response (with channels) first
                            if let Ok(data) = resp.json::<YouTubeSearchResponse>().await {
                                videos.set(data.videos);
                                channels.set(data.channels.unwrap_or_default());
                            } else {
                                error.set(Some("Failed to parse response".to_string()));
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
                emit_state.emit(());
            });
        })
    };

    // Handle search input - just update state, no auto-search
    let on_search_input = {
        let search_query = search_query.clone();
        Callback::from(move |e: InputEvent| {
            let input: HtmlInputElement = e.target_unchecked_into();
            search_query.set(input.value());
        })
    };

    // Trigger search on button click or Enter key
    let on_search_submit = {
        let do_search = do_search.clone();
        let search_query = search_query.clone();
        let active_filter = active_filter.clone();

        Callback::from(move |_: ()| {
            let query = (*search_query).clone();
            let filter = (*active_filter).clone();
            do_search.emit((query, filter));
        })
    };

    let on_search_keydown = {
        let on_search_submit = on_search_submit.clone();
        Callback::from(move |e: KeyboardEvent| {
            if e.key() == "Enter" {
                e.prevent_default();
                on_search_submit.emit(());
            }
        })
    };

    // Filter pill click handler
    let on_filter_click = {
        let active_filter = active_filter.clone();
        let do_search = do_search.clone();
        let search_query = search_query.clone();
        let emit_state = emit_state.clone();

        Callback::from(move |filter: YtFilter| {
            active_filter.set(filter.clone());
            let query = (*search_query).clone();
            do_search.emit((query, filter));
            emit_state.emit(());
        })
    };

    // Channel click handler - browse channel's videos
    let on_channel_click = {
        let videos = videos.clone();
        let channels = channels.clone();
        let loading = loading.clone();
        let error = error.clone();
        let current_view = current_view.clone();
        let browsing_channel = browsing_channel.clone();
        let cached_videos = cached_videos.clone();
        let cached_channels = cached_channels.clone();
        let cached_view = cached_view.clone();
        let emit_state = emit_state.clone();

        Callback::from(move |(channel_name, channel_id): (String, String)| {
            // Cache current state before navigating
            cached_videos.set((*videos).clone());
            cached_channels.set((*channels).clone());
            cached_view.set(Some((*current_view).clone()));

            browsing_channel.set(Some((channel_name, channel_id.clone())));
            current_view.set(YtView::ChannelVideos);

            let videos = videos.clone();
            let channels_inner = channels.clone();
            let loading = loading.clone();
            let error = error.clone();
            let emit_state = emit_state.clone();

            loading.set(true);
            error.set(None);

            spawn_local(async move {
                let endpoint = format!(
                    "/api/youtube/channel/{}/videos",
                    urlencoding::encode(&channel_id)
                );
                match Api::get(&endpoint).send().await {
                    Ok(resp) => {
                        if resp.ok() {
                            if let Ok(data) = resp.json::<YouTubeSearchResponse>().await {
                                videos.set(data.videos);
                                channels_inner.set(Vec::new());
                            } else {
                                error.set(Some("Failed to parse channel videos".to_string()));
                            }
                        } else {
                            if let Ok(err_data) = resp.json::<serde_json::Value>().await {
                                let msg = err_data["error"]
                                    .as_str()
                                    .unwrap_or("Failed to load channel");
                                error.set(Some(msg.to_string()));
                            }
                        }
                    }
                    Err(e) => {
                        error.set(Some(format!("Network error: {}", e)));
                    }
                }
                loading.set(false);
                emit_state.emit(());
            });
        })
    };

    // Breadcrumb back handler - restore previous view
    let on_breadcrumb_back = {
        let videos = videos.clone();
        let channels = channels.clone();
        let current_view = current_view.clone();
        let browsing_channel = browsing_channel.clone();
        let cached_videos = cached_videos.clone();
        let cached_channels = cached_channels.clone();
        let cached_view = cached_view.clone();
        let emit_state = emit_state.clone();

        Callback::from(move |_: MouseEvent| {
            // Restore cached state
            videos.set((*cached_videos).clone());
            channels.set((*cached_channels).clone());
            if let Some(ref view) = *cached_view {
                current_view.set(view.clone());
            } else {
                current_view.set(YtView::Subscriptions);
            }
            browsing_channel.set(None);
            cached_videos.set(Vec::new());
            cached_channels.set(Vec::new());
            cached_view.set(None);
            emit_state.emit(());
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
    let is_channel_view = matches!(*current_view, YtView::ChannelVideos);

    // Mock videos for preview
    let preview_videos = vec![
        YouTubeVideo {
            id: "preview1".to_string(),
            title: "How to Get Started with...".to_string(),
            channel: Some("Tech Channel".to_string()),
            channel_id: None,
            thumbnail: None,
            duration: Some("12:34".to_string()),
            published_at: None,
            view_count: Some("1.2M views".to_string()),
        },
        YouTubeVideo {
            id: "preview2".to_string(),
            title: "Top 10 Tips for Better...".to_string(),
            channel: Some("Tutorial Hub".to_string()),
            channel_id: None,
            thumbnail: None,
            duration: Some("8:45".to_string()),
            published_at: None,
            view_count: Some("890K views".to_string()),
        },
        YouTubeVideo {
            id: "preview3".to_string(),
            title: "The Complete Guide to...".to_string(),
            channel: Some("Learn Daily".to_string()),
            channel_id: None,
            thumbnail: None,
            duration: Some("25:10".to_string()),
            published_at: None,
            view_count: Some("2.3M views".to_string()),
        },
        YouTubeVideo {
            id: "preview4".to_string(),
            title: "Why You Should Try...".to_string(),
            channel: Some("Lifestyle Pro".to_string()),
            channel_id: None,
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

    let display_channels = (*channels).clone();

    // Render filter pills
    let render_filters = {
        let active_filter = active_filter.clone();
        let on_filter_click = on_filter_click.clone();

        move || -> Html {
            let filters = [
                YtFilter::All,
                YtFilter::Videos,
                YtFilter::Shorts,
                YtFilter::Channels,
            ];
            html! {
                <div class="youtube-filter-row">
                    { for filters.iter().map(|f| {
                        let is_active = *active_filter == *f;
                        let f_clone = f.clone();
                        let on_click = {
                            let on_filter_click = on_filter_click.clone();
                            Callback::from(move |_: MouseEvent| {
                                on_filter_click.emit(f_clone.clone());
                            })
                        };
                        html! {
                            <button
                                class={classes!("youtube-filter-pill", is_active.then(|| "active"))}
                                onclick={on_click}
                            >
                                {f.label()}
                            </button>
                        }
                    })}
                </div>
            }
        }
    };

    // Render channel cards
    let render_channel_cards = {
        let on_channel_click = on_channel_click.clone();

        move |channels_list: &[YouTubeChannel]| -> Html {
            html! {
                { for channels_list.iter().map(|ch| {
                    let on_click = {
                        let on_channel_click = on_channel_click.clone();
                        let name = ch.title.clone();
                        let id = ch.id.clone();
                        Callback::from(move |_: MouseEvent| {
                            on_channel_click.emit((name.clone(), id.clone()));
                        })
                    };
                    html! {
                        <div class="youtube-channel-card" onclick={on_click}>
                            <img
                                class="youtube-channel-avatar"
                                src={ch.thumbnail.clone()}
                                alt={ch.title.clone()}
                                loading="lazy"
                            />
                            <div class="youtube-channel-info">
                                <div class="youtube-channel-name">{&ch.title}</div>
                                { if !ch.description.is_empty() {
                                    html! { <div class="youtube-channel-desc">{&ch.description}</div> }
                                } else {
                                    html! {}
                                }}
                                { if !ch.subscriber_count.is_empty() {
                                    html! { <div class="youtube-channel-subs">{&ch.subscriber_count}</div> }
                                } else {
                                    html! {}
                                }}
                            </div>
                        </div>
                    }
                })}
            }
        }
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
                                // Breadcrumb when browsing a channel
                                {
                                    if is_channel_view {
                                        let channel_name = (*browsing_channel).as_ref()
                                            .map(|(name, _)| name.clone())
                                            .unwrap_or_default();
                                        html! {
                                            <div class="youtube-breadcrumb">
                                                <button class="youtube-breadcrumb-back" onclick={on_breadcrumb_back.clone()}>
                                                    <i class="fa-solid fa-arrow-left"></i>
                                                </button>
                                                <span class="youtube-breadcrumb-label">{channel_name}</span>
                                                <button class="youtube-quick-close" onclick={on_close.clone()} style="margin-left: auto;">{"x"}</button>
                                            </div>
                                        }
                                    } else {
                                        html! {
                                            <div class="youtube-quick-header">
                                                <div class="youtube-header-left">
                                                    <span class="youtube-title">
                                                        <i class="fab fa-youtube"></i>
                                                        {"YouTube"}
                                                    </span>
                                                    {
                                                        if !is_preview {
                                                            html! {
                                                                <>
                                                                    <input
                                                                        type="text"
                                                                        class="youtube-search-box"
                                                                        ref={search_input_ref}
                                                                        placeholder="Search videos..."
                                                                        value={(*search_query).clone()}
                                                                        oninput={on_search_input}
                                                                        onkeydown={on_search_keydown}
                                                                    />
                                                                    <button
                                                                        class="youtube-search-btn"
                                                                        onclick={{
                                                                            let on_search_submit = on_search_submit.clone();
                                                                            Callback::from(move |_: MouseEvent| {
                                                                                on_search_submit.emit(());
                                                                            })
                                                                        }}
                                                                    >
                                                                        <i class="fa-solid fa-magnifying-glass"></i>
                                                                    </button>
                                                                </>
                                                            }
                                                        } else {
                                                            html! {}
                                                        }
                                                    }
                                                </div>
                                                <button class="youtube-quick-close" onclick={on_close.clone()}>{"x"}</button>
                                            </div>
                                        }
                                    }
                                }
                                // Filter pills (hide when in channel view)
                                {
                                    if !is_preview && !is_channel_view {
                                        render_filters()
                                    } else {
                                        html! {}
                                    }
                                }
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
                                    } else if display_videos.is_empty() && display_channels.is_empty() {
                                        html! {
                                            <div class="youtube-empty-state">
                                                {"No results found"}
                                            </div>
                                        }
                                    } else {
                                        html! {
                                            <>
                                                // Render channel cards above video grid
                                                { if !display_channels.is_empty() {
                                                    render_channel_cards(&display_channels)
                                                } else {
                                                    html! {}
                                                }}
                                                // Video grid
                                                { if !display_videos.is_empty() {
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
                                                } else {
                                                    html! {}
                                                }}
                                            </>
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
