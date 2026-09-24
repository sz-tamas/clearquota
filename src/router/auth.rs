use super::{AppState, handlers};
use axum::{Router, routing::post};
use std::sync::Arc;

pub fn router() -> Router<Arc<AppState>> {
	Router::new().route("/validate", post(handlers::validate_saved_authentication))
}
