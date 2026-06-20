// SPDX-License-Identifier: MIT OR Apache-2.0
// Integration tests for the W3b production cron expression parser
// (src/cron_parser.rs). This file complements the existing
// `#[cfg(test)] mod tests` block in the source itself and exercises
// additional public API paths — Field::matches edge cases, FromStr
// error variants, CronExpr::matches with every field variant, and
// whitespace handling.
//
// Run with: `cargo test --test cron_parser`

use std::str::FromStr;

use taskkit::cron_parser::{CronExpr, Field};

#[test]
fn test_field_any_matches_anything() {
    let f = Field::Any;
    for v in [0usize, 1, 7, 30, 59, 100, usize::MAX] {
        assert!(f.matches(v), "Field::Any should match {}", v);
    }
}

#[test]
fn test_field_every_zero_steps_must_be_rejected() {
    // FromStr: */0 must error with the "step cannot be 0" message.
    let err = Field::from_str("*/0").unwrap_err();
    assert!(err.contains("step cannot be 0"), "got: {}", err);
}

#[test]
fn test_field_every_non_numeric_is_error() {
    // FromStr: */abc must surface a parse error.
    let err = Field::from_str("*/abc").unwrap_err();
    assert!(err.contains("bad */n"), "got: {}", err);
}

#[test]
fn test_field_range_start_greater_than_end_is_error() {
    // FromStr: range with start > end must error.
    let err = Field::from_str("20-10").unwrap_err();
    assert!(err.contains("range start > end"), "got: {}", err);
}

#[test]
fn test_field_range_with_more_than_two_dashes_is_error() {
    // FromStr: malformed range "1-2-3" must error.
    let err = Field::from_str("1-2-3").unwrap_err();
    assert!(err.contains("bad range"), "got: {}", err);
}

#[test]
fn test_field_range_non_numeric_start_or_end_errors() {
    // FromStr: range with non-numeric start.
    let err = Field::from_str("a-5").unwrap_err();
    assert!(err.contains("bad range start"), "got: {}", err);
    // FromStr: range with non-numeric end.
    let err = Field::from_str("5-z").unwrap_err();
    assert!(err.contains("bad range end"), "got: {}", err);
}

#[test]
fn test_field_list_with_invalid_member_is_error() {
    // FromStr: a list with one bad member must surface a "bad list" error.
    let err = Field::from_str("1,2,abc,4").unwrap_err();
    assert!(err.contains("bad list"), "got: {}", err);
}

#[test]
fn test_field_values_matches_only_listed() {
    let f = Field::from_str("5,15,45").unwrap();
    assert!(f.matches(5));
    assert!(f.matches(15));
    assert!(f.matches(45));
    assert!(!f.matches(0));
    assert!(!f.matches(7));
    assert!(!f.matches(46));
}

#[test]
fn test_field_range_without_step_matches_anything_in_window() {
    // Range with no step matches every value in the closed interval.
    let f = Field::from_str("9-17").unwrap();
    assert!(f.matches(9));
    assert!(f.matches(10));
    assert!(f.matches(17));
    assert!(!f.matches(8));
    assert!(!f.matches(18));
}

#[test]
fn test_field_range_with_step_honours_modulo() {
    // Range(start, end, Some(step)) — note the production FromStr does
    // not parse the /step suffix, so we construct directly to exercise
    // the Some(step) branch of Field::matches.
    let f = Field::Range(0, 30, Some(5));
    assert!(f.matches(0));
    assert!(f.matches(5));
    assert!(f.matches(10));
    assert!(f.matches(30));
    assert!(!f.matches(1));
    assert!(!f.matches(3));
    assert!(!f.matches(31));
}

#[test]
fn test_field_range_below_start_and_above_end() {
    let f = Field::Range(10, 20, None);
    assert!(!f.matches(0));
    assert!(!f.matches(9));
    assert!(f.matches(10));
    assert!(f.matches(20));
    assert!(!f.matches(21));
    assert!(!f.matches(100));
}

