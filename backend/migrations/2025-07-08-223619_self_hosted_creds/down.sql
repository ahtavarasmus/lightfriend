-- This file should undo anything in `up.sql`
alter table user_settings drop column encrypted_twilio_account_sid;
alter table user_settings drop column encrypted_twilio_auth_token;
alter table user_settings drop column encrypted_openrouter_api_key;
alter table user_settings drop column server_url;
alter table user_settings drop column encrypted_geoapify_key;
alter table user_settings drop column encrypted_pirate_weather_key;
alter table user_settings drop column server_instance_id;
alter table user_settings drop column server_instance_last_ping_timestamp;
