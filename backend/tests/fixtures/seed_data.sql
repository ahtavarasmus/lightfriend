-- Representative test data for migration safety testing.
-- Inserted after initial PG migrations to simulate a production-like database.
-- Tests that new migrations apply cleanly against existing data.

-- User secrets (simulates encrypted credential storage)
INSERT INTO user_secrets (user_id, matrix_username, matrix_device_id, encrypted_matrix_access_token, encrypted_matrix_password)
VALUES
    (1, '@alice:localhost', 'DEVICEABC', 'enc_token_alice', 'enc_pass_alice'),
    (2, '@bob:localhost', 'DEVICEDEF', 'enc_token_bob', 'enc_pass_bob'),
    (3, NULL, NULL, NULL, NULL);

-- User info (location, timezone)
INSERT INTO user_info (user_id, location, info, timezone, latitude, longitude)
VALUES
    (1, 'Helsinki, Finland', 'Test user Alice', 'Europe/Helsinki', 60.1699, 24.9384),
    (2, 'New York, USA', 'Test user Bob', 'America/New_York', 40.7128, -74.0060);

-- Contact profiles
INSERT INTO contact_profiles (user_id, nickname, whatsapp_chat, notification_mode, notification_type, created_at)
VALUES
    (1, 'Mom', 'mom_wa_chat_id', 'always', 'sms', 1710000000),
    (1, 'Boss', NULL, 'urgent_only', 'call', 1710000001),
    (2, 'Partner', 'partner_wa', 'always', 'sms', 1710000002);

-- Contact profile exceptions
INSERT INTO contact_profile_exceptions (profile_id, platform, notification_mode, notification_type)
VALUES
    (1, 'whatsapp', 'urgent_only', 'call');

-- Bridges
INSERT INTO bridges (user_id, bridge_type, status, room_id, created_at)
VALUES
    (1, 'whatsapp', 'connected', '!room1:localhost', 1710000000),
    (1, 'signal', 'disconnected', '!room2:localhost', 1710000001),
    (2, 'telegram', 'connected', '!room3:localhost', 1710000002);

-- Bridge disconnection events
INSERT INTO bridge_disconnection_events (user_id, bridge_type, detected_at)
VALUES
    (1, 'signal', 1710050000);

-- Message history
INSERT INTO message_history (user_id, role, encrypted_content, conversation_id, created_at)
VALUES
    (1, 'user', 'enc_msg_1', 'conv_alice_1', 1710000000),
    (1, 'assistant', 'enc_msg_2', 'conv_alice_1', 1710000001),
    (2, 'user', 'enc_msg_3', 'conv_bob_1', 1710000002);

-- Usage logs
INSERT INTO usage_logs (user_id, activity_type, credits, created_at, success)
VALUES
    (1, 'sms_received', 0.5, 1710000000, true),
    (1, 'voice_call', 2.0, 1710000001, true),
    (2, 'sms_received', 0.5, 1710000002, false);

-- Items (tasks/reminders)
INSERT INTO items (user_id, summary, priority, created_at)
VALUES
    (1, 'Buy groceries', 1, 1710000000),
    (2, 'Call dentist', 2, 1710000001);

-- IMAP connections
INSERT INTO imap_connection (user_id, method, encrypted_password, status, last_update, created_on, description)
VALUES
    (1, 'password', 'enc_imap_pass', 'connected', 1710000000, 1710000000, 'Gmail');

-- MCP servers
INSERT INTO mcp_servers (user_id, name, url_encrypted, is_enabled, created_at)
VALUES
    (1, 'My MCP Server', 'enc_url', 1, 1710000000);

-- TOTP secrets
INSERT INTO totp_secrets (user_id, encrypted_secret, enabled, created_at)
VALUES
    (1, 'enc_totp_secret', 1, 1710000000);

-- TOTP backup codes
INSERT INTO totp_backup_codes (user_id, code_hash, used)
VALUES
    (1, 'hash_code_1', 0),
    (1, 'hash_code_2', 1);
