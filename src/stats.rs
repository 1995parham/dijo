use std::collections::HashSet;

use chrono::{Datelike, NaiveDate};

use crate::habit::GoalPeriod;
use crate::utils::week_bounds;

/// Aggregate stats for a single habit, derived purely from the set of dates on
/// which it reached its goal. The unit of counting is the habit's goal period:
/// days for a daily goal, whole weeks for a weekly one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct HabitStats {
    /// Total number of periods (days/weeks) the goal was reached.
    pub total: u32,
    /// Length of the streak ending in the current period (counting it, or the
    /// previous one if the current period is not done yet).
    pub current_streak: u32,
    /// Longest run of consecutive reached periods, ever.
    pub longest_streak: u32,
    /// Reached periods as a percentage of the span since the first reached one.
    pub completion_rate: u32,
}

/// Map a date onto a monotonic index for its goal period, so that consecutive
/// periods differ by exactly 1. Daily uses the day number; weekly uses the
/// week's Monday collapsed to a week count.
fn period_index(date: NaiveDate, period: GoalPeriod) -> i64 {
    match period {
        GoalPeriod::Daily => date.num_days_from_ce() as i64,
        // Adding 7 days always bumps the quotient by exactly 1, so successive
        // Mondays land on successive indices.
        GoalPeriod::Weekly => week_bounds(date).0.num_days_from_ce() as i64 / 7,
    }
}