#[test]
fn test_cron_expr_matches_with_combined_fields() {
    // Exercise all five fields of CronExpr::matches in one expression.
    // "0 9 * * 1-5" — 9am on weekdays only.
    let c: CronExpr = "0 9 * * 1-5".parse().unwrap();
    // Mon 9:00 should match
    assert!(c.matches(0, 9, 15, 6, 1));
    // Fri 9:00 should match
    assert!(c.matches(0, 9, 19, 6, 5));
    // Sun 9:00 must not (weekday 0)
    assert!(!c.matches(0, 9, 14, 6, 0));
    // Sat 9:00 must not (weekday 6)
    assert!(!c.matches(0, 9, 20, 6, 6));
    // Mon 10:00 must not (hour 10, not 9)
    assert!(!c.matches(0, 10, 15, 6, 1));
    // Mon 9:01 must not (minute 1, not 0)
    assert!(!c.matches(1, 9, 15, 6, 1));
    // CronExpr field accessors are public — sanity check equality.
    assert_eq!(c.minute, Field::Values(vec![0]));
    assert_eq!(c.hour, Field::Values(vec![9]));
    assert_eq!(c.day, Field::Any);
    assert_eq!(c.month, Field::Any);
    assert_eq!(c.weekday, Field::Range(1, 5, None));
}

#[test]
fn test_cron_expr_every_minute_matches_everything() {
    // The "* * * * *" pattern should match for any combination of fields.
    let c: CronExpr = "* * * * *".parse().unwrap();
    for minute in [0, 30, 59] {
        for hour in [0, 12, 23] {
            for day in [1, 15, 31] {
                for month in [1, 6, 12] {
                    for weekday in [0, 3, 6] {
                        assert!(
                            c.matches(minute, hour, day, month, weekday),
                            "every-minute must match minute={} hour={} day={} month={} weekday={}",
                            minute, hour, day, month, weekday
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn test_cron_expr_specific_minute_list() {
    // "5,15,45 * * * *" — matches at minutes 5, 15, 45 of any hour.
    let c: CronExpr = "5,15,45 * * * *".parse().unwrap();
    assert!(c.matches(5, 12, 1, 1, 0));
    assert!(c.matches(15, 0, 31, 12, 6));
    assert!(c.matches(45, 23, 28, 2, 3));
    assert!(!c.matches(0, 12, 1, 1, 0));
    assert!(!c.matches(14, 12, 1, 1, 0));
}

#[test]
fn test_cron_expr_rejects_wrong_field_counts() {
    // Too few fields
    assert!("* * *".parse::<CronExpr>().is_err());
    // Too many fields
    assert!("* * * * * *".parse::<CronExpr>().is_err());
    // Empty string
    assert!("".parse::<CronExpr>().is_err());
    // Single field
    assert!("*".parse::<CronExpr>().is_err());
}

#[test]
fn test_cron_expr_whitespace_separator_handling() {
    // Multiple spaces between fields must be treated as a single separator
    // by split_whitespace, so this should still parse.
    let c: CronExpr = "0   9    *  *   1-5".parse().unwrap();
    assert!(c.matches(0, 9, 15, 6, 1));
    assert!(!c.matches(0, 9, 15, 6, 0));
    // Tabs are also split_whitespace separators.
    let c2: CronExpr = "0\t9\t*\t*\t1-5".parse().unwrap();
    assert!(c2.matches(0, 9, 15, 6, 3));
}

#[test]
fn test_field_from_str_single_value_is_values_with_one_element() {
    // A single literal "5" (no range, no list, no star) must parse to
    // Field::Values(vec![5]).
    let f = Field::from_str("5").unwrap();
    assert_eq!(f, Field::Values(vec![5]));
}

#[test]
fn test_field_from_str_non_numeric_single_value_errors() {
    // "not-a-number" contains '-' so it tries to parse as range first
    let err = Field::from_str("not-a-number").unwrap_err();
    assert!(err.contains("bad range"), "got: {}", err);
}

#[test]
fn test_field_every_with_large_step() {
    // */1440 is technically not a valid minute (60 max) but FromStr does
    // not validate bounds — it just stores the step. Field::matches
    // should still apply it as a modulo.
    let f = Field::from_str("*/1440").unwrap();
    assert_eq!(f, Field::Every(1440));
    // 0 is always a multiple of any n.
    assert!(f.matches(0));
    assert!(!f.matches(1));
    // 1440 matches because 1440 % 1440 == 0
    assert!(f.matches(1440));
}
