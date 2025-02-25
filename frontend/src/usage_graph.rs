use yew::prelude::*;
use web_sys::HtmlCanvasElement;
use wasm_bindgen::JsCast;
use plotters::prelude::*;
use plotters_canvas::CanvasBackend;
use gloo_net::http::Request;
use serde::Deserialize;
use crate::config;

#[derive(Properties, PartialEq)]
pub struct Props {
    pub user_id: i32,
}

#[derive(Deserialize, Clone, PartialEq)]
struct UsageDataPoint {
    timestamp: i32,
    iq_used: i32,
}

#[function_component]
pub fn UsageGraph(props: &Props) -> Html {
    let canvas_ref = use_node_ref();
    let usage_data = use_state(Vec::new);

    // Fetch usage data
    {
        let usage_data = usage_data.clone();
        let user_id = props.user_id;
        use_effect_with_deps(move |_| {
            let thirty_days_ago = chrono::Utc::now()
                .timestamp() as i32 - (30 * 24 * 60 * 60);

            wasm_bindgen_futures::spawn_local(async move {
                if let Some(token) = web_sys::window()
                    .and_then(|w| w.local_storage().ok())
                    .flatten()
                    .and_then(|storage| storage.get_item("token").ok())
                    .flatten()
                {
                    if let Ok(response) = Request::post(&format!(
                        "{}/api/profile/usage",
                        config::get_backend_url()
                    ))
                    .header("Authorization", &format!("Bearer {}", token))
                    .json(&serde_json::json!({
                        "user_id": user_id,
                        "from": thirty_days_ago
                    }))
                    .unwrap()
                    .send()
                    .await
                    {
                        if let Ok(data) = response.json::<Vec<UsageDataPoint>>().await {
                            usage_data.set(data);
                        }
                    }
                }
            });
            || ()
        }, ());
    }

    // Draw the histogram
    {
        let canvas_ref = canvas_ref.clone();
        let usage_data_for_effect = (*usage_data).clone();
        let usage_data_for_deps = (*usage_data).clone();
        use_effect_with_deps(move |_| {
            if let Some(canvas) = canvas_ref.cast::<HtmlCanvasElement>() {
                if !usage_data_for_effect.is_empty() {
                    // Clear the canvas
                    let context = canvas
                        .get_context("2d")
                        .unwrap()
                        .unwrap()
                        .dyn_into::<web_sys::CanvasRenderingContext2d>()
                        .unwrap();
                    context.clear_rect(0.0, 0.0, canvas.width() as f64, canvas.height() as f64);

                    // Set canvas size explicitly
                    canvas.set_width(600);
                    canvas.set_height(400);

                    // Create the backend
                    let backend = CanvasBackend::with_canvas_object(canvas.clone()).unwrap();
                    let root = backend.into_drawing_area();
                    root.fill(&WHITE).unwrap();

                    // Group data by day


                    let mut daily_usage: std::collections::HashMap<String, i32> = std::collections::HashMap::new();
                    for point in usage_data_for_effect.iter() {
                        // Convert timestamp to date string for better x-axis display
                        let date = chrono::NaiveDateTime::from_timestamp_opt(point.timestamp as i64, 0)
                            .unwrap()
                            .format("%d")
                            .to_string();
                        *daily_usage.entry(date).or_insert(0) += point.iq_used;
                    }

                    let mut data: Vec<(String, i32)> = daily_usage.into_iter().collect();
                    data.sort();

                    if !data.is_empty() {

                        let max_usage = data.iter().map(|(_, usage)| *usage).max().unwrap_or(0);
                        let max_euros = (max_usage as f64 / 60.0).ceil() as i32;

                        let mut chart = ChartBuilder::on(&root)
                            .margin(10)
                            .caption("Daily IQ Usage", ("sans-serif", 20))
                            .x_label_area_size(40)
                            .y_label_area_size(60) // Increased to accommodate both scales
                            .build_cartesian_2d(
                                0..data.len(),
                                0..max_usage + (max_usage / 10) // Add 10% padding to max
                            )
                            .unwrap();

                        chart
                            .configure_mesh()
                            .disable_x_mesh()
                            .disable_y_mesh()
                            .x_labels(data.len())
                            .x_label_formatter(&|x| {
                                data.get(*x as usize)
                                    .map(|(date, _)| date.clone())
                                    .unwrap_or_default()
                            })
                            .y_label_formatter(&|y| {
                                format!("{} (€{:.1})", y, *y as f64 / 300.0)
                            })
                            .draw()
                            .unwrap();

                        // Draw bars
                        chart
                            .draw_series(
                                data.iter().enumerate().map(|(i, (_, usage))| {
                                    Rectangle::new(
                                        [(i, 0), (i + 1, *usage as i32)],
                                        BLUE.filled(),
                                    )
                                })
                            )
                            .unwrap();
                    }
                }
            }
            || ()
        }, usage_data_for_deps);
    }

    html! {
        <div class="usage-graph">
            <canvas
                ref={canvas_ref}
                width="600"
                height="400"
                style="max-width: 100%;"
            />
        </div>
    }
}

