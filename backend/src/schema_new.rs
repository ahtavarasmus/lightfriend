// @generated automatically by Diesel CLI.

diesel::table! {
    bridges (id) {
        bridge_type -> Text,
        created_at -> Nullable<Integer>,
        data -> Nullable<Text>,
        id -> Nullable<Integer>,
        last_seen_online -> Nullable<Integer>,
        room_id -> Nullable<Text>,
        status -> Text,
        user_id -> Integer,
    }
}

diesel::table! {
    calendar_notifications (id) {
        event_id -> Text,
        id -> Nullable<Integer>,
        notification_time -> Integer,
        user_id -> Integer,
    }
}

diesel::table! {
    conversations (id) {
        active -> Bool,
        conversation_sid -> Text,
        created_at -> Integer,
        id -> Integer,
        service_sid -> Text,
        twilio_number -> Text,
        user_id -> Integer,
        user_number -> Text,
    }
}

diesel::table! {
    country_availability (id) {
        country_code -> Text,
        created_at -> Integer,
        has_local_numbers -> Bool,
        id -> Integer,
        inbound_sms_price -> Nullable<Float>,
        inbound_voice_price_per_min -> Nullable<Float>,
        last_checked -> Integer,
        outbound_sms_price -> Nullable<Float>,
        outbound_voice_price_per_min -> Nullable<Float>,
    }
}

diesel::table! {
    critical_categories (id) {
        active -> Bool,
        category_name -> Text,
        definition -> Nullable<Text>,
        id -> Nullable<Integer>,
        user_id -> Integer,
    }
}

diesel::table! {
    email_judgments (id) {
        email_timestamp -> Integer,
        id -> Nullable<Integer>,
        processed_at -> Integer,
        reason -> Text,
        score -> Integer,
        should_notify -> Bool,
        user_id -> Integer,
    }
}

diesel::table! {
    google_calendar (id) {
        created_on -> Integer,
        description -> Text,
        encrypted_access_token -> Text,
        encrypted_refresh_token -> Text,
        expires_in -> Integer,
        id -> Nullable<Integer>,
        last_update -> Integer,
        status -> Text,
        user_id -> Integer,
    }
}

diesel::table! {
    google_tasks (id) {
        created_on -> Integer,
        description -> Text,
        encrypted_access_token -> Text,
        encrypted_refresh_token -> Text,
        expires_in -> Integer,
        id -> Nullable<Integer>,
        last_update -> Integer,
        status -> Text,
        user_id -> Integer,
    }
}

diesel::table! {
    imap_connection (id) {
        created_on -> Integer,
        description -> Text,
        encrypted_password -> Text,
        expires_in -> Integer,
        id -> Nullable<Integer>,
        imap_port -> Nullable<Integer>,
        imap_server -> Nullable<Text>,
        last_update -> Integer,
        method -> Text,
        status -> Text,
        user_id -> Integer,
    }
}

diesel::table! {
    keywords (id) {
        id -> Nullable<Integer>,
        keyword -> Text,
        service_type -> Text,
        user_id -> Integer,
    }
}

diesel::table! {
    message_history (id) {
        conversation_id -> Text,
        created_at -> Integer,
        encrypted_content -> Text,
        id -> Nullable<Integer>,
        role -> Text,
        tool_call_id -> Nullable<Text>,
        tool_calls_json -> Nullable<Text>,
        tool_name -> Nullable<Text>,
        user_id -> Integer,
    }
}

diesel::table! {
    priority_senders (id) {
        id -> Nullable<Integer>,
        noti_mode -> Text,
        noti_type -> Nullable<Text>,
        sender -> Text,
        service_type -> Text,
        user_id -> Integer,
    }
}

diesel::table! {
    processed_emails (id) {
        email_uid -> Text,
        id -> Nullable<Integer>,
        processed_at -> Integer,
        user_id -> Integer,
    }
}

diesel::table! {
    refund_info (id) {
        has_refunded -> Integer,
        id -> Nullable<Integer>,
        last_credit_pack_amount -> Nullable<Float>,
        last_credit_pack_purchase_timestamp -> Nullable<Integer>,
        refunded_at -> Nullable<Integer>,
        user_id -> Integer,
    }
}

