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

    /// A conference with several tracks (ETAPS: ESOP, TACAS) is listed once
    /// per track; the first entry is its primary track. What belongs to the
    /// conference rather than a track, the conference dates and the
    /// student-volunteer deadline, is published from the primary track's
    /// records, and only the primary track searches for volunteers.
    pub fn is_primary(&self, all: &[ConferenceCfg]) -> bool {
        all.iter().find(|c| c.conference.eq_ignore_ascii_case(&self.conference)).is_none_or(|p| p.track.eq_ignore_ascii_case(&self.track))
    }
}

/// The tracks of a conference, in the order listed (primary first).
pub fn tracks_of(all: &[ConferenceCfg], conference: &str) -> Vec<String> {
    all.iter().filter(|c| c.conference.eq_ignore_ascii_case(conference)).map(|c| c.track.clone()).collect()
}

pub fn load(path: &Path) -> Result<Vec<ConferenceCfg>> {
    let text = std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let cfgs: Vec<ConferenceCfg> = serde_json::from_str(&text).with_context(|| format!("parsing {}", path.display()))?;
    Ok(cfgs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_listed_track_of_a_conference_is_primary() {
        let mk = |c: &str, t: &str| ConferenceCfg { conference: c.into(), track: t.into(), since: 2026 };
        let all = vec![mk("POPL", "POPL"), mk("ETAPS", "ESOP"), mk("ETAPS", "TACAS")];
        assert!(all[0].is_primary(&all) && all[1].is_primary(&all) && !all[2].is_primary(&all));
        assert!(mk("CAV", "CAV").is_primary(&all), "a conference not in the list is its own primary");
        assert_eq!(tracks_of(&all, "ETAPS"), vec!["ESOP", "TACAS"]);
        assert_eq!(tracks_of(&all, "POPL"), vec!["POPL"]);
        assert_eq!(all[2].label(2027), "ETAPS 2027 (TACAS)");
    }
}
