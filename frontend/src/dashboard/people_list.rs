use yew::prelude::*;
use web_sys::HtmlInputElement;
use wasm_bindgen_futures::spawn_local;
use serde::Deserialize;
use crate::utils::api::Api;

const PEOPLE_STYLES: &str = r#"
.people-section {
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
    overflow-y: auto;
    flex: 1;
    min-height: 0;
}
.people-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
}
.people-header-label {
    font-size: 0.75rem;
    color: #666;
    text-transform: uppercase;
    letter-spacing: 0.05em;
}
.people-add-btn {
    background: transparent;
    border: 1px solid rgba(126, 178, 255, 0.3);
    color: #7EB2FF;
    font-size: 0.75rem;
    padding: 0.2rem 0.6rem;
    border-radius: 6px;
    cursor: pointer;
}
.people-add-btn:hover { background: rgba(126, 178, 255, 0.1); }
.person-row {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.4rem 0.5rem;
    border-radius: 6px;
    cursor: pointer;
    transition: background 0.15s;
}
.person-row:hover { background: rgba(255, 255, 255, 0.04); }
.person-row.expanded {
    background: rgba(255, 255, 255, 0.04);
    border-radius: 6px 6px 0 0;
}
.person-name {
    font-size: 0.85rem;
    color: #ddd;
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
}
.person-channels-badges {
    display: flex;
    gap: 0.3rem;
}
.person-ch-badge {
    font-size: 0.6rem;
    color: #777;
    background: rgba(255, 255, 255, 0.05);
    padding: 0.1rem 0.35rem;
    border-radius: 3px;
    text-transform: capitalize;
}
.person-detail {
    padding: 0.5rem;
    background: rgba(255, 255, 255, 0.02);
    border-radius: 0 0 6px 6px;
    margin-bottom: 0.25rem;
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
}
.person-ch-row {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    font-size: 0.8rem;
}
.person-ch-row .plat {
    color: #888;
    min-width: 65px;
    font-size: 0.75rem;
    text-transform: capitalize;
}
.person-ch-row .handle {
    color: #bbb;
}
.person-ch-del {
    background: transparent;
    border: none;
    color: #555;
    font-size: 0.65rem;
    cursor: pointer;
    padding: 0 0.3rem;
}
.person-ch-del:hover { color: #f87171; }
.person-actions-row {
    display: flex;
    justify-content: space-between;
    margin-top: 0.25rem;
}
.person-del-btn {
    background: transparent;
    border: none;
    color: #666;
    font-size: 0.7rem;
    cursor: pointer;
    padding: 0.15rem 0.4rem;
}
.person-del-btn:hover { color: #f87171; }
.people-empty {
    font-size: 0.8rem;
    color: #555;
    padding: 0.5rem 0;
}

/* ---- Add/Edit form ---- */
.pf-form {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
    padding: 0.5rem;
    background: rgba(255, 255, 255, 0.03);
    border-radius: 6px;
    border: 1px solid rgba(255, 255, 255, 0.06);
}
.pf-input {
    background: #12121f;
    border: 1px solid rgba(255, 255, 255, 0.15);
    border-radius: 5px;
    color: #ddd;
    padding: 0.35rem 0.5rem;
    font-size: 0.8rem;
    font-family: inherit;
    width: 100%;
    box-sizing: border-box;
}
.pf-input:focus {
    outline: none;
    border-color: rgba(126, 178, 255, 0.4);
}
.pf-label {
    font-size: 0.7rem;
    color: #777;
    margin-top: 0.2rem;
}
.pf-ch-group {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    position: relative;
}
.pf-ch-icon {
    font-size: 0.8rem;
    width: 1.2rem;
    text-align: center;
    flex-shrink: 0;
}
.pf-ch-icon.wa { color: #25D366; }
.pf-ch-icon.tg { color: #0088cc; }
.pf-ch-icon.sg { color: #3A76F0; }
.pf-ch-icon.em { color: #aaa; }
.pf-actions {
    display: flex;
    gap: 0.4rem;
    justify-content: flex-end;
}
.pf-save {
    background: rgba(126, 178, 255, 0.2);
    color: #7EB2FF;
    border: none;
    padding: 0.3rem 0.7rem;
    border-radius: 5px;
    font-size: 0.75rem;
    cursor: pointer;
}
.pf-save:hover { background: rgba(126, 178, 255, 0.3); }
.pf-cancel {
    background: transparent;
    color: #888;
    border: 1px solid rgba(255, 255, 255, 0.08);
    padding: 0.3rem 0.7rem;
    border-radius: 5px;
    font-size: 0.75rem;
    cursor: pointer;
}
.pf-suggestions {
    position: absolute;
    top: 100%;
    left: 1.6rem;
    right: 0;
    background: #1e1e2f;
    border: 1px solid rgba(255, 255, 255, 0.15);
    border-radius: 6px;
    max-height: 150px;
    overflow-y: auto;
    z-index: 20;
    margin-top: 2px;
}
.pf-suggestion-item {
    padding: 0.35rem 0.5rem;
    font-size: 0.78rem;
    color: #ccc;
    cursor: pointer;
}
.pf-suggestion-item:hover {
    background: rgba(126, 178, 255, 0.15);
    color: #fff;
}
.pf-suggestion-item.disabled {
    color: #555;
    cursor: default;
}
.pf-suggestion-item .attached-tag {
    color: #666;
    font-size: 0.68rem;
    margin-left: 0.4rem;
}
"#;

#[derive(Clone, PartialEq, Deserialize)]
pub struct PersonResponse {
    pub person: PersonData,
    pub channels: Vec<ChannelData>,
    #[serde(default)]
    pub edits: Vec<PersonEditData>,
}

#[derive(Clone, PartialEq, Deserialize)]
pub struct PersonData {
    pub id: i32,
    pub name: String,
}

#[derive(Clone, PartialEq, Deserialize)]
pub struct ChannelData {
    pub id: i32,
    pub platform: String,
    pub handle: Option<String>,
}

#[derive(Clone, PartialEq, Deserialize)]
pub struct PersonEditData {
    pub property_name: String,
    pub value: String,
}

impl PersonResponse {
    fn display_name(&self) -> &str {
        self.edits
            .iter()
            .find(|e| e.property_name == "nickname")
            .map(|e| e.value.as_str())
            .unwrap_or(&self.person.name)
    }
}

#[derive(Clone, PartialEq, Deserialize)]
struct SearchRoom {
    display_name: String,
    room_id: String,
    is_group: bool,
    #[serde(default)]
    person_name: Option<String>,
    #[serde(default)]
    is_phone_contact: Option<bool>,
}

#[function_component(PeopleList)]
pub fn people_list() -> Html {
    let persons = use_state(|| Vec::<PersonResponse>::new());
    let loading = use_state(|| true);
    let expanded_id = use_state(|| None::<i32>);
    let adding = use_state(|| false);
    let refresh_seq = use_state(|| 0u32);

    // Add form state
    let add_name = use_state(|| String::new());
    let add_wa = use_state(|| String::new());
    let add_wa_room = use_state(|| None::<String>);
    let add_tg = use_state(|| String::new());
    let add_tg_room = use_state(|| None::<String>);
    let add_sg = use_state(|| String::new());
    let add_sg_room = use_state(|| None::<String>);
    let add_email = use_state(|| String::new());

    // Search state
    let search_results = use_state(|| Vec::<SearchRoom>::new());
    let search_active = use_state(|| None::<String>); // which platform is searching
    let searching = use_state(|| false);

    // Fetch persons
    {
        let persons = persons.clone();
        let loading = loading.clone();
        let seq = *refresh_seq;
        use_effect_with_deps(
            move |_| {
                let persons = persons.clone();
                let loading = loading.clone();
                spawn_local(async move {
                    if let Ok(r) = Api::get("/api/persons").send().await {
                        if let Ok(data) = r.json::<Vec<PersonResponse>>().await {
                            persons.set(data);
                        }
                    }
                    loading.set(false);
                });
                || ()
            },
            seq,
        );
    }

    // Search chats helper
    let do_search = {
        let search_results = search_results.clone();
        let search_active = search_active.clone();
        let searching = searching.clone();
        Callback::from(move |(platform, query): (String, String)| {
            if query.len() < 2 {
                search_results.set(Vec::new());
                search_active.set(None);
                return;
            }
            let search_results = search_results.clone();
            let search_active = search_active.clone();
            let searching = searching.clone();
            search_active.set(Some(platform.clone()));
            searching.set(true);
            spawn_local(async move {
                let url = format!(
                    "/api/persons/search/{}?q={}",
                    platform,
                    js_sys::encode_uri_component(&query)
                );
                match Api::get(&url).send().await {
                    Ok(r) => {
                        if let Ok(data) = r.json::<serde_json::Value>().await {
                            if let Some(results) = data.get("results").and_then(|r| r.as_array()) {
                                let rooms: Vec<SearchRoom> = results
                                    .iter()
                                    .filter_map(|r| serde_json::from_value(r.clone()).ok())
                                    .collect();
                                search_results.set(rooms);
                            }
                        }
                    }
                    Err(_) => {}
                }
                searching.set(false);
            });
        })
    };

    let on_add_click = {
        let adding = adding.clone();
        let add_name = add_name.clone();
        let add_wa = add_wa.clone();
        let add_tg = add_tg.clone();
        let add_sg = add_sg.clone();
        let add_email = add_email.clone();
        let add_wa_room = add_wa_room.clone();
        let add_tg_room = add_tg_room.clone();
        let add_sg_room = add_sg_room.clone();
        Callback::from(move |_: MouseEvent| {
            add_name.set(String::new());
            add_wa.set(String::new());
            add_tg.set(String::new());
            add_sg.set(String::new());
            add_email.set(String::new());
            add_wa_room.set(None);
            add_tg_room.set(None);
            add_sg_room.set(None);
            adding.set(true);
        })
    };

    let on_add_cancel = {
        let adding = adding.clone();
        Callback::from(move |_: MouseEvent| adding.set(false))
    };

    let on_add_save = {
        let add_name = add_name.clone();
        let add_wa = add_wa.clone();
        let add_wa_room = add_wa_room.clone();
        let add_tg = add_tg.clone();
        let add_tg_room = add_tg_room.clone();
        let add_sg = add_sg.clone();
        let add_sg_room = add_sg_room.clone();
        let add_email = add_email.clone();
        let adding = adding.clone();
        let refresh_seq = refresh_seq.clone();
        Callback::from(move |_: MouseEvent| {
            let name = (*add_name).clone();
            if name.trim().is_empty() {
                return;
            }

            // Build channels
            let mut channels = Vec::new();
            let wa = (*add_wa).trim().to_string();
            if !wa.is_empty() {
                channels.push(serde_json::json!({
                    "platform": "whatsapp",
                    "handle": wa,
                    "room_id": *add_wa_room,
                }));
            }
            let tg = (*add_tg).trim().to_string();
            if !tg.is_empty() {
                channels.push(serde_json::json!({
                    "platform": "telegram",
                    "handle": tg,
                    "room_id": *add_tg_room,
                }));
            }
            let sg = (*add_sg).trim().to_string();
            if !sg.is_empty() {
                channels.push(serde_json::json!({
                    "platform": "signal",
                    "handle": sg,
                    "room_id": *add_sg_room,
                }));
            }
            let em = (*add_email).trim().to_string();
            if !em.is_empty() {
                channels.push(serde_json::json!({
                    "platform": "email",
                    "handle": em,
                }));
            }

            let body = serde_json::json!({
                "name": name.trim(),
                "channels": channels,
            });

            let adding = adding.clone();
            let refresh_seq = refresh_seq.clone();
            spawn_local(async move {
                if let Ok(req) = Api::post("/api/persons").json(&body) {
                    if let Ok(r) = req.send().await {
                        if r.ok() {
                            adding.set(false);
                            refresh_seq.set(js_sys::Date::now() as u32);
                        }
                    }
                }
            });
        })
    };

    // Channel search input helper macro - renders an input with suggestions dropdown
    let render_channel_input = |
        icon_class: &str,
        platform: &str,
        placeholder: &str,
        value: UseStateHandle<String>,
        room_id: UseStateHandle<Option<String>>,
        do_search: &Callback<(String, String)>,
        search_active: &UseStateHandle<Option<String>>,
        search_results: &UseStateHandle<Vec<SearchRoom>>,
        searching: &UseStateHandle<bool>,
    | {
        let plat = platform.to_string();
        let plat2 = platform.to_string();
        let is_active = search_active.as_ref() == Some(&plat);

        let on_input = {
            let value = value.clone();
            let room_id = room_id.clone();
            let do_search = do_search.clone();
            let plat = plat.clone();
            Callback::from(move |e: InputEvent| {
                if let Some(input) = e.target_dyn_into::<HtmlInputElement>() {
                    let v = input.value();
                    value.set(v.clone());
                    room_id.set(None);
                    do_search.emit((plat.clone(), v));
                }
            })
        };

        let on_focus = {
            let value = value.clone();
            let do_search = do_search.clone();
            let plat = plat.clone();
            Callback::from(move |_: FocusEvent| {
                let v = (*value).clone();
                if v.len() >= 2 {
                    do_search.emit((plat.clone(), v));
                }
            })
        };

        let on_blur = {
            let search_active = search_active.clone();
            Callback::from(move |_: FocusEvent| {
                // Delay to allow click on suggestion
                let search_active = search_active.clone();
                gloo_timers::callback::Timeout::new(200, move || {
                    search_active.set(None);
                }).forget();
            })
        };

        let suggestions = if is_active {
            let rooms = (**search_results).clone();
            html! {
                <div class="pf-suggestions">
                    if **searching {
                        <div class="pf-suggestion-item disabled">{"Searching..."}</div>
                    } else if rooms.is_empty() {
                        <div class="pf-suggestion-item disabled">{"No chats found"}</div>
                    } else {
                        { for rooms.iter().map(|room| {
                            let name = room.display_name.clone();
                            let rid = room.room_id.clone();
                            let attached = room.person_name.clone();
                            let is_disabled = attached.is_some();
                            let value = value.clone();
                            let room_id = room_id.clone();
                            let search_active = search_active.clone();
                            let item_class = if is_disabled { "pf-suggestion-item disabled" } else { "pf-suggestion-item" };

                            let on_click = if is_disabled {
                                Callback::noop()
                            } else {
                                let name = name.clone();
                                Callback::from(move |e: MouseEvent| {
                                    e.prevent_default();
                                    value.set(name.clone());
                                    let rid_opt = if rid.is_empty() { None } else { Some(rid.clone()) };
                                    room_id.set(rid_opt);
                                    search_active.set(None);
                                })
                            };

                            html! {
                                <div class={item_class} onmousedown={on_click}>
                                    <span>{&room.display_name}</span>
                                    if let Some(ref owner) = attached {
                                        <span class="attached-tag">{format!("({})", owner)}</span>
                                    }
                                </div>
                            }
                        })}
                    }
                </div>
            }
        } else {
            html! {}
        };

        html! {
            <div class="pf-ch-group">
                <span class={format!("pf-ch-icon {}", icon_class)}>
                    <i class={match plat2.as_str() {
                        "whatsapp" => "fa-brands fa-whatsapp",
                        "telegram" => "fa-brands fa-telegram",
                        "signal" => "fa-solid fa-comment-dots",
                        _ => "fa-solid fa-envelope",
                    }}></i>
                </span>
                <input
                    class="pf-input"
                    type="text"
                    placeholder={placeholder.to_string()}
                    value={(*value).clone()}
                    oninput={on_input}
                    onfocus={on_focus}
                    onblur={on_blur}
                />
                {suggestions}
            </div>
        }
    };

    html! {
        <>
            <style>{PEOPLE_STYLES}</style>
            <div class="people-section">
                <div class="people-header">
                    <span class="people-header-label">{"People"}</span>
                    <button class="people-add-btn" onclick={on_add_click}>
                        <i class="fa-solid fa-plus" style="margin-right: 0.3rem; font-size: 0.65rem;"></i>
                        {"Add"}
                    </button>
                </div>

                if *adding {
                    <div class="pf-form">
                        <div class="pf-label">{"Name"}</div>
                        <input
                            class="pf-input"
                            type="text"
                            placeholder="Person name..."
                            value={(*add_name).clone()}
                            oninput={{
                                let add_name = add_name.clone();
                                Callback::from(move |e: InputEvent| {
                                    if let Some(input) = e.target_dyn_into::<HtmlInputElement>() {
                                        add_name.set(input.value());
                                    }
                                })
                            }}
                        />

                        <div class="pf-label">{"Channels"}</div>
                        {render_channel_input("wa", "whatsapp", "Search WhatsApp...", add_wa.clone(), add_wa_room.clone(), &do_search, &search_active, &search_results, &searching)}
                        {render_channel_input("tg", "telegram", "Search Telegram...", add_tg.clone(), add_tg_room.clone(), &do_search, &search_active, &search_results, &searching)}
                        {render_channel_input("sg", "signal", "Search Signal...", add_sg.clone(), add_sg_room.clone(), &do_search, &search_active, &search_results, &searching)}

                        <div class="pf-ch-group">
                            <span class="pf-ch-icon em">
                                <i class="fa-solid fa-envelope"></i>
                            </span>
                            <input
                                class="pf-input"
                                type="text"
                                placeholder="email@example.com"
                                value={(*add_email).clone()}
                                oninput={{
                                    let add_email = add_email.clone();
                                    Callback::from(move |e: InputEvent| {
                                        if let Some(input) = e.target_dyn_into::<HtmlInputElement>() {
                                            add_email.set(input.value());
                                        }
                                    })
                                }}
                            />
                        </div>

                        <div class="pf-actions">
                            <button class="pf-cancel" onclick={on_add_cancel}>{"Cancel"}</button>
                            <button class="pf-save" onclick={on_add_save}>{"Create"}</button>
                        </div>
                    </div>
                }

                if !*loading && persons.is_empty() && !*adding {
                    <div class="people-empty">{"No people added yet. People let you give nicknames and connect the same person across WhatsApp, Telegram, Signal, and email."}</div>
                }

                { for persons.iter().map(|p| {
                    let pid = p.person.id;
                    let name = p.display_name().to_string();
                    let is_expanded = *expanded_id == Some(pid);
                    let channels = p.channels.clone();

                    let on_click = {
                        let expanded_id = expanded_id.clone();
                        Callback::from(move |_: MouseEvent| {
                            if *expanded_id == Some(pid) {
                                expanded_id.set(None);
                            } else {
                                expanded_id.set(Some(pid));
                            }
                        })
                    };

                    let on_delete = {
                        let refresh_seq = refresh_seq.clone();
                        let expanded_id = expanded_id.clone();
                        Callback::from(move |e: MouseEvent| {
                            e.stop_propagation();
                            let refresh_seq = refresh_seq.clone();
                            let expanded_id = expanded_id.clone();
                            spawn_local(async move {
                                let _ = Api::delete(&format!("/api/persons/{}", pid)).send().await;
                                expanded_id.set(None);
                                refresh_seq.set(js_sys::Date::now() as u32);
                            });
                        })
                    };

                    let row_class = if is_expanded { "person-row expanded" } else { "person-row" };

                    html! {
                        <div key={pid}>
                            <div class={row_class} onclick={on_click}>
                                <span class="person-name">{&name}</span>
                                <div class="person-channels-badges">
                                    { for channels.iter().map(|ch| {
                                        let display_platform = match ch.platform.as_str() {
                                            "whatsapp" => "WhatsApp",
                                            "telegram" => "Telegram",
                                            "signal" => "Signal",
                                            "email" => "Email",
                                            other => other,
                                        };
                                        html! { <span class="person-ch-badge">{display_platform}</span> }
                                    })}
                                </div>
                            </div>
                            if is_expanded {
                                <div class="person-detail">
                                    if channels.is_empty() {
                                        <div class="person-ch-row">
                                            <span style="color: #666; font-size: 0.78rem;">{"No channels linked."}</span>
                                        </div>
                                    }
                                    { for channels.iter().map(|ch| {
                                        let ch_id = ch.id;
                                        let handle = ch.handle.clone().unwrap_or_default();
                                        let on_ch_delete = {
                                            let refresh_seq = refresh_seq.clone();
                                            Callback::from(move |e: MouseEvent| {
                                                e.stop_propagation();
                                                let refresh_seq = refresh_seq.clone();
                                                spawn_local(async move {
                                                    let _ = Api::delete(&format!("/api/persons/{}/channels/{}", pid, ch_id)).send().await;
                                                    refresh_seq.set(js_sys::Date::now() as u32);
                                                });
                                            })
                                        };
                                        let display_plat = match ch.platform.as_str() {
                                            "whatsapp" => "WhatsApp",
                                            "telegram" => "Telegram",
                                            "signal" => "Signal",
                                            "email" => "Email",
                                            other => other,
                                        };
                                        html! {
                                            <div class="person-ch-row">
                                                <span class="plat">{display_plat}</span>
                                                <span class="handle">{if handle.is_empty() { "Chat linked".to_string() } else { handle }}</span>
                                                <button class="person-ch-del" onclick={on_ch_delete}>
                                                    <i class="fa-solid fa-xmark"></i>
                                                </button>
                                            </div>
                                        }
                                    })}
                                    <div class="person-actions-row">
                                        <button class="person-del-btn" onclick={on_delete}>
                                            <i class="fa-solid fa-trash-can" style="margin-right: 0.25rem;"></i>
                                            {"Remove person"}
                                        </button>
                                    </div>
                                </div>
                            }
                        </div>
                    }
                })}
            </div>
        </>
    }
}
