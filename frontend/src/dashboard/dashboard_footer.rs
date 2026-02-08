use yew::prelude::*;
use crate::dashboard::quiet_mode::{QuietModeIndicator, QuietModeStatus};

const FOOTER_STYLES: &str = r#"
.peace-footer {
    display: flex;
    flex-direction: column;
    gap: 1rem;
    text-align: center;
}
.footer-info {
    color: #666;
    font-size: 0.85rem;
}
.footer-watching {
    margin-bottom: 0.25rem;
}
.footer-digest {
    color: #888;
}
.footer-actions {
    display: flex;
    justify-content: center;
    align-items: center;
    gap: 0.75rem;
    flex-wrap: wrap;
}
.footer-btn {
    background: transparent;
    border: 1px solid rgba(255, 255, 255, 0.15);
    color: #999;
    padding: 0.5rem 1.25rem;
    border-radius: 6px;
    font-size: 0.9rem;
    cursor: pointer;
    transition: all 0.2s ease;
}
.footer-btn:hover {
    background: rgba(255, 255, 255, 0.05);
    border-color: rgba(255, 255, 255, 0.25);
    color: #ccc;
}
"#;

#[derive(Clone, PartialEq)]
pub struct WatchedContact {
    pub nickname: String,
    pub notification_mode: String,
}

#[derive(Clone, PartialEq)]
pub struct NextDigestInfo {
    pub time_display: String,
}

#[derive(Properties, PartialEq, Clone)]
pub struct DashboardFooterProps {
    pub watched_contacts: Vec<WatchedContact>,
    pub next_digest: Option<NextDigestInfo>,
    pub quiet_mode: QuietModeStatus,
    pub on_activity_click: Callback<()>,
    #[prop_or_default]
    pub on_quiet_mode_change: Option<Callback<()>>,
}

#[function_component(DashboardFooter)]
pub fn dashboard_footer(props: &DashboardFooterProps) -> Html {
    // Format watched contacts as a comma-separated list
    let watched_names: Vec<String> = props
        .watched_contacts
        .iter()
        .take(5)
        .map(|c| c.nickname.clone())
        .collect();

    let watching_text = if watched_names.is_empty() {
        "No contacts being watched".to_string()
    } else if watched_names.len() < props.watched_contacts.len() {
        format!(
            "Watching: {} + {} more",
            watched_names.join(", "),
            props.watched_contacts.len() - watched_names.len()
        )
    } else {
        format!("Watching: {}", watched_names.join(", "))
    };

    let digest_text = match &props.next_digest {
        Some(info) => format!("Next digest: {}", info.time_display),
        None => "No digest scheduled".to_string(),
    };

    html! {
        <>
            <style>{FOOTER_STYLES}</style>
            <div class="peace-footer">
                <div class="footer-info">
                    <div class="footer-watching">{watching_text}</div>
                    <div class="footer-digest">{digest_text}</div>
                </div>
                <div class="footer-actions">
                    <QuietModeIndicator
                        initial_status={props.quiet_mode.clone()}
                        on_change={props.on_quiet_mode_change.clone()}
                    />
                    <button
                        class="footer-btn"
                        onclick={{
                            let cb = props.on_activity_click.clone();
                            Callback::from(move |_| cb.emit(()))
                        }}
                    >
                        {"Activity"}
                    </button>
                </div>
            </div>
        </>
    }
}
