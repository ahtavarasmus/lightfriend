use js_sys::{Function, Reflect};
use wasm_bindgen::{JsCast, JsValue};

const PENDING_PAYMENT_KEY: &str = "lightfriend_datafast_payment_pending";
const PAYMENT_TRACKER_NAME: &str = "lightfriendTrackDataFastPayment";

pub fn mark_payment_pending() {
    let Some(window) = web_sys::window() else {
        return;
    };
    let Ok(Some(storage)) = window.local_storage() else {
        return;
    };

    let _ = storage.set_item(PENDING_PAYMENT_KEY, "true");
}

pub fn attribute_pending_payment(email: &str) {
    if email.trim().is_empty() {
        return;
    }

    let Some(window) = web_sys::window() else {
        return;
    };
    let Ok(Some(storage)) = window.local_storage() else {
        return;
    };
    if storage
        .get_item(PENDING_PAYMENT_KEY)
        .ok()
        .flatten()
        .as_deref()
        != Some("true")
    {
        return;
    }

    let Ok(tracker) = Reflect::get(window.as_ref(), &JsValue::from_str(PAYMENT_TRACKER_NAME))
    else {
        return;
    };
    let Ok(tracker) = tracker.dyn_into::<Function>() else {
        return;
    };
    let Ok(attributed) = tracker.call1(&JsValue::NULL, &JsValue::from_str(email.trim())) else {
        return;
    };

    if attributed.as_bool() == Some(true) {
        let _ = storage.remove_item(PENDING_PAYMENT_KEY);
    }
}
