-- Initial PostgreSQL schema for Lightfriend
-- Converted from SQLite schema

-- Users table (core)
CREATE TABLE users (
    id SERIAL PRIMARY KEY,
    email TEXT NOT NULL,
    password_hash TEXT NOT NULL,
    phone_number TEXT NOT NULL,
    nickname TEXT,
    time_to_live INTEGER,
    verified BOOLEAN NOT NULL DEFAULT FALSE,
    credits REAL NOT NULL DEFAULT 0.0,
    preferred_number TEXT,
    charge_when_under BOOLEAN NOT NULL DEFAULT FALSE,
    charge_back_to REAL,
    stripe_customer_id TEXT,
    stripe_payment_method_id TEXT,
    stripe_checkout_session_id TEXT,
    matrix_username TEXT,
    encrypted_matrix_access_token TEXT,
    sub_tier TEXT,
    matrix_device_id TEXT,
    credits_left REAL NOT NULL DEFAULT 0.0,
    encrypted_matrix_password TEXT,
    encrypted_matrix_secret_storage_recovery_key TEXT,
    last_credits_notification INTEGER,
    discount BOOLEAN NOT NULL DEFAULT FALSE,
    discount_tier TEXT,
    free_reply BOOLEAN NOT NULL DEFAULT FALSE,
    confirm_send_event TEXT,
    waiting_checks_count INTEGER NOT NULL DEFAULT 0,
    next_billing_date_timestamp INTEGER,
    magic_token TEXT,
    plan_type TEXT,
    matrix_e2ee_enabled BOOLEAN NOT NULL DEFAULT FALSE,
    migrated_to_new_server BOOLEAN NOT NULL DEFAULT FALSE,
    active_enclave TEXT
);

-- Admin alerts
CREATE TABLE admin_alerts (
    id SERIAL PRIMARY KEY,
    alert_type TEXT NOT NULL,
    severity TEXT NOT NULL,
    message TEXT NOT NULL,
    location TEXT NOT NULL,
    module TEXT NOT NULL,
    acknowledged INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL DEFAULT EXTRACT(EPOCH FROM NOW())::INTEGER
);

-- Bridge disconnection events
CREATE TABLE bridge_disconnection_events (
    id SERIAL PRIMARY KEY,
    user_id INTEGER NOT NULL REFERENCES users(id),
    bridge_type TEXT NOT NULL,
    detected_at INTEGER NOT NULL DEFAULT EXTRACT(EPOCH FROM NOW())::INTEGER
);

-- Bridges
CREATE TABLE bridges (
    id SERIAL PRIMARY KEY,
    user_id INTEGER NOT NULL REFERENCES users(id),
    bridge_type TEXT NOT NULL,
    status TEXT NOT NULL,
    room_id TEXT,
    data TEXT,
    created_at INTEGER DEFAULT EXTRACT(EPOCH FROM NOW())::INTEGER,
    last_seen_online INTEGER
);

-- Calendar notifications
CREATE TABLE calendar_notifications (
    id SERIAL PRIMARY KEY,
    user_id INTEGER NOT NULL REFERENCES users(id),
    event_id TEXT NOT NULL,
    notification_time INTEGER NOT NULL
);

-- Contact profiles
CREATE TABLE contact_profiles (
    id SERIAL PRIMARY KEY,
    user_id INTEGER NOT NULL REFERENCES users(id),
    nickname TEXT NOT NULL,
    whatsapp_chat TEXT,
    telegram_chat TEXT,
    signal_chat TEXT,
    email_addresses TEXT,
    notification_mode TEXT NOT NULL,
    notification_type TEXT NOT NULL,
    notify_on_call INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL DEFAULT EXTRACT(EPOCH FROM NOW())::INTEGER
);

-- Contact profile exceptions
CREATE TABLE contact_profile_exceptions (
    id SERIAL PRIMARY KEY,
    profile_id INTEGER NOT NULL REFERENCES contact_profiles(id),
    platform TEXT NOT NULL,
    notification_mode TEXT NOT NULL,
    notification_type TEXT NOT NULL,
    notify_on_call INTEGER NOT NULL DEFAULT 0
);

