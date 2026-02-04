use yew::prelude::*;

const SCHEDULED_STYLES: &str = r#"
.next-scheduled {
    text-align: center;
    padding: 1.5rem 1rem;
}
.next-scheduled.empty {
    opacity: 0.6;
}
.next-scheduled-time {
    font-size: 1.5rem;
    color: #7EB2FF;
    font-weight: 500;
    margin-bottom: 0.25rem;
}
.next-scheduled-desc {
    color: #999;
    font-size: 0.95rem;
}
"#;

#[derive(Clone, PartialEq)]
pub struct ScheduledItem {
    pub time_display: String,
    pub description: String,
    pub task_id: Option<i32>,
}

#[derive(Properties, PartialEq, Clone)]
pub struct NextScheduledProps {
    pub item: Option<ScheduledItem>,
}

#[function_component(NextScheduled)]
pub fn next_scheduled(props: &NextScheduledProps) -> Html {
    html! {
        <>
            <style>{SCHEDULED_STYLES}</style>
            {
                match &props.item {
                    Some(item) => html! {
                        <div class="next-scheduled">
                            <div class="next-scheduled-time">{&item.time_display}</div>
                            <div class="next-scheduled-desc">{&item.description}</div>
                        </div>
                    },
                    None => html! {
                        <div class="next-scheduled empty">
                            <div class="next-scheduled-desc">{"No upcoming tasks"}</div>
                        </div>
                    },
                }
            }
        </>
    }
}
