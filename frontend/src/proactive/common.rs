use yew::prelude::*;
use gloo_net::http::Request;
use wasm_bindgen_futures::spawn_local;
use web_sys::{window, js_sys, HtmlInputElement, KeyboardEvent, InputEvent, Event};
use wasm_bindgen::JsValue;
use crate::config;
use crate::pages::proactive::{PrioritySender, ImportancePriority};
use serde_json::json;

pub(crate) fn format_timestamp(ts: i32) -> String {
    let date = js_sys::Date::new(&js_sys::Number::from(ts as f64 * 1000.0));
    let opts = js_sys::Object::new();
    js_sys::Reflect::set(&opts, &JsValue::from_str("year"),  &JsValue::from_str("numeric")).unwrap();
    js_sys::Reflect::set(&opts, &JsValue::from_str("month"), &JsValue::from_str("long")).unwrap();
    js_sys::Reflect::set(&opts, &JsValue::from_str("day"),   &JsValue::from_str("numeric")).unwrap();
    date.to_locale_string("en-US", &opts).into()
}

#[derive(Properties, PartialEq, Clone)]
pub struct KeywordsProps {
    pub service_type: String,
    pub keywords: Vec<String>,
    pub on_change: Callback<Vec<String>>,
}

#[function_component(KeywordsSection)]
pub fn keywords_section(props: &KeywordsProps) -> Html {
    let new_kw = use_state(|| String::new());
    let keywords_local = use_state(|| props.keywords.clone());

    {
        let keywords_local = keywords_local.clone();
        let props_keywords = props.keywords.clone();
        use_effect_with_deps(
            move |_| { keywords_local.set(props_keywords); || () },
            props.keywords.clone(),
        );
    }

    let refresh_from_server = {
        let stype = props.service_type.clone();
        let kw_loc = keywords_local.clone();
        let on_par = props.on_change.clone();
        Callback::from(move |_| {
            if let Some(token) = window()
                .and_then(|w| w.local_storage().ok())
                .flatten()
                .and_then(|s| s.get_item("token").ok())
                .flatten()
            {
                let stype = stype.clone();
                let kw_loc = kw_loc.clone();
                let on_par = on_par.clone();
                spawn_local(async move {
                    if let Ok(resp) = Request::get(&format!(
                        "{}/api/filters/keywords/{}",
                        crate::config::get_backend_url(),
                        stype
                    ))
                    .header("Authorization", &format!("Bearer {}", token))
                    .send()
                    .await
                    {
                        if let Ok(list) = resp.json::<Vec<String>>().await {
                            kw_loc.set(list.clone());
                            on_par.emit(list);
                        }
                    }
                });
            }
        })
    };

    let add_keyword = {
        let stype = props.service_type.clone();
        let new_kw = new_kw.clone();
        let reload = refresh_from_server.clone();
        Callback::from(move |_| {
            let kw = (*new_kw).trim().to_string();
            if kw.is_empty() { return; }
            if let Some(token) = window()
                .and_then(|w| w.local_storage().ok())
                .flatten()
                .and_then(|s| s.get_item("token").ok())
                .flatten()
            {
                let stype = stype.clone();
                let reload = reload.clone();
                let new_kw = new_kw.clone();
                spawn_local(async move {
                    let _ = Request::post(&format!(
                            "{}/api/filters/keyword/{}",
                            crate::config::get_backend_url(), stype
                        ))
                        .header("Authorization", &format!("Bearer {}", token))
                        .json(&json!({ "keyword": kw, "service_type": stype }))
                        .unwrap()
                        .send()
                        .await;
                    new_kw.set(String::new());
                    reload.emit(());
                });
            }
        })
    };

    let del_keyword = {
        let stype = props.service_type.clone();
        let reload = refresh_from_server.clone();
        Callback::from(move |kw_to_del: String| {
            if let Some(token) = window()
                .and_then(|w| w.local_storage().ok())
                .flatten()
                .and_then(|s| s.get_item("token").ok())
                .flatten()
            {
                let stype = stype.clone();
                let reload = reload.clone();
                spawn_local(async move {
                    let _ = Request::delete(&format!(
                            "{}/api/filters/keyword/{}/{}",
                            crate::config::get_backend_url(), stype, kw_to_del
                        ))
                        .header("Authorization", &format!("Bearer {}", token))
                        .send()
                        .await;
                    reload.emit(());
                });
            }
        })
    };

    html! {
        <div class="filter-section">
            <h3>{"Keywords"}</h3>

            <div class="keyword-input">
                <input
                    type="text"
                    placeholder="Add new keyword"
                    value={(*new_kw).clone()}
                    oninput={Callback::from({
                        let new_kw = new_kw.clone();
                        move |e: InputEvent| {
                            let input: HtmlInputElement = e.target_unchecked_into();
                            new_kw.set(input.value());
                        }
                    })}
                    onkeypress={Callback::from({
                        let add_keyword = add_keyword.clone();
                        move |e: KeyboardEvent| {
                            if e.key() == "Enter" { add_keyword.emit(()); }
                        }
                    })}
                />
                <button
                    onclick={Callback::from({
                        let add_keyword = add_keyword.clone();
                        move |_| add_keyword.emit(())
                    })}
                >
                {"Add"}
                </button>
            </div>

            <ul class="keyword-list">
            {
                (*keywords_local).iter().map(|kw| {
                    let kw_clone = kw.clone();
                    html! {
                        <li class="keyword-item">
                            <span>{kw}</span>
                            <button class="delete-btn"
                                    onclick={Callback::from({
                                        let kw_clone = kw_clone.clone();
                                        let del_keyword = del_keyword.clone();
                                        move |_| del_keyword.emit(kw_clone.clone())
                                    })}>
                                {"×"}
                            </button>
                        </li>
                    }
                }).collect::<Html>()
            }
            </ul>
        </div>
    }
}

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct WhatsAppRoom {
    pub room_id: String,
    pub display_name: String,
    pub last_activity: i64,
    pub last_activity_formatted: String,
}

