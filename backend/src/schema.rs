// @generated automatically by Diesel CLI.

diesel::table! {
    conversations (id) {
        id -> Integer,
        user_id -> Integer,
        conversation_sid -> Text,
        service_sid -> Text,
        created_at -> Integer,
        active -> Bool,
        twilio_number -> Text,
        user_number -> Text,
    }
}

diesel::table! {
    google_calendar (id) {
        id -> Nullable<Integer>,
        user_id -> Integer,
        encrypted_access_token -> Text,
        encrypted_refresh_token -> Text,
        status -> Text,
        last_update -> Integer,
        created_on -> Integer,
        description -> Text,
    }
}

diesel::table! {
    subscriptions (id) {
        id -> Nullable<Integer>,
        user_id -> Integer,
        paddle_subscription_id -> Text,
        paddle_customer_id -> Text,
        stage -> Text,
        status -> Text,
        next_bill_date -> Integer,
        is_scheduled_to_cancel -> Nullable<Bool>,
    }
}

diesel::table! {
    unipile_connection (id) {
        id -> Nullable<Integer>,
        user_id -> Integer,
        account_type -> Text,
        account_id -> Text,
        status -> Text,
        last_update -> Integer,
        created_on -> Integer,
        description -> Text,
    }
}

diesel::table! {
    usage_logs (id) {
        id -> Nullable<Integer>,
        user_id -> Integer,
        conversation_id -> Nullable<Text>,
        status -> Nullable<Text>,
        activity_type -> Text,
        credits -> Nullable<Float>,
        created_at -> Integer,
        success -> Nullable<Bool>,
        summary -> Nullable<Text>,
        recharge_threshold_timestamp -> Nullable<Integer>,
        zero_credits_timestamp -> Nullable<Integer>,
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
        verified -> Bool,
        credits -> Float,
        notify -> Bool,
        info -> Nullable<Text>,
        preferred_number -> Nullable<Text>,
        debug_logging_permission -> Bool,
        charge_when_under -> Bool,
        charge_back_to -> Nullable<Float>,
        stripe_customer_id -> Nullable<Text>,
        stripe_payment_method_id -> Nullable<Text>,
        stripe_checkout_session_id -> Nullable<Text>,
    }
}

diesel::joinable!(conversations -> users (user_id));
diesel::joinable!(subscriptions -> users (user_id));

diesel::allow_tables_to_appear_in_same_query!(
    conversations,
    google_calendar,
    subscriptions,
    unipile_connection,
    usage_logs,
    users,
);
