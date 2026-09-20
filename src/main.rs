mod config;
mod database;
mod models;
mod providers;
mod secrets;
mod web;

use std::sync::Arc;

use axum::Router;
use config::Config;
use database::Database;
use providers::ProviderRegistry;
use secrets::GcpSecretManagerResolver;
use tower_http::services::ServeDir;
use web::{AppState, routes};

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
        .merge(routes::router())
        .nest_service("/static", ServeDir::new("static"))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(&config.bind_address).await?;
    println!(
        "Usage Dashboard listening on http://{}",
        config.bind_address
    );
    axum::serve(listener, app).await?;
    Ok(())
}
