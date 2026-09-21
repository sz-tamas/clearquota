-- A conservative completeness flag for normalized monthly data. Existing rows
-- default to partial because their source coverage cannot be proven retroactively.
ALTER TABLE monthly_usage_snapshots
    ADD COLUMN is_partial INTEGER NOT NULL DEFAULT 1 CHECK (is_partial IN (0, 1));
