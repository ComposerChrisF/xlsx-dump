//! Excel serial date-times to ISO 8601 text.
//!
//! A spreadsheet date is a number of days since an epoch, with a date or time number format; the
//! format is what makes it a date.  Converting here (rather than through `chrono`) keeps the rule
//! in one visible place: the time of day is rounded to the millisecond, because serials carry
//! binary-fraction noise (`0.3958333…` is 09:30, not 09:29:59.99999).

const MS_PER_DAY: i64 = 86_400_000;

/// An Excel serial date-time as ISO 8601: `YYYY-MM-DD` with no time part, `YYYY-MM-DDTHH:MM:SS`
/// with one (`.sss` only when the milliseconds are non-zero), or `HH:MM:SS` for a pure time (a
/// serial below 1 in the 1900 system, where day 0 is not a real date).  `None` for a serial that is
/// no date at all (negative, or beyond year 9999); the caller then keeps the number.
pub fn serial_to_iso(serial: f64, is_1904: bool) -> Option<String> {
    // 2_958_466 is 10000-01-01, the first day Excel cannot show.
    if !(0.0..2_958_466.0).contains(&serial) {
        return None;
    }
    let total_ms = (serial * MS_PER_DAY as f64).round() as i64;
    let day = total_ms.div_euclid(MS_PER_DAY);
    let ms = total_ms.rem_euclid(MS_PER_DAY);

    let date = if is_1904 {
        // Day 0 is 1904-01-01, and the 1904 system has no phantom leap day.
        Some(days_to_civil(day + days_from_civil(1904, 1, 1)))
    } else if day == 0 {
        None // a pure time of day
    } else if day == 60 {
        // Excel's fictitious 1900-02-29 (the Lotus 1-2-3 leap-year bug), reproduced as it displays.
        Some((1900, 2, 29))
    } else {
        // Day 1 is 1900-01-01.  Up to the phantom leap day, the epoch is 1899-12-31; after it,
        // 1899-12-30, which absorbs the day that never existed.
        let epoch = if day < 60 {
            days_from_civil(1899, 12, 31)
        } else {
            days_from_civil(1899, 12, 30)
        };
        Some(days_to_civil(epoch + day))
    };

    // Checked again after rounding: a serial a hair below the limit rounds up into year 10000.
    if date.is_some_and(|(y, _, _)| y > 9999) {
        return None;
    }
    let time = (ms != 0).then(|| time_of_day(ms));
    Some(match (date, time) {
        (Some((y, m, d)), None) => format!("{y:04}-{m:02}-{d:02}"),
        (Some((y, m, d)), Some(t)) => format!("{y:04}-{m:02}-{d:02}T{t}"),
        (None, Some(t)) => t,
        (None, None) => "00:00:00".to_string(),
    })
}

/// An Excel duration (a serial in days, under an elapsed-time format such as `[h]:mm`) as an ISO
/// 8601 duration: `PT36H30M`, `PT0S` for zero, a leading `-` when negative.
pub fn serial_to_iso_duration(serial: f64) -> String {
    let total_ms = (serial * MS_PER_DAY as f64).round() as i64;
    let sign = if total_ms < 0 { "-" } else { "" };
    let total_ms = total_ms.unsigned_abs();
    let hours = total_ms / 3_600_000;
    let minutes = total_ms / 60_000 % 60;
    let secs = total_ms / 1000 % 60;
    let ms = total_ms % 1000;
    let mut out = format!("{sign}PT");
    if hours > 0 {
        out.push_str(&format!("{hours}H"));
    }
    if minutes > 0 {
        out.push_str(&format!("{minutes}M"));
    }
    if secs > 0 || ms > 0 || (hours == 0 && minutes == 0) {
        if ms > 0 {
            out.push_str(&format!("{secs}.{ms:03}S"));
        } else {
            out.push_str(&format!("{secs}S"));
        }
    }
    out
}

fn time_of_day(ms: i64) -> String {
    let (h, m, s, frac) = (ms / 3_600_000, ms / 60_000 % 60, ms / 1000 % 60, ms % 1000);
    if frac == 0 {
        format!("{h:02}:{m:02}:{s:02}")
    } else {
        format!("{h:02}:{m:02}:{s:02}.{frac:03}")
    }
}

/// Days since 1970-01-01 for a proleptic Gregorian date (Howard Hinnant's algorithm).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// The inverse of [`days_from_civil`].
fn days_to_civil(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (
        if m <= 2 {
            yoe + era * 400 + 1
        } else {
            yoe + era * 400
        },
        m,
        d,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_in_the_1900_system() {
        assert_eq!(serial_to_iso(1.0, false).unwrap(), "1900-01-01");
        assert_eq!(serial_to_iso(59.0, false).unwrap(), "1900-02-28");
        assert_eq!(serial_to_iso(60.0, false).unwrap(), "1900-02-29");
        assert_eq!(serial_to_iso(61.0, false).unwrap(), "1900-03-01");
        assert_eq!(serial_to_iso(46_186.0, false).unwrap(), "2026-06-13");
        assert_eq!(serial_to_iso(2_958_465.0, false).unwrap(), "9999-12-31");
    }

    #[test]
    fn times_round_to_the_millisecond() {
        assert_eq!(
            serial_to_iso(46_186.395_833_333, false).unwrap(),
            "2026-06-13T09:30:00"
        );
        assert_eq!(serial_to_iso(0.75, false).unwrap(), "18:00:00");
        assert_eq!(
            serial_to_iso(0.5 + 1.5 / 86_400.0, false).unwrap(),
            "12:00:01.500"
        );
    }

    #[test]
    fn dates_in_the_1904_system() {
        assert_eq!(serial_to_iso(0.0, true).unwrap(), "1904-01-01");
        assert_eq!(serial_to_iso(44_724.0, true).unwrap(), "2026-06-13");
    }

    #[test]
    fn non_dates_are_refused() {
        assert_eq!(serial_to_iso(-1.0, false), None);
        assert_eq!(serial_to_iso(3_000_000.0, false), None);
        assert_eq!(serial_to_iso(2_958_465.999_999_999_5, false), None);
    }

    #[test]
    fn durations() {
        assert_eq!(serial_to_iso_duration(1.520_833_333_3), "PT36H30M");
        assert_eq!(serial_to_iso_duration(0.0), "PT0S");
        assert_eq!(serial_to_iso_duration(-0.5), "-PT12H");
        assert_eq!(serial_to_iso_duration(1.0 / 86_400.0), "PT1S");
    }
}
