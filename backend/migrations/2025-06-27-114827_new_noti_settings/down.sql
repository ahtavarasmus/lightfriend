-- This file should undo anything in `up.sql`
alter table users drop column waiting_checks_count;
alter table user_settings drop column morning_digest;
alter table user_settings drop column day_digest;
alter table user_settings drop column evening_digest;