-- Conversations
CREATE TABLE conversations (
    id SERIAL PRIMARY KEY,
    user_id INTEGER NOT NULL REFERENCES users(id),
    conversation_sid TEXT NOT NULL,
    service_sid TEXT NOT NULL,
    created_at INTEGER NOT NULL DEFAULT EXTRACT(EPOCH FROM NOW())::INTEGER,
    active BOOLEAN NOT NULL DEFAULT TRUE,
    twilio_number TEXT NOT NULL,
    user_number TEXT NOT NULL
);

-- Country availability
CREATE TABLE country_availability (
    id SERIAL PRIMARY KEY,
    country_code TEXT NOT NULL,
    has_local_numbers BOOLEAN NOT NULL DEFAULT FALSE,
    outbound_sms_price REAL,
    inbound_sms_price REAL,
    outbound_voice_price_per_min REAL,
    inbound_voice_price_per_min REAL,
    last_checked INTEGER NOT NULL DEFAULT EXTRACT(EPOCH FROM NOW())::INTEGER,
    created_at INTEGER NOT NULL DEFAULT EXTRACT(EPOCH FROM NOW())::INTEGER
);

-- Critical categories
CREATE TABLE critical_categories (
    id SERIAL PRIMARY KEY,
    user_id INTEGER NOT NULL,
    category_name TEXT NOT NULL,
    definition TEXT,
    active BOOLEAN NOT NULL DEFAULT TRUE
);

-- Digests
CREATE TABLE digests (
    id SERIAL PRIMARY KEY,
    user_id INTEGER NOT NULL REFERENCES users(id),
    time TEXT NOT NULL,
    tools TEXT NOT NULL,
    tool_params TEXT,
    enabled INTEGER NOT NULL DEFAULT 1,
    last_sent_at INTEGER,
    created_at INTEGER NOT NULL DEFAULT EXTRACT(EPOCH FROM NOW())::INTEGER
);

-- Disabled alert types
CREATE TABLE disabled_alert_types (
    id SERIAL PRIMARY KEY,
    alert_type TEXT NOT NULL UNIQUE,
    disabled_at INTEGER NOT NULL DEFAULT EXTRACT(EPOCH FROM NOW())::INTEGER
);

-- Email judgments
CREATE TABLE email_judgments (
    id SERIAL PRIMARY KEY,
    user_id INTEGER NOT NULL,
    email_timestamp INTEGER NOT NULL,
    processed_at INTEGER NOT NULL DEFAULT EXTRACT(EPOCH FROM NOW())::INTEGER,
    should_notify BOOLEAN NOT NULL DEFAULT FALSE,
    score INTEGER NOT NULL DEFAULT 0,
    reason TEXT NOT NULL
);

-- Google Calendar
CREATE TABLE google_calendar (
    id SERIAL PRIMARY KEY,
    user_id INTEGER NOT NULL,
    encrypted_access_token TEXT NOT NULL,
    encrypted_refresh_token TEXT NOT NULL,
    status TEXT NOT NULL,
    last_update INTEGER NOT NULL DEFAULT EXTRACT(EPOCH FROM NOW())::INTEGER,
    created_on INTEGER NOT NULL DEFAULT EXTRACT(EPOCH FROM NOW())::INTEGER,
    description TEXT NOT NULL DEFAULT '',
    expires_in INTEGER NOT NULL DEFAULT 0
);

-- IMAP connection
CREATE TABLE imap_connection (
    id SERIAL PRIMARY KEY,
    user_id INTEGER NOT NULL REFERENCES users(id),
    method TEXT NOT NULL,
    encrypted_password TEXT NOT NULL,
    status TEXT NOT NULL,
    last_update INTEGER NOT NULL DEFAULT EXTRACT(EPOCH FROM NOW())::INTEGER,
    created_on INTEGER NOT NULL DEFAULT EXTRACT(EPOCH FROM NOW())::INTEGER,
    description TEXT NOT NULL DEFAULT '',
    expires_in INTEGER NOT NULL DEFAULT 0,
    imap_server TEXT,
    imap_port INTEGER
);

