-- Add latitude and longitude columns to user_info for caching geocoded coordinates
ALTER TABLE user_info ADD COLUMN latitude REAL;
ALTER TABLE user_info ADD COLUMN longitude REAL;
