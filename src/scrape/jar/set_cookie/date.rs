//! The cookie-date parser of RFC 6265 section 5.1.1, used by the `Expires` attribute.

use chrono::NaiveDate;

use super::DECIMAL;

/// The month names a cookie date may use, matched on their first three letters without regard to
/// ASCII case.
const MONTHS: [&str; 12] = [
    "jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec",
];

/// How many leading letters of a date token name its month.
const MONTH_LETTERS: usize = 3;

/// The digits a `day-of-month` token starts with: one or two.
const DAY_DIGITS: (usize, usize) = (1, 2);

/// The digits a `year` token starts with: two to four.
const YEAR_DIGITS: (usize, usize) = (2, 4);

/// The most digits one field of a `time` token may have.
const TIME_FIELD_DIGITS: usize = 2;

/// Two-digit years from this value up are read as 19xx; below it, as 20xx.
const TWO_DIGIT_YEAR_PIVOT: u32 = 70;

/// The largest year written with two digits.
const LAST_TWO_DIGIT_YEAR: u32 = 99;

/// What a two-digit year from the pivot up is added to.
const NINETEEN_HUNDRED: u32 = 1900;

/// What a two-digit year below the pivot is added to.
const TWO_THOUSAND: u32 = 2000;

/// The earliest year a cookie date may name (step 5).
const MIN_YEAR: u32 = 1601;

/// The largest day of the month a cookie date may name before the calendar check.
const MAX_DAY: u32 = 31;

/// The largest hour a cookie date may name.
const MAX_HOUR: u32 = 23;

/// The largest minute or second a cookie date may name; leap seconds cannot be written.
const MAX_MINUTE_OR_SECOND: u32 = 59;

/// Parses a cookie date (RFC 6265 section 5.1.1) into seconds since the Unix epoch.
///
/// The tokens may come in any order; two-digit years from 70 are read as 19xx and below 70 as
/// 20xx. Returns `None` when a field is missing or out of range, or the date does not exist.
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::scrape::jar::set_cookie::parse_cookie_date;
///
/// assert_eq!(parse_cookie_date("Thu, 01 Jan 1970 00:01:40 GMT"), Some(100));
/// assert_eq!(parse_cookie_date("30 Feb 2030 00:00:00"), None);
/// ```
#[must_use]
pub fn parse_cookie_date(text: &str) -> Option<i64> {
    let mut fields = DateFields::default();
    let tokens = text.as_bytes().split(|&b| is_date_delimiter(b));
    for token in tokens.filter(|token| !token.is_empty()) {
        fields.take(token);
    }
    fields.finish()
}

/// The fields found so far while reading a cookie date.
#[derive(Default)]
struct DateFields {
    time: Option<(u32, u32, u32)>,
    day: Option<u32>,
    month: Option<u32>,
    year: Option<u32>,
}

impl DateFields {
    /// Gives `token` to the first production that is still unset and matches it (step 2).
    fn take(&mut self, token: &[u8]) {
        if self.time.is_none() {
            if let Some(time) = parse_time(token) {
                self.time = Some(time);
                return;
            }
        }
        if self.day.is_none() {
            if let Some(day) = leading_number(token, DAY_DIGITS) {
                self.day = Some(day);
                return;
            }
        }
        if self.month.is_none() {
            if let Some(month) = parse_month(token) {
                self.month = Some(month);
                return;
            }
        }
        if self.year.is_none() {
            self.year = leading_number(token, YEAR_DIGITS);
        }
    }

    /// Checks the ranges (steps 3 to 6) and returns seconds since the Unix epoch.
    fn finish(self) -> Option<i64> {
        let (hour, minute, second) = self.time?;
        let (day, month) = (self.day?, self.month?);
        let year = match self.year? {
            short @ TWO_DIGIT_YEAR_PIVOT..=LAST_TWO_DIGIT_YEAR => short + NINETEEN_HUNDRED,
            short @ 0..TWO_DIGIT_YEAR_PIVOT => short + TWO_THOUSAND,
            full => full,
        };
        let in_range = (1..=MAX_DAY).contains(&day)
            && year >= MIN_YEAR
            && hour <= MAX_HOUR
            && minute <= MAX_MINUTE_OR_SECOND
            && second <= MAX_MINUTE_OR_SECOND;
        if !in_range {
            return None;
        }
        let date = NaiveDate::from_ymd_opt(i32::try_from(year).ok()?, month, day)?;
        let at = date.and_hms_opt(hour, minute, second)?;
        Some(at.and_utc().timestamp())
    }
}

/// The cookie-date delimiters: `%x09 / %x20-2F / %x3B-40 / %x5B-60 / %x7B-7E`.
fn is_date_delimiter(byte: u8) -> bool {
    matches!(byte, 0x09 | 0x20..=0x2F | 0x3B..=0x40 | 0x5B..=0x60 | 0x7B..=0x7E)
}

/// The value of the digits `token` starts with, when there are `min..=max` of them. What follows
/// the digits is a non-digit or nothing, as the `day-of-month` and `year` productions require.
fn leading_number(token: &[u8], (min, max): (usize, usize)) -> Option<u32> {
    let count = token.iter().take_while(|b| b.is_ascii_digit()).count();
    (min..=max)
        .contains(&count)
        .then(|| digits_value(&token[..count]))
}

