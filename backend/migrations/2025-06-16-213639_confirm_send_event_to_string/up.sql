-- Your SQL goes here
alter table users drop column confirm_send_event;
alter table users add column confirm_send_event text;
