-- Create admin_alerts table to store all alert history
CREATE TABLE admin_alerts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    alert_type TEXT NOT NULL,
    severity TEXT NOT NULL,
    message TEXT NOT NULL,
    location TEXT NOT NULL,
    module TEXT NOT NULL,
    acknowledged INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL
);

CREATE INDEX idx_admin_alerts_type ON admin_alerts(alert_type);
CREATE INDEX idx_admin_alerts_created ON admin_alerts(created_at);
CREATE INDEX idx_admin_alerts_severity ON admin_alerts(severity);
CREATE INDEX idx_admin_alerts_acknowledged ON admin_alerts(acknowledged);

-- Create disabled_alert_types table to track disabled alerts
CREATE TABLE disabled_alert_types (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    alert_type TEXT NOT NULL UNIQUE,
    disabled_at INTEGER NOT NULL
);

CREATE INDEX idx_disabled_alert_types_type ON disabled_alert_types(alert_type);
