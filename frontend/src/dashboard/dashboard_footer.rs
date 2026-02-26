use yew::prelude::*;

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
}

#[function_component(DashboardFooter)]
pub fn dashboard_footer(props: &DashboardFooterProps) -> Html {
    let digest_html = match &props.next_digest {
        Some(info) => {
            html! { <div class="footer-digest">{format!("Next digest: {}", info.time_display)}</div> }
        }
        None => {
            html! { <div class="footer-digest">{"No digest scheduled"}</div> }
        }
    };

    html! {
        <>
            <style>{FOOTER_STYLES}</style>
            <div class="peace-footer">
                <div class="footer-info">
                    {digest_html}
                </div>
            </div>
        </>
    }
}
