-- Your SQL goes here
CREATE TABLE subscriptions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL,
    paddle_subscription_id TEXT NOT NULL,
    paddle_customer_id TEXT NOT NULL,
    stage TEXT NOT NULL,
    status TEXT NOT NULL,
    next_bill_date INTEGER NOT NULL,
    FOREIGN KEY (user_id) REFERENCES users(id)
);
