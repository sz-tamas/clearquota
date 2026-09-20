use async_trait::async_trait;
use chrono::{Datelike, Utc};
use secrecy::{ExposeSecret, SecretString};
use serde_json::Value;

use crate::models::{
    Metric, OpenAiCostLedger, OpenAiCostLedgerEntry, OpenAiUsageLedger, OpenAiUsageLedgerEntry,
    ProviderConfig, UsageSnapshot,
};

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
        let month_start = now
            .date_naive()
            .with_day(1)
            .ok_or(ProviderError::InvalidResponse)?
            .and_hms_opt(0, 0, 0)
            .ok_or(ProviderError::InvalidResponse)?
            .and_utc()
            .timestamp();
        let ledger_start = config
            .openai_credit_start
            .unwrap_or(month_start)
            .min(month_start);
        let client = reqwest::Client::new();
        let costs = fetch_costs(&client, secret, ledger_start, now.timestamp());
        let alerts = fetch_spend_alerts(&client, secret);
        let usage = fetch_completions_usage(&client, secret, month_start, now.timestamp());
        let (costs, alert, usage) = tokio::join!(costs, alerts, usage);
        let costs = costs?;
        let mut status = "ok";
        let limit = match alert {
            Ok(limit) => Some(limit),
            Err(error) => {
                status = "partial";
                eprintln!(
                    "event=openai_usage_partially_collected unavailable=/organization/spend_alerts error={error}"
                );
                None
            }
        };
        let usage = match usage {
            Ok(usage) => Some(usage),
            Err(error) => {
                status = "partial";
                eprintln!(
                    "event=openai_usage_partially_collected unavailable=/organization/usage/completions error={error}"
                );
                None
            }
        };
        let monthly_amount: f64 = costs
            .entries
            .iter()
            .filter(|entry| entry.bucket_start >= month_start)
            .map(|entry| entry.amount)
            .sum();
        eprintln!(
            "event=openai_usage_collected costs_endpoint=/organization/costs spend_alerts_endpoint=/organization/spend_alerts currency={} current_month_spend={:.2} monthly_limit_available={}",
            costs.currency,
            monthly_amount,
            limit.is_some()
        );

        Ok(UsageSnapshot {
            provider_id: config.id.clone(),
            timestamp: unix_timestamp(),
            status: status.to_owned(),
            cost: Some(monthly_amount),
            currency: Some(costs.currency.clone()),
            metrics: openai_metrics(monthly_amount, limit, &costs.currency, usage.as_deref()),
            openai_cost_ledger: Some(OpenAiCostLedger {
                start_time: ledger_start,
                end_time: now.timestamp(),
                entries: costs.entries,
            }),
            openai_usage_ledger: usage.map(|usage| OpenAiUsageLedger {
                start_time: month_start,
                end_time: now.timestamp(),
                entries: usage,
            }),
        })
    }
}

fn openai_metrics(
    monthly_amount: f64,
    limit: Option<f64>,
    currency: &str,
    usage: Option<&[OpenAiUsageLedgerEntry]>,
) -> Vec<Metric> {
    let mut metrics = vec![Metric {
        id: "organization_spend_limit".to_owned(),
        label: "Organization spend limit".to_owned(),
        used: monthly_amount,
        limit,
        unit: currency.to_uppercase(),
    }];
    if let Some(entries) = usage {
        let input_tokens: i64 = entries.iter().map(|entry| entry.input_tokens).sum();
        let cached_input_tokens: i64 = entries.iter().map(|entry| entry.cached_input_tokens).sum();
        let output_tokens: i64 = entries.iter().map(|entry| entry.output_tokens).sum();
        let requests: i64 = entries.iter().map(|entry| entry.request_count).sum();
        metrics.extend([
            Metric {
                id: "total_tokens".to_owned(),
                label: "Total tokens".to_owned(),
                used: (input_tokens + output_tokens) as f64,
                limit: None,
                unit: "tokens".to_owned(),
            },
            Metric {
                id: "responses_chat_completions".to_owned(),
                label: "Responses and Chat Completions".to_owned(),
                used: requests as f64,
                limit: None,
                unit: "requests".to_owned(),
            },
            Metric {
                id: "prompt_cache_hit_rate".to_owned(),
                label: "Prompt caching hit rate".to_owned(),
                used: if input_tokens == 0 {
                    0.0
                } else {
                    cached_input_tokens as f64 / input_tokens as f64 * 100.0
                },
                limit: None,
                unit: "%".to_owned(),
            },
            Metric {
                id: "total_requests".to_owned(),
                label: "Total requests".to_owned(),
                used: requests as f64,
                limit: None,
                unit: "requests".to_owned(),
            },
        ]);
    }
    metrics
}

