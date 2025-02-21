-- This file should undo anything in `up.sql`
CREATE TABLE phone_verification_otps (
    id INTEGER PRIMARY KEY NOT NULL,
    user_id INTEGER NOT NULL,
    phone_number TEXT NOT NULL,
    otp_code TEXT NOT NULL,
    created_at TIMESTAMP NOT NULL,
    expires_at TIMESTAMP NOT NULL,
    FOREIGN KEY (user_id) REFERENCES users(id)
);
