CREATE TABLE IF NOT EXISTS openai_credit_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    provider_id TEXT NOT NULL REFERENCES providers(id) ON DELETE CASCADE,
    event_type TEXT NOT NULL CHECK (event_type IN ('purchase', 'refund', 'adjustment')),
    effective_at INTEGER NOT NULL,
    amount REAL NOT NULL CHECK (amount > 0),
    note TEXT NOT NULL DEFAULT ''
);

CREATE INDEX IF NOT EXISTS openai_credit_events_provider_date
    ON openai_credit_events(provider_id, effective_at, id);