#[derive(Properties, PartialEq, Clone)]
pub struct PrioritySendersProps {
    pub service_type: String,
    pub senders: Vec<PrioritySender>,
    pub on_change: Callback<Vec<PrioritySender>>,
}

#[function_component(PrioritySendersSection)]
pub fn priority_senders_section(props: &PrioritySendersProps) -> Html {
    let new_sender = use_state(|| String::new());
    let senders_local = use_state(|| props.senders.clone());
    let search_results = use_state(|| Vec::<WhatsAppRoom>::new());
    let show_suggestions = use_state(|| false);
    let is_searching = use_state(|| false);

    {
        let senders_local = senders_local.clone();
        let parent_copy = props.senders.clone();
        use_effect_with_deps(
            move |_| { senders_local.set(parent_copy); || () },
            props.senders.clone(),
        );
    }

    let refresh = {
        let stype = props.service_type.clone();
        let loc = senders_local.clone();
        let par = props.on_change.clone();
        Callback::from(move |_| {
            if let Some(tok) = window()
                .and_then(|w| w.local_storage().ok())
                .flatten()
                .and_then(|s| s.get_item("token").ok())
                .flatten()
            {
                let stype = stype.clone();
                let loc = loc.clone();
                let par = par.clone();
                spawn_local(async move {
                    if let Ok(r) = Request::get(&format!(
                        "{}/api/filters/priority-senders/{}",
                        crate::config::get_backend_url(), stype
                    ))
                    .header("Authorization", &format!("Bearer {}", tok))
                    .send()
                    .await
                    {
                        if let Ok(list) = r.json::<Vec<PrioritySender>>().await {
                            loc.set(list.clone());
                            par.emit(list);
                        }
                    }
                });
            }
        })
    };

    let search_whatsapp_rooms = {
        let search_results = search_results.clone();
        let show_suggestions = show_suggestions.clone();
        let is_searching = is_searching.clone();
        Callback::from(move |search_term: String| {
            if search_term.trim().is_empty() {
                search_results.set(Vec::new());
                show_suggestions.set(false);
                return;
            }

            if let Some(tok) = window()
                .and_then(|w| w.local_storage().ok())
                .flatten()
                .and_then(|s| s.get_item("token").ok())
                .flatten()
            {
                let search_results = search_results.clone();
                let show_suggestions = show_suggestions.clone();
                let is_searching = is_searching.clone();
                is_searching.set(true);
                
                spawn_local(async move {
                    match Request::get(&format!(
                        "{}/api/whatsapp/search-rooms?search={}",
                        crate::config::get_backend_url(),
                        urlencoding::encode(&search_term)
                    ))
                    .header("Authorization", &format!("Bearer {}", tok))
                    .send()
                    .await
                    {
                        Ok(response) => {
                            if let Ok(rooms) = response.json::<Vec<WhatsAppRoom>>().await {
                                search_results.set(rooms);
                                show_suggestions.set(true);
                            }
                        }
                        Err(e) => {
                            web_sys::console::log_1(&format!("Search error: {}", e).into());
                        }
                    }
                    is_searching.set(false);
                });
            }
        })
    };

    let add_sender = {
        let stype = props.service_type.clone();
        let new_s = new_sender.clone();
        let reload = refresh.clone();
        let show_suggestions = show_suggestions.clone();
        Callback::from(move |_| {
            let s = (*new_s).trim().to_string();
            if s.is_empty() { return; }
            if let Some(tok) = window()
                .and_then(|w| w.local_storage().ok())
                .flatten()
                .and_then(|s| s.get_item("token").ok())
                .flatten()
            {
                let stype = stype.clone();
                let new_s = new_s.clone();
                let reload = reload.clone();
                let show_suggestions = show_suggestions.clone();
                spawn_local(async move {
                    let _ = Request::post(&format!(
                            "{}/api/filters/priority-sender/{}",
                            crate::config::get_backend_url(), stype
                        ))
                        .header("Authorization", &format!("Bearer {}", tok))
                        .json(&json!({ "sender": s, "service_type": stype }))
                        .unwrap()
                        .send()
                        .await;
                    new_s.set(String::new());
                    show_suggestions.set(false);
                    reload.emit(());
                });
            }
        })
    };

    let select_suggestion = {
        let new_sender = new_sender.clone();
        let show_suggestions = show_suggestions.clone();
        Callback::from(move |room_name: String| {
            // Extract clean name from display name (remove " (WA)" suffix)
            let clean_name = room_name
                .split(" (WA)")
                .next()
                .unwrap_or(&room_name)
                .trim()
                .to_string();
            new_sender.set(clean_name);
            show_suggestions.set(false);
        })
    };

    let del_sender = {
        let stype = props.service_type.clone();
        let reload = refresh.clone();
        Callback::from(move |who: String| {
            if let Some(tok) = window()
                .and_then(|w| w.local_storage().ok())
                .flatten()
                .and_then(|s| s.get_item("token").ok())
                .flatten()
            {
                let stype = stype.clone();
                let reload = reload.clone();
                spawn_local(async move {
                    let _ = Request::delete(&format!(
                            "{}/api/filters/priority-sender/{}/{}",
                            crate::config::get_backend_url(), stype, who
                        ))
                        .header("Authorization", &format!("Bearer {}", tok))
                        .send()
                        .await;
                    reload.emit(());
                });
            }
        })
    };

    let hide_suggestions = {
        let show_suggestions = show_suggestions.clone();
        Callback::from(move |_| {
            show_suggestions.set(false);
        })
    };

    html! {
        <div class="filter-section">
            <h3>{"Priority Senders"}</h3>

            <div class="filter-input-container">
                <div class="filter-input">
                    {
                        if props.service_type == "whatsapp" {
                            html! {
                                <div class="whatsapp-search-container">
                                    <input
                                        type="text"
                                        placeholder="Search WhatsApp chats or add manually"
                                        value={(*new_sender).clone()}
                                        oninput={Callback::from({
                                            let new_sender = new_sender.clone();
                                            let search_whatsapp_rooms = search_whatsapp_rooms.clone();
                                            move |e: InputEvent| {
                                                let el: HtmlInputElement = e.target_unchecked_into();
                                                let value = el.value();
                                                new_sender.set(value.clone());
                                                search_whatsapp_rooms.emit(value);
                                            }
                                        })}
                                        onkeypress={Callback::from({
                                            let add_sender = add_sender.clone();
                                            move |e: KeyboardEvent| if e.key() == "Enter" { add_sender.emit(()) }
                                        })}
                                        onblur={Callback::from({
                                            let hide_suggestions = hide_suggestions.clone();
                                            move |_| {
                                                // Delay hiding to allow click on suggestions
                                                let hide_suggestions = hide_suggestions.clone();
                                                spawn_local(async move {
                                                    gloo_timers::future::TimeoutFuture::new(200).await;
                                                    hide_suggestions.emit(());
                                                });
                                            }
                                        })}
                                    />
                                    {
                                        if *is_searching {
                                            html! {
                                                <div class="search-loading">
                                                    <span>{"🔍 Searching..."}</span>
                                                </div>
                                            }
                                        } else {
                                            html! {}
                                        }
                                    }
                                    {
                                        if *show_suggestions && !(*search_results).is_empty() {
                                            html! {
                                                <div class="suggestions-dropdown">
                                                    {
                                                        (*search_results).iter().map(|room| {
                                                            let room_name = room.display_name.clone();
                                                            let clean_name = room_name
                                                                .split(" (WA)")
                                                                .next()
                                                                .unwrap_or(&room_name)
                                                                .trim()
                                                                .to_string();
                                                            html! {
                                                                <div 
                                                                    class="suggestion-item"
                                                                    onmousedown={Callback::from({
                                                                        let select_suggestion = select_suggestion.clone();
                                                                        let room_name = room_name.clone();
                                                                        move |_| select_suggestion.emit(room_name.clone())
                                                                    })}
                                                                >
                                                                    <div class="suggestion-name">{clean_name}</div>
                                                                    <div class="suggestion-activity">{&room.last_activity_formatted}</div>
                                                                </div>
                                                            }
                                                        }).collect::<Html>()
                                                    }
                                                </div>
                                            }
                                        } else {
                                            html! {}
                                        }
                                    }
                                </div>
                            }
                        } else {
                            html! {
                                <input
                                    type="text"
                                    placeholder="Add priority sender"
                                    value={(*new_sender).clone()}
                                    oninput={Callback::from({
                                        let new_sender = new_sender.clone();
                                        move |e: InputEvent| {
                                            let el: HtmlInputElement = e.target_unchecked_into();
                                            new_sender.set(el.value());
                                        }
                                    })}
                                    onkeypress={Callback::from({
                                        let add_sender = add_sender.clone();
                                        move |e: KeyboardEvent| if e.key() == "Enter" { add_sender.emit(()) }
                                    })}
                                />
                            }
                        }
                    }
                    <button
                        onclick={Callback::from({
                            let add_sender = add_sender.clone();
                            move |_| add_sender.emit(())
                        })}
                    >
                        {"Add"}
                    </button>
                </div>
            </div>

            <ul class="filter-list">
            {
                (*senders_local).iter().map(|ps| {
                    let who = ps.sender.clone();
                    html! {
                        <li class="filter-item">
                            <span>{&ps.sender}</span>
                            <button class="delete-btn"
                                onclick={Callback::from({
                                    let who = who.clone();
                                    let del_sender = del_sender.clone();
                                    move |_| del_sender.emit(who.clone())
                                })}
                            >{"×"}</button>
                        </li>
                    }
                }).collect::<Html>()
            }
            </ul>
        </div>
    }
}

