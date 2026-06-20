// SPDX-License-Identifier: MIT OR Apache-2.0
// Integration tests for the W3b cron parser spike.
//
// We pull the spike source in via `#[path]` rather than registering
// it as a module in `lib.rs`. This keeps the spike self-contained
// (per spike constraints: no existing files may be modified) and
// still lets `cargo test cron_parser_spike` compile and run it.
//
// Run with: `cargo test --test cron_parser_spike cron_parser_spike`

#[path = "../src/cron_parser_spike.rs"]
mod cron_parser_spike;

use chrono::{TimeZone, Utc};

use cron_parser_spike::{CronError, CronExpr, CronParser};

#[test]
fn test_cron_parser_spike_can_parse_every_minute() {
    let mut parser = CronParser::default();
    let result = parser.parse("* * * * *");
    assert!(
        result.is_ok(),
        "expected parse(\"* * * * *\") to succeed, got {:?}",
        result
    );
    let expr = result.unwrap();
    assert_eq!(expr.expression, "* * * * *");
    assert!(expr.is_every_minute());
}

#[test]
fn test_cron_parser_spike_rejects_unknown() {
    let mut parser = CronParser::default();
    let result = parser.parse("@hourly");
    assert!(
        result.is_err(),
        "expected parse(\"@hourly\") to fail, got {:?}",
        result
    );
    match result.unwrap_err() {
        CronError::NotImplemented(expr) => assert_eq!(expr, "@hourly"),
        other => panic!("expected CronError::NotImplemented, got {:?}", other),
    }
}

#[test]
fn test_cron_parser_spike_matches_in_range() {
    let mut parser = CronParser::default();
    parser.parse("* * * * *").expect("spike should accept every-minute");

    let instant = Utc.with_ymd_and_hms(2026, 6, 15, 12, 34, 56).unwrap();
    assert!(
        parser.matches(instant),
        "spike should report match for known time"
    );

    let next = parser.next_after(instant);
    assert!(next.is_some(), "next_after should return Some for parsed expr");
    assert!(
        next.unwrap() > instant,
        "next_after must be strictly after the base instant"
    );
}

#[test]
fn test_cron_parser_spike_is_every_minute_predicate() {
    let every = CronExpr { expression: "* * * * *".to_string() };
    assert!(every.is_every_minute());

    let other = CronExpr { expression: "0 12 * * *".to_string() };
    assert!(!other.is_every_minute());

    let hourly_alias = CronExpr { expression: "@hourly".to_string() };
    assert!(!hourly_alias.is_every_minute());
}

#[test]
fn test_cron_parser_spike_rejects_blank_and_various() {
    let mut parser = CronParser::default();
    // All non-every-minute expressions must yield NotImplemented in the spike.
    for input in &["", "*/5 * * * *", "0 0 * * *", "0 9 * * 1-5", "@daily", "@reboot", "0 0 1 1 0"] {
        let result = parser.parse(input);
        assert!(
            matches!(result, Err(CronError::NotImplemented(_))),
            "expected NotImplemented for {:?}, got {:?}",
            input,
            result
        );
    }
}

#[test]
fn test_cron_parser_spike_unparsed_state_is_empty() {
    let parser = CronParser::default();
    // Before any parse call, matches/next_after must reflect the empty inner.
    let instant = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
    assert!(!parser.matches(instant));
    assert!(parser.next_after(instant).is_none());
}

#[test]
fn test_cron_parser_spike_next_after_is_one_minute_later() {
    let mut parser = CronParser::default();
    parser.parse("* * * * *").unwrap();
    let instant = Utc.with_ymd_and_hms(2026, 6, 15, 12, 34, 56).unwrap();
    let next = parser.next_after(instant).expect("next_after should be Some");
    let expected = instant + chrono::Duration::minutes(1);
    assert_eq!(next, expected);
}

#[test]
fn test_cron_parser_spike_default_is_empty() {
    let parser = CronParser::default();
    // Default has no parsed expression.
    let instant = Utc.with_ymd_and_hms(1970, 1, 1, 0, 0, 0).unwrap();
    assert!(!parser.matches(instant));
    assert!(parser.next_after(instant).is_none());
}

#[test]
fn test_cron_parser_spike_re_parse_replaces_expression() {
    let mut parser = CronParser::default();
    parser.parse("* * * * *").unwrap();
    // Re-parsing the same expression should still succeed.
    let second = parser.parse("* * * * *").unwrap();
    assert!(second.is_every_minute());
    // After successful parse, matches/next_after still behave.
    let instant = Utc.with_ymd_and_hms(2026, 6, 15, 0, 0, 0).unwrap();
    assert!(parser.matches(instant));
    assert!(parser.next_after(instant).is_some());
}

#[test]
fn test_cron_parser_spike_error_display_messages() {
    let not_impl = CronError::NotImplemented("@weekly".to_string());
    let invalid = CronError::Invalid("oops".to_string());
    // Display must mention the offending expression for both variants.
    assert!(format!("{}", not_impl).contains("@weekly"));
    assert!(format!("{}", invalid).contains("oops"));
}

#[test]
fn test_cron_parser_spike_invalid_variant_distinct() {
    // The Invalid variant is reserved for the W3b follow-up real impl;
    // it must be a distinct discriminant from NotImplemented.
    let not_impl = CronError::NotImplemented("x".to_string());
    let invalid = CronError::Invalid("x".to_string());
    assert_ne!(not_impl, invalid);
    assert_ne!(format!("{:?}", not_impl), format!("{:?}", invalid));
}
