use super::{AppState, dashboard};
use crate::{
	dtos::{AccountSettingsTemplate, AppError},
	models::{Account, UpdateAccount},
	secrets::begin_authentication,
	utils::valid_project_id,
};
use askama::Template;
use axum::{Form, extract::State, response::Html};
use axum::{
	Router,
	routing::{get, post},
};
use std::sync::Arc;

pub fn router() -> Router<Arc<AppState>> {
	Router::new()
		.route("/settings", get(account_settings))
		.route("/", post(update_account))
}

pub(super) async fn account_settings(State(state): State<Arc<AppState>>) -> Result<Html<String>, AppError> {
	let account = state.database.active_account()?.ok_or(AppError::NotFound)?;
	Ok(Html(AccountSettingsTemplate { account }.render()?))
}

pub(super) async fn update_account(
	State(state): State<Arc<AppState>>,
	Form(input): Form<UpdateAccount>,
) -> Result<Html<String>, AppError> {
	if !valid_project_id(&input.project_id) {
		return Err(AppError::BadRequest);
	}
	let account = state.database.active_account()?.ok_or(AppError::NotFound)?;
	if account.project_id != input.project_id {
		state.database.update_active_account(input)?;
	}
	let account = state.database.active_account()?.ok_or(AppError::BadRequest)?;
	launch_authentication(&state, &account).await?;
	dashboard::render_dashboard(&state)
}

pub(super) async fn launch_authentication(state: &AppState, account: &Account) -> Result<(), AppError> {
	state.database.set_auth_status(&account.id, "authenticating", None, None)?;
	let project_id = account.project_id.clone();
	tokio::spawn(async move {
		let _ = begin_authentication(&project_id).await;
	});
	// Give the desktop browser launch a moment before replacing the launch button.
	tokio::time::sleep(std::time::Duration::from_millis(700)).await;
	Ok(())
}
