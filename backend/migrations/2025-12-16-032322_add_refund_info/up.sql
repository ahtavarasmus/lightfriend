CREATE TABLE refund_info (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL UNIQUE,
    has_refunded INTEGER NOT NULL DEFAULT 0,
    last_credit_pack_amount REAL,
    last_credit_pack_purchase_timestamp INTEGER,
    refunded_at INTEGER,
    FOREIGN KEY (user_id) REFERENCES users(id)
);
