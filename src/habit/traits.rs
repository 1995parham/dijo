use chrono::{Datelike, Days, Local, NaiveDate};
use cursive::direction::Direction;
use cursive::event::{Event, EventResult};
use cursive::view::CannotFocus;
use cursive::{Printer, Vec2};

use crate::command::GoalKind;
use crate::habit::{Bit, Count, Float, InnerData, TrackEvent};
use crate::views::ShadowView;

pub trait Habit {
    type HabitType;

    fn get_by_date(&self, date: NaiveDate) -> Option<&Self::HabitType>;
    fn get_dates(&self) -> Vec<NaiveDate>;
    fn goal(&self) -> u32;
    fn insert_entry(&mut self, date: NaiveDate, val: Self::HabitType);
    fn modify(&mut self, date: NaiveDate, event: TrackEvent);
    fn name(&self) -> String;
    fn reached_goal(&self, date: NaiveDate) -> bool;
    fn remaining(&self, date: NaiveDate) -> u32;
    fn kind(&self) -> GoalKind;

    fn inner_data_ref(&self) -> &InnerData;
    fn inner_data_mut_ref(&mut self) -> &mut InnerData;
}

#[typetag::serde(tag = "type")]
pub trait HabitWrapper: erased_serde::Serialize + Sync + Send {
    fn draw(&self, printer: &Printer);
    fn get_dates(&self) -> Vec<NaiveDate>;
    fn goal(&self) -> u32;
    fn kind(&self) -> GoalKind;
    fn modify(&mut self, date: NaiveDate, event: TrackEvent);
    fn name(&self) -> String;
    fn on_event(&mut self, event: Event) -> EventResult;
    fn remaining(&self, date: NaiveDate) -> u32;
    fn required_size(&mut self, _: Vec2) -> Vec2;
    fn take_focus(&mut self, _: Direction) -> Result<EventResult, CannotFocus>;
    fn missed_dates(&self) -> Vec<NaiveDate>;

    fn inner_data_ref(&self) -> &InnerData;
    fn inner_data_mut_ref(&mut self) -> &mut InnerData;
}

macro_rules! auto_habit_impl {
    ($struct_name:ident) => {
        #[typetag::serde]
        impl HabitWrapper for $struct_name {
            // ShadowView
            fn draw(&self, printer: &Printer) {
                ShadowView::draw(self, printer)
            }
            fn on_event(&mut self, event: Event) -> EventResult {
                ShadowView::on_event(self, event)
            }
            fn required_size(&mut self, x: Vec2) -> Vec2 {
                ShadowView::required_size(self, x)
            }
            fn take_focus(&mut self, d: Direction) -> Result<EventResult, CannotFocus> {
                ShadowView::take_focus(self, d)
            }

            // Habit
            fn get_dates(&self) -> Vec<NaiveDate> {
                Habit::get_dates(self)
            }
            fn remaining(&self, date: NaiveDate) -> u32 {
                Habit::remaining(self, date)
            }
            fn goal(&self) -> u32 {
                Habit::goal(self)
            }
            fn kind(&self) -> GoalKind {
                Habit::kind(self)
            }
            fn modify(&mut self, date: NaiveDate, event: TrackEvent) {
                Habit::modify(self, date, event);
            }
            fn name(&self) -> String {
                Habit::name(self)
            }
            fn inner_data_ref(&self) -> &InnerData {
                Habit::inner_data_ref(self)
            }
            fn inner_data_mut_ref(&mut self) -> &mut InnerData {
                Habit::inner_data_mut_ref(self)
            }
            fn missed_dates(&self) -> Vec<NaiveDate> {
                let today = Local::now().date_naive();
                let mut index = today.with_day(1).unwrap();

                let mut days: Vec<NaiveDate> = Vec::new();

                while index < today {
                    if Habit::get_by_date(self, index).is_none() {
                        days.push(index.clone());
                    }

                    index = index + Days::new(1);
                }

                days
            }
        }
    };
}

macro_rules! generate_implementations {
    ($($x:ident),*) => (
        $(
            auto_habit_impl!($x);
        )*
    );
}

generate_implementations!(Count, Bit, Float);
