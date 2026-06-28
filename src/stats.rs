use std::collections::HashSet;

use chrono::NaiveDate;

/// Aggregate stats for a single habit, derived purely from the set of dates on
/// which it reached its goal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct HabitStats {
    /// Total number of days the goal was reached.
    pub total: u32,
    /// Length of the streak ending today (counting today, or yesterday if today
    /// is not done yet).
    pub current_streak: u32,
    /// Longest run of consecutive reached days, ever.
    pub longest_streak: u32,
    /// Reached days as a percentage of the span since the first reached day.
    pub completion_rate: u32,
}

/// Compute [`HabitStats`] from the days a habit reached its goal.
///
/// `reached` need not be sorted or deduplicated. `today` anchors the current
/// streak and the completion-rate span.
pub fn habit_stats(reached: &[NaiveDate], today: NaiveDate) -> HabitStats {
    let mut reached: Vec<NaiveDate> = reached.to_vec();
    reached.sort_unstable();
    reached.dedup();

    let total = reached.len() as u32;

    // longest run of consecutive reached days
    let mut longest = 0u32;
    let mut run = 0u32;
    let mut prev: Option<NaiveDate> = None;
    for &d in &reached {
        run = match prev {
            Some(p) if p.succ_opt() == Some(d) => run + 1,
            _ => 1,
        };
        longest = longest.max(run);
        prev = Some(d);
    }

    let reached_set: HashSet<NaiveDate> = reached.iter().copied().collect();

    // current streak: count back from today, tolerating a still-open today
    // (start from yesterday if today isn't done yet)
    let mut current = 0u32;
    let mut day = if reached_set.contains(&today) {
        Some(today)
    } else {
        today.pred_opt()
    };
    while let Some(d) = day {
        if reached_set.contains(&d) {
            current += 1;
            day = d.pred_opt();
        } else {
            break;
        }
    }

    // completion rate over the span since the first reached day
    let completion_rate = match reached.first() {
        Some(&first) => {
            let span = (today - first).num_days() + 1;
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

    #[test]
    fn empty_is_all_zero() {
        assert_eq!(habit_stats(&[], d(2026, 6, 28)), HabitStats::default());
    }

    #[test]
    fn single_day_done_today() {
        let today = d(2026, 6, 28);
        let s = habit_stats(&[today], today);
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
        let s = habit_stats(&dates, d(2026, 6, 28));
        assert_eq!(s.longest_streak, 3);
        assert_eq!(s.total, 5);
    }

    #[test]
    fn current_streak_counts_back_from_today() {
        let today = d(2026, 6, 28);
        let dates = [d(2026, 6, 26), d(2026, 6, 27), d(2026, 6, 28)];
        assert_eq!(habit_stats(&dates, today).current_streak, 3);
    }

    #[test]
    fn current_streak_tolerates_an_open_today() {
        // today isn't done, but the three days before it are: streak is still 3
        let today = d(2026, 6, 28);
        let dates = [d(2026, 6, 25), d(2026, 6, 26), d(2026, 6, 27)];
        assert_eq!(habit_stats(&dates, today).current_streak, 3);
    }

    #[test]
    fn current_streak_breaks_on_a_gap() {
        let today = d(2026, 6, 28);
        // yesterday missing, so a done-today is a streak of exactly 1
        let dates = [d(2026, 6, 20), d(2026, 6, 28)];
        assert_eq!(habit_stats(&dates, today).current_streak, 1);
    }

    #[test]
    fn unsorted_and_duplicate_input_is_normalised() {
        let today = d(2026, 6, 28);
        let dates = [d(2026, 6, 2), d(2026, 6, 1), d(2026, 6, 2)];
        let s = habit_stats(&dates, today);
        assert_eq!(s.total, 2);
        assert_eq!(s.longest_streak, 2);
    }

    #[test]
    fn completion_rate_is_reached_over_span() {
        // 2 of the 4 days in the span [25..28] were reached -> 50%
        let today = d(2026, 6, 28);
        let dates = [d(2026, 6, 25), d(2026, 6, 27)];
        assert_eq!(habit_stats(&dates, today).completion_rate, 50);
    }
}
