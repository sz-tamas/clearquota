-- Sanitized OpenAI cost-report records. No credentials, response bodies, or
-- authorization data are stored here.
CREATE TABLE IF NOT EXISTS openai_cost_ledger_entries (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    provider_id TEXT NOT NULL REFERENCES providers(id) ON DELETE CASCADE,
    bucket_start INTEGER NOT NULL,
    bucket_end INTEGER NOT NULL,
    amount REAL NOT NULL,
    currency TEXT NOT NULL,
    project_id TEXT,
    api_key_id TEXT,
    line_item TEXT
);

CREATE UNIQUE INDEX IF NOT EXISTS openai_cost_ledger_entries_identity
    ON openai_cost_ledger_entries (
        provider_id, bucket_start, bucket_end, currency,
        COALESCE(project_id, ''), COALESCE(api_key_id, ''), COALESCE(line_item, '')
    );

CREATE INDEX IF NOT EXISTS openai_cost_ledger_entries_provider_bucket
    ON openai_cost_ledger_entries(provider_id, bucket_start DESC);
