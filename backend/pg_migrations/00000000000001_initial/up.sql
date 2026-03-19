-- PostgreSQL tables for sensitive/user-content data (Nitro Enclave)

CREATE TABLE user_secrets (
    id SERIAL PRIMARY KEY,
    user_id INTEGER NOT NULL UNIQUE,
    matrix_username TEXT,
    matrix_device_id TEXT,
    encrypted_matrix_access_token TEXT,
    encrypted_matrix_password TEXT,
    encrypted_matrix_secret_storage_recovery_key TEXT,
    encrypted_twilio_account_sid TEXT,
    encrypted_twilio_auth_token TEXT
);

CREATE TABLE user_info (
    id SERIAL PRIMARY KEY,
    user_id INTEGER NOT NULL UNIQUE,
    location TEXT,
    info TEXT,
    timezone TEXT,
    nearby_places TEXT,
    latitude REAL,
    longitude REAL
);

CREATE TABLE contact_profiles (
    id SERIAL PRIMARY KEY,
    user_id INTEGER NOT NULL,
    nickname TEXT NOT NULL,
    whatsapp_chat TEXT,
    telegram_chat TEXT,
    signal_chat TEXT,
    email_addresses TEXT,
    notification_mode TEXT NOT NULL,
    notification_type TEXT NOT NULL,
    notify_on_call INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL,
    whatsapp_room_id TEXT,
    telegram_room_id TEXT,
    signal_room_id TEXT,
    notes TEXT
);