#[derive(Properties, PartialEq, Clone)]
pub struct ImportanceProps {
    pub service_type: String,
    pub current_threshold: i32,
    pub on_change: Callback<i32>,
}

#[function_component(ImportancePrioritySection)]
pub fn importance_priority_section(props: &ImportanceProps) -> Html {
    let value = use_state(|| props.current_threshold);
    let is_modified = use_state(|| false);

    {
        let value = value.clone();
        let is_modified = is_modified.clone();
        use_effect_with_deps(
            move |new_prop| {
                if *value != new_prop.current_threshold {
                    value.set(new_prop.current_threshold);
                    is_modified.set(false);
                }
                || ()
            },
            props.clone(),
        );
    }

    let save_threshold = {
        let stype = props.service_type.clone();
        let val = value.clone();
        let is_mod = is_modified.clone();
        let notify = props.on_change.clone();
        Callback::from(move |_| {
            let threshold = *val;
            if let Some(tok) = web_sys::window()
                .and_then(|w| w.local_storage().ok())
                .flatten()
                .and_then(|s| s.get_item("token").ok())
                .flatten()
            {
                let stype = stype.clone(); let is_mod = is_mod.clone(); let notify = notify.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    let _ = gloo_net::http::Request::post(&format!(
                            "{}/api/filters/importance-priority/{}",
                            crate::config::get_backend_url(), stype
                        ))
                        .header("Authorization", &format!("Bearer {}", tok))
                        .json(&serde_json::json!({ "threshold": threshold, "service_type": stype }))
                        .unwrap()
                        .send()
                        .await;
                    is_mod.set(false);
                    notify.emit(threshold);
                });
            }
        })
    };

    html! {
        <div class="filter-section">
            <h3>{"Importance Priority"}</h3>
            <div class="filter-input">
                <div class="importance-input-group">
                    <input
                        type="number"
                        min="1" max="10"
                        value={value.to_string()}
                        oninput={Callback::from({
                            let value = value.clone();
                            let is_mod = is_modified.clone();
                            move |e: web_sys::InputEvent| {
                                let el: web_sys::HtmlInputElement = e.target_unchecked_into();
                                let new_val = el.value().parse::<i32>().unwrap_or(7);
                                if new_val != *value {
                                    value.set(new_val);
                                    is_mod.set(true);
                                }
                            }
                        })}
                    />
                    <span class="priority-label">{"out of 10"}</span>
                </div>

                if *is_modified {
                    <button class="save-btn"
                            onclick={Callback::from({
                                let save = save_threshold.clone();
                                move |_| save.emit(())
                            })}
                    >{"Save"}</button>
                }
            </div>
        </div>
    }
}

