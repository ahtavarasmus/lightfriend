-- Your SQL goes here
alter table users add column waiting_checks_count integer not null default 0;
alter table user_settings add column morning_digest text;
alter table user_settings add column day_digest text;
alter table user_settings add column evening_digest text;
