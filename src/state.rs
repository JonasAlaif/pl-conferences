//! `state.json`: what was attempted, what happened, and which maintenance
//! codes are active. Also renders `MAINTENANCE.md` and the README summary.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Ok,
    NotFound,
    FetchFailed,
    Invalid,
    NoVolunteerProgram,
    Abandoned,
    /// A serious error (e.g. Ollama unreachable); retried next run.
    Error,
}

/// Maintenance codes. Each marks a degraded-but-working path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Hash)]
pub enum Code {
    /// Primary search backend (Brave) returned nothing; a fallback backend was used.
    E001,
    /// All search backends failed; LLM URL guessing was used.
    E002,
    /// Page found only by link-following from a search hit.
    E003,
    /// Headless Chrome was needed to render the page.
    E004,
    /// Extraction needed the corrective retry.
    E005,
    /// Conference-year not found for three or more consecutive runs.
    E006,
    /// Ollama or the model was installed from a fallback source.
    E007,
    /// Primary search backend throttled or blocked the runner; a fallback backend was used.
    E008,
    /// A page could only be fetched by ignoring an invalid TLS certificate.
    E009,
    /// The model tag now resolves to a different build than the one pinned in the workflow.
    E010,
}

impl Code {
    pub fn hint(self) -> &'static str {
        match self {
            Code::E001 => "The primary search backend (Brave HTML) returned no results; a fallback backend was used. Check `src/search.rs` selectors if this persists.",
            Code::E002 => "Every search backend failed; the page was found by asking the model to guess URLs. Search parsing in `src/search.rs` probably needs updating.",
            Code::E003 => "The search hit did not contain the dates; they were found by following a link from it. Usually harmless, but check the query in `src/discover.rs` if it becomes common.",
            Code::E004 => "The page needed headless Chrome to render. Fine on GitHub runners; verify Chrome is still preinstalled if fetches start failing.",
            Code::E005 => "The model's first answer failed validation and a corrective retry was needed. Consider tuning the prompt or schema in `src/schema.rs`.",
            Code::E006 => "No page could be found for this conference-year in three or more consecutive runs. The conference may have moved, been renamed or ended; check `conferences.json`.",
            Code::E007 => "Ollama or the model was installed from a fallback source in the workflow. Update the pinned versions in `.github/workflows/scrape.yml`.",
            Code::E008 => "The primary search backend (Brave HTML) throttled or blocked the runner (HTTP 429/202/403); a fallback backend was used. Nothing to fix unless it happens every month; then raise `PLC_SEARCH_GAP` or reorder backends in `src/search.rs`.",
            Code::E009 => "The page's TLS certificate was invalid and was ignored. Check whether the conference site moved; the source URL is in the JSON next to the calendar.",
            Code::E010 => "The Ollama registry now serves a different build for the pinned model tag (manifest digest changed). Re-run the accuracy harness (`cargo test --test live -- --ignored`) and update `MODEL_DIGEST` in `.github/workflows/scrape.yml` if results are still good.",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attempt {
    pub last_attempt: DateTime<Utc>,
    pub attempts: u32,
    pub outcome: Outcome,
    #[serde(default)]
    pub consecutive_failures: u32,
    #[serde(default)]
    pub backend: Option<String>,
    #[serde(default)]
    pub codes: Vec<Code>,
    #[serde(default)]
    pub note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct State {
    pub last_run: Option<DateTime<Utc>>,
    pub run_count: u64,
    #[serde(default)]
    pub last_run_codes: Vec<Code>,
    /// Keyed by `CONF/TRACK/YEAR/cfp` or `CONF/TRACK/YEAR/volunteer`.
    #[serde(default)]
    pub runs: BTreeMap<String, Attempt>,
}

impl State {
    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let text = std::fs::read_to_string(path)?;
        serde_json::from_str(&text).with_context(|| format!("parsing {}", path.display()))
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        std::fs::write(path, serde_json::to_string_pretty(self)? + "\n")?;
        Ok(())
    }

    pub fn get(&self, key: &str) -> Option<&Attempt> {
        self.runs.get(key)
    }

    /// Record an attempt. Not-found streaks of three or more raise E006, but
    /// only once the edition is `overdue` (its year has started); a
    /// next-year call for papers that is not out yet is normal.
    pub fn record(&mut self, key: &str, outcome: Outcome, backend: Option<String>, mut codes: Vec<Code>, note: String, now: DateTime<Utc>, overdue: bool) {
        let prev = self.runs.get(key).cloned();
        let attempts = prev.as_ref().map(|a| a.attempts).unwrap_or(0) + 1;
        let failed = !matches!(outcome, Outcome::Ok | Outcome::NoVolunteerProgram | Outcome::Abandoned);
        let consecutive_failures = if failed { prev.as_ref().map(|a| a.consecutive_failures).unwrap_or(0) + 1 } else { 0 };
        if outcome == Outcome::NotFound && consecutive_failures >= 3 && overdue {
            codes.push(Code::E006);
        }
        codes.sort();
        codes.dedup();
        self.runs.insert(key.to_string(), Attempt { last_attempt: now, attempts, outcome, consecutive_failures, backend, codes, note });
    }

    /// Active codes with the keys they apply to, for MAINTENANCE.md.
    pub fn active_codes(&self) -> BTreeMap<Code, Vec<(String, DateTime<Utc>)>> {
        let mut m: BTreeMap<Code, Vec<(String, DateTime<Utc>)>> = BTreeMap::new();
        for (k, a) in &self.runs {
            for c in &a.codes {
                m.entry(*c).or_default().push((k.clone(), a.last_attempt));
            }
        }
        for c in &self.last_run_codes {
            m.entry(*c).or_default().push(("(workflow)".into(), self.last_run.unwrap_or_else(Utc::now)));
        }
        m
    }

    pub fn render_maintenance(&self) -> String {
        let mut s = String::from("# Maintenance status\n\n");
        s.push_str("This file is regenerated on every run from `state.json`. Each code marks a fallback path that worked but suggests the primary mechanism needs a look. Codes disappear once the primary path works again.\n\n");
        match self.last_run {
            Some(t) => s.push_str(&format!("Last run: {}\n\n", t.format("%Y-%m-%d %H:%M UTC"))),
            None => s.push_str("Last run: never\n\n"),
        }
        let active = self.active_codes();
        if active.is_empty() {
            s.push_str("No active maintenance codes.\n");
        } else {
            for (code, keys) in &active {
                s.push_str(&format!("## {code:?}\n\n{}\n\n", code.hint()));
                for (k, t) in keys {
                    s.push_str(&format!("- `{k}` (last seen {})\n", t.format("%Y-%m-%d")));
                }
                s.push('\n');
            }
        }
        s.push_str("## Recent outcomes\n\n| Conference-year | Outcome | Attempts | Last attempt | Note |\n|---|---|---|---|---|\n");
        for (k, a) in self.runs.iter().rev() {
            s.push_str(&format!("| `{k}` | {} | {} | {} | {} |\n", format!("{:?}", a.outcome).to_lowercase(), a.attempts, a.last_attempt.format("%Y-%m-%d"), a.note.replace('|', "/")));
        }
        s
    }

    /// One-line summary for the README.
    pub fn summary_line(&self) -> String {
        let active = self.active_codes();
        let last = self.last_run.map(|t| t.format("%Y-%m-%d").to_string()).unwrap_or_else(|| "never".into());
        if active.is_empty() {
            format!("Last run: {last}. No maintenance needed. See [MAINTENANCE.md](MAINTENANCE.md).")
        } else {
            let codes: Vec<String> = active.keys().map(|c| format!("{c:?}")).collect();
            format!("Last run: {last}. **Maintenance codes active: {}** - see [MAINTENANCE.md](MAINTENANCE.md).", codes.join(", "))
        }
    }
}

/// Replace the text between `<!-- name:start -->` and `<!-- name:end -->`.
pub fn replace_marked(text: &str, name: &str, body: &str) -> String {
    let start = format!("<!-- {name}:start -->");
    let end = format!("<!-- {name}:end -->");
    match (text.find(&start), text.find(&end)) {
        (Some(a), Some(b)) if a < b => format!("{}{start}\n{body}\n{}", &text[..a], &text[b..]),
        _ => format!("{text}\n{start}\n{body}\n{end}\n"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn not_found_streak_raises_e006() {
        let mut st = State::default();
        let now = Utc::now();
        for _ in 0..2 {
            st.record("X/X/2027/cfp", Outcome::NotFound, None, vec![], String::new(), now, true);
        }
        assert!(st.get("X/X/2027/cfp").unwrap().codes.is_empty());
        st.record("X/X/2027/cfp", Outcome::NotFound, None, vec![], String::new(), now, false);
        assert!(st.get("X/X/2027/cfp").unwrap().codes.is_empty(), "not overdue: no E006");
        st.record("X/X/2027/cfp", Outcome::NotFound, None, vec![], String::new(), now, true);
        assert_eq!(st.get("X/X/2027/cfp").unwrap().codes, vec![Code::E006]);
        st.record("X/X/2027/cfp", Outcome::Ok, Some("ddg-html".into()), vec![Code::E003], String::new(), now, true);
        let a = st.get("X/X/2027/cfp").unwrap();
        assert_eq!(a.consecutive_failures, 0);
        assert_eq!(a.codes, vec![Code::E003]);
        assert!(st.render_maintenance().contains("## E003"));
        assert!(st.summary_line().contains("E003"));
    }

    #[test]
    fn marked_replacement() {
        let t = "# Title\n<!-- x:start -->\nold\n<!-- x:end -->\ntail";
        assert_eq!(replace_marked(t, "x", "new"), "# Title\n<!-- x:start -->\nnew\n<!-- x:end -->\ntail");
        assert!(replace_marked("plain", "x", "new").contains("<!-- x:start -->\nnew\n<!-- x:end -->"));
    }
}
