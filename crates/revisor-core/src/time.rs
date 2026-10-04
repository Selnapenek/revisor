use chrono::{DateTime, Utc};

/// "3h ago" style label for an RFC 3339 timestamp. Unparseable input gives "".
pub fn relative_time(iso: &str) -> String {
    let Ok(date) = DateTime::parse_from_rfc3339(iso) else {
        return String::new();
    };
    relative_secs((Utc::now() - date.with_timezone(&Utc)).num_seconds())
}

fn relative_secs(seconds: i64) -> String {
    let minutes = seconds / 60;
    let hours = minutes / 60;
    let days = hours / 24;
    let months = days / 30;
    match () {
        _ if seconds < 60 => "just now".into(),
        _ if minutes < 60 => format!("{minutes}m ago"),
        _ if hours < 24 => format!("{hours}h ago"),
        _ if days < 30 => format!("{days}d ago"),
        _ if months < 12 => format!("{months}mo ago"),
        _ => format!("{}y ago", months / 12),
    }
}

#[cfg(test)]
mod tests {
    use super::relative_secs;

    #[test]
    fn buckets() {
        assert_eq!(relative_secs(5), "just now");
        assert_eq!(relative_secs(120), "2m ago");
        assert_eq!(relative_secs(3 * 3600), "3h ago");
        assert_eq!(relative_secs(2 * 86400), "2d ago");
        assert_eq!(relative_secs(65 * 86400), "2mo ago");
        assert_eq!(relative_secs(400 * 86400), "1y ago");
    }
}
