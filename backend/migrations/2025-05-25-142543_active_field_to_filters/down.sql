-- This file should undo anything in `up.sql`

alter table proactive_settings drop column whatsapp_keywords_active;
alter table proactive_settings drop column whatsapp_priority_senders_active;
alter table proactive_settings drop column whatsapp_waiting_checks_active;
alter table proactive_settings drop column whatsapp_general_importance_active;


alter table proactive_settings drop column email_keywords_active;
alter table proactive_settings drop column email_priority_senders_active;
alter table proactive_settings drop column email_waiting_checks_active;
alter table proactive_settings drop column email_general_importance_active;
