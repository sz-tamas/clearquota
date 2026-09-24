use super::AppState;
use axum::{Router, routing::get};
use std::sync::Arc;

pub fn router() -> Router<Arc<AppState>> {
	Router::new().route("/", get(|| async { "ok" }))
}
