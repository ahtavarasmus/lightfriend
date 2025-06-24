-- This file should undo anything in `up.sql`
alter table proactive_settings drop column proactive_telegram;
alter table proactive_settings drop column telegram_general_checks;
alter table proactive_settings drop column telegram_keywords_active;
alter table proactive_settings drop column telegram_priority_senders_active;
alter table proactive_settings drop column telegram_waiting_checks_active;
alter table proactive_settings drop column telegram_general_importance_active;