async fn fetch_completions_usage(
    client: &reqwest::Client,
    secret: &SecretString,
    start_time: i64,
    end_time: i64,
) -> Result<Vec<OpenAiUsageLedgerEntry>, ProviderError> {
    let mut entries = Vec::new();
    let mut page: Option<String> = None;
    for page_count in 0..100 {
        let mut query = vec![
            ("start_time", start_time.to_string()),
            ("end_time", end_time.to_string()),
            ("bucket_width", "1d".to_owned()),
            ("limit", "31".to_owned()),
            ("group_by", "project_id".to_owned()),
            ("group_by", "model".to_owned()),
        ];
        if let Some(next_page) = page.as_ref() {
            query.push(("page", next_page.clone()));
        }
        let payload = get_json(
            client,
            secret,
            "https://api.openai.com/v1/organization/usage/completions",
            &query,
        )
        .await?;
        let parsed = parse_usage_entries(&payload).map_err(|_| {
            eprintln!(
                "event=openai_usage_response_invalid endpoint=/organization/usage/completions"
            );
            ProviderError::OpenAiUsageInvalidResponse
        })?;
        entries.extend(parsed);
        let has_more = payload
            .get("has_more")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        page = payload
            .get("next_page")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned);
        if !has_more || page.is_none() {
            return Ok(entries);
        }
        if page_count == 99 {
            break;
        }
    }
    Err(ProviderError::OpenAiUsageInvalidResponse)
}

fn parse_usage_entries(payload: &Value) -> Result<Vec<OpenAiUsageLedgerEntry>, ProviderError> {
    let buckets = payload
        .get("data")
        .and_then(Value::as_array)
        .ok_or(ProviderError::InvalidResponse)?;
    buckets
        .iter()
        .flat_map(|bucket| {
            let start = bucket.get("start_time").and_then(Value::as_i64);
            let end = bucket.get("end_time").and_then(Value::as_i64);
            bucket
                .get("results")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .map(move |result| {
                    Ok(OpenAiUsageLedgerEntry {
                        bucket_start: start.ok_or(ProviderError::InvalidResponse)?,
                        bucket_end: end.ok_or(ProviderError::InvalidResponse)?,
                        input_tokens: non_negative_i64(result, "input_tokens")?,
                        cached_input_tokens: non_negative_i64(result, "input_cached_tokens")?,
                        output_tokens: non_negative_i64(result, "output_tokens")?,
                        request_count: non_negative_i64(result, "num_model_requests")?,
                        project_id: result
                            .get("project_id")
                            .and_then(Value::as_str)
                            .map(ToOwned::to_owned),
                        model: result
                            .get("model")
                            .and_then(Value::as_str)
                            .map(ToOwned::to_owned),
                    })
                })
        })
        .collect()
}

fn non_negative_i64(result: &Value, field: &str) -> Result<i64, ProviderError> {
    result
        .get(field)
        .and_then(Value::as_i64)
        .filter(|value| *value >= 0)
        .ok_or(ProviderError::InvalidResponse)
}

struct Costs {
    currency: String,
    entries: Vec<OpenAiCostLedgerEntry>,
}

async fn fetch_costs(
    client: &reqwest::Client,
    secret: &SecretString,
    start_time: i64,
    end_time: i64,
) -> Result<Costs, ProviderError> {
    let mut entries = Vec::new();
    let mut page: Option<String> = None;
    for page_count in 0..100 {
        let mut query = vec![
            ("start_time", start_time.to_string()),
            ("end_time", end_time.to_string()),
            ("bucket_width", "1d".to_owned()),
            ("limit", "180".to_owned()),
            ("group_by", "project_id".to_owned()),
            ("group_by", "api_key_id".to_owned()),
            ("group_by", "line_item".to_owned()),
        ];
        if let Some(next_page) = page.as_ref() {
            query.push(("page", next_page.clone()));
        }
        let payload = get_json(
            client,
            secret,
            "https://api.openai.com/v1/organization/costs",
            &query,
        )
        .await?;
        let parsed = parse_cost_entries(&payload).map_err(|_| {
            eprintln!("event=openai_usage_response_invalid endpoint=/organization/costs");
            ProviderError::OpenAiCostsInvalidResponse
        })?;
        entries.extend(parsed);
        let has_more = payload
            .get("has_more")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        page = payload
            .get("next_page")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned);
        if !has_more || page.is_none() {
            return costs_from_entries(entries);
        }
        if page_count == 99 {
            break;
        }
    }
    Err(ProviderError::OpenAiCostsInvalidResponse)
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

fn costs_from_entries(entries: Vec<OpenAiCostLedgerEntry>) -> Result<Costs, ProviderError> {
    let mut currency = None;
    for entry in &entries {
        let item_currency = entry.currency.as_str();
        match &currency {
            Some(existing) if existing != item_currency => {
                return Err(ProviderError::InvalidResponse);
            }
            Some(_) => (),
            None => currency = Some(item_currency.to_owned()),
        }
    }
    Ok(Costs {
        currency: currency.unwrap_or_else(|| "usd".to_owned()),
        entries,
    })
}

