use yew::prelude::*;
use gloo_net::http::Request;
use wasm_bindgen_futures::spawn_local;
use web_sys::{window, HtmlInputElement, KeyboardEvent, InputEvent};
use serde_json::json;

use crate::pages::proactive::{PrioritySender, WaitingCheck, ImportancePriority};
use super::common::{KeywordsSection, PrioritySendersSection, ImportancePrioritySection};