-- Keywords
CREATE TABLE keywords (
    id SERIAL PRIMARY KEY,
    user_id INTEGER NOT NULL REFERENCES users(id),
    keyword TEXT NOT NULL,
    service_type TEXT NOT NULL
);

-- Message history
CREATE TABLE message_history (
    id SERIAL PRIMARY KEY,
    user_id INTEGER NOT NULL REFERENCES users(id),
    role TEXT NOT NULL,
    encrypted_content TEXT NOT NULL,
    tool_name TEXT,
    tool_call_id TEXT,
    created_at INTEGER NOT NULL DEFAULT EXTRACT(EPOCH FROM NOW())::INTEGER,
    conversation_id TEXT NOT NULL,
    tool_calls_json TEXT
);

-- Message status log
CREATE TABLE message_status_log (
    id SERIAL PRIMARY KEY,
    message_sid TEXT NOT NULL,
    user_id INTEGER NOT NULL,
    direction TEXT NOT NULL,
    to_number TEXT NOT NULL,
    from_number TEXT,
    status TEXT NOT NULL,
    error_code TEXT,
    error_message TEXT,
    created_at INTEGER NOT NULL DEFAULT EXTRACT(EPOCH FROM NOW())::INTEGER,
    updated_at INTEGER NOT NULL DEFAULT EXTRACT(EPOCH FROM NOW())::INTEGER,
    price REAL,
    price_unit TEXT
);

-- Priority senders
CREATE TABLE priority_senders (
    id SERIAL PRIMARY KEY,
    user_id INTEGER NOT NULL REFERENCES users(id),
    sender TEXT NOT NULL,
    service_type TEXT NOT NULL,
    noti_type TEXT,
    noti_mode TEXT NOT NULL
);

-- Processed emails
CREATE TABLE processed_emails (
    id SERIAL PRIMARY KEY,
    user_id INTEGER NOT NULL REFERENCES users(id),
    email_uid TEXT NOT NULL,
    processed_at INTEGER NOT NULL DEFAULT EXTRACT(EPOCH FROM NOW())::INTEGER
);

-- Refund info
CREATE TABLE refund_info (
    id SERIAL PRIMARY KEY,
    user_id INTEGER NOT NULL REFERENCES users(id),
    has_refunded INTEGER NOT NULL DEFAULT 0,
    last_credit_pack_amount REAL,
    last_credit_pack_purchase_timestamp INTEGER,
    refunded_at INTEGER
);

-- Site metrics
CREATE TABLE site_metrics (
    id SERIAL PRIMARY KEY,
    metric_key TEXT NOT NULL,
    metric_value TEXT NOT NULL,
    updated_at INTEGER NOT NULL DEFAULT EXTRACT(EPOCH FROM NOW())::INTEGER
);

-- Subaccounts
CREATE TABLE subaccounts (
    id SERIAL PRIMARY KEY,
    user_id TEXT NOT NULL,
    subaccount_sid TEXT NOT NULL,
    auth_token TEXT NOT NULL,
    country TEXT,
    number TEXT,
    cost_this_month REAL,
    created_at INTEGER DEFAULT EXTRACT(EPOCH FROM NOW())::INTEGER,
    status TEXT,
    tinfoil_key TEXT,
    messaging_service_sid TEXT,
    subaccount_type TEXT NOT NULL,
    country_code TEXT
);

-- Tasks
CREATE TABLE tasks (
    id SERIAL PRIMARY KEY,
    user_id INTEGER NOT NULL REFERENCES users(id),
    trigger TEXT NOT NULL,
    condition TEXT,
    action TEXT NOT NULL,
    notification_type TEXT,
    status TEXT,
    created_at INTEGER NOT NULL DEFAULT EXTRACT(EPOCH FROM NOW())::INTEGER,
    completed_at INTEGER,
    is_permanent INTEGER,
    recurrence_rule TEXT,
    recurrence_time TEXT,
    sources TEXT
);