diesel::table! {
    subaccounts (id) {
        auth_token -> Text,
        cost_this_month -> Nullable<Float>,
        country -> Nullable<Text>,
        country_code -> Nullable<Text>,
        created_at -> Nullable<Integer>,
        id -> Integer,
        messaging_service_sid -> Nullable<Text>,
        number -> Nullable<Text>,
        status -> Nullable<Text>,
        subaccount_sid -> Text,
        subaccount_type -> Text,
        tinfoil_key -> Nullable<Text>,
        user_id -> Text,
    }
}

diesel::table! {
    task_notifications (id) {
        id -> Nullable<Integer>,
        notified_at -> Integer,
        task_id -> Text,
        user_id -> Integer,
    }
}

diesel::table! {
    tesla (id) {
        created_on -> Integer,
        encrypted_access_token -> Text,
        encrypted_refresh_token -> Text,
        expires_in -> Integer,
        id -> Nullable<Integer>,
        last_update -> Integer,
        region -> Text,
        selected_vehicle_id -> Nullable<Text>,
        selected_vehicle_name -> Nullable<Text>,
        selected_vehicle_vin -> Nullable<Text>,
        status -> Text,
        user_id -> Integer,
        virtual_key_paired -> Integer,
    }
}

diesel::table! {
    totp_backup_codes (id) {
        code_hash -> Text,
        id -> Nullable<Integer>,
        used -> Integer,
        user_id -> Integer,
    }
}

diesel::table! {
    totp_secrets (id) {
        created_at -> Integer,
        enabled -> Integer,
        encrypted_secret -> Text,
        id -> Nullable<Integer>,
        user_id -> Integer,
    }
}

diesel::table! {
    uber (id) {
        created_on -> Integer,
        description -> Text,
        encrypted_access_token -> Text,
        encrypted_refresh_token -> Text,
        expires_in -> Integer,
        id -> Nullable<Integer>,
        last_update -> Integer,
        status -> Text,
        user_id -> Integer,
    }
}

diesel::table! {
    usage_logs (id) {
        activity_type -> Text,
        call_duration -> Nullable<Integer>,
        created_at -> Integer,
        credits -> Nullable<Float>,
        id -> Nullable<Integer>,
        reason -> Nullable<Text>,
        recharge_threshold_timestamp -> Nullable<Integer>,
        sid -> Nullable<Text>,
        status -> Nullable<Text>,
        success -> Nullable<Bool>,
        time_consumed -> Nullable<Integer>,
        user_id -> Integer,
        zero_credits_timestamp -> Nullable<Integer>,
    }
}

diesel::table! {
    user_info (id) {
        blocker_password_vault -> Nullable<Text>,
        dictionary -> Nullable<Text>,
        id -> Nullable<Integer>,
        info -> Nullable<Text>,
        location -> Nullable<Text>,
        lockbox_password_vault -> Nullable<Text>,
        nearby_places -> Nullable<Text>,
        recent_contacts -> Nullable<Text>,
        timezone -> Nullable<Text>,
        user_id -> Integer,
    }
}

diesel::table! {
    user_settings (id) {
        action_on_critical_message -> Nullable<Text>,
        agent_language -> Text,
        critical_enabled -> Nullable<Text>,
        day_digest -> Nullable<Text>,
        elevenlabs_phone_number_id -> Nullable<Text>,
        encrypted_geoapify_key -> Nullable<Text>,
        encrypted_openrouter_api_key -> Nullable<Text>,
        encrypted_pirate_weather_key -> Nullable<Text>,
        encrypted_textbee_api_key -> Nullable<Text>,
        encrypted_textbee_device_id -> Nullable<Text>,
        encrypted_twilio_account_sid -> Nullable<Text>,
        encrypted_twilio_auth_token -> Nullable<Text>,
        evening_digest -> Nullable<Text>,
        id -> Nullable<Integer>,
        last_instant_digest_time -> Nullable<Integer>,
        magic_login_token -> Nullable<Text>,
        magic_login_token_expiration_timestamp -> Nullable<Integer>,
        monthly_message_count -> Integer,
        morning_digest -> Nullable<Text>,
        notification_type -> Nullable<Text>,
        notify -> Bool,
        notify_about_calls -> Bool,
        notify_on_climate_ready -> Bool,
        number_of_digests_locked -> Integer,
        outbound_message_pricing -> Nullable<Float>,
        phone_service_active -> Bool,
        proactive_agent_on -> Bool,
        save_context -> Nullable<Integer>,
        server_ip -> Nullable<Text>,
        server_url -> Nullable<Text>,
        sub_country -> Nullable<Text>,
        timezone_auto -> Nullable<Bool>,
        user_id -> Integer,
    }
}

