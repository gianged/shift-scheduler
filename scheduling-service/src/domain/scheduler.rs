use std::path::Path;

use chrono::{Duration, NaiveDate};
use chrono_tz::Tz;
use serde::Deserialize;
use shared::types::ShiftType;
use thiserror::Error;
use uuid::Uuid;

use crate::domain::circuit_breaker::CircuitBreakerConfig;
use crate::domain::job::NewShiftAssignment;
use crate::infrastructure::health_check::HealthCheckSettings;

/// Errors that can occur when loading or validating the scheduling configuration.
#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Parse error: {0}")]
    Parse(#[from] toml::de::Error),
    #[error("Invalid config: {0}")]
    Invalid(String),
}

/// Fixed scheduling period of 28 days (4 weeks).
const PERIOD_DAYS: usize = 28;
/// Days per week, used to track weekly rule boundaries.
const DAYS_PER_WEEK: usize = 7;

/// Scheduling configuration loaded from TOML, controlling scheduling rules,
/// circuit breaker settings, and health check parameters.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct SchedulingConfig {
    pub timezone: String,
    pub min_day_off_per_week: Option<u8>,
    pub max_day_off_per_week: Option<u8>,
    pub no_morning_after_evening: bool,
    pub max_daily_shift_diff: Option<u8>,
    pub circuit_breaker: CircuitBreakerConfig,
    pub health_check: HealthCheckSettings,
}

impl Default for SchedulingConfig {
    fn default() -> Self {
        Self {
            timezone: "UTC".to_string(),
            min_day_off_per_week: Some(1),
            max_day_off_per_week: Some(2),
            no_morning_after_evening: true,
            max_daily_shift_diff: Some(1),
            circuit_breaker: CircuitBreakerConfig::default(),
            health_check: HealthCheckSettings::default(),
        }
    }
}

impl SchedulingConfig {
    /// Loads config from a TOML file, falling back to defaults if the file does not exist.
    pub fn load(path: &str) -> Result<Self, ConfigError> {
        if !Path::new(path).exists() {
            tracing::info!("Config file not found at {path}, using defaults");
            return Ok(Self::default());
        }
        let content = std::fs::read_to_string(path)?;
        let config: Self = toml::from_str(&content)?;
        config.validate()?;
        tracing::info!(?config, "Loaded scheduling config from {path}");
        Ok(config)
    }

    fn validate(&self) -> Result<(), ConfigError> {
        if let (Some(min), Some(max)) = (self.min_day_off_per_week, self.max_day_off_per_week) {
            if min > max {
                return Err(ConfigError::Invalid(format!(
                    "min_day_off_per_week ({min}) must be <= max_day_off_per_week ({max})"
                )));
            }
            if max >= 7 {
                return Err(ConfigError::Invalid(format!(
                    "max_day_off_per_week ({max}) must be < 7"
                )));
            }
        }
        Ok(())
    }

    /// Parses the configured timezone string, falling back to UTC on invalid values.
    pub fn timezone(&self) -> Tz {
        self.timezone.parse::<Tz>().unwrap_or_else(|_| {
            tracing::warn!(
                timezone = %self.timezone,
                "Invalid timezone, falling back to UTC"
            );
            Tz::UTC
        })
    }
}

/// Errors that can occur during schedule generation.
#[derive(Debug, Error)]
pub enum SchedulingError {
    #[error("No valid shift found for staff {staff_id} on day {day}")]
    NoValidShift { staff_id: Uuid, day: usize },
}

// region: Trait-based scheduling rules

/// Snapshot of scheduling state passed to rules when evaluating a candidate shift.
pub struct AssignmentContext {
    pub previous_shift: Option<ShiftType>,
    pub day_offs_this_week: u8,
    pub days_remaining_in_week: u8,
    pub morning_count: usize,
    pub evening_count: usize,
}

/// A pluggable scheduling constraint. Rules are evaluated in order; a candidate shift
/// is only assigned if all rules return `true`.
pub trait SchedulingRule: Send + Sync {
    fn name(&self) -> &str;
    fn is_valid(&self, ctx: &AssignmentContext, candidate: &ShiftType) -> bool;
}

/// Prevents assigning a morning shift immediately following an evening shift.
pub struct NoMorningAfterEveningRule;

impl SchedulingRule for NoMorningAfterEveningRule {
    fn name(&self) -> &str {
        "no_morning_after_evening"
    }

    fn is_valid(&self, ctx: &AssignmentContext, candidate: &ShiftType) -> bool {
        !matches!(
            (ctx.previous_shift, candidate),
            (Some(ShiftType::Evening), ShiftType::Morning)
        )
    }
}

