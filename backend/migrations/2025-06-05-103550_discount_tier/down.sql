-- This file should undo anything in `up.sql`
alter table users drop column discount_tier;
alter table users add column debug_logging_permission boolean not null default false;
