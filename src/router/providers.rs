use super::{AppState, dashboard};
use crate::{
	dtos::*,
	models::{NewOpenAiCreditEvent, NewProvider, ProviderConfig, UpdateProvider, UsagePeriod},
	secrets::check_application_default_credentials,
	utils::{
		is_htmx_navigation, month_value, normalize_secret_reference, period_label, selected_period,
		valid_credit_allowance, valid_secret_name,
	},
};
use askama::Template;
use axum::{
	Form,
	extract::{Path, Query, State},
	http::HeaderMap,
	response::{Html, Redirect},
};
use axum::{
	Router,
	routing::{get, post},
};
use chrono::NaiveDate;
use std::sync::Arc;

pub fn router() -> Router<Arc<AppState>> {
	Router::new()
		.route("/", get(providers_page).post(create_provider))
		.route("/list", get(provider_list))
		.route("/refresh", post(refresh_all_providers))
		.route("/new", get(new_provider_form))
		.route("/{id}/edit", get(edit_provider))
		.route("/{id}", post(update_provider))
		.route("/{id}/settings", post(update_provider_settings))
		.route("/{id}/credit-events", post(add_openai_credit_event))
		.route(
			"/{id}/credit-events/{event_id}/delete",
			post(delete_openai_credit_event),
		)
		.route("/{id}/delete", post(delete_provider))
		.route("/{id}/delete/confirm", get(delete_confirmation))
}

pub(super) async fn providers_page(
	State(state): State<Arc<AppState>>,
	headers: HeaderMap,
) -> Result<Html<String>, AppError> {
	render_providers_page(&state, !is_htmx_navigation(&headers))
}

pub(super) fn render_providers_page(state: &AppState, validate_authentication: bool) -> Result<Html<String>, AppError> {
	let account = dashboard::account_for_render(state)?.ok_or(AppError::NotFound)?;
	let providers = state
		.database
		.list_providers(&account.id)?
		.into_iter()
		.map(|provider| {
			let ledger_html = if provider.provider_type == "openai" {
				render_openai_credit_ledger(state, provider.clone())?
			} else {
				String::new()
			};
			let settings_html = if matches!(provider.provider_type.as_str(), "apify" | "resend") {
				render_provider_settings(provider.clone())?
			} else {
				String::new()
			};
			Ok(ProviderRow {
				provider,
				ledger_html,
				settings_html,
			})
		})
		.collect::<Result<Vec<_>, AppError>>()?;
	Ok(Html(
		ProvidersTemplate {
			validate_authentication: account.auth_status == "ready" && validate_authentication,
			account,
			providers,
		}
		.render()?,
	))
}

pub(super) async fn new_provider_form(
	State(state): State<Arc<AppState>>,
	Query(query): Query<NewProviderQuery>,
) -> Result<Html<String>, AppError> {
	state.database.active_account()?.ok_or(AppError::BadRequest)?;
	let provider_type = query.provider.unwrap_or_else(|| "resend".to_owned());
	if !matches!(provider_type.as_str(), "apify" | "openai" | "resend") {
		return Err(AppError::BadRequest);
	}
	let display_name = match provider_type.as_str() {
		"apify" => "Apify",
		"openai" => "OpenAI",
		_ => "Resend",
	};
	Ok(Html(
		NewProviderDialogTemplate {
			provider_type,
			display_name,
		}
		.render()?,
	))
}

pub(super) async fn create_provider(
	State(state): State<Arc<AppState>>,
	Form(input): Form<NewProvider>,
) -> Result<Html<String>, AppError> {
	let account = state.database.active_account()?.ok_or(AppError::BadRequest)?;
	if check_application_default_credentials().await.is_err() {
		state.database.set_auth_status(
			&account.id,
			"failed",
			None,
			Some("Google Application Default Credentials are unavailable. Authenticate with Google to continue."),
		)?;
		return render_providers_page(&state, false);
	}
	if account.auth_status != "ready"
		|| !matches!(input.provider_type.as_str(), "apify" | "openai" | "resend")
		|| input.display_name.trim().is_empty()
		|| input.secret_ref.trim().is_empty()
	{
		return Err(AppError::BadRequest);
	}
	let is_resend = input.provider_type == "resend";
	if !valid_secret_name(input.secret_ref.trim()) {
		return Err(AppError::BadRequest);
	}
	state.database.add_provider(&account.id, input)?;
	state.database.set_onboarding_step(&account.id, if is_resend { 4 } else { 3 })?;
	render_providers_page(&state, false)
}

pub(super) async fn edit_provider(
	Path(id): Path<String>,
	State(state): State<Arc<AppState>>,
) -> Result<Html<String>, AppError> {
	let provider = provider_for_active_account(&state, &id)?;
	Ok(Html(EditProviderTemplate { provider }.render()?))
}

