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
    bridge_disconnection_events (id) {
        id -> Nullable<Integer>,
        user_id -> Integer,
        bridge_type -> Text,
        detected_at -> Integer,
    }
}

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
    contact_profile_exceptions (id) {
        id -> Nullable<Integer>,
        profile_id -> Integer,
        platform -> Text,
        notification_mode -> Text,
        notification_type -> Text,
        notify_on_call -> Integer,
    }
}

diesel::table! {
    contact_profiles (id) {
        id -> Nullable<Integer>,
        user_id -> Integer,
        nickname -> Text,
        whatsapp_chat -> Nullable<Text>,
        telegram_chat -> Nullable<Text>,
        signal_chat -> Nullable<Text>,
        email_addresses -> Nullable<Text>,
        notification_mode -> Text,
        notification_type -> Text,
        notify_on_call -> Integer,
        created_at -> Integer,
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
    critical_categories (id) {
        id -> Nullable<Integer>,
        user_id -> Integer,
        category_name -> Text,
        definition -> Nullable<Text>,
        active -> Bool,
    }
}

diesel::table! {
    digests (id) {
        id -> Nullable<Integer>,
        user_id -> Integer,
        time -> Text,
        tools -> Text,
        tool_params -> Nullable<Text>,
        enabled -> Integer,
        last_sent_at -> Nullable<Integer>,
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
    subaccounts (id) {
        id -> Integer,
        user_id -> Text,
        subaccount_sid -> Text,
        auth_token -> Text,
        country -> Nullable<Text>,
        number -> Nullable<Text>,
        cost_this_month -> Nullable<Float>,
        created_at -> Nullable<Integer>,
        status -> Nullable<Text>,
        tinfoil_key -> Nullable<Text>,
        messaging_service_sid -> Nullable<Text>,
        subaccount_type -> Text,
        country_code -> Nullable<Text>,
    }
}

diesel::table! {
    tasks (id) {
        id -> Nullable<Integer>,
        user_id -> Integer,
        trigger -> Text,
        condition -> Nullable<Text>,
        action -> Text,
        notification_type -> Nullable<Text>,
        status -> Nullable<Text>,
        created_at -> Integer,
        completed_at -> Nullable<Integer>,
        is_permanent -> Nullable<Integer>,
        recurrence_rule -> Nullable<Text>,
        recurrence_time -> Nullable<Text>,
        sources -> Nullable<Text>,
    }
}

diesel::table! {
    tesla (id) {
        id -> Nullable<Integer>,
        user_id -> Integer,
        encrypted_access_token -> Text,
        encrypted_refresh_token -> Text,
        status -> Text,
        last_update -> Integer,
        created_on -> Integer,
        expires_in -> Integer,
        region -> Text,
        selected_vehicle_vin -> Nullable<Text>,
        selected_vehicle_name -> Nullable<Text>,
        selected_vehicle_id -> Nullable<Text>,
        virtual_key_paired -> Integer,
        granted_scopes -> Nullable<Text>,
    }
}

diesel::table! {
    totp_backup_codes (id) {
        id -> Nullable<Integer>,
        user_id -> Integer,
        code_hash -> Text,
        used -> Integer,
    }
}

diesel::table! {
    totp_secrets (id) {
        id -> Nullable<Integer>,
        user_id -> Integer,
        encrypted_secret -> Text,
        enabled -> Integer,
        created_at -> Integer,
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
        notify -> Bool,
        notification_type -> Nullable<Text>,
        timezone_auto -> Nullable<Bool>,
        agent_language -> Text,
        sub_country -> Nullable<Text>,
        save_context -> Nullable<Integer>,
        morning_digest -> Nullable<Text>,
        day_digest -> Nullable<Text>,
        evening_digest -> Nullable<Text>,
        number_of_digests_locked -> Integer,
        critical_enabled -> Nullable<Text>,
        encrypted_twilio_account_sid -> Nullable<Text>,
        encrypted_twilio_auth_token -> Nullable<Text>,
        encrypted_openrouter_api_key -> Nullable<Text>,
        server_url -> Nullable<Text>,
        encrypted_geoapify_key -> Nullable<Text>,
        encrypted_pirate_weather_key -> Nullable<Text>,
        server_ip -> Nullable<Text>,
        encrypted_textbee_device_id -> Nullable<Text>,
        encrypted_textbee_api_key -> Nullable<Text>,
        elevenlabs_phone_number_id -> Nullable<Text>,
        proactive_agent_on -> Bool,
        notify_about_calls -> Bool,
        action_on_critical_message -> Nullable<Text>,
        magic_login_token -> Nullable<Text>,
        magic_login_token_expiration_timestamp -> Nullable<Integer>,
        monthly_message_count -> Integer,
        outbound_message_pricing -> Nullable<Float>,
        last_instant_digest_time -> Nullable<Integer>,
        phone_service_active -> Bool,
        default_notification_mode -> Nullable<Text>,
        default_notification_type -> Nullable<Text>,
        default_notify_on_call -> Integer,
        llm_provider -> Nullable<Text>,
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
        matrix_device_id -> Nullable<Text>,
        credits_left -> Float,
        encrypted_matrix_password -> Nullable<Text>,
        encrypted_matrix_secret_storage_recovery_key -> Nullable<Text>,
        last_credits_notification -> Nullable<Integer>,
        discount -> Bool,
        discount_tier -> Nullable<Text>,
        free_reply -> Bool,
        confirm_send_event -> Nullable<Text>,
        waiting_checks_count -> Integer,
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

diesel::table! {
    webauthn_challenges (id) {
        id -> Nullable<Integer>,
        user_id -> Integer,
        challenge -> Text,
        challenge_type -> Text,
        context -> Nullable<Text>,
        created_at -> Integer,
        expires_at -> Integer,
    }
}

diesel::table! {
    webauthn_credentials (id) {
        id -> Nullable<Integer>,
        user_id -> Integer,
        credential_id -> Text,
        encrypted_public_key -> Text,
        device_name -> Text,
        counter -> Integer,
        transports -> Nullable<Text>,
        aaguid -> Nullable<Text>,
        created_at -> Integer,
        last_used_at -> Nullable<Integer>,
        enabled -> Integer,
    }
}

diesel::table! {
    youtube (id) {
        id -> Nullable<Integer>,
        user_id -> Integer,
        encrypted_access_token -> Text,
        encrypted_refresh_token -> Text,
        status -> Text,
        expires_in -> Integer,
        last_update -> Integer,
        created_on -> Integer,
        description -> Text,
    }
}

diesel::joinable!(bridge_disconnection_events -> users (user_id));
diesel::joinable!(bridges -> users (user_id));
diesel::joinable!(calendar_notifications -> users (user_id));
diesel::joinable!(contact_profile_exceptions -> contact_profiles (profile_id));
diesel::joinable!(contact_profiles -> users (user_id));
diesel::joinable!(conversations -> users (user_id));
diesel::joinable!(digests -> users (user_id));
diesel::joinable!(imap_connection -> users (user_id));
diesel::joinable!(keywords -> users (user_id));
diesel::joinable!(message_history -> users (user_id));
diesel::joinable!(priority_senders -> users (user_id));
diesel::joinable!(processed_emails -> users (user_id));
diesel::joinable!(refund_info -> users (user_id));
diesel::joinable!(tasks -> users (user_id));
diesel::joinable!(tesla -> users (user_id));
diesel::joinable!(totp_backup_codes -> users (user_id));
diesel::joinable!(totp_secrets -> users (user_id));
diesel::joinable!(user_info -> users (user_id));
diesel::joinable!(user_settings -> users (user_id));
diesel::joinable!(webauthn_challenges -> users (user_id));
diesel::joinable!(webauthn_credentials -> users (user_id));

diesel::allow_tables_to_appear_in_same_query!(
    admin_alerts,
    bridge_disconnection_events,
    bridges,
    calendar_notifications,
    contact_profile_exceptions,
    contact_profiles,
    conversations,
    country_availability,
    critical_categories,
    digests,
    disabled_alert_types,
    email_judgments,
    google_calendar,
    imap_connection,
    keywords,
    message_history,
    message_status_log,
    priority_senders,
    processed_emails,
    refund_info,
    subaccounts,
    tasks,
    tesla,
    totp_backup_codes,
    totp_secrets,
    uber,
    usage_logs,
    user_info,
    user_settings,
    users,
    waitlist,
    webauthn_challenges,
    webauthn_credentials,
    youtube,
);
