use super::{AppState, account, dashboard};
use crate::{
	dtos::{AUTH_CHECK_ERROR, AppError, AuthRequiredTemplate},
	models::NewAccount,
	secrets::check_application_default_credentials,
	utils::valid_project_id,
};
use askama::Template;
use axum::{Form, Router, extract::State, response::Html, routing::post};
use std::sync::Arc;

pub fn router() -> Router<Arc<AppState>> {
	Router::new()
		.route("/account", post(create_account))
		.route("/auth", post(start_auth))
		.route("/auth/check", post(check_auth))
		.route("/alerts/skip", post(skip_alerts))
}

pub(super) async fn create_account(
	State(state): State<Arc<AppState>>,
	Form(input): Form<NewAccount>,
) -> Result<Html<String>, AppError> {
	if !valid_project_id(&input.project_id) {
		return Err(AppError::BadRequest);
	}
	let account = state.database.create_account(input)?;
	Ok(Html(AuthRequiredTemplate { account }.render()?))
}

pub(super) async fn start_auth(State(state): State<Arc<AppState>>) -> Result<Html<String>, AppError> {
	let account = state.database.active_account()?.ok_or(AppError::BadRequest)?;
	account::launch_authentication(&state, &account).await?;
	let account = state.database.active_account()?.ok_or(AppError::BadRequest)?;
	Ok(Html(AuthRequiredTemplate { account }.render()?))
}

pub(super) async fn check_auth(State(state): State<Arc<AppState>>) -> Result<Html<String>, AppError> {
	let account = state.database.active_account()?.ok_or(AppError::BadRequest)?;
	match check_application_default_credentials().await {
		Ok(()) => {
			state
				.database
				.set_auth_status(&account.id, "ready", Some(&account.project_id), None)?;
			state.database.set_onboarding_step(&account.id, 4)?;
		}
		Err(_) => {
			state.database.set_auth_status(&account.id, "failed", None, None)?;
			return dashboard::render_dashboard_with_auth_error(&state, AUTH_CHECK_ERROR);
		}
	}
	dashboard::render_dashboard(&state)
}

pub(super) async fn skip_alerts(State(state): State<Arc<AppState>>) -> Result<Html<String>, AppError> {
	let account = state.database.active_account()?.ok_or(AppError::BadRequest)?;
	state.database.set_onboarding_step(&account.id, 4)?;
	dashboard::render_dashboard(&state)
}
