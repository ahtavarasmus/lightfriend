-- SQLite doesn't support DROP COLUMN directly, need to recreate the table
-- This is a simplified version that creates a new table without the column

CREATE TABLE user_settings_backup AS SELECT
    id, user_id, phone_number, preferred_name, system_prompt, use_history, timezone,
    morning_digest, day_digest, evening_digest, context_entries,
    stripe_customer_id, stripe_subscription_id, stripe_subscription_status,
    is_admin, referrer_code, referrer_id, referral_credits, charge_when_under,
    charge_back_to, credits, credits_left, plan_type, billing_region,
    is_self_hosted, server_ip, twilio_account_sid, twilio_auth_token,
    twilio_phone_number, openrouter_api_key, last_sms_time, email_notifications,
    auto_reply, auto_reply_context, last_morning_digest, last_day_digest,
    last_evening_digest, use_bridges, textbee_api_key, textbee_device_id,
    two_factor_secret, two_factor_enabled
FROM user_settings;

DROP TABLE user_settings;

ALTER TABLE user_settings_backup RENAME TO user_settings;
