-- This file should undo anything in `up.sql`
alter table users add column msgs_left integer not null default 0;
