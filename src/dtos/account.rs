use crate::models::Account;
use askama::Template;

#[derive(Template)]
#[template(path = "partials/account_settings.html")]
pub(crate) struct AccountSettingsTemplate {
	pub(crate) account: Account,
}

#[derive(Template)]
#[template(path = "pages/settings.html")]
pub(crate) struct SettingsTemplate {
	pub(crate) account: Option<Account>,
	pub(crate) accounts: Vec<Account>,
	pub(crate) storage_path: String,
	pub(crate) storage_size: String,
	pub(crate) snapshot_count: i64,
	pub(crate) log_count: i64,
}
