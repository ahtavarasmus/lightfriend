-- Your SQL goes here
alter table users add column twilio_account_sid text;
alter table users add column twilio_auth_token text;
alter table users add column server_url text;
alter table users drop column confirm_send_event;
alter table user_settings drop column server_key;
alter table user_settings drop column encrypted_twilio_account_sid;
alter table user_settings drop column encrypted_twilio_auth_token;
alter table user_settings drop column encrypted_geoapify_key;
alter table user_settings drop column encrypted_pirate_weather_key;
alter table user_settings drop column server_instance_id;
alter table user_settings drop column server_instance_last_ping_timestamp;
alter table user_settings drop column sub_country;
alter table user_settings drop column number_of_digests_locked;
drop table temp_variables;
