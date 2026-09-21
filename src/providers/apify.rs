use async_trait::async_trait;
use secrecy::{ExposeSecret, SecretString};
use serde_json::{Value, json};

use crate::models::{Metric, ProviderConfig, UsagePeriod, UsageSnapshot};

use super::{Provider, ProviderError};

pub struct ApifyProvider;

#[async_trait]
impl Provider for ApifyProvider {
	async fn collect(
		&self,
		config: &ProviderConfig,
		secret: &SecretString,
		period: UsagePeriod,
	) -> Result<UsageSnapshot, ProviderError> {
		let client = reqwest::Client::new();
		let now = chrono::Utc::now().timestamp();
		let last_included = if period.is_current { now.min(period.end - 1) } else { period.end - 1 };
		let query_dates = [period.start, last_included];
		let mut daily_totals = std::collections::BTreeMap::new();
		let mut native_cycles = Vec::new();
		for timestamp in query_dates {
			let date = chrono::DateTime::from_timestamp(timestamp, 0)
				.ok_or(ProviderError::InvalidResponse)?
				.date_naive()
				.to_string();
			let payload = fetch_monthly_usage(&client, secret, &date).await?;
			let data = payload.get("data").unwrap_or(&payload);
			absorb_cycle(data, period, now, &mut daily_totals, &mut native_cycles)?;
		}
		let used = daily_totals.values().sum::<f64>();
		let limit = config.apify_monthly_credit_allowance;
		if !limit.is_finite() || limit <= 0.0 {
			return Err(ProviderError::InvalidResponse);
		}
		Ok(UsageSnapshot {
			provider_id: config.id.clone(),
			timestamp: unix_timestamp(),
			status: "ok".to_owned(),
			cost: Some(used),
			currency: Some("USD".to_owned()),
			metrics: monthly_credit_metrics(used, limit),
			period,
			metadata: json!({
				"apify_native_billing_cycles": native_cycles,
				"calendar_month_daily_entries": daily_totals.len()
			}),
			openai_cost_ledger: None,
			openai_usage_ledger: None,
		})
	}
}

fn absorb_cycle(
	data: &Value,
	period: UsagePeriod,
	now: i64,
	daily_totals: &mut std::collections::BTreeMap<i64, f64>,
	native_cycles: &mut Vec<Value>,
) -> Result<(), ProviderError> {
	let cycle = data.get("usageCycle").ok_or(ProviderError::InvalidResponse)?;
	let cycle_start = cycle
		.get("startAt")
		.and_then(Value::as_str)
		.ok_or(ProviderError::InvalidResponse)?;
	let cycle_end = cycle
		.get("endAt")
		.and_then(Value::as_str)
		.ok_or(ProviderError::InvalidResponse)?;
	let native_total = data
		.get("totalUsageCreditsUsdAfterVolumeDiscount")
		.and_then(Value::as_f64)
		.ok_or(ProviderError::InvalidResponse)?;
	if !native_cycles
		.iter()
		.any(|entry| entry.get("start_at").and_then(Value::as_str) == Some(cycle_start))
	{
		native_cycles.push(json!({
			"start_at": cycle_start,
			"end_at": cycle_end,
			"total_usage_credits_usd_after_volume_discount": native_total
		}));
	}
	let days = data
		.get("dailyServiceUsages")
		.and_then(Value::as_array)
		.ok_or(ProviderError::InvalidResponse)?;
	for day in days {
		let day_start = day
			.get("date")
			.and_then(Value::as_str)
			.and_then(|value| chrono::DateTime::parse_from_rfc3339(value).ok())
			.map(|value| value.timestamp())
			.ok_or(ProviderError::InvalidResponse)?;
		if day_start < period.start || day_start >= period.end || day_start > now {
			continue;
		}
		let amount = day
			.get("serviceUsage")
			.and_then(Value::as_object)
			.ok_or(ProviderError::InvalidResponse)?
			.values()
			.filter_map(|usage| usage.get("amountAfterVolumeDiscountUsd").and_then(Value::as_f64))
			.sum::<f64>();
		daily_totals.insert(day_start, amount);
	}
	Ok(())
}

async fn fetch_monthly_usage(
	client: &reqwest::Client,
	secret: &SecretString,
	date: &str,
) -> Result<Value, ProviderError> {
	let response = client
		.get("https://api.apify.com/v2/users/me/usage/monthly")
		.query(&[("date", date)])
		.bearer_auth(secret.expose_secret())
		.send()
		.await
		.map_err(|_| ProviderError::Request)?;
	if !response.status().is_success() {
		return Err(ProviderError::Request);
	}
	response.json().await.map_err(|_| ProviderError::InvalidResponse)
}

fn monthly_credit_metrics(used: f64, allowance: f64) -> Vec<Metric> {
	vec![
		Metric {
			id: "monthly_credit_allowance".to_owned(),
			label: "Monthly credit allowance".to_owned(),
			used,
			limit: Some(allowance),
			unit: "USD".to_owned(),
		},
		Metric {
			id: "monthly_credit_remaining".to_owned(),
			label: "Monthly credit remaining".to_owned(),
			used: allowance - used,
			limit: None,
			unit: "USD".to_owned(),
		},
		Metric {
			id: "monthly_credit_used_percent".to_owned(),
			label: "Monthly credit used".to_owned(),
			used: used / allowance * 100.0,
			limit: None,
			unit: "%".to_owned(),
		},
	]
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

	#[test]
	fn calculates_credit_allowance_metrics() {
		let metrics = monthly_credit_metrics(8.92, 49.0);
		assert_eq!(metrics[0].limit, Some(49.0));
		assert!((metrics[1].used - 40.08).abs() < f64::EPSILON);
		assert!((metrics[2].used - 18.204_081_632_653_06).abs() < f64::EPSILON);
	}

	#[test]
	fn derives_calendar_usage_and_preserves_native_cycle_metadata() {
		let august_start = chrono::DateTime::parse_from_rfc3339("2026-08-01T00:00:00Z")
			.unwrap()
			.timestamp();
		let september_start = chrono::DateTime::parse_from_rfc3339("2026-09-01T00:00:00Z")
			.unwrap()
			.timestamp();
		let data = json!({
			"usageCycle": {"startAt": "2026-07-15T00:00:00Z", "endAt": "2026-08-14T23:59:59Z"},
			"totalUsageCreditsUsdAfterVolumeDiscount": 9.5,
			"dailyServiceUsages": [
				{"date": "2026-07-31T00:00:00Z", "serviceUsage": {"compute": {"amountAfterVolumeDiscountUsd": 8.0}}},
				{"date": "2026-08-01T00:00:00Z", "serviceUsage": {
					"compute": {"amountAfterVolumeDiscountUsd": 1.25},
					"storage": {"amountAfterVolumeDiscountUsd": 0.75}
				}}
			]
		});
		let mut daily = std::collections::BTreeMap::new();
		let mut cycles = Vec::new();
		absorb_cycle(
			&data,
			UsagePeriod {
				start: august_start,
				end: september_start,
				is_current: false,
			},
			september_start,
			&mut daily,
			&mut cycles,
		)
		.unwrap();
		assert_eq!(daily.values().sum::<f64>(), 2.0);
		assert_eq!(cycles.len(), 1);
		assert_eq!(cycles[0]["total_usage_credits_usd_after_volume_discount"], 9.5);
	}
}
