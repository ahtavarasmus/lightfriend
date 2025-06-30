use web_sys::{window, js_sys, HtmlInputElement, KeyboardEvent, InputEvent, Event};
use wasm_bindgen::JsValue;

pub(crate) fn format_timestamp(ts: i32) -> String {
    let date = js_sys::Date::new(&js_sys::Number::from(ts as f64 * 1000.0));
    let opts = js_sys::Object::new();
    js_sys::Reflect::set(&opts, &JsValue::from_str("year"),  &JsValue::from_str("numeric")).unwrap();
    js_sys::Reflect::set(&opts, &JsValue::from_str("month"), &JsValue::from_str("long")).unwrap();
    js_sys::Reflect::set(&opts, &JsValue::from_str("day"),   &JsValue::from_str("numeric")).unwrap();
    date.to_locale_string("en-US", &opts).into()
}

