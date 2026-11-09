use std::time::Duration;

/// `m:ss`, or `h:mm:ss` from one hour up.
pub fn clock(d: Duration) -> String {
    let s = d.as_secs();
    let (h, m, s) = (s / 3600, (s / 60) % 60, s % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_minutes_and_hours() {
        assert_eq!(clock(Duration::ZERO), "0:00");
        assert_eq!(clock(Duration::from_secs(253)), "4:13");
        assert_eq!(clock(Duration::from_millis(59_999)), "0:59");
        assert_eq!(clock(Duration::from_secs(3725)), "1:02:05");
    }
}
