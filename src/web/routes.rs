use std::sync::Arc;

use askama::Template;
use axum::{
	Form, Router,
	extract::{Path, Query, State},
	http::HeaderMap,
	response::{Html, IntoResponse, Redirect},
	routing::{get, post},
};
use chrono::{Datelike, NaiveDate, Utc};
use serde::Deserialize;

use crate::{
	models::{
		Account, NewAccount, NewOpenAiCreditEvent, NewProvider, OpenAiCreditEvent, ProviderConfig, RunLog,
		UpdateAccount, UpdateProvider, UsagePeriod, UsageSnapshot,
	},
	secrets::{begin_authentication, check_application_default_credentials},
	web::AppState,
};

pub fn router() -> Router<Arc<AppState>> {
	Router::new()
		.route("/", get(index))
		.route("/providers", get(providers_page).post(create_provider))
		.route("/runlogs", get(run_logs))
		.route("/health", get(|| async { "ok" }))
		.route("/authentication/validate", post(validate_saved_authentication))
		.route("/onboarding/account", post(create_account))
		.route("/onboarding/auth", post(start_auth))
		.route("/onboarding/auth/check", post(check_auth))
		.route("/onboarding/alerts/skip", post(skip_alerts))
		.route("/account/settings", get(account_settings))
		.route("/account", post(update_account))
		.route("/providers/list", get(provider_list))
		.route("/providers/refresh", post(refresh_all_providers))
		.route("/providers/new", get(new_provider_form))
		.route("/providers/{id}/edit", get(edit_provider))
		.route("/providers/{id}", post(update_provider))
		.route("/providers/{id}/credit-events", post(add_openai_credit_event))
		.route(
			"/providers/{id}/credit-events/{event_id}/delete",
			post(delete_openai_credit_event),
		)
		.route("/providers/{id}/delete", post(delete_provider))
		.route("/providers/{id}/delete/confirm", get(delete_confirmation))
		.fallback(not_found)
}

#[derive(Default, Deserialize)]
struct MonthQuery {
	month: Option<String>,
}

fn current_period() -> UsagePeriod {
	let today = Utc::now().date_naive();
	period_for_month(today.year(), today.month(), true).expect("the current UTC month is valid")
}

fn selected_period(value: Option<&str>) -> Result<UsagePeriod, AppError> {
	let current = current_period();
	let Some(value) = value else {
		return Ok(current);
	};
	let date = NaiveDate::parse_from_str(&format!("{value}-01"), "%Y-%m-%d").map_err(|_| AppError::BadRequest)?;
	let period = period_for_month(date.year(), date.month(), false).ok_or(AppError::BadRequest)?;
	if period.start > current.start {
		return Err(AppError::BadRequest);
	}
	Ok(UsagePeriod {
		is_current: period.start == current.start,
		..period
	})
}

fn period_for_month(year: i32, month: u32, is_current: bool) -> Option<UsagePeriod> {
	let start = NaiveDate::from_ymd_opt(year, month, 1)?;
	let (next_year, next_month) = if month == 12 { (year + 1, 1) } else { (year, month + 1) };
	let end = NaiveDate::from_ymd_opt(next_year, next_month, 1)?;
	Some(UsagePeriod {
		start: start.and_hms_opt(0, 0, 0)?.and_utc().timestamp(),
		end: end.and_hms_opt(0, 0, 0)?.and_utc().timestamp(),
		is_current,
	})
}

fn previous_period(period: UsagePeriod) -> UsagePeriod {
	let start = chrono::DateTime::from_timestamp(period.start, 0)
		.expect("stored period timestamp is valid")
		.date_naive();
	let (year, month) = if start.month() == 1 {
		(start.year() - 1, 12)
	} else {
		(start.year(), start.month() - 1)
	};
	period_for_month(year, month, false).expect("adjacent month is valid")
}

fn next_period(period: UsagePeriod) -> UsagePeriod {
	let end = chrono::DateTime::from_timestamp(period.end, 0)
		.expect("stored period timestamp is valid")
		.date_naive();
	let current = current_period();
	let next = period_for_month(end.year(), end.month(), false).expect("adjacent month is valid");
	UsagePeriod {
		is_current: next.start == current.start,
		..next
	}
}