fn parse_cost_entries(payload: &Value) -> Result<Vec<OpenAiCostLedgerEntry>, ProviderError> {
    let buckets = payload
        .get("data")
        .and_then(Value::as_array)
        .ok_or(ProviderError::InvalidResponse)?;
    buckets
        .iter()
        .flat_map(|bucket| {
            let start = bucket.get("start_time").and_then(Value::as_i64);
            let end = bucket.get("end_time").and_then(Value::as_i64);
            bucket
                .get("results")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .map(move |result| {
                    let amount = result.get("amount").ok_or(ProviderError::InvalidResponse)?;
                    Ok(OpenAiCostLedgerEntry {
                        bucket_start: start.ok_or(ProviderError::InvalidResponse)?,
                        bucket_end: end.ok_or(ProviderError::InvalidResponse)?,
                        amount: amount
                            .get("value")
                            .and_then(Value::as_f64)
                            .filter(|value| value.is_finite())
                            .ok_or(ProviderError::InvalidResponse)?,
                        currency: amount
                            .get("currency")
                            .and_then(Value::as_str)
                            .ok_or(ProviderError::InvalidResponse)?
                            .to_owned(),
                        project_id: result
                            .get("project_id")
                            .and_then(Value::as_str)
                            .map(ToOwned::to_owned),
                        api_key_id: result
                            .get("api_key_id")
                            .and_then(Value::as_str)
                            .map(ToOwned::to_owned),
                        line_item: result
                            .get("line_item")
                            .and_then(Value::as_str)
                            .map(ToOwned::to_owned),
                    })
                })
        })
        .collect()
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
        let entries = parse_cost_entries(&json!({"data":[
            {"start_time":1,"end_time":2,"results":[{"amount":{"value":8.0,"currency":"usd"},"project_id":"proj_a","api_key_id":"key_a","line_item":"completions"}]},
            {"start_time":2,"end_time":3,"results":[{"amount":{"value":0.92,"currency":"usd"}}]}
        ]}))
        .unwrap();
        let costs = costs_from_entries(entries).unwrap();
        assert_eq!(
            costs.entries.iter().map(|entry| entry.amount).sum::<f64>(),
            8.92
        );
        assert_eq!(costs.currency, "usd");
        assert_eq!(costs.entries.len(), 2);
        assert_eq!(costs.entries[0].project_id.as_deref(), Some("proj_a"));
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

    #[test]
    fn parses_grouped_daily_completions_usage() {
        let entries = parse_usage_entries(&json!({"data":[{
            "start_time": 100,
            "end_time": 200,
            "results": [
                {
                    "input_tokens": 1000,
                    "input_cached_tokens": 400,
                    "output_tokens": 500,
                    "num_model_requests": 5,
                    "project_id": "proj_a",
                    "model": "gpt-test"
                },
                {
                    "input_tokens": 20,
                    "input_cached_tokens": 0,
                    "output_tokens": 10,
                    "num_model_requests": 1,
                    "project_id": null,
                    "model": null
                }
            ]
        }]}))
        .unwrap();

        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].cached_input_tokens, 400);
        assert_eq!(entries[0].request_count, 5);
        assert_eq!(entries[0].project_id.as_deref(), Some("proj_a"));
        assert_eq!(entries[1].project_id, None);
    }

    #[test]
    fn creates_usage_metrics_from_period_totals() {
        let usage = vec![
            OpenAiUsageLedgerEntry {
                bucket_start: 1,
                bucket_end: 2,
                input_tokens: 800,
                cached_input_tokens: 400,
                output_tokens: 200,
                request_count: 3,
                project_id: None,
                model: None,
            },
            OpenAiUsageLedgerEntry {
                bucket_start: 2,
                bucket_end: 3,
                input_tokens: 200,
                cached_input_tokens: 100,
                output_tokens: 50,
                request_count: 2,
                project_id: None,
                model: None,
            },
        ];
        let metrics = openai_metrics(8.92, Some(12.0), "usd", Some(&usage));

        assert_eq!(
            metrics
                .iter()
                .find(|metric| metric.id == "total_tokens")
                .unwrap()
                .used,
            1250.0
        );
        assert_eq!(
            metrics
                .iter()
                .find(|metric| metric.id == "prompt_cache_hit_rate")
                .unwrap()
                .used,
            50.0
        );
        assert_eq!(
            metrics
                .iter()
                .find(|metric| metric.id == "total_requests")
                .unwrap()
                .used,
            5.0
        );
    }

    #[test]
    fn rejects_negative_usage_values() {
        let result = parse_usage_entries(&json!({"data":[{
            "start_time": 100,
            "end_time": 200,
            "results": [{
                "input_tokens": -1,
                "input_cached_tokens": 0,
                "output_tokens": 0,
                "num_model_requests": 1
            }]
        }]}));

        assert!(result.is_err());
    }
}
