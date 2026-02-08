use yew::prelude::*;
use wasm_bindgen_futures::spawn_local;
use wasm_bindgen::JsCast;
use web_sys::{MouseEvent, Event};
use serde::{Deserialize, Serialize};
use crate::utils::api::Api;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct McpServer {
    pub id: i32,
    pub name: String,
    pub url: String,
    pub has_auth_token: bool,
    pub is_enabled: bool,
    pub created_at: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolInfo {
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpTestResponse {
    pub success: bool,
    pub tools_count: Option<usize>,
    pub tools: Option<Vec<McpToolInfo>>,
    pub error: Option<String>,
}

#[derive(Properties, PartialEq)]
pub struct McpConnectProps {
    pub user_id: i32,
}

#[function_component(McpConnect)]
pub fn mcp_connect(props: &McpConnectProps) -> Html {
    let servers = use_state(Vec::<McpServer>::new);
    let loading = use_state(|| true);
    let error = use_state(|| None::<String>);
    let show_add_modal = use_state(|| false);
    let testing_server = use_state(|| None::<i32>);
    let test_result = use_state(|| None::<McpTestResponse>);

    // Add form state
    let new_name = use_state(String::new);
    let new_url = use_state(String::new);
    let new_auth_token = use_state(String::new);
    let adding = use_state(|| false);
    let test_url_result = use_state(|| None::<McpTestResponse>);
    let testing_url = use_state(|| false);

    // Fetch servers on mount
    {
        let servers = servers.clone();
        let loading = loading.clone();
        let error = error.clone();
        use_effect_with_deps(move |_| {
            let servers = servers.clone();
            let loading = loading.clone();
            let error = error.clone();
            spawn_local(async move {
                match Api::get("/api/mcp/servers").send().await {
                    Ok(response) => {
                        if response.ok() {
                            if let Ok(data) = response.json::<Vec<McpServer>>().await {
                                servers.set(data);
                            }
                        } else {
                            error.set(Some("Failed to fetch MCP servers".to_string()));
                        }
                    }
                    Err(e) => {
                        error.set(Some(format!("Network error: {}", e)));
                    }
                }
                loading.set(false);
            });
            || ()
        }, ());
    }

    let on_add_server = {
        let servers = servers.clone();
        let show_add_modal = show_add_modal.clone();
        let new_name = new_name.clone();
        let new_url = new_url.clone();
        let new_auth_token = new_auth_token.clone();
        let adding = adding.clone();
        let error = error.clone();
        let test_url_result = test_url_result.clone();

        Callback::from(move |_: MouseEvent| {
            let servers = servers.clone();
            let show_add_modal = show_add_modal.clone();
            let name = (*new_name).clone();
            let url = (*new_url).clone();
            let auth_token = if (*new_auth_token).is_empty() {
                None
            } else {
                Some((*new_auth_token).clone())
            };
            let new_name = new_name.clone();
            let new_url = new_url.clone();
            let new_auth_token = new_auth_token.clone();
            let adding = adding.clone();
            let error = error.clone();
            let test_url_result = test_url_result.clone();

            if name.trim().is_empty() || url.trim().is_empty() {
                error.set(Some("Name and URL are required".to_string()));
                return;
            }

            adding.set(true);
            spawn_local(async move {
                let body = serde_json::json!({
                    "name": name,
                    "url": url,
                    "auth_token": auth_token,
                });

                let request = match Api::post("/api/mcp/servers").json(&body) {
                    Ok(r) => r,
                    Err(e) => {
                        error.set(Some(format!("Failed to create request: {}", e)));
                        adding.set(false);
                        return;
                    }
                };
                match request.send().await {
                    Ok(response) => {
                        if response.ok() {
                            if let Ok(server) = response.json::<McpServer>().await {
                                let mut new_servers = (*servers).clone();
                                new_servers.insert(0, server);
                                servers.set(new_servers);
                                show_add_modal.set(false);
                                new_name.set(String::new());
                                new_url.set(String::new());
                                new_auth_token.set(String::new());
                                test_url_result.set(None);
                            }
                        } else if let Ok(err_data) = response.json::<serde_json::Value>().await {
                            error.set(Some(
                                err_data.get("error")
                                    .and_then(|e| e.as_str())
                                    .unwrap_or("Failed to add server")
                                    .to_string()
                            ));
                        }
                    }
                    Err(e) => {
                        error.set(Some(format!("Network error: {}", e)));
                    }
                }
                adding.set(false);
            });
        })
    };

    let on_test_url = {
        let new_url = new_url.clone();
        let new_auth_token = new_auth_token.clone();
        let testing_url = testing_url.clone();
        let test_url_result = test_url_result.clone();
        let error = error.clone();

        Callback::from(move |_: MouseEvent| {
            let url = (*new_url).clone();
            let auth_token = if (*new_auth_token).is_empty() {
                None
            } else {
                Some((*new_auth_token).clone())
            };
            let testing_url = testing_url.clone();
            let test_url_result = test_url_result.clone();
            let error = error.clone();

            if url.trim().is_empty() {
                error.set(Some("URL is required".to_string()));
                return;
            }

            testing_url.set(true);
            spawn_local(async move {
                let body = serde_json::json!({
                    "url": url,
                    "auth_token": auth_token,
                });

                let request = match Api::post("/api/mcp/test").json(&body) {
                    Ok(r) => r,
                    Err(e) => {
                        error.set(Some(format!("Failed to create request: {}", e)));
                        testing_url.set(false);
                        return;
                    }
                };
                match request.send().await {
                    Ok(response) => {
                        if let Ok(result) = response.json::<McpTestResponse>().await {
                            test_url_result.set(Some(result));
                        } else {
                            error.set(Some("Failed to parse test response".to_string()));
                        }
                    }
                    Err(e) => {
                        error.set(Some(format!("Network error: {}", e)));
                    }
                }
                testing_url.set(false);
            });
        })
    };

    let on_toggle_server = {
        let servers = servers.clone();
        Callback::from(move |server_id: i32| {
            let servers = servers.clone();
            spawn_local(async move {
                if let Ok(response) = Api::patch(&format!("/api/mcp/servers/{}/toggle", server_id))
                    .send()
                    .await
                {
                    if response.ok() {
                        if let Ok(result) = response.json::<serde_json::Value>().await {
                            if let Some(is_enabled) = result.get("is_enabled").and_then(|v| v.as_bool()) {
                                let mut new_servers = (*servers).clone();
                                if let Some(server) = new_servers.iter_mut().find(|s| s.id == server_id) {
                                    server.is_enabled = is_enabled;
                                }
                                servers.set(new_servers);
                            }
                        }
                    }
                }
            });
        })
    };

    let on_delete_server = {
        let servers = servers.clone();
        Callback::from(move |server_id: i32| {
            let servers = servers.clone();
            spawn_local(async move {
                if let Ok(response) = Api::delete(&format!("/api/mcp/servers/{}", server_id))
                    .send()
                    .await
                {
                    if response.ok() {
                        let new_servers: Vec<McpServer> = (*servers)
                            .iter()
                            .filter(|s| s.id != server_id)
                            .cloned()
                            .collect();
                        servers.set(new_servers);
                    }
                }
            });
        })
    };

    let on_test_server = {
        let testing_server = testing_server.clone();
        let test_result = test_result.clone();
        Callback::from(move |server_id: i32| {
            let testing_server = testing_server.clone();
            let test_result = test_result.clone();
            testing_server.set(Some(server_id));
            test_result.set(None);
            spawn_local(async move {
                if let Ok(response) = Api::post(&format!("/api/mcp/servers/{}/test", server_id))
                    .send()
                    .await
                {
                    if let Ok(result) = response.json::<McpTestResponse>().await {
                        test_result.set(Some(result));
                    }
                }
                testing_server.set(None);
            });
        })
    };

    let on_open_add_modal = {
        let show_add_modal = show_add_modal.clone();
        let test_url_result = test_url_result.clone();
        let error = error.clone();
        Callback::from(move |_: MouseEvent| {
            show_add_modal.set(true);
            test_url_result.set(None);
            error.set(None);
        })
    };

    let on_close_modal = {
        let show_add_modal = show_add_modal.clone();
        let new_name = new_name.clone();
        let new_url = new_url.clone();
        let new_auth_token = new_auth_token.clone();
        let test_url_result = test_url_result.clone();
        Callback::from(move |_: MouseEvent| {
            show_add_modal.set(false);
            new_name.set(String::new());
            new_url.set(String::new());
            new_auth_token.set(String::new());
            test_url_result.set(None);
        })
    };

    html! {
        <div class="mcp-connect">
            <div class="mcp-header">
                <div class="mcp-title">
                    <i class="fa-solid fa-plug"></i>
                    <span>{"MCP Servers"}</span>
                </div>
                <button class="add-server-btn" onclick={on_open_add_modal}>
                    <i class="fa-solid fa-plus"></i>
                    {" Add Server"}
                </button>
            </div>

            <p class="mcp-description">
                {"Connect custom MCP servers to extend your AI assistant with additional tools and integrations."}
            </p>

            if let Some(err) = (*error).as_ref() {
                <div class="error-message">
                    {err}
                    <button class="dismiss-error" onclick={{
                        let error = error.clone();
                        Callback::from(move |_: MouseEvent| error.set(None))
                    }}>{"x"}</button>
                </div>
            }

            if *loading {
                <div class="loading">{"Loading..."}</div>
            } else if servers.is_empty() {
                <div class="no-servers">
                    <i class="fa-solid fa-server"></i>
                    <p>{"No MCP servers configured yet."}</p>
                    <p class="hint">{"Add a server to give your AI assistant new capabilities."}</p>
                </div>
            } else {
                <div class="servers-list">
                    { for servers.iter().map(|server| {
                        let server_id = server.id;
                        let is_enabled = server.is_enabled;
                        let on_toggle = {
                            let on_toggle_server = on_toggle_server.clone();
                            Callback::from(move |_: Event| on_toggle_server.emit(server_id))
                        };
                        let on_delete = {
                            let on_delete_server = on_delete_server.clone();
                            Callback::from(move |_: MouseEvent| on_delete_server.emit(server_id))
                        };
                        let on_test = {
                            let on_test_server = on_test_server.clone();
                            Callback::from(move |_: MouseEvent| on_test_server.emit(server_id))
                        };
                        let is_testing = *testing_server == Some(server_id);
                        let result = if *testing_server == Some(server_id) || (*testing_server).is_none() {
                            (*test_result).clone()
                        } else {
                            None
                        };

                        html! {
                            <div class={classes!("server-card", if !is_enabled { "disabled" } else { "" })}>
                                <div class="server-info">
                                    <div class="server-name">
                                        <i class={classes!("fa-solid", "fa-circle", if is_enabled { "status-enabled" } else { "status-disabled" })}></i>
                                        {&server.name}
                                    </div>
                                    <div class="server-url">{&server.url}</div>
                                    { if server.has_auth_token {
                                        html! { <span class="auth-badge"><i class="fa-solid fa-key"></i>{" Auth"}</span> }
                                    } else {
                                        html! {}
                                    }}
                                </div>
                                <div class="server-actions">
                                    <button
                                        class="test-btn"
                                        onclick={on_test}
                                        disabled={is_testing}
                                    >
                                        { if is_testing {
                                            html! { <><i class="fa-solid fa-spinner fa-spin"></i>{" Testing..."}</> }
                                        } else {
                                            html! { <><i class="fa-solid fa-flask"></i>{" Test"}</> }
                                        }}
                                    </button>
                                    <label class="toggle-switch">
                                        <input
                                            type="checkbox"
                                            checked={is_enabled}
                                            onchange={on_toggle}
                                        />
                                        <span class="toggle-slider"></span>
                                    </label>
                                    <button class="delete-btn" onclick={on_delete}>
                                        <i class="fa-solid fa-trash"></i>
                                    </button>
                                </div>
                                { if let Some(ref res) = result {
                                    if *testing_server != Some(server_id) && (*test_result).is_some() {
                                        html! {}
                                    } else {
                                        html! {
                                            <div class={classes!("test-result", if res.success { "success" } else { "error" })}>
                                                { if res.success {
                                                    html! {
                                                        <>
                                                            <i class="fa-solid fa-check-circle"></i>
                                                            {format!(" Connected - {} tools available", res.tools_count.unwrap_or(0))}
                                                            { if let Some(ref tools) = res.tools {
                                                                html! {
                                                                    <div class="tools-list">
                                                                        { for tools.iter().take(5).map(|t| {
                                                                            html! { <span class="tool-name">{&t.name}</span> }
                                                                        })}
                                                                        { if tools.len() > 5 {
                                                                            html! { <span class="more-tools">{format!("...+{} more", tools.len() - 5)}</span> }
                                                                        } else {
                                                                            html! {}
                                                                        }}
                                                                    </div>
                                                                }
                                                            } else {
                                                                html! {}
                                                            }}
                                                        </>
                                                    }
                                                } else {
                                                    html! {
                                                        <>
                                                            <i class="fa-solid fa-times-circle"></i>
                                                            {format!(" Failed: {}", res.error.as_ref().unwrap_or(&"Unknown error".to_string()))}
                                                        </>
                                                    }
                                                }}
                                            </div>
                                        }
                                    }
                                } else {
                                    html! {}
                                }}
                            </div>
                        }
                    })}
                </div>
            }

            // Add Server Modal
            if *show_add_modal {
                <div class="modal-overlay" onclick={on_close_modal.clone()}>
                    <div class="modal-content" onclick={Callback::from(|e: MouseEvent| e.stop_propagation())}>
                        <div class="modal-header">
                            <h3>{"Add MCP Server"}</h3>
                            <button class="close-btn" onclick={on_close_modal.clone()}>{"x"}</button>
                        </div>
                        <div class="modal-body">
                            <div class="form-group">
                                <label>{"Server Name"}</label>
                                <input
                                    type="text"
                                    autocomplete="off"
                                    placeholder="e.g., homeassistant"
                                    value={(*new_name).clone()}
                                    oninput={{
                                        let new_name = new_name.clone();
                                        Callback::from(move |e: InputEvent| {
                                            if let Some(target) = e.target() {
                                                if let Ok(input) = target.dyn_into::<web_sys::HtmlInputElement>() {
                                                    new_name.set(input.value());
                                                }
                                            }
                                        })
                                    }}
                                />
                                <span class="hint">{"Only letters, numbers, hyphens, and underscores"}</span>
                            </div>
                            <div class="form-group">
                                <label>{"Server URL"}</label>
                                <input
                                    type="url"
                                    autocomplete="off"
                                    placeholder="https://your-mcp-server.com/mcp"
                                    value={(*new_url).clone()}
                                    oninput={{
                                        let new_url = new_url.clone();
                                        Callback::from(move |e: InputEvent| {
                                            if let Some(target) = e.target() {
                                                if let Ok(input) = target.dyn_into::<web_sys::HtmlInputElement>() {
                                                    new_url.set(input.value());
                                                }
                                            }
                                        })
                                    }}
                                />
                            </div>
                            <div class="form-group">
                                <label>{"Auth Token"}<span class="optional">{" (optional)"}</span></label>
                                <input
                                    type="password"
                                    autocomplete="new-password"
                                    placeholder="Bearer token or API key"
                                    value={(*new_auth_token).clone()}
                                    oninput={{
                                        let new_auth_token = new_auth_token.clone();
                                        Callback::from(move |e: InputEvent| {
                                            if let Some(target) = e.target() {
                                                if let Ok(input) = target.dyn_into::<web_sys::HtmlInputElement>() {
                                                    new_auth_token.set(input.value());
                                                }
                                            }
                                        })
                                    }}
                                />
                            </div>

                            <button
                                class="test-connection-btn"
                                onclick={on_test_url}
                                disabled={*testing_url || (*new_url).is_empty()}
                            >
                                { if *testing_url {
                                    html! { <><i class="fa-solid fa-spinner fa-spin"></i>{" Testing..."}</> }
                                } else {
                                    html! { <><i class="fa-solid fa-flask"></i>{" Test Connection"}</> }
                                }}
                            </button>

                            { if let Some(ref result) = *test_url_result {
                                html! {
                                    <div class={classes!("test-result", if result.success { "success" } else { "error" })}>
                                        { if result.success {
                                            html! {
                                                <>
                                                    <i class="fa-solid fa-check-circle"></i>
                                                    {format!(" Connected! {} tools discovered:", result.tools_count.unwrap_or(0))}
                                                    { if let Some(ref tools) = result.tools {
                                                        html! {
                                                            <ul class="discovered-tools">
                                                                { for tools.iter().map(|t| {
                                                                    html! {
                                                                        <li>
                                                                            <strong>{&t.name}</strong>
                                                                            { if let Some(ref desc) = t.description {
                                                                                html! { <span class="tool-desc">{format!(" - {}", desc)}</span> }
                                                                            } else {
                                                                                html! {}
                                                                            }}
                                                                        </li>
                                                                    }
                                                                })}
                                                            </ul>
                                                        }
                                                    } else {
                                                        html! {}
                                                    }}
                                                </>
                                            }
                                        } else {
                                            html! {
                                                <>
                                                    <i class="fa-solid fa-times-circle"></i>
                                                    {format!(" Connection failed: {}", result.error.as_ref().unwrap_or(&"Unknown error".to_string()))}
                                                </>
                                            }
                                        }}
                                    </div>
                                }
                            } else {
                                html! {}
                            }}
                        </div>
                        <div class="modal-footer">
                            <button class="cancel-btn" onclick={on_close_modal.clone()}>{"Cancel"}</button>
                            <button
                                class="add-btn"
                                onclick={on_add_server}
                                disabled={*adding || (*new_name).is_empty() || (*new_url).is_empty()}
                            >
                                { if *adding {
                                    html! { <><i class="fa-solid fa-spinner fa-spin"></i>{" Adding..."}</> }
                                } else {
                                    html! { "Add Server" }
                                }}
                            </button>
                        </div>
                    </div>
                </div>
            }

            <style>
            {r#"
.mcp-connect {
    padding: 1rem;
}
.mcp-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 0.75rem;
}
.mcp-title {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    color: #8B5CF6;
    font-size: 1.1rem;
    font-weight: 500;
}
.mcp-description {
    color: #888;
    font-size: 0.9rem;
    margin-bottom: 1rem;
}
.add-server-btn {
    background: linear-gradient(45deg, #8B5CF6, #7C3AED);
    color: white;
    border: none;
    padding: 0.5rem 1rem;
    border-radius: 6px;
    cursor: pointer;
    font-size: 0.9rem;
    transition: all 0.2s;
}
.add-server-btn:hover {
    transform: translateY(-1px);
    box-shadow: 0 4px 12px rgba(139, 92, 246, 0.3);
}
.servers-list {
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
}
.server-card {
    background: rgba(0, 0, 0, 0.2);
    border: 1px solid rgba(139, 92, 246, 0.2);
    border-radius: 8px;
    padding: 1rem;
    transition: all 0.2s;
}
.server-card:hover {
    border-color: rgba(139, 92, 246, 0.4);
}
.server-card.disabled {
    opacity: 0.6;
}
.server-info {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.5rem;
    margin-bottom: 0.75rem;
}
.server-name {
    font-weight: 500;
    color: #fff;
    display: flex;
    align-items: center;
    gap: 0.5rem;
}
.server-url {
    color: #888;
    font-size: 0.85rem;
    word-break: break-all;
}
.status-enabled {
    color: #22C55E;
    font-size: 0.5rem;
}
.status-disabled {
    color: #666;
    font-size: 0.5rem;
}
.auth-badge {
    background: rgba(139, 92, 246, 0.2);
    color: #8B5CF6;
    padding: 0.2rem 0.5rem;
    border-radius: 4px;
    font-size: 0.75rem;
}
.server-actions {
    display: flex;
    align-items: center;
    gap: 0.75rem;
}
.test-btn {
    background: rgba(139, 92, 246, 0.1);
    color: #8B5CF6;
    border: 1px solid rgba(139, 92, 246, 0.3);
    padding: 0.4rem 0.75rem;
    border-radius: 4px;
    cursor: pointer;
    font-size: 0.85rem;
    transition: all 0.2s;
}
.test-btn:hover:not(:disabled) {
    background: rgba(139, 92, 246, 0.2);
}
.test-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
}
.delete-btn {
    background: transparent;
    color: #EF4444;
    border: none;
    padding: 0.4rem 0.5rem;
    border-radius: 4px;
    cursor: pointer;
    transition: all 0.2s;
}
.delete-btn:hover {
    background: rgba(239, 68, 68, 0.1);
}
.toggle-switch {
    position: relative;
    width: 40px;
    height: 22px;
    cursor: pointer;
}
.toggle-switch input {
    opacity: 0;
    width: 0;
    height: 0;
}
.toggle-slider {
    position: absolute;
    cursor: pointer;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    background-color: rgba(100, 100, 100, 0.5);
    transition: 0.3s;
    border-radius: 22px;
}
.toggle-slider:before {
    position: absolute;
    content: "";
    height: 16px;
    width: 16px;
    left: 3px;
    bottom: 3px;
    background-color: white;
    transition: 0.3s;
    border-radius: 50%;
}
input:checked + .toggle-slider {
    background: linear-gradient(45deg, #8B5CF6, #7C3AED);
}
input:checked + .toggle-slider:before {
    transform: translateX(18px);
}
.test-result {
    margin-top: 0.75rem;
    padding: 0.5rem 0.75rem;
    border-radius: 6px;
    font-size: 0.85rem;
}
.test-result.success {
    background: rgba(34, 197, 94, 0.1);
    border: 1px solid rgba(34, 197, 94, 0.3);
    color: #22C55E;
}
.test-result.error {
    background: rgba(239, 68, 68, 0.1);
    border: 1px solid rgba(239, 68, 68, 0.3);
    color: #EF4444;
}
.tools-list {
    display: flex;
    flex-wrap: wrap;
    gap: 0.5rem;
    margin-top: 0.5rem;
}
.tool-name {
    background: rgba(139, 92, 246, 0.2);
    color: #8B5CF6;
    padding: 0.2rem 0.5rem;
    border-radius: 4px;
    font-size: 0.75rem;
}
.more-tools {
    color: #888;
    font-size: 0.75rem;
}
.no-servers {
    text-align: center;
    padding: 2rem;
    color: #666;
}
.no-servers i {
    font-size: 2rem;
    margin-bottom: 0.5rem;
    color: #8B5CF6;
    opacity: 0.5;
}
.no-servers .hint {
    font-size: 0.85rem;
    color: #555;
}
.loading {
    text-align: center;
    padding: 1rem;
    color: #888;
}
.error-message {
    background: rgba(239, 68, 68, 0.1);
    border: 1px solid rgba(239, 68, 68, 0.3);
    color: #EF4444;
    padding: 0.75rem;
    border-radius: 6px;
    margin-bottom: 1rem;
    display: flex;
    justify-content: space-between;
    align-items: center;
}
.dismiss-error {
    background: none;
    border: none;
    color: #EF4444;
    cursor: pointer;
    font-size: 1rem;
}
/* Modal styles */
.modal-overlay {
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    background: rgba(0, 0, 0, 0.8);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1200;
}
.modal-content {
    background: #1a1a1a;
    border-radius: 12px;
    width: 90%;
    max-width: 500px;
    max-height: 90vh;
    overflow-y: auto;
}
.modal-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 1rem 1.5rem;
    border-bottom: 1px solid rgba(255, 255, 255, 0.1);
}
.modal-header h3 {
    margin: 0;
    color: #fff;
}
.close-btn {
    background: none;
    border: none;
    color: #888;
    font-size: 1.2rem;
    cursor: pointer;
}
.close-btn:hover {
    color: #fff;
}
.modal-body {
    padding: 1.5rem;
}
.form-group {
    margin-bottom: 1rem;
}
.form-group label {
    display: block;
    color: #ccc;
    margin-bottom: 0.5rem;
    font-size: 0.9rem;
}
.form-group .optional {
    color: #666;
}
.form-group input {
    width: 100%;
    padding: 0.75rem;
    background: rgba(0, 0, 0, 0.3);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 6px;
    color: #fff;
    font-size: 0.95rem;
    box-sizing: border-box;
}
.form-group input:focus {
    outline: none;
    border-color: #8B5CF6;
}
.form-group .hint {
    display: block;
    color: #666;
    font-size: 0.8rem;
    margin-top: 0.25rem;
}
.test-connection-btn {
    width: 100%;
    background: rgba(139, 92, 246, 0.1);
    color: #8B5CF6;
    border: 1px solid rgba(139, 92, 246, 0.3);
    padding: 0.75rem;
    border-radius: 6px;
    cursor: pointer;
    font-size: 0.95rem;
    margin-top: 0.5rem;
    transition: all 0.2s;
}
.test-connection-btn:hover:not(:disabled) {
    background: rgba(139, 92, 246, 0.2);
}
.test-connection-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
}
.discovered-tools {
    list-style: none;
    padding: 0;
    margin: 0.5rem 0 0 0;
}
.discovered-tools li {
    padding: 0.25rem 0;
    font-size: 0.85rem;
}
.discovered-tools .tool-desc {
    color: #888;
}
.modal-footer {
    display: flex;
    justify-content: flex-end;
    gap: 0.75rem;
    padding: 1rem 1.5rem;
    border-top: 1px solid rgba(255, 255, 255, 0.1);
}
.cancel-btn {
    background: transparent;
    color: #888;
    border: 1px solid rgba(255, 255, 255, 0.1);
    padding: 0.75rem 1.5rem;
    border-radius: 6px;
    cursor: pointer;
    transition: all 0.2s;
}
.cancel-btn:hover {
    background: rgba(255, 255, 255, 0.05);
    color: #fff;
}
.add-btn {
    background: linear-gradient(45deg, #8B5CF6, #7C3AED);
    color: white;
    border: none;
    padding: 0.75rem 1.5rem;
    border-radius: 6px;
    cursor: pointer;
    transition: all 0.2s;
}
.add-btn:hover:not(:disabled) {
    transform: translateY(-1px);
    box-shadow: 0 4px 12px rgba(139, 92, 246, 0.3);
}
.add-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
}
            "#}
            </style>
        </div>
    }
}