fn period_label(period: UsagePeriod) -> String {
	chrono::DateTime::from_timestamp(period.start, 0)
		.expect("stored period timestamp is valid")
		.format("%B %Y")
		.to_string()
}

fn month_url(period: UsagePeriod) -> String {
	format!("/?month={}", month_value(period))
}

fn month_value(period: UsagePeriod) -> String {
	chrono::DateTime::from_timestamp(period.start, 0)
		.expect("stored period timestamp is valid")
		.format("%Y-%m")
		.to_string()
}

fn month_from_url(url: &str) -> Option<&str> {
	url.split_once('?')
		.and_then(|(_, query)| query.split('&').find_map(|parameter| parameter.strip_prefix("month=")))
}

async fn not_found() -> Result<(axum::http::StatusCode, Html<String>), AppError> {
	Ok((axum::http::StatusCode::NOT_FOUND, Html(NotFoundTemplate.render()?)))
}

async fn index(
	State(state): State<Arc<AppState>>,
	headers: HeaderMap,
	Query(query): Query<MonthQuery>,
) -> Result<Html<String>, AppError> {
	let validate_authentication = state
		.database
		.active_account()?
		.is_some_and(|account| account.auth_status == "ready")
		&& !is_htmx_navigation(&headers);
	render_dashboard_with_auth_validation(
		&state,
		validate_authentication,
		selected_period(query.month.as_deref())?,
	)
}

async fn run_logs(State(state): State<Arc<AppState>>, headers: HeaderMap) -> Result<Html<String>, AppError> {
	render_run_logs(&state, !is_htmx_navigation(&headers))
}

async fn providers_page(State(state): State<Arc<AppState>>, headers: HeaderMap) -> Result<Html<String>, AppError> {
	render_providers_page(&state, !is_htmx_navigation(&headers))
}

fn render_providers_page(state: &AppState, validate_authentication: bool) -> Result<Html<String>, AppError> {
	let account = state.database.active_account()?.ok_or(AppError::NotFound)?;
	let providers = state.database.list_providers(&account.id)?;
	Ok(Html(
		ProvidersTemplate {
			validate_authentication: account.auth_status == "ready" && validate_authentication,
			account,
			providers,
		}
		.render()?,
	))
}

