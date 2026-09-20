use std::sync::Arc;

use askama::Template;
use axum::{
    Form, Router,
    extract::{Path, State},
    response::{Html, IntoResponse, Redirect},
    routing::{get, post},
};

use crate::{
    models::{
        Account, NewAccount, NewProvider, ProviderConfig, UpdateAccount, UpdateProvider,
        UsageSnapshot,
    },
    secrets::{begin_authentication, check_application_default_credentials},
    web::AppState,
};

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(index))
        .route("/health", get(|| async { "ok" }))
        .route(
            "/authentication/validate",
            post(validate_saved_authentication),
        )
        .route("/onboarding/account", post(create_account))
        .route("/onboarding/auth", post(start_auth))
        .route("/onboarding/auth/check", post(check_auth))
        .route("/onboarding/alerts/skip", post(skip_alerts))
        .route("/account/settings", get(account_settings))
        .route("/account", post(update_account))
        .route("/providers", post(create_provider))
        .route("/providers/list", get(provider_list))
        .route("/providers/refresh", post(refresh_all_providers))
        .route("/providers/new", get(new_provider_form))
        .route("/providers/{id}/edit", get(edit_provider))
        .route("/providers/{id}", post(update_provider))
        .route("/providers/{id}/delete", post(delete_provider))
        .route("/providers/{id}/delete/confirm", get(delete_confirmation))
}

async fn index(State(state): State<Arc<AppState>>) -> Result<Html<String>, AppError> {
    let validate_authentication = state
        .database
        .active_account()?
        .is_some_and(|account| account.auth_status == "ready");
    render_dashboard_with_auth_validation(&state, validate_authentication)
}

async fn validate_saved_authentication(
    State(state): State<Arc<AppState>>,
) -> Result<Html<String>, AppError> {
    refresh_saved_authentication(&state).await?;
    render_dashboard(&state)
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
    let account = state
        .database
        .active_account()?
        .ok_or(AppError::BadRequest)?;
    state
        .database
        .set_auth_status(&account.id, "authenticating", None, None)?;
    tokio::spawn(async move {
        let _ = begin_authentication(&account.project_id).await;
    });
    let account = state
        .database
        .active_account()?
        .ok_or(AppError::BadRequest)?;
    Ok(Html(AuthRequiredTemplate { account }.render()?))
}

