-- This file should undo anything in `up.sql`
DROP TABLE IF EXISTS bridges;

alter table users drop column matrix_username;
alter table users drop column encrypted_matrix_access_token;