CREATE TABLE contact_profile_exceptions (
    id SERIAL PRIMARY KEY,
    profile_id INTEGER NOT NULL,
    platform TEXT NOT NULL,
    notification_mode TEXT NOT NULL,
    notification_type TEXT NOT NULL,
    notify_on_call INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE imap_connection (
    id SERIAL PRIMARY KEY,
    user_id INTEGER NOT NULL,
    method TEXT NOT NULL,
    encrypted_password TEXT NOT NULL,
    status TEXT NOT NULL,
    last_update INTEGER NOT NULL,
    created_on INTEGER NOT NULL,
    description TEXT NOT NULL,
    imap_server TEXT,
    imap_port INTEGER
);

CREATE TABLE message_history (
    id SERIAL PRIMARY KEY,
    user_id INTEGER NOT NULL,
    role TEXT NOT NULL,
    encrypted_content TEXT NOT NULL,
    tool_name TEXT,
    tool_call_id TEXT,
    created_at INTEGER NOT NULL,
    conversation_id TEXT NOT NULL,
    tool_calls_json TEXT
);

CREATE TABLE tesla (
    id SERIAL PRIMARY KEY,
    user_id INTEGER NOT NULL UNIQUE,
    encrypted_access_token TEXT NOT NULL,
    encrypted_refresh_token TEXT NOT NULL,
    status TEXT NOT NULL,
    last_update INTEGER NOT NULL,
    created_on INTEGER NOT NULL,
    expires_in INTEGER NOT NULL,
    region TEXT NOT NULL,
    selected_vehicle_vin TEXT,
    selected_vehicle_name TEXT,
    selected_vehicle_id TEXT,
    virtual_key_paired INTEGER NOT NULL DEFAULT 0,
    granted_scopes TEXT
);

CREATE TABLE youtube (
    id SERIAL PRIMARY KEY,
    user_id INTEGER NOT NULL UNIQUE,
    encrypted_access_token TEXT NOT NULL,
    encrypted_refresh_token TEXT NOT NULL,
    status TEXT NOT NULL,
    expires_in INTEGER NOT NULL,
    last_update INTEGER NOT NULL,
    created_on INTEGER NOT NULL,
    description TEXT NOT NULL
);

CREATE TABLE mcp_servers (
    id SERIAL PRIMARY KEY,
    user_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    url_encrypted TEXT NOT NULL,
    auth_token_encrypted TEXT,
    is_enabled INTEGER NOT NULL DEFAULT 1,
    created_at INTEGER NOT NULL
);

CREATE TABLE totp_secrets (
    id SERIAL PRIMARY KEY,
    user_id INTEGER NOT NULL UNIQUE,
    encrypted_secret TEXT NOT NULL,
    enabled INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL
);

CREATE TABLE totp_backup_codes (
    id SERIAL PRIMARY KEY,
    user_id INTEGER NOT NULL,
    code_hash TEXT NOT NULL,
    used INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE webauthn_credentials (
    id SERIAL PRIMARY KEY,
    user_id INTEGER NOT NULL,
    credential_id TEXT NOT NULL,
    encrypted_public_key TEXT NOT NULL,
    device_name TEXT NOT NULL,
    counter INTEGER NOT NULL DEFAULT 0,
    transports TEXT,
    aaguid TEXT,
    created_at INTEGER NOT NULL,
    last_used_at INTEGER,
    enabled INTEGER NOT NULL DEFAULT 1
);

CREATE TABLE webauthn_challenges (
    id SERIAL PRIMARY KEY,
    user_id INTEGER NOT NULL,
    challenge TEXT NOT NULL,
    challenge_type TEXT NOT NULL,
    context TEXT,
    created_at INTEGER NOT NULL,
    expires_at INTEGER NOT NULL
);

CREATE TABLE items (
    id SERIAL PRIMARY KEY,
    user_id INTEGER NOT NULL,
    summary TEXT NOT NULL,
    due_at INTEGER,
    priority INTEGER NOT NULL DEFAULT 0,
    source_id TEXT,
    created_at INTEGER NOT NULL
);

CREATE TABLE bridges (
    id SERIAL PRIMARY KEY,
    user_id INTEGER NOT NULL,
    bridge_type TEXT NOT NULL,
    status TEXT NOT NULL,
    room_id TEXT,
    data TEXT,
    created_at INTEGER,
    last_seen_online INTEGER
);

CREATE TABLE bridge_disconnection_events (
    id SERIAL PRIMARY KEY,
    user_id INTEGER NOT NULL,
    bridge_type TEXT NOT NULL,
    detected_at INTEGER NOT NULL
);

CREATE TABLE usage_logs (
    id SERIAL PRIMARY KEY,
    user_id INTEGER NOT NULL,
    sid TEXT,
    activity_type TEXT NOT NULL,
    credits REAL,
    created_at INTEGER NOT NULL,
    time_consumed INTEGER,
    success BOOLEAN,
    reason TEXT,
    status TEXT,
    recharge_threshold_timestamp INTEGER,
    zero_credits_timestamp INTEGER,
    call_duration INTEGER
);

CREATE TABLE processed_emails (
    id SERIAL PRIMARY KEY,
    user_id INTEGER NOT NULL,
    email_uid TEXT NOT NULL,
    processed_at INTEGER NOT NULL
);

-- Indexes for common queries
CREATE INDEX idx_user_secrets_user_id ON user_secrets(user_id);
CREATE INDEX idx_user_info_user_id ON user_info(user_id);
CREATE INDEX idx_contact_profiles_user_id ON contact_profiles(user_id);
CREATE INDEX idx_contact_profile_exceptions_profile_id ON contact_profile_exceptions(profile_id);
CREATE INDEX idx_imap_connection_user_id ON imap_connection(user_id);
CREATE INDEX idx_message_history_user_id ON message_history(user_id);
CREATE INDEX idx_message_history_conversation ON message_history(user_id, conversation_id);
CREATE INDEX idx_tesla_user_id ON tesla(user_id);
CREATE INDEX idx_youtube_user_id ON youtube(user_id);
CREATE INDEX idx_mcp_servers_user_id ON mcp_servers(user_id);
CREATE INDEX idx_totp_secrets_user_id ON totp_secrets(user_id);
CREATE INDEX idx_totp_backup_codes_user_id ON totp_backup_codes(user_id);
CREATE INDEX idx_webauthn_credentials_user_id ON webauthn_credentials(user_id);
CREATE INDEX idx_webauthn_challenges_user_id ON webauthn_challenges(user_id);
CREATE INDEX idx_items_user_id ON items(user_id);
CREATE INDEX idx_bridges_user_id ON bridges(user_id);
CREATE INDEX idx_bridge_disconnection_events_user_id ON bridge_disconnection_events(user_id);
CREATE INDEX idx_usage_logs_user_id ON usage_logs(user_id);
CREATE INDEX idx_processed_emails_user_id ON processed_emails(user_id);
