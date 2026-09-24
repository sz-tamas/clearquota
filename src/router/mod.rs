pub mod account;
pub mod auth;
pub mod dashboard;
mod handlers;
pub mod health;
pub mod onboarding;
pub mod providers;
pub mod runlogs;

use crate::{database::Database, providers::ProviderRegistry, secrets::SecretResolver};
use axum::Router;
use std::sync::Arc;

pub struct AppState {
	pub database: Database,
	pub secret_resolver: Arc<dyn SecretResolver>,
	pub providers: ProviderRegistry,
}

pub fn router() -> Router<Arc<AppState>> {
	Router::new()
		.route("/settings", axum::routing::get(account::settings_page))
		.route(
			"/settings/clear/{category}",
			axum::routing::post(account::clear_local_data),
		)
		.route("/settings/erase", axum::routing::post(account::erase_local_data))
		.merge(dashboard::router())
		.nest("/providers", providers::router())
		.nest("/runlogs", runlogs::router())
		.nest("/health", health::router())
		.nest("/authentication", auth::router())
		.nest("/onboarding", onboarding::router())
		.nest("/account", account::router())
		.fallback(handlers::not_found)
}
