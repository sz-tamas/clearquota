-- Sanitized, aggregated OpenAI completions usage. No credentials, request
-- content, response bodies, or authorization data are stored here.
CREATE TABLE IF NOT EXISTS openai_usage_ledger_entries (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    provider_id TEXT NOT NULL REFERENCES providers(id) ON DELETE CASCADE,
    bucket_start INTEGER NOT NULL,
    bucket_end INTEGER NOT NULL,
    input_tokens INTEGER NOT NULL,
    cached_input_tokens INTEGER NOT NULL,
    output_tokens INTEGER NOT NULL,
    request_count INTEGER NOT NULL,
    project_id TEXT,
    model TEXT
);

CREATE UNIQUE INDEX IF NOT EXISTS openai_usage_ledger_entries_identity
    ON openai_usage_ledger_entries (
        provider_id, bucket_start, bucket_end,
        COALESCE(project_id, ''), COALESCE(model, '')
    );

CREATE INDEX IF NOT EXISTS openai_usage_ledger_entries_provider_bucket
    ON openai_usage_ledger_entries(provider_id, bucket_start DESC);
