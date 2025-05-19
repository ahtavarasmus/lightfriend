-- Your SQL goes here
alter table users add column confirm_send_event boolean not null default false;
