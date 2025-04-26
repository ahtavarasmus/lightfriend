-- Your SQL goes here
alter table users drop column matrix_password;
alter table users add column encrypted_matrix_password TEXT;