diesel::table! {
    users (id) {
        charge_back_to -> Nullable<Float>,
        charge_when_under -> Bool,
        confirm_send_event -> Nullable<Text>,
        credits -> Float,
        credits_left -> Float,
        discount -> Bool,
        discount_tier -> Nullable<Text>,
        email -> Text,
        encrypted_matrix_access_token -> Nullable<Text>,
        encrypted_matrix_password -> Nullable<Text>,
        encrypted_matrix_secret_storage_recovery_key -> Nullable<Text>,
        free_reply -> Bool,
        id -> Integer,
        last_credits_notification -> Nullable<Integer>,
        magic_token -> Nullable<Text>,
        matrix_device_id -> Nullable<Text>,
        matrix_username -> Nullable<Text>,
        next_billing_date_timestamp -> Nullable<Integer>,
        nickname -> Nullable<Text>,
        password_hash -> Text,
        phone_number -> Text,
        phone_number_country -> Nullable<Text>,
        plan_type -> Nullable<Text>,
        preferred_number -> Nullable<Text>,
        stripe_checkout_session_id -> Nullable<Text>,
        stripe_customer_id -> Nullable<Text>,
        stripe_payment_method_id -> Nullable<Text>,
        sub_tier -> Nullable<Text>,
        time_to_live -> Nullable<Integer>,
        verified -> Bool,
        waiting_checks_count -> Integer,
    }
}

diesel::table! {
    waiting_checks (id) {
        content -> Text,
        id -> Nullable<Integer>,
        noti_type -> Nullable<Text>,
        service_type -> Text,
        user_id -> Integer,
    }
}

diesel::table! {
    waitlist (id) {
        created_at -> Integer,
        email -> Text,
        id -> Nullable<Integer>,
    }
}

diesel::table! {
    webauthn_challenges (id) {
        challenge -> Text,
        challenge_type -> Text,
        context -> Nullable<Text>,
        created_at -> Integer,
        expires_at -> Integer,
        id -> Nullable<Integer>,
        user_id -> Integer,
    }
}

diesel::table! {
    webauthn_credentials (id) {
        aaguid -> Nullable<Text>,
        counter -> Integer,
        created_at -> Integer,
        credential_id -> Text,
        device_name -> Text,
        enabled -> Integer,
        encrypted_public_key -> Text,
        id -> Nullable<Integer>,
        last_used_at -> Nullable<Integer>,
        transports -> Nullable<Text>,
        user_id -> Integer,
    }
}

diesel::table! {
    youtube (id) {
        created_on -> Integer,
        description -> Text,
        encrypted_access_token -> Text,
        encrypted_refresh_token -> Text,
        expires_in -> Integer,
        id -> Nullable<Integer>,
        last_update -> Integer,
        status -> Text,
        user_id -> Integer,
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
diesel::joinable!(refund_info -> users (user_id));
diesel::joinable!(tesla -> users (user_id));
diesel::joinable!(totp_backup_codes -> users (user_id));
diesel::joinable!(totp_secrets -> users (user_id));
diesel::joinable!(user_info -> users (user_id));
diesel::joinable!(user_settings -> users (user_id));
diesel::joinable!(waiting_checks -> users (user_id));
diesel::joinable!(webauthn_challenges -> users (user_id));
diesel::joinable!(webauthn_credentials -> users (user_id));

diesel::allow_tables_to_appear_in_same_query!(
    bridges,
    calendar_notifications,
    conversations,
    country_availability,
    critical_categories,
    email_judgments,
    google_calendar,
    google_tasks,
    imap_connection,
    keywords,
    message_history,
    priority_senders,
    processed_emails,
    refund_info,
    subaccounts,
    task_notifications,
    tesla,
    totp_backup_codes,
    totp_secrets,
    uber,
    usage_logs,
    user_info,
    user_settings,
    users,
    waiting_checks,
    waitlist,
    webauthn_challenges,
    webauthn_credentials,
    youtube,
);
