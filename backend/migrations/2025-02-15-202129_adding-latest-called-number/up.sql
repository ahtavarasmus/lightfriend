-- Your SQL goes here
-- First add the new column
ALTER TABLE users ADD COLUMN preferred_number TEXT;

-- Update verified users based on their phone number prefix
-- US numbers (starting with +1)
UPDATE users 
SET preferred_number = '+18153684737'
WHERE verified = 1 
AND phone_number LIKE '+1%';


-- netherlands numbers (starting with +46)
UPDATE users 
SET preferred_number = '+3197006520696'
WHERE verified = 1 
AND phone_number LIKE '+31%';

-- Set Finnish number as default for all remaining verified users
UPDATE users 
SET preferred_number = '+358454901522'
WHERE verified = 1 
AND preferred_number IS NULL;

