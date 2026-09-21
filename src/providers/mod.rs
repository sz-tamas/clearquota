mod apify;
mod openai;
mod resend;

use async_trait::async_trait;
use secrecy::SecretString;

use crate::models::{ProviderConfig, UsagePeriod, UsageSnapshot};

pub use apify::ApifyProvider;
pub use openai::OpenAiProvider;
pub use resend::ResendProvider;

#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
	#[error("this provider is not implemented yet")]
	Unsupported,
	#[error("provider request failed")]
	Request,
	#[error(
		"Your Resend API key has Sending access only. Please create a Full access API key to retrieve usage statistics."
	)]
	ResendSendingAccessOnly,
	#[error("provider returned an invalid usage response")]
	InvalidResponse,
	#[error("OpenAI costs response did not contain usable monthly cost data")]
	OpenAiCostsInvalidResponse,
	#[error("OpenAI spend alerts response did not contain a 100% monthly threshold")]
	OpenAiSpendAlertInvalidResponse,
	#[error("OpenAI completions usage response did not contain usable activity data")]
	OpenAiUsageInvalidResponse,
}

#[async_trait]
pub trait Provider: Send + Sync {
	async fn collect(
		&self,
		config: &ProviderConfig,
		secret: &SecretString,
		period: UsagePeriod,
	) -> Result<UsageSnapshot, ProviderError>;
}

#[derive(Default)]
pub struct ProviderRegistry;

impl ProviderRegistry {
	pub async fn collect(
		&self,
		config: &ProviderConfig,
		secret: &SecretString,
		period: UsagePeriod,
	) -> Result<UsageSnapshot, ProviderError> {
		match config.provider_type.as_str() {
			"apify" => ApifyProvider.collect(config, secret, period).await,
			"openai" => OpenAiProvider.collect(config, secret, period).await,
			"resend" => ResendProvider.collect(config, secret, period).await,
			_ => Err(ProviderError::Unsupported),
		}
	}
}
