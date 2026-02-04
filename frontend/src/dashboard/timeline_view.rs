use yew::prelude::*;
use web_sys::HtmlElement;

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
    left: 0;
    right: 0;
    bottom: 0;
    border-radius: 12px;
    opacity: 0.6;
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
    top: 28px;
    font-size: 0.7rem;
    font-weight: 500;
    color: rgba(255, 255, 255, 0.5);
    transform: translateX(-50%);
    white-space: nowrap;
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
"#;

#[derive(Clone, PartialEq)]
pub struct UpcomingTask {
    pub task_id: Option<i32>,
    pub timestamp: i32,
    pub time_display: String,
    pub description: String,
}

#[derive(Clone, PartialEq)]
pub struct UpcomingDigest {
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
}

// Total hours in timeline (7 days)
const TOTAL_HOURS: f64 = 24.0 * 7.0;
// Pixels per hour
const PX_PER_HOUR: f64 = 60.0;

#[function_component(TimelineView)]
pub fn timeline_view(props: &TimelineViewProps) -> Html {
    let container_ref = use_node_ref();

    // Calculate timeline dimensions
    let total_width = TOTAL_HOURS * PX_PER_HOUR;
    let now_ts = props.now_timestamp as f64;

    // Get sunrise/sunset hours (default to reasonable values if not provided)
    let sunrise = props.sunrise_hour.unwrap_or(7.0) as f64;
    let sunset = props.sunset_hour.unwrap_or(19.0) as f64;

    // Generate time-of-day gradient based on hours
    let generate_gradient = |start_hour: f64, hours: f64| -> String {
        // Color stops for different times of day based on sunrise/sunset
        let get_color = |hour: f64| -> &str {
            let h = hour % 24.0;
            // Night: before dawn starts (2 hours before sunrise)
            if h < (sunrise - 2.0).max(0.0) { "#1a1a2e" }
            // Pre-dawn: purple (1.5-2 hours before sunrise)
            else if h < (sunrise - 1.5).max(0.0) { "#2d1f3d" }
            // Dawn start: purple-pink (1-1.5 hours before sunrise)
            else if h < (sunrise - 1.0).max(0.0) { "#4a3050" }
            // Dawn: pink (0.5-1 hour before sunrise)
            else if h < (sunrise - 0.5).max(0.0) { "#6b4a5e" }
            // Morning transition: muted pink (sunrise to 1 hour after)
            else if h < sunrise + 1.0 { "#87616b" }
            // Day: blue-gray (main daylight hours)
            else if h < sunset - 1.0 { "#3d4f6f" }
            // Pre-dusk: purple (1 hour before sunset)
            else if h < sunset - 0.5 { "#5a5070" }
            // Dusk: pink (30 min before sunset to sunset)
            else if h < sunset { "#6b4a5e" }
            // Evening: purple (sunset to 1 hour after)
            else if h < sunset + 1.0 { "#4a3050" }
            // Post-evening transition
            else if h < sunset + 2.0 { "#2d1f3d" }
            // Night: deep blue
            else { "#1a1a2e" }
        };

        let mut stops = Vec::new();
        let step = 1.0; // 1 hour steps
        let mut h = 0.0;
        while h <= hours {
            let pct = (h / hours) * 100.0;
            let hour_of_day = (start_hour + h) % 24.0;
            let color = get_color(hour_of_day);
            stops.push(format!("{} {:.1}%", color, pct));
            h += step;
        }

        format!("linear-gradient(to right, {})", stops.join(", "))
    };

    // Get current hour of day for gradient start
    let current_hour = {
        // Convert timestamp to hour of day using JS
        let date = js_sys::Date::new(&wasm_bindgen::JsValue::from_f64(now_ts * 1000.0));
        date.get_hours() as f64 + (date.get_minutes() as f64 / 60.0)
    };

    let gradient = generate_gradient(current_hour, TOTAL_HOURS);

    // Generate hour marks (every 3 hours)
    let hour_marks: Vec<Html> = (0..=(TOTAL_HOURS as i32)).step_by(3).map(|h| {
        let hour_of_day = ((current_hour as i32 + h) % 24) as i32;
        let display = if hour_of_day == 0 {
            "12am".to_string()
        } else if hour_of_day == 12 {
            "12pm".to_string()
        } else if hour_of_day > 12 {
            format!("{}pm", hour_of_day - 12)
        } else {
            format!("{}am", hour_of_day)
        };

        let left = h as f64 * PX_PER_HOUR;

        html! {
            <div
                class="timeline-hour-mark"
                style={format!("left: {}px;", left)}
            >
                {display}
            </div>
        }
    }).collect();

    // Generate day markers
    let day_markers: Vec<Html> = {
        let mut markers = Vec::new();
        let date = js_sys::Date::new(&wasm_bindgen::JsValue::from_f64(now_ts * 1000.0));
        let current_hour_int = date.get_hours() as i32;
        let hours_until_midnight = 24 - current_hour_int;

        for day in 0..7 {
            let hours_from_now = if day == 0 {
                hours_until_midnight
            } else {
                hours_until_midnight + (day * 24)
            };

            let left = hours_from_now as f64 * PX_PER_HOUR;

            // Get day name
            let day_name = if day == 0 {
                "Tomorrow".to_string()
            } else {
                let future_date = js_sys::Date::new(&wasm_bindgen::JsValue::from_f64(
                    (now_ts + (hours_from_now as f64 * 3600.0)) * 1000.0
                ));
                let day_of_week = future_date.get_day();
                match day_of_week {
                    0 => "Sun",
                    1 => "Mon",
                    2 => "Tue",
                    3 => "Wed",
                    4 => "Thu",
                    5 => "Fri",
                    6 => "Sat",
                    _ => "",
                }.to_string()
            };

            if day < 6 {
                markers.push(html! {
                    <>
                        <div class="timeline-day-marker" style={format!("left: {}px;", left)}></div>
                        <div class="timeline-day-label" style={format!("left: {}px;", left + 20.0)}>{day_name}</div>
                    </>
                });
            }
        }
        markers
    };

    // Generate task markers
    let on_task_click = props.on_task_click.clone();
    let task_markers: Vec<Html> = props.upcoming_tasks.iter().enumerate().map(|(idx, task)| {
        let task_ts = task.timestamp as f64;
        let hours_from_now = (task_ts - now_ts) / 3600.0;

        // Only show tasks within the timeline range
        if hours_from_now < 0.0 || hours_from_now > TOTAL_HOURS {
            return html! {};
        }

        let left = hours_from_now * PX_PER_HOUR;

        // Stagger vertical position to avoid overlaps
        let top = match idx % 3 {
            0 => 0,
            1 => 35,
            _ => 70,
        };

        // Create click handler
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

    // Generate digest markers
    let on_digest_click = props.on_digest_click.clone();
    let num_tasks = props.upcoming_tasks.len();
    let digest_markers: Vec<Html> = props.upcoming_digests.iter().enumerate().map(|(idx, digest)| {
        let digest_ts = digest.timestamp as f64;
        let hours_from_now = (digest_ts - now_ts) / 3600.0;

        // Only show digests within the timeline range
        if hours_from_now < 0.0 || hours_from_now > TOTAL_HOURS {
            return html! {};
        }

        let left = hours_from_now * PX_PER_HOUR;

        // Stagger vertical position to avoid overlaps (offset from tasks)
        let top = match (num_tasks + idx) % 3 {
            0 => 0,
            1 => 35,
            _ => 70,
        };

        // Format sources for display
        let sources_display = digest.sources.as_ref()
            .map(|s| s.replace(",", ", "))
            .unwrap_or_else(|| "Digest".to_string());

        // Create click handler
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

    // Scroll to show "now" on the left with some padding
    {
        let container_ref = container_ref.clone();
        use_effect_with_deps(move |_| {
            if let Some(container) = container_ref.cast::<HtmlElement>() {
                // Small delay to ensure rendering is complete
                let _ = container.scroll_to_with_x_and_y(0.0, 0.0);
            }
            || ()
        }, ());
    }

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
            <div class="timeline-container" ref={container_ref}>
                <div
                    class="timeline-track"
                    style={format!("width: {}px;", total_width)}
                >
                    // Background gradient
                    <div
                        class="timeline-background"
                        style={format!("background: {};", gradient)}
                    ></div>

                    // Hour marks
                    <div class="timeline-hours">
                        {for hour_marks}
                    </div>

                    // Day markers
                    <div class="timeline-day-markers">
                        {for day_markers}
                    </div>

                    // Now marker
                    <div class="timeline-now-marker" style="left: 0px;">
                        <div class="timeline-now-label">{"Now"}</div>
                    </div>

                    // Task markers
                    <div class="timeline-tasks">
                        {for task_markers}
                        {for digest_markers}
                    </div>
                </div>
            </div>
        </>
    }
}
