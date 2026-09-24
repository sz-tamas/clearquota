use super::{AppState, dashboard};
use crate::{
	dtos::{AccountSettingsTemplate, AppError, SettingsTemplate},
	models::{Account, UpdateAccount},
	secrets::begin_authentication,
	utils::valid_project_id,
};
use askama::Template;
use axum::{
	Form,
	extract::{Path, State},
	response::{Html, Redirect},
};
use axum::{
	Router,
	routing::{get, post},
};
use std::sync::Arc;

#[derive(serde::Deserialize)]
pub struct EraseConfirmation {
	confirmation: String,
}

pub async fn settings_page(State(state): State<Arc<AppState>>) -> Result<Html<String>, AppError> {
	let (snapshot_count, log_count) = state.database.local_data_counts()?;
	let bytes = state.database.storage_size();
	let storage_size = if bytes < 1024 {
		format!("{bytes} B")
	} else if bytes < 1024 * 1024 {
		format!("{:.1} KB", bytes as f64 / 1024.0)
	} else {
		format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
	};
	Ok(Html(
		SettingsTemplate {
			account: state.database.active_account()?,
			accounts: state.database.list_accounts()?,
			storage_path: state.database.storage_path().display().to_string(),
			storage_size,
			snapshot_count,
			log_count,
		}
		.render()?,
	))
}

pub async fn clear_local_data(
	State(state): State<Arc<AppState>>,
	Path(category): Path<String>,
) -> Result<Redirect, AppError> {
	match category.as_str() {
		"usage" => state.database.clear_usage_history()?,
		"logs" => state.database.clear_refresh_logs()?,
		_ => return Err(AppError::NotFound),
	}
	Ok(Redirect::to("/settings"))
}

pub async fn erase_local_data(
	State(state): State<Arc<AppState>>,
	Form(input): Form<EraseConfirmation>,
) -> Result<Redirect, AppError> {
	if input.confirmation != "ERASE" {
		return Err(AppError::BadRequest);
	}
	state.database.erase_all_local_data()?;
	Ok(Redirect::to("/settings"))
}

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
