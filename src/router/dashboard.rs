use super::AppState;
use crate::utils::{
	credit_usage_percent, current_period, format_count, format_email_count, format_signed_usd, format_usd, month_url,
	next_period, percent_of, previous_period, sparkline_points,
};
use crate::{
	dtos::*,
	models::{Account, ProviderConfig, UsagePeriod, UsageSnapshot},
	secrets::check_application_default_credentials,
	utils::is_htmx_navigation,
};
use askama::Template;
use axum::{
	Router,
	extract::{Query, State},
	http::HeaderMap,
	response::Html,
	routing::get,
};

use chrono::Utc;
use std::sync::Arc;

pub(super) use crate::utils::{month_from_url, month_value, period_label, selected_period};

pub fn router() -> Router<Arc<AppState>> {
	Router::new().route("/", get(overview)).route("/dashboard", get(dashboard))
}

pub(super) async fn overview(State(state): State<Arc<AppState>>, headers: HeaderMap) -> Result<Html<String>, AppError> {
	render_overview(&state, !is_htmx_navigation(&headers))
}

pub(super) fn render_overview(state: &AppState, validate_authentication: bool) -> Result<Html<String>, AppError> {
	let account = account_for_render(state)?;
	let validate_authentication =
		account.as_ref().is_some_and(|account| account.auth_status == "ready") && validate_authentication;
	let summary = account
		.as_ref()
		.map(|account| overview_summary(state, &account.id, current_period()))
		.transpose()?;
	Ok(Html(
		OverviewTemplate {
			account,
			summary,
			validate_authentication,
		}
		.render()?,
	))
}

fn overview_summary(state: &AppState, account_id: &str, period: UsagePeriod) -> Result<OverviewSummary, AppError> {
	let providers = state.database.list_providers(account_id)?;
	let mut tracked_spend = 0.0;
	let mut nearing_limit = 0_usize;
	let mut newest_refresh = None;
	let provider_usage = providers
		.into_iter()
		.map(|provider| {
			let snapshot = state.database.snapshot_for_period(&provider.id, period)?;
			if let Some(snapshot) = &snapshot {
				tracked_spend += snapshot.cost.unwrap_or_default().max(0.0);
				newest_refresh = newest_refresh.max(snapshot.timestamp.parse::<i64>().ok());
			}
			let usage = overview_provider_usage(state, &provider, snapshot.as_ref())?;
			if usage.is_nearing_limit {
				nearing_limit += 1;
			}
			Ok(usage)
		})
		.collect::<Result<Vec<_>, AppError>>()?;
	Ok(OverviewSummary {
		tracked_spend: format_usd(tracked_spend),
		nearing_limit: format!("{nearing_limit:02}"),
		connected_count: provider_usage.len(),
		last_refreshed: newest_refresh
			.map(|timestamp| {
				chrono::DateTime::from_timestamp(timestamp, 0)
					.map(|date| date.format("%b %-d, %H:%M UTC").to_string())
					.unwrap_or_else(|| "Not refreshed".to_owned())
			})
			.unwrap_or_else(|| "Not refreshed".to_owned()),
		provider_usage,
	})
}

fn overview_provider_usage(
	state: &AppState,
	provider: &ProviderConfig,
	snapshot: Option<&UsageSnapshot>,
) -> Result<OverviewProviderUsage, AppError> {
	let (label, used, limit, percent) = match (provider.provider_type.as_str(), snapshot) {
		("openai", _) => state
			.database
			.openai_credit_totals(&provider.id)?
			.map(|(credit, spent)| {
				(
					"Net credit used",
					format_usd(spent),
					format_usd(credit),
					percent_of(spent, credit),
				)
			})
			.or_else(|| {
				snapshot.and_then(|snapshot| {
					snapshot
						.metrics
						.iter()
						.find(|metric| metric.id == "organization_spend_limit")
						.map(|metric| {
							let limit = metric.limit.unwrap_or_default();
							(
								"Organization costs",
								format_usd(metric.used),
								format_usd(limit),
								percent_of(metric.used, limit),
							)
						})
				})
			})
			.unwrap_or(("Net credit used", "—".to_owned(), "No credit balance".to_owned(), 0.0)),
		("apify", Some(snapshot)) => snapshot
			.metrics
			.iter()
			.find(|metric| metric.id == "monthly_credit_allowance")
			.and_then(|metric| metric.limit.map(|limit| (metric.used, limit)))
			.map(|(used, limit)| {
				(
					"Monthly credit use",
					format_usd(used),
					format_usd(limit),
					percent_of(used, limit),
				)
			})
			.unwrap_or(("Monthly credit use", "—".to_owned(), "No allowance".to_owned(), 0.0)),
		("resend", Some(snapshot)) => {
			let sent = snapshot
				.metrics
				.iter()
				.find(|metric| metric.id == "emails_sent_current_month")
				.map_or(0.0, |metric| metric.used);
			let received = snapshot
				.metrics
				.iter()
				.find(|metric| metric.id == "emails_received_current_month")
				.map_or(0.0, |metric| metric.used);
			let used = sent + received;
			let limit = provider.monthly_quota as f64;
			(
				"Emails sent + received",
				format_email_count(used),
				format_email_count(limit),
				percent_of(used, limit),
			)
		}
		(_, _) => ("Current-month usage", "—".to_owned(), "Not refreshed".to_owned(), 0.0),
	};
	let percent = percent.clamp(0.0, 100.0);
	Ok(OverviewProviderUsage {
		name: provider.display_name.clone(),
		provider_type: provider.provider_type.clone(),
		label: label.to_owned(),
		used,
		limit,
		percent: format!("{percent:.0}"),
		is_nearing_limit: percent >= 80.0,
	})
}

pub(super) async fn dashboard(
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

pub(super) fn render_dashboard(state: &AppState) -> Result<Html<String>, AppError> {
	render_dashboard_with_auth_validation(state, false, current_period())
}

pub(super) fn render_dashboard_with_auth_error(state: &AppState, error: &str) -> Result<Html<String>, AppError> {
	render_dashboard_page(state, false, current_period(), Some(error))
}

pub(super) fn render_dashboard_with_auth_validation(
	state: &AppState,
	validate_authentication: bool,
	period: UsagePeriod,
) -> Result<Html<String>, AppError> {
	render_dashboard_page(state, validate_authentication, period, None)
}

fn render_dashboard_page(
	state: &AppState,
	validate_authentication: bool,
	period: UsagePeriod,
	auth_error: Option<&str>,
) -> Result<Html<String>, AppError> {
	let mut account = account_for_render(state)?;
	if let Some(account) = account.as_mut()
		&& let Some(error) = auth_error
	{
		account.auth_error = Some(error.to_owned());
	}
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

pub(super) fn account_for_render(state: &AppState) -> Result<Option<Account>, AppError> {
	let mut account = state.database.active_account()?;
	if let Some(account) = account.as_mut()
		&& account.auth_error.as_deref() == Some(AUTH_CHECK_ERROR)
	{
		account.auth_error = None;
	}
	Ok(account)
}

/// Setup progress is persisted, but usable ADC is checked each time the dashboard opens.
pub(super) async fn refresh_saved_authentication(state: &AppState) -> Result<(), AppError> {
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

pub(super) fn render_cards(state: &AppState, account_id: &str, period: UsagePeriod) -> Result<String, AppError> {
	state
		.database
		.list_providers(account_id)?
		.into_iter()
		.map(|provider| {
			let snapshot = state.database.snapshot_for_period(&provider.id, period)?;
			let openai_activity = openai_activity_summary(state, &provider, period)?;
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
