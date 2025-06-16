-- This file should undo anything in `up.sql`
alter table users add column confirm_send_event boolean not null default false;
