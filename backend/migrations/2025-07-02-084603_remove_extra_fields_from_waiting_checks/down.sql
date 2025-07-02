-- This file should undo anything in `up.sql`
alter table waiting_checks add column due_date integer not null;
alter table waiting_checks add column remove_when_found boolean not null;
