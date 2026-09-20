ALTER TABLE providers ADD COLUMN apify_monthly_credit_allowance REAL NOT NULL DEFAULT 0 CHECK (apify_monthly_credit_allowance >= 0);
