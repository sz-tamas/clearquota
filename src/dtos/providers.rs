use crate::models::{Account, OpenAiCreditEvent, ProviderConfig};

use askama::Template;
use serde::Deserialize;

#[derive(Default, Deserialize)]
pub(crate) struct NewProviderQuery {
	pub(crate) provider: Option<String>,
}

#[derive(Template)]
#[template(path = "pages/providers.html")]
pub(crate) struct ProvidersTemplate {
	pub(crate) account: Account,
	pub(crate) providers: Vec<ProviderRow>,
	pub(crate) validate_authentication: bool,
}

pub(crate) struct ProviderRow {
	pub(crate) provider: ProviderConfig,
	pub(crate) ledger_html: String,
	pub(crate) settings_html: String,
}

#[derive(Deserialize)]
pub(crate) struct ProviderSettingsInput {
	pub(crate) plan: Option<String>,
	pub(crate) monthly_quota: Option<i64>,
	pub(crate) daily_quota: Option<i64>,
	pub(crate) apify_monthly_credit_allowance: Option<f64>,
}

#[derive(Template)]
#[template(path = "partials/new_provider_dialog.html")]
pub(crate) struct NewProviderDialogTemplate {
	pub(crate) provider_type: String,
	pub(crate) display_name: &'static str,
}

#[derive(Template)]
#[template(path = "partials/provider_list.html")]
pub(crate) struct ProviderListTemplate {
	pub(crate) cards_html: String,
}

#[derive(Template)]
#[template(path = "partials/edit_provider.html")]
pub(crate) struct EditProviderTemplate {
	pub(crate) provider: ProviderConfig,
}

#[derive(Template)]
#[template(path = "partials/openai_credit_ledger.html")]
pub(crate) struct OpenAiCreditLedgerTemplate {
	pub(crate) provider: ProviderConfig,
	pub(crate) credit_events: Vec<OpenAiCreditEvent>,
}

#[derive(Template)]
#[template(path = "partials/provider_settings.html")]
pub(crate) struct ProviderSettingsTemplate {
	pub(crate) provider: ProviderConfig,
}

#[derive(Template)]
#[template(path = "partials/delete_provider.html")]
pub(crate) struct DeleteProviderTemplate {
	pub(crate) provider: ProviderConfig,
}

#[derive(Template)]
#[template(path = "partials/refresh_complete.html")]
pub(crate) struct RefreshCompleteTemplate {
	pub(crate) succeeded: usize,
	pub(crate) failed: usize,
	pub(crate) period_label: String,
	pub(crate) provider_list_url: String,
}
