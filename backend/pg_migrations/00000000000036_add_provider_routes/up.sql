-- Per-country ordered list of SMS providers for outbound fallback.
-- The router tries providers in this order and falls through to the
-- next on send failure. Two-letter ISO codes match utils::country
-- output. provider_order is a JSON array of channel ids registered
-- in ChannelRouter, e.g. '["twilio","telnyx","sinch"]'.
--
-- Countries with no row inherit the router default ("twilio" only).
-- Operators can flip a country's primary at runtime via
-- POST /api/admin/provider-routes without a redeploy.
CREATE TABLE IF NOT EXISTS provider_routes (
    country_code TEXT PRIMARY KEY,
    provider_order TEXT NOT NULL,
    updated_at INT4 NOT NULL
);

-- Seed US with the current behavior: Twilio primary, Telnyx fallback.
-- Sinch is registered too but kept out of the default order until
-- we've validated end-to-end deliverability through it.
INSERT INTO provider_routes (country_code, provider_order, updated_at)
VALUES ('US', '["twilio","telnyx"]', EXTRACT(EPOCH FROM NOW())::INT4)
ON CONFLICT (country_code) DO NOTHING;
