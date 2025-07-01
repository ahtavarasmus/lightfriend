-- Your SQL goes here
alter table user_settings add column require_confirmation boolean not null default true;
