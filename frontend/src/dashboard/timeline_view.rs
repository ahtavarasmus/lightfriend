use yew::prelude::*;

const TIMELINE_STYLES: &str = r#"
.timeline-container {
    position: relative;
    width: 100%;
    overflow-x: auto;
    overflow-y: hidden;
    scrollbar-width: thin;
    scrollbar-color: rgba(126, 178, 255, 0.3) transparent;
    border-radius: 12px;
    margin: 0.5rem 0;
}

.timeline-container::-webkit-scrollbar {
    height: 6px;
}

.timeline-container::-webkit-scrollbar-track {
    background: transparent;
}

.timeline-container::-webkit-scrollbar-thumb {
    background: rgba(126, 178, 255, 0.3);
    border-radius: 3px;
}

.timeline-track {
    position: relative;
    height: 140px;
    min-width: 100%;
}

.timeline-background {
    position: absolute;
    top: 0;
    bottom: 0;
    border-radius: 0;
    opacity: 0.6;
}

.timeline-background:first-child {
    border-radius: 12px 0 0 12px;
}

.timeline-background:last-of-type {
    border-radius: 0 12px 12px 0;
}

.timeline-hours {
    position: absolute;
    top: 8px;
    left: 0;
    right: 0;
    height: 20px;
    display: flex;
    pointer-events: none;
}

.timeline-hour-mark {
    position: absolute;
    font-size: 0.65rem;
    color: rgba(255, 255, 255, 0.4);
    transform: translateX(-50%);
    white-space: nowrap;
}

.timeline-day-markers {
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    pointer-events: none;
}

.timeline-day-marker {
    position: absolute;
    top: 0;
    bottom: 0;
    width: 1px;
    background: rgba(255, 255, 255, 0.15);
    border-left: 1px dashed rgba(255, 255, 255, 0.2);
}

.timeline-day-label {
    position: absolute;
    top: 24px;
    font-size: 0.6rem;
    font-weight: 600;
    color: rgba(255, 255, 255, 0.45);
    white-space: nowrap;
    pointer-events: none;
    text-transform: uppercase;
    letter-spacing: 0.3px;
}

.timeline-now-marker {
    position: absolute;
    top: 24px;
    bottom: 8px;
    width: 2px;
    background: #7EB2FF;
    border-radius: 1px;
    z-index: 10;
    box-shadow: 0 0 8px rgba(126, 178, 255, 0.5);
}

.timeline-now-label {
    position: absolute;
    top: -18px;
    left: 50%;
    transform: translateX(-50%);
    font-size: 0.6rem;
    font-weight: 600;
    color: #7EB2FF;
    text-transform: uppercase;
    letter-spacing: 0.5px;
}

.timeline-tasks {
    position: absolute;
    top: 50px;
    left: 0;
    right: 0;
    bottom: 16px;
}

.timeline-task {
    position: absolute;
    background: rgba(30, 30, 46, 0.9);
    border: 1px solid rgba(126, 178, 255, 0.3);
    border-radius: 8px;
    padding: 6px 10px;
    max-width: 120px;
    min-width: 80px;
    cursor: pointer;
    transition: transform 0.2s ease, box-shadow 0.2s ease;
    z-index: 5;
}

.timeline-task:hover {
    transform: translateY(-2px);
    box-shadow: 0 4px 12px rgba(126, 178, 255, 0.2);
    border-color: rgba(126, 178, 255, 0.5);
}

.timeline-task-time {
    font-size: 0.8rem;
    font-weight: 600;
    color: #7EB2FF;
    margin-bottom: 2px;
}

.timeline-task-desc {
    font-size: 0.7rem;
    color: #999;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
}

.timeline-empty {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 100px;
    color: #666;
    font-size: 0.9rem;
    text-align: center;
    padding: 1rem;
}

.timeline-sun-icon, .timeline-moon-icon {
    position: absolute;
    font-size: 1rem;
    opacity: 0.4;
    pointer-events: none;
}

.timeline-digest {
    position: absolute;
    background: rgba(110, 200, 140, 0.15);
    border: 1px solid rgba(110, 200, 140, 0.4);
    border-radius: 8px;
    padding: 6px 10px;
    max-width: 120px;
    min-width: 80px;
    cursor: pointer;
    transition: transform 0.2s ease, box-shadow 0.2s ease;
    z-index: 5;
}

