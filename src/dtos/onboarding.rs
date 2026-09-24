use crate::models::Account;
use askama::Template;

#[derive(Template)]
#[template(path = "partials/auth_required.html")]
pub(crate) struct AuthRequiredTemplate {
	pub(crate) account: Account,
}
