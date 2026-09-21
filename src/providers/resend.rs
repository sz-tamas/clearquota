use super::{Provider, ProviderError};
use crate::models::{Metric, ProviderConfig, UsageSnapshot};
use async_trait::async_trait;
use chrono::{Datelike, Utc};
use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;
pub struct ResendProvider;
#[derive(Deserialize)]
struct EmailMetricsResponse {
    totals: EmailMetricsTotals,
}
#[derive(Deserialize)]
struct EmailMetricsTotals {
    sent: Option<f64>,
    received: Option<f64>,
    delivered: Option<f64>,
    bounced: Option<f64>,
    complained: Option<f64>,
    failed: Option<f64>,
}
#[derive(Deserialize)]
struct ResendErrorResponse {
    name: Option<String>,
}
#[async_trait]
impl Provider for ResendProvider {
    async fn collect(
        &self,
        config: &ProviderConfig,
        secret: &SecretString,
    ) -> Result<UsageSnapshot, ProviderError> {
        let end = Utc::now().date_naive();
        let start = end.with_day(1).ok_or(ProviderError::InvalidResponse)?;
        let client = reqwest::Client::new();
        let monthly = fetch(
            &client,
            secret,
            start.to_string(),
            end.to_string(),
            "sent,received,delivered,bounced,complained,failed",
        );
        let daily = fetch(
            &client,
            secret,
            end.to_string(),
            end.to_string(),
            "sent,received",
        );
        let (monthly, daily) = tokio::try_join!(monthly, daily)?;
        let mut metrics = Vec::new();
        add(
            &mut metrics,
            "emails_sent_current_month",
            "Emails sent (current month)",
            monthly.totals.sent,
        );
        add(
            &mut metrics,
            "emails_received_current_month",
            "Emails received (current month)",
            monthly.totals.received,
        );
        add(
            &mut metrics,
            "emails_delivered_current_month",
            "Emails delivered (current month)",
            monthly.totals.delivered,
        );
        add(
            &mut metrics,
            "emails_bounced_current_month",
            "Emails bounced (current month)",
            monthly.totals.bounced,
        );
        add(
            &mut metrics,
            "emails_complained_current_month",
            "Spam complaints (current month)",
            monthly.totals.complained,
        );
        add(
            &mut metrics,
            "emails_failed_current_month",
            "Emails failed (current month)",
            monthly.totals.failed,
        );
        add(
            &mut metrics,
            "emails_sent_today",
            "Emails sent (today)",
            daily.totals.sent,
        );
        add(
            &mut metrics,
            "emails_received_today",
            "Emails received (today)",
            daily.totals.received,
        );
        quota(
            &mut metrics,
            "monthly",
            monthly.totals.sent.unwrap_or(0.0) + monthly.totals.received.unwrap_or(0.0),
            config.monthly_quota,
            &config.plan,
        );
        quota(
            &mut metrics,
            "daily",
            daily.totals.sent.unwrap_or(0.0) + daily.totals.received.unwrap_or(0.0),
            config.daily_quota,
            &config.plan,
        );
        Ok(UsageSnapshot {
            provider_id: config.id.clone(),
            timestamp: unix_timestamp(),
            status: "ok".to_owned(),
            cost: None,
            currency: None,
            metrics,
            openai_cost_ledger: None,
            openai_usage_ledger: None,
        })
    }
}
async fn fetch(
    client: &reqwest::Client,
    secret: &SecretString,
    start: String,
    end: String,
    kinds: &str,
) -> Result<EmailMetricsResponse, ProviderError> {
    let response = client
        .get("https://api.resend.com/emails/metrics")
        .query(&[
            ("start_date", start.clone()),
            ("end_date", end.clone()),
            ("metrics", kinds.to_owned()),
        ])
        .bearer_auth(secret.expose_secret())
        .header("User-Agent", "clearquota/0.1")
        .send()
        .await
        .map_err(|_| ProviderError::Request)?;
    if !response.status().is_success() {
        let status = response.status();
        let name = response
            .json::<ResendErrorResponse>()
            .await
            .ok()
            .and_then(|x| x.name);
        if status == reqwest::StatusCode::UNAUTHORIZED
            && name.as_deref() == Some("restricted_api_key")
        {
            return Err(ProviderError::ResendSendingAccessOnly);
        }
        return Err(ProviderError::Request);
    }
    eprintln!(
        "event=resend_usage_collected endpoint=/emails/metrics status={} metrics={} start_date={} end_date={}",
        response.status().as_u16(),
        kinds,
        start,
        end
    );
    response
        .json()
        .await
        .map_err(|_| ProviderError::InvalidResponse)
}
fn add(metrics: &mut Vec<Metric>, id: &str, label: &str, value: Option<f64>) {
    if let Some(used) = value {
        metrics.push(Metric {
            id: id.to_owned(),
            label: label.to_owned(),
            used,
            limit: None,
            unit: "emails".to_owned(),
        })
    }
}
fn quota(metrics: &mut Vec<Metric>, period: &str, used: f64, limit: i64, plan: &str) {
    let limit = limit as f64;
    metrics.push(Metric {
        id: format!("{}_email_quota", period),
        label: format!("{} quota usage ({})", period, plan),
        used,
        limit: Some(limit),
        unit: "emails".to_owned(),
    });
    metrics.push(Metric {
        id: format!("{}_email_quota_remaining", period),
        label: format!("{} quota remaining", period),
        used: (limit as i64).saturating_sub(used as i64) as f64,
        limit: None,
        unit: "emails".to_owned(),
    });
    metrics.push(Metric {
        id: format!("{}_email_quota_used_percent", period),
        label: format!("{} quota used", period),
        used: used / limit * 100.0,
        limit: None,
        unit: "%".to_owned(),
    })
}
fn unix_timestamp() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .to_string()
}