.timeline-digest:hover {
    transform: translateY(-2px);
    box-shadow: 0 4px 12px rgba(110, 200, 140, 0.2);
    border-color: rgba(110, 200, 140, 0.6);
}

.timeline-digest-time {
    font-size: 0.8rem;
    font-weight: 600;
    color: #6ec88c;
    margin-bottom: 2px;
}

.timeline-digest-desc {
    font-size: 0.7rem;
    color: #999;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
}

.timeline-wrapper {
    position: relative;
}

.timeline-quiet-overlay {
    position: absolute;
    top: 0;
    left: 0;
    bottom: 0;
    background: rgba(80, 80, 80, 0.3);
    border-right: 2px dashed rgba(150, 150, 150, 0.5);
    pointer-events: none;
    z-index: 3;
    display: flex;
    align-items: flex-start;
    justify-content: flex-end;
    padding: 8px;
}

.timeline-quiet-label {
    font-size: 0.65rem;
    color: rgba(150, 150, 150, 0.8);
    font-weight: 500;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    background: rgba(0, 0, 0, 0.3);
    padding: 2px 6px;
    border-radius: 4px;
}

.timeline-current-day {
    color: #7EB2FF;
    padding: 4px 8px 0;
    font-size: 0.75rem;
    font-weight: 600;
    white-space: nowrap;
    opacity: 0.7;
}

.timeline-gap-indicator {
    position: absolute;
    top: 0;
    bottom: 0;
    background: repeating-linear-gradient(
        -45deg,
        transparent,
        transparent 4px,
        rgba(255, 255, 255, 0.03) 4px,
        rgba(255, 255, 255, 0.03) 8px
    );
    border-left: 1px dashed rgba(255, 255, 255, 0.15);
    border-right: 1px dashed rgba(255, 255, 255, 0.15);
    display: flex;
    align-items: center;
    justify-content: center;
}

.timeline-gap-label {
    font-size: 0.6rem;
    color: rgba(255, 255, 255, 0.35);
    text-transform: uppercase;
    letter-spacing: 0.5px;
    white-space: nowrap;
}
"#;

#[derive(Clone, PartialEq)]
pub struct UpcomingTask {
    pub task_id: Option<i32>,
    pub timestamp: i32,
    pub trigger_type: String,
    pub time_display: String,
    pub description: String,
    pub date_display: String,
    pub relative_display: String,
    pub condition: Option<String>,
    pub sources_display: Option<String>,
}

#[derive(Clone, PartialEq)]
pub struct UpcomingDigest {
    pub task_id: Option<i32>,
    pub timestamp: i32,
    pub time_display: String,
    pub sources: Option<String>,
}

#[derive(Properties, PartialEq, Clone)]
pub struct TimelineViewProps {
    pub upcoming_tasks: Vec<UpcomingTask>,
    #[prop_or_default]
    pub upcoming_digests: Vec<UpcomingDigest>,
    pub now_timestamp: i32,
    #[prop_or_default]
    pub on_task_click: Option<Callback<UpcomingTask>>,
    #[prop_or_default]
    pub on_digest_click: Option<Callback<UpcomingDigest>>,
    #[prop_or_default]
    pub sunrise_hour: Option<f32>,
    #[prop_or_default]
    pub sunset_hour: Option<f32>,
    /// Quiet mode until timestamp: None = not active, Some(0) = indefinite, Some(ts) = until ts
    #[prop_or_default]
    pub quiet_until: Option<i32>,
    /// When set, auto-scroll the timeline to this timestamp
    #[prop_or_default]
    pub scroll_to_timestamp: Option<i32>,
}

// Pixels per hour - fixed for consistent spacing within clusters
const PX_PER_HOUR: f64 = 60.0;

// Left margin so the "Now" marker isn't flush against the edge
const NOW_MARGIN_PX: f64 = 40.0;

// Gap compression constants
const GAP_THRESHOLD_HOURS: f64 = 24.0;   // gaps > 24h get compressed
const GAP_PX_WIDTH: f64 = 80.0;          // compressed gap indicator width
const CLUSTER_PADDING_HOURS: f64 = 3.0;  // context hours around events
const NOW_ZONE_HOURS: f64 = 24.0;        // always show first 24h linear

// --- Layout data structures ---

#[derive(Clone, Debug)]
enum Segment {
    Cluster {
        start_ts: f64,
        end_ts: f64,
        start_px: f64,
        px_width: f64,
    },
    Gap {
        start_ts: f64,
        end_ts: f64,
        start_px: f64,
        px_width: f64,
    },
}

