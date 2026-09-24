use askama::Template;
use axum::response::IntoResponse;

#[derive(Template)]
#[template(path = "pages/not_found.html")]
pub(crate) struct NotFoundTemplate;

#[derive(Debug, thiserror::Error)]
pub(crate) enum AppError {
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