-- Tesla
CREATE TABLE tesla (
    id SERIAL PRIMARY KEY,
    user_id INTEGER NOT NULL REFERENCES users(id),
    encrypted_access_token TEXT NOT NULL,
    encrypted_refresh_token TEXT NOT NULL,
    status TEXT NOT NULL,
    last_update INTEGER NOT NULL DEFAULT EXTRACT(EPOCH FROM NOW())::INTEGER,
    created_on INTEGER NOT NULL DEFAULT EXTRACT(EPOCH FROM NOW())::INTEGER,
    expires_in INTEGER NOT NULL DEFAULT 0,
    region TEXT NOT NULL,
    selected_vehicle_vin TEXT,
    selected_vehicle_name TEXT,
    selected_vehicle_id TEXT,
    virtual_key_paired INTEGER NOT NULL DEFAULT 0,
    granted_scopes TEXT
);

-- TOTP backup codes
CREATE TABLE totp_backup_codes (
    id SERIAL PRIMARY KEY,
    user_id INTEGER NOT NULL REFERENCES users(id),
    code_hash TEXT NOT NULL,
    used INTEGER NOT NULL DEFAULT 0
);

-- TOTP secrets
CREATE TABLE totp_secrets (
    id SERIAL PRIMARY KEY,
    user_id INTEGER NOT NULL REFERENCES users(id),
    encrypted_secret TEXT NOT NULL,
    enabled INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL DEFAULT EXTRACT(EPOCH FROM NOW())::INTEGER
);

-- Uber
CREATE TABLE uber (
    id SERIAL PRIMARY KEY,
    user_id INTEGER NOT NULL,
    encrypted_access_token TEXT NOT NULL,
    encrypted_refresh_token TEXT NOT NULL,
    status TEXT NOT NULL,
    last_update INTEGER NOT NULL DEFAULT EXTRACT(EPOCH FROM NOW())::INTEGER,
    created_on INTEGER NOT NULL DEFAULT EXTRACT(EPOCH FROM NOW())::INTEGER,
    description TEXT NOT NULL DEFAULT '',
    expires_in INTEGER NOT NULL DEFAULT 0
);

-- Usage logs
CREATE TABLE usage_logs (
    id SERIAL PRIMARY KEY,
    user_id INTEGER NOT NULL,
    sid TEXT,
    activity_type TEXT NOT NULL,
    credits REAL,
    created_at INTEGER NOT NULL DEFAULT EXTRACT(EPOCH FROM NOW())::INTEGER,
    time_consumed INTEGER,
    success BOOLEAN,
    reason TEXT,
    status TEXT,
    recharge_threshold_timestamp INTEGER,
    zero_credits_timestamp INTEGER,
    call_duration INTEGER
);

-- User info
CREATE TABLE user_info (
    id SERIAL PRIMARY KEY,
    user_id INTEGER NOT NULL REFERENCES users(id),
    location TEXT,
    dictionary TEXT,
    info TEXT,
    timezone TEXT,
    nearby_places TEXT,
    recent_contacts TEXT,
    blocker_password_vault TEXT,
    lockbox_password_vault TEXT
);