#[derive(Clone, Debug)]
struct TimelineLayout {
    segments: Vec<Segment>,
    total_px: f64,
}

// --- Pure layout functions ---

fn build_layout(now_ts: f64, event_timestamps: &[f64], end_ts: f64) -> TimelineLayout {
    // Step 1: Start with the "now zone" cluster
    let now_zone_end = now_ts + NOW_ZONE_HOURS * 3600.0;
    let mut clusters: Vec<(f64, f64)> = vec![(now_ts, now_zone_end)];

    // Step 2: Sort event timestamps, group into clusters
    let mut sorted_ts: Vec<f64> = event_timestamps
        .iter()
        .filter(|ts| **ts > now_ts && **ts <= end_ts)
        .copied()
        .collect();
    sorted_ts.sort_by(|a, b| a.partial_cmp(b).unwrap());

    // Build raw clusters from events
    let mut event_clusters: Vec<(f64, f64)> = Vec::new();
    for ts in &sorted_ts {
        let padded_start = (ts - CLUSTER_PADDING_HOURS * 3600.0).max(now_ts);
        let padded_end = (ts + CLUSTER_PADDING_HOURS * 3600.0).min(end_ts);

        if let Some(last) = event_clusters.last_mut() {
            // Merge if within threshold of last cluster
            if padded_start <= last.1 + GAP_THRESHOLD_HOURS * 3600.0 {
                last.1 = padded_end.max(last.1);
                continue;
            }
        }
        event_clusters.push((padded_start, padded_end));
    }

    // Step 3: Merge event clusters with the now zone
    for ec in event_clusters {
        let mut merged = false;
        for existing in clusters.iter_mut() {
            if ec.0 <= existing.1 + GAP_THRESHOLD_HOURS * 3600.0 && ec.1 >= existing.0 {
                existing.0 = existing.0.min(ec.0);
                existing.1 = existing.1.max(ec.1);
                merged = true;
                break;
            }
        }
        if !merged {
            clusters.push(ec);
        }
    }

    // Sort and merge any overlapping clusters
    clusters.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    let mut merged_clusters: Vec<(f64, f64)> = Vec::new();
    for c in clusters {
        if let Some(last) = merged_clusters.last_mut() {
            if c.0 <= last.1 {
                last.1 = last.1.max(c.1);
                continue;
            }
        }
        merged_clusters.push(c);
    }

    // Step 4: Build segments with px positions (offset by NOW_MARGIN_PX)
    let mut segments: Vec<Segment> = Vec::new();
    let mut current_px = NOW_MARGIN_PX;

    for (i, (start, end)) in merged_clusters.iter().enumerate() {
        // Insert gap before this cluster (except before the first)
        if i > 0 {
            let prev_end = merged_clusters[i - 1].1;
            segments.push(Segment::Gap {
                start_ts: prev_end,
                end_ts: *start,
                start_px: current_px,
                px_width: GAP_PX_WIDTH,
            });
            current_px += GAP_PX_WIDTH;
        }

        let cluster_hours = (end - start) / 3600.0;
        let cluster_px = cluster_hours * PX_PER_HOUR;
        segments.push(Segment::Cluster {
            start_ts: *start,
            end_ts: *end,
            start_px: current_px,
            px_width: cluster_px,
        });
        current_px += cluster_px;
    }

    TimelineLayout {
        total_px: current_px,
        segments,
    }
}

fn ts_to_px(ts: f64, layout: &TimelineLayout) -> Option<f64> {
    for seg in &layout.segments {
        match seg {
            Segment::Cluster { start_ts, end_ts, start_px, px_width } => {
                if ts >= *start_ts && ts <= *end_ts {
                    let fraction = if (end_ts - start_ts).abs() < 1.0 {
                        0.0
                    } else {
                        (ts - start_ts) / (end_ts - start_ts)
                    };
                    return Some(start_px + fraction * px_width);
                }
            }
            Segment::Gap { start_ts, end_ts, start_px, px_width } => {
                if ts >= *start_ts && ts <= *end_ts {
                    let fraction = if (end_ts - start_ts).abs() < 1.0 {
                        0.0
                    } else {
                        (ts - start_ts) / (end_ts - start_ts)
                    };
                    return Some(start_px + fraction * px_width);
                }
            }
        }
    }
    // If ts is before the layout, return 0; if after, return total_px
    if let Some(first) = layout.segments.first() {
        let first_ts = match first {
            Segment::Cluster { start_ts, .. } | Segment::Gap { start_ts, .. } => *start_ts,
        };
        if ts < first_ts {
            return Some(0.0);
        }
    }
    Some(layout.total_px)
}

