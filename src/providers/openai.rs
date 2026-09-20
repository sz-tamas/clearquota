use async_trait::async_trait;
use chrono::{Datelike, Utc};
use secrecy::{ExposeSecret, SecretString};
use serde_json::Value;

use crate::models::{Metric, ProviderConfig, UsageSnapshot};

use super::{Provider, ProviderError};

pub struct OpenAiProvider;

#[async_trait]
impl Provider for OpenAiProvider {
    async fn collect(
        &self,
        config: &ProviderConfig,
        secret: &SecretString,
    ) -> Result<UsageSnapshot, ProviderError> {
        let now = Utc::now();
        let start = now
            .date_naive()
            .with_day(1)
            .ok_or(ProviderError::InvalidResponse)?
            .and_hms_opt(0, 0, 0)
            .ok_or(ProviderError::InvalidResponse)?
            .and_utc()
            .timestamp();
        let client = reqwest::Client::new();
        let costs = fetch_costs(&client, secret, start, now.timestamp());
        let alerts = fetch_spend_alerts(&client, secret);
        let (costs, alert) = tokio::join!(costs, alerts);
        let costs = costs?;
        let (limit, status) = match alert {
            Ok(limit) => (Some(limit), "ok"),
            Err(error) => {
                eprintln!(
                    "event=openai_usage_partially_collected unavailable=/organization/spend_alerts error={error}"
                );
                (None, "partial")
            }
        };
        eprintln!(
            "event=openai_usage_collected costs_endpoint=/organization/costs spend_alerts_endpoint=/organization/spend_alerts currency={} current_month_spend={:.2} monthly_limit_available={}",
            costs.currency,
            costs.amount,
            limit.is_some()
        );

        Ok(UsageSnapshot {
            provider_id: config.id.clone(),
            timestamp: unix_timestamp(),
            status: status.to_owned(),
            cost: Some(costs.amount),
            currency: Some(costs.currency.clone()),
            metrics: vec![Metric {
                id: "organization_spend_limit".to_owned(),
                label: "Organization spend limit".to_owned(),
                used: costs.amount,
                limit,
                unit: costs.currency.to_uppercase(),
            }],
        })
    }
}

struct Costs {
    amount: f64,
    currency: String,
}

async fn fetch_costs(
    client: &reqwest::Client,
    secret: &SecretString,
    start_time: i64,
    end_time: i64,
) -> Result<Costs, ProviderError> {
    let payload = get_json(
        client,
        secret,
        "https://api.openai.com/v1/organization/costs",
        &[
            ("start_time", start_time.to_string()),
            ("end_time", end_time.to_string()),
            ("bucket_width", "1d".to_owned()),
            ("limit", "31".to_owned()),
        ],
    )
    .await?;
    parse_costs(&payload).map_err(|_| {
        eprintln!("event=openai_usage_response_invalid endpoint=/organization/costs");
        ProviderError::OpenAiCostsInvalidResponse
    })
}

async fn fetch_spend_alerts(
    client: &reqwest::Client,
    secret: &SecretString,
) -> Result<f64, ProviderError> {
    let payload = get_json(
        client,
        secret,
        "https://api.openai.com/v1/organization/spend_alerts",
        &[],
    )
    .await?;
    parse_monthly_limit(&payload).map_err(|_| {
        eprintln!("event=openai_usage_response_invalid endpoint=/organization/spend_alerts");
        ProviderError::OpenAiSpendAlertInvalidResponse
    })
}

async fn get_json(
    client: &reqwest::Client,
    secret: &SecretString,
    url: &str,
    query: &[(&'static str, String)],
) -> Result<Value, ProviderError> {
    let response = client
        .get(url)
        .query(query)
        .bearer_auth(secret.expose_secret())
        .header("User-Agent", "rust-usage-dash/0.1")
        .send()
        .await
        .map_err(|_| ProviderError::Request)?;
    if !response.status().is_success() {
        eprintln!(
            "event=openai_usage_request_failed endpoint={} status={}",
            url_path(url),
            response.status().as_u16()
        );
        return Err(ProviderError::Request);
    }
    let payload = response
        .json()
        .await
        .map_err(|_| ProviderError::InvalidResponse)?;
    eprintln!(
        "event=openai_usage_response_received endpoint={} status=200",
        url_path(url)
    );
    Ok(payload)
}

fn url_path(url: &str) -> &str {
    url.strip_prefix("https://api.openai.com/v1").unwrap_or(url)
}

fn parse_costs(payload: &Value) -> Result<Costs, ProviderError> {
    let mut amount = 0.0;
    let mut currency = None;
    let buckets = payload
        .get("data")
        .and_then(Value::as_array)
        .ok_or(ProviderError::InvalidResponse)?;
    for result in buckets
        .iter()
        .filter_map(|bucket| bucket.get("results").and_then(Value::as_array))
        .flatten()
    {
        let item = result.get("amount").ok_or(ProviderError::InvalidResponse)?;
        amount += item
            .get("value")
            .and_then(Value::as_f64)
            .ok_or(ProviderError::InvalidResponse)?;
        let item_currency = item
            .get("currency")
            .and_then(Value::as_str)
            .ok_or(ProviderError::InvalidResponse)?;
        match &currency {
            Some(existing) if existing != item_currency => {
                return Err(ProviderError::InvalidResponse);
            }
            Some(_) => (),
            None => currency = Some(item_currency.to_owned()),
        }
    }
    Ok(Costs {
        amount,
        currency: currency.unwrap_or_else(|| "usd".to_owned()),
    })
}

fn parse_monthly_limit(payload: &Value) -> Result<f64, ProviderError> {
    let alerts = payload
        .get("data")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_else(|| std::slice::from_ref(payload));
    alerts
        .iter()
        .filter(|alert| {
            alert
                .get("interval")
                .and_then(Value::as_str)
                .is_none_or(|value| value == "month")
        })
        .filter_map(|alert| alert.get("threshold_amount").and_then(Value::as_f64))
        .filter(|amount| amount.is_finite() && *amount >= 0.0)
        .max_by(|left, right| left.total_cmp(right))
        .map(|cents| cents / 100.0)
        .ok_or(ProviderError::InvalidResponse)
}

fn unix_timestamp() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn sums_daily_costs() {
        let costs = parse_costs(&json!({"data":[
            {"results":[{"amount":{"value":8.0,"currency":"usd"}}]},
            {"results":[{"amount":{"value":0.92,"currency":"usd"}}]}
        ]}))
        .unwrap();
        assert_eq!(costs.amount, 8.92);
        assert_eq!(costs.currency, "usd");
    }

    #[test]
    fn selects_the_largest_monthly_alert_and_converts_cents() {
        let limit = parse_monthly_limit(&json!({"data":[
            {"interval":"month","threshold_amount":960},
            {"interval":"month","threshold_amount":1200,"currency":"USD"}
        ]}))
        .unwrap();
        assert_eq!(limit, 12.0);
    }

    #[test]
    fn ignores_non_monthly_alerts() {
        let limit = parse_monthly_limit(&json!({"data":[
            {"interval":"day","threshold_amount":9999},
            {"interval":"month","threshold_amount":1200}
        ]}))
        .unwrap();
        assert_eq!(limit, 12.0);
    }
}
