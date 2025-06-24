-- Your SQL goes here

alter table proactive_settings add column proactive_telegram boolean not null default true;
alter table proactive_settings add column telegram_general_checks text;
alter table proactive_settings add column telegram_keywords_active boolean not null default true;
alter table proactive_settings add column telegram_priority_senders_active boolean not null default true;
alter table proactive_settings add column telegram_waiting_checks_active boolean not null default true;
alter table proactive_settings add column telegram_general_importance_active boolean not null default true;
