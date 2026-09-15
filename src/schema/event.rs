use schemars::JsonSchema;
use serde::Deserialize;

use super::Date;

#[derive(Debug, JsonSchema, Deserialize)]
pub struct ImportantDates {
    pub paper_submission: PaperSubmission,
    pub author_response: AuthorResponse,
    pub notification: Notification,
}

// prompt!(PaperSubmissionExplanation => "The deadline for submitting papers.");

#[derive(Debug, JsonSchema, Deserialize)]
pub struct PaperSubmission {
    // pub explanation: PaperSubmissionExplanation,
    pub paper_submission_deadline: Date,
}

// prompt!(AuthorResponseExplanation => "The period during which authors receive feedback on their papers and write a response for the reviewers. Usually lasts a few days. Usually starts a few weeks or months after the submissions deadline.");

#[derive(Debug, JsonSchema, Deserialize)]
pub struct AuthorResponse {
    // pub explanation: AuthorResponseExplanation,
    pub response_start: Date,
    pub response_end: Date,
}

// prompt!(NotificationExplanation => "The date when authors are notified of the acceptance or rejection of their papers. Usually a few weeks or months after the author response period.");

#[derive(Debug, JsonSchema, Deserialize)]
pub struct Notification {
    // pub explanation: NotificationExplanation,
    pub notification_date: Date,
}
