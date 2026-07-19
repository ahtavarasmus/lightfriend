-- Serialize bridge reconnects with destructive Tuwunel room cleanup and keep
-- evidence that bridge-side portal cleanup completed before room deletion.
CREATE TABLE IF NOT EXISTS bridge_connection_leases (
    user_id INT4 NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    bridge_type TEXT NOT NULL,
    lease_kind TEXT NOT NULL,
    owner_token TEXT NOT NULL,
    lease_until INT4 NOT NULL,
    updated_at INT4 NOT NULL,
    PRIMARY KEY (user_id, bridge_type)
);

ALTER TABLE bridge_cleanup_jobs
    ADD COLUMN IF NOT EXISTS portal_cleanup_status TEXT NOT NULL DEFAULT 'pending',
    ADD COLUMN IF NOT EXISTS portal_cleanup_confirmed_at INT4,
    ADD COLUMN IF NOT EXISTS portal_cleanup_error TEXT,
    ADD COLUMN IF NOT EXISTS rootfs_free_before_bytes INT8,
    ADD COLUMN IF NOT EXISTS rootfs_free_after_bytes INT8,
    ADD COLUMN IF NOT EXISTS tuwunel_before_bytes INT8,
    ADD COLUMN IF NOT EXISTS tuwunel_after_bytes INT8;

UPDATE bridge_cleanup_jobs
   SET portal_cleanup_status = 'legacy_unverified'
 WHERE expected_bridge_id IS NULL
   AND portal_cleanup_status = 'pending';