fn px_to_ts(px: f64, layout: &TimelineLayout) -> f64 {
    for seg in &layout.segments {
        match seg {
            Segment::Cluster { start_ts, end_ts, start_px, px_width } => {
                if px >= *start_px && px <= start_px + px_width {
                    let fraction = if *px_width < 1.0 {
                        0.0
                    } else {
                        (px - start_px) / px_width
                    };
                    return start_ts + fraction * (end_ts - start_ts);
                }
            }
            Segment::Gap { start_ts, end_ts, start_px, px_width } => {
                if px >= *start_px && px <= start_px + px_width {
                    let fraction = if *px_width < 1.0 {
                        0.0
                    } else {
                        (px - start_px) / px_width
                    };
                    return start_ts + fraction * (end_ts - start_ts);
                }
            }
        }
    }
    // Fallback: before first segment -> first start_ts, after last -> last end_ts
    if let Some(first) = layout.segments.first() {
        let first_start_px = match first {
            Segment::Cluster { start_px, .. } | Segment::Gap { start_px, .. } => *start_px,
        };
        if px < first_start_px {
            return match first {
                Segment::Cluster { start_ts, .. } | Segment::Gap { start_ts, .. } => *start_ts,
            };
        }
    }
    if let Some(last) = layout.segments.last() {
        return match last {
            Segment::Cluster { end_ts, .. } | Segment::Gap { end_ts, .. } => *end_ts,
        };
    }
    0.0
}

fn format_gap_label(duration_secs: f64) -> String {
    let hours = duration_secs / 3600.0;
    if hours < 48.0 {
        format!("{}h", hours.round() as i32)
    } else {
        let days = (hours / 24.0).round() as i32;
        if days < 14 {
            format!("{} days", days)
        } else {
            let weeks = (days as f64 / 7.0).round() as i32;
            if weeks == 1 {
                "1 week".to_string()
            } else {
                format!("{} weeks", weeks)
            }
        }
    }
}

