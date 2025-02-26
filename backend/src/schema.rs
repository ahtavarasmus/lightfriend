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
    usage_logs (id) {
        id -> Nullable<Integer>,
        user_id -> Integer,
        activity_type -> Text,
        iq_used -> Integer,
        iq_cost_per_euro -> Integer,
        created_at -> Integer,
        success -> Bool,
        summary -> Nullable<Text>,
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
        iq -> Integer,
        notify_credits -> Bool,
        locality -> Text,
        info -> Nullable<Text>,
        preferred_number -> Nullable<Text>,
        iq_cost_per_euro -> Integer,
        debug_logging_permission -> Bool,
    }
}

diesel::joinable!(conversations -> users (user_id));
diesel::joinable!(subscriptions -> users (user_id));
diesel::joinable!(usage_logs -> users (user_id));

diesel::allow_tables_to_appear_in_same_query!(
    conversations,
    subscriptions,
    usage_logs,
    users,
);
