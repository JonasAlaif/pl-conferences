use core::fmt;

use super::Conference;

macro_rules! display {
    ($conf:expr, $fmt:literal) => {{
        struct Wrapper<'a>(&'a Conference);
        impl fmt::Display for Wrapper<'_> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, $fmt, conf = self.0)
            }
        }
        Wrapper($conf)
    }};
}

impl Conference {
    pub fn correct_search(&self) -> impl fmt::Display + '_ {
        display!(
            self,
            "Is the above search result the call for papers page for the '{conf}' conference?"
        )
    }

    pub fn correct_page(&self) -> impl fmt::Display + '_ {
        display!(self, "Does the above webpage look like a page about the '{conf}' conference? It must include the conference name and year.")
    }

    pub fn details(&self) -> impl fmt::Display + '_ {
        display!(self, "The above page is about '{conf}'. Find all the required information about this conference.")
    }

    pub fn two_rounds(&self) -> impl fmt::Display + '_ {
        display!(self, "The above page is about '{conf}'. Does this conference have two rounds of submissions? Answer no if there is only one round, or if the number of rounds is not clear.")
    }

    pub fn summary_table(&self) -> impl fmt::Display + '_ {
        display!(self, "The above page is about '{conf}'. Summarize all the dates from this document. Include **all** dates and ranges/durations in the document. All must have a proper description.")
    }

    pub fn dates(&self) -> impl fmt::Display + '_ {
        display!(self, "The list above contains all important dates for '{conf}'. Summarize the dates in this list as a json structure.")
    }
}
