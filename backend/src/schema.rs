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
        whatsapp_room_id -> Nullable<Text>,
        telegram_room_id -> Nullable<Text>,
        signal_room_id -> Nullable<Text>,
        notes -> Nullable<Text>,
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
    imap_connection (id) {
        id -> Nullable<Integer>,
        user_id -> Integer,
        method -> Text,
        encrypted_password -> Text,
        status -> Text,
        last_update -> Integer,
        created_on -> Integer,
        description -> Text,
        imap_server -> Nullable<Text>,
        imap_port -> Nullable<Integer>,
    }
}

diesel::table! {
    items (id) {
        id -> Nullable<Integer>,
        user_id -> Integer,
        summary -> Text,
        due_at -> Nullable<Integer>,
        priority -> Integer,
        source_id -> Nullable<Text>,
        created_at -> Integer,
    }
}

diesel::table! {
    mcp_servers (id) {
        id -> Nullable<Integer>,
        user_id -> Integer,
        name -> Text,
        url_encrypted -> Text,
        auth_token_encrypted -> Nullable<Text>,
        is_enabled -> Integer,
        created_at -> Integer,
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
    site_metrics (id) {
        id -> Nullable<Integer>,
        metric_key -> Text,
        metric_value -> Text,
        updated_at -> Integer,
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
        info -> Nullable<Text>,
        timezone -> Nullable<Text>,
        nearby_places -> Nullable<Text>,
        latitude -> Nullable<Float>,
        longitude -> Nullable<Float>,
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
        encrypted_twilio_account_sid -> Nullable<Text>,
        encrypted_twilio_auth_token -> Nullable<Text>,
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
        matrix_username -> Nullable<Text>,
        encrypted_matrix_access_token -> Nullable<Text>,
        sub_tier -> Nullable<Text>,
        matrix_device_id -> Nullable<Text>,
        credits_left -> Float,
        encrypted_matrix_password -> Nullable<Text>,
        encrypted_matrix_secret_storage_recovery_key -> Nullable<Text>,
        last_credits_notification -> Nullable<Integer>,
        next_billing_date_timestamp -> Nullable<Integer>,
        magic_token -> Nullable<Text>,
        plan_type -> Nullable<Text>,
        matrix_e2ee_enabled -> Bool,
        migrated_to_new_server -> Bool,
        last_backup_at -> Nullable<Integer>,
        backup_session_active -> Bool,
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
diesel::joinable!(contact_profile_exceptions -> contact_profiles (profile_id));
diesel::joinable!(contact_profiles -> users (user_id));
diesel::joinable!(imap_connection -> users (user_id));
diesel::joinable!(items -> users (user_id));
diesel::joinable!(mcp_servers -> users (user_id));
diesel::joinable!(message_history -> users (user_id));
diesel::joinable!(processed_emails -> users (user_id));
diesel::joinable!(refund_info -> users (user_id));
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
    contact_profile_exceptions,
    contact_profiles,
    country_availability,
    disabled_alert_types,
    imap_connection,
    items,
    mcp_servers,
    message_history,
    message_status_log,
    processed_emails,
    refund_info,
    site_metrics,
    tesla,
    totp_backup_codes,
    totp_secrets,
    usage_logs,
    user_info,
    user_settings,
    users,
    waitlist,
    webauthn_challenges,
    webauthn_credentials,
    youtube,
);