/// Limits the maximum number of days off per week per staff member.
pub struct MaxDayOffRule {
    pub max: u8,
}

impl SchedulingRule for MaxDayOffRule {
    fn name(&self) -> &str {
        "max_day_off"
    }

    fn is_valid(&self, ctx: &AssignmentContext, candidate: &ShiftType) -> bool {
        if *candidate != ShiftType::DayOff {
            return true;
        }
        ctx.day_offs_this_week < self.max
    }
}

/// Ensures each staff member gets at least a minimum number of days off per week.
/// Rejects working shifts when remaining days in the week cannot satisfy the minimum.
pub struct MinDayOffRule {
    pub min: u8,
}

impl SchedulingRule for MinDayOffRule {
    fn name(&self) -> &str {
        "min_day_off"
    }

    fn is_valid(&self, ctx: &AssignmentContext, candidate: &ShiftType) -> bool {
        if *candidate == ShiftType::DayOff {
            return true;
        }
        ctx.day_offs_this_week + ctx.days_remaining_in_week >= self.min
    }
}

/// Keeps the daily count of morning and evening shifts balanced within a maximum difference.
pub struct DailyBalanceRule {
    pub max_diff: u8,
}

impl SchedulingRule for DailyBalanceRule {
    fn name(&self) -> &str {
        "daily_balance"
    }

    fn is_valid(&self, ctx: &AssignmentContext, candidate: &ShiftType) -> bool {
        let (m, e) = match candidate {
            ShiftType::Morning => (ctx.morning_count + 1, ctx.evening_count),
            ShiftType::Evening => (ctx.morning_count, ctx.evening_count + 1),
            ShiftType::DayOff => return true,
        };
        m.abs_diff(e) <= self.max_diff as usize
    }
}

impl SchedulingConfig {
    /// Constructs the list of scheduling rules based on the current configuration.
    pub fn build_rules(&self) -> Vec<Box<dyn SchedulingRule>> {
        let mut rules: Vec<Box<dyn SchedulingRule>> = Vec::new();
        if self.no_morning_after_evening {
            rules.push(Box::new(NoMorningAfterEveningRule));
        }
        if let Some(max) = self.max_day_off_per_week {
            rules.push(Box::new(MaxDayOffRule { max }));
        }
        if let Some(min) = self.min_day_off_per_week {
            rules.push(Box::new(MinDayOffRule { min }));
        }
        if let Some(max_diff) = self.max_daily_shift_diff {
            rules.push(Box::new(DailyBalanceRule { max_diff }));
        }
        rules
    }
}

// endregion: Trait-based scheduling rules

// region: Main algo

/// Generates a 28-day shift schedule for the given staff.
///
/// Uses a greedy assignment algorithm that iterates day-by-day, alternating staff
/// order and shift preference each day for fairness. All configured rules are
/// evaluated per candidate assignment.
#[tracing::instrument(skip(staff_ids, rules))]
pub fn gen_schedule(
    staff_ids: &[Uuid],
    period_begin_date: NaiveDate,
    rules: &[Box<dyn SchedulingRule>],
) -> Result<Vec<NewShiftAssignment>, SchedulingError> {
    tracing::debug!(
        staff_count = staff_ids.len(),
        "Starting schedule generation"
    );

    let mut assignments: Vec<NewShiftAssignment> = Vec::new();

    let mut previous_shifts: Vec<Option<ShiftType>> = vec![None; staff_ids.len()];
    let mut weekly_day_offs: Vec<u8> = vec![0; staff_ids.len()];

    // Alternate staff iteration order per day for fairness
    let staff_count = staff_ids.len();
    let forward: Vec<usize> = (0..staff_count).collect();
    let reverse: Vec<usize> = (0..staff_count).rev().collect();

    for day in 0..PERIOD_DAYS {
        let date = period_begin_date + Duration::days(day as i64);
        let day_in_week = day % DAYS_PER_WEEK;
        let days_remaining_in_week = (DAYS_PER_WEEK - 1 - day_in_week) as u8;

        if day_in_week == 0 {
            weekly_day_offs.fill(0);
        }

        // Alternate Morning/Evening preference; DayOff always last to avoid greedy consumption
        let shift_options = if day % 2 == 0 {
            [ShiftType::Morning, ShiftType::Evening, ShiftType::DayOff]
        } else {
            [ShiftType::Evening, ShiftType::Morning, ShiftType::DayOff]
        };

        let mut morning_count: usize = 0;
        let mut evening_count: usize = 0;

        let order = if day % 2 == 0 { &forward } else { &reverse };

        for &i in order {
            let staff_id = &staff_ids[i];
            let mut assigned = false;

            for shift in &shift_options {
                let ctx = AssignmentContext {
                    previous_shift: previous_shifts[i],
                    day_offs_this_week: weekly_day_offs[i],
                    days_remaining_in_week,
                    morning_count,
                    evening_count,
                };

                let ok = rules.iter().all(|rule| rule.is_valid(&ctx, shift));

                if ok {
                    match *shift {
                        ShiftType::DayOff => weekly_day_offs[i] += 1,
                        ShiftType::Morning => morning_count += 1,
                        ShiftType::Evening => evening_count += 1,
                    }

                    previous_shifts[i] = Some(*shift);
                    assignments.push(NewShiftAssignment {
                        staff_id: *staff_id,
                        date,
                        shift_type: *shift,
                    });
                    assigned = true;
                    break;
                }
            }

            if !assigned {
                return Err(SchedulingError::NoValidShift {
                    staff_id: *staff_id,
                    day,
                });
            }
        }
    }

    tracing::debug!(
        assignment_count = assignments.len(),
        "Schedule generation completed"
    );

    Ok(assignments)
}