#[function_component(TimelineView)]
pub fn timeline_view(props: &TimelineViewProps) -> Html {
    let container_ref = use_node_ref();

    let now_ts = props.now_timestamp as f64;

    // Collect all event timestamps for layout computation
    let event_timestamps: Vec<f64> = {
        let mut ts: Vec<f64> = props.upcoming_tasks.iter().map(|t| t.timestamp as f64).collect();
        ts.extend(props.upcoming_digests.iter().map(|d| d.timestamp as f64));
        ts
    };

    // Compute the end timestamp: 90 days from now (dashboard passes this)
    let ninety_days_secs = 90.0 * 24.0 * 3600.0;
    let end_ts = now_ts + ninety_days_secs;

    // Build the gap-compressed layout
    let layout = build_layout(now_ts, &event_timestamps, end_ts);
    let total_width = layout.total_px;

    // Track scroll position using polling (more reliable than scroll events in WASM)
    let scroll_position = use_state(|| 0.0_f64);

    // Poll scroll position every 100ms
    {
        let container_ref = container_ref.clone();
        let scroll_position = scroll_position.clone();
        use_effect_with_deps(
            move |_: &()| {
                let container_ref = container_ref.clone();
                let scroll_position = scroll_position.clone();

                let interval = gloo_timers::callback::Interval::new(100, move || {
                    if let Some(elem) = container_ref.cast::<web_sys::HtmlElement>() {
                        let scroll_left = elem.scroll_left() as f64;
                        if (*scroll_position - scroll_left).abs() > 1.0 {
                            scroll_position.set(scroll_left);
                        }
                    }
                });

                move || drop(interval)
            },
            (),
        );
    }

    // Auto-scroll to a specific timestamp when requested
    {
        let container_ref = container_ref.clone();
        let layout_clone = layout.clone();
        use_effect_with_deps(
            move |scroll_to: &Option<i32>| {
                if let Some(target_ts) = *scroll_to {
                    let container_ref = container_ref.clone();
                    let target_px = ts_to_px(target_ts as f64, &layout_clone).unwrap_or(0.0);

                    gloo_timers::callback::Timeout::new(50, move || {
                        if let Some(elem) = container_ref.cast::<web_sys::HtmlElement>() {
                            let client_width = elem.client_width() as f64;
                            let scroll_target = (target_px - client_width * 0.3).max(0.0);
                            elem.set_scroll_left(scroll_target as i32);
                        }
                    }).forget();
                }
                || ()
            },
            props.scroll_to_timestamp,
        );
    }

    // Calculate date label based on the center of the visible viewport
    let current_label = {
        let scroll_left = *scroll_position;
        // Use viewport center: need client_width from the container
        let client_width = container_ref.cast::<web_sys::HtmlElement>()
            .map(|el| el.client_width() as f64)
            .unwrap_or(300.0);
        let center_px = scroll_left + client_width / 2.0;
        let center_ts = px_to_ts(center_px, &layout);

        let view_date = js_sys::Date::new(&wasm_bindgen::JsValue::from_f64(center_ts * 1000.0));
        let now_date = js_sys::Date::new(&wasm_bindgen::JsValue::from_f64(now_ts * 1000.0));

        let view_day = view_date.get_date();
        let view_month = view_date.get_month();
        let now_day = now_date.get_date();
        let now_month = now_date.get_month();

        if view_day == now_day && view_month == now_month {
            "Today".to_string()
        } else {
            let tomorrow_ts = now_ts + 86400.0;
            let tomorrow_date = js_sys::Date::new(&wasm_bindgen::JsValue::from_f64(tomorrow_ts * 1000.0));
            if view_day == tomorrow_date.get_date() && view_month == tomorrow_date.get_month() {
                "Tomorrow".to_string()
            } else {
                let day_of_week = view_date.get_day();
                let day_str = match day_of_week {
                    0 => "Sun",
                    1 => "Mon",
                    2 => "Tue",
                    3 => "Wed",
                    4 => "Thu",
                    5 => "Fri",
                    6 => "Sat",
                    _ => "",
                };
                let month_str = match view_date.get_month() {
                    0 => "Jan",
                    1 => "Feb",
                    2 => "Mar",
                    3 => "Apr",
                    4 => "May",
                    5 => "Jun",
                    6 => "Jul",
                    7 => "Aug",
                    8 => "Sep",
                    9 => "Oct",
                    10 => "Nov",
                    11 => "Dec",
                    _ => "",
                };
                format!("{} {} {}", day_str, month_str, view_day)
            }
        }
    };

    // Get sunrise/sunset hours (default to reasonable values if not provided)
    let sunrise = props.sunrise_hour.unwrap_or(7.0) as f64;
    let sunset = props.sunset_hour.unwrap_or(19.0) as f64;

    // Generate time-of-day gradient for a cluster
    let generate_gradient = {
        let sunrise = sunrise;
        let sunset = sunset;
        move |start_hour: f64, hours: f64| -> String {
            let get_color = |hour: f64| -> &str {
                let h = hour % 24.0;
                if h < (sunrise - 2.0).max(0.0) { "#1a1a2e" }
                else if h < (sunrise - 1.5).max(0.0) { "#2d1f3d" }
                else if h < (sunrise - 1.0).max(0.0) { "#4a3050" }
                else if h < (sunrise - 0.5).max(0.0) { "#6b4a5e" }
                else if h < sunrise + 1.0 { "#87616b" }
                else if h < sunset - 1.0 { "#3d4f6f" }
                else if h < sunset - 0.5 { "#5a5070" }
                else if h < sunset { "#6b4a5e" }
                else if h < sunset + 1.0 { "#4a3050" }
                else if h < sunset + 2.0 { "#2d1f3d" }
                else { "#1a1a2e" }
            };

            let mut stops = Vec::new();
            let step = 1.0;
            let mut h = 0.0;
            while h <= hours {
                let pct = if hours > 0.0 { (h / hours) * 100.0 } else { 0.0 };
                let hour_of_day = (start_hour + h) % 24.0;
                let color = get_color(hour_of_day);
                stops.push(format!("{} {:.1}%", color, pct));
                h += step;
            }

            if stops.is_empty() {
                return "linear-gradient(to right, #1a1a2e, #1a1a2e)".to_string();
            }

            format!("linear-gradient(to right, {})", stops.join(", "))
        }
    };

    // Render per-segment backgrounds and gap indicators
    let segment_backgrounds: Vec<Html> = layout.segments.iter().enumerate().map(|(i, seg)| {
        match seg {
            Segment::Cluster { start_ts, end_ts, start_px, px_width } => {
                let start_date = js_sys::Date::new(&wasm_bindgen::JsValue::from_f64(start_ts * 1000.0));
                let start_hour_of_day = start_date.get_hours() as f64 + (start_date.get_minutes() as f64 / 60.0);
                let cluster_hours = (end_ts - start_ts) / 3600.0;
                let gradient = generate_gradient(start_hour_of_day, cluster_hours);

                let mut border_radius = "0".to_string();
                if i == 0 {
                    border_radius = "12px 0 0 12px".to_string();
                }
                if i == layout.segments.len() - 1 {
                    if i == 0 {
                        border_radius = "12px".to_string();
                    } else {
                        border_radius = "0 12px 12px 0".to_string();
                    }
                }

                html! {
                    <div
                        class="timeline-background"
                        style={format!(
                            "left: {}px; width: {}px; background: {}; border-radius: {};",
                            start_px, px_width, gradient, border_radius
                        )}
                    ></div>
                }
            }
            Segment::Gap { start_ts, end_ts, start_px, px_width } => {
                let duration = end_ts - start_ts;
                let label = format_gap_label(duration);
                html! {
                    <div
                        class="timeline-gap-indicator"
                        style={format!("left: {}px; width: {}px;", start_px, px_width)}
                    >
                        <span class="timeline-gap-label">{label}</span>
                    </div>
                }
            }
        }
    }).collect();

    // Generate hour marks per cluster only
    let hour_marks: Vec<Html> = layout.segments.iter().flat_map(|seg| {
        match seg {
            Segment::Cluster { start_ts, end_ts, .. } => {
                let start_date = js_sys::Date::new(&wasm_bindgen::JsValue::from_f64(start_ts * 1000.0));
                let start_hour_raw = start_date.get_hours() as f64 + (start_date.get_minutes() as f64 / 60.0);

                // Find next 3-hour-aligned mark at or after start
                let first_mark_hour = ((start_hour_raw / 3.0).ceil() * 3.0) as i32;
                let cluster_hours = ((end_ts - start_ts) / 3600.0) as i32;

                let layout_ref = &layout;
                let marks: Vec<Html> = (0..=cluster_hours).step_by(3).filter_map(|offset| {
                    let mark_ts = start_ts + ((first_mark_hour as f64 - start_hour_raw) + offset as f64) * 3600.0;
                    if mark_ts < *start_ts || mark_ts > *end_ts {
                        return None;
                    }

                    let mark_date = js_sys::Date::new(&wasm_bindgen::JsValue::from_f64(mark_ts * 1000.0));
                    let hour_of_day = mark_date.get_hours() as i32;
                    let display = if hour_of_day == 0 {
                        "12am".to_string()
                    } else if hour_of_day == 12 {
                        "12pm".to_string()
                    } else if hour_of_day > 12 {
                        format!("{}pm", hour_of_day - 12)
                    } else {
                        format!("{}am", hour_of_day)
                    };

                    let left = ts_to_px(mark_ts, layout_ref).unwrap_or(0.0);

                    Some(html! {
                        <div
                            class="timeline-hour-mark"
                            style={format!("left: {}px;", left)}
                        >
                            {display}
                        </div>
                    })
                }).collect();
                marks
            }
            Segment::Gap { .. } => Vec::new(),
        }
    }).collect();

    // Generate day markers: midnight lines + labels, plus cluster-start labels after gaps
    let day_markers: Vec<Html> = {
        let mut markers = Vec::new();
        let date = js_sys::Date::new(&wasm_bindgen::JsValue::from_f64(now_ts * 1000.0));
        let current_hour_int = date.get_hours() as i32;
        let hours_until_midnight = 24 - current_hour_int;

        let layout_end_ts = layout.segments.last().map(|s| match s {
            Segment::Cluster { end_ts, .. } | Segment::Gap { end_ts, .. } => *end_ts,
        }).unwrap_or(now_ts + 86400.0);

        let total_days = ((layout_end_ts - now_ts) / 86400.0).ceil() as i32;

        // Midnight markers within clusters
        for day in 0..total_days {
            let hours_from_now = if day == 0 {
                hours_until_midnight
            } else {
                hours_until_midnight + (day * 24)
            };

            let midnight_ts = now_ts + (hours_from_now as f64 * 3600.0);
            if midnight_ts > layout_end_ts {
                break;
            }

            let in_gap = layout.segments.iter().any(|seg| {
                matches!(seg, Segment::Gap { start_ts, end_ts, .. } if midnight_ts > *start_ts && midnight_ts < *end_ts)
            });
            if in_gap {
                continue;
            }

            let left = ts_to_px(midnight_ts, &layout).unwrap_or(0.0);

            let future_date = js_sys::Date::new(&wasm_bindgen::JsValue::from_f64(midnight_ts * 1000.0));
            let date_num = future_date.get_date();
            let day_name = if day == 0 {
                "Tomorrow".to_string()
            } else {
                let day_of_week = future_date.get_day();
                let day_str = match day_of_week {
                    0 => "Sun",
                    1 => "Mon",
                    2 => "Tue",
                    3 => "Wed",
                    4 => "Thu",
                    5 => "Fri",
                    6 => "Sat",
                    _ => "",
                };
                format!("{} {}", day_str, date_num)
            };

            markers.push(html! {
                <>
                    <div class="timeline-day-marker" style={format!("left: {}px;", left)}></div>
                    <div class="timeline-day-label" style={format!("left: {}px;", left + 6.0)}>{day_name}</div>
                </>
            });
        }

        // Cluster-start date labels after gaps (so you know what day you jumped to)
        let mut prev_was_gap = false;
        for seg in &layout.segments {
            match seg {
                Segment::Gap { .. } => {
                    prev_was_gap = true;
                }
                Segment::Cluster { start_ts, start_px, .. } if prev_was_gap => {
                    prev_was_gap = false;
                    let cluster_date = js_sys::Date::new(&wasm_bindgen::JsValue::from_f64(start_ts * 1000.0));
                    let day_of_week = cluster_date.get_day();
                    let day_str = match day_of_week {
                        0 => "Sun",
                        1 => "Mon",
                        2 => "Tue",
                        3 => "Wed",
                        4 => "Thu",
                        5 => "Fri",
                        6 => "Sat",
                        _ => "",
                    };
                    let month = cluster_date.get_month();
                    let month_str = match month {
                        0 => "Jan", 1 => "Feb", 2 => "Mar", 3 => "Apr",
                        4 => "May", 5 => "Jun", 6 => "Jul", 7 => "Aug",
                        8 => "Sep", 9 => "Oct", 10 => "Nov", 11 => "Dec",
                        _ => "",
                    };
                    let date_num = cluster_date.get_date();
                    let label = format!("{} {} {}", day_str, month_str, date_num);

                    markers.push(html! {
                        <div
                            class="timeline-day-label"
                            style={format!("left: {}px; color: rgba(126, 178, 255, 0.6);", start_px + 6.0)}
                        >
                            {label}
                        </div>
                    });
                }
                _ => {
                    prev_was_gap = false;
                }
            }
        }

        markers
    };

    // Collect all items (tasks + digests) with their positions for collision detection
    #[derive(Clone)]
    struct TimelineItem {
        timestamp: i32,
        left_px: f64,
        is_digest: bool,
        idx: usize,
    }

    let mut all_items: Vec<TimelineItem> = Vec::new();

    // Add tasks
    for (idx, task) in props.upcoming_tasks.iter().enumerate() {
        if let Some(left) = ts_to_px(task.timestamp as f64, &layout) {
            all_items.push(TimelineItem {
                timestamp: task.timestamp,
                left_px: left,
                is_digest: false,
                idx,
            });
        }
    }

    // Add digests
    for (idx, digest) in props.upcoming_digests.iter().enumerate() {
        if let Some(left) = ts_to_px(digest.timestamp as f64, &layout) {
            all_items.push(TimelineItem {
                timestamp: digest.timestamp,
                left_px: left,
                is_digest: true,
                idx,
            });
        }
    }

    // Sort by timestamp
    all_items.sort_by_key(|item| item.timestamp);

    // Assign rows based on collision detection
    const ITEM_WIDTH: f64 = 100.0;
    let mut item_rows: std::collections::HashMap<(bool, usize), i32> = std::collections::HashMap::new();
    let mut row_end_positions: Vec<f64> = vec![-1000.0; 3];

    for item in &all_items {
        let mut assigned_row = 0;
        for (row, end_pos) in row_end_positions.iter().enumerate() {
            if item.left_px >= *end_pos {
                assigned_row = row as i32;
                break;
            }
            if row == row_end_positions.len() - 1 {
                assigned_row = row_end_positions
                    .iter()
                    .enumerate()
                    .min_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
                    .map(|(i, _)| i as i32)
                    .unwrap_or(0);
            }
        }

        row_end_positions[assigned_row as usize] = item.left_px + ITEM_WIDTH;
        item_rows.insert((item.is_digest, item.idx), assigned_row);
    }

    // Generate task markers with layout-based positioning
    let on_task_click = props.on_task_click.clone();
    let item_rows_clone = item_rows.clone();
    let task_markers: Vec<Html> = props.upcoming_tasks.iter().enumerate().map(|(idx, task)| {
        let left = match ts_to_px(task.timestamp as f64, &layout) {
            Some(px) => px,
            None => return html! {},
        };

        let row = item_rows_clone.get(&(false, idx)).copied().unwrap_or(0);
        let top = row * 35;

        let onclick = {
            let task = task.clone();
            let callback = on_task_click.clone();
            Callback::from(move |e: MouseEvent| {
                e.stop_propagation();
                if let Some(cb) = &callback {
                    cb.emit(task.clone());
                }
            })
        };

        html! {
            <div
                class="timeline-task"
                style={format!("left: {}px; top: {}px;", left, top)}
                onclick={onclick}
            >
                <div class="timeline-task-time">{&task.time_display}</div>
                <div class="timeline-task-desc" title={task.description.clone()}>{&task.description}</div>
            </div>
        }
    }).collect();

    // Generate digest markers with layout-based positioning
    let on_digest_click = props.on_digest_click.clone();
    let digest_markers: Vec<Html> = props.upcoming_digests.iter().enumerate().map(|(idx, digest)| {
        let left = match ts_to_px(digest.timestamp as f64, &layout) {
            Some(px) => px,
            None => return html! {},
        };

        let row = item_rows.get(&(true, idx)).copied().unwrap_or(0);
        let top = row * 35;

        let sources_display = digest.sources.as_ref()
            .map(|s| s.replace(",", ", "))
            .unwrap_or_else(|| "Digest".to_string());

        let onclick = {
            let digest = digest.clone();
            let callback = on_digest_click.clone();
            Callback::from(move |e: MouseEvent| {
                e.stop_propagation();
                if let Some(cb) = &callback {
                    cb.emit(digest.clone());
                }
            })
        };

        html! {
            <div
                class="timeline-digest"
                style={format!("left: {}px; top: {}px;", left, top)}
                onclick={onclick}
            >
                <div class="timeline-digest-time">{&digest.time_display}</div>
                <div class="timeline-digest-desc" title={sources_display.clone()}>{"Digest"}</div>
            </div>
        }
    }).collect();

    // Calculate quiet mode overlay width using ts_to_px
    let quiet_overlay_width: Option<f64> = props.quiet_until.map(|until| {
        if until == 0 {
            total_width
        } else {
            ts_to_px(until as f64, &layout).unwrap_or(0.0)
        }
    });

    if props.upcoming_tasks.is_empty() && props.upcoming_digests.is_empty() {
        return html! {
            <>
                <style>{TIMELINE_STYLES}</style>
                <div class="timeline-empty">
                    {"No upcoming tasks scheduled"}
                </div>
            </>
        };
    }

    html! {
        <>
            <style>{TIMELINE_STYLES}</style>
            <div class="timeline-wrapper">
                <div class="timeline-container" ref={container_ref}>
                    <div
                        class="timeline-track"
                        style={format!("width: {}px;", total_width)}
                    >
                        // Per-segment backgrounds and gap indicators
                        {for segment_backgrounds}

                        // Quiet mode overlay (if active)
                        if let Some(width) = quiet_overlay_width {
                            if width > 0.0 {
                                <div
                                    class="timeline-quiet-overlay"
                                    style={format!("width: {}px;", width)}
                                >
                                    <span class="timeline-quiet-label">{"Quiet"}</span>
                                </div>
                            }
                        }

                        // Hour marks
                        <div class="timeline-hours">
                            {for hour_marks}
                        </div>

                        // Day markers
                        <div class="timeline-day-markers">
                            {for day_markers}
                        </div>

                        // Now marker
                        <div class="timeline-now-marker" style={format!("left: {}px;", NOW_MARGIN_PX)}>
                            <div class="timeline-now-label">{"Now"}</div>
                        </div>

                        // Task markers
                        <div class="timeline-tasks">
                            {for task_markers}
                            {for digest_markers}
                        </div>
                    </div>
                </div>

                // Current day label below timeline
                <div class="timeline-current-day">
                    {current_label.clone()}
                </div>
            </div>
        </>
    }
}
