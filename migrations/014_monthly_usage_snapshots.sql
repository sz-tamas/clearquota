-- Sanitized normalized metrics keyed by UTC calendar month. Secret values and
-- provider response bodies must never be stored here.
CREATE TABLE IF NOT EXISTS monthly_usage_snapshots (
    provider_id TEXT NOT NULL REFERENCES providers(id) ON DELETE CASCADE,
    period_start INTEGER NOT NULL,
    period_end INTEGER NOT NULL,
    refreshed_at TEXT NOT NULL,
    status TEXT NOT NULL,
    cost REAL,
    currency TEXT,
    metrics_json TEXT NOT NULL,
    PRIMARY KEY (provider_id, period_start),
    CHECK (period_end > period_start)
);

CREATE INDEX IF NOT EXISTS monthly_usage_snapshots_period
    ON monthly_usage_snapshots(period_start DESC);

-- Preserve useful prototype history by taking the last refresh captured in each
-- provider/month. These inferred rows remain replaceable by an exact period
-- refresh through the normal upsert path.
INSERT OR IGNORE INTO monthly_usage_snapshots (
    provider_id,
    period_start,
    period_end,
    refreshed_at,
    status,
    cost,
    currency,
    metrics_json
)
SELECT
    snapshot.provider_id,
    CAST(strftime('%s', snapshot.timestamp, 'unixepoch', 'start of month') AS INTEGER),
    CAST(strftime('%s', snapshot.timestamp, 'unixepoch', 'start of month', '+1 month') AS INTEGER),
    snapshot.timestamp,
    snapshot.status,
    snapshot.cost,
    snapshot.currency,
    snapshot.raw_metrics_json
FROM usage_snapshots AS snapshot
WHERE snapshot.id = (
    SELECT candidate.id
    FROM usage_snapshots AS candidate
    WHERE candidate.provider_id = snapshot.provider_id
      AND strftime('%Y-%m', candidate.timestamp, 'unixepoch') =
          strftime('%Y-%m', snapshot.timestamp, 'unixepoch')
    ORDER BY CAST(candidate.timestamp AS INTEGER) DESC, candidate.id DESC
    LIMIT 1
);
