-- This file should undo anything in `up.sql`
alter table priority_senders drop column noti_type;
alter table waiting_checks drop column noti_type;
