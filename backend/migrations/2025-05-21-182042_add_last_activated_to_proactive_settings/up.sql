-- Your SQL goes here
alter table proactive_settings add column proactive_calendar_last_activated integer not null default 0;
alter table proactive_settings add column proactive_email_last_activated integer not null default 0;
