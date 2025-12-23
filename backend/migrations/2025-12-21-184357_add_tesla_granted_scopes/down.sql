-- Remove granted_scopes column
-- Note: SQLite doesn't support DROP COLUMN directly in older versions
-- This creates a new table without the column and migrates data
CREATE TABLE tesla_backup AS SELECT
    id, user_id, encrypted_access_token, encrypted_refresh_token,
    status, last_update, created_on, expires_in, region,
    selected_vehicle_vin, selected_vehicle_name, selected_vehicle_id,
    virtual_key_paired
FROM tesla;

DROP TABLE tesla;

CREATE TABLE tesla (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL UNIQUE REFERENCES users(id),
    encrypted_access_token TEXT NOT NULL,
    encrypted_refresh_token TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'active',
    last_update INTEGER NOT NULL,
    created_on INTEGER NOT NULL,
    expires_in INTEGER NOT NULL,
    region TEXT NOT NULL DEFAULT 'https://fleet-api.prd.eu.vn.cloud.tesla.com',
    selected_vehicle_vin TEXT,
    selected_vehicle_name TEXT,
    selected_vehicle_id TEXT,
    virtual_key_paired INTEGER NOT NULL DEFAULT 0
);

INSERT INTO tesla SELECT * FROM tesla_backup;
DROP TABLE tesla_backup;
