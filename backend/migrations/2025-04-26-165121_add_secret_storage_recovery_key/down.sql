-- This file should undo anything in `up.sql`
alter table users drop column encrypted_matrix_secret_storage_recovery_key;
