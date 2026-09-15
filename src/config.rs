//! `conferences.json`: the only hand-edited input.

use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Clone, Deserialize)]
pub struct ConferenceCfg {
    /// Conference name as used on the web, e.g. "SPLASH".
    pub conference: String,
    /// Research-paper track to extract, e.g. "OOPSLA". Often equal to `conference`.
    pub track: String,
    /// First year to collect.
    pub since: i32,
}

impl ConferenceCfg {
    /// "POPL 2026" or "SPLASH 2026 (OOPSLA)".
    pub fn label(&self, year: i32) -> String {
        if self.track.eq_ignore_ascii_case(&self.conference) {
            format!("{} {year}", self.conference)
        } else {
            format!("{} {year} ({})", self.conference, self.track)
        }
    }

    /// Directory key `CONF/TRACK/YEAR`.
    pub fn key(&self, year: i32) -> String {
        format!("{}/{}/{year}", self.conference, self.track)
    }
}

pub fn load(path: &Path) -> Result<Vec<ConferenceCfg>> {
    let text = std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let cfgs: Vec<ConferenceCfg> = serde_json::from_str(&text).with_context(|| format!("parsing {}", path.display()))?;
    Ok(cfgs)
}
