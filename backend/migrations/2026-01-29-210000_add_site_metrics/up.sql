CREATE TABLE site_metrics (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    metric_key TEXT NOT NULL UNIQUE,
    metric_value TEXT NOT NULL,
    updated_at INTEGER NOT NULL
);
