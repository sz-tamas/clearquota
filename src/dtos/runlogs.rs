use crate::models::Account;
use askama::Template;

#[derive(Template)]
#[template(path = "pages/run_logs.html")]
pub(crate) struct RunLogsTemplate {
	pub(crate) account: Account,
	pub(crate) logs: Vec<RunLogView>,
	pub(crate) validate_authentication: bool,
}

pub(crate) struct RunLogView {
	pub(crate) provider_name: String,
	pub(crate) provider_type: String,
	pub(crate) created_at: String,
	pub(crate) status: String,
	pub(crate) message: String,
}
