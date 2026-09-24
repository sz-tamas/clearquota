use crate::{dtos::AppError, models::UsagePeriod};
use chrono::{Datelike, NaiveDate, Utc};

pub(crate) fn valid_project_id(value: &str) -> bool {
	!value.is_empty()
		&& value.len() <= 63
		&& value
			.chars()
			.all(|character| character.is_ascii_lowercase() || character.is_ascii_digit() || character == '-')
}

pub(crate) fn is_htmx_navigation(headers: &axum::http::HeaderMap) -> bool {
	headers.get("HX-Request").is_some_and(|value| value == "true")
}

pub(crate) fn current_period() -> UsagePeriod {
	let today = Utc::now().date_naive();
	period_for_month(today.year(), today.month(), true).expect("the current UTC month is valid")
}

pub(crate) fn selected_period(value: Option<&str>) -> Result<UsagePeriod, AppError> {
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

pub(crate) fn period_for_month(year: i32, month: u32, is_current: bool) -> Option<UsagePeriod> {
	let start = NaiveDate::from_ymd_opt(year, month, 1)?;
	let (next_year, next_month) = if month == 12 { (year + 1, 1) } else { (year, month + 1) };
	let end = NaiveDate::from_ymd_opt(next_year, next_month, 1)?;
	Some(UsagePeriod {
		start: start.and_hms_opt(0, 0, 0)?.and_utc().timestamp(),
		end: end.and_hms_opt(0, 0, 0)?.and_utc().timestamp(),
		is_current,
	})
}

pub(crate) fn previous_period(period: UsagePeriod) -> UsagePeriod {
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

pub(crate) fn next_period(period: UsagePeriod) -> UsagePeriod {
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

pub(crate) fn period_label(period: UsagePeriod) -> String {
	chrono::DateTime::from_timestamp(period.start, 0)
		.expect("stored period timestamp is valid")
		.format("%B %Y")
		.to_string()
}

pub(crate) fn month_url(period: UsagePeriod) -> String {
	format!("/dashboard?month={}", month_value(period))
}

pub(crate) fn month_value(period: UsagePeriod) -> String {
	chrono::DateTime::from_timestamp(period.start, 0)
		.expect("stored period timestamp is valid")
		.format("%Y-%m")
		.to_string()
}

pub(crate) fn month_from_url(url: &str) -> Option<&str> {
	url.split_once('?')
		.and_then(|(_, query)| query.split('&').find_map(|parameter| parameter.strip_prefix("month=")))
}

pub(crate) fn percent_of(used: f64, limit: f64) -> f64 {
	if limit > 0.0 { used / limit * 100.0 } else { 0.0 }
}

pub(crate) fn sparkline_points(values: &[f64]) -> String {
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

pub(crate) fn credit_usage_percent(spent: f64, credit: f64) -> f64 {
	if credit <= 0.0 { 0.0 } else { (spent / credit * 100.0).clamp(0.0, 100.0) }
}

pub(crate) fn format_usd(value: f64) -> String {
	format!("${:.2}", value.max(0.0))
}

pub(crate) fn format_signed_usd(value: f64) -> String {
	if value < 0.0 { format!("-${:.2}", value.abs()) } else { format_usd(value) }
}

pub(crate) fn format_email_count(value: f64) -> String {
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

pub(crate) fn format_count(value: i64) -> String {
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

#[cfg(test)]
mod tests {
	use super::*;

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
		assert_eq!(
			month_from_url("http://127.0.0.1:5050/dashboard?month=2026-08"),
			Some("2026-08")
		);
		assert_eq!(month_from_url("http://127.0.0.1:5050/dashboard"), None);
	}
}

pub(crate) fn valid_credit_allowance(value: Option<f64>) -> bool {
	value.is_some_and(|amount| amount.is_finite() && amount > 0.0)
}

pub(crate) fn normalize_secret_reference(project_id: &str, provided: &str) -> Result<String, AppError> {
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

pub(crate) fn valid_secret_name(value: &str) -> bool {
	!value.is_empty()
		&& value.len() <= 255
		&& value
			.chars()
			.all(|character| character.is_ascii_alphanumeric() || character == '_' || character == '-')
}
