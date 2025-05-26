-- Your SQL goes here
alter table proactive_settings add column whatsapp_keywords_active boolean not null default true;
alter table proactive_settings add column whatsapp_priority_senders_active boolean not null default true;
alter table proactive_settings add column whatsapp_waiting_checks_active boolean not null default true;
alter table proactive_settings add column whatsapp_general_importance_active boolean not null default true;


alter table proactive_settings add column email_keywords_active boolean not null default true;
alter table proactive_settings add column email_priority_senders_active boolean not null default true;
alter table proactive_settings add column email_waiting_checks_active boolean not null default true;
alter table proactive_settings add column email_general_importance_active boolean not null default true;
