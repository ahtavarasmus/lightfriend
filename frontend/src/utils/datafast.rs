use js_sys::{Function, Object, Reflect};
use wasm_bindgen::{JsCast, JsValue};

const PENDING_PAYMENT_KEY: &str = "lightfriend_datafast_payment_pending";
const PAYMENT_TRACKER_NAME: &str = "lightfriendTrackDataFastPayment";
const GOAL_TRACKER_NAME: &str = "lightfriendTrackDataFastGoal";
const GOAL_ONCE_TRACKER_NAME: &str = "lightfriendTrackDataFastGoalOnce";

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

    if attribute_payment(email) {
        let _ = storage.remove_item(PENDING_PAYMENT_KEY);
    }
}

pub fn attribute_payment(email: &str) -> bool {
    if email.trim().is_empty() {
        return false;
    }

    let Some(window) = web_sys::window() else {
        return false;
    };
    let Ok(tracker) = Reflect::get(window.as_ref(), &JsValue::from_str(PAYMENT_TRACKER_NAME))
    else {
        return false;
    };
    let Ok(tracker) = tracker.dyn_into::<Function>() else {
        return false;
    };
    let Ok(attributed) = tracker.call1(&JsValue::NULL, &JsValue::from_str(email.trim())) else {
        return false;
    };

    attributed.as_bool() == Some(true)
}

fn goal_metadata(metadata: &[(&str, &str)]) -> Object {
    let values = Object::new();
    for (key, value) in metadata {
        let _ = Reflect::set(
            values.as_ref(),
            &JsValue::from_str(key),
            &JsValue::from_str(value),
        );
    }
    values
}

pub fn track_goal(name: &str, metadata: &[(&str, &str)]) -> bool {
    if name.trim().is_empty() {
        return false;
    }

    let Some(window) = web_sys::window() else {
        return false;
    };
    let Ok(tracker) = Reflect::get(window.as_ref(), &JsValue::from_str(GOAL_TRACKER_NAME)) else {
        return false;
    };
    let Ok(tracker) = tracker.dyn_into::<Function>() else {
        return false;
    };
    let Ok(tracked) = tracker.call2(
        &JsValue::NULL,
        &JsValue::from_str(name.trim()),
        goal_metadata(metadata).as_ref(),
    ) else {
        return false;
    };

    tracked.as_bool() == Some(true)
}

pub fn track_goal_once(name: &str, dedupe_key: &str, metadata: &[(&str, &str)]) -> bool {
    if name.trim().is_empty() || dedupe_key.trim().is_empty() {
        return false;
    }

    let Some(window) = web_sys::window() else {
        return false;
    };
    let Ok(tracker) = Reflect::get(window.as_ref(), &JsValue::from_str(GOAL_ONCE_TRACKER_NAME))
    else {
        return false;
    };
    let Ok(tracker) = tracker.dyn_into::<Function>() else {
        return false;
    };
    let Ok(tracked) = tracker.call3(
        &JsValue::NULL,
        &JsValue::from_str(name.trim()),
        &JsValue::from_str(dedupe_key.trim()),
        goal_metadata(metadata).as_ref(),
    ) else {
        return false;
    };

    tracked.as_bool() == Some(true)
}
