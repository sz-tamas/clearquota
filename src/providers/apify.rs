use async_trait::async_trait;
use secrecy::{ExposeSecret, SecretString};
use serde_json::Value;

use crate::models::{Metric, ProviderConfig, UsageSnapshot};

use super::{Provider, ProviderError};

pub struct ApifyProvider;

#[async_trait]
impl Provider for ApifyProvider {
    async fn collect(
        &self,
        config: &ProviderConfig,
        secret: &SecretString,
    ) -> Result<UsageSnapshot, ProviderError> {
        let response = reqwest::Client::new()
            .get("https://api.apify.com/v2/users/me/usage/monthly")
            .bearer_auth(secret.expose_secret())
            .send()
            .await
            .map_err(|_| ProviderError::Request)?;
        if !response.status().is_success() {
            return Err(ProviderError::Request);
        }
        let payload: Value = response
            .json()
            .await
            .map_err(|_| ProviderError::InvalidResponse)?;
        let data = payload.get("data").unwrap_or(&payload);
        let used = data
            .get("totalUsageCreditsUsdAfterVolumeDiscount")
            .and_then(Value::as_f64)
            .ok_or(ProviderError::InvalidResponse)?;
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
            openai_cost_ledger: None,
            openai_usage_ledger: None,
        })
    }
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
}
