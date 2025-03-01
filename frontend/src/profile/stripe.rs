use yew::prelude::*;
use web_sys::{window, Element, HtmlElement};
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::spawn_local;
use gloo_net::http::Request;
use serde::{Deserialize, Serialize};
use serde_json::json;
use crate::config;
use wasm_bindgen::prelude::wasm_bindgen;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = window)]
    fn open(url: &str, target: &str, features: &str);
}


