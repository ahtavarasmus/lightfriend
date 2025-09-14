-- Your SQL goes here
alter table bridges add column last_seen_online integer;
alter table bridges drop column cooldown_seconds;
