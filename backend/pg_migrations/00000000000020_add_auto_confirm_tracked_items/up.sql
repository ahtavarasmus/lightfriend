ALTER TABLE user_settings ADD COLUMN IF NOT EXISTS auto_confirm_tracked_items BOOLEAN NOT NULL DEFAULT true;
