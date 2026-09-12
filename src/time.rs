//! Minimal Unix-timestamp formatting, computed by hand (the standard
//! civil-from-days algorithm) rather than pulling in a date/time crate
//! for what's otherwise a couple of display/timestamp fields.

/// Unix timestamp -> `YYYY-MM-DD`.
pub fn unix_to_ymd(unix_secs: i64) -> String {
    let (y, m, d) = civil_from_days(unix_secs.div_euclid(86_400));
    format!("{y:04}-{m:02}-{d:02}")
}

/// Unix timestamp -> `YYYY-MM-DDTHH:MM:SSZ`, for `X-PacPad-Created`.
pub fn unix_to_iso8601(unix_secs: i64) -> String {
    let days = unix_secs.div_euclid(86_400);
    let secs_of_day = unix_secs.rem_euclid(86_400);
    let (y, mo, d) = civil_from_days(days);
    let h = secs_of_day / 3600;
    let mi = (secs_of_day % 3600) / 60;
    let s = secs_of_day % 60;
    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{mi:02}:{s:02}Z")
}

pub fn now_iso8601() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    unix_to_iso8601(secs)
}

fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ymd_matches_known_timestamps() {
        // Verified against `date -u -d @<secs> '+%Y-%m-%d'` on this
        // machine, not assumed.
        assert_eq!(unix_to_ymd(0), "1970-01-01");
        assert_eq!(unix_to_ymd(1788972672), "2026-09-09");
        assert_eq!(unix_to_ymd(1784392535), "2026-07-18");
    }

    #[test]
    fn iso8601_includes_time_of_day() {
        // Verified against `date -u -d @<secs> '+%Y-%m-%dT%H:%M:%SZ'`.
        assert_eq!(unix_to_iso8601(0), "1970-01-01T00:00:00Z");
        assert_eq!(unix_to_iso8601(1788972672), "2026-09-09T16:51:12Z");
    }
}
