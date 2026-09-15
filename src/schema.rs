//! What the model must produce, and the checks applied to it.

use chrono::NaiveDate;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

const DATE_PATTERN: &str = r"^[0-9]{4}-[0-9]{2}-[0-9]{2}$";

/// Lets the pipeline decide whether a corrective retry can help at all.
pub trait Extraction {
    /// The page is about the requested conference edition.
    fn is_about(&self) -> bool;
    /// The page claims to contain the data we want.
    fn claims_data(&self) -> bool;
}

impl Extraction for CfpExtraction {
    fn is_about(&self) -> bool {
        self.page_is_about_conference
    }
    fn claims_data(&self) -> bool {
        self.has_submission_deadline && !self.rounds.is_empty()
    }
}

impl Extraction for VolunteerExtraction {
    fn is_about(&self) -> bool {
        self.page_is_about_conference
    }
    fn claims_data(&self) -> bool {
        self.has_volunteer_program && self.application_deadline.is_some()
    }
}

/// Result of extracting a call for papers.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CfpExtraction {
    /// True if this page is about the requested conference edition (right name and year).
    pub page_is_about_conference: bool,
    /// True if the page states the paper submission deadline for the requested track.
    pub has_submission_deadline: bool,
    /// The conference itself: when and where it takes place.
    #[schemars(required)]
    pub conference: Option<ConferenceInfo>,
    /// One entry per deadline for submitting NEW papers to this track. Most conferences have exactly one. A multi-stage review process (reviews, author response, revision, final decision) is still one round, not several.
    pub rounds: Vec<Round>,
    /// A few sentences for authors: how and where to submit, page limit, review process, anonymisation, anything an author must know before submitting.
    pub submission_details: String,
    /// Full URL of the paper submission site (where authors upload their paper), if it is written on the page; null otherwise.
    #[schemars(required)]
    pub submission_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ConferenceInfo {
    /// First day of the conference, YYYY-MM-DD.
    #[schemars(regex(pattern = DATE_PATTERN))]
    pub start_date: String,
    /// Last day of the conference, YYYY-MM-DD.
    #[schemars(regex(pattern = DATE_PATTERN))]
    pub end_date: String,
    /// City where the conference takes place, if stated.
    #[schemars(required)]
    pub city: Option<String>,
    /// Country where the conference takes place, if stated.
    #[schemars(required)]
    pub country: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Round {
    /// Short label such as "Round 1" or "Round 2"; empty if there is only one round.
    pub label: String,
    /// Deadline for submitting new papers to this track (not abstracts, artifacts, revisions or camera-ready versions), YYYY-MM-DD.
    #[schemars(regex(pattern = DATE_PATTERN))]
    pub submission_deadline: String,
    /// The exact words on the page that state this submission deadline, copied verbatim (a short fragment).
    pub submission_deadline_quote: String,
    /// First day of the author response / rebuttal period, YYYY-MM-DD, if any.
    #[schemars(required)]
    #[schemars(regex(pattern = DATE_PATTERN))]
    pub author_response_start: Option<String>,
    /// Last day of the author response / rebuttal period, YYYY-MM-DD, if any.
    #[schemars(required)]
    #[schemars(regex(pattern = DATE_PATTERN))]
    pub author_response_end: Option<String>,
    /// Date authors are notified whether their paper is accepted (acceptance/rejection notification, not the camera-ready or revision deadline), YYYY-MM-DD, if stated.
    #[schemars(required)]
    #[schemars(regex(pattern = DATE_PATTERN))]
    pub notification: Option<String>,
    /// The exact words on the page that state the notification date, copied verbatim; null if not stated.
    #[schemars(required)]
    pub notification_quote: Option<String>,
}

/// Result of extracting a student-volunteer page.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct VolunteerExtraction {
    /// True if this page is about the requested conference edition (right name and year).
    pub page_is_about_conference: bool,
    /// True if the page explains how students can apply to be volunteers at this edition (not just a list of names).
    pub has_volunteer_program: bool,
    /// Deadline for applying as a student volunteer, YYYY-MM-DD, if stated.
    #[schemars(required)]
    #[schemars(regex(pattern = DATE_PATTERN))]
    pub application_deadline: Option<String>,
    /// The exact words on the page that state the application deadline, copied verbatim; null if no deadline is stated.
    #[schemars(required)]
    pub application_deadline_quote: Option<String>,
    /// A few sentences on who can apply, what volunteers get, and how to apply.
    #[schemars(required)]
    pub how_to_apply: Option<String>,
    /// Full URL of the application form or sign-up page for volunteers, if it is written on the page; null otherwise.
    #[schemars(required)]
    pub application_url: Option<String>,
}

