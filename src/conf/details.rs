use chrono::NaiveDate;

pub struct ConferenceAll {
    pub details: ConferenceDetails,
    pub dates: AllDates,
}

pub struct ConferenceDetails {
    pub city: String,
    pub country: String,
    pub start: NaiveDate,
    pub end: NaiveDate,
}

pub enum AllDates {
    OneRound(ImportantDates),
    TwoRounds(ImportantDates, ImportantDates),
}

pub struct ImportantDates {
    pub paper_submission: NaiveDate,
    pub author_response_start: NaiveDate,
    pub author_response_end: NaiveDate,
    pub notification: NaiveDate,
}

impl From<crate::schema::ConferenceDetails> for ConferenceDetails {
    fn from(value: crate::schema::ConferenceDetails) -> Self {
        Self {
            city: value.conference_city,
            country: value.conference_country,
            start: value.start_of_conference.into(),
            end: value.end_of_conference.into(),
        }
    }
}

impl From<crate::schema::DatesTwoRounds> for (ImportantDates, ImportantDates) {
    fn from(value: crate::schema::DatesTwoRounds) -> Self {
        (value.round_1.into(), value.round_2.into())
    }
}

impl From<crate::schema::DatesOneRound> for ImportantDates {
    fn from(value: crate::schema::DatesOneRound) -> Self {
        value.dates.into()
    }
}

impl From<crate::schema::ImportantDates> for ImportantDates {
    fn from(value: crate::schema::ImportantDates) -> Self {
        Self {
            paper_submission: value.paper_submission.paper_submission_deadline.into(),
            author_response_start: value.author_response.response_start.into(),
            author_response_end: value.author_response.response_end.into(),
            notification: value.notification.notification_date.into(),
        }
    }
}

impl ImportantDates {
    pub fn check_valid(self) -> Option<Self> {
        if self.paper_submission < self.author_response_start
            && self.author_response_start < self.author_response_end
            && self.author_response_end < self.notification
        {
            Some(self)
        } else {
            None
        }
    }
}
