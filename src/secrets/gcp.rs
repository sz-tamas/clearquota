use async_trait::async_trait;
use base64::{
	Engine,
	engine::general_purpose::{URL_SAFE, URL_SAFE_NO_PAD},
};
use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;
use tokio::process::Command;
use zeroize::{Zeroize, Zeroizing};

use super::{SecretError, SecretResolver};

pub struct GcpSecretManagerResolver;

#[derive(Deserialize)]
struct AccessSecretVersionResponse {
	payload: SecretPayload,
}

#[derive(Deserialize)]
struct SecretPayload {
	data: String,
}

/// Opens the Google Cloud ADC browser flow. Verification is a separate user action.
pub async fn begin_authentication(project_id: &str) -> Result<(), SecretError> {
	let login = Command::new("gcloud")
		.args(["auth", "application-default", "login", "--project", project_id, "--quiet"])
		.output()
		.await
		.map_err(|_| SecretError::AuthenticationFailed)?;
	if login.status.success() {
		Ok(())
	} else {
		Err(SecretError::AuthenticationFailed)
	}
}

/// Checks the exact ADC credential used by the collector without exposing its token.
pub async fn check_application_default_credentials() -> Result<(), SecretError> {
	application_default_access_token().await.map(|_| ())
}

async fn application_default_access_token() -> Result<SecretString, SecretError> {
	let credentials = Command::new("gcloud")
		.args(["auth", "application-default", "print-access-token", "--quiet"])
		.output()
		.await
		.map_err(|_| SecretError::AuthenticationFailed)?;
	if !credentials.status.success() || credentials.stdout.is_empty() {
		return Err(SecretError::AuthenticationFailed);
	}
	let mut token = match String::from_utf8(credentials.stdout) {
		Ok(token) => token,
		Err(error) => {
			let mut invalid_bytes = error.into_bytes();
			invalid_bytes.zeroize();
			return Err(SecretError::AuthenticationFailed);
		}
	};
	let without_newline = token.trim_end_matches(['\r', '\n']).len();
	token.truncate(without_newline);
	if token.is_empty() {
		return Err(SecretError::AuthenticationFailed);
	}
	Ok(SecretString::from(token))
}

#[async_trait]
impl SecretResolver for GcpSecretManagerResolver {
	async fn resolve(&self, reference: &str) -> Result<SecretString, SecretError> {
		let parts: Vec<_> = reference.split('/').collect();
		if parts.len() != 6
			|| parts[0] != "projects"
			|| parts[2] != "secrets"
			|| parts[4] != "versions"
			|| parts.iter().any(|part| part.is_empty())
		{
			return Err(SecretError::InvalidReference);
		}
		// `gcloud secrets ...` authenticates as the CLI's active account, which
		// is separate from ADC. Use an ADC token directly so this is the same
		// identity checked during onboarding.
		let token = application_default_access_token().await?;
		let response = reqwest::Client::new()
			.get(format!("https://secretmanager.googleapis.com/v1/{reference}:access"))
			.bearer_auth(token.expose_secret())
			.send()
			.await
			.map_err(|_| SecretError::AccessFailed)?;
		if !response.status().is_success() {
			return Err(SecretError::AccessFailed);
		}
		let mut payload = response
			.json::<AccessSecretVersionResponse>()
			.await
			.map_err(|_| SecretError::InvalidPayload)?;
		let encoded = Zeroizing::new(std::mem::take(&mut payload.payload.data));
		let bytes = Zeroizing::new(
			URL_SAFE_NO_PAD
				.decode(encoded.as_bytes())
				.or_else(|_| URL_SAFE.decode(encoded.as_bytes()))
				.map_err(|_| SecretError::InvalidPayload)?,
		);
		let mut value = match String::from_utf8(bytes.to_vec()) {
			Ok(value) => value,
			Err(error) => {
				let mut invalid_bytes = error.into_bytes();
				invalid_bytes.zeroize();
				return Err(SecretError::InvalidPayload);
			}
		};
		let without_newline = value.trim_end_matches(['\r', '\n']).len();
		value.truncate(without_newline);
		// `encoded` and `bytes` are explicitly zeroized when they leave scope.
		// SecretString zeroizes the returned provider credential when dropped.
		Ok(SecretString::from(value))
	}
}