/// Which search hit (0-based index into the list shown) is the best match, or null if none fits.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Choice {
    /// 0-based index of the chosen entry, or null if none is suitable.
    #[schemars(required)]
    pub index: Option<u32>,
}

/// Plausible official URLs, used when no search engine answers.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct UrlGuesses {
    /// Up to five full URLs (with scheme) most likely to be the official website of the conference edition.
    pub urls: Vec<String>,
}

/// Whitespace-insensitive, case-insensitive containment check used to make
/// sure a quoted fragment really is on the page.
pub fn page_contains(page: &str, quote: &str) -> bool {
    let norm = |t: &str| t.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase();
    let q = norm(quote);
    if q.len() < 4 {
        return false;
    }
    let p = norm(page);
    if p.contains(&q) {
        return true;
    }
    // Tolerate small copying slips (a stripped URL, a fixed typo): most of
    // the longer words must appear.
    let words: Vec<&str> = q.split(' ').filter(|w| w.len() >= 4).collect();
    let found = words.iter().filter(|w| p.contains(*w)).count();
    !words.is_empty() && found * 4 >= words.len() * 3
}

pub fn parse_date(s: &str) -> Result<NaiveDate, String> {
    NaiveDate::parse_from_str(s.trim(), "%Y-%m-%d").map_err(|_| format!("'{s}' is not a valid YYYY-MM-DD date"))
}

fn parse_opt(s: &Option<String>, what: &str, errs: &mut Vec<String>) -> Option<NaiveDate> {
    match s.as_deref().map(str::trim).filter(|s| !s.is_empty() && *s != "null") {
        None => None,
        Some(s) => match parse_date(s) {
            Ok(d) => Some(d),
            Err(e) => {
                errs.push(format!("{what}: {e}"));
                None
            }
        },
    }
}

/// Lifecycle of a conference-year, derived from stored data and today.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    /// Nothing known yet.
    Future,
    /// Conference dates/location known, no paper deadlines yet.
    ConferenceAvailable,
    /// Deadlines known and still ahead (or rebuttal still running).
    DeadlinesAvailable,
    /// The last round's rebuttal (or notification, or submission) has passed: deadlines are final.
    PostRebuttal,
    /// The conference is over.
    Happened,
}

impl Stage {
    /// Active stages are re-collected on every run; the others are archived.
    pub fn active(self) -> bool {
        matches!(self, Stage::Future | Stage::ConferenceAvailable | Stage::DeadlinesAvailable)
    }
    pub fn label(self) -> &'static str {
        match self {
            Stage::Future => "future",
            Stage::ConferenceAvailable => "conference available",
            Stage::DeadlinesAvailable => "deadlines available",
            Stage::PostRebuttal => "post-rebuttal",
            Stage::Happened => "happened",
        }
    }
}

/// The deadlines part of a call for papers, stored as `cfp.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Deadlines {
    pub rounds: Vec<ValidRound>,
    #[serde(default)]
    pub submission_details: String,
    #[serde(default)]
    pub submission_url: Option<String>,
}

impl Cfp {
    /// The deadlines part, if the page had any.
    pub fn deadlines(&self) -> Option<Deadlines> {
        (!self.rounds.is_empty()).then(|| Deadlines { rounds: self.rounds.clone(), submission_details: self.submission_details.clone(), submission_url: self.submission_url.clone() })
    }
}

/// Date after which the stored deadlines can no longer change.
pub fn deadlines_final_after(d: &Deadlines) -> Option<NaiveDate> {
    let last = d.rounds.last()?;
    Some(last.response_end.or(last.response_start).or(last.notification).unwrap_or(last.submission))
}

/// Conference dates keep being (re)collected until the conference is over.
pub fn conference_active(c: Option<&Conference>, today: NaiveDate) -> bool {
    c.is_none_or(|c| today <= c.end)
}

/// Deadlines keep being (re)collected until the last rebuttal has ended.
pub fn deadlines_active(d: Option<&Deadlines>, today: NaiveDate) -> bool {
    d.is_none_or(|d| deadlines_final_after(d).is_none_or(|cut| today <= cut))
}

