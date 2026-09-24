use std::{fs, path::Path};

use rusqlite::{Connection, OptionalExtension, params};

use crate::models::{
	Account, NewAccount, NewProvider, OpenAiActivitySummary, OpenAiCostLedger, OpenAiCreditEvent, OpenAiDailyActivity,
	OpenAiDailySpend, OpenAiProjectSpend, OpenAiUsageLedger, ProviderConfig, RunLog, UpdateAccount, UpdateProvider,
	UsageSnapshot,
};

#[derive(Clone)]
pub struct Database {
	path: std::path::PathBuf,
}

impl Database {
	pub fn storage_path(&self) -> std::path::PathBuf {
		if self.path.is_absolute() {
			self.path.clone()
		} else {
			std::env::current_dir().unwrap_or_default().join(&self.path)
		}
	}

	pub fn storage_size(&self) -> u64 {
		let path = self.storage_path();
		let sidecar = |suffix: &str| {
			let mut name = path.as_os_str().to_os_string();
			name.push(suffix);
			std::path::PathBuf::from(name)
		};
		[path.clone(), sidecar("-wal"), sidecar("-shm")]
			.into_iter()
			.filter_map(|path| fs::metadata(path).ok().map(|metadata| metadata.len()))
			.sum()
	}

	pub fn local_data_counts(&self) -> Result<(i64, i64), rusqlite::Error> {
		self.connection()?.query_row(
			"SELECT (SELECT COUNT(*) FROM usage_snapshots), (SELECT COUNT(*) FROM run_logs)",
			[],
			|row| Ok((row.get(0)?, row.get(1)?)),
		)
	}

	pub fn clear_usage_history(&self) -> Result<(), rusqlite::Error> {
		let mut connection = self.connection()?;
		let transaction = connection.transaction()?;
		for table in [
			"usage_snapshots",
			"monthly_usage_snapshots",
			"openai_cost_ledger_entries",
			"openai_usage_ledger_entries",
		] {
			transaction.execute(&format!("DELETE FROM {table}"), [])?;
		}
		transaction.commit()
	}

	pub fn clear_refresh_logs(&self) -> Result<(), rusqlite::Error> {
		self.connection()?.execute("DELETE FROM run_logs", [])?;
		Ok(())
	}

