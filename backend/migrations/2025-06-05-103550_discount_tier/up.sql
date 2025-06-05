-- Your SQL goes here
alter table users add column discount_tier TEXT;
alter table users drop column debug_logging_permission;
