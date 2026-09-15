use std::borrow::Cow;

use schemars::{schema::Schema, JsonSchema, SchemaGenerator};
use serde::Deserialize;

// #[derive(JsonSchema, Deserialize)]
// pub struct ConferenceDates {
//     pub conference: String,
//     pub year: Year,
//     pub city: String,
//     pub country: String,
//     pub conference_dates: DateKind,
//     pub round_1_important_dates: Vec<ImportantEvent>,
//     pub round_2_important_dates: Vec<ImportantEvent>,
// }

// #[derive(JsonSchema, Deserialize)]
// pub struct ImportantEvent {
//     kind: EventKind,
//     name: String,
//     date: DateKind,
// }

// #[derive(JsonSchema, Deserialize)]
// pub enum EventKind {
//     PaperSubmissionDeadline,
//     AuthorResponsePeriod,
//     Notification,
//     Conference,
// }

// #[derive(JsonSchema, Deserialize)]
// pub enum DateKind {
//     OneDate(Date),
//     StartAndEnd {
//         from: Date,
//         to: Date,
//     },
// }

#[derive(Debug, Clone, Copy, JsonSchema, Deserialize)]
pub struct Date {
    #[validate(length(min = 2000, max = 2099))]
    year: i32,
    #[validate(length(min = 1, max = 12))]
    month: u32,
    #[validate(length(min = 1, max = 31))]
    day: u32,
}

impl From<Date> for chrono::NaiveDate {
    fn from(date: Date) -> Self {
        chrono::NaiveDate::from_ymd_opt(date.year, date.month, date.day).unwrap()
    }
}

impl Date {
    pub fn as_naive_date(self) -> chrono::NaiveDate {
        self.into()
    }
}
