-- Remaining SQLite tables migrated to PostgreSQL
-- Using IF NOT EXISTS for tables that may have been created by migrate_to_pg

CREATE TABLE IF NOT EXISTS users (
    id SERIAL PRIMARY KEY,
    email TEXT NOT NULL,
    password_hash TEXT NOT NULL,
    phone_number TEXT NOT NULL,
    nickname TEXT,
    time_to_live INTEGER,
    credits REAL NOT NULL,
    preferred_number TEXT,
    charge_when_under BOOLEAN NOT NULL,
    charge_back_to REAL,
    stripe_customer_id TEXT,
    stripe_payment_method_id TEXT,
    stripe_checkout_session_id TEXT,
    sub_tier TEXT,
    credits_left REAL NOT NULL,
    last_credits_notification INTEGER,
    next_billing_date_timestamp INTEGER,
    magic_token TEXT,
    plan_type TEXT,
    matrix_e2ee_enabled BOOLEAN NOT NULL
);

CREATE TABLE IF NOT EXISTS user_settings (
    id SERIAL PRIMARY KEY,
    user_id INTEGER NOT NULL UNIQUE,
    notify BOOLEAN NOT NULL,
    notification_type TEXT,
    timezone_auto BOOLEAN,
    agent_language TEXT NOT NULL,
    sub_country TEXT,
    save_context INTEGER,
    critical_enabled TEXT,
    elevenlabs_phone_number_id TEXT,
    notify_about_calls BOOLEAN NOT NULL,
    action_on_critical_message TEXT,
    phone_service_active BOOLEAN NOT NULL,
    default_notification_mode TEXT,
    default_notification_type TEXT,
    default_notify_on_call INTEGER NOT NULL,
    llm_provider TEXT,
    phone_contact_notification_mode TEXT,
    phone_contact_notification_type TEXT,
    phone_contact_notify_on_call INTEGER NOT NULL,
    auto_create_items BOOLEAN NOT NULL
);

CREATE TABLE IF NOT EXISTS refund_info (
    id SERIAL PRIMARY KEY,
    user_id INTEGER NOT NULL UNIQUE,
    has_refunded INTEGER NOT NULL,
    last_credit_pack_amount REAL,
    last_credit_pack_purchase_timestamp INTEGER,
    refunded_at INTEGER
);

CREATE TABLE IF NOT EXISTS country_availability (
    id SERIAL PRIMARY KEY,
    country_code TEXT NOT NULL UNIQUE,
    has_local_numbers BOOLEAN NOT NULL,
    outbound_sms_price REAL,
    inbound_sms_price REAL,
    outbound_voice_price_per_min REAL,
    inbound_voice_price_per_min REAL,
    last_checked INTEGER NOT NULL,
    created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS message_status_log (
    id SERIAL PRIMARY KEY,
    message_sid TEXT NOT NULL UNIQUE,
    user_id INTEGER NOT NULL,
    direction TEXT NOT NULL,
    to_number TEXT NOT NULL,
    from_number TEXT,
    status TEXT NOT NULL,
    error_code TEXT,
    error_message TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    price REAL,
    price_unit TEXT
);

CREATE TABLE IF NOT EXISTS admin_alerts (
    id SERIAL PRIMARY KEY,
    alert_type TEXT NOT NULL,
    severity TEXT NOT NULL,
    message TEXT NOT NULL,
    location TEXT NOT NULL,
    module TEXT NOT NULL,
    acknowledged INTEGER NOT NULL,
    created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS disabled_alert_types (
    id SERIAL PRIMARY KEY,
    alert_type TEXT NOT NULL UNIQUE,
    disabled_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS site_metrics (
    id SERIAL PRIMARY KEY,
    metric_key TEXT NOT NULL UNIQUE,
    metric_value TEXT NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS waitlist (
    id SERIAL PRIMARY KEY,
    email TEXT NOT NULL UNIQUE,
    created_at INTEGER NOT NULL
);

-- Indexes for user_id columns (IF NOT EXISTS requires PG 9.5+)
CREATE INDEX IF NOT EXISTS idx_user_settings_user_id ON user_settings(user_id);
CREATE INDEX IF NOT EXISTS idx_refund_info_user_id ON refund_info(user_id);
CREATE INDEX IF NOT EXISTS idx_message_status_log_user_id ON message_status_log(user_id);