// endregion: Main algo

#[cfg(test)]
mod tests {
    use super::*;

    fn default_config() -> SchedulingConfig {
        SchedulingConfig::default()
    }

    fn monday() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 2, 16).unwrap()
    }

    // Rule tests

    #[test]
    fn no_morning_after_evening_rule_blocks() {
        let rule = NoMorningAfterEveningRule;
        let ctx = AssignmentContext {
            previous_shift: Some(ShiftType::Evening),
            day_offs_this_week: 0,
            days_remaining_in_week: 6,
            morning_count: 0,
            evening_count: 0,
        };
        assert!(!rule.is_valid(&ctx, &ShiftType::Morning));
    }

    #[test]
    fn no_morning_after_evening_rule_allows_evening_after_evening() {
        let rule = NoMorningAfterEveningRule;
        let ctx = AssignmentContext {
            previous_shift: Some(ShiftType::Evening),
            day_offs_this_week: 0,
            days_remaining_in_week: 6,
            morning_count: 0,
            evening_count: 0,
        };
        assert!(rule.is_valid(&ctx, &ShiftType::Evening));
    }

    #[test]
    fn max_day_off_rule_blocks_excess() {
        let rule = MaxDayOffRule { max: 2 };
        let ctx = AssignmentContext {
            previous_shift: None,
            day_offs_this_week: 2,
            days_remaining_in_week: 4,
            morning_count: 0,
            evening_count: 0,
        };
        assert!(!rule.is_valid(&ctx, &ShiftType::DayOff));
    }

    #[test]
    fn max_day_off_rule_allows_under_max() {
        let rule = MaxDayOffRule { max: 2 };
        let ctx = AssignmentContext {
            previous_shift: None,
            day_offs_this_week: 1,
            days_remaining_in_week: 4,
            morning_count: 0,
            evening_count: 0,
        };
        assert!(rule.is_valid(&ctx, &ShiftType::DayOff));
    }

    #[test]
    fn min_day_off_rule_forces_day_off() {
        let rule = MinDayOffRule { min: 1 };
        // 0 day offs, 0 days remaining -> working shift must be rejected
        let ctx = AssignmentContext {
            previous_shift: None,
            day_offs_this_week: 0,
            days_remaining_in_week: 0,
            morning_count: 0,
            evening_count: 0,
        };
        assert!(!rule.is_valid(&ctx, &ShiftType::Morning));
    }

    #[test]
    fn daily_balance_rule_rejects_imbalance() {
        let rule = DailyBalanceRule { max_diff: 1 };
        // 3 morning, 1 evening -> adding morning would make diff 3
        let ctx = AssignmentContext {
            previous_shift: None,
            day_offs_this_week: 0,
            days_remaining_in_week: 6,
            morning_count: 3,
            evening_count: 1,
        };
        assert!(!rule.is_valid(&ctx, &ShiftType::Morning));
    }

    #[test]
    fn daily_balance_rule_day_off_always_passes() {
        let rule = DailyBalanceRule { max_diff: 1 };
        let ctx = AssignmentContext {
            previous_shift: None,
            day_offs_this_week: 0,
            days_remaining_in_week: 6,
            morning_count: 10,
            evening_count: 0,
        };
        assert!(rule.is_valid(&ctx, &ShiftType::DayOff));
    }

    #[test]
    fn build_rules_includes_morning_after_evening_when_enabled() {
        let config = default_config();
        let rules = config.build_rules();
        assert!(rules.iter().any(|r| r.name() == "no_morning_after_evening"));
    }

    #[test]
    fn build_rules_excludes_morning_after_evening_when_disabled() {
        let config = SchedulingConfig {
            no_morning_after_evening: false,
            ..default_config()
        };
        let rules = config.build_rules();
        assert!(!rules.iter().any(|r| r.name() == "no_morning_after_evening"));
    }

    // gen_schedule tests

    fn validate_schedule(
        assignments: &[NewShiftAssignment],
        staff_ids: &[Uuid],
        config: &SchedulingConfig,
    ) {
        let begin_date = assignments[0].date;

        for &sid in staff_ids {
            let staff_assignments: Vec<_> =
                assignments.iter().filter(|a| a.staff_id == sid).collect();
            assert_eq!(
                staff_assignments.len(),
                PERIOD_DAYS,
                "Staff {sid} should have {PERIOD_DAYS} assignments"
            );

            if config.no_morning_after_evening {
                for w in staff_assignments.windows(2) {
                    assert!(
                        !(w[0].shift_type == ShiftType::Evening
                            && w[1].shift_type == ShiftType::Morning),
                        "Staff {sid} has morning after evening on {}",
                        w[1].date
                    );
                }
            }

            for week in 0..(PERIOD_DAYS / DAYS_PER_WEEK) {
                let start = week * DAYS_PER_WEEK;
                let end = start + DAYS_PER_WEEK;
                let day_offs = staff_assignments[start..end]
                    .iter()
                    .filter(|a| a.shift_type == ShiftType::DayOff)
                    .count() as u8;
                if let Some(min) = config.min_day_off_per_week {
                    assert!(
                        day_offs >= min,
                        "Staff {sid} week {week}: {day_offs} < min {min}"
                    );
                }
                if let Some(max) = config.max_day_off_per_week {
                    assert!(
                        day_offs <= max,
                        "Staff {sid} week {week}: {day_offs} > max {max}"
                    );
                }
            }
        }

        if let Some(max_diff) = config.max_daily_shift_diff {
            for day in 0..PERIOD_DAYS {
                let date = begin_date + Duration::days(day as i64);
                let morning = assignments
                    .iter()
                    .filter(|a| a.date == date && a.shift_type == ShiftType::Morning)
                    .count();
                let evening = assignments
                    .iter()
                    .filter(|a| a.date == date && a.shift_type == ShiftType::Evening)
                    .count();
                assert!(
                    morning.abs_diff(evening) <= max_diff as usize,
                    "Day {date}: morning={morning} evening={evening} exceeds max diff {max_diff}"
                );
            }
        }
    }

    #[test]
    fn gen_schedule_single_staff() {
        let staff_ids = vec![Uuid::new_v4()];
        let config = default_config();
        let rules = config.build_rules();
        let assignments = gen_schedule(&staff_ids, monday(), &rules).unwrap();
        validate_schedule(&assignments, &staff_ids, &config);
    }

    #[test]
    fn gen_schedule_multiple_staff() {
        let staff_ids: Vec<_> = (0..4).map(|_| Uuid::new_v4()).collect();
        let config = default_config();
        let rules = config.build_rules();
        let assignments = gen_schedule(&staff_ids, monday(), &rules).unwrap();
        assert_eq!(assignments.len(), 4 * PERIOD_DAYS);
        validate_schedule(&assignments, &staff_ids, &config);
    }

    #[test]
    fn gen_schedule_empty_staff() {
        let config = default_config();
        let rules = config.build_rules();
        let output = gen_schedule(&[], monday(), &rules).unwrap();
        assert!(output.is_empty());
    }

    #[test]
    fn gen_schedule_relaxed_config() {
        let staff_ids: Vec<_> = (0..4).map(|_| Uuid::new_v4()).collect();
        let config = SchedulingConfig {
            no_morning_after_evening: false,
            max_daily_shift_diff: Some(4),
            ..default_config()
        };
        let rules = config.build_rules();
        let assignments = gen_schedule(&staff_ids, monday(), &rules).unwrap();
        validate_schedule(&assignments, &staff_ids, &config);
    }

    #[test]
    fn gen_schedule_large_staff_group() {
        let staff_ids: Vec<_> = (0..20).map(|_| Uuid::new_v4()).collect();
        let config = default_config();
        let rules = config.build_rules();
        let assignments = gen_schedule(&staff_ids, monday(), &rules).unwrap();
        assert_eq!(assignments.len(), 20 * PERIOD_DAYS);
        validate_schedule(&assignments, &staff_ids, &config);
    }
}
