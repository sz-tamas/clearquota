use std::{fs, path::Path};

use rusqlite::{Connection, OptionalExtension, params};

use crate::models::{
    Account, NewAccount, NewProvider, ProviderConfig, UpdateAccount, UpdateProvider, UsageSnapshot,
};

#[derive(Clone)]
pub struct Database {
    path: std::path::PathBuf,
}

impl Database {
    pub fn open(path: &Path) -> Result<Self, rusqlite::Error> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|_| rusqlite::Error::InvalidPath(path.to_path_buf()))?;
        }
        let db = Self {
            path: path.to_path_buf(),
        };
        db.connection()?
            .execute_batch("PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL;")?;
        Ok(db)
    }

    fn connection(&self) -> Result<Connection, rusqlite::Error> {
        let connection = Connection::open(&self.path)?;
        connection.execute_batch("PRAGMA foreign_keys = ON;")?;
        Ok(connection)
    }

    pub fn migrate(&self) -> Result<(), rusqlite::Error> {
        let connection = self.connection()?;
        connection.execute_batch(include_str!("../migrations/001_initial.sql"))?;
        // A fresh database receives the account relationship. Existing prototype
        // databases may already have this column, so tolerate that one migration error.
        match connection.execute_batch(include_str!(
            "../migrations/002_connections_and_onboarding.sql"
        )) {
            Ok(()) => (),
            Err(error) if error.to_string().contains("duplicate column name") => (),
            Err(error) => return Err(error),
        };
        match connection.execute_batch(include_str!(
            "../migrations/003_provider_refresh_status.sql"
        )) {
            Ok(()) => (),
            Err(error) if error.to_string().contains("duplicate column name") => (),
            Err(error) => return Err(error),
        };
        match connection.execute_batch(include_str!("../migrations/005_provider_plan_quota.sql")) {
            Ok(()) => (),
            Err(error) if error.to_string().contains("duplicate column name") => (),
            Err(error) => return Err(error),
        };
        connection.execute_batch(include_str!(
            "../migrations/006_clear_obsolete_resend_quota_error.sql"
        ))?;
        match connection.execute_batch(include_str!("../migrations/007_resend_daily_quota.sql")) {
            Ok(()) => (),
            Err(error) if error.to_string().contains("duplicate column name") => (),
            Err(error) => return Err(error),
        };
        match connection.execute_batch(include_str!(
            "../migrations/008_apify_monthly_credit_allowance.sql"
        )) {
            Ok(()) => (),
            Err(error) if error.to_string().contains("duplicate column name") => (),
            Err(error) => return Err(error),
        };
        Self::compact_provider_secret_names(&connection)
    }

    fn compact_provider_secret_names(connection: &Connection) -> Result<(), rusqlite::Error> {
        let mut statement = connection.prepare("SELECT id, secret_ref FROM providers")?;
        let entries = statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        for (id, reference) in entries {
            let parts: Vec<_> = reference.split('/').collect();
            if parts.len() >= 4
                && parts[0] == "projects"
                && parts[2] == "secrets"
                && !parts[3].is_empty()
            {
                connection.execute(
                    "UPDATE providers SET secret_ref = ?2 WHERE id = ?1",
                    params![id, parts[3]],
                )?;
            }
        }
        Ok(())
    }

    pub fn list_providers(&self, account_id: &str) -> Result<Vec<ProviderConfig>, rusqlite::Error> {
        let connection = self.connection()?;
        let mut statement = connection.prepare("SELECT id, account_id, provider_type, display_name, secret_ref, enabled, last_error, plan, monthly_quota, daily_quota, apify_monthly_credit_allowance FROM providers WHERE account_id = ?1 ORDER BY created_at DESC")?;
        statement
            .query_map([account_id], |row| {
                Ok(ProviderConfig {
                    id: row.get(0)?,
                    account_id: row.get(1)?,
                    provider_type: row.get(2)?,
                    display_name: row.get(3)?,
                    secret_ref: row.get(4)?,
                    enabled: row.get::<_, i64>(5)? != 0,
                    last_error: row.get(6)?,
                    plan: row.get(7)?,
                    monthly_quota: row.get(8)?,
                    daily_quota: row.get(9)?,
                    apify_monthly_credit_allowance: row.get(10)?,
                })
            })?
            .collect()
    }

    pub fn find_provider(&self, id: &str) -> Result<Option<ProviderConfig>, rusqlite::Error> {
        self.connection()?.query_row("SELECT id, account_id, provider_type, display_name, secret_ref, enabled, last_error, plan, monthly_quota, daily_quota, apify_monthly_credit_allowance FROM providers WHERE id = ?1", [id], |row| Ok(ProviderConfig { id: row.get(0)?, account_id: row.get(1)?, provider_type: row.get(2)?, display_name: row.get(3)?, secret_ref: row.get(4)?, enabled: row.get::<_, i64>(5)? != 0, last_error: row.get(6)?, plan: row.get(7)?, monthly_quota: row.get(8)?, daily_quota: row.get(9)?, apify_monthly_credit_allowance: row.get(10)? })).optional()
    }

    pub fn add_provider(
        &self,
        account_id: &str,
        input: NewProvider,
    ) -> Result<(), rusqlite::Error> {
        let now = now();
        let id = format!(
            "{}-{}",
            input.provider_type,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis()
        );
        self.connection()?.execute("INSERT INTO providers (id, account_id, provider_type, display_name, secret_ref, plan, monthly_quota, daily_quota, apify_monthly_credit_allowance, enabled, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 1, ?10, ?10)", params![id, account_id, input.provider_type, input.display_name, input.secret_ref, input.plan.unwrap_or_default(), input.monthly_quota.unwrap_or(0), input.daily_quota.unwrap_or(0), input.apify_monthly_credit_allowance.unwrap_or(0.0), now])?;
        Ok(())
    }

    pub fn update_provider(
        &self,
        id: &str,
        account_id: &str,
        input: UpdateProvider,
    ) -> Result<(), rusqlite::Error> {
        self.connection()?.execute("UPDATE providers SET display_name = ?3, secret_ref = ?4, plan = ?5, monthly_quota = ?6, daily_quota = ?7, apify_monthly_credit_allowance = ?8, updated_at = ?9 WHERE id = ?1 AND account_id = ?2", params![id, account_id, input.display_name, input.secret_ref, input.plan.unwrap_or_default(), input.monthly_quota.unwrap_or(0), input.daily_quota.unwrap_or(0), input.apify_monthly_credit_allowance.unwrap_or(0.0), now()])?;
        Ok(())
    }

    pub fn delete_provider(&self, id: &str, account_id: &str) -> Result<(), rusqlite::Error> {
        let removed = self.connection()?.execute(
            "DELETE FROM providers WHERE id = ?1 AND account_id = ?2",
            params![id, account_id],
        )?;
        if removed == 0 {
            return Err(rusqlite::Error::QueryReturnedNoRows);
        }
        Ok(())
    }

    pub fn active_account(&self) -> Result<Option<Account>, rusqlite::Error> {
        self.connection()?.query_row("SELECT id, project_id, project_name, auth_status, auth_error FROM accounts WHERE is_active = 1 ORDER BY created_at DESC LIMIT 1", [], |row| Ok(Account { id: row.get(0)?, project_id: row.get(1)?, project_name: row.get(2)?, auth_status: row.get(3)?, auth_error: row.get(4)? })).optional()
    }

    pub fn create_account(&self, input: NewAccount) -> Result<Account, rusqlite::Error> {
        let now = now();
        let id = format!(
            "gcp-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis()
        );
        let connection = self.connection()?;
        connection.execute("UPDATE accounts SET is_active = 0 WHERE is_active = 1", [])?;
        connection.execute("INSERT INTO accounts (id, project_id, is_active, created_at, updated_at) VALUES (?1, ?2, 1, ?3, ?3)", params![id, input.project_id, now])?;
        connection.execute(
            "INSERT INTO onboarding (account_id, current_step) VALUES (?1, 1)",
            [&id],
        )?;
        self.active_account()?
            .ok_or(rusqlite::Error::QueryReturnedNoRows)
    }

    pub fn update_active_account(&self, input: UpdateAccount) -> Result<(), rusqlite::Error> {
        self.connection()?.execute("UPDATE accounts SET project_id = ?1, project_name = NULL, updated_at = ?2 WHERE is_active = 1", params![input.project_id, now()])?;
        Ok(())
    }

    pub fn set_auth_status(
        &self,
        account_id: &str,
        status: &str,
        project_name: Option<&str>,
        error: Option<&str>,
    ) -> Result<(), rusqlite::Error> {
        self.connection()?.execute("UPDATE accounts SET auth_status = ?2, project_name = COALESCE(?3, project_name), auth_error = ?4, updated_at = ?5 WHERE id = ?1", params![account_id, status, project_name, error, now()])?;
        Ok(())
    }
    pub fn set_onboarding_step(&self, account_id: &str, step: i64) -> Result<(), rusqlite::Error> {
        self.connection()?.execute("UPDATE onboarding SET current_step = ?2, completed_at = CASE WHEN ?2 = 4 THEN ?3 ELSE completed_at END WHERE account_id = ?1", params![account_id, step, now()])?;
        Ok(())
    }

    pub fn latest_snapshot(
        &self,
        provider_id: &str,
    ) -> Result<Option<UsageSnapshot>, rusqlite::Error> {
        self.connection()?.query_row("SELECT timestamp, status, cost, currency, raw_metrics_json FROM usage_snapshots WHERE provider_id = ?1 ORDER BY id DESC LIMIT 1", [provider_id], |row| Ok(UsageSnapshot { provider_id: provider_id.to_owned(), timestamp: row.get(0)?, status: row.get(1)?, cost: row.get(2)?, currency: row.get(3)?, metrics: serde_json::from_str(&row.get::<_, String>(4)?).unwrap_or_default() })).optional()
    }

    pub fn save_snapshot(&self, snapshot: &UsageSnapshot) -> Result<(), rusqlite::Error> {
        let metrics = serde_json::to_string(&snapshot.metrics).unwrap_or_else(|_| "[]".to_owned());
        self.connection()?.execute("INSERT INTO usage_snapshots (provider_id, timestamp, status, cost, currency, raw_metrics_json) VALUES (?1, ?2, ?3, ?4, ?5, ?6)", params![snapshot.provider_id, snapshot.timestamp, snapshot.status, snapshot.cost, snapshot.currency, metrics])?;
        Ok(())
    }

    pub fn set_provider_refresh_error(
        &self,
        id: &str,
        error: Option<&str>,
    ) -> Result<(), rusqlite::Error> {
        self.connection()?.execute(
            "UPDATE providers SET last_error = ?2, updated_at = ?3 WHERE id = ?1",
            params![id, error, now()],
        )?;
        Ok(())
    }
}

fn now() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .to_string()
}
