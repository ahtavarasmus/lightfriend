-- Pin every US user (E.164 starting with +1) to Telnyx explicitly.
-- Country-based routing in ChannelRouter::pick_channel_for already
-- prefers telnyx for US when the channel is registered, but this
-- migration makes the choice explicit in the row and overrides any
-- prior pins (notably user id=1 was set to 'sinch' in migration 29).
--
-- Why now: Twilio is degraded; Telnyx is the only working US provider
-- and our 10DLC campaign (CUV1XUL) is approved on it.
UPDATE users
SET preferred_sms_provider = 'telnyx'
WHERE phone_number LIKE '+1%';