-- User settings
CREATE TABLE user_settings (
    id SERIAL PRIMARY KEY,
    user_id INTEGER NOT NULL REFERENCES users(id),
    notify BOOLEAN NOT NULL DEFAULT TRUE,
    notification_type TEXT,
    timezone_auto BOOLEAN,
    agent_language TEXT NOT NULL DEFAULT 'en',
    sub_country TEXT,
    save_context INTEGER,
    morning_digest TEXT,
    day_digest TEXT,
    evening_digest TEXT,
    number_of_digests_locked INTEGER NOT NULL DEFAULT 0,
    critical_enabled TEXT,
    encrypted_twilio_account_sid TEXT,
    encrypted_twilio_auth_token TEXT,
    encrypted_openrouter_api_key TEXT,
    server_url TEXT,
    encrypted_geoapify_key TEXT,
    encrypted_pirate_weather_key TEXT,
    server_ip TEXT,
    encrypted_textbee_device_id TEXT,
    encrypted_textbee_api_key TEXT,
    elevenlabs_phone_number_id TEXT,
    proactive_agent_on BOOLEAN NOT NULL DEFAULT FALSE,
    notify_about_calls BOOLEAN NOT NULL DEFAULT FALSE,
    action_on_critical_message TEXT,
    magic_login_token TEXT,
    magic_login_token_expiration_timestamp INTEGER,
    monthly_message_count INTEGER NOT NULL DEFAULT 0,
    outbound_message_pricing REAL,
    last_instant_digest_time INTEGER,
    phone_service_active BOOLEAN NOT NULL DEFAULT FALSE,
    default_notification_mode TEXT,
    default_notification_type TEXT,
    default_notify_on_call INTEGER NOT NULL DEFAULT 0,
    llm_provider TEXT
);

-- Waitlist
CREATE TABLE waitlist (
    id SERIAL PRIMARY KEY,
    email TEXT NOT NULL,
    created_at INTEGER NOT NULL DEFAULT EXTRACT(EPOCH FROM NOW())::INTEGER
);

-- WebAuthn challenges
CREATE TABLE webauthn_challenges (
    id SERIAL PRIMARY KEY,
    user_id INTEGER NOT NULL REFERENCES users(id),
    challenge TEXT NOT NULL,
    challenge_type TEXT NOT NULL,
    context TEXT,
    created_at INTEGER NOT NULL DEFAULT EXTRACT(EPOCH FROM NOW())::INTEGER,
    expires_at INTEGER NOT NULL
);

-- WebAuthn credentials
CREATE TABLE webauthn_credentials (
    id SERIAL PRIMARY KEY,
    user_id INTEGER NOT NULL REFERENCES users(id),
    credential_id TEXT NOT NULL,
    encrypted_public_key TEXT NOT NULL,
    device_name TEXT NOT NULL,
    counter INTEGER NOT NULL DEFAULT 0,
    transports TEXT,
    aaguid TEXT,
    created_at INTEGER NOT NULL DEFAULT EXTRACT(EPOCH FROM NOW())::INTEGER,
    last_used_at INTEGER,
    enabled INTEGER NOT NULL DEFAULT 1
);

-- YouTube
CREATE TABLE youtube (
    id SERIAL PRIMARY KEY,
    user_id INTEGER NOT NULL,
    encrypted_access_token TEXT NOT NULL,
    encrypted_refresh_token TEXT NOT NULL,
    status TEXT NOT NULL,
    expires_in INTEGER NOT NULL DEFAULT 0,
    last_update INTEGER NOT NULL DEFAULT EXTRACT(EPOCH FROM NOW())::INTEGER,
    created_on INTEGER NOT NULL DEFAULT EXTRACT(EPOCH FROM NOW())::INTEGER,
    description TEXT NOT NULL DEFAULT ''
);

-- Create indexes for common queries
CREATE INDEX idx_users_email ON users(email);
CREATE INDEX idx_users_phone_number ON users(phone_number);
CREATE INDEX idx_bridges_user_id ON bridges(user_id);
CREATE INDEX idx_message_history_user_id ON message_history(user_id);
CREATE INDEX idx_message_history_conversation_id ON message_history(conversation_id);
CREATE INDEX idx_usage_logs_user_id ON usage_logs(user_id);
CREATE INDEX idx_tasks_user_id ON tasks(user_id);
CREATE INDEX idx_message_status_log_message_sid ON message_status_log(message_sid);
CREATE INDEX idx_message_status_log_user_id ON message_status_log(user_id);
