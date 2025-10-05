-- Your SQL goes here
alter table users drop column next_billing_date_timestamp;
alter table users drop column waiting_checks_count;
alter table users drop column free_reply;
alter table users drop column discount_tier;
alter table users drop column discount;
alter table users drop column last_credits_notification;
alter table users drop column encrypted_matrix_secret_storage_recovery_key;
alter table users drop column sub_tier;
alter table users drop column stripe_checkout_session_id;
alter table users drop column stripe_payment_method_id;
alter table users drop column stripe_customer_id;
alter table users drop column charge_back_to;
alter table users drop column charge_when_under;
alter table users drop column verified;
alter table users drop column time_to_live;
alter table users drop column password_hash;
alter table users drop column email;
alter table users add column twilio_messaging_service_sid text;
