use std::path::Path;

use chrono::{Duration, NaiveDate};
use chrono_tz::Tz;
use serde::Deserialize;
use shared::types::ShiftType;
use thiserror::Error;
use uuid::Uuid;

use crate::domain::job::NewShiftAssignment;

const PERIOD_DAYS: usize = 28;
const DAYS_PER_WEEK: usize = 7;

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct SchedulingConfig {
    pub timezone: String,
    pub min_day_off_per_week: u8,
    pub max_day_off_per_week: u8,
    pub no_morning_after_evening: bool,
    pub max_daily_shift_diff: u8,
}

impl Default for SchedulingConfig {
    fn default() -> Self {
        Self {
            timezone: "UTC".to_string(),
            min_day_off_per_week: 1,
            max_day_off_per_week: 2,
            no_morning_after_evening: true,
            max_daily_shift_diff: 1,
        }
    }
}

impl SchedulingConfig {
    pub fn load(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        if !Path::new(path).exists() {
            tracing::info!("Config file not found at {path}, using defaults");
            return Ok(Self::default());
        }
        let content = std::fs::read_to_string(path)?;
        let config: Self = toml::from_str(&content)?;
        tracing::info!(?config, "Loaded scheduling config from {path}");
        Ok(config)
    }

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

#[derive(Debug, Error)]
pub enum SchedulingError {
    #[error("No valid shift found for staff {staff_id} on day {day}")]
    NoValidShift { staff_id: Uuid, day: usize },
}

// region: Trait-based scheduling rules

pub struct AssignmentContext {
    pub previous_shift: Option<ShiftType>,
    pub day_offs_this_week: u8,
    pub days_remaining_in_week: u8,
    pub morning_count: usize,
    pub evening_count: usize,
}

pub trait SchedulingRule: Send + Sync {
    fn name(&self) -> &str;
    fn is_valid(&self, ctx: &AssignmentContext, candidate: &ShiftType) -> bool;
}

pub struct NoMorningAfterEveningRule;

impl SchedulingRule for NoMorningAfterEveningRule {
    fn name(&self) -> &str {
        "no_morning_after_evening"
    }

    fn is_valid(&self, ctx: &AssignmentContext, candidate: &ShiftType) -> bool {
        !matches!(
            (ctx.previous_shift.as_ref(), candidate),
            (Some(ShiftType::Evening), ShiftType::Morning)
        )
    }
}

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
    pub fn build_rules(&self) -> Vec<Box<dyn SchedulingRule>> {
        let mut rules: Vec<Box<dyn SchedulingRule>> = Vec::new();
        if self.no_morning_after_evening {
            rules.push(Box::new(NoMorningAfterEveningRule));
        }
        rules.push(Box::new(MaxDayOffRule {
            max: self.max_day_off_per_week,
        }));
        rules.push(Box::new(MinDayOffRule {
            min: self.min_day_off_per_week,
        }));
        rules.push(Box::new(DailyBalanceRule {
            max_diff: self.max_daily_shift_diff,
        }));
        rules
    }
}

// endregion: Trait-based scheduling rules

// region: Main algo

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

    let shift_options = [ShiftType::Morning, ShiftType::Evening, ShiftType::DayOff];
    let mut assignments: Vec<NewShiftAssignment> = Vec::new();

    // per staff track both fields
    let mut previous_shifts: Vec<Option<ShiftType>> = vec![None; staff_ids.len()];
    let mut weekly_day_offs: Vec<u8> = vec![0; staff_ids.len()];

    for day in 0..PERIOD_DAYS {
        let date = period_begin_date + Duration::days(day as i64);
        let day_in_week = day % DAYS_PER_WEEK;
        let days_remaining_in_week = (DAYS_PER_WEEK - 1 - day_in_week) as u8;

        // Weekly counter reset on Monday
        if day_in_week == 0 {
            weekly_day_offs.fill(0);
        }

        // track daily shift count for balance constraint
        let mut morning_count: usize = 0;
        let mut evening_count: usize = 0;

        for (i, staff_id) in staff_ids.iter().enumerate() {
            let mut assigned = false;

            for shift in &shift_options {
                let ctx = AssignmentContext {
                    previous_shift: previous_shifts[i].clone(),
                    day_offs_this_week: weekly_day_offs[i],
                    days_remaining_in_week,
                    morning_count,
                    evening_count,
                };

                let ok = rules.iter().all(|rule| rule.is_valid(&ctx, shift));

                if ok {
                    if *shift == ShiftType::DayOff {
                        weekly_day_offs[i] += 1;
                    }
                    if *shift == ShiftType::Morning {
                        morning_count += 1;
                    }
                    if *shift == ShiftType::Evening {
                        evening_count += 1;
                    }

                    previous_shifts[i] = Some(shift.clone());
                    assignments.push(NewShiftAssignment {
                        staff_id: *staff_id,
                        date,
                        shift_type: shift.clone(),
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
                assert!(
                    day_offs >= config.min_day_off_per_week,
                    "Staff {sid} week {week}: {day_offs} < min {}",
                    config.min_day_off_per_week
                );
                assert!(
                    day_offs <= config.max_day_off_per_week,
                    "Staff {sid} week {week}: {day_offs} > max {}",
                    config.max_day_off_per_week
                );
            }
        }

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
                morning.abs_diff(evening) <= config.max_daily_shift_diff as usize,
                "Day {date}: morning={morning} evening={evening} exceeds max diff {}",
                config.max_daily_shift_diff
            );
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
            max_daily_shift_diff: 4,
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