	pub fn erase_all_local_data(&self) -> Result<(), rusqlite::Error> {
		let mut connection = self.connection()?;
		let transaction = connection.transaction()?;
		transaction.execute("DELETE FROM providers", [])?;
		transaction.execute("DELETE FROM accounts", [])?;
		transaction.commit()
	}
	pub fn open(path: &Path) -> Result<Self, rusqlite::Error> {
		if let Some(parent) = path.parent() {
			fs::create_dir_all(parent).map_err(|_| rusqlite::Error::InvalidPath(path.to_path_buf()))?;
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
		match connection.execute_batch(include_str!("../migrations/002_connections_and_onboarding.sql")) {
			Ok(()) => (),
			Err(error) if error.to_string().contains("duplicate column name") => (),
			Err(error) => return Err(error),
		};
		match connection.execute_batch(include_str!("../migrations/003_provider_refresh_status.sql")) {
			Ok(()) => (),
			Err(error) if error.to_string().contains("duplicate column name") => (),
			Err(error) => return Err(error),
		};
		match connection.execute_batch(include_str!("../migrations/005_provider_plan_quota.sql")) {
			Ok(()) => (),
			Err(error) if error.to_string().contains("duplicate column name") => (),
			Err(error) => return Err(error),
		};
		connection.execute_batch(include_str!("../migrations/006_clear_obsolete_resend_quota_error.sql"))?;
		match connection.execute_batch(include_str!("../migrations/007_resend_daily_quota.sql")) {
			Ok(()) => (),
			Err(error) if error.to_string().contains("duplicate column name") => (),
			Err(error) => return Err(error),
		};
		match connection.execute_batch(include_str!("../migrations/008_apify_monthly_credit_allowance.sql")) {
			Ok(()) => (),
			Err(error) if error.to_string().contains("duplicate column name") => (),
			Err(error) => return Err(error),
		};
		connection.execute_batch(include_str!("../migrations/009_openai_cost_ledger.sql"))?;
		connection.execute_batch(include_str!("../migrations/011_openai_credit_events.sql"))?;
		connection.execute_batch(include_str!("../migrations/012_openai_usage_ledger.sql"))?;
		connection.execute_batch(include_str!("../migrations/013_run_logs.sql"))?;
		connection.execute_batch(include_str!("../migrations/014_monthly_usage_snapshots.sql"))?;
		match connection.execute_batch(include_str!("../migrations/015_monthly_usage_metadata.sql")) {
			Ok(()) => (),
			Err(error) if error.to_string().contains("duplicate column name") => (),
			Err(error) => return Err(error),
		};
		match connection.execute_batch(include_str!("../migrations/016_monthly_usage_partial.sql")) {
			Ok(()) => (),
			Err(error) if error.to_string().contains("duplicate column name") => (),
			Err(error) => return Err(error),
		};
		Self::compact_provider_secret_names(&connection)
	}

	fn compact_provider_secret_names(connection: &Connection) -> Result<(), rusqlite::Error> {
		let mut statement = connection.prepare("SELECT id, secret_ref FROM providers")?;
		let entries = statement
			.query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))?
			.collect::<Result<Vec<_>, _>>()?;
		for (id, reference) in entries {
			let parts: Vec<_> = reference.split('/').collect();
			if parts.len() >= 4 && parts[0] == "projects" && parts[2] == "secrets" && !parts[3].is_empty() {
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
		let mut statement = connection.prepare("SELECT id, account_id, provider_type, display_name, secret_ref, last_error, plan, monthly_quota, daily_quota, apify_monthly_credit_allowance, (SELECT MIN(effective_at) FROM openai_credit_events WHERE provider_id = providers.id) FROM providers WHERE account_id = ?1 ORDER BY created_at DESC")?;
		statement
			.query_map([account_id], |row| {
				Ok(ProviderConfig {
					id: row.get(0)?,
					account_id: row.get(1)?,
					provider_type: row.get(2)?,
					display_name: row.get(3)?,
					secret_ref: row.get(4)?,
					last_error: row.get(5)?,
					plan: row.get(6)?,
					monthly_quota: row.get(7)?,
					daily_quota: row.get(8)?,
					apify_monthly_credit_allowance: row.get(9)?,
					openai_credit_start: row.get(10)?,
				})
			})?
			.collect()
	}

	pub fn find_provider(&self, id: &str) -> Result<Option<ProviderConfig>, rusqlite::Error> {
		self.connection()?.query_row("SELECT id, account_id, provider_type, display_name, secret_ref, last_error, plan, monthly_quota, daily_quota, apify_monthly_credit_allowance, (SELECT MIN(effective_at) FROM openai_credit_events WHERE provider_id = providers.id) FROM providers WHERE id = ?1", [id], |row| Ok(ProviderConfig { id: row.get(0)?, account_id: row.get(1)?, provider_type: row.get(2)?, display_name: row.get(3)?, secret_ref: row.get(4)?, last_error: row.get(5)?, plan: row.get(6)?, monthly_quota: row.get(7)?, daily_quota: row.get(8)?, apify_monthly_credit_allowance: row.get(9)?, openai_credit_start: row.get(10)? })).optional()
	}

	pub fn add_provider(&self, account_id: &str, input: NewProvider) -> Result<(), rusqlite::Error> {
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

	pub fn update_provider(&self, id: &str, account_id: &str, input: UpdateProvider) -> Result<(), rusqlite::Error> {
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

	pub fn list_accounts(&self) -> Result<Vec<Account>, rusqlite::Error> {
		let connection = self.connection()?;
		let mut statement = connection.prepare(
			"SELECT id, project_id, project_name, auth_status, auth_error FROM accounts ORDER BY created_at DESC",
		)?;
		statement
			.query_map([], |row| {
				Ok(Account {
					id: row.get(0)?,
					project_id: row.get(1)?,
					project_name: row.get(2)?,
					auth_status: row.get(3)?,
					auth_error: row.get(4)?,
				})
			})?
			.collect()
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
		connection.execute(
			"INSERT INTO accounts (id, project_id, is_active, created_at, updated_at) VALUES (?1, ?2, 1, ?3, ?3)",
			params![id, input.project_id, now],
		)?;
		connection.execute(
			"INSERT INTO onboarding (account_id, current_step) VALUES (?1, 1)",
			[&id],
		)?;
		self.active_account()?.ok_or(rusqlite::Error::QueryReturnedNoRows)
	}

	pub fn update_active_account(&self, input: UpdateAccount) -> Result<(), rusqlite::Error> {
		self.connection()?.execute(
			"UPDATE accounts SET project_id = ?1, project_name = NULL, updated_at = ?2 WHERE is_active = 1",
			params![input.project_id, now()],
		)?;
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

	pub fn snapshot_for_period(
		&self,
		provider_id: &str,
		period: crate::models::UsagePeriod,
	) -> Result<Option<UsageSnapshot>, rusqlite::Error> {
		self.connection()?.query_row("SELECT refreshed_at, status, cost, currency, metrics_json, period_end, metadata_json, is_partial FROM monthly_usage_snapshots WHERE provider_id = ?1 AND period_start = ?2", params![provider_id, period.start], |row| Ok(UsageSnapshot { provider_id: provider_id.to_owned(), timestamp: row.get(0)?, status: row.get(1)?, cost: row.get(2)?, currency: row.get(3)?, metrics: serde_json::from_str(&row.get::<_, String>(4)?).unwrap_or_default(), period: crate::models::UsagePeriod { start: period.start, end: row.get(5)?, is_current: period.is_current }, metadata: serde_json::from_str(&row.get::<_, String>(6)?).unwrap_or_default(), is_partial: row.get(7)?, openai_cost_ledger: None, openai_usage_ledger: None })).optional()
	}

	pub fn save_snapshot(&self, snapshot: &UsageSnapshot) -> Result<(), rusqlite::Error> {
		let metrics = serde_json::to_string(&snapshot.metrics).unwrap_or_else(|_| "[]".to_owned());
		let metadata = serde_json::to_string(&snapshot.metadata).unwrap_or_else(|_| "{}".to_owned());
		let mut connection = self.connection()?;
		let transaction = connection.transaction()?;
		transaction.execute("INSERT INTO usage_snapshots (provider_id, timestamp, status, cost, currency, raw_metrics_json) VALUES (?1, ?2, ?3, ?4, ?5, ?6)", params![snapshot.provider_id, snapshot.timestamp, snapshot.status, snapshot.cost, snapshot.currency, metrics])?;
		transaction.execute(
			"INSERT INTO monthly_usage_snapshots (provider_id, period_start, period_end, refreshed_at, status, cost, currency, metrics_json, metadata_json, is_partial) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10) ON CONFLICT(provider_id, period_start) DO UPDATE SET period_end = excluded.period_end, refreshed_at = excluded.refreshed_at, status = excluded.status, cost = excluded.cost, currency = excluded.currency, metrics_json = excluded.metrics_json, metadata_json = excluded.metadata_json, is_partial = excluded.is_partial",
			params![snapshot.provider_id, snapshot.period.start, snapshot.period.end, snapshot.timestamp, snapshot.status, snapshot.cost, snapshot.currency, metrics, metadata, snapshot.is_partial],
		)?;
		if let Some(ledger) = &snapshot.openai_cost_ledger {
			self.replace_openai_cost_ledger(&transaction, &snapshot.provider_id, ledger)?;
		}
		if let Some(ledger) = &snapshot.openai_usage_ledger {
			self.replace_openai_usage_ledger(&transaction, &snapshot.provider_id, ledger)?;
		}
		transaction.commit()?;
		Ok(())
	}

	pub fn save_run_log(&self, provider_id: &str, status: &str, message: &str) -> Result<(), rusqlite::Error> {
		self.connection()?.execute(
			"INSERT INTO run_logs (provider_id, created_at, status, message) VALUES (?1, ?2, ?3, ?4)",
			params![provider_id, now(), status, message],
		)?;
		Ok(())
	}

	pub fn list_run_logs(&self, account_id: &str) -> Result<Vec<RunLog>, rusqlite::Error> {
		let connection = self.connection()?;
		let mut statement = connection.prepare(
            "SELECT run_logs.id, providers.display_name, providers.provider_type, run_logs.created_at, run_logs.status, run_logs.message FROM run_logs INNER JOIN providers ON providers.id = run_logs.provider_id WHERE providers.account_id = ?1 ORDER BY run_logs.id DESC",
        )?;
		statement
			.query_map([account_id], |row| {
				Ok(RunLog {
					id: row.get(0)?,
					provider_name: row.get(1)?,
					provider_type: row.get(2)?,
					created_at: row.get(3)?,
					status: row.get(4)?,
					message: row.get(5)?,
				})
			})?
			.collect()
	}

	pub fn openai_cost_since(&self, provider_id: &str, start_time: i64) -> Result<f64, rusqlite::Error> {
		self.connection()?.query_row(
			"SELECT COALESCE(SUM(amount), 0) FROM openai_cost_ledger_entries WHERE provider_id = ?1 AND bucket_start >= ?2",
			params![provider_id, start_time],
			|row| row.get(0),
		)
	}

	#[allow(dead_code, reason = "backend query prepared for the upcoming dashboard UI")]
	pub fn openai_activity_summary(
		&self,
		provider_id: &str,
		start_time: i64,
		end_time: i64,
	) -> Result<OpenAiActivitySummary, rusqlite::Error> {
		let (input_tokens, cached_input_tokens, output_tokens, request_count) = self
            .connection()?
            .query_row(
                "SELECT COALESCE(SUM(input_tokens), 0), COALESCE(SUM(cached_input_tokens), 0), COALESCE(SUM(output_tokens), 0), COALESCE(SUM(request_count), 0) FROM openai_usage_ledger_entries WHERE provider_id = ?1 AND bucket_start >= ?2 AND bucket_start < ?3",
                params![provider_id, start_time, end_time],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )?;
		let cache_hit_rate = (input_tokens > 0).then(|| cached_input_tokens as f64 / input_tokens as f64 * 100.0);
		Ok(OpenAiActivitySummary {
			input_tokens,
			cached_input_tokens,
			output_tokens,
			request_count,
			cache_hit_rate,
		})
	}

	#[allow(dead_code, reason = "backend query prepared for the upcoming dashboard UI")]
	pub fn openai_daily_activity(
		&self,
		provider_id: &str,
		start_time: i64,
		end_time: i64,
	) -> Result<Vec<OpenAiDailyActivity>, rusqlite::Error> {
		let connection = self.connection()?;
		let mut statement = connection.prepare("SELECT bucket_start, SUM(input_tokens), SUM(cached_input_tokens), SUM(output_tokens), SUM(request_count) FROM openai_usage_ledger_entries WHERE provider_id = ?1 AND bucket_start >= ?2 AND bucket_start < ?3 GROUP BY bucket_start ORDER BY bucket_start")?;
		statement
			.query_map(params![provider_id, start_time, end_time], |row| {
				Ok(OpenAiDailyActivity {
					bucket_start: row.get(0)?,
					input_tokens: row.get(1)?,
					cached_input_tokens: row.get(2)?,
					output_tokens: row.get(3)?,
					request_count: row.get(4)?,
				})
			})?
			.collect()
	}

	#[allow(dead_code, reason = "backend query prepared for the upcoming dashboard UI")]
	pub fn openai_daily_spend(
		&self,
		provider_id: &str,
		start_time: i64,
		end_time: i64,
	) -> Result<Vec<OpenAiDailySpend>, rusqlite::Error> {
		let connection = self.connection()?;
		let mut statement = connection.prepare("SELECT bucket_start, SUM(amount), currency FROM openai_cost_ledger_entries WHERE provider_id = ?1 AND bucket_start >= ?2 AND bucket_start < ?3 GROUP BY bucket_start, currency ORDER BY bucket_start")?;
		statement
			.query_map(params![provider_id, start_time, end_time], |row| {
				Ok(OpenAiDailySpend {
					bucket_start: row.get(0)?,
					amount: row.get(1)?,
					currency: row.get(2)?,
				})
			})?
			.collect()
	}

	#[allow(dead_code, reason = "backend query prepared for the upcoming dashboard UI")]
	pub fn openai_project_spend(
		&self,
		provider_id: &str,
		start_time: i64,
		end_time: i64,
	) -> Result<Vec<OpenAiProjectSpend>, rusqlite::Error> {
		let connection = self.connection()?;
		let mut statement = connection.prepare("SELECT project_id, SUM(amount), currency FROM openai_cost_ledger_entries WHERE provider_id = ?1 AND bucket_start >= ?2 AND bucket_start < ?3 GROUP BY project_id, currency ORDER BY SUM(amount) DESC")?;
		statement
			.query_map(params![provider_id, start_time, end_time], |row| {
				Ok(OpenAiProjectSpend {
					project_id: row.get(0)?,
					amount: row.get(1)?,
					currency: row.get(2)?,
				})
			})?
			.collect()
	}

	pub fn list_openai_credit_events(&self, provider_id: &str) -> Result<Vec<OpenAiCreditEvent>, rusqlite::Error> {
		let connection = self.connection()?;
		let mut statement = connection.prepare("SELECT id, event_type, effective_at, amount, note FROM openai_credit_events WHERE provider_id = ?1 ORDER BY effective_at, id")?;
		statement
			.query_map([provider_id], |row| {
				let effective_at: i64 = row.get(2)?;
				let date = chrono::DateTime::from_timestamp(effective_at, 0)
					.map(|value| value.date_naive().to_string())
					.unwrap_or_default();
				Ok(OpenAiCreditEvent {
					id: row.get(0)?,
					event_type: row.get(1)?,
					date,
					amount: row.get(3)?,
					note: row.get(4)?,
				})
			})?
			.collect()
	}

	pub fn add_openai_credit_event(
		&self,
		provider_id: &str,
		event_type: &str,
		effective_at: i64,
		amount: f64,
		note: &str,
	) -> Result<(), rusqlite::Error> {
		self.connection()?.execute(
			"INSERT INTO openai_credit_events (provider_id, event_type, effective_at, amount, note) VALUES (?1, ?2, ?3, ?4, ?5)",
			params![provider_id, event_type, effective_at, amount, note],
		)?;
		Ok(())
	}

	pub fn delete_openai_credit_event(&self, provider_id: &str, event_id: i64) -> Result<(), rusqlite::Error> {
		self.connection()?.execute(
			"DELETE FROM openai_credit_events WHERE id = ?1 AND provider_id = ?2",
			params![event_id, provider_id],
		)?;
		Ok(())
	}

	pub fn openai_credit_totals(&self, provider_id: &str) -> Result<Option<(f64, f64)>, rusqlite::Error> {
		let connection = self.connection()?;
		let start: Option<i64> = connection.query_row(
			"SELECT MIN(effective_at) FROM openai_credit_events WHERE provider_id = ?1",
			[provider_id],
			|row| row.get(0),
		)?;
		let Some(start) = start else {
			return Ok(None);
		};
		let credit = connection.query_row("SELECT COALESCE(SUM(CASE WHEN event_type = 'purchase' THEN amount ELSE -amount END), 0) FROM openai_credit_events WHERE provider_id = ?1", [provider_id], |row| row.get(0))?;
		let spent = self.openai_cost_since(provider_id, start)?;
		Ok(Some((credit, spent)))
	}

	fn replace_openai_cost_ledger(
		&self,
		transaction: &rusqlite::Transaction<'_>,
		provider_id: &str,
		ledger: &OpenAiCostLedger,
	) -> Result<(), rusqlite::Error> {
		transaction.execute(
			"DELETE FROM openai_cost_ledger_entries WHERE provider_id = ?1 AND bucket_start >= ?2 AND bucket_start < ?3",
			params![provider_id, ledger.start_time, ledger.end_time],
		)?;
		let mut statement = transaction.prepare("INSERT INTO openai_cost_ledger_entries (provider_id, bucket_start, bucket_end, amount, currency, project_id, api_key_id, line_item) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)")?;
		for entry in &ledger.entries {
			statement.execute(params![
				provider_id,
				entry.bucket_start,
				entry.bucket_end,
				entry.amount,
				entry.currency,
				entry.project_id,
				entry.api_key_id,
				entry.line_item
			])?;
		}
		drop(statement);
		Ok(())
	}

	fn replace_openai_usage_ledger(
		&self,
		transaction: &rusqlite::Transaction<'_>,
		provider_id: &str,
		ledger: &OpenAiUsageLedger,
	) -> Result<(), rusqlite::Error> {
		transaction.execute(
			"DELETE FROM openai_usage_ledger_entries WHERE provider_id = ?1 AND bucket_start >= ?2 AND bucket_start < ?3",
			params![provider_id, ledger.start_time, ledger.end_time],
		)?;
		let mut statement = transaction.prepare("INSERT INTO openai_usage_ledger_entries (provider_id, bucket_start, bucket_end, input_tokens, cached_input_tokens, output_tokens, request_count, project_id, model) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)")?;
		for entry in &ledger.entries {
			statement.execute(params![
				provider_id,
				entry.bucket_start,
				entry.bucket_end,
				entry.input_tokens,
				entry.cached_input_tokens,
				entry.output_tokens,
				entry.request_count,
				entry.project_id,
				entry.model,
			])?;
		}
		drop(statement);
		Ok(())
	}

	pub fn set_provider_refresh_error(&self, id: &str, error: Option<&str>) -> Result<(), rusqlite::Error> {
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

#[cfg(test)]
mod tests {
	use super::*;
	use crate::models::{NewProvider, OpenAiCostLedgerEntry, OpenAiUsageLedgerEntry, UsageSnapshot};

	fn test_database() -> (Database, std::path::PathBuf, String) {
		static NEXT_TEST_DATABASE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
		let unique = std::time::SystemTime::now()
			.duration_since(std::time::UNIX_EPOCH)
			.unwrap_or_default()
			.as_nanos();
		let sequence = NEXT_TEST_DATABASE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
		let path = std::env::temp_dir().join(format!(
			"clearquota-database-{}-{unique}-{sequence}.sqlite",
			std::process::id()
		));
		let database = Database::open(&path).unwrap();
		database.migrate().unwrap();
		let account = database
			.create_account(NewAccount {
				project_id: "gcp-test".to_owned(),
			})
			.unwrap();
		database
			.add_provider(
				&account.id,
				NewProvider {
					provider_type: "openai".to_owned(),
					display_name: "OpenAI".to_owned(),
					secret_ref: "OPENAI_ADMIN_KEY".to_owned(),
					plan: None,
					monthly_quota: None,
					daily_quota: None,
					apify_monthly_credit_allowance: None,
				},
			)
			.unwrap();
		let provider_id = database.list_providers(&account.id).unwrap()[0].id.clone();
		(database, path, provider_id)
	}

	#[test]
	fn local_data_controls_keep_and_remove_the_expected_records() {
		let (database, path, provider_id) = test_database();
		database.save_run_log(&provider_id, "succeeded", "Refresh completed.").unwrap();
		database
			.save_snapshot(&UsageSnapshot {
				provider_id: provider_id.clone(),
				timestamp: "200".to_owned(),
				status: "ok".to_owned(),
				cost: Some(1.0),
				currency: Some("usd".to_owned()),
				metrics: vec![],
				period: crate::models::UsagePeriod {
					start: 100,
					end: 201,
					is_current: false,
				},
				is_partial: false,
				metadata: serde_json::json!({}),
				openai_cost_ledger: None,
				openai_usage_ledger: None,
			})
			.unwrap();
		assert_eq!(database.local_data_counts().unwrap(), (1, 1));
		database.clear_usage_history().unwrap();
		assert_eq!(database.local_data_counts().unwrap(), (0, 1));
		assert!(database.find_provider(&provider_id).unwrap().is_some());
		database.clear_refresh_logs().unwrap();
		assert_eq!(database.local_data_counts().unwrap(), (0, 0));
		database.erase_all_local_data().unwrap();
		assert!(database.active_account().unwrap().is_none());
		assert!(database.find_provider(&provider_id).unwrap().is_none());
		std::fs::remove_file(path).ok();
	}

	#[test]
	fn stores_and_aggregates_openai_activity_and_project_spend() {
		let (database, path, provider_id) = test_database();
		database
			.save_snapshot(&UsageSnapshot {
				provider_id: provider_id.clone(),
				timestamp: "200".to_owned(),
				status: "ok".to_owned(),
				cost: Some(3.5),
				currency: Some("usd".to_owned()),
				metrics: vec![],
				period: crate::models::UsagePeriod {
					start: 100,
					end: 201,
					is_current: false,
				},
				is_partial: false,
				metadata: serde_json::json!({}),
				openai_cost_ledger: Some(OpenAiCostLedger {
					start_time: 100,
					end_time: 200,
					entries: vec![
						OpenAiCostLedgerEntry {
							bucket_start: 100,
							bucket_end: 200,
							amount: 2.5,
							currency: "usd".to_owned(),
							project_id: Some("proj_a".to_owned()),
							api_key_id: None,
							line_item: Some("completions".to_owned()),
						},
						OpenAiCostLedgerEntry {
							bucket_start: 100,
							bucket_end: 200,
							amount: 1.0,
							currency: "usd".to_owned(),
							project_id: None,
							api_key_id: None,
							line_item: Some("completions".to_owned()),
						},
					],
				}),
				openai_usage_ledger: Some(OpenAiUsageLedger {
					start_time: 100,
					end_time: 200,
					entries: vec![
						OpenAiUsageLedgerEntry {
							bucket_start: 100,
							bucket_end: 200,
							input_tokens: 800,
							cached_input_tokens: 400,
							output_tokens: 200,
							request_count: 3,
							project_id: Some("proj_a".to_owned()),
							model: Some("gpt-test".to_owned()),
						},
						OpenAiUsageLedgerEntry {
							bucket_start: 100,
							bucket_end: 200,
							input_tokens: 200,
							cached_input_tokens: 100,
							output_tokens: 50,
							request_count: 2,
							project_id: None,
							model: Some("gpt-test".to_owned()),
						},
					],
				}),
			})
			.unwrap();

		assert_eq!(
			database.openai_activity_summary(&provider_id, 100, 201).unwrap(),
			OpenAiActivitySummary {
				input_tokens: 1000,
				cached_input_tokens: 500,
				output_tokens: 250,
				request_count: 5,
				cache_hit_rate: Some(50.0),
			}
		);
		assert_eq!(
			database.openai_daily_activity(&provider_id, 100, 201).unwrap(),
			vec![OpenAiDailyActivity {
				bucket_start: 100,
				input_tokens: 1000,
				cached_input_tokens: 500,
				output_tokens: 250,
				request_count: 5,
			}]
		);
		assert_eq!(
			database.openai_project_spend(&provider_id, 100, 201).unwrap(),
			vec![
				OpenAiProjectSpend {
					project_id: Some("proj_a".to_owned()),
					amount: 2.5,
					currency: "usd".to_owned(),
				},
				OpenAiProjectSpend {
					project_id: None,
					amount: 1.0,
					currency: "usd".to_owned(),
				},
			]
		);
		assert_eq!(
			database.openai_daily_spend(&provider_id, 100, 201).unwrap(),
			vec![OpenAiDailySpend {
				bucket_start: 100,
				amount: 3.5,
				currency: "usd".to_owned(),
			}]
		);

		drop(database);
		for database_path in [
			path.clone(),
			path.with_extension("sqlite-shm"),
			path.with_extension("sqlite-wal"),
		] {
			let _ = std::fs::remove_file(database_path);
		}
	}

	#[test]
	fn stores_run_logs_for_the_active_accounts_providers() {
		let (database, path, provider_id) = test_database();
		database
			.save_run_log(&provider_id, "succeeded", "Usage refresh completed successfully.")
			.unwrap();

		let account_id = database.active_account().unwrap().unwrap().id;
		let logs = database.list_run_logs(&account_id).unwrap();
		assert_eq!(logs.len(), 1);
		assert_eq!(logs[0].provider_name, "OpenAI");
		assert_eq!(logs[0].provider_type, "openai");
		assert_eq!(logs[0].status, "succeeded");
		assert_eq!(logs[0].message, "Usage refresh completed successfully.");

		std::fs::remove_file(path).ok();
	}

	#[test]
	fn upserts_one_month_without_removing_older_months() {
		let (database, path, provider_id) = test_database();
		let save = |start, end, cost| {
			database
				.save_snapshot(&UsageSnapshot {
					provider_id: provider_id.clone(),
					timestamp: format!("{end}"),
					status: "ok".to_owned(),
					cost: Some(cost),
					currency: Some("usd".to_owned()),
					metrics: vec![],
					period: crate::models::UsagePeriod {
						start,
						end,
						is_current: false,
					},
					is_partial: cost == 1.5,
					metadata: serde_json::json!({}),
					openai_cost_ledger: None,
					openai_usage_ledger: None,
				})
				.unwrap();
		};
		save(100, 200, 1.0);
		save(200, 300, 2.0);
		save(100, 200, 1.5);

		for (start, end, expected, is_partial) in [(100, 200, 1.5, true), (200, 300, 2.0, false)] {
			let snapshot = database
				.snapshot_for_period(
					&provider_id,
					crate::models::UsagePeriod {
						start,
						end,
						is_current: false,
					},
				)
				.unwrap()
				.unwrap();
			assert_eq!(snapshot.cost, Some(expected));
			assert_eq!(snapshot.is_partial, is_partial);
		}

		database
			.connection()
			.unwrap()
			.execute("DELETE FROM monthly_usage_snapshots", [])
			.unwrap();
		database.migrate().unwrap();
		let backfilled = database
			.snapshot_for_period(
				&provider_id,
				crate::models::UsagePeriod {
					start: 0,
					end: 2_678_400,
					is_current: false,
				},
			)
			.unwrap()
			.unwrap();
		assert_eq!(backfilled.cost, Some(2.0));
		assert!(backfilled.is_partial);

		std::fs::remove_file(path).ok();
	}
}
