-- Your SQL goes here
alter table user_settings add column encrypted_twilio_account_sid text;
alter table user_settings add column encrypted_twilio_auth_token text;
alter table user_settings add column encrypted_openrouter_api_key text;
alter table user_settings add column server_url text;
alter table user_settings add column encrypted_geoapify_key text;
alter table user_settings add column encrypted_pirate_weather_key text;
alter table user_settings add column server_instance_id text;
alter table user_settings add column server_instance_last_ping_timestamp integer;