pub(super) async fn update_provider(
	Path(id): Path<String>,
	State(state): State<Arc<AppState>>,
	Form(mut input): Form<UpdateProvider>,
) -> Result<Html<String>, AppError> {
	let provider = provider_for_active_account(&state, &id)?;
	if input.display_name.trim().is_empty() || input.secret_ref.trim().is_empty() {
		return Err(AppError::BadRequest);
	}
	if !valid_secret_name(input.secret_ref.trim()) {
		return Err(AppError::BadRequest);
	}
	input.plan = Some(provider.plan.clone());
	input.monthly_quota = Some(provider.monthly_quota);
	input.daily_quota = Some(provider.daily_quota);
	input.apify_monthly_credit_allowance = Some(provider.apify_monthly_credit_allowance);
	state.database.update_provider(&provider.id, &provider.account_id, input)?;
	render_providers_page(&state, false)
}

pub(super) async fn update_provider_settings(
	Path(id): Path<String>,
	State(state): State<Arc<AppState>>,
	Form(input): Form<ProviderSettingsInput>,
) -> Result<Html<String>, AppError> {
	let provider = provider_for_active_account(&state, &id)?;
	let (plan, monthly_quota, daily_quota, apify_monthly_credit_allowance) = match provider.provider_type.as_str() {
		"apify" if valid_credit_allowance(input.apify_monthly_credit_allowance) => (
			provider.plan.clone(),
			provider.monthly_quota,
			provider.daily_quota,
			input.apify_monthly_credit_allowance.unwrap_or_default(),
		),
		"resend"
			if input.plan.as_deref().is_some_and(|plan| !plan.trim().is_empty())
				&& input.monthly_quota.is_some_and(|quota| quota > 0)
				&& input.daily_quota.is_some_and(|quota| quota > 0) =>
		{
			(
				input.plan.unwrap_or_default().trim().to_owned(),
				input.monthly_quota.unwrap_or_default(),
				input.daily_quota.unwrap_or_default(),
				provider.apify_monthly_credit_allowance,
			)
		}
		_ => return Err(AppError::BadRequest),
	};
	state.database.update_provider(
		&provider.id,
		&provider.account_id,
		UpdateProvider {
			display_name: provider.display_name.clone(),
			secret_ref: provider.secret_ref.clone(),
			plan: Some(plan),
			monthly_quota: Some(monthly_quota),
			daily_quota: Some(daily_quota),
			apify_monthly_credit_allowance: Some(apify_monthly_credit_allowance),
		},
	)?;
	let updated = provider_for_active_account(&state, &id)?;
	Ok(Html(render_provider_settings(updated)?))
}

fn render_provider_settings(provider: ProviderConfig) -> Result<String, AppError> {
	Ok(ProviderSettingsTemplate { provider }.render()?)
}

pub(super) async fn add_openai_credit_event(
	Path(id): Path<String>,
	State(state): State<Arc<AppState>>,
	Form(input): Form<NewOpenAiCreditEvent>,
) -> Result<Html<String>, AppError> {
	let provider = provider_for_active_account(&state, &id)?;
	let effective_at = NaiveDate::parse_from_str(&input.date, "%Y-%m-%d")
		.ok()
		.and_then(|date| date.and_hms_opt(0, 0, 0))
		.map(|date| date.and_utc().timestamp())
		.ok_or(AppError::BadRequest)?;
	if provider.provider_type != "openai"
		|| !matches!(input.event_type.as_str(), "purchase" | "refund" | "adjustment")
		|| !input.amount.is_finite()
		|| input.amount <= 0.0
		|| effective_at > chrono::Utc::now().timestamp()
	{
		return Err(AppError::BadRequest);
	}
	state.database.add_openai_credit_event(
		&provider.id,
		&input.event_type,
		effective_at,
		input.amount,
		input.note.as_deref().unwrap_or("").trim(),
	)?;
	Ok(Html(render_openai_credit_ledger(&state, provider)?))
}

pub(super) async fn delete_openai_credit_event(
	Path((id, event_id)): Path<(String, i64)>,
	State(state): State<Arc<AppState>>,
) -> Result<Html<String>, AppError> {
	let provider = provider_for_active_account(&state, &id)?;
	if provider.provider_type != "openai" {
		return Err(AppError::BadRequest);
	}
	state.database.delete_openai_credit_event(&provider.id, event_id)?;
	Ok(Html(render_openai_credit_ledger(&state, provider)?))
}

fn render_openai_credit_ledger(state: &AppState, provider: ProviderConfig) -> Result<String, AppError> {
	let credit_events = state.database.list_openai_credit_events(&provider.id)?;
	Ok(OpenAiCreditLedgerTemplate {
		provider,
		credit_events,
	}
	.render()?)
}

pub(super) async fn delete_confirmation(
	Path(id): Path<String>,
	State(state): State<Arc<AppState>>,
) -> Result<Html<String>, AppError> {
	let provider = provider_for_active_account(&state, &id)?;
	Ok(Html(DeleteProviderTemplate { provider }.render()?))
}