pub fn stage(conference: Option<&Conference>, deadlines: Option<&Deadlines>, today: NaiveDate) -> Stage {
    if conference.is_some_and(|k| today > k.end) {
        return Stage::Happened;
    }
    match deadlines {
        Some(d) => match deadlines_final_after(d) {
            Some(cutoff) if today > cutoff => Stage::PostRebuttal,
            _ => Stage::DeadlinesAvailable,
        },
        None if conference.is_some() => Stage::ConferenceAvailable,
        None => Stage::Future,
    }
}

/// A recorded change of a stored value between two runs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Change {
    pub at: chrono::DateTime<chrono::Utc>,
    /// e.g. "round 1 submission", "conference start", "volunteer deadline".
    pub field: String,
    pub old: String,
    pub new: String,
}

fn fmt_opt(d: Option<NaiveDate>) -> String {
    d.map(|d| d.to_string()).unwrap_or_else(|| "none".into())
}

/// Differences in dates and location (prose is ignored) between two records.
pub fn diff_cfp(old: &Cfp, new: &Cfp, at: chrono::DateTime<chrono::Utc>) -> Vec<Change> {
    let mut out = match (old.deadlines(), new.deadlines()) {
        (Some(o), Some(n)) => diff_deadlines(&o, &n, at),
        _ => vec![],
    };
    out.extend(diff_conference(old.conference.as_ref(), new.conference.as_ref(), at));
    out
}

fn conference_text(c: Option<&Conference>) -> String {
    match c {
        Some(c) => format!("{}..{} {}", c.start, c.end, [c.city.clone(), c.country.clone()].into_iter().flatten().collect::<Vec<_>>().join(", ")),
        None => "none".into(),
    }
}

/// Change in conference dates or location.
pub fn diff_conference(old: Option<&Conference>, new: Option<&Conference>, at: chrono::DateTime<chrono::Utc>) -> Vec<Change> {
    let (o, n) = (conference_text(old), conference_text(new));
    if o == n { vec![] } else { vec![Change { at, field: "conference".into(), old: o, new: n }] }
}

/// Changes in deadline dates (prose and links are ignored).
pub fn diff_deadlines(old: &Deadlines, new: &Deadlines, at: chrono::DateTime<chrono::Utc>) -> Vec<Change> {
    let mut out = vec![];
    let mut push = |field: String, o: String, n: String| {
        if o != n {
            out.push(Change { at, field, old: o, new: n });
        }
    };
    if old.rounds.len() != new.rounds.len() {
        push("rounds".into(), format!("{} round(s)", old.rounds.len()), format!("{} round(s)", new.rounds.len()));
    }
    for (i, (o, n)) in old.rounds.iter().zip(&new.rounds).enumerate() {
        let r = i + 1;
        push(format!("round {r} submission"), o.submission.to_string(), n.submission.to_string());
        push(format!("round {r} response"), format!("{}..{}", fmt_opt(o.response_start), fmt_opt(o.response_end)), format!("{}..{}", fmt_opt(n.response_start), fmt_opt(n.response_end)));
        push(format!("round {r} notification"), fmt_opt(o.notification), fmt_opt(n.notification));
    }
    out
}

/// Validated, typed version of [`CfpExtraction`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cfp {
    #[serde(default)]
    pub conference: Option<Conference>,
    #[serde(default)]
    pub rounds: Vec<ValidRound>,
    #[serde(default)]
    pub submission_details: String,
    /// Paper submission site, when found on the page or among its links.
    #[serde(default)]
    pub submission_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conference {
    pub start: NaiveDate,
    pub end: NaiveDate,
    #[serde(default)]
    pub city: Option<String>,
    #[serde(default)]
    pub country: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidRound {
    #[serde(default)]
    pub label: String,
    pub submission: NaiveDate,
    #[serde(default)]
    pub response_start: Option<NaiveDate>,
    #[serde(default)]
    pub response_end: Option<NaiveDate>,
    #[serde(default)]
    pub notification: Option<NaiveDate>,
}

fn clean_opt(s: &Option<String>) -> Option<String> {
    s.as_deref().map(str::trim).filter(|s| !s.is_empty() && !s.eq_ignore_ascii_case("null") && !s.eq_ignore_ascii_case("unknown")).map(String::from)
}

/// A URL the model reported is kept only if it is well-formed and appears in
/// `grounding` (the page text plus its link targets).
pub fn grounded_url(u: &Option<String>, grounding: &str) -> Option<String> {
    let u = clean_opt(u)?;
    let parsed = url::Url::parse(&u).ok()?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return None;
    }
    let key = u.trim_end_matches('/').to_lowercase();
    grounding.to_lowercase().contains(&key).then_some(u)
}

