use std::fmt::Display;

#[derive(Clone, Copy)]
pub struct Round(Option<bool>);

impl Round {
    pub fn no_rounds() -> Self {
        Self(None)
    }
    pub fn first_round() -> Self {
        Self(Some(false))
    }
    pub fn second_round() -> Self {
        Self(Some(true))
    }

    fn round_suffix(self) -> &'static str {
        match self {
            Round(None) => "",
            Round(Some(false)) => " for the first round",
            Round(Some(true)) => " for the second round",
        }
    }

    fn round_focus(self) -> &'static str {
        match self {
            Round(None) => "",
            Round(Some(false)) => " Look for the first round (R1) only!",
            Round(Some(true)) => " Look for the second round (R2) only!",
        }
    }
}

impl Conference {
    pub fn submission_deadline(self, round: Round) -> impl Display {
        struct SubmissionDeadline(Conference, Round);
        impl Display for SubmissionDeadline {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "The above page is about '{}'. What date is the paper submission deadline{}?{}", self.0, self.1.round_suffix(), self.1.round_focus())
            }
        }
        SubmissionDeadline(self, round)
    }

    pub fn rebuttal_start(self, round: Round) -> impl Display {
        struct RebuttalStart(Conference, Round);
        impl Display for RebuttalStart {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "The above page is about '{}'. What date does the author response period start{}? Be extra careful about choosing the correct year.{}", self.0, self.1.round_suffix(), self.1.round_focus())
            }
        }
        RebuttalStart(self, round)
    }

    pub fn rebuttal_end(self, round: Round) -> impl Display {
        struct RebuttalEnd(Conference, Round);
        impl Display for RebuttalEnd {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "The above page is about '{}'. What date does the author response period end{}? Be extra careful about choosing the correct year.{}", self.0, self.1.round_suffix(), self.1.round_focus())
            }
        }
        RebuttalEnd(self, round)
    }
}
