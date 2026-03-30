ALTER TABLE user_settings ADD COLUMN IF NOT EXISTS auto_track_items_system BOOLEAN NOT NULL DEFAULT false;
