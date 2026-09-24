mod config;
mod database;
mod dtos;
mod models;
mod providers;
mod router;
mod secrets;
mod utils;

use std::sync::Arc;

use axum::Router;
use config::Config;
use database::Database;
use providers::ProviderRegistry;
use router::AppState;
use secrets::GcpSecretManagerResolver;
use tower_http::services::ServeDir;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	let config = Config::from_env()?;
	let database = Database::open(&config.database_path)?;
	database.migrate()?;

	let state = Arc::new(AppState {
		database,
		secret_resolver: Arc::new(GcpSecretManagerResolver),
		providers: ProviderRegistry,
	});

	let app = Router::new()
		.merge(router::router())
		.nest_service("/static", ServeDir::new("static"))
		.with_state(state);

	let listener = tokio::net::TcpListener::bind(&config.bind_address).await?;
	println!("ClearQuota listening on http://{}", config.bind_address);
	axum::serve(listener, app).await?;
	Ok(())
}
