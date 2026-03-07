use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::component::CompactComponent;
use super::deserialize_optional_field;
use super::enums::{Priority, TicketStatus, TicketType};
use super::milestone::CompactMilestone;
use super::user::CompactUser;

/// Database row for the `tickets` table.
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct TicketRow {
    pub id: i64,
    #[sqlx(rename = "type")]
    pub ticket_type: TicketType,
    pub title: String,
    pub status: TicketStatus,
    pub priority: Priority,
    pub owner_id: i64,
    pub component_id: i64,
    pub estimation_hours: Option<f64>,
    pub created_by: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Full ticket in API responses, with expanded relations and computed fields.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TicketResponse {
    pub id: i64,
    #[serde(rename = "type")]
    pub ticket_type: TicketType,
    pub title: String,
    pub status: TicketStatus,
    pub priority: Priority,
    pub owner: CompactUser,
    pub component: CompactComponent,
    pub estimation_hours: Option<f64>,
    pub estimation_display: Option<String>,
    pub created_by: CompactUser,
    pub cc: Vec<CompactUser>,
    pub milestones: Vec<CompactMilestone>,
    pub comment_count: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Request body for creating a new ticket.
#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateTicketRequest {
    #[serde(rename = "type")]
    pub ticket_type: TicketType,
    pub title: String,
    pub owner_id: i64,
    pub component_id: i64,
    pub priority: Option<Priority>,
    pub description: Option<String>,
    pub cc: Option<Vec<i64>>,
    pub milestones: Option<Vec<i64>>,
    pub estimation: Option<String>,
}

/// Request body for updating an existing ticket.
///
/// `estimation` uses double-Option: absent = don't change, null = clear, value = set.
#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateTicketRequest {
    pub title: Option<String>,
    pub status: Option<TicketStatus>,
    pub priority: Option<Priority>,
    pub owner_id: Option<i64>,
    pub component_id: Option<i64>,
    #[serde(rename = "type")]
    pub ticket_type: Option<TicketType>,
    pub cc: Option<Vec<i64>>,
    pub milestones: Option<Vec<i64>>,
    #[serde(default, deserialize_with = "deserialize_optional_field")]
    pub estimation: Option<Option<String>>,
}

const HOURS_PER_DAY: f64 = 8.0;
const HOURS_PER_WEEK: f64 = 40.0;

/// Formats estimation hours into a human-readable string.
///
/// Uses `w` (40h), `d` (8h), `h` units with compound output (e.g. `"1d4h"`).
pub fn format_estimation(hours: f64) -> String {
    if hours <= 0.0 {
        return "0h".to_string();
    }

    let mut remaining = hours;
    let mut parts = Vec::new();

    let weeks = (remaining / HOURS_PER_WEEK).floor() as u64;
    if weeks > 0 {
        parts.push(format!("{weeks}w"));
        remaining -= weeks as f64 * HOURS_PER_WEEK;
    }

    let days = (remaining / HOURS_PER_DAY).floor() as u64;
    if days > 0 {
        parts.push(format!("{days}d"));
        remaining -= days as f64 * HOURS_PER_DAY;
    }

    if remaining > 0.0 {
        // Avoid floating-point noise like "3.9999999h"
        let h = remaining.round() as u64;
        if h > 0 {
            parts.push(format!("{h}h"));
        }
    }

    if parts.is_empty() {
        "0h".to_string()
    } else {
        parts.join("")
    }
}

/// Parses a human-readable estimation string into hours.
///
/// Accepted formats: `"4h"`, `"2d"`, `"1w"`, `"1d4h"`, `"1w2d4h"`.
pub fn parse_estimation(input: &str) -> Result<f64, String> {
    let input = input.trim().to_lowercase();
    if input.is_empty() {
        return Err("empty estimation string".to_string());
    }

    let mut total = 0.0;
    let mut num_buf = String::new();
    let mut found_unit = false;

    for ch in input.chars() {
        if ch.is_ascii_digit() || ch == '.' {
            num_buf.push(ch);
        } else {
            let value: f64 = num_buf
                .parse()
                .map_err(|_| format!("invalid number in estimation: '{num_buf}'"))?;
            num_buf.clear();

            match ch {
                'w' => total += value * HOURS_PER_WEEK,
                'd' => total += value * HOURS_PER_DAY,
                'h' => total += value,
                _ => return Err(format!("unknown estimation unit: '{ch}'")),
            }
            found_unit = true;
        }
    }

    if !num_buf.is_empty() {
        return Err(format!("trailing number without unit: '{num_buf}'"));
    }

    if !found_unit {
        return Err(format!("no valid estimation units found in '{input}'"));
    }

    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_estimation_weeks() {
        assert_eq!(format_estimation(40.0), "1w");
        assert_eq!(format_estimation(80.0), "2w");
    }

    #[test]
    fn format_estimation_days() {
        assert_eq!(format_estimation(16.0), "2d");
        assert_eq!(format_estimation(8.0), "1d");
    }

    #[test]
    fn format_estimation_hours() {
        assert_eq!(format_estimation(4.0), "4h");
        assert_eq!(format_estimation(1.0), "1h");
    }

    #[test]
    fn format_estimation_compound() {
        assert_eq!(format_estimation(12.0), "1d4h");
        assert_eq!(format_estimation(52.0), "1w1d4h");
    }

    #[test]
    fn format_estimation_zero() {
        assert_eq!(format_estimation(0.0), "0h");
        assert_eq!(format_estimation(-1.0), "0h");
    }

    #[test]
    fn parse_estimation_single_units() {
        assert_eq!(parse_estimation("1w").unwrap(), 40.0);
        assert_eq!(parse_estimation("2d").unwrap(), 16.0);
        assert_eq!(parse_estimation("4h").unwrap(), 4.0);
    }

    #[test]
    fn parse_estimation_compound() {
        assert_eq!(parse_estimation("1d4h").unwrap(), 12.0);
        assert_eq!(parse_estimation("1w2d4h").unwrap(), 60.0);
    }

    #[test]
    fn parse_estimation_case_insensitive() {
        assert_eq!(parse_estimation("2D").unwrap(), 16.0);
        assert_eq!(parse_estimation("1W").unwrap(), 40.0);
    }

    #[test]
    fn parse_estimation_invalid() {
        assert!(parse_estimation("").is_err());
        assert!(parse_estimation("abc").is_err());
        assert!(parse_estimation("4x").is_err());
        assert!(parse_estimation("42").is_err());
    }
}
