DROP TABLE IF EXISTS bridge_connection_leases;

ALTER TABLE bridge_cleanup_jobs
    DROP COLUMN IF EXISTS tuwunel_after_bytes,
    DROP COLUMN IF EXISTS tuwunel_before_bytes,
    DROP COLUMN IF EXISTS rootfs_free_after_bytes,
    DROP COLUMN IF EXISTS rootfs_free_before_bytes,
    DROP COLUMN IF EXISTS portal_cleanup_error,
    DROP COLUMN IF EXISTS portal_cleanup_confirmed_at,
    DROP COLUMN IF EXISTS portal_cleanup_status;
