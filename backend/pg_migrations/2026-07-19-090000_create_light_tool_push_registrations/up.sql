CREATE TABLE light_tool_push_registrations (
    device_id INT4 PRIMARY KEY REFERENCES light_tool_devices(id) ON DELETE CASCADE,
    encrypted_endpoint TEXT NOT NULL,
    endpoint_hash TEXT NOT NULL,
    registered_at INT4 NOT NULL,
    updated_at INT4 NOT NULL
);
