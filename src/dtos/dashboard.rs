use crate::models::{Account, ProviderConfig, UsageSnapshot};
use askama::Template;
use serde::Deserialize;

#[derive(Default, Deserialize)]
pub(crate) struct MonthQuery {
	pub(crate) month: Option<String>,
}

#[derive(Template)]
#[template(path = "pages/index.html")]
pub(crate) struct DashboardTemplate {
	pub(crate) account: Option<Account>,
	pub(crate) cards_html: String,
	pub(crate) selected_month: String,
	pub(crate) previous_month_url: String,
	pub(crate) next_month_url: Option<String>,
	pub(crate) refresh_url: String,
	pub(crate) show_dashboard_skeleton: bool,
	pub(crate) validate_authentication: bool,
}

#[derive(Template)]
#[template(path = "pages/overview.html")]
pub(crate) struct OverviewTemplate {
	pub(crate) account: Option<Account>,
	pub(crate) summary: Option<OverviewSummary>,
	pub(crate) validate_authentication: bool,
}

pub(crate) struct OverviewSummary {
	pub(crate) tracked_spend: String,
	pub(crate) nearing_limit: String,
	pub(crate) connected_count: usize,
	pub(crate) last_refreshed: String,
	pub(crate) provider_usage: Vec<OverviewProviderUsage>,
}

pub(crate) struct OverviewProviderUsage {
	pub(crate) name: String,
	pub(crate) provider_type: String,
	pub(crate) label: String,
	pub(crate) used: String,
	pub(crate) limit: String,
	pub(crate) percent: String,
	pub(crate) is_nearing_limit: bool,
}

#[derive(Template)]
#[template(path = "partials/provider_card.html")]
pub(crate) struct ProviderCardTemplate {
	pub(crate) provider: ProviderConfig,
	pub(crate) snapshot: Option<UsageSnapshot>,
	pub(crate) apify_credit: Option<ApifyCreditSummary>,
	pub(crate) openai_spend_limit: Option<OpenAiSpendLimitSummary>,
	pub(crate) openai_credit: Option<OpenAiCreditSummary>,
	pub(crate) openai_activity: Option<OpenAiActivitySummaryView>,
	pub(crate) resend_quota: Option<ResendQuotaSummary>,
	pub(crate) resend_daily_quota: Option<ResendQuotaSummary>,
}

pub(crate) struct OpenAiActivitySummaryView {
	pub(crate) activity_available: bool,
	pub(crate) total_tokens: String,
	pub(crate) input_tokens: String,
	pub(crate) output_tokens: String,
	pub(crate) requests: String,
	pub(crate) cache_hit_rate: String,
	pub(crate) token_points: String,
	pub(crate) request_points: String,
	pub(crate) cache_points: String,
	pub(crate) spend_points: String,
	pub(crate) projects: Vec<OpenAiProjectSpendView>,
}

#[derive(Clone)]
pub(crate) struct OpenAiProjectSpendView {
	pub(crate) name: String,
	pub(crate) amount: String,
	pub(crate) percent: String,
}

pub(crate) struct ApifyCreditSummary {
	pub(crate) used: String,
	pub(crate) limit: String,
	pub(crate) remaining: String,
	pub(crate) percent_used: String,
}

pub(crate) struct OpenAiSpendLimitSummary {
	pub(crate) used: String,
	pub(crate) limit: Option<String>,
	pub(crate) percent: String,
	pub(crate) spend_points: String,
	pub(crate) projects: Vec<OpenAiProjectSpendView>,
}
pub(crate) struct OpenAiCreditSummary {
	pub(crate) used: String,
	pub(crate) total: String,
	pub(crate) remaining: String,
	pub(crate) percent: String,
}

pub(crate) struct ResendQuotaSummary {
	pub(crate) plan: String,
	pub(crate) used: String,
	pub(crate) limit: String,
	pub(crate) remaining: String,
	pub(crate) percent_used: String,
}
