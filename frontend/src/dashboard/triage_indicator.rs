use yew::prelude::*;

const TRIAGE_STYLES: &str = r#"
.triage-indicator {
    display: flex;
    align-items: center;
    justify-content: space-between;
    background: rgba(30, 144, 255, 0.08);
    border: 1px solid rgba(30, 144, 255, 0.15);
    border-radius: 12px;
    padding: 1rem 1.25rem;
}
.triage-indicator.all-clear {
    background: rgba(76, 175, 80, 0.08);
    border-color: rgba(76, 175, 80, 0.15);
}
.triage-indicator.has-attention {
    background: rgba(255, 193, 7, 0.08);
    border-color: rgba(255, 193, 7, 0.2);
}
.triage-content {
    display: flex;
    align-items: center;
    gap: 0.75rem;
}
.triage-icon {
    font-size: 1.25rem;
    color: #4CAF50;
}
.triage-icon.attention {
    color: #FFC107;
}
.triage-text {
    color: #ddd;
    font-size: 0.95rem;
}
.triage-handle-btn {
    background: linear-gradient(135deg, #1E90FF, #4169E1);
    border: none;
    color: white;
    padding: 0.5rem 1rem;
    border-radius: 6px;
    font-size: 0.9rem;
    cursor: pointer;
    transition: transform 0.2s, box-shadow 0.2s;
}
.triage-handle-btn:hover {
    transform: translateY(-1px);
    box-shadow: 0 4px 12px rgba(30, 144, 255, 0.3);
}
"#;

#[derive(Clone, PartialEq)]
pub struct AttentionItem {
    pub id: i32,
    pub item_type: String,
    pub summary: String,
    pub timestamp: i32,
    pub source: Option<String>,
}

#[derive(Properties, PartialEq, Clone)]
pub struct TriageIndicatorProps {
    pub attention_count: i32,
    pub attention_items: Vec<AttentionItem>,
    #[prop_or_default]
    pub on_handle: Option<Callback<()>>,
}

#[function_component(TriageIndicator)]
pub fn triage_indicator(props: &TriageIndicatorProps) -> Html {
    if props.attention_count == 0 {
        return html! {
            <>
                <style>{TRIAGE_STYLES}</style>
                <div class="triage-indicator all-clear">
                    <div class="triage-content">
                        <span class="triage-icon">{"*"}</span>
                        <span class="triage-text">{"No urgent items"}</span>
                    </div>
                </div>
            </>
        };
    }

    let count = props.attention_count;
    let things_text = if count == 1 { "thing needs" } else { "things need" };

    html! {
        <>
            <style>{TRIAGE_STYLES}</style>
            <div class="triage-indicator has-attention">
                <div class="triage-content">
                    <span class="triage-icon attention">{"!"}</span>
                    <span class="triage-text">
                        {format!("{} {} attention", count, things_text)}
                    </span>
                </div>
                {
                    if let Some(ref on_handle) = props.on_handle {
                        let cb = on_handle.clone();
                        html! {
                            <button
                                class="triage-handle-btn"
                                onclick={Callback::from(move |_| cb.emit(()))}
                            >
                                {"Handle"}
                            </button>
                        }
                    } else {
                        html! {}
                    }
                }
            </div>
        </>
    }
}
