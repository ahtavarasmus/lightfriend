-- This file should undo anything in `up.sql`
alter table users drop column next_billing_date_timestamp;
