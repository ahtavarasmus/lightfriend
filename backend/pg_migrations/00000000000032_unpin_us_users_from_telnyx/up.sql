-- Twilio US is back online; revert the default for US users to Twilio.
-- Migration 31 pinned every +1 user to Telnyx while Twilio was degraded
-- and our 10DLC campaign (CUV1XUL) was approved on Telnyx as a stopgap.
-- Clear those pins so US users fall back to country-based routing,
-- which now defaults to Twilio (router.rs).
--
-- Telnyx stays wired as a channel. Individual users can still be pinned
-- to it via /api/admin/set-preferred-sms-provider if Twilio degrades
-- again or for testing.
--
-- Only clears telnyx pins on +1 numbers — any non-US telnyx overrides
-- were set by admins explicitly and are left alone.
UPDATE users
SET preferred_sms_provider = NULL
WHERE preferred_sms_provider = 'telnyx'
  AND phone_number LIKE '+1%';
