use anyhow::Result;
use chrono::{DateTime, Utc};

/// Parse an RFC3339 timestamp string into a DateTime<Utc>
/// Handles both standard RFC3339 format and timestamps with incomplete timezone offsets (e.g., +01 instead of +01:00)
pub fn parse_rfc3339_timestamp(timestamp_str: &str) -> Result<DateTime<Utc>> {
    // Try standard RFC3339 parsing first
    if let Ok(dt) = DateTime::parse_from_rfc3339(timestamp_str) {
        return Ok(dt.with_timezone(&Utc));
    }

    // If that fails, try to fix incomplete timezone offset (e.g., +01 -> +01:00)
    if let Some(fixed) = fix_incomplete_timezone(timestamp_str) {
        if let Ok(dt) = DateTime::parse_from_rfc3339(&fixed) {
            return Ok(dt.with_timezone(&Utc));
        }
    }

    anyhow::bail!("Failed to parse RFC3339 timestamp: {}", timestamp_str)
}

/// Fixes incomplete timezone offsets like +01 to +01:00
fn fix_incomplete_timezone(timestamp_str: &str) -> Option<String> {
    // Look for pattern like +HH or -HH at the end
    let bytes = timestamp_str.as_bytes();
    let len = bytes.len();

    if len < 3 {
        return None;
    }

    // Check if the last 3 characters match [+-]DD pattern
    if (bytes[len - 3] == b'+' || bytes[len - 3] == b'-')
        && bytes[len - 2].is_ascii_digit()
        && bytes[len - 1].is_ascii_digit()
    {
        return Some(format!("{}:00", timestamp_str));
    }

    None
}