pub(super) async fn delete_provider(
	Path(id): Path<String>,
	State(state): State<Arc<AppState>>,
) -> Result<Redirect, AppError> {
	let provider = provider_for_active_account(&state, &id)?;
	state.database.delete_provider(&provider.id, &provider.account_id)?;
	Ok(Redirect::to("/providers"))
}

pub(super) async fn refresh_all_providers(
	State(state): State<Arc<AppState>>,
	Query(query): Query<MonthQuery>,
) -> Result<Html<String>, AppError> {
	let period = selected_period(query.month.as_deref())?;
	let account = state.database.active_account()?.ok_or(AppError::BadRequest)?;
	let mut succeeded = 0;
	let mut failed = 0;
	for provider in state.database.list_providers(&account.id)? {
		let mut provider = provider;
		match normalize_secret_reference(&account.project_id, &provider.secret_ref) {
			Ok(reference) => provider.secret_ref = reference,
			Err(_) => {
				let message = "Secret name or Secret Manager reference is invalid for the active Google project.";
				state.database.set_provider_refresh_error(&provider.id, Some(message))?;
				state.database.save_run_log(&provider.id, "failed", message)?;
				failed += 1;
				continue;
			}
		}
		if let Err(error) = refresh_provider_usage(&state, &provider, period).await {
			state.database.set_provider_refresh_error(&provider.id, Some(&error))?;
			state.database.save_run_log(&provider.id, "failed", &error)?;
			failed += 1;
		} else {
			state.database.save_run_log(
				&provider.id,
				"succeeded",
				&format!("{} usage refresh completed successfully.", period_label(period)),
			)?;
			succeeded += 1;
		}
	}
	Ok(Html(
		RefreshCompleteTemplate {
			succeeded,
			failed,
			period_label: period_label(period),
			provider_list_url: format!("/providers/list?month={}", month_value(period)),
		}
		.render()?,
	))
}

pub(super) async fn provider_list(
	State(state): State<Arc<AppState>>,
	Query(query): Query<MonthQuery>,
) -> Result<Html<String>, AppError> {
	let period = selected_period(query.month.as_deref())?;
	let cards_html = match state.database.active_account()? {
		Some(account) => dashboard::render_cards(&state, &account.id, period)?,
		None => String::new(),
	};
	Ok(Html(ProviderListTemplate { cards_html }.render()?))
}

fn provider_for_active_account(state: &AppState, id: &str) -> Result<ProviderConfig, AppError> {
	let provider = state.database.find_provider(id)?.ok_or(AppError::NotFound)?;
	let account = state.database.active_account()?.ok_or(AppError::NotFound)?;
	if provider.account_id != account.id {
		return Err(AppError::NotFound);
	}
	Ok(provider)
}

async fn refresh_provider_usage(
	state: &AppState,
	provider: &ProviderConfig,
	period: UsagePeriod,
) -> Result<(), String> {
	let secret = state
        .secret_resolver
        .resolve(&provider.secret_ref)
        .await
        .map_err(|error| match error {
            crate::secrets::SecretError::InvalidReference => {
                "Secret Manager reference is invalid.".to_owned()
            }
            crate::secrets::SecretError::AuthenticationFailed => "Google Application Default Credentials are unavailable. Sign in again and check credentials before refreshing.".to_owned(),
            crate::secrets::SecretError::AccessFailed => "Google Cloud could not access this Secret Manager secret. Check that the ADC identity has Secret Manager Secret Accessor on this secret and that the Secret Manager API is enabled.".to_owned(),
            crate::secrets::SecretError::InvalidPayload => "Google Cloud returned a Secret Manager value that could not be read safely.".to_owned(),
        })?;
	let snapshot = state.providers.collect(provider, &secret, period).await.map_err(|error| {
		format!(
			"{} {} refresh failed: {error}",
			provider.display_name,
			period_label(period)
		)
	})?;
	state
		.database
		.save_snapshot(&snapshot)
		.map_err(|_| format!("{} usage could not be saved to SQLite", period_label(period)))?;
	state
		.database
		.set_provider_refresh_error(&provider.id, None)
		.map_err(|_| "Usage was collected but refresh status could not be saved.".to_owned())?;
	Ok(())
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn normalizes_a_short_secret_name_to_the_active_project() {
		assert_eq!(
			normalize_secret_reference("usage-project", "OPENAI_ADMIN_KEY").unwrap(),
			"projects/usage-project/secrets/OPENAI_ADMIN_KEY/versions/latest"
		);
	}

	#[test]
	fn rejects_a_reference_for_another_project() {
		assert!(
			normalize_secret_reference(
				"usage-project",
				"projects/other-project/secrets/OPENAI_ADMIN_KEY/versions/latest"
			)
			.is_err()
		);
	}

	#[test]
	fn credit_allowance_must_be_positive_and_finite() {
		assert!(valid_credit_allowance(Some(19.0)));
		assert!(!valid_credit_allowance(Some(0.0)));
		assert!(!valid_credit_allowance(Some(f64::NAN)));
	}
}
