use std::collections::BTreeSet;

use serde::Deserialize;
use wasm_bindgen_futures::spawn_local;
use web_sys::{HtmlInputElement, HtmlSelectElement};
use yew::prelude::*;

use crate::utils::api::Api;

#[derive(Clone, PartialEq, Deserialize, Debug)]
struct Contact {
    id: String,
    display_name: String,
    #[serde(default)]
    subtitle: Option<String>,
    #[serde(default)]
    platform: Option<String>,
    #[serde(default)]
    is_group: bool,
    source: String,
}

fn score_contact(query_lower: &str, contact: &Contact) -> Option<i32> {
    let name = contact.display_name.to_lowercase();
    if name == query_lower {
        Some(3)
    } else if name.starts_with(query_lower) {
        Some(2)
    } else if name.contains(query_lower) {
        Some(1)
    } else {
        None
    }
}

const ALWAYS_SHOW_STYLES: &str = r#"
.always-show-page {
    display: flex;
    flex-direction: column;
    gap: 1.25rem;
}
.always-show-page h3 {
    margin: 0;
    text-wrap: balance;
}
.always-show-intro,
.always-show-empty,
.always-show-hint {
    color: #888;
    font-size: 0.82rem;
    line-height: 1.55;
    margin: 0;
    text-wrap: pretty;
}
.always-show-form {
    display: flex;
    flex-direction: column;
    gap: 0.65rem;
    padding: 1rem;
    border: 1px solid rgba(255, 255, 255, 0.09);
    border-radius: 10px;
    background: rgba(255, 255, 255, 0.025);
}
.always-show-field-label {
    color: #aaa;
    font-size: 0.72rem;
    font-weight: 600;
    letter-spacing: 0.04em;
    text-transform: uppercase;
}
.always-show-select,
.always-show-input {
    width: 100%;
    min-height: 42px;
    box-sizing: border-box;
    padding: 0.6rem 0.7rem;
    border: 1px solid rgba(255, 255, 255, 0.13);
    border-radius: 8px;
    background: #121212;
    color: #ddd;
    font: inherit;
    font-size: 0.86rem;
}
.always-show-select:focus-visible,
.always-show-input:focus-visible,
.always-show-add:focus-visible,
.always-show-result:focus-visible,
.always-show-remove:focus-visible {
    outline: 2px solid rgba(126, 178, 255, 0.75);
    outline-offset: 2px;
}
.always-show-combobox {
    position: relative;
}
.always-show-results {
    position: absolute;
    z-index: 20;
    top: calc(100% + 4px);
    left: 0;
    right: 0;
    max-height: 220px;
    overflow-y: auto;
    border: 1px solid rgba(255, 255, 255, 0.14);
    border-radius: 8px;
    background: #202020;
    box-shadow: 0 12px 28px rgba(0, 0, 0, 0.28);
}
.always-show-result {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.75rem;
    width: 100%;
    min-height: 44px;
    padding: 0.55rem 0.7rem;
    border: 0;
    border-bottom: 1px solid rgba(255, 255, 255, 0.06);
    background: transparent;
    color: #ddd;
    font: inherit;
    text-align: left;
    cursor: pointer;
}
.always-show-result:last-child { border-bottom: 0; }
.always-show-result:hover { background: rgba(126, 178, 255, 0.12); }
.always-show-result-name { font-size: 0.84rem; }
.always-show-result-meta { color: #777; font-size: 0.7rem; }
.always-show-no-results {
    padding: 0.75rem;
    color: #777;
    font-size: 0.78rem;
}
.always-show-add {
    align-self: flex-start;
    min-height: 40px;
    padding: 0.55rem 0.9rem;
    border: 1px solid rgba(126, 178, 255, 0.35);
    border-radius: 8px;
    background: rgba(126, 178, 255, 0.11);
    color: #9ec5ff;
    font: inherit;
    font-size: 0.82rem;
    cursor: pointer;
    transition: background 160ms ease, border-color 160ms ease, transform 160ms ease;
}
.always-show-add:hover:not(:disabled) {
    border-color: rgba(126, 178, 255, 0.55);
    background: rgba(126, 178, 255, 0.17);
}
.always-show-add:active:not(:disabled),
.always-show-remove:active:not(:disabled) { transform: scale(0.98); }
.always-show-add:disabled { cursor: not-allowed; opacity: 0.45; }
.always-show-error {
    margin: 0;
    color: #ff8a8a;
    font-size: 0.78rem;
    line-height: 1.45;
}
.always-show-list {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
}
.always-show-row {
    display: flex;
    align-items: center;
    gap: 0.8rem;
    min-height: 52px;
    padding: 0.65rem 0.75rem;
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 9px;
    background: rgba(255, 255, 255, 0.025);
}
.always-show-row-copy { flex: 1; min-width: 0; }
.always-show-row-title {
    color: #ddd;
    font-size: 0.86rem;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
}
.always-show-row-platform {
    margin-top: 0.15rem;
    color: #777;
    font-size: 0.7rem;
    text-transform: capitalize;
}
.always-show-remove {
    min-width: 40px;
    min-height: 36px;
    padding: 0.35rem 0.55rem;
    border: 0;
    border-radius: 7px;
    background: transparent;
    color: #888;
    font: inherit;
    font-size: 0.74rem;
    cursor: pointer;
    transition: background 160ms ease, color 160ms ease, transform 160ms ease;
}
.always-show-remove:hover:not(:disabled) {
    background: rgba(255, 92, 92, 0.1);
    color: #ff8a8a;
}
.always-show-remove:disabled { cursor: wait; opacity: 0.45; }
@media (prefers-color-scheme: light) {
    .always-show-form,
    .always-show-row { border-color: rgba(0, 0, 0, 0.09); background: rgba(0, 0, 0, 0.02); }
    .always-show-select,
    .always-show-input { border-color: rgba(0, 0, 0, 0.14); background: #fff; color: #222; }
    .always-show-results { border-color: rgba(0, 0, 0, 0.14); background: #fff; }
    .always-show-result { color: #333; border-bottom-color: rgba(0, 0, 0, 0.06); }
    .always-show-row-title { color: #333; }
}
@media (prefers-reduced-motion: reduce) {
    .always-show-add,
    .always-show-remove { transition: none; }
}
"#;

#[derive(Clone, Debug, Deserialize, PartialEq)]
struct AlwaysShowEntry {
    id: i32,
    platform: String,
    display_name: String,
    subtitle: String,
}

fn platform_supports_mentions(platform: &str) -> bool {
    platform == "whatsapp"
}

fn platform_label(platform: &str) -> String {
    match platform {
        "whatsapp" => "WhatsApp".to_string(),
        "signal" => "Signal".to_string(),
        "telegram" => "Telegram".to_string(),
        "email" => "Email".to_string(),
        other => {
            let mut chars = other.chars();
            chars
                .next()
                .map(|first| first.to_uppercase().collect::<String>() + chars.as_str())
                .unwrap_or_default()
        }
    }
}

#[function_component(AlwaysShowSettings)]
pub fn always_show_settings() -> Html {
    let entries = use_state(Vec::<AlwaysShowEntry>::new);
    let contacts = use_state(Vec::<Contact>::new);
    let loading = use_state(|| true);
    let saving = use_state(|| false);
    let removing = use_state(|| None::<i32>);
    let error = use_state(|| None::<String>);
    let source = use_state(|| "email".to_string());
    let query = use_state(String::new);
    let selected_contact = use_state(|| None::<Contact>);
    let results_open = use_state(|| false);
    let group_mode = use_state(|| "all".to_string());

    {
        let entries = entries.clone();
        let contacts = contacts.clone();
        let loading = loading.clone();
        let error = error.clone();
        use_effect_with_deps(
            move |_| {
                spawn_local(async move {
                    let entries_result = Api::get("/api/always-show").send().await;
                    let contacts_result = Api::get("/api/dashboard/contacts").send().await;

                    match entries_result {
                        Ok(response) if response.ok() => {
                            match response.json::<Vec<AlwaysShowEntry>>().await {
                                Ok(data) => entries.set(data),
                                Err(_) => error
                                    .set(Some("Couldn't read your always-show list.".to_string())),
                            }
                        }
                        _ => error.set(Some("Couldn't load your always-show list.".to_string())),
                    }
                    if let Ok(response) = contacts_result {
                        if response.ok() {
                            if let Ok(data) = response.json::<Vec<Contact>>().await {
                                contacts.set(data);
                            }
                        }
                    }
                    loading.set(false);
                });
                || ()
            },
            (),
        );
    }

    let platforms: BTreeSet<String> = contacts
        .iter()
        .filter(|contact| contact.source != "person")
        .filter_map(|contact| contact.platform.clone())
        .filter(|platform| platform != "email")
        .collect();

    let normalized_query = query.trim().to_lowercase();
    let mut filtered_contacts: Vec<(i32, Contact)> = contacts
        .iter()
        .filter(|contact| contact.source != "person")
        .filter(|contact| contact.platform.as_deref() == Some(source.as_str()))
        .filter_map(|contact| {
            if normalized_query.is_empty() {
                Some((0, contact.clone()))
            } else {
                score_contact(&normalized_query, contact).map(|score| (score, contact.clone()))
            }
        })
        .collect();
    filtered_contacts.sort_by(|(score_a, a), (score_b, b)| {
        score_b.cmp(score_a).then(
            a.display_name
                .to_lowercase()
                .cmp(&b.display_name.to_lowercase()),
        )
    });
    filtered_contacts.truncate(50);

    let can_add = !*saving
        && if source.as_str() == "email" {
            let value = query.trim();
            value.contains('@')
                && value
                    .split('@')
                    .nth(1)
                    .is_some_and(|domain| domain.contains('.'))
        } else {
            selected_contact.is_some()
        };
    let selected_is_group = selected_contact
        .as_ref()
        .is_some_and(|contact| contact.is_group);
    let mentions_supported = selected_is_group && platform_supports_mentions(source.as_str());

    let on_add = {
        let source = source.clone();
        let query = query.clone();
        let selected_contact = selected_contact.clone();
        let entries = entries.clone();
        let saving = saving.clone();
        let error = error.clone();
        let results_open = results_open.clone();
        let group_mode = group_mode.clone();
        Callback::from(move |_: MouseEvent| {
            if *saving {
                return;
            }
            let request = if source.as_str() == "email" {
                serde_json::json!({ "kind": "email", "email": query.trim() })
            } else if let Some(contact) = selected_contact.as_ref() {
                serde_json::json!({
                    "kind": "platform",
                    "contact_id": contact.id,
                    "group_mode": contact.is_group.then(|| (*group_mode).clone()),
                })
            } else {
                return;
            };

            saving.set(true);
            error.set(None);
            let entries = entries.clone();
            let saving = saving.clone();
            let error = error.clone();
            let query = query.clone();
            let selected_contact = selected_contact.clone();
            let results_open = results_open.clone();
            let group_mode = group_mode.clone();
            spawn_local(async move {
                let response = match Api::post("/api/always-show").json(&request) {
                    Ok(request) => request.send().await,
                    Err(_) => {
                        error.set(Some("Couldn't prepare this entry.".to_string()));
                        saving.set(false);
                        return;
                    }
                };
                match response {
                    Ok(response) if response.ok() => {
                        if let Ok(entry) = response.json::<AlwaysShowEntry>().await {
                            let mut next = (*entries).clone();
                            if let Some(existing) =
                                next.iter_mut().find(|existing| existing.id == entry.id)
                            {
                                *existing = entry;
                            } else {
                                next.insert(0, entry);
                            }
                            entries.set(next);
                            query.set(String::new());
                            selected_contact.set(None);
                            results_open.set(false);
                            group_mode.set("all".to_string());
                        }
                    }
                    Ok(response) => {
                        let message = response
                            .json::<serde_json::Value>()
                            .await
                            .ok()
                            .and_then(|body| {
                                body.get("error")
                                    .and_then(|value| value.as_str())
                                    .map(str::to_string)
                            })
                            .unwrap_or_else(|| "Couldn't save this entry.".to_string());
                        error.set(Some(message));
                    }
                    Err(_) => error.set(Some("Couldn't save this entry.".to_string())),
                }
                saving.set(false);
            });
        })
    };

    html! {
        <>
            <style>{ALWAYS_SHOW_STYLES}</style>
            <section class="always-show-page" aria-labelledby="always-show-title">
                <div>
                    <h3 id="always-show-title">{"Always show messages from…"}</h3>
                    <p class="always-show-intro">
                        {"Messages from this list skip the normal importance filter and are delivered right away."}
                    </p>
                </div>

                <div class="always-show-form">
                    <label class="always-show-field-label" for="always-show-source">{"Source"}</label>
                    <select
                        id="always-show-source"
                        class="always-show-select"
                        onchange={{
                            let source = source.clone();
                            let query = query.clone();
                            let selected_contact = selected_contact.clone();
                            let results_open = results_open.clone();
                            let group_mode = group_mode.clone();
                            Callback::from(move |event: Event| {
                                if let Some(select) = event.target_dyn_into::<HtmlSelectElement>() {
                                    source.set(select.value());
                                    query.set(String::new());
                                    selected_contact.set(None);
                                    results_open.set(false);
                                    group_mode.set("all".to_string());
                                }
                            })
                        }}
                    >
                        <option value="email" selected={source.as_str() == "email"}>{"Email"}</option>
                        { for platforms.iter().map(|platform| html! {
                            <option
                                value={platform.clone()}
                                selected={source.as_str() == platform.as_str()}
                            >
                                {platform_label(platform)}
                            </option>
                        }) }
                    </select>

                    if source.as_str() == "email" {
                        <label class="always-show-field-label" for="always-show-email">{"Email address"}</label>
                        <input
                            id="always-show-email"
                            class="always-show-input"
                            type="email"
                            autocomplete="email"
                            placeholder="person@example.com"
                            value={(*query).clone()}
                            oninput={{
                                let query = query.clone();
                                Callback::from(move |event: InputEvent| {
                                    if let Some(input) = event.target_dyn_into::<HtmlInputElement>() {
                                        query.set(input.value());
                                    }
                                })
                            }}
                        />
                    } else {
                        <label class="always-show-field-label" for="always-show-contact">{"Person or chat"}</label>
                        <div class="always-show-combobox">
                            <input
                                id="always-show-contact"
                                class="always-show-input"
                                type="text"
                                role="combobox"
                                aria-autocomplete="list"
                                aria-expanded={(*results_open).to_string()}
                                aria-controls="always-show-results"
                                autocomplete="off"
                                placeholder={format!("Search {} contacts and chats", platform_label(source.as_str()))}
                                value={(*query).clone()}
                                onfocus={{
                                    let results_open = results_open.clone();
                                    Callback::from(move |_| results_open.set(true))
                                }}
                                oninput={{
                                    let query = query.clone();
                                    let selected_contact = selected_contact.clone();
                                    let results_open = results_open.clone();
                                    let group_mode = group_mode.clone();
                                    Callback::from(move |event: InputEvent| {
                                        if let Some(input) = event.target_dyn_into::<HtmlInputElement>() {
                                            query.set(input.value());
                                            selected_contact.set(None);
                                            results_open.set(true);
                                            group_mode.set("all".to_string());
                                        }
                                    })
                                }}
                            />
                            if *results_open {
                                <div id="always-show-results" class="always-show-results" role="listbox">
                                    if filtered_contacts.is_empty() {
                                        <div class="always-show-no-results">{"No matching known contacts or chats."}</div>
                                    } else {
                                        { for filtered_contacts.iter().map(|(_, contact)| {
                                            let picked = contact.clone();
                                            let picked_name = contact.display_name.clone();
                                            let subtitle = contact.subtitle.clone().unwrap_or_else(|| platform_label(source.as_str()));
                                            html! {
                                                <button
                                                    type="button"
                                                    class="always-show-result"
                                                    role="option"
                                                    onclick={{
                                                        let query = query.clone();
                                                        let selected_contact = selected_contact.clone();
                                                        let results_open = results_open.clone();
                                                        let group_mode = group_mode.clone();
                                                        Callback::from(move |_| {
                                                            query.set(picked_name.clone());
                                                            selected_contact.set(Some(picked.clone()));
                                                            results_open.set(false);
                                                            group_mode.set("all".to_string());
                                                        })
                                                    }}
                                                >
                                                    <span class="always-show-result-name">{contact.display_name.clone()}</span>
                                                    <span class="always-show-result-meta">{subtitle}</span>
                                                </button>
                                            }
                                        }) }
                                    }
                                </div>
                            }
                        </div>
                        <p class="always-show-hint">{"Choose a result from the list. Typed names are not saved."}</p>
                        if selected_is_group {
                            <label class="always-show-field-label" for="always-show-group-mode">{"Group delivery"}</label>
                            <select
                                id="always-show-group-mode"
                                class="always-show-select"
                                onchange={{
                                    let group_mode = group_mode.clone();
                                    Callback::from(move |event: Event| {
                                        if let Some(select) = event.target_dyn_into::<HtmlSelectElement>() {
                                            group_mode.set(select.value());
                                        }
                                    })
                                }}
                            >
                                <option value="all" selected={group_mode.as_str() == "all"}>{"All messages"}</option>
                                if mentions_supported {
                                    <option
                                        value="mention_only"
                                        selected={group_mode.as_str() == "mention_only"}
                                    >
                                        {"Mentions only"}
                                    </option>
                                }
                            </select>
                            if !mentions_supported {
                                <p class="always-show-hint">
                                    {"Reliable mention tags are not available for this platform, so only all messages can be selected."}
                                </p>
                            }
                        }
                    }

                    <button type="button" class="always-show-add" disabled={!can_add} onclick={on_add}>
                        {if *saving { "Adding…" } else { "Add to always show" }}
                    </button>
                    if let Some(message) = error.as_ref() {
                        <p class="always-show-error" role="alert">{message}</p>
                    }
                </div>

                <div class="always-show-list" aria-live="polite">
                    if *loading {
                        <p class="always-show-empty">{"Loading…"}</p>
                    } else if entries.is_empty() {
                        <p class="always-show-empty">{"No one is on this list yet."}</p>
                    } else {
                        { for entries.iter().map(|entry| {
                            let id = entry.id;
                            let entries = entries.clone();
                            let removing = removing.clone();
                            let removing_for_click = removing.clone();
                            let error = error.clone();
                            html! {
                                <div class="always-show-row">
                                    <div class="always-show-row-copy">
                                        <div class="always-show-row-title">{entry.display_name.clone()}</div>
                                        <div class="always-show-row-platform">{entry.subtitle.clone()}</div>
                                    </div>
                                    <button
                                        type="button"
                                        class="always-show-remove"
                                        aria-label={format!("Remove {}", entry.display_name)}
                                        disabled={*removing == Some(id)}
                                        onclick={Callback::from(move |_| {
                                            removing_for_click.set(Some(id));
                                            error.set(None);
                                            let entries = entries.clone();
                                            let removing = removing_for_click.clone();
                                            let error = error.clone();
                                            spawn_local(async move {
                                                match Api::delete(&format!("/api/always-show/{}", id)).send().await {
                                                    Ok(response) if response.ok() => {
                                                        let next = entries.iter().filter(|entry| entry.id != id).cloned().collect();
                                                        entries.set(next);
                                                    }
                                                    _ => error.set(Some("Couldn't remove this entry.".to_string())),
                                                }
                                                removing.set(None);
                                            });
                                        })}
                                    >
                                        {if *removing == Some(id) { "…" } else { "Remove" }}
                                    </button>
                                </div>
                            }
                        }) }
                    }
                </div>
            </section>
        </>
    }
}