/// Check dates parse and are consistent with each other and with `year`.
pub fn validate_cfp(x: &CfpExtraction, year: i32, page: &str) -> Result<Cfp, Vec<String>> {
    let mut errs = vec![];
    if !x.page_is_about_conference {
        errs.push("page_is_about_conference is false".into());
    }
    // Either deadlines or the conference dates must be there; a page with
    // only dates and location gives a "conference available" record.
    let has_rounds = x.has_submission_deadline && !x.rounds.is_empty();
    if !has_rounds && x.conference.is_none() {
        errs.push("neither a submission deadline nor conference dates found".into());
    }
    let conference = x.conference.as_ref().and_then(|c| {
        let start = parse_date(&c.start_date).map_err(|e| errs.push(format!("conference start_date: {e}"))).ok()?;
        let end = parse_date(&c.end_date).map_err(|e| errs.push(format!("conference end_date: {e}"))).ok()?;
        if start > end {
            errs.push("conference start_date is after end_date".into());
        }
        if start.year() != year && end.year() != year {
            errs.push(format!("conference dates are not in {year}"));
        }
        Some(Conference { start, end, city: clean_opt(&c.city), country: clean_opt(&c.country) })
    });
    let mut rounds = vec![];
    for (i, r) in x.rounds.iter().enumerate().take(if has_rounds { usize::MAX } else { 0 }) {
        let what = format!("round {}", i + 1);
        let Ok(submission) = parse_date(&r.submission_deadline).map_err(|e| errs.push(format!("{what} submission_deadline: {e}"))) else {
            continue;
        };
        // Grounding: the quote must be on the page. (Requiring the date
        // itself inside the quote was tried and rejected: in tables the date
        // sits in another cell, so the model rightly quotes the label.)
        if !page.is_empty() && !page_contains(page, &r.submission_deadline_quote) {
            errs.push(format!("{what} submission_deadline_quote {:?} does not appear on the page; quote the page verbatim", r.submission_deadline_quote));
        }
        if submission.year() < year - 1 || submission.year() > year {
            errs.push(format!("{what} submission deadline {submission} is not in {} or {year}", year - 1));
        }
        let response_start = parse_opt(&r.author_response_start, &format!("{what} author_response_start"), &mut errs);
        let response_end = parse_opt(&r.author_response_end, &format!("{what} author_response_end"), &mut errs);
        let notification = parse_opt(&r.notification, &format!("{what} notification"), &mut errs);
        if notification.is_some() && !page.is_empty() {
            match clean_opt(&r.notification_quote) {
                Some(q) if page_contains(page, &q) => {}
                Some(q) => errs.push(format!("{what} notification_quote {q:?} does not appear on the page; quote the page verbatim")),
                None => errs.push(format!("{what} notification is given but notification_quote is null; quote the page or set notification to null")),
            }
        }
        let mut last = submission;
        for (name, d) in [("author_response_start", response_start), ("author_response_end", response_end), ("notification", notification)] {
            if let Some(d) = d {
                if d < last {
                    errs.push(format!("{what} {name} {d} is before an earlier date {last}"));
                }
                last = d;
            }
        }
        if let Some(c) = &conference {
            if last > c.start {
                errs.push(format!("{what} dates run past the conference start {}", c.start));
            }
        }
        rounds.push(ValidRound { label: r.label.trim().to_string(), submission, response_start, response_end, notification });
    }
    for w in rounds.windows(2) {
        if w[1].submission <= w[0].submission {
            errs.push("rounds are not in chronological order".into());
        }
    }
    if errs.is_empty() {
        Ok(Cfp { conference, rounds, submission_details: x.submission_details.trim().to_string(), submission_url: grounded_url(&x.submission_url, page) })
    } else {
        Err(errs)
    }
}

/// Validated volunteer result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Volunteer {
    pub deadline: NaiveDate,
    #[serde(default)]
    pub how_to_apply: String,
    /// Application form or sign-up page, when found on the page or among its links.
    #[serde(default)]
    pub application_url: Option<String>,
}

