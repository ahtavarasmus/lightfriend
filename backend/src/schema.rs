// @generated automatically by Diesel CLI.

diesel::table! {
    bridges (id) {
        id -> Nullable<Integer>,
        user_id -> Integer,
        bridge_type -> Text,
        status -> Text,
        room_id -> Nullable<Text>,
        data -> Nullable<Text>,
        created_at -> Nullable<Integer>,
    }
}

diesel::table! {
    calendar_notifications (id) {
        id -> Nullable<Integer>,
        user_id -> Integer,
        event_id -> Text,
        notification_time -> Integer,
    }
}

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
    email_judgments (id) {
        id -> Nullable<Integer>,
        user_id -> Integer,
        email_timestamp -> Integer,
        processed_at -> Integer,
        should_notify -> Bool,
        score -> Integer,
        reason -> Text,
    }
}

diesel::table! {
    gmail (id) {
        id -> Nullable<Integer>,
        user_id -> Integer,
        encrypted_access_token -> Text,
        encrypted_refresh_token -> Text,
        status -> Text,
        last_update -> Integer,
        created_on -> Integer,
        description -> Text,
        expires_in -> Integer,
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
        expires_in -> Integer,
    }
}

diesel::table! {
    google_tasks (id) {
        id -> Nullable<Integer>,
        user_id -> Integer,
        encrypted_access_token -> Text,
        encrypted_refresh_token -> Text,
        status -> Text,
        last_update -> Integer,
        created_on -> Integer,
        description -> Text,
        expires_in -> Integer,
    }
}

diesel::table! {
    imap_connection (id) {
        id -> Nullable<Integer>,
        user_id -> Integer,
        method -> Text,
        encrypted_password -> Text,
        status -> Text,
        last_update -> Integer,
        created_on -> Integer,
        description -> Text,
        expires_in -> Integer,
        imap_server -> Nullable<Text>,
        imap_port -> Nullable<Integer>,
    }
}

diesel::table! {
    importance_priorities (id) {
        id -> Nullable<Integer>,
        user_id -> Integer,
        threshold -> Integer,
        service_type -> Text,
    }
}

diesel::table! {
    keywords (id) {
        id -> Nullable<Integer>,
        user_id -> Integer,
        keyword -> Text,
        service_type -> Text,
    }
}

diesel::table! {
    priority_senders (id) {
        id -> Nullable<Integer>,
        user_id -> Integer,
        sender -> Text,
        service_type -> Text,
    }
}

diesel::table! {
    proactive_settings (id) {
        id -> Nullable<Integer>,
        user_id -> Integer,
        imap_proactive -> Bool,
        imap_general_checks -> Nullable<Text>,
        proactive_calendar -> Bool,
        created_at -> Integer,
        updated_at -> Integer,
        proactive_calendar_last_activated -> Integer,
        proactive_email_last_activated -> Integer,
        proactive_whatsapp -> Bool,
        whatsapp_general_checks -> Nullable<Text>,
    }
}

diesel::table! {
    processed_emails (id) {
        id -> Nullable<Integer>,
        user_id -> Integer,
        email_uid -> Text,
        processed_at -> Integer,
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
    task_notifications (id) {
        id -> Nullable<Integer>,
        user_id -> Integer,
        task_id -> Text,
        notified_at -> Integer,
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
        sid -> Nullable<Text>,
        activity_type -> Text,
        credits -> Nullable<Float>,
        created_at -> Integer,
        time_consumed -> Nullable<Integer>,
        success -> Nullable<Bool>,
        reason -> Nullable<Text>,
        status -> Nullable<Text>,
        recharge_threshold_timestamp -> Nullable<Integer>,
        zero_credits_timestamp -> Nullable<Integer>,
        call_duration -> Nullable<Integer>,
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
        matrix_username -> Nullable<Text>,
        encrypted_matrix_access_token -> Nullable<Text>,
        timezone -> Nullable<Text>,
        timezone_auto -> Nullable<Bool>,
        sub_tier -> Nullable<Text>,
        msgs_left -> Integer,
        matrix_device_id -> Nullable<Text>,
        credits_left -> Float,
        discount -> Bool,
        encrypted_matrix_password -> Nullable<Text>,
        encrypted_matrix_secret_storage_recovery_key -> Nullable<Text>,
        last_credits_notification -> Nullable<Integer>,
        confirm_send_event -> Bool,
    }
}

diesel::table! {
    waiting_checks (id) {
        id -> Nullable<Integer>,
        user_id -> Integer,
        due_date -> Integer,
        content -> Text,
        remove_when_found -> Bool,
        service_type -> Text,
    }
}

diesel::joinable!(bridges -> users (user_id));
diesel::joinable!(calendar_notifications -> users (user_id));
diesel::joinable!(conversations -> users (user_id));
diesel::joinable!(gmail -> users (user_id));
diesel::joinable!(imap_connection -> users (user_id));
diesel::joinable!(importance_priorities -> users (user_id));
diesel::joinable!(keywords -> users (user_id));
diesel::joinable!(priority_senders -> users (user_id));
diesel::joinable!(proactive_settings -> users (user_id));
diesel::joinable!(processed_emails -> users (user_id));
diesel::joinable!(subscriptions -> users (user_id));
diesel::joinable!(waiting_checks -> users (user_id));

diesel::allow_tables_to_appear_in_same_query!(
    bridges,
    calendar_notifications,
    conversations,
    email_judgments,
    gmail,
    google_calendar,
    google_tasks,
    imap_connection,
    importance_priorities,
    keywords,
    priority_senders,
    proactive_settings,
    processed_emails,
    subscriptions,
    task_notifications,
    unipile_connection,
    usage_logs,
    users,
    waiting_checks,
);
