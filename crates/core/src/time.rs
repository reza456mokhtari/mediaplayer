//! Timestamp formatting shared by every UI surface.

use std::time::Duration;

/// Formats a playback timestamp as `m:ss`, or `h:mm:ss` from one hour up.
///
/// Fractional seconds are truncated, as players do for elapsed time.
pub fn format_timestamp(time: Duration) -> String {
    let total = time.as_secs();
    let (hours, minutes, seconds) = (total / 3600, total % 3600 / 60, total % 60);
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes}:{seconds:02}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_minutes_and_seconds() {
        assert_eq!(format_timestamp(Duration::ZERO), "0:00");
        assert_eq!(format_timestamp(Duration::from_secs(7)), "0:07");
        assert_eq!(format_timestamp(Duration::from_secs(187)), "3:07");
        assert_eq!(format_timestamp(Duration::from_secs(3599)), "59:59");
    }

    #[test]
    fn truncates_fractional_seconds() {
        assert_eq!(format_timestamp(Duration::from_millis(59_999)), "0:59");
    }

    #[test]
    fn switches_to_hours_at_one_hour() {
        assert_eq!(format_timestamp(Duration::from_secs(3600)), "1:00:00");
        assert_eq!(format_timestamp(Duration::from_secs(3723)), "1:02:03");
        assert_eq!(format_timestamp(Duration::from_secs(36_000)), "10:00:00");
    }
}
