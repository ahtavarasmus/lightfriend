-- This file should undo anything in `up.sql`
alter table user_settings drop column encrypted_textbee_device_id;
alter table user_settings drop column encrypted_textbee_api_key;