/// `conference_end`, when known, bounds the application deadline: volunteers
/// are recruited before the conference, never after.
pub fn validate_volunteer(x: &VolunteerExtraction, year: i32, page: &str, conference_end: Option<NaiveDate>) -> Result<Option<Volunteer>, Vec<String>> {
    let mut errs = vec![];
    if !x.page_is_about_conference {
        errs.push("page_is_about_conference is false".into());
    }
    if !x.has_volunteer_program {
        return if errs.is_empty() { Ok(None) } else { Err(errs) };
    }
    let Some(deadline) = parse_opt(&x.application_deadline, "application_deadline", &mut errs) else {
        return if errs.is_empty() { Ok(None) } else { Err(errs) };
    };
    if deadline.year() < year - 1 || deadline.year() > year {
        errs.push(format!("application deadline {deadline} is not in {} or {year}", year - 1));
    }
    if let Some(end) = conference_end {
        if deadline > end {
            errs.push(format!("application deadline {deadline} is after the conference ends ({end}); volunteers are recruited before the conference"));
        }
    }
    match clean_opt(&x.application_deadline_quote) {
        Some(q) if page.is_empty() || page_contains(page, &q) => {}
        Some(q) => errs.push(format!("application_deadline_quote {q:?} does not appear on the page; quote the page verbatim")),
        None => errs.push("application_deadline is given but application_deadline_quote is null; quote the page or set the deadline to null".into()),
    }
    if errs.is_empty() {
        Ok(Some(Volunteer { deadline, how_to_apply: clean_opt(&x.how_to_apply).unwrap_or_default(), application_url: grounded_url(&x.application_url, page) }))
    } else {
        Err(errs)
    }
}

use chrono::Datelike;

#[cfg(test)]
mod tests {
    use super::*;

    fn round(sub: &str, rs: Option<&str>, re: Option<&str>, notif: Option<&str>) -> Round {
        Round {
            label: String::new(),
            submission_deadline: sub.into(),
            submission_deadline_quote: "Thu 10 Jul 2025".into(),
            notification_quote: notif.map(|_| "Final acceptance notification Thu 6 Nov 2025".to_string()),
            author_response_start: rs.map(String::from),
            author_response_end: re.map(String::from),
            notification: notif.map(String::from),
        }
    }

    fn ok_extraction() -> CfpExtraction {
        CfpExtraction {
            page_is_about_conference: true,
            has_submission_deadline: true,
            conference: Some(ConferenceInfo { start_date: "2026-01-11".into(), end_date: "2026-01-17".into(), city: Some("Rennes".into()), country: Some("France".into()) }),
            rounds: vec![round("2025-07-10", Some("2025-09-08"), Some("2025-09-11"), Some("2025-11-06"))],
            submission_details: "Submit via HotCRP.".into(),
            submission_url: Some("https://popl26.hotcrp.com".into()),
        }
    }

    #[test]
    fn urls_are_kept_only_when_on_the_page() {
        let page = "Submission deadline: Thu 10 Jul 2025. Final acceptance notification Thu 6 Nov 2025. Submit at https://popl26.hotcrp.com/";
        let c = validate_cfp(&ok_extraction(), 2026, page).unwrap();
        assert_eq!(c.submission_url.as_deref(), Some("https://popl26.hotcrp.com"));
        let c = validate_cfp(&ok_extraction(), 2026, "Submission deadline: Thu 10 Jul 2025. Final acceptance notification Thu 6 Nov 2025").unwrap();
        assert_eq!(c.submission_url, None, "a URL that is not on the page is dropped");
        assert_eq!(grounded_url(&Some("javascript:void(0)".into()), "javascript:void(0)"), None);
    }

    #[test]
    fn accepts_consistent_dates() {
        let c = validate_cfp(&ok_extraction(), 2026, "Submission deadline: Thu 10 Jul 2025. Final acceptance notification Thu 6 Nov 2025").unwrap();
        assert_eq!(c.rounds[0].submission, NaiveDate::from_ymd_opt(2025, 7, 10).unwrap());
        assert_eq!(c.conference.unwrap().city.as_deref(), Some("Rennes"));
    }