/// Compute [`HabitStats`] from the days a habit reached its goal.
///
/// `reached` need not be sorted or deduplicated; days that fall in the same
/// period collapse into one. `today` anchors the current streak and the
/// completion-rate span, and `period` sets the counting unit.
pub fn habit_stats(reached: &[NaiveDate], today: NaiveDate, period: GoalPeriod) -> HabitStats {
    let mut idx: Vec<i64> = reached.iter().map(|&d| period_index(d, period)).collect();
    idx.sort_unstable();
    idx.dedup();

    let total = idx.len() as u32;

    // longest run of consecutive reached periods
    let mut longest = 0u32;
    let mut run = 0u32;
    let mut prev: Option<i64> = None;
    for &i in &idx {
        run = match prev {
            Some(p) if p + 1 == i => run + 1,
            _ => 1,
        };
        longest = longest.max(run);
        prev = Some(i);
    }

    let reached_set: HashSet<i64> = idx.iter().copied().collect();

    // current streak: count back from the current period, tolerating a still-
    // open one (start from the previous period if this one isn't done yet)
    let today_idx = period_index(today, period);
    let mut current = 0u32;
    let mut cur = if reached_set.contains(&today_idx) {
        today_idx
    } else {
        today_idx - 1
    };
    while reached_set.contains(&cur) {
        current += 1;
        cur -= 1;
    }

    // completion rate over the span since the first reached period
    let completion_rate = match idx.first() {
        Some(&first) => {
            let span = today_idx - first + 1;
            if span > 0 {
                (total as i64 * 100 / span) as u32
            } else {
                0
            }
        }
        None => 0,
    };

    HabitStats {
        total,
        current_streak: current,
        longest_streak: longest,
        completion_rate,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    fn daily(reached: &[NaiveDate], today: NaiveDate) -> HabitStats {
        habit_stats(reached, today, GoalPeriod::Daily)
    }

    fn weekly(reached: &[NaiveDate], today: NaiveDate) -> HabitStats {
        habit_stats(reached, today, GoalPeriod::Weekly)
    }

    #[test]
    fn empty_is_all_zero() {
        assert_eq!(daily(&[], d(2026, 6, 28)), HabitStats::default());
    }

    #[test]
    fn single_day_done_today() {
        let today = d(2026, 6, 28);
        let s = daily(&[today], today);
        assert_eq!(s.total, 1);
        assert_eq!(s.current_streak, 1);
        assert_eq!(s.longest_streak, 1);
        assert_eq!(s.completion_rate, 100);
    }

    #[test]
    fn longest_streak_picks_the_longest_run() {
        // a 3-day run, a gap, then a 2-day run
        let dates = [
            d(2026, 6, 1),
            d(2026, 6, 2),
            d(2026, 6, 3),
            d(2026, 6, 10),
            d(2026, 6, 11),
        ];
        let s = daily(&dates, d(2026, 6, 28));
        assert_eq!(s.longest_streak, 3);
        assert_eq!(s.total, 5);
    }

    #[test]
    fn current_streak_counts_back_from_today() {
        let today = d(2026, 6, 28);
        let dates = [d(2026, 6, 26), d(2026, 6, 27), d(2026, 6, 28)];
        assert_eq!(daily(&dates, today).current_streak, 3);
    }

    #[test]
    fn current_streak_tolerates_an_open_today() {
        // today isn't done, but the three days before it are: streak is still 3
        let today = d(2026, 6, 28);
        let dates = [d(2026, 6, 25), d(2026, 6, 26), d(2026, 6, 27)];
        assert_eq!(daily(&dates, today).current_streak, 3);
    }

    #[test]
    fn current_streak_breaks_on_a_gap() {
        let today = d(2026, 6, 28);
        // yesterday missing, so a done-today is a streak of exactly 1
        let dates = [d(2026, 6, 20), d(2026, 6, 28)];
        assert_eq!(daily(&dates, today).current_streak, 1);
    }

    #[test]
    fn unsorted_and_duplicate_input_is_normalised() {
        let today = d(2026, 6, 28);
        let dates = [d(2026, 6, 2), d(2026, 6, 1), d(2026, 6, 2)];
        let s = daily(&dates, today);
        assert_eq!(s.total, 2);
        assert_eq!(s.longest_streak, 2);
    }

    #[test]
    fn completion_rate_is_reached_over_span() {
        // 2 of the 4 days in the span [25..28] were reached -> 50%
        let today = d(2026, 6, 28);
        let dates = [d(2026, 6, 25), d(2026, 6, 27)];
        assert_eq!(daily(&dates, today).completion_rate, 50);
    }

    // ---- weekly counting ----
    // 2024-01-01 is a Monday, so each Mon–Sun block is one week:
    //   wk0: Jan 01..07, wk1: Jan 08..14, wk2: Jan 15..21, wk3: Jan 22..28.

    #[test]
    fn weekly_collapses_days_of_a_week_into_one() {
        // three separate days in the same week count as a single reached week
        let dates = [d(2024, 1, 1), d(2024, 1, 3), d(2024, 1, 6)];
        let s = weekly(&dates, d(2024, 1, 28));
        assert_eq!(s.total, 1);
        assert_eq!(s.longest_streak, 1);
    }

    #[test]
    fn weekly_streak_counts_consecutive_weeks() {
        // one reached day in each of three back-to-back weeks
        let dates = [d(2024, 1, 2), d(2024, 1, 9), d(2024, 1, 16)];
        let s = weekly(&dates, d(2024, 1, 16));
        assert_eq!(s.total, 3);
        assert_eq!(s.current_streak, 3);
        assert_eq!(s.longest_streak, 3);
    }

    #[test]
    fn weekly_streak_tolerates_an_open_current_week() {
        // this week (wk3, containing the 28th) has nothing yet, but the two
        // prior weeks are done: the streak still stands at 2
        let dates = [d(2024, 1, 9), d(2024, 1, 16)];
        assert_eq!(weekly(&dates, d(2024, 1, 28)).current_streak, 2);
    }

    #[test]
    fn weekly_streak_breaks_on_a_skipped_week() {
        // wk0 done, wk1 skipped, wk3 (current) done -> streak of exactly 1
        let dates = [d(2024, 1, 1), d(2024, 1, 22)];
        assert_eq!(weekly(&dates, d(2024, 1, 22)).current_streak, 1);
    }

    #[test]
    fn weekly_completion_rate_is_over_weeks() {
        // reached wk0 and wk2 of the 4-week span wk0..wk3 -> 2/4 = 50%
        let dates = [d(2024, 1, 1), d(2024, 1, 15)];
        assert_eq!(weekly(&dates, d(2024, 1, 28)).completion_rate, 50);
    }
}
