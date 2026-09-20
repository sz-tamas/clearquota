ALTER TABLE providers ADD COLUMN daily_quota INTEGER NOT NULL DEFAULT 0 CHECK (daily_quota >= 0);
