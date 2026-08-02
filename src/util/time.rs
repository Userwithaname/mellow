/// Takes a duration in seconds and returns a nicely formatted
/// `String` for display (`(h:)(m)m:ss`)
///
/// # Example
/// ```rust
/// # use mellow::util::format_duration;
/// #
/// assert_eq!(format_duration(83), "1:23");
/// assert_eq!(format_duration(60 * 60 + 83), "1:01:23");
/// ```
#[inline]
#[must_use]
pub fn format_duration(seconds_total: u64) -> String {
    let seconds = seconds_total % 60;
    if seconds_total < 60 * 60 {
        format!("{}:{seconds:02}", (seconds_total - seconds) / 60)
    } else {
        let minutes_total = (seconds_total - seconds) / 60;
        let minutes = minutes_total % 60;
        format!(
            "{}:{minutes:02}:{seconds:02}",
            (minutes_total - minutes) / 60,
        )
    }
}
/// Takes a duration in milliseconds and returns a nicely
/// formatted `String` for display (`(h:)(m)m:ss`)
///
/// # Example
/// ```rust
/// # use mellow::util::format_duration_ms;
/// #
/// assert_eq!(format_duration_ms(83000), "1:23");
/// assert_eq!(format_duration_ms((60 * 60 + 83) * 1000), "1:01:23");
/// ```
#[inline]
#[must_use]
pub fn format_duration_ms(milliseconds_total: u64) -> String {
    format_duration(milliseconds_total / 1000)
}
/// Takes a duration in minutes and returns a nicely formatted
/// `String` for display (`(([x]d,) [y]h,) [z]m`)
///
/// # Example
/// ```rust
/// # use mellow::util::format_duration_minutes;
/// #
/// assert_eq!(format_duration_minutes(1), "1m");
/// assert_eq!(format_duration_minutes(60 + 23), "1h, 23m");
/// assert_eq!(format_duration_minutes(24 * 60 + 2 * 60 + 3), "1d, 2h, 3m");
/// ```
#[inline]
#[must_use]
pub fn format_duration_minutes(minutes_total: u64) -> String {
    // TODO: Could use "days"/"hours"/"minutes", but it should to respect plural/singular
    let minutes = minutes_total % 60;
    if minutes_total < 60 {
        format!("{minutes}m")
    } else if minutes_total < 60 * 24 {
        format!("{}h, {minutes}m", (minutes_total - minutes) / 60).replace(", 0m", "")
    } else {
        let hours_total = (minutes_total - minutes) / 60;
        let hours = hours_total % 24;
        format!("{}d, {hours}h, {minutes}m", (hours_total - hours) / 24)
            .replace(", 0m", "")
            .replace(", 0h", "")
    }
}
