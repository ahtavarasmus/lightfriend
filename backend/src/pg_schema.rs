// PostgreSQL schema - hand-written to match pg_migrations.
// Diesel CLI lacks PG backend support, so this must be maintained manually.

diesel::table! {
    user_secrets (id) {
        id -> Int4,
        user_id -> Int4,
        matrix_username -> Nullable<Text>,
        matrix_device_id -> Nullable<Text>,
        encrypted_matrix_access_token -> Nullable<Text>,
        encrypted_matrix_password -> Nullable<Text>,
        encrypted_matrix_secret_storage_recovery_key -> Nullable<Text>,
        encrypted_twilio_account_sid -> Nullable<Text>,
        encrypted_twilio_auth_token -> Nullable<Text>,
    }
}

diesel::table! {
    user_info (id) {
        id -> Int4,
        user_id -> Int4,
        location -> Nullable<Text>,
        info -> Nullable<Text>,
        timezone -> Nullable<Text>,
        nearby_places -> Nullable<Text>,
        latitude -> Nullable<Float4>,
        longitude -> Nullable<Float4>,
    }
}

diesel::table! {
    imap_connection (id) {
        id -> Int4,
        user_id -> Int4,
        method -> Text,
        encrypted_password -> Text,
        status -> Text,
        last_update -> Int4,
        created_on -> Int4,
        description -> Text,
        imap_server -> Nullable<Text>,
        imap_port -> Nullable<Int4>,
    }
}

diesel::table! {
    message_history (id) {
        id -> Int4,
        user_id -> Int4,
        role -> Text,
        encrypted_content -> Text,
        tool_name -> Nullable<Text>,
        tool_call_id -> Nullable<Text>,
        created_at -> Int4,
        conversation_id -> Text,
        tool_calls_json -> Nullable<Text>,
    }
}

diesel::table! {
    tesla (id) {
        id -> Int4,
        user_id -> Int4,
        encrypted_access_token -> Text,
        encrypted_refresh_token -> Text,
        status -> Text,
        last_update -> Int4,
        created_on -> Int4,
        expires_in -> Int4,
        region -> Text,
        selected_vehicle_vin -> Nullable<Text>,
        selected_vehicle_name -> Nullable<Text>,
        selected_vehicle_id -> Nullable<Text>,
        virtual_key_paired -> Int4,
        granted_scopes -> Nullable<Text>,
    }
}

diesel::table! {
    youtube (id) {
        id -> Int4,
        user_id -> Int4,
        encrypted_access_token -> Text,
        encrypted_refresh_token -> Text,
        status -> Text,
        expires_in -> Int4,
        last_update -> Int4,
        created_on -> Int4,
        description -> Text,
    }
}

diesel::table! {
    mcp_servers (id) {
        id -> Int4,
        user_id -> Int4,
        name -> Text,
        url_encrypted -> Text,
        auth_token_encrypted -> Nullable<Text>,
        is_enabled -> Int4,
        created_at -> Int4,
    }
}

diesel::table! {
    totp_secrets (id) {
        id -> Int4,
        user_id -> Int4,
        encrypted_secret -> Text,
        enabled -> Int4,
        created_at -> Int4,
    }
}

diesel::table! {
    totp_backup_codes (id) {
        id -> Int4,
        user_id -> Int4,
        code_hash -> Text,
        used -> Int4,
    }
}

diesel::table! {
    webauthn_credentials (id) {
        id -> Int4,
        user_id -> Int4,
        credential_id -> Text,
        encrypted_public_key -> Text,
        device_name -> Text,
        counter -> Int4,
        transports -> Nullable<Text>,
        aaguid -> Nullable<Text>,
        created_at -> Int4,
        last_used_at -> Nullable<Int4>,
        enabled -> Int4,
    }
}

diesel::table! {
    webauthn_challenges (id) {
        id -> Int4,
        user_id -> Int4,
        challenge -> Text,
        challenge_type -> Text,
        context -> Nullable<Text>,
        created_at -> Int4,
        expires_at -> Int4,
    }
}

