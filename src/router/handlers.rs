use askama::Template;
use std::sync::Arc;

use axum::{extract::State, http::HeaderMap, response::Html};

use super::dashboard::{
	month_from_url, refresh_saved_authentication, render_dashboard_with_auth_validation, render_overview,
	selected_period,
};
use super::providers::render_providers_page;
use super::runlogs::render_run_logs;
use crate::{dtos::*, router::AppState};

pub(super) async fn not_found() -> Result<(axum::http::StatusCode, Html<String>), AppError> {
	Ok((axum::http::StatusCode::NOT_FOUND, Html(NotFoundTemplate.render()?)))
}

pub(super) async fn validate_saved_authentication(
	State(state): State<Arc<AppState>>,
	headers: HeaderMap,
) -> Result<Html<String>, AppError> {
	refresh_saved_authentication(&state).await?;
	let current_url = headers.get("HX-Current-URL").and_then(|value| value.to_str().ok());
	if current_url.is_some_and(|url| url.split('?').next().is_some_and(|path| path.ends_with("/runlogs"))) {
		return render_run_logs(&state, false);
	}
	if current_url.is_some_and(|url| url.split('?').next().is_some_and(|path| path.ends_with("/providers"))) {
		return render_providers_page(&state, false);
	}
	if current_url.is_some_and(|url| url.split('?').next().is_some_and(|path| path.ends_with('/'))) {
		return render_overview(&state, false);
	}
	let month = current_url.and_then(month_from_url);
	render_dashboard_with_auth_validation(&state, false, selected_period(month)?)
}
