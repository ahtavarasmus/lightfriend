-- This file should undo anything in `up.sql`
alter table users add column twilio_messaging_service_sid text;
