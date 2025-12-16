use yew::prelude::*;

#[derive(Properties, PartialEq, Clone)]
pub struct FeaturePreviewProps {
    /// Path to the GIF file (e.g., "/assets/previews/tesla-controls-preview.gif")
    pub gif_src: AttrValue,
    /// Caption text shown below the GIF
    #[prop_or_default]
    pub caption: Option<AttrValue>,
    /// Badge text (defaults to "Preview")
    #[prop_or(AttrValue::from("Preview"))]
    pub badge_text: AttrValue,
    /// Optional link for the connect button
    #[prop_or_default]
    pub connect_href: Option<AttrValue>,
    /// Optional text for connect button (defaults to "Connect")
    #[prop_or(AttrValue::from("Connect"))]
    pub connect_text: AttrValue,
    /// Maximum width of the GIF (defaults to 400px)
    #[prop_or(400)]
    pub max_width: u32,
}

#[function_component(FeaturePreview)]
pub fn feature_preview(props: &FeaturePreviewProps) -> Html {
    let img_loaded = use_state(|| false);

    let onload = {
        let img_loaded = img_loaded.clone();
        Callback::from(move |_| {
            img_loaded.set(true);
        })
    };

    html! {
        <div class="feature-preview-container">
            <div class="feature-preview">
                <span class="preview-badge">{&props.badge_text}</span>

                {if !*img_loaded {
                    html! {
                        <div class="preview-loading">
                            <i class="fas fa-spinner fa-spin"></i>
                            {" Loading preview..."}
                        </div>
                    }
                } else {
                    html! {}
                }}

                <img
                    src={props.gif_src.clone()}
                    alt="Feature preview"
                    class={classes!("preview-gif", (!*img_loaded).then_some("preview-gif-hidden"))}
                    style={format!("max-width: {}px;", props.max_width)}
                    loading="lazy"
                    {onload}
                />

                {if let Some(caption) = &props.caption {
                    html! { <p class="preview-caption">{caption}</p> }
                } else {
                    html! {}
                }}
            </div>

            {if let Some(href) = &props.connect_href {
                html! {
                    <a href={href.clone()} class="preview-connect-btn">
                        {&props.connect_text}
                    </a>
                }
            } else {
                html! {}
            }}

            <style>{get_styles()}</style>
        </div>
    }
}

fn get_styles() -> &'static str {
    r#"
        .feature-preview-container {
            display: flex;
            flex-direction: column;
            align-items: center;
            gap: 1rem;
        }

        .feature-preview {
            position: relative;
            background: rgba(20, 20, 20, 0.8);
            border: 1px solid rgba(30, 144, 255, 0.2);
            border-radius: 12px;
            overflow: hidden;
            padding: 1rem;
            width: 100%;
            max-width: 450px;
        }

        .preview-badge {
            position: absolute;
            top: 12px;
            right: 12px;
            background: rgba(30, 144, 255, 0.85);
            color: white;
            padding: 4px 12px;
            border-radius: 20px;
            font-size: 11px;
            font-weight: 600;
            text-transform: uppercase;
            letter-spacing: 0.5px;
            z-index: 1;
        }

        .preview-loading {
            display: flex;
            align-items: center;
            justify-content: center;
            gap: 8px;
            color: #7EB2FF;
            padding: 3rem;
            font-size: 14px;
        }

        .preview-gif {
            width: 100%;
            border-radius: 8px;
            display: block;
            margin: 0 auto;
            transition: opacity 0.3s ease;
        }

        .preview-gif-hidden {
            opacity: 0;
            height: 0;
            padding: 0;
            margin: 0;
        }

        .preview-caption {
            text-align: center;
            color: #999;
            font-size: 13px;
            margin: 0.75rem 0 0 0;
            line-height: 1.4;
        }

        .preview-connect-btn {
            display: inline-block;
            background: linear-gradient(45deg, #1E90FF, #4169E1);
            color: white;
            padding: 0.75rem 1.5rem;
            border-radius: 8px;
            text-decoration: none;
            font-size: 14px;
            font-weight: 500;
            transition: all 0.3s ease;
        }

        .preview-connect-btn:hover {
            transform: translateY(-2px);
            box-shadow: 0 4px 20px rgba(30, 144, 255, 0.3);
        }

        @media (max-width: 480px) {
            .feature-preview {
                padding: 0.75rem;
            }

            .preview-badge {
                top: 8px;
                right: 8px;
                padding: 3px 10px;
                font-size: 10px;
            }
        }
    "#
}
