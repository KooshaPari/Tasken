// SPDX-License-Identifier: MIT OR Apache-2.0
//! W3b — Cron expression parser spike.
//!
//! Minimal parser for the 5-field cron format used by Tasken's
//! scheduling layer. Supports `*`, `*/n`, `n`, `n-m`, `n,m`, and
//! combinations thereof. No external deps. Spike code: not used in
//! production until reviewed.

use std::str::FromStr;

#[derive(Debug, Clone, PartialEq)]
pub struct CronExpr {
    pub minute: Field,
    pub hour: Field,
    pub day: Field,
    pub month: Field,
    pub weekday: Field,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Field {
    Any,
    Every(usize),
    Values(Vec<usize>),
    Range(usize, usize, Option<usize>),
}

impl Field {
    pub fn matches(&self, value: usize) -> bool {
        match self {
            Field::Any => true,
            Field::Every(n) => value % n == 0,
            Field::Values(vs) => vs.contains(&value),
            Field::Range(start, end, step) => {
                if value < *start || value > *end {
                    return false;
                }
                match step {
                    None => true,
                    Some(s) => (value - start) % s == 0,
                }
            }
        }
    }
}

impl FromStr for Field {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s == "*" {
            return Ok(Field::Any);
        }
        if let Some(rest) = s.strip_prefix("*/") {
            let n: usize = rest.parse().map_err(|e| format!("bad */n: {}", e))?;
            if n == 0 {
                return Err("step cannot be 0".into());
            }
            return Ok(Field::Every(n));
        }
        if s.contains(',') {
            let vs: Result<Vec<usize>, _> = s.split(',').map(|p| p.parse()).collect();
            return Ok(Field::Values(vs.map_err(|e| format!("bad list: {}", e))?));
        }
        if s.contains('-') {
            let parts: Vec<&str> = s.split('-').collect();
            if parts.len() != 2 {
                return Err(format!("bad range: {}", s));
            }
            let start: usize = parts[0].parse().map_err(|e| format!("bad range start: {}", e))?;
            let end: usize = parts[1].parse().map_err(|e| format!("bad range end: {}", e))?;
            if start > end {
                return Err(format!("range start > end: {}", s));
            }
            return Ok(Field::Range(start, end, None));
        }
        let n: usize = s.parse().map_err(|e| format!("bad field: {}", e))?;
        Ok(Field::Values(vec![n]))
    }
}

impl CronExpr {
    pub fn matches(&self, minute: usize, hour: usize, day: usize, month: usize, weekday: usize) -> bool {
        self.minute.matches(minute)
            && self.hour.matches(hour)
            && self.day.matches(day)
            && self.month.matches(month)
            && self.weekday.matches(weekday)
    }
}

impl FromStr for CronExpr {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.split_whitespace().collect();
        if parts.len() != 5 {
            return Err(format!("cron must have 5 fields, got {}", parts.len()));
        }
        Ok(CronExpr {
            minute: parts[0].parse()?,
            hour: parts[1].parse()?,
            day: parts[2].parse()?,
            month: parts[3].parse()?,
            weekday: parts[4].parse()?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_star() {
        let f: Field = "*".parse().unwrap();
        assert_eq!(f, Field::Any);
    }

    #[test]
    fn parse_every_n() {
        let f: Field = "*/15".parse().unwrap();
        assert_eq!(f, Field::Every(15));
    }

    #[test]
    fn parse_list() {
        let f: Field = "1,15,45".parse().unwrap();
        assert_eq!(f, Field::Values(vec![1, 15, 45]));
    }

    #[test]
    fn parse_range() {
        let f: Field = "9-17".parse().unwrap();
        assert_eq!(f, Field::Range(9, 17, None));
    }

    #[test]
    fn field_matches() {
        let f: Field = "*/15".parse().unwrap();
        assert!(f.matches(0));
        assert!(f.matches(15));
        assert!(f.matches(45));
        assert!(!f.matches(7));
    }

    #[test]
    fn full_cron_every_minute() {
        let c: CronExpr = "* * * * *".parse().unwrap();
        assert!(c.matches(0, 0, 1, 1, 0));
        assert!(c.matches(59, 23, 31, 12, 6));
    }

    #[test]
    fn full_cron_weekdays_9am() {
        let c: CronExpr = "0 9 * * 1-5".parse().unwrap();
        assert!(c.matches(0, 9, 15, 6, 1)); // Mon 9:00
        assert!(!c.matches(0, 9, 15, 6, 0)); // Sun 9:00 — no
        assert!(!c.matches(0, 10, 15, 6, 1)); // Mon 10:00 — no
    }

    #[test]
    fn reject_bad_field_count() {
        assert!("* * *".parse::<CronExpr>().is_err());
        assert!("* * * * * *".parse::<CronExpr>().is_err());
    }
}