async fn check_auth(State(state): State<Arc<AppState>>) -> Result<Html<String>, AppError> {
    let account = state
        .database
        .active_account()?
        .ok_or(AppError::BadRequest)?;
    match check_application_default_credentials().await {
        Ok(()) => {
            state.database.set_auth_status(
                &account.id,
                "ready",
                Some(&account.project_id),
                None,
            )?;
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
    state
        .database
        .active_account()?
        .ok_or(AppError::BadRequest)?;
    Ok(Html(NewProviderDialogTemplate.render()?))
}

async fn create_provider(
    State(state): State<Arc<AppState>>,
    Form(input): Form<NewProvider>,
) -> Result<Html<String>, AppError> {
    let account = state
        .database
        .active_account()?
        .ok_or(AppError::BadRequest)?;
    if check_application_default_credentials().await.is_err() {
        state.database.set_auth_status(
            &account.id,
            "failed",
            None,
            Some("Google Application Default Credentials are unavailable. Authenticate with Google to continue."),
        )?;
        return render_dashboard(&state);
    }
    if account.auth_status != "ready"
        || !matches!(
            input.provider_type.as_str(),
            "apify" | "openai" | "neon" | "upstash" | "resend"
        )
        || input.display_name.trim().is_empty()
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
    state
        .database
        .set_onboarding_step(&account.id, if is_resend { 4 } else { 3 })?;
    render_dashboard(&state)
}

async fn edit_provider(
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> Result<Html<String>, AppError> {
    let provider = provider_for_active_account(&state, &id)?;
    Ok(Html(EditProviderTemplate { provider }.render()?))
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
    state
        .database
        .update_provider(&provider.id, &provider.account_id, input)?;
    render_dashboard(&state)
}

fn valid_credit_allowance(value: Option<f64>) -> bool {
    value.is_some_and(|amount| amount.is_finite() && amount > 0.0)
}

async fn delete_confirmation(
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> Result<Html<String>, AppError> {
    let provider = provider_for_active_account(&state, &id)?;
    Ok(Html(DeleteProviderTemplate { provider }.render()?))
}

async fn delete_provider(
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> Result<Redirect, AppError> {
    let provider = provider_for_active_account(&state, &id)?;
    state
        .database
        .delete_provider(&provider.id, &provider.account_id)?;
    Ok(Redirect::to("/"))
}

async fn refresh_all_providers(
    State(state): State<Arc<AppState>>,
) -> Result<Html<String>, AppError> {
    let account = state
        .database
        .active_account()?
        .ok_or(AppError::BadRequest)?;
    let mut succeeded = 0;
    let mut failed = 0;
    for provider in state.database.list_providers(&account.id)? {
        let mut provider = provider;
        match normalize_secret_reference(&account.project_id, &provider.secret_ref) {
            Ok(reference) => provider.secret_ref = reference,
            Err(_) => {
                state.database.set_provider_refresh_error(&provider.id, Some("Secret name or Secret Manager reference is invalid for the active Google project."))?;
                failed += 1;
                continue;
            }
        }
        if let Err(error) = refresh_provider_usage(&state, &provider).await {
            state
                .database
                .set_provider_refresh_error(&provider.id, Some(&error))?;
            failed += 1;
        } else {
            succeeded += 1;
        }
    }
    Ok(Html(
        RefreshCompleteTemplate { succeeded, failed }.render()?,
    ))
}

async fn provider_list(State(state): State<Arc<AppState>>) -> Result<Html<String>, AppError> {
    let cards_html = match state.database.active_account()? {
        Some(account) => render_cards(&state, &account.id)?,
        None => String::new(),
    };
    Ok(Html(ProviderListTemplate { cards_html }.render()?))
}

async fn skip_alerts(State(state): State<Arc<AppState>>) -> Result<Html<String>, AppError> {
    let account = state
        .database
        .active_account()?
        .ok_or(AppError::BadRequest)?;
    state.database.set_onboarding_step(&account.id, 4)?;
    render_dashboard(&state)
}

fn render_dashboard(state: &AppState) -> Result<Html<String>, AppError> {
    render_dashboard_with_auth_validation(state, false)
}

fn render_dashboard_with_auth_validation(
    state: &AppState,
    validate_authentication: bool,
) -> Result<Html<String>, AppError> {
    let account = state.database.active_account()?;
    // Loading the project display name here ensures the authenticated project
    // metadata remains part of the dashboard state, ready for the header UI.
    let _project_name = account
        .as_ref()
        .and_then(|item| item.project_name.as_deref());
    let cards_html = match &account {
        Some(account) => render_cards(state, &account.id)?,
        None => String::new(),
    };
    Ok(Html(
        DashboardTemplate {
            account,
            cards_html,
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

fn render_cards(state: &AppState, account_id: &str) -> Result<String, AppError> {
    state
        .database
        .list_providers(account_id)?
        .into_iter()
        .map(|provider| {
            let snapshot = state.database.latest_snapshot(&provider.id)?;
            ProviderCardTemplate {
                apify_credit: apify_credit_summary(&provider, snapshot.as_ref()),
                openai_spend_limit: openai_spend_limit_summary(&provider, snapshot.as_ref()),
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

fn apify_credit_summary(
    provider: &ProviderConfig,
    snapshot: Option<&UsageSnapshot>,
) -> Option<ApifyCreditSummary> {
    if provider.provider_type != "apify" {
        return None;
    }
    let snapshot = snapshot?;
    let allowance = snapshot
        .metrics
        .iter()
        .find(|metric| metric.id == "monthly_credit_allowance")?;
    let remaining = snapshot
        .metrics
        .iter()
        .find(|metric| metric.id == "monthly_credit_remaining")?;
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
    })
}

fn format_usd(value: f64) -> String {
    format!("${:.2}", value.max(0.0))
}

fn format_signed_usd(value: f64) -> String {
    if value < 0.0 {
        format!("-${:.2}", value.abs())
    } else {
        format_usd(value)
    }
}

fn resend_quota_summary(
    provider: &ProviderConfig,
    snapshot: Option<&UsageSnapshot>,
) -> Option<ResendQuotaSummary> {
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
    let sent = snapshot
        .metrics
        .iter()
        .find(|metric| metric.id == "emails_sent_today")?
        .used;
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

fn provider_for_active_account(state: &AppState, id: &str) -> Result<ProviderConfig, AppError> {
    let provider = state
        .database
        .find_provider(id)?
        .ok_or(AppError::NotFound)?;
    let account = state.database.active_account()?.ok_or(AppError::NotFound)?;
    if provider.account_id != account.id {
        return Err(AppError::NotFound);
    }
    Ok(provider)
}

async fn refresh_provider_usage(
    state: &AppState,
    provider: &ProviderConfig,
) -> Result<UsageSnapshot, String> {
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
    let snapshot = state
        .providers
        .collect(provider, &secret)
        .await
        .map_err(|error| format!("{} refresh failed: {error}", provider.display_name))?;
    state
        .database
        .save_snapshot(&snapshot)
        .map_err(|_| "Usage was collected but could not be saved to SQLite.".to_owned())?;
    state
        .database
        .set_provider_refresh_error(&provider.id, None)
        .map_err(|_| "Usage was collected but refresh status could not be saved.".to_owned())?;
    Ok(snapshot)
}

fn valid_project_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 63
        && value.chars().all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '-'
        })
}

fn normalize_secret_reference(project_id: &str, provided: &str) -> Result<String, AppError> {
    let value = provided.trim();
    if value.is_empty() {
        return Err(AppError::BadRequest);
    }
    let parts: Vec<_> = value.split('/').collect();
    match parts.as_slice() {
        ["projects", project, "secrets", secret]
            if *project == project_id && valid_secret_name(secret) =>
        {
            Ok(format!(
                "projects/{project}/secrets/{secret}/versions/latest"
            ))
        }
        ["projects", project, "secrets", secret, "versions", version]
            if *project == project_id && valid_secret_name(secret) && !version.is_empty() =>
        {
            Ok(value.to_owned())
        }
        [secret] if valid_secret_name(secret) => Ok(format!(
            "projects/{project_id}/secrets/{secret}/versions/latest"
        )),
        _ => Err(AppError::BadRequest),
    }
}

fn valid_secret_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 255
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric() || character == '_' || character == '-'
        })
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

#[derive(Template)]
#[template(path = "pages/index.html")]
struct DashboardTemplate {
    account: Option<Account>,
    cards_html: String,
    validate_authentication: bool,
}
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
    resend_quota: Option<ResendQuotaSummary>,
    resend_daily_quota: Option<ResendQuotaSummary>,
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
