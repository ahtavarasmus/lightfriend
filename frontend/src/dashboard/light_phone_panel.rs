use crate::utils::api::Api;
use chrono::DateTime;
use gloo_timers::callback::Interval;
use qrcodegen::{QrCode, QrCodeEcc};
use serde::Deserialize;
use std::{cell::Cell, rc::Rc};
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;

const LIGHT_PHONE_STYLES: &str = r#"
.light-phone-pairing {
    display: flex;
    flex-direction: column;
    gap: 1rem;
}
.light-phone-copy {
    color: #999;
    font-size: 0.85rem;
    line-height: 1.5;
    margin: 0;
}
.light-phone-actions {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    flex-wrap: wrap;
}
.light-phone-button {
    min-height: 38px;
    padding: 0.55rem 0.9rem;
    border: 1px solid rgba(126, 178, 255, 0.35);
    border-radius: 6px;
    background: rgba(126, 178, 255, 0.14);
    color: #b9d3ff;
    cursor: pointer;
    font-size: 0.82rem;
    font-weight: 500;
}
.light-phone-button:hover:not(:disabled) {
    background: rgba(126, 178, 255, 0.22);
}
.light-phone-button:disabled {
    cursor: wait;
    opacity: 0.55;
}
.light-phone-qr-wrap {
    width: min(100%, 280px);
    align-self: center;
    padding: 14px;
    background: #fff;
    border-radius: 6px;
}
.light-phone-qr {
    display: block;
    width: 100%;
    aspect-ratio: 1;
}
.light-phone-expiry {
    color: #aaa;
    font-size: 0.78rem;
    font-variant-numeric: tabular-nums;
}
.light-phone-expiry.expired {
    color: #f3b36a;
}
.light-phone-connected {
    color: #78d69b;
    background: rgba(52, 168, 94, 0.08);
    border: 1px solid rgba(52, 168, 94, 0.24);
    border-radius: 4px;
    padding: 0.65rem 0.75rem;
    font-size: 0.85rem;
}
.light-phone-error {
    color: #f88;
    background: rgba(220, 50, 50, 0.08);
    border: 1px solid rgba(220, 50, 50, 0.2);
    border-radius: 4px;
    padding: 0.55rem 0.65rem;
    font-size: 0.78rem;
}
"#;

#[derive(Deserialize)]
struct PairingOfferResponse {
    pairing_uri: String,
    expires_at: String,
}