fn render_run_logs(state: &AppState, validate_authentication: bool) -> Result<Html<String>, AppError> {
	let account = state.database.active_account()?.ok_or(AppError::NotFound)?;
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

fn is_htmx_navigation(headers: &HeaderMap) -> bool {
	headers.get("HX-Request").is_some_and(|value| value == "true")
}

async fn validate_saved_authentication(
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
	let month = current_url.and_then(month_from_url);
	render_dashboard_with_auth_validation(&state, false, selected_period(month)?)
}

async fn create_account(
	State(state): State<Arc<AppState>>,
	Form(input): Form<NewAccount>,
) -> Result<Html<String>, AppError> {
	if !valid_project_id(&input.project_id) {
		return Err(AppError::BadRequest);
	}
	let account = state.database.create_account(input)?;
	Ok(Html(AuthRequiredTemplate { account }.render()?))
}

async fn account_settings(State(state): State<Arc<AppState>>) -> Result<Html<String>, AppError> {
	let account = state.database.active_account()?.ok_or(AppError::NotFound)?;
	Ok(Html(AccountSettingsTemplate { account }.render()?))
}

async fn update_account(
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
	render_dashboard(&state)
}

async fn start_auth(State(state): State<Arc<AppState>>) -> Result<Html<String>, AppError> {
	let account = state.database.active_account()?.ok_or(AppError::BadRequest)?;
	state.database.set_auth_status(&account.id, "authenticating", None, None)?;
	tokio::spawn(async move {
		let _ = begin_authentication(&account.project_id).await;
	});
	let account = state.database.active_account()?.ok_or(AppError::BadRequest)?;
	Ok(Html(AuthRequiredTemplate { account }.render()?))
}

async fn check_auth(State(state): State<Arc<AppState>>) -> Result<Html<String>, AppError> {
	let account = state.database.active_account()?.ok_or(AppError::BadRequest)?;
	match check_application_default_credentials().await {
		Ok(()) => {
			state
				.database
				.set_auth_status(&account.id, "ready", Some(&account.project_id), None)?;
			state.database.set_onboarding_step(&account.id, 4)?;
		}
		Err(_) => state.database.set_auth_status(
			&account.id,
			"failed",
			None,
			Some("Google credentials are not ready yet. Complete sign-in, then check again."),
		)?,
	}
	render_dashboard(&state)
}

async fn new_provider_form(State(state): State<Arc<AppState>>) -> Result<Html<String>, AppError> {
	state.database.active_account()?.ok_or(AppError::BadRequest)?;
	Ok(Html(NewProviderDialogTemplate.render()?))
}

async fn create_provider(
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
		|| !matches!(
			input.provider_type.as_str(),
			"apify" | "openai" | "neon" | "upstash" | "resend"
		) || input.display_name.trim().is_empty()
		|| input.secret_ref.trim().is_empty()
	{
		return Err(AppError::BadRequest);
	}
	let is_resend = input.provider_type == "resend";
	let is_apify = input.provider_type == "apify";
	if is_resend
		&& (input.plan.as_deref().is_none_or(str::is_empty)
			|| input.monthly_quota.unwrap_or(0) <= 0
			|| input.daily_quota.unwrap_or(0) <= 0)
	{
		return Err(AppError::BadRequest);
	}
	if is_apify && !valid_credit_allowance(input.apify_monthly_credit_allowance) {
		return Err(AppError::BadRequest);
	}
	if !valid_secret_name(input.secret_ref.trim()) {
		return Err(AppError::BadRequest);
	}
	state.database.add_provider(&account.id, input)?;
	state.database.set_onboarding_step(&account.id, if is_resend { 4 } else { 3 })?;
	render_providers_page(&state, false)
}

async fn edit_provider(Path(id): Path<String>, State(state): State<Arc<AppState>>) -> Result<Html<String>, AppError> {
	let provider = provider_for_active_account(&state, &id)?;
	let credit_events = state.database.list_openai_credit_events(&provider.id)?;
	Ok(Html(
		EditProviderTemplate {
			provider,
			credit_events,
		}
		.render()?,
	))
}

async fn update_provider(
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
	if provider.provider_type == "resend" {
		if input.plan.as_deref().is_none_or(str::is_empty)
			|| input.monthly_quota.unwrap_or(0) <= 0
			|| input.daily_quota.unwrap_or(0) <= 0
		{
			return Err(AppError::BadRequest);
		}
	} else if provider.provider_type == "apify" {
		if !valid_credit_allowance(input.apify_monthly_credit_allowance) {
			return Err(AppError::BadRequest);
		}
		input.plan = Some(provider.plan.clone());
		input.monthly_quota = Some(provider.monthly_quota);
		input.daily_quota = Some(provider.daily_quota);
	} else {
		input.plan = Some(provider.plan.clone());
		input.monthly_quota = Some(provider.monthly_quota);
		input.daily_quota = Some(provider.daily_quota);
		input.apify_monthly_credit_allowance = Some(provider.apify_monthly_credit_allowance);
	}
	state.database.update_provider(&provider.id, &provider.account_id, input)?;
	render_providers_page(&state, false)
}

fn valid_credit_allowance(value: Option<f64>) -> bool {
	value.is_some_and(|amount| amount.is_finite() && amount > 0.0)
}

async fn add_openai_credit_event(
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
	edit_provider(Path(id), State(state)).await
}

async fn delete_openai_credit_event(
	Path((id, event_id)): Path<(String, i64)>,
	State(state): State<Arc<AppState>>,
) -> Result<Html<String>, AppError> {
	let provider = provider_for_active_account(&state, &id)?;
	if provider.provider_type != "openai" {
		return Err(AppError::BadRequest);
	}
	state.database.delete_openai_credit_event(&provider.id, event_id)?;
	edit_provider(Path(id), State(state)).await
}

async fn delete_confirmation(
	Path(id): Path<String>,
	State(state): State<Arc<AppState>>,
) -> Result<Html<String>, AppError> {
	let provider = provider_for_active_account(&state, &id)?;
	Ok(Html(DeleteProviderTemplate { provider }.render()?))
}

async fn delete_provider(Path(id): Path<String>, State(state): State<Arc<AppState>>) -> Result<Redirect, AppError> {
	let provider = provider_for_active_account(&state, &id)?;
	state.database.delete_provider(&provider.id, &provider.account_id)?;
	Ok(Redirect::to("/providers"))
}

async fn refresh_all_providers(
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

async fn provider_list(
	State(state): State<Arc<AppState>>,
	Query(query): Query<MonthQuery>,
) -> Result<Html<String>, AppError> {
	let period = selected_period(query.month.as_deref())?;
	let cards_html = match state.database.active_account()? {
		Some(account) => render_cards(&state, &account.id, period)?,
		None => String::new(),
	};
	Ok(Html(ProviderListTemplate { cards_html }.render()?))
}

async fn skip_alerts(State(state): State<Arc<AppState>>) -> Result<Html<String>, AppError> {
	let account = state.database.active_account()?.ok_or(AppError::BadRequest)?;
	state.database.set_onboarding_step(&account.id, 4)?;
	render_dashboard(&state)
}

fn render_dashboard(state: &AppState) -> Result<Html<String>, AppError> {
	render_dashboard_with_auth_validation(state, false, current_period())
}

fn render_dashboard_with_auth_validation(
	state: &AppState,
	validate_authentication: bool,
	period: UsagePeriod,
) -> Result<Html<String>, AppError> {
	let account = state.database.active_account()?;
	let show_dashboard_skeleton = account
		.as_ref()
		.is_some_and(|account| validate_authentication || account.auth_status != "ready");
	// Loading the project display name here ensures the authenticated project
	// metadata remains part of the dashboard state, ready for the header UI.
	let _project_name = account.as_ref().and_then(|item| item.project_name.as_deref());
	let cards_html = match (&account, show_dashboard_skeleton) {
		(Some(account), false) => render_cards(state, &account.id, period)?,
		_ => String::new(),
	};
	Ok(Html(
		DashboardTemplate {
			account,
			cards_html,
			selected_month: period_label(period),
			previous_month_url: month_url(previous_period(period)),
			next_month_url: (!period.is_current).then(|| month_url(next_period(period))),
			refresh_url: format!("/providers/refresh?month={}", month_value(period)),
			show_dashboard_skeleton,
			validate_authentication,
		}
		.render()?,
	))
}

/// Setup progress is persisted, but usable ADC is checked each time the dashboard opens.
async fn refresh_saved_authentication(state: &AppState) -> Result<(), AppError> {
	let Some(account) = state.database.active_account()? else {
		return Ok(());
	};
	if account.auth_status != "ready" {
		return Ok(());
	}
	if check_application_default_credentials().await.is_err() {
		state.database.set_auth_status(
			&account.id,
			"failed",
			None,
			Some("Google Application Default Credentials are unavailable. Authenticate with Google to continue."),
		)?;
	}
	Ok(())
}

fn render_cards(state: &AppState, account_id: &str, period: UsagePeriod) -> Result<String, AppError> {
	state
		.database
		.list_providers(account_id)?
		.into_iter()
		.map(|provider| {
			let snapshot = state.database.snapshot_for_period(&provider.id, period)?;
			let openai_activity = openai_activity_summary(state, &provider, period)?;
			let openai_period = openai_period_label(&provider, period);
			let mut openai_spend_limit = openai_spend_limit_summary(&provider, snapshot.as_ref());
			if let (Some(spend_limit), Some(activity)) = (openai_spend_limit.as_mut(), openai_activity.as_ref()) {
				spend_limit.projects.clone_from(&activity.projects);
				spend_limit.spend_points.clone_from(&activity.spend_points);
			}
			ProviderCardTemplate {
				apify_credit: apify_credit_summary(&provider, snapshot.as_ref()),
				openai_spend_limit,
				openai_credit: if period.is_current {
					openai_credit_summary(&provider, state.database.openai_credit_totals(&provider.id)?)
				} else {
					None
				},
				openai_activity,
				openai_period,
				resend_quota: resend_quota_summary(&provider, snapshot.as_ref()),
				resend_daily_quota: resend_daily_quota_summary(&provider, snapshot.as_ref()),
				provider,
				snapshot,
			}
			.render()
			.map_err(AppError::from)
		})
		.collect::<Result<Vec<_>, AppError>>()
		.map(|cards| cards.join("\n"))
}

fn openai_period_label(provider: &ProviderConfig, period: UsagePeriod) -> Option<String> {
	if provider.provider_type != "openai" {
		return None;
	}
	let start = chrono::DateTime::from_timestamp(period.start, 0)?;
	let last_day = if period.is_current {
		Utc::now().day()
	} else {
		chrono::DateTime::from_timestamp(period.end - 1, 0)?.day()
	};
	Some(format!(
		"{} · {} 1–{}, {} UTC",
		if period.is_current { "Current month" } else { "Selected month" },
		start.format("%B"),
		last_day,
		start.year()
	))
}

fn openai_activity_summary(
	state: &AppState,
	provider: &ProviderConfig,
	period: UsagePeriod,
) -> Result<Option<OpenAiActivitySummaryView>, AppError> {
	if provider.provider_type != "openai" {
		return Ok(None);
	}
	let start = period.start;
	let end = if period.is_current { Utc::now().timestamp() + 1 } else { period.end };
	let daily_activity = state.database.openai_daily_activity(&provider.id, start, end)?;
	let daily_spend = state.database.openai_daily_spend(&provider.id, start, end)?;
	let project_spend = state.database.openai_project_spend(&provider.id, start, end)?;
	if daily_activity.is_empty() && daily_spend.is_empty() && project_spend.is_empty() {
		return Ok(None);
	}
	let activity = state.database.openai_activity_summary(&provider.id, start, end)?;
	let activity_available = !daily_activity.is_empty();
	let total_project_spend: f64 = project_spend.iter().map(|project| project.amount).sum();
	let projects = project_spend
		.into_iter()
		.map(|project| OpenAiProjectSpendView {
			name: project.project_id.unwrap_or_else(|| "Default project".to_owned()),
			amount: format_signed_usd(project.amount),
			percent: if total_project_spend > 0.0 {
				format!(
					"{:.1}",
					(project.amount / total_project_spend * 100.0).clamp(0.0, 100.0)
				)
			} else {
				"0".to_owned()
			},
		})
		.collect();
	let token_values = daily_activity
		.iter()
		.map(|day| (day.input_tokens + day.output_tokens) as f64)
		.collect::<Vec<_>>();
	let request_values = daily_activity.iter().map(|day| day.request_count as f64).collect::<Vec<_>>();
	let cache_values = daily_activity
		.iter()
		.map(|day| {
			if day.input_tokens == 0 {
				0.0
			} else {
				day.cached_input_tokens as f64 / day.input_tokens as f64 * 100.0
			}
		})
		.collect::<Vec<_>>();
	let spend_values = daily_spend.iter().map(|day| day.amount).collect::<Vec<_>>();

	Ok(Some(OpenAiActivitySummaryView {
		activity_available,
		total_tokens: format_count(activity.input_tokens + activity.output_tokens),
		input_tokens: format_count(activity.input_tokens),
		output_tokens: format_count(activity.output_tokens),
		requests: format_count(activity.request_count),
		cache_hit_rate: activity
			.cache_hit_rate
			.map(|rate| format!("{rate:.1}%"))
			.unwrap_or_else(|| "—".to_owned()),
		token_points: sparkline_points(&token_values),
		request_points: sparkline_points(&request_values),
		cache_points: sparkline_points(&cache_values),
		spend_points: sparkline_points(&spend_values),
		projects,
	}))
}

fn sparkline_points(values: &[f64]) -> String {
	if values.is_empty() {
		return "0,36 100,36".to_owned();
	}
	if values.len() == 1 {
		return "0,20 100,20".to_owned();
	}
	let minimum = values.iter().copied().fold(f64::INFINITY, f64::min);
	let maximum = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
	values
		.iter()
		.enumerate()
		.map(|(index, value)| {
			let x = index as f64 / (values.len() - 1) as f64 * 100.0;
			let y = if (maximum - minimum).abs() < f64::EPSILON {
				20.0
			} else {
				36.0 - ((value - minimum) / (maximum - minimum) * 32.0)
			};
			format!("{x:.1},{y:.1}")
		})
		.collect::<Vec<_>>()
		.join(" ")
}

fn apify_credit_summary(provider: &ProviderConfig, snapshot: Option<&UsageSnapshot>) -> Option<ApifyCreditSummary> {
	if provider.provider_type != "apify" {
		return None;
	}
	let snapshot = snapshot?;
	let allowance = snapshot.metrics.iter().find(|metric| metric.id == "monthly_credit_allowance")?;
	let remaining = snapshot.metrics.iter().find(|metric| metric.id == "monthly_credit_remaining")?;
	let percent = snapshot
		.metrics
		.iter()
		.find(|metric| metric.id == "monthly_credit_used_percent")?;
	Some(ApifyCreditSummary {
		used: format_usd(allowance.used),
		limit: format_usd(allowance.limit?),
		remaining: format_signed_usd(remaining.used),
		percent_used: format!("{:.1}", percent.used),
	})
}

fn openai_spend_limit_summary(
	provider: &ProviderConfig,
	snapshot: Option<&UsageSnapshot>,
) -> Option<OpenAiSpendLimitSummary> {
	if provider.provider_type != "openai" {
		return None;
	}
	let metric = snapshot?
		.metrics
		.iter()
		.find(|metric| metric.id == "organization_spend_limit")?;
	Some(OpenAiSpendLimitSummary {
		used: format_usd(metric.used),
		limit: metric.limit.map(format_usd),
		percent: metric
			.limit
			.filter(|limit| *limit > 0.0)
			.map(|limit| format!("{:.1}", (metric.used / limit * 100.0).clamp(0.0, 100.0)))
			.unwrap_or_else(|| "0".to_owned()),
		spend_points: "0,36 100,36".to_owned(),
		projects: Vec::new(),
	})
}
fn openai_credit_summary(provider: &ProviderConfig, totals: Option<(f64, f64)>) -> Option<OpenAiCreditSummary> {
	if provider.provider_type != "openai" {
		return None;
	}
	let (credit, spent) = totals?;
	Some(OpenAiCreditSummary {
		used: format_usd(spent),
		total: format_signed_usd(credit),
		remaining: format_signed_usd(credit - spent),
		percent: format!("{:.1}", credit_usage_percent(spent, credit)),
	})
}

fn credit_usage_percent(spent: f64, credit: f64) -> f64 {
	if credit <= 0.0 { 0.0 } else { (spent / credit * 100.0).clamp(0.0, 100.0) }
}

fn format_usd(value: f64) -> String {
	format!("${:.2}", value.max(0.0))
}

fn format_signed_usd(value: f64) -> String {
	if value < 0.0 { format!("-${:.2}", value.abs()) } else { format_usd(value) }
}

fn resend_quota_summary(provider: &ProviderConfig, snapshot: Option<&UsageSnapshot>) -> Option<ResendQuotaSummary> {
	if provider.provider_type != "resend" || provider.monthly_quota <= 0 {
		return None;
	}
	let snapshot = snapshot?;
	let sent = snapshot
		.metrics
		.iter()
		.find(|metric| metric.id == "emails_sent_current_month")?
		.used;
	let received = snapshot
		.metrics
		.iter()
		.find(|metric| metric.id == "emails_received_current_month")?
		.used;
	let used = sent + received;
	quota_summary(provider.plan.clone(), used, provider.monthly_quota)
}

fn quota_summary(plan: String, used: f64, quota: i64) -> Option<ResendQuotaSummary> {
	let limit = quota as f64;
	Some(ResendQuotaSummary {
		plan,
		used: format_email_count(used),
		limit: format_email_count(limit),
		remaining: format_email_count((quota.saturating_sub(used as i64)) as f64),
		percent_used: format!("{:.1}", used / limit * 100.0),
	})
}

fn resend_daily_quota_summary(
	provider: &ProviderConfig,
	snapshot: Option<&UsageSnapshot>,
) -> Option<ResendQuotaSummary> {
	if provider.provider_type != "resend" || provider.daily_quota <= 0 {
		return None;
	}
	let snapshot = snapshot?;
	let sent = snapshot.metrics.iter().find(|metric| metric.id == "emails_sent_today")?.used;
	let received = snapshot
		.metrics
		.iter()
		.find(|metric| metric.id == "emails_received_today")?
		.used;
	quota_summary(provider.plan.clone(), sent + received, provider.daily_quota)
}

fn format_email_count(value: f64) -> String {
	let value = value.max(0.0).round() as i64;
	let digits = value.to_string();
	let mut formatted = String::with_capacity(digits.len() + digits.len() / 3);
	for (index, digit) in digits.chars().enumerate() {
		if index > 0 && (digits.len() - index).is_multiple_of(3) {
			formatted.push(',');
		}
		formatted.push(digit);
	}
	formatted
}

fn format_count(value: i64) -> String {
	let digits = value.max(0).to_string();
	let mut formatted = String::with_capacity(digits.len() + digits.len() / 3);
	for (index, digit) in digits.chars().enumerate() {
		if index > 0 && (digits.len() - index).is_multiple_of(3) {
			formatted.push(',');
		}
		formatted.push(digit);
	}
	formatted
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

fn valid_project_id(value: &str) -> bool {
	!value.is_empty()
		&& value.len() <= 63
		&& value
			.chars()
			.all(|character| character.is_ascii_lowercase() || character.is_ascii_digit() || character == '-')
}

fn normalize_secret_reference(project_id: &str, provided: &str) -> Result<String, AppError> {
	let value = provided.trim();
	if value.is_empty() {
		return Err(AppError::BadRequest);
	}
	let parts: Vec<_> = value.split('/').collect();
	match parts.as_slice() {
		["projects", project, "secrets", secret] if *project == project_id && valid_secret_name(secret) => {
			Ok(format!("projects/{project}/secrets/{secret}/versions/latest"))
		}
		["projects", project, "secrets", secret, "versions", version]
			if *project == project_id && valid_secret_name(secret) && !version.is_empty() =>
		{
			Ok(value.to_owned())
		}
		[secret] if valid_secret_name(secret) => Ok(format!("projects/{project_id}/secrets/{secret}/versions/latest")),
		_ => Err(AppError::BadRequest),
	}
}

fn valid_secret_name(value: &str) -> bool {
	!value.is_empty()
		&& value.len() <= 255
		&& value
			.chars()
			.all(|character| character.is_ascii_alphanumeric() || character == '_' || character == '-')
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

	#[test]
	fn openai_credit_usage_is_capped_for_the_progress_bar() {
		assert_eq!(credit_usage_percent(8.92, 10.0), 89.2);
		assert_eq!(credit_usage_percent(12.0, 10.0), 100.0);
		assert_eq!(credit_usage_percent(8.92, 0.0), 0.0);
	}

	#[test]
	fn formats_large_activity_counts() {
		assert_eq!(format_count(2_462_423), "2,462,423");
		assert_eq!(format_count(0), "0");
	}

	#[test]
	fn builds_normalized_sparkline_points() {
		assert_eq!(sparkline_points(&[]), "0,36 100,36");
		assert_eq!(sparkline_points(&[4.0]), "0,20 100,20");
		assert_eq!(sparkline_points(&[0.0, 10.0]), "0.0,36.0 100.0,4.0");
	}

	#[test]
	fn builds_calendar_month_boundaries_across_years_and_leap_days() {
		let december = period_for_month(2025, 12, false).unwrap();
		let january = next_period(december);
		assert_eq!(period_label(january), "January 2026");
		assert_eq!(previous_period(january), december);

		let february = period_for_month(2024, 2, false).unwrap();
		assert_eq!(february.end - february.start, 29 * 24 * 60 * 60);
	}

	#[test]
	fn preserves_selected_month_during_authentication_validation() {
		assert_eq!(month_from_url("http://127.0.0.1:5050/?month=2026-08"), Some("2026-08"));
		assert_eq!(month_from_url("http://127.0.0.1:5050/"), None);
	}
}

#[derive(Template)]
#[template(path = "pages/index.html")]
struct DashboardTemplate {
	account: Option<Account>,
	cards_html: String,
	selected_month: String,
	previous_month_url: String,
	next_month_url: Option<String>,
	refresh_url: String,
	show_dashboard_skeleton: bool,
	validate_authentication: bool,
}
#[derive(Template)]
#[template(path = "pages/providers.html")]
struct ProvidersTemplate {
	account: Account,
	providers: Vec<ProviderConfig>,
	validate_authentication: bool,
}
#[derive(Template)]
#[template(path = "pages/run_logs.html")]
struct RunLogsTemplate {
	account: Account,
	logs: Vec<RunLogView>,
	validate_authentication: bool,
}

struct RunLogView {
	provider_name: String,
	provider_type: String,
	created_at: String,
	status: String,
	message: String,
}
#[derive(Template)]
#[template(path = "pages/not_found.html")]
struct NotFoundTemplate;
#[derive(Template)]
#[template(path = "partials/auth_required.html")]
struct AuthRequiredTemplate {
	account: Account,
}
#[derive(Template)]
#[template(path = "partials/new_provider_dialog.html")]
struct NewProviderDialogTemplate;
#[derive(Template)]
#[template(path = "partials/provider_card.html")]
struct ProviderCardTemplate {
	provider: ProviderConfig,
	snapshot: Option<UsageSnapshot>,
	apify_credit: Option<ApifyCreditSummary>,
	openai_spend_limit: Option<OpenAiSpendLimitSummary>,
	openai_credit: Option<OpenAiCreditSummary>,
	openai_activity: Option<OpenAiActivitySummaryView>,
	openai_period: Option<String>,
	resend_quota: Option<ResendQuotaSummary>,
	resend_daily_quota: Option<ResendQuotaSummary>,
}

struct OpenAiActivitySummaryView {
	activity_available: bool,
	total_tokens: String,
	input_tokens: String,
	output_tokens: String,
	requests: String,
	cache_hit_rate: String,
	token_points: String,
	request_points: String,
	cache_points: String,
	spend_points: String,
	projects: Vec<OpenAiProjectSpendView>,
}

#[derive(Clone)]
struct OpenAiProjectSpendView {
	name: String,
	amount: String,
	percent: String,
}

struct ApifyCreditSummary {
	used: String,
	limit: String,
	remaining: String,
	percent_used: String,
}

struct OpenAiSpendLimitSummary {
	used: String,
	limit: Option<String>,
	percent: String,
	spend_points: String,
	projects: Vec<OpenAiProjectSpendView>,
}
struct OpenAiCreditSummary {
	used: String,
	total: String,
	remaining: String,
	percent: String,
}

struct ResendQuotaSummary {
	plan: String,
	used: String,
	limit: String,
	remaining: String,
	percent_used: String,
}
#[derive(Template)]
#[template(path = "partials/provider_list.html")]
struct ProviderListTemplate {
	cards_html: String,
}
#[derive(Template)]
#[template(path = "partials/edit_provider.html")]
struct EditProviderTemplate {
	provider: ProviderConfig,
	credit_events: Vec<OpenAiCreditEvent>,
}
#[derive(Template)]
#[template(path = "partials/delete_provider.html")]
struct DeleteProviderTemplate {
	provider: ProviderConfig,
}
#[derive(Template)]
#[template(path = "partials/account_settings.html")]
struct AccountSettingsTemplate {
	account: Account,
}
#[derive(Template)]
#[template(path = "partials/refresh_complete.html")]
struct RefreshCompleteTemplate {
	succeeded: usize,
	failed: usize,
	period_label: String,
	provider_list_url: String,
}

#[derive(Debug, thiserror::Error)]
enum AppError {
	#[error("database error")]
	Database(#[from] rusqlite::Error),
	#[error("template error")]
	Template(#[from] askama::Error),
	#[error("invalid request")]
	BadRequest,
	#[error("provider was not found")]
	NotFound,
}
impl IntoResponse for AppError {
	fn into_response(self) -> axum::response::Response {
		let status = match self {
			Self::BadRequest => axum::http::StatusCode::BAD_REQUEST,
			Self::NotFound => axum::http::StatusCode::NOT_FOUND,
			_ => axum::http::StatusCode::INTERNAL_SERVER_ERROR,
		};
		(status, self.to_string()).into_response()
	}
}
