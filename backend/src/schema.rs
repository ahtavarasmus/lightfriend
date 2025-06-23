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
    idea_email_subscriptions (id) {
        id -> Nullable<Integer>,
        idea_id -> Integer,
        email -> Text,
        created_at -> Integer,
    }
}

diesel::table! {
    idea_upvotes (id) {
        id -> Nullable<Integer>,
        idea_id -> Integer,
        voter_id -> Text,
        created_at -> Integer,
    }
}

diesel::table! {
    ideas (id) {
        id -> Nullable<Integer>,
        creator_id -> Text,
        text -> Text,
        created_at -> Integer,
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
    message_history (id) {
        id -> Nullable<Integer>,
        user_id -> Integer,
        role -> Text,
        encrypted_content -> Text,
        tool_name -> Nullable<Text>,
        tool_call_id -> Nullable<Text>,
        created_at -> Integer,
        conversation_id -> Text,
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
        whatsapp_keywords_active -> Bool,
        whatsapp_priority_senders_active -> Bool,
        whatsapp_waiting_checks_active -> Bool,
        whatsapp_general_importance_active -> Bool,
        email_keywords_active -> Bool,
        email_priority_senders_active -> Bool,
        email_waiting_checks_active -> Bool,
        email_general_importance_active -> Bool,
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
    temp_variables (id) {
        id -> Integer,
        user_id -> Integer,
        confirm_send_event_type -> Text,
        confirm_send_event_recipient -> Nullable<Text>,
        confirm_send_event_subject -> Nullable<Text>,
        confirm_send_event_content -> Nullable<Text>,
        confirm_send_event_start_time -> Nullable<Text>,
        confirm_send_event_duration -> Nullable<Text>,
        confirm_send_event_id -> Nullable<Text>,
        confirm_send_event_image_url -> Nullable<Text>,
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
    user_settings (id) {
        id -> Nullable<Integer>,
        user_id -> Integer,
        notify -> Bool,
        notification_type -> Nullable<Text>,
        timezone -> Nullable<Text>,
        timezone_auto -> Nullable<Bool>,
        agent_language -> Text,
        sub_country -> Nullable<Text>,
        save_context -> Nullable<Integer>,
        info -> Nullable<Text>,
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
        preferred_number -> Nullable<Text>,
        charge_when_under -> Bool,
        charge_back_to -> Nullable<Float>,
        stripe_customer_id -> Nullable<Text>,
        stripe_payment_method_id -> Nullable<Text>,
        stripe_checkout_session_id -> Nullable<Text>,
        matrix_username -> Nullable<Text>,
        encrypted_matrix_access_token -> Nullable<Text>,
        sub_tier -> Nullable<Text>,
        msgs_left -> Integer,
        matrix_device_id -> Nullable<Text>,
        credits_left -> Float,
        encrypted_matrix_password -> Nullable<Text>,
        encrypted_matrix_secret_storage_recovery_key -> Nullable<Text>,
        last_credits_notification -> Nullable<Integer>,
        discount -> Bool,
        discount_tier -> Nullable<Text>,
        free_reply -> Bool,
        confirm_send_event -> Nullable<Text>,
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
diesel::joinable!(idea_email_subscriptions -> ideas (idea_id));
diesel::joinable!(idea_upvotes -> ideas (idea_id));
diesel::joinable!(imap_connection -> users (user_id));
diesel::joinable!(importance_priorities -> users (user_id));
diesel::joinable!(keywords -> users (user_id));
diesel::joinable!(message_history -> users (user_id));
diesel::joinable!(priority_senders -> users (user_id));
diesel::joinable!(proactive_settings -> users (user_id));
diesel::joinable!(processed_emails -> users (user_id));
diesel::joinable!(subscriptions -> users (user_id));
diesel::joinable!(temp_variables -> users (user_id));
diesel::joinable!(user_settings -> users (user_id));
diesel::joinable!(waiting_checks -> users (user_id));

diesel::allow_tables_to_appear_in_same_query!(
    bridges,
    calendar_notifications,
    conversations,
    email_judgments,
    gmail,
    google_calendar,
    google_tasks,
    idea_email_subscriptions,
    idea_upvotes,
    ideas,
    imap_connection,
    importance_priorities,
    keywords,
    message_history,
    priority_senders,
    proactive_settings,
    processed_emails,
    subscriptions,
    task_notifications,
    temp_variables,
    unipile_connection,
    usage_logs,
    user_settings,
    users,
    waiting_checks,
);
