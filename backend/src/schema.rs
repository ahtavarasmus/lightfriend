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
        last_seen_online -> Nullable<Integer>,
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
    critical_categories (id) {
        id -> Nullable<Integer>,
        user_id -> Integer,
        category_name -> Text,
        definition -> Nullable<Text>,
        active -> Bool,
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
    keywords (id) {
        id -> Nullable<Integer>,
        user_id -> Integer,
        keyword -> Text,
        service_type -> Text,
    }
}

diesel::table! {
    message_history (id) {
        id -> Nullable<Integer>,
        user_id -> Integer,
        role -> Text,
        encrypted_content -> Text,
        tool_name -> Nullable<Text>,
        tool_call_id -> Nullable<Text>,
        created_at -> Integer,
        conversation_id -> Text,
        tool_calls_json -> Nullable<Text>,
    }
}

diesel::table! {
    priority_senders (id) {
        id -> Nullable<Integer>,
        user_id -> Integer,
        sender -> Text,
        service_type -> Text,
        noti_type -> Nullable<Text>,
        noti_mode -> Text,
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
    task_notifications (id) {
        id -> Nullable<Integer>,
        user_id -> Integer,
        task_id -> Text,
        notified_at -> Integer,
    }
}

diesel::table! {
    uber (id) {
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
    user_info (id) {
        id -> Nullable<Integer>,
        user_id -> Integer,
        location -> Nullable<Text>,
        dictionary -> Nullable<Text>,
        info -> Nullable<Text>,
        timezone -> Nullable<Text>,
        nearby_places -> Nullable<Text>,
        recent_contacts -> Nullable<Text>,
        blocker_password_vault -> Nullable<Text>,
        lockbox_password_vault -> Nullable<Text>,
    }
}

diesel::table! {
    user_settings (id) {
        id -> Nullable<Integer>,
        user_id -> Integer,
        notification_type -> Nullable<Text>,
        timezone_auto -> Nullable<Bool>,
        agent_language -> Text,
        save_context -> Nullable<Integer>,
        morning_digest -> Nullable<Text>,
        day_digest -> Nullable<Text>,
        evening_digest -> Nullable<Text>,
        critical_enabled -> Nullable<Text>,
        server_url -> Nullable<Text>,
        encrypted_textbee_device_id -> Nullable<Text>,
        encrypted_textbee_api_key -> Nullable<Text>,
        elevenlabs_phone_number_id -> Nullable<Text>,
        proactive_agent_on -> Bool,
        notify_about_calls -> Bool,
        action_on_critical_message -> Nullable<Text>,
    }
}

diesel::table! {
    users (id) {
        id -> Integer,
        phone_number -> Text,
        nickname -> Nullable<Text>,
        credits -> Float,
        preferred_number -> Nullable<Text>,
        matrix_username -> Nullable<Text>,
        encrypted_matrix_access_token -> Nullable<Text>,
        matrix_device_id -> Nullable<Text>,
        credits_left -> Float,
        encrypted_matrix_password -> Nullable<Text>,
        phone_number_country -> Nullable<Text>,
        twilio_account_sid -> Nullable<Text>,
        twilio_auth_token -> Nullable<Text>,
        server_url -> Nullable<Text>,
        twilio_messaging_service_sid -> Nullable<Text>,
        tinfoil_api_key -> Nullable<Text>,
    }
}

diesel::table! {
    waiting_checks (id) {
        id -> Nullable<Integer>,
        user_id -> Integer,
        content -> Text,
        service_type -> Text,
        noti_type -> Nullable<Text>,
    }
}

diesel::joinable!(bridges -> users (user_id));
diesel::joinable!(calendar_notifications -> users (user_id));
diesel::joinable!(conversations -> users (user_id));
diesel::joinable!(imap_connection -> users (user_id));
diesel::joinable!(keywords -> users (user_id));
diesel::joinable!(message_history -> users (user_id));
diesel::joinable!(priority_senders -> users (user_id));
diesel::joinable!(processed_emails -> users (user_id));
diesel::joinable!(user_info -> users (user_id));
diesel::joinable!(user_settings -> users (user_id));
diesel::joinable!(waiting_checks -> users (user_id));

diesel::allow_tables_to_appear_in_same_query!(
    bridges,
    calendar_notifications,
    conversations,
    critical_categories,
    email_judgments,
    google_calendar,
    google_tasks,
    imap_connection,
    keywords,
    message_history,
    priority_senders,
    processed_emails,
    task_notifications,
    uber,
    usage_logs,
    user_info,
    user_settings,
    users,
    waiting_checks,
);
