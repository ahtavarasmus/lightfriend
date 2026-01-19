-- Drop phone_number_country column - now detected on demand via phonenumber crate
ALTER TABLE users DROP COLUMN phone_number_country;
