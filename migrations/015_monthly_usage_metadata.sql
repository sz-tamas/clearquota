-- Sanitized provider-specific period context. This may contain billing-cycle
-- boundaries and normalized totals, but never credentials or response bodies.
ALTER TABLE monthly_usage_snapshots
    ADD COLUMN metadata_json TEXT NOT NULL DEFAULT '{}';
