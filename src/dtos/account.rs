use crate::models::Account;
use askama::Template;

#[derive(Template)]
#[template(path = "partials/account_settings.html")]
pub(crate) struct AccountSettingsTemplate {
	pub(crate) account: Account,
}
