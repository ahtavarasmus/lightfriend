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
.footer-btn-digest {
    background: rgba(245, 158, 11, 0.08);
    border: 1px solid rgba(245, 158, 11, 0.25);
    color: #e8a838;
    padding: 0.5rem 1.25rem;
    border-radius: 6px;
    font-size: 0.85rem;
    cursor: pointer;
    transition: all 0.2s ease;
}
.footer-btn-digest:hover {
    background: rgba(245, 158, 11, 0.15);
    border-color: rgba(245, 158, 11, 0.4);
    color: #f5b041;
}
"#;

#[derive(Clone, PartialEq)]
pub struct NextDigestInfo {
    pub time_display: String,
}

#[derive(Properties, PartialEq, Clone)]
pub struct DashboardFooterProps {
    pub next_digest: Option<NextDigestInfo>,
    pub quiet_mode: QuietModeStatus,
    pub on_activity_click: Callback<()>,
    #[prop_or_default]
    pub on_quiet_mode_change: Option<Callback<()>>,
    #[prop_or_default]
    pub on_digest_suggestion: Option<Callback<()>>,
}

#[function_component(DashboardFooter)]
pub fn dashboard_footer(props: &DashboardFooterProps) -> Html {
    let digest_html = match &props.next_digest {
        Some(info) => {
            html! { <div class="footer-digest">{format!("Next digest: {}", info.time_display)}</div> }
        }
        None => {
            if let Some(ref cb) = props.on_digest_suggestion {
                let cb = cb.clone();
                html! {
                    <button
                        class="footer-btn-digest"
                        onclick={Callback::from(move |_| cb.emit(()))}
                    >
                        {"Set up a daily digest"}
                    </button>
                }
            } else {
                html! { <div class="footer-digest">{"No digest scheduled"}</div> }
            }
        }
    };

    html! {
        <>
            <style>{FOOTER_STYLES}</style>
            <div class="peace-footer">
                <div class="footer-info">
                    {digest_html}
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