    #[test]
    fn rejects_bad_ordering_and_dates() {
        let mut x = ok_extraction();
        x.rounds[0].notification = Some("2025-08-01".into());
        assert!(validate_cfp(&x, 2026, "").unwrap_err().iter().any(|e| e.contains("before an earlier date")));
        let mut x = ok_extraction();
        x.rounds[0].submission_deadline = "2025-02-31".into();
        assert!(validate_cfp(&x, 2026, "").is_err());
        let mut x = ok_extraction();
        x.conference.as_mut().unwrap().start_date = "2027-01-11".into();
        x.conference.as_mut().unwrap().end_date = "2027-01-17".into();
        assert!(validate_cfp(&x, 2026, "").unwrap_err().iter().any(|e| e.contains("not in 2026")));
        let x = ok_extraction();
        assert!(validate_cfp(&x, 2026, "a page without that date").unwrap_err().iter().any(|e| e.contains("does not appear")));
    }

    #[test]
    fn grounding_is_whitespace_and_case_insensitive() {
        assert!(page_contains("Deadline:   Thu 10\n Jul 2025", "thu 10 jul 2025"));
        assert!(page_contains("Submissions due October 10, 2025 (AoE)", "October 10 2025"));
        assert!(!page_contains("nothing here", "Thu 10 Jul 2025"));
    }

    #[test]
    fn stages_follow_the_calendar() {
        let d = |s: &str| NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap();
        let c = validate_cfp(&ok_extraction(), 2026, "Submission deadline: Thu 10 Jul 2025. Final acceptance notification Thu 6 Nov 2025").unwrap();
        let (conf, dl) = (c.conference.as_ref(), c.deadlines());
        assert_eq!(stage(None, None, d("2025-01-01")), Stage::Future);
        assert_eq!(stage(conf, dl.as_ref(), d("2025-08-01")), Stage::DeadlinesAvailable);
        assert_eq!(stage(conf, dl.as_ref(), d("2025-09-11")), Stage::DeadlinesAvailable);
        assert_eq!(stage(conf, dl.as_ref(), d("2025-09-12")), Stage::PostRebuttal);
        assert_eq!(stage(conf, dl.as_ref(), d("2026-01-18")), Stage::Happened);
        assert_eq!(stage(conf, None, d("2025-08-01")), Stage::ConferenceAvailable);
        assert_eq!(stage(None, dl.as_ref(), d("2025-08-01")), Stage::DeadlinesAvailable);
        assert!(Stage::ConferenceAvailable.active() && !Stage::PostRebuttal.active());
        assert!(deadlines_active(dl.as_ref(), d("2025-09-11")) && !deadlines_active(dl.as_ref(), d("2025-09-12")));
        assert!(conference_active(conf, d("2026-01-17")) && !conference_active(conf, d("2026-01-18")));
        assert!(conference_active(None, d("2030-01-01")) && deadlines_active(None, d("2030-01-01")));
    }

    #[test]
    fn conference_only_page_is_valid() {
        let mut x = ok_extraction();
        x.has_submission_deadline = false;
        x.rounds.clear();
        let c = validate_cfp(&x, 2026, "").unwrap();
        assert!(c.rounds.is_empty() && c.conference.is_some());
        x.conference = None;
        assert!(validate_cfp(&x, 2026, "").is_err());
    }

    #[test]
    fn diff_reports_changed_dates_only() {
        let page = "Submission deadline: Thu 10 Jul 2025. Final acceptance notification Thu 6 Nov 2025";
        let a = validate_cfp(&ok_extraction(), 2026, page).unwrap();
        let mut x = ok_extraction();
        x.rounds[0].submission_deadline = "2025-07-17".into();
        x.submission_details = "different prose".into();
        let b = validate_cfp(&x, 2026, page).unwrap();
        let d = diff_cfp(&a, &b, chrono::Utc::now());
        assert_eq!(d.len(), 1);
        assert_eq!((d[0].field.as_str(), d[0].old.as_str(), d[0].new.as_str()), ("round 1 submission", "2025-07-10", "2025-07-17"));
        assert!(diff_cfp(&a, &a, chrono::Utc::now()).is_empty());
    }

    #[test]
    fn schema_has_date_pattern() {
        let s = serde_json::to_value(schemars::schema_for!(CfpExtraction)).unwrap();
        let text = s.to_string();
        assert!(text.contains("^[0-9]{4}-[0-9]{2}-[0-9]{2}$"), "{text}");
    }
}
