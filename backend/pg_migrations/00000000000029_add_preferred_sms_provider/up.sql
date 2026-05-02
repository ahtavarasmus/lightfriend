-- Per-user SMS provider override. NULL = use country-based routing
-- (the default: US users prefer Sinch when registered, everyone else
-- gets Twilio). Setting a value pins this user to that provider for
-- both outbound (ChannelRouter::pick_channel_for) and inbound (Sinch
-- webhook bypasses its US-only check when the value is 'sinch').
--
-- Primary use: admin verification of the Sinch path from a non-US
-- phone. Future: could expose to users who want to opt into Telnyx
-- without us routing by country.
ALTER TABLE users
ADD COLUMN preferred_sms_provider TEXT;

-- Pin user 1 (admin bootstrap) to sinch so the admin can text the
-- US Sinch number from any country and have replies route back via
-- Sinch.
UPDATE users SET preferred_sms_provider = 'sinch' WHERE id = 1;