diesel::table! {
    items (id) {
        id -> Int4,
        user_id -> Int4,
        summary -> Text,
        due_at -> Nullable<Int4>,
        priority -> Int4,
        source_id -> Nullable<Text>,
        created_at -> Int4,
    }
}

diesel::table! {
    bridges (id) {
        id -> Int4,
        user_id -> Int4,
        bridge_type -> Text,
        status -> Text,
        room_id -> Nullable<Text>,
        data -> Nullable<Text>,
        created_at -> Nullable<Int4>,
        last_seen_online -> Nullable<Int4>,
    }
}

diesel::table! {
    bridge_disconnection_events (id) {
        id -> Int4,
        user_id -> Int4,
        bridge_type -> Text,
        detected_at -> Int4,
    }
}

diesel::table! {
    usage_logs (id) {
        id -> Int4,
        user_id -> Int4,
        sid -> Nullable<Text>,
        activity_type -> Text,
        credits -> Nullable<Float4>,
        created_at -> Int4,
        time_consumed -> Nullable<Int4>,
        success -> Nullable<Bool>,
        reason -> Nullable<Text>,
        status -> Nullable<Text>,
        recharge_threshold_timestamp -> Nullable<Int4>,
        zero_credits_timestamp -> Nullable<Int4>,
        call_duration -> Nullable<Int4>,
    }
}

diesel::table! {
    processed_emails (id) {
        id -> Int4,
        user_id -> Int4,
        email_uid -> Text,
        processed_at -> Int4,
    }
}

// Tables from 00000000000002_remaining_sqlite_tables

diesel::table! {
    users (id) {
        id -> Int4,
        email -> Text,
        password_hash -> Text,
        phone_number -> Text,
        nickname -> Nullable<Text>,
        time_to_live -> Nullable<Int4>,
        credits -> Float4,
        preferred_number -> Nullable<Text>,
        charge_when_under -> Bool,
        charge_back_to -> Nullable<Float4>,
        stripe_customer_id -> Nullable<Text>,
        stripe_payment_method_id -> Nullable<Text>,
        stripe_checkout_session_id -> Nullable<Text>,
        sub_tier -> Nullable<Text>,
        credits_left -> Float4,
        last_credits_notification -> Nullable<Int4>,
        next_billing_date_timestamp -> Nullable<Int4>,
        magic_token -> Nullable<Text>,
        plan_type -> Nullable<Text>,
        matrix_e2ee_enabled -> Bool,
    }
}

diesel::table! {
    user_settings (id) {
        id -> Int4,
        user_id -> Int4,
        notify -> Bool,
        notification_type -> Nullable<Text>,
        timezone_auto -> Nullable<Bool>,
        agent_language -> Text,
        sub_country -> Nullable<Text>,
        save_context -> Nullable<Int4>,
        critical_enabled -> Nullable<Text>,
        elevenlabs_phone_number_id -> Nullable<Text>,
        notify_about_calls -> Bool,
        action_on_critical_message -> Nullable<Text>,
        phone_service_active -> Bool,
        default_notification_mode -> Nullable<Text>,
        default_notification_type -> Nullable<Text>,
        default_notify_on_call -> Int4,
        llm_provider -> Nullable<Text>,
        phone_contact_notification_mode -> Nullable<Text>,
        phone_contact_notification_type -> Nullable<Text>,
        phone_contact_notify_on_call -> Int4,
        auto_create_items -> Bool,
    }
}

diesel::table! {
    refund_info (id) {
        id -> Int4,
        user_id -> Int4,
        has_refunded -> Int4,
        last_credit_pack_amount -> Nullable<Float4>,
        last_credit_pack_purchase_timestamp -> Nullable<Int4>,
        refunded_at -> Nullable<Int4>,
    }
}

