// @generated automatically by Diesel CLI.

diesel::table! {
    admin_alerts (id) {
        id -> Nullable<Integer>,
        alert_type -> Text,
        severity -> Text,
        message -> Text,
        location -> Text,
        module -> Text,
        acknowledged -> Integer,
        created_at -> Integer,
    }
}

diesel::table! {
    country_availability (id) {
        id -> Integer,
        country_code -> Text,
        has_local_numbers -> Bool,
        outbound_sms_price -> Nullable<Float>,
        inbound_sms_price -> Nullable<Float>,
        outbound_voice_price_per_min -> Nullable<Float>,
        inbound_voice_price_per_min -> Nullable<Float>,
        last_checked -> Integer,
        created_at -> Integer,
    }
}

diesel::table! {
    disabled_alert_types (id) {
        id -> Nullable<Integer>,
        alert_type -> Text,
        disabled_at -> Integer,
    }
}

diesel::table! {
    message_status_log (id) {
        id -> Nullable<Integer>,
        message_sid -> Text,
        user_id -> Integer,
        direction -> Text,
        to_number -> Text,
        from_number -> Nullable<Text>,
        status -> Text,
        error_code -> Nullable<Text>,
        error_message -> Nullable<Text>,
        created_at -> Integer,
        updated_at -> Integer,
        price -> Nullable<Float>,
        price_unit -> Nullable<Text>,
    }
}

diesel::table! {
    refund_info (id) {
        id -> Nullable<Integer>,
        user_id -> Integer,
        has_refunded -> Integer,
        last_credit_pack_amount -> Nullable<Float>,
        last_credit_pack_purchase_timestamp -> Nullable<Integer>,
        refunded_at -> Nullable<Integer>,
    }
}

diesel::table! {
    site_metrics (id) {
        id -> Nullable<Integer>,
        metric_key -> Text,
        metric_value -> Text,
        updated_at -> Integer,
    }
}

diesel::table! {
    user_settings (id) {
        id -> Nullable<Integer>,
        user_id -> Integer,
        notify -> Bool,
        notification_type -> Nullable<Text>,
        timezone_auto -> Nullable<Bool>,
        agent_language -> Text,
        sub_country -> Nullable<Text>,
        save_context -> Nullable<Integer>,
        critical_enabled -> Nullable<Text>,
        elevenlabs_phone_number_id -> Nullable<Text>,
        notify_about_calls -> Bool,
        action_on_critical_message -> Nullable<Text>,
        phone_service_active -> Bool,
        default_notification_mode -> Nullable<Text>,
        default_notification_type -> Nullable<Text>,
        default_notify_on_call -> Integer,
        llm_provider -> Nullable<Text>,
        phone_contact_notification_mode -> Nullable<Text>,
        phone_contact_notification_type -> Nullable<Text>,
        phone_contact_notify_on_call -> Integer,
        auto_create_items -> Bool,
    }
}

diesel::table! {
    users (id) {
        id -> Integer,
        email -> Text,
        password_hash -> Text,
        phone_number -> Text,
        nickname -> Nullable<Text>,
        time_to_live -> Nullable<Integer>,
        credits -> Float,
        preferred_number -> Nullable<Text>,
        charge_when_under -> Bool,
        charge_back_to -> Nullable<Float>,
        stripe_customer_id -> Nullable<Text>,
        stripe_payment_method_id -> Nullable<Text>,
        stripe_checkout_session_id -> Nullable<Text>,
        sub_tier -> Nullable<Text>,
        credits_left -> Float,
        last_credits_notification -> Nullable<Integer>,
        next_billing_date_timestamp -> Nullable<Integer>,
        magic_token -> Nullable<Text>,
        plan_type -> Nullable<Text>,
        matrix_e2ee_enabled -> Bool,
    }
}

diesel::table! {
    waitlist (id) {
        id -> Nullable<Integer>,
        email -> Text,
        created_at -> Integer,
    }
}

diesel::joinable!(refund_info -> users (user_id));
diesel::joinable!(user_settings -> users (user_id));

diesel::allow_tables_to_appear_in_same_query!(
    admin_alerts,
    country_availability,
    disabled_alert_types,
    message_status_log,
    refund_info,
    site_metrics,
    user_settings,
    users,
    waitlist,
);
