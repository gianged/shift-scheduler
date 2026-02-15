use chrono::{Duration, NaiveDate};
use shared::types::ShiftType;
use thiserror::Error;
use uuid::Uuid;

use crate::domain::job::NewShiftAssignment;

const PERIOD_DAYS: usize = 28;
const DAYS_PER_WEEK: usize = 7;

#[derive(Debug, Clone)]
pub struct SchedulingConfig {
    pub min_day_off_per_week: u8,
    pub max_day_off_per_week: u8,
    pub no_morning_after_evening: bool,
    pub max_daily_shift_diff: u8,
}

impl Default for SchedulingConfig {
    fn default() -> Self {
        Self {
            min_day_off_per_week: 1,
            max_day_off_per_week: 2,
            no_morning_after_evening: true,
            max_daily_shift_diff: 1,
        }
    }
}

#[derive(Debug, Error)]
pub enum SchedulingError {
    #[error("No valid shift found for staff {staff_id} on day {day}")]
    NoValidShift { staff_id: Uuid, day: usize },
}

// region: Contraisn functions

fn is_morning_after_evening_ok(
    previous: Option<&ShiftType>,
    current: &ShiftType,
    config: &SchedulingConfig,
) -> bool {
    if !config.no_morning_after_evening {
        return true;
    }
    !matches!(
        (previous, current),
        (Some(ShiftType::Evening), ShiftType::Morning)
    )
}

fn is_day_off_within_max(
    day_offs_this_week: u8,
    current: &ShiftType,
    config: &SchedulingConfig,
) -> bool {
    if *current != ShiftType::DayOff {
        return true;
    }
    day_offs_this_week < config.max_day_off_per_week
}

fn can_still_meet_min_day_off(
    day_offs_this_week: u8,
    days_remaining_in_week: u8,
    current: &ShiftType,
    config: &SchedulingConfig,
) -> bool {
    if *current == ShiftType::DayOff {
        return true;
    }

    // check mininum after
    day_offs_this_week + days_remaining_in_week >= config.min_day_off_per_week
}

fn is_daily_balance_ok(
    morning_count: usize,
    evening_count: usize,
    current: &ShiftType,
    config: &SchedulingConfig,
) -> bool {
    let (m, e) = match current {
        ShiftType::Morning => (morning_count + 1, evening_count),
        ShiftType::Evening => (morning_count, evening_count + 1),
        ShiftType::DayOff => return true,
    };
    m.abs_diff(e) <= config.max_daily_shift_diff as usize
}

// endregion: Contraisn functions

// region: Main algo

#[tracing::instrument(skip(staff_ids, config))]
pub fn gen_schedule(
    staff_ids: &[Uuid],
    period_begin_date: NaiveDate,
    config: &SchedulingConfig,
) -> Result<Vec<NewShiftAssignment>, SchedulingError> {
    tracing::debug!(
        staff_count = staff_ids.len(),
        "Starting schedule generation"
    );

    let shift_options = [ShiftType::Morning, ShiftType::Evening, ShiftType::DayOff];
    let mut assignments: Vec<NewShiftAssignment> = Vec::new();

    // per staff track: pervious day shift
    let mut previous_shifts: Vec<Option<ShiftType>> = vec![None; staff_ids.len()];
    //per staff track: day off count in current week
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
                let ok = is_morning_after_evening_ok(previous_shifts[i].as_ref(), shift, config)
                    && is_day_off_within_max(weekly_day_offs[i], shift, config)
                    && can_still_meet_min_day_off(
                        weekly_day_offs[i],
                        days_remaining_in_week,
                        shift,
                        config,
                    )
                    && is_daily_balance_ok(morning_count, evening_count, shift, config);

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

    // Constraint function tests

    #[test]
    fn morning_after_evening_blocked() {
        let config = default_config();
        assert!(!is_morning_after_evening_ok(
            Some(&ShiftType::Evening),
            &ShiftType::Morning,
            &config,
        ));
    }

    #[test]
    fn morning_after_evening_allowed_when_disabled() {
        let config = SchedulingConfig {
            no_morning_after_evening: false,
            ..default_config()
        };
        assert!(is_morning_after_evening_ok(
            Some(&ShiftType::Evening),
            &ShiftType::Morning,
            &config,
        ));
    }

    #[test]
    fn day_off_within_max() {
        let config = default_config(); // max 2
        assert!(!is_day_off_within_max(2, &ShiftType::DayOff, &config));
    }

    #[test]
    fn day_off_within_max_ok() {
        let config = default_config(); // max 2
        assert!(is_day_off_within_max(1, &ShiftType::DayOff, &config));
    }

    #[test]
    fn can_still_meet_min_day_off_forces_day_off() {
        let config = default_config(); // min 1
        // 0 day offs, 0 days remaining -> working shift must be rejected
        assert!(!can_still_meet_min_day_off(
            0,
            0,
            &ShiftType::Morning,
            &config,
        ));
    }

    #[test]
    fn daily_balance_ok_rejects_imbalance() {
        let config = default_config(); // max_diff 1
        // 3 morning, 1 evening -> adding morning would make diff 3
        assert!(!is_daily_balance_ok(3, 1, &ShiftType::Morning, &config));
    }

    #[test]
    fn daily_balance_ok_day_off_always_passes() {
        let config = default_config();
        assert!(is_daily_balance_ok(10, 0, &ShiftType::DayOff, &config));
    }

    // gen_schedule test

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
        let assignments = gen_schedule(&staff_ids, monday(), &config).unwrap();
        validate_schedule(&assignments, &staff_ids, &config);
    }

    #[test]
    fn gen_schedule_multiple_staff() {
        let staff_ids: Vec<_> = (0..4).map(|_| Uuid::new_v4()).collect();
        let config = default_config();
        let assignments = gen_schedule(&staff_ids, monday(), &config).unwrap();
        assert_eq!(assignments.len(), 4 * PERIOD_DAYS);
        validate_schedule(&assignments, &staff_ids, &config);
    }

    #[test]
    fn gen_schedule_empty_staff() {
        let config = default_config();
        let output = gen_schedule(&[], monday(), &config).unwrap();
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
        let assignments = gen_schedule(&staff_ids, monday(), &config).unwrap();
        validate_schedule(&assignments, &staff_ids, &config);
    }
}
