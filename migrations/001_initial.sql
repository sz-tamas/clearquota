CREATE TABLE IF NOT EXISTS providers (
    id TEXT PRIMARY KEY NOT NULL,
    provider_type TEXT NOT NULL,
    display_name TEXT NOT NULL,
    secret_ref TEXT NOT NULL,
    enabled INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0, 1)),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS usage_snapshots (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    provider_id TEXT NOT NULL REFERENCES providers(id) ON DELETE CASCADE,
    timestamp TEXT NOT NULL,
    status TEXT NOT NULL,
    cost REAL,
    currency TEXT,
    raw_metrics_json TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS usage_snapshots_provider_timestamp
    ON usage_snapshots(provider_id, timestamp DESC);
