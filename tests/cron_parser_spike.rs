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

use cron_parser_spike::{CronError, CronParser};

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
