-- This file should undo anything in `up.sql`

alter table priority_senders drop column service_type;
alter table keywords drop column service_type;
