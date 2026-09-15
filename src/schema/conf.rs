use schemars::JsonSchema;
use serde::Deserialize;

use super::{Date, ImportantDates};

#[derive(Debug, JsonSchema, Deserialize)]
pub struct ConferenceDetails {
    #[validate(length(min = 1))]
    pub conference_and_year: String,
    #[validate(length(min = 1))]
    pub conference_city: String,
    #[validate(length(min = 1))]
    pub conference_country: String,
    pub start_of_conference: Date,
    pub end_of_conference: Date,
}

// prompt!(RoundsExplanation => "The conference has two rounds of paper submissions, each roughly half a year apart. In `summary_of_round_1_dates` and `summary_of_round_2_dates`, we include a very detailed summary of all relevant dates of the respective round. The summary includes **all** dates for the round. This is used to fill out the other fields.");

#[derive(Debug, JsonSchema, Deserialize)]
pub struct DatesTwoRounds {
    // pub explanation: RoundsExplanation,
    // pub summary_of_round_1_dates: String,
    pub round_1: ImportantDates,
    // pub summary_of_round_2_dates: String,
    pub round_2: ImportantDates,
}

// prompt!(ReasoningInstruction => "We include a very detailed summary of all relevant dates in the page. All relevant dates are included. This is used this to fill out the other fields.");

#[derive(Debug, JsonSchema, Deserialize)]
pub struct DatesOneRound {
    // pub explanation: ReasoningInstruction,
    // pub summary_of_dates: String,
    pub dates: ImportantDates,
}

// prompt!(ImportantDatesR2Instruction => "Start by summarising all the important dates for round 2. Ignore the dates for round 1.");

// #[derive(Debug, JsonSchema, Deserialize)]
// pub struct ImportantDatesR2 {
//     pub instructions: ImportantDatesR2Instruction,
//     pub summary_of_dates: String,
//     pub dates: ImportantDates
// }
