-- Your SQL goes here
alter table waiting_checks drop column waiting_type;
alter table importance_priorities drop column importance_type;
alter table importance_priorities add column service_type TEXT NOT NULL DEFAULT "email";
alter table waiting_checks add column service_type TEXT NOT NULL DEFAULT "email";