diesel::table! {
    country_availability (id) {
        id -> Int4,
        country_code -> Text,
        has_local_numbers -> Bool,
        outbound_sms_price -> Nullable<Float4>,
        inbound_sms_price -> Nullable<Float4>,
        outbound_voice_price_per_min -> Nullable<Float4>,
        inbound_voice_price_per_min -> Nullable<Float4>,
        last_checked -> Int4,
        created_at -> Int4,
    }
}

diesel::table! {
    message_status_log (id) {
        id -> Int4,
        message_sid -> Text,
        user_id -> Int4,
        direction -> Text,
        to_number -> Text,
        from_number -> Nullable<Text>,
        status -> Text,
        error_code -> Nullable<Text>,
        error_message -> Nullable<Text>,
        created_at -> Int4,
        updated_at -> Int4,
        price -> Nullable<Float4>,
        price_unit -> Nullable<Text>,
    }
}

diesel::table! {
    admin_alerts (id) {
        id -> Int4,
        alert_type -> Text,
        severity -> Text,
        message -> Text,
        location -> Text,
        module -> Text,
        acknowledged -> Int4,
        created_at -> Int4,
    }
}

diesel::table! {
    disabled_alert_types (id) {
        id -> Int4,
        alert_type -> Text,
        disabled_at -> Int4,
    }
}

diesel::table! {
    site_metrics (id) {
        id -> Int4,
        metric_key -> Text,
        metric_value -> Text,
        updated_at -> Int4,
    }
}

diesel::table! {
    waitlist (id) {
        id -> Int4,
        email -> Text,
        created_at -> Int4,
    }
}

// Ontology v1: Person + Channel tables

diesel::table! {
    ont_persons (id) {
        id -> Int4,
        user_id -> Int4,
        name -> Text,
        created_at -> Int4,
        updated_at -> Int4,
    }
}

diesel::table! {
    ont_person_edits (id) {
        id -> Int4,
        user_id -> Int4,
        person_id -> Int4,
        property_name -> Text,
        value -> Text,
        edited_at -> Int4,
    }
}

diesel::table! {
    ont_channels (id) {
        id -> Int4,
        user_id -> Int4,
        person_id -> Int4,
        platform -> Text,
        handle -> Nullable<Text>,
        room_id -> Nullable<Text>,
        notification_mode -> Text,
        notification_type -> Text,
        notify_on_call -> Int4,
        created_at -> Int4,
    }
}

diesel::table! {
    ont_changelog (id) {
        id -> Int8,
        user_id -> Int4,
        entity_type -> Text,
        entity_id -> Int4,
        change_type -> Text,
        changed_fields -> Nullable<Text>,
        source -> Text,
        created_at -> Int4,
    }
}

// Ontology v2: Links between entities

diesel::table! {
    ont_links (id) {
        id -> Int4,
        user_id -> Int4,
        source_type -> Text,
        source_id -> Int4,
        target_type -> Text,
        target_id -> Int4,
        link_type -> Text,
        metadata -> Nullable<Text>,
        created_at -> Int4,
    }
}

diesel::joinable!(ont_person_edits -> ont_persons (person_id));
diesel::joinable!(ont_channels -> ont_persons (person_id));

diesel::joinable!(refund_info -> users (user_id));
diesel::joinable!(user_settings -> users (user_id));

diesel::allow_tables_to_appear_in_same_query!(
    user_secrets,
    user_info,
    imap_connection,
    message_history,
    tesla,
    youtube,
    mcp_servers,
    totp_secrets,
    totp_backup_codes,
    webauthn_credentials,
    webauthn_challenges,
    items,
    bridges,
    bridge_disconnection_events,
    usage_logs,
    processed_emails,
    users,
    user_settings,
    refund_info,
    country_availability,
    message_status_log,
    admin_alerts,
    disabled_alert_types,
    site_metrics,
    waitlist,
    ont_persons,
    ont_person_edits,
    ont_channels,
    ont_changelog,
    ont_links,
);
