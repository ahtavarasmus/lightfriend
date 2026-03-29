# Deploy Pipeline Cleanup TODO

## DONE (this commit)

- [x] Delete `export-trigger` VSOCK 9003 from supervisord.conf
- [x] Delete `maintenance-trigger` VSOCK 9005 from supervisord.conf
- [x] Delete `vsock-backup-receiver` port 9001 from user_data.sh
- [x] Delete `vsock-restore-server` port 9002 from user_data.sh
- [x] Delete `vsock-seed-server` port 9003 from user_data.sh
- [x] Delete `trigger-export.sh` from user_data.sh
- [x] Delete `trigger-maintenance.sh` from user_data.sh
- [x] Delete raw VSOCK 9002 fallback from entrypoint backup download
- [x] Delete raw VSOCK 9004 abort paths from entrypoint restore
- [x] Standardize PORT default to 3100 everywhere
- [x] Move startup scripts from /tmp/ to /data/seed/
- [x] Fix tuwunel checkpoint cleanup in export.sh
- [x] Remove dead services from VSOCK_SVCS lists

## Remaining TODO

| # | What | Severity | Fix |
|---|------|----------|-----|
| 1 | Old EIF files accumulating in S3 | Medium | Add S3 lifecycle policy or cleanup step in workflow |
| 2 | Backup files accumulating on host | Medium | Add cron/cleanup in user_data.sh |
| 3 | No retry on verify result upload | Medium | Check HTTP response, retry on failure |
| 4 | Magic sleep values undocumented | Low | Add comments |
| 5 | Hardcoded DB credentials in entrypoint | Low | Move to env vars from host |

## Architectural (longer term)

| # | What | Notes |
|---|------|-------|
| 6 | user_data.sh is 600+ lines embedded in Terraform | Extract scripts to separate files |
| 7 | s3-signal-poller old code on existing instances | Only fixed on new instances via deploy. Poller restart in workflow is the workaround. |
| 8 | No maintenance mode check before export | pg_dump runs while backend serves writes |
| 9 | KMS unreachable = enclave exits, no degraded mode | By design for trustless, but needs documentation |
| 10 | Export not resumable on encryption failure | Staging deleted, must re-run from scratch |
