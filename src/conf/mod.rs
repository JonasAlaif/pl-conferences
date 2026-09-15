mod details;
mod queries;
mod search;

pub use details::*;

use core::fmt;

#[derive(Clone)]
pub struct Conference {
    pub name: String,
    pub track: Option<String>,
    pub year: u16,
}

impl fmt::Display for Conference {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.name.fmt(f)?;
        if let Some(track) = &self.track {
            write!(f, " {}", track)?;
        };
        write!(f, " {}", self.year)
    }
}
