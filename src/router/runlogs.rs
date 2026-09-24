use super::{AppState, dashboard};
use crate::{
	dtos::{AppError, RunLogView, RunLogsTemplate},
	models::RunLog,
	utils::is_htmx_navigation,
};
use askama::Template;
use axum::{Router, extract::State, http::HeaderMap, response::Html, routing::get};
use std::sync::Arc;

pub fn router() -> Router<Arc<AppState>> {
	Router::new().route("/", get(run_logs))
}

pub(super) async fn run_logs(State(state): State<Arc<AppState>>, headers: HeaderMap) -> Result<Html<String>, AppError> {
	render_run_logs(&state, !is_htmx_navigation(&headers))
}

pub(super) fn render_run_logs(state: &AppState, validate_authentication: bool) -> Result<Html<String>, AppError> {
	let account = dashboard::account_for_render(state)?.ok_or(AppError::NotFound)?;
	let logs = state.database.list_run_logs(&account.id)?;
	Ok(Html(
		RunLogsTemplate {
			validate_authentication: account.auth_status == "ready" && validate_authentication,
			account,
			logs: logs.into_iter().map(run_log_view).collect(),
		}
		.render()?,
	))
}

fn run_log_view(log: RunLog) -> RunLogView {
	let created_at = log
		.created_at
		.parse::<i64>()
		.ok()
		.and_then(|timestamp| chrono::DateTime::from_timestamp(timestamp, 0))
		.map(|timestamp| timestamp.format("%Y-%m-%d %H:%M:%S UTC").to_string())
		.unwrap_or(log.created_at);
	RunLogView {
		provider_name: log.provider_name,
		provider_type: log.provider_type,
		created_at,
		status: log.status,
		message: log.message,
	}
}
