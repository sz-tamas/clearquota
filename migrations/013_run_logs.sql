-- Audit trail for provider refresh attempts. Entries deliberately contain only
-- normalized outcomes; credentials, secret references, headers, and provider
-- response bodies must never be stored here.
CREATE TABLE IF NOT EXISTS run_logs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    provider_id TEXT NOT NULL REFERENCES providers(id) ON DELETE CASCADE,
    created_at TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('succeeded', 'failed')),
    message TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS run_logs_provider_created_at
    ON run_logs(provider_id, created_at DESC);
