use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct ProviderConfig {
	pub id: String,
	pub account_id: String,
	pub provider_type: String,
	pub display_name: String,
	pub secret_ref: String,
	pub last_error: Option<String>,
	pub plan: String,
	pub monthly_quota: i64,
	pub daily_quota: i64,
	pub apify_monthly_credit_allowance: f64,
	pub openai_credit_start: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metric {
	pub id: String,
	pub label: String,
	pub used: f64,
	pub limit: Option<f64>,
	pub unit: String,
}

#[derive(Debug, Clone)]
pub struct UsageSnapshot {
	pub provider_id: String,
	pub timestamp: String,
	pub status: String,
	pub cost: Option<f64>,
	pub currency: Option<String>,
	pub metrics: Vec<Metric>,
	/// Sanitized OpenAI cost records to persist apart from the aggregate snapshot.
	pub openai_cost_ledger: Option<OpenAiCostLedger>,
	/// Sanitized OpenAI activity records to persist apart from the aggregate snapshot.
	pub openai_usage_ledger: Option<OpenAiUsageLedger>,
}

/// A sanitized record of one provider refresh attempt.
#[derive(Debug, Clone, PartialEq)]
pub struct RunLog {
	pub id: i64,
	pub provider_name: String,
	pub provider_type: String,
	pub created_at: String,
	pub status: String,
	pub message: String,
}

#[derive(Debug, Clone)]
pub struct OpenAiUsageLedger {
	pub start_time: i64,
	pub end_time: i64,
	pub entries: Vec<OpenAiUsageLedgerEntry>,
}

#[derive(Debug, Clone)]
pub struct OpenAiUsageLedgerEntry {
	pub bucket_start: i64,
	pub bucket_end: i64,
	pub input_tokens: i64,
	pub cached_input_tokens: i64,
	pub output_tokens: i64,
	pub request_count: i64,
	pub project_id: Option<String>,
	pub model: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code, reason = "backend read model for the upcoming dashboard UI")]
pub struct OpenAiActivitySummary {
	pub input_tokens: i64,
	pub cached_input_tokens: i64,
	pub output_tokens: i64,
	pub request_count: i64,
	pub cache_hit_rate: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code, reason = "backend read model for the upcoming dashboard UI")]
pub struct OpenAiDailyActivity {
	pub bucket_start: i64,
	pub input_tokens: i64,
	pub cached_input_tokens: i64,
	pub output_tokens: i64,
	pub request_count: i64,
}

#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code, reason = "backend read model for the upcoming dashboard UI")]
pub struct OpenAiDailySpend {
	pub bucket_start: i64,
	pub amount: f64,
	pub currency: String,
}

#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code, reason = "backend read model for the upcoming dashboard UI")]
pub struct OpenAiProjectSpend {
	pub project_id: Option<String>,
	pub amount: f64,
	pub currency: String,
}

#[derive(Debug, Clone)]
pub struct OpenAiCostLedger {
	pub start_time: i64,
	pub end_time: i64,
	pub entries: Vec<OpenAiCostLedgerEntry>,
}

#[derive(Debug, Clone)]
pub struct OpenAiCostLedgerEntry {
	pub bucket_start: i64,
	pub bucket_end: i64,
	pub amount: f64,
	pub currency: String,
	pub project_id: Option<String>,
	pub api_key_id: Option<String>,
	pub line_item: Option<String>,
}

#[derive(Debug, Clone)]
pub struct OpenAiCreditEvent {
	pub id: i64,
	pub event_type: String,
	pub date: String,
	pub amount: f64,
	pub note: String,
}

#[derive(Deserialize)]
pub struct NewOpenAiCreditEvent {
	pub event_type: String,
	pub date: String,
	pub amount: f64,
	pub note: Option<String>,
}

#[derive(Deserialize)]
pub struct NewProvider {
	pub provider_type: String,
	pub display_name: String,
	pub secret_ref: String,
	pub plan: Option<String>,
	pub monthly_quota: Option<i64>,
	pub daily_quota: Option<i64>,
	pub apify_monthly_credit_allowance: Option<f64>,
}

#[derive(Deserialize)]
pub struct UpdateProvider {
	pub display_name: String,
	pub secret_ref: String,
	pub plan: Option<String>,
	pub monthly_quota: Option<i64>,
	pub daily_quota: Option<i64>,
	pub apify_monthly_credit_allowance: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct Account {
	pub id: String,
	pub project_id: String,
	pub project_name: Option<String>,
	pub auth_status: String,
	pub auth_error: Option<String>,
}

#[derive(Deserialize)]
pub struct NewAccount {
	pub project_id: String,
}

#[derive(Deserialize)]
pub struct UpdateAccount {
	pub project_id: String,
}
