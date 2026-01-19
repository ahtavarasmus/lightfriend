DROP INDEX IF EXISTS idx_disabled_alert_types_type;
DROP TABLE IF EXISTS disabled_alert_types;

DROP INDEX IF EXISTS idx_admin_alerts_acknowledged;
DROP INDEX IF EXISTS idx_admin_alerts_severity;
DROP INDEX IF EXISTS idx_admin_alerts_created;
DROP INDEX IF EXISTS idx_admin_alerts_type;
DROP TABLE IF EXISTS admin_alerts;
