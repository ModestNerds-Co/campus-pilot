-- Keep entitlement usage storage aligned with the platform table lifecycle
-- convention. Usage events remain append-only; their lifecycle columns are
-- initial evidence and tenant-ownership metadata, not mutation APIs.

ALTER TABLE entitlement_meter_buckets
ADD COLUMN IF NOT EXISTS deleted_at TIMESTAMPTZ;

ALTER TABLE entitlement_usage_reservations
ADD COLUMN IF NOT EXISTS deleted_at TIMESTAMPTZ;

ALTER TABLE entitlement_usage_events
ADD COLUMN IF NOT EXISTS updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW();

ALTER TABLE entitlement_usage_events
ADD COLUMN IF NOT EXISTS deleted_at TIMESTAMPTZ;
