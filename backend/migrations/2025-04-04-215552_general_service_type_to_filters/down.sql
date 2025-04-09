-- This file should undo anything in `up.sql`
alter table waiting_checks add column waiting_type TEXT NOT NULL DEFAULT "email";
alter table importance_priorities add column importance_type TEXT NOT NULL DEFAULT "email";
alter table importance_priorities drop column service_type;
alter table waiting_checks drop column service_type;
