-- This file should undo anything in `up.sql`

alter table proactive_settings drop column proactive_calendar_last_activated;
alter table proactive_settings drop column proactive_email_last_activated;