/// The value of a run of at most four ASCII digits.
fn digits_value(digits: &[u8]) -> u32 {
    digits
        .iter()
        .fold(0, |acc, b| acc * DECIMAL + u32::from(b - b'0'))
}

/// `hms-time ( non-digit *OCTET )`, where `hms-time` is three fields of one or two digits joined
/// by `:`.
fn parse_time(token: &[u8]) -> Option<(u32, u32, u32)> {
    let (hour, rest) = time_field(token)?;
    let (minute, rest) = time_field(rest.strip_prefix(b":")?)?;
    let (second, rest) = time_field(rest.strip_prefix(b":")?)?;
    if rest.first().is_some_and(u8::is_ascii_digit) {
        return None;
    }
    Some((hour, minute, second))
}

/// One `time-field`: one or two digits, and what follows them.
fn time_field(bytes: &[u8]) -> Option<(u32, &[u8])> {
    let count = bytes.iter().take_while(|b| b.is_ascii_digit()).count();
    (1..=TIME_FIELD_DIGITS)
        .contains(&count)
        .then(|| (digits_value(&bytes[..count]), &bytes[count..]))
}

/// The month a token names by its first three letters, from 1 for January.
fn parse_month(token: &[u8]) -> Option<u32> {
    let head = token.get(..MONTH_LETTERS)?;
    let index = MONTHS
        .iter()
        .position(|month| head.eq_ignore_ascii_case(month.as_bytes()))?;
    u32::try_from(index + 1).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stealth::humanize::Rng;

    fn date(text: &str) -> Option<i64> {
        parse_cookie_date(text)
    }

    fn epoch(y: i32, mo: u32, d: u32, h: u32, mi: u32, s: u32) -> i64 {
        NaiveDate::from_ymd_opt(y, mo, d)
            .and_then(|day| day.and_hms_opt(h, mi, s))
            .map(|at| at.and_utc().timestamp())
            .expect("valid test date")
    }

    #[test]
    fn dates_in_the_three_http_formats_parse() {
        let want = Some(epoch(1994, 11, 6, 8, 49, 37));
        assert_eq!(date("Sun, 06 Nov 1994 08:49:37 GMT"), want);
        assert_eq!(date("Sunday, 06-Nov-94 08:49:37 GMT"), want);
        assert_eq!(date("Sun Nov  6 08:49:37 1994"), want);
    }

    #[test]
    fn date_tokens_may_come_in_any_order_and_case() {
        let want = Some(epoch(2031, 2, 3, 4, 5, 6));
        assert_eq!(date("2031 FEBRUARY 3 4:5:6"), want);
        assert_eq!(date("04:05:06 03 feb 2031"), want);
        assert_eq!(date("3-Feb-2031 04:05:06 UTC trailing junk"), want);
        assert_eq!(
            date("Feb3,2031,04:05:06"),
            None,
            "a day glued to the month is part of the month token"
        );
    }

    #[test]
    fn two_digit_years_pivot_at_70() {
        assert_eq!(date("01 Jan 70 00:00:00"), Some(0));
        assert_eq!(date("01 Jan 99 00:00:00"), Some(epoch(1999, 1, 1, 0, 0, 0)));
        assert_eq!(date("01 Jan 69 00:00:00"), Some(epoch(2069, 1, 1, 0, 0, 0)));
        assert_eq!(date("01 Jan 00 00:00:00"), Some(epoch(2000, 1, 1, 0, 0, 0)));
    }

    #[test]
    fn impossible_dates_fail() {
        for text in [
            "30 Feb 2030 00:00:00",
            "00 Jan 2030 00:00:00",
            "32 Jan 2030 00:00:00",
            "01 Jan 2030 24:00:00",
            "01 Jan 2030 00:60:00",
            "01 Jan 2030 00:00:60",
            "01 Jan 1600 00:00:00",
            "01 Jan 2030",
            "Jan 2030 00:00:00",
            "01 2030 00:00:00",
            "01 Jan 00:00:00",
            "001 Jan 2030 00:00:00",
            "01 Jan 20301 00:00:00",
            "01 Jan 2030 000:00:00",
            "",
        ] {
            assert_eq!(date(text), None, "{text:?}");
        }
    }

    #[test]
    fn generated_epochs_survive_a_format_and_parse_round_trip() {
        let mut rng = Rng::new(42);
        let span = u64::try_from(epoch(9999, 12, 31, 23, 59, 59)).expect("positive");
        for _ in 0..5_000 {
            let secs = i64::try_from(rng.next_u64() % span).expect("in range");
            let at = chrono::DateTime::from_timestamp(secs, 0).expect("in range");
            let text = at.format("%a, %d %b %Y %H:%M:%S GMT").to_string();
            assert_eq!(date(&text), Some(secs), "{text}");
        }
    }

    #[test]
    fn generated_garbage_never_panics() {
        const PIECES: [&str; 12] = [
            "1", "12", "123", "12345", ":", "Jan", "x", " ", ",", "-", "\u{7f}", "99",
        ];
        let mut rng = Rng::new(3);
        for _ in 0..20_000 {
            let len = rng.next_u64() % 12;
            let text: String = (0..len).map(|_| *rng.pick(&PIECES)).collect();
            if let Some(secs) = parse_cookie_date(&text) {
                assert!(
                    chrono::DateTime::from_timestamp(secs, 0).is_some(),
                    "{text:?}"
                );
            }
        }
    }
}
