-- Your SQL goes here
alter table priority_senders add column service_type TEXT NOT NULL DEFAULT "email";
alter table keywords add column service_type TEXT NOT NULL DEFAULT "email";