#[derive(Deserialize)]
struct ErrorResponse {
    error: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum PairingStatus {
    None,
    Pending,
    Connected,
    Expired,
}

#[derive(Deserialize)]
struct PairingStatusResponse {
    status: PairingStatus,
}

#[derive(Clone, PartialEq)]
struct PairingOffer {
    pairing_uri: String,
    expires_at: i64,
}

fn qr_code(value: &str) -> Result<QrCode, ()> {
    QrCode::encode_text(value, QrCodeEcc::Medium).map_err(|_| ())
}

fn qr_svg(value: &str) -> Html {
    let Ok(code) = qr_code(value) else {
        return html! { <div class="light-phone-error">{"Could not render this pairing code."}</div> };
    };
    let border = 4;
    let size = code.size();
    let view_size = size + border * 2;
    let modules = (0..size).flat_map(|y| {
        let code = &code;
        (0..size).filter_map(move |x| {
            code.get_module(x, y).then(|| {
                html! {
                    <rect
                        x={(x + border).to_string()}
                        y={(y + border).to_string()}
                        width="1"
                        height="1"
                        fill="#000"
                    />
                }
            })
        })
    });

    html! {
        <svg
            class="light-phone-qr"
            viewBox={format!("0 0 {view_size} {view_size}")}
            role="img"
            aria-label="Light Phone pairing QR code"
            shape-rendering="crispEdges"
        >
            <rect width={view_size.to_string()} height={view_size.to_string()} fill="#fff" />
            {for modules}
        </svg>
    }
}

fn format_remaining(seconds: i64) -> String {
    let seconds = seconds.max(0);
    format!("{:02}:{:02}", seconds / 60, seconds % 60)
}

#[function_component(LightPhonePanel)]
pub fn light_phone_panel() -> Html {
    let offer = use_state(|| None::<PairingOffer>);
    let loading = use_state(|| false);
    let error = use_state(|| None::<String>);
    let now = use_state(|| chrono::Utc::now().timestamp());
    let connected = use_state(|| false);
    let server_expired = use_state(|| false);
    let poll_epoch = use_mut_ref(|| 0_u64);

    {
        let now = now.clone();
        use_effect_with_deps(
            move |_| {
                let interval = Interval::new(1_000, move || {
                    now.set(chrono::Utc::now().timestamp());
                });
                move || drop(interval)
            },
            (),
        );
    }

    {
        let offer_dependency = (*offer).clone();
        let offer = offer.clone();
        let connected = connected.clone();
        let server_expired = server_expired.clone();
        let poll_epoch = poll_epoch.clone();
        use_effect_with_deps(
            move |active_offer| {
                let interval = active_offer.as_ref().map(|_| {
                    let active_epoch = *poll_epoch.borrow();
                    let request_in_flight = Rc::new(Cell::new(false));
                    Interval::new(1_500, move || {
                        if request_in_flight.replace(true) {
                            return;
                        }
                        let request_in_flight = request_in_flight.clone();
                        let offer = offer.clone();
                        let connected = connected.clone();
                        let server_expired = server_expired.clone();
                        let poll_epoch = poll_epoch.clone();
                        spawn_local(async move {
                            if let Ok(response) =
                                Api::get("/api/me/light-tool/pairing-sessions").send().await
                            {
                                if response.ok() {
                                    if let Ok(response) =
                                        response.json::<PairingStatusResponse>().await
                                    {
                                        if *poll_epoch.borrow() != active_epoch {
                                            request_in_flight.set(false);
                                            return;
                                        }
                                        match response.status {
                                            PairingStatus::Connected => {
                                                connected.set(true);
                                                server_expired.set(false);
                                                offer.set(None);
                                            }
                                            PairingStatus::Expired | PairingStatus::None => {
                                                server_expired.set(true);
                                                offer.set(None);
                                            }
                                            PairingStatus::Pending => {}
                                        }
                                    }
                                }
                            }
                            request_in_flight.set(false);
                        });
                    })
                });
                move || drop(interval)
            },
            offer_dependency,
        );
    }

    let generate = {
        let offer = offer.clone();
        let loading = loading.clone();
        let error = error.clone();
        let now = now.clone();
        let connected = connected.clone();
        let server_expired = server_expired.clone();
        let poll_epoch = poll_epoch.clone();
        Callback::from(move |_: MouseEvent| {
            *poll_epoch.borrow_mut() += 1;
            offer.set(None);
            error.set(None);
            loading.set(true);
            connected.set(false);
            server_expired.set(false);

            let offer = offer.clone();
            let loading = loading.clone();
            let error = error.clone();
            let now = now.clone();
            spawn_local(async move {
                match Api::post("/api/me/light-tool/pairing-sessions")
                    .send()
                    .await
                {
                    Ok(response) if response.ok() => {
                        match response.json::<PairingOfferResponse>().await {
                            Ok(response) => {
                                let expires_at = DateTime::parse_from_rfc3339(&response.expires_at)
                                    .map(|date| date.timestamp());
                                if expires_at.is_err() || qr_code(&response.pairing_uri).is_err() {
                                    error.set(Some(
                                        "The server returned an invalid pairing code.".to_string(),
                                    ));
                                } else {
                                    now.set(chrono::Utc::now().timestamp());
                                    offer.set(Some(PairingOffer {
                                        pairing_uri: response.pairing_uri,
                                        expires_at: expires_at.unwrap(),
                                    }));
                                }
                            }
                            Err(_) => {
                                error.set(Some("Could not read the server response.".to_string()))
                            }
                        }
                    }
                    Ok(response) => {
                        let status = response.status();
                        let message = response
                            .json::<ErrorResponse>()
                            .await
                            .ok()
                            .map(|body| body.error)
                            .unwrap_or_else(|| {
                                format!("Could not create a pairing code ({status}).")
                            });
                        error.set(Some(message));
                    }
                    Err(_) => {
                        error.set(Some("Network error creating the pairing code.".to_string()))
                    }
                }
                loading.set(false);
            });
        })
    };

    let pairing_content = if *connected {
        html! {
            <>
                <div class="light-phone-connected" role="status">
                    {"Light Phone connected."}
                </div>
                <button class="light-phone-button" onclick={generate.clone()} disabled={*loading}>
                    {if *loading { "Creating..." } else { "Pair another Light Phone" }}
                </button>
            </>
        }
    } else if *server_expired {
        html! {
            <>
                <span class="light-phone-expiry expired">{"This pairing code has expired."}</span>
                <button class="light-phone-button" onclick={generate.clone()} disabled={*loading}>
                    {if *loading { "Creating..." } else { "Create new code" }}
                </button>
            </>
        }
    } else if let Some(current_offer) = offer.as_ref() {
        let remaining = current_offer.expires_at - *now;
        if remaining > 0 {
            html! {
                <>
                    <p class="light-phone-copy">{"Open the Lightfriend tool on your Light Phone and scan this code."}</p>
                    <div class="light-phone-qr-wrap">
                        {qr_svg(&current_offer.pairing_uri)}
                    </div>
                    <div class="light-phone-actions">
                        <span class="light-phone-expiry">
                            {format!("Expires in {}", format_remaining(remaining))}
                        </span>
                        <button class="light-phone-button" onclick={generate.clone()} disabled={*loading}>
                            {"New code"}
                        </button>
                    </div>
                </>
            }
        } else {
            html! {
                <>
                    <span class="light-phone-expiry expired">{"This pairing code has expired."}</span>
                    <button class="light-phone-button" onclick={generate.clone()} disabled={*loading}>
                        {if *loading { "Creating..." } else { "Create new code" }}
                    </button>
                </>
            }
        }
    } else {
        html! {
            <>
                <p class="light-phone-copy">
                    {"Create a one-time code to connect the Lightfriend tool to this account."}
                </p>
                <div class="light-phone-actions">
                    <button class="light-phone-button" onclick={generate} disabled={*loading}>
                        {if *loading { "Creating..." } else { "Create pairing code" }}
                    </button>
                </div>
            </>
        }
    };

    html! {
        <>
            <style>{LIGHT_PHONE_STYLES}</style>
            <div class="light-phone-pairing">
                {pairing_content}
                {
                    error.as_ref().map(|message| html! {
                        <div class="light-phone-error" role="alert">{message}</div>
                    }).unwrap_or_default()
                }
            </div>
        </>
    }
}
