use gloo_timers::callback::{Interval, Timeout};
use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast;
use web_sys::{CloseEvent, MessageEvent, WebSocket};
use yew::Callback;

/// Manages a WebSocket connection with auto-reconnect and ping keepalive.
pub struct WsConnection {
    ws: Rc<RefCell<Option<WebSocket>>>,
    on_message: Callback<String>,
    reconnect_attempts: Rc<RefCell<u32>>,
    _ping_interval: Rc<RefCell<Option<Interval>>>,
    // Store closures so they don't get dropped
    _closures: Rc<RefCell<Vec<Closure<dyn FnMut(web_sys::Event)>>>>,
    _msg_closures: Rc<RefCell<Vec<Closure<dyn FnMut(MessageEvent)>>>>,
    _close_closures: Rc<RefCell<Vec<Closure<dyn FnMut(CloseEvent)>>>>,
}

impl WsConnection {
    pub fn new(on_message: Callback<String>) -> Self {
        let conn = Self {
            ws: Rc::new(RefCell::new(None)),
            on_message,
            reconnect_attempts: Rc::new(RefCell::new(0)),
            _ping_interval: Rc::new(RefCell::new(None)),
            _closures: Rc::new(RefCell::new(Vec::new())),
            _msg_closures: Rc::new(RefCell::new(Vec::new())),
            _close_closures: Rc::new(RefCell::new(Vec::new())),
        };
        conn.connect();
        conn
    }

    fn get_ws_url() -> String {
        let backend = crate::config::get_backend_url();
        let ws_base = if backend.is_empty() {
            // Same-origin: derive from window.location
            let window = web_sys::window().expect("no window");
            let location = window.location();
            let protocol = if location.protocol().unwrap_or_default() == "https:" {
                "wss:"
            } else {
                "ws:"
            };
            let host = location.host().unwrap_or_default();
            format!("{}//{}",  protocol, host)
        } else {
            backend
                .replace("https://", "wss://")
                .replace("http://", "ws://")
        };
        format!("{}/api/ws", ws_base)
    }

    pub fn connect(&self) {
        let url = Self::get_ws_url();
        gloo_console::log!("WebSocket connecting to", &url);

        let ws = match WebSocket::new(&url) {
            Ok(ws) => ws,
            Err(e) => {
                gloo_console::error!("WebSocket creation failed:", format!("{:?}", e));
                self.schedule_reconnect();
                return;
            }
        };

        // On message
        let on_message = self.on_message.clone();
        let onmessage_cb = Closure::wrap(Box::new(move |e: MessageEvent| {
            if let Ok(text) = e.data().dyn_into::<js_sys::JsString>() {
                on_message.emit(String::from(text));
            }
        }) as Box<dyn FnMut(MessageEvent)>);
        ws.set_onmessage(Some(onmessage_cb.as_ref().unchecked_ref()));
        self._msg_closures.borrow_mut().push(onmessage_cb);

        // On close - auto-reconnect
        let reconnect_attempts = self.reconnect_attempts.clone();
        let ws_ref = self.ws.clone();
        let on_message_for_reconnect = self.on_message.clone();
        let ping_interval = self._ping_interval.clone();
        let onclose_cb = Closure::wrap(Box::new(move |_: CloseEvent| {
            gloo_console::log!("WebSocket closed, scheduling reconnect...");
            *ws_ref.borrow_mut() = None;
            // Stop ping
            *ping_interval.borrow_mut() = None;

            let attempts = *reconnect_attempts.borrow();
            let delay_ms = std::cmp::min(1000 * 2u32.pow(attempts), 30_000);
            *reconnect_attempts.borrow_mut() = attempts + 1;

            let on_msg = on_message_for_reconnect.clone();
            Timeout::new(delay_ms, move || {
                let new_conn = WsConnection::new(on_msg);
                // The new connection is self-managing; we just need it to not be dropped
                // It will reconnect and manage itself
                std::mem::forget(new_conn);
            })
            .forget();
        }) as Box<dyn FnMut(CloseEvent)>);
        ws.set_onclose(Some(onclose_cb.as_ref().unchecked_ref()));
        self._close_closures.borrow_mut().push(onclose_cb);

        // On open - reset reconnect counter, start ping
        let reconnect_attempts = self.reconnect_attempts.clone();
        let ws_for_ping = ws.clone();
        let ping_interval = self._ping_interval.clone();
        let onopen_cb = Closure::wrap(Box::new(move |_: web_sys::Event| {
            gloo_console::log!("WebSocket connected");
            *reconnect_attempts.borrow_mut() = 0;

            // Start ping every 30s
            let ws_ping = ws_for_ping.clone();
            let interval = Interval::new(30_000, move || {
                if ws_ping.ready_state() == WebSocket::OPEN {
                    let _ = ws_ping.send_with_str("{\"type\":\"ping\"}");
                }
            });
            *ping_interval.borrow_mut() = Some(interval);
        }) as Box<dyn FnMut(web_sys::Event)>);
        ws.set_onopen(Some(onopen_cb.as_ref().unchecked_ref()));
        self._closures.borrow_mut().push(onopen_cb);

        *self.ws.borrow_mut() = Some(ws);
    }

    fn schedule_reconnect(&self) {
        let attempts = *self.reconnect_attempts.borrow();
        let delay_ms = std::cmp::min(1000 * 2u32.pow(attempts), 30_000);
        *self.reconnect_attempts.borrow_mut() = attempts + 1;

        let on_msg = self.on_message.clone();
        Timeout::new(delay_ms, move || {
            let new_conn = WsConnection::new(on_msg);
            std::mem::forget(new_conn);
        })
        .forget();
    }

    pub fn send(&self, msg: &str) -> bool {
        if let Some(ws) = self.ws.borrow().as_ref() {
            if ws.ready_state() == WebSocket::OPEN {
                let _ = ws.send_with_str(msg);
                return true;
            }
        }
        false
    }

    pub fn is_connected(&self) -> bool {
        self.ws
            .borrow()
            .as_ref()
            .map(|ws| ws.ready_state() == WebSocket::OPEN)
            .unwrap_or(false)
    }

    pub fn close(&self) {
        // Stop ping interval
        *self._ping_interval.borrow_mut() = None;
        if let Some(ws) = self.ws.borrow_mut().take() {
            // Clear callbacks to prevent reconnect
            ws.set_onclose(None);
            ws.set_onmessage(None);
            ws.set_onopen(None);
            let _ = ws.close();
        }
    }
}

impl Drop for WsConnection {
    fn drop(&mut self) {
        self.close();
    }
}
