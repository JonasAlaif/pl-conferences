use core::fmt;

use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Debug, JsonSchema, Deserialize)]
pub struct DateTable {
    pub important_dates: Vec<ImportantDate>,
}

#[derive(Debug, JsonSchema, Deserialize)]
pub struct ImportantDate {
    pub details: String,
    pub date: String,
}

impl fmt::Display for DateTable {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "# Important Dates")?;
        for date in &self.important_dates {
            writeln!(f, "- {}: {}", date.details, date.date)?;
        }
        Ok(())
    }
}
