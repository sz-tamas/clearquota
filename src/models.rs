use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct ProviderConfig {
    pub id: String,
    pub account_id: String,
    pub provider_type: String,
    pub display_name: String,
    pub secret_ref: String,
    pub enabled: bool,
    pub last_error: Option<String>,
    pub plan: String,
    pub monthly_quota: i64,
    pub daily_quota: i64,
    pub apify_monthly_credit_allowance: f64,
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
