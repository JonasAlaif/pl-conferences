//! iCalendar output. Hand-written: the format needed here is tiny.

use crate::schema::{Cfp, Change, Conference, Deadlines, Volunteer};
use chrono::{DateTime, Datelike, NaiveDate, Utc};
use sha1::{Digest, Sha1};

pub const REPO_URL: &str = "https://github.com/JonasAlaif/pl-conferences";
pub const MAINTENANCE_URL: &str = "https://github.com/JonasAlaif/pl-conferences/blob/main/MAINTENANCE.md";

#[derive(Debug, Clone)]
pub struct Event {
    pub uid: String,
    pub summary: String,
    pub description: String,
    pub location: String,
    pub start: NaiveDate,
    /// Inclusive last day.
    pub end: NaiveDate,
    pub stamp: DateTime<Utc>,
}

/// Provenance in descriptions: the disclaimer with the source first, the
/// recorded changes and maintenance codes last.
#[derive(Debug, Clone, Default)]
pub struct Provenance {
    pub source_url: String,
    pub codes: Vec<String>,
    /// Recorded changes; entries whose `field` matches an event are listed in it.
    pub history: Vec<Change>,
}

impl Provenance {
    fn disclaimer(&self) -> String {
        format!("Extracted automatically by a small language model; please cross-check against the source before relying on it: {}", self.source_url)
    }

    fn tail(&self, field: &str) -> String {
        let mut s = String::new();
        for c in self.history.iter().filter(|c| c.field == field || c.field == "rounds") {
            s.push_str(&format!("Changed {}: was {} (now {}).\n", c.at.format("%Y-%m-%d"), c.old, c.new));
        }
        if !self.codes.is_empty() {
            s.push_str(&format!("Maintenance: {} - see {MAINTENANCE_URL}\n", self.codes.join(", ")));
        }
        s.push_str(REPO_URL);
        s
    }

    /// A description: disclaimer, then `middle` (details, links, notes),
    /// then changes, codes and the repository.
    fn describe(&self, field: &str, middle: Vec<String>) -> String {
        let mut parts = vec![self.disclaimer()];
        parts.extend(middle);
        parts.push(self.tail(field));
        parts.join("\n\n")
    }
}

fn uid(key: &str, event: &str) -> String {
    let mut h = Sha1::new();
    h.update(format!("{key}/{event}").as_bytes());
    let hex: String = h.finalize().iter().map(|b| format!("{b:02x}")).collect();
    format!("{hex}@pl-conferences")
}

/// The parts of the key "SPLASH/OOPSLA/2027": conference, track, year.
fn split_key(key: &str) -> (&str, &str, &str) {
    let mut parts = key.split('/');
    let conf = parts.next().unwrap_or(key);
    (conf, parts.next().unwrap_or(conf), parts.next().unwrap_or(""))
}

fn tag(name: &str, year: &str) -> String {
    format!("{name} {}", year.get(2..).unwrap_or(year))
}

/// The tags events are filed under, from the key "SPLASH/OOPSLA/2027":
/// "SPLASH 27" for the conference and its volunteers, "OOPSLA 27" for the
/// deadlines, since the track is what one submits to.
fn tags(key: &str) -> (String, String) {
    let (conf, track, year) = split_key(key);
    (tag(conf, year), tag(track, year))
}

/// The title of a conference event or a volunteer deadline names only the
/// conference; when the tracks followed have names of their own, this says
/// where their deadlines are filed. `tracks` is empty when unknown.
fn tracks_line(key: &str, tracks: &[String]) -> Option<String> {
    let (conf, own, year) = split_key(key);
    let tracks: Vec<&String> = tracks.iter().filter(|t| !t.eq_ignore_ascii_case(conf)).collect();
    if tracks.is_empty() {
        return (!own.eq_ignore_ascii_case(conf)).then(|| format!("The deadlines of its {own} track are filed as [{}].", tag(own, year)));
    }
    let names = tracks.iter().map(|t| t.as_str()).collect::<Vec<_>>().join(" and ");
    let tagged = tracks.iter().map(|t| format!("[{}]", tag(t, year))).collect::<Vec<_>>().join(" and ");
    Some(format!("The deadlines of its {names} {} are filed as {tagged}.", if tracks.len() > 1 { "tracks" } else { "track" }))
}

/// The other way round, for a deadline filed under a track name.
fn track_of_line(key: &str) -> Option<String> {
    let (conf, track, year) = split_key(key);
    (!track.eq_ignore_ascii_case(conf)).then(|| format!("{track} is a track of {conf} {year}."))
}

/// `uid_name` is the event's name when it was first published; UIDs must
/// not change with a rename, or subscribers see every event twice.
#[allow(clippy::too_many_arguments)]
fn event(tag: &str, key: &str, stamp: DateTime<Utc>, uid_name: &str, name: &str, desc: String, loc: &str, start: NaiveDate, end: NaiveDate) -> Event {
    Event { uid: uid(key, uid_name), summary: format!("[{tag}] {name}"), description: desc, location: loc.to_string(), start, end, stamp }
}

/// The single conference event (dates and location). `tracks` are the
/// conference's tracks from `conferences.json`, for the description.
pub fn conference_events(key: &str, tracks: &[String], c: &Conference, prov: &Provenance, stamp: DateTime<Utc>) -> Vec<Event> {
    let loc = match (&c.city, &c.country) {
        (Some(ci), Some(co)) => format!("{ci}, {co}"),
        (Some(x), None) | (None, Some(x)) => x.clone(),
        (None, None) => String::new(),
    };
    vec![event(&tags(key).0, key, stamp, "Conference", "Conference", prov.describe("conference", tracks_line(key, tracks).into_iter().collect()), &loc, c.start, c.end)]
}

/// Deadline events: submission, rebuttal and notification per round.
pub fn deadline_events(key: &str, d: &Deadlines, prov: &Provenance, stamp: DateTime<Utc>) -> Vec<Event> {
    let mut events = vec![];
    let two = d.rounds.len() > 1;
    let tag = tags(key).1;
    let mk = |uid_name: &str, name: &str, desc: String, start: NaiveDate, end: NaiveDate| event(&tag, key, stamp, uid_name, name, desc, "", start, end);
    for (i, r) in d.rounds.iter().enumerate() {
        let prefix = if two { format!("R{} ", i + 1) } else { String::new() };
        let n = i + 1;
        let mut parts: Vec<String> = track_of_line(key).into_iter().collect();
        if !d.submission_details.is_empty() {
            parts.push(d.submission_details.clone());
        }
        if let Some(u) = &d.submission_url {
            parts.push(format!("Submit at: {u}"));
        }
        if let Some(c) = &r.submission_conflict {
            parts.push(format!("Note: the page also states \"{c}\"."));
        }
        let desc = prov.describe(&format!("round {n} submission"), parts);
        events.push(mk(&format!("{prefix}Paper Submission Deadline"), &format!("{prefix}Submission Deadline"), desc, r.submission, r.submission));
        if let (Some(s), Some(e)) = (r.response_start, r.response_end.or(r.response_start)) {
            let name = format!("{prefix}Rebuttal");
            events.push(mk(&name, &name, prov.describe(&format!("round {n} response"), track_of_line(key).into_iter().collect()), s, e));
        }
        if let Some(nd) = r.notification {
            let name = format!("{prefix}Notification");
            events.push(mk(&name, &name, prov.describe(&format!("round {n} notification"), track_of_line(key).into_iter().collect()), nd, nd));
        }
    }
    events
}

/// Both parts of a call for papers, for tests and the aggregate view.
pub fn cfp_events(key: &str, cfp: &Cfp, prov: &Provenance, stamp: DateTime<Utc>) -> Vec<Event> {
    let mut events = vec![];
    if let Some(d) = cfp.deadlines() {
        events.extend(deadline_events(key, &d, prov, stamp));
    }
    if let Some(c) = &cfp.conference {
        events.extend(conference_events(key, &[], c, prov, stamp));
    }
    events
}

pub fn volunteer_events(key: &str, tracks: &[String], v: &Volunteer, prov: &Provenance, stamp: DateTime<Utc>) -> Vec<Event> {
    let mut parts: Vec<String> = tracks_line(key, tracks).into_iter().collect();
    if !v.how_to_apply.is_empty() {
        parts.push(v.how_to_apply.clone());
    }
    if let Some(u) = &v.application_url {
        parts.push(format!("Apply at: {u}"));
    }
    if let Some(c) = &v.deadline_conflict {
        parts.push(format!("Note: the page also states \"{c}\"."));
    }
    let name = "Volunteer Application Deadline";
    vec![event(&tags(key).0, key, stamp, name, name, prov.describe("volunteer deadline", parts), "", v.deadline, v.deadline)]
}

fn escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace(';', "\\;").replace(',', "\\,").replace("\r\n", "\\n").replace('\n', "\\n")
}

/// RFC 5545 line folding at 75 octets.
fn fold(line: &str) -> String {
    let mut out = String::new();
    let mut count = 0;
    for ch in line.chars() {
        let w = ch.len_utf8();
        if count + w > 74 {
            out.push_str("\r\n ");
            count = 1;
        }
        out.push(ch);
        count += w;
    }
    out
}

fn ymd(d: NaiveDate) -> String {
    format!("{:04}{:02}{:02}", d.year(), d.month(), d.day())
}

pub fn calendar(name: &str, events: &[Event]) -> String {
    let mut s = String::new();
    let mut push = |l: &str| {
        s.push_str(&fold(l));
        s.push_str("\r\n");
    };
    push("BEGIN:VCALENDAR");
    push("VERSION:2.0");
    push("PRODID:-//pl-conferences//EN");
    push("CALSCALE:GREGORIAN");
    push("METHOD:PUBLISH");
    push(&format!("X-WR-CALNAME:{}", escape(name)));
    push(&format!("URL:{REPO_URL}"));
    let mut sorted: Vec<&Event> = events.iter().collect();
    sorted.sort_by_key(|e| (e.start, e.summary.clone()));
    for e in sorted {
        push("BEGIN:VEVENT");
        push(&format!("UID:{}", e.uid));
        push(&format!("DTSTAMP:{}", e.stamp.format("%Y%m%dT%H%M%SZ")));
        push(&format!("DTSTART;VALUE=DATE:{}", ymd(e.start)));
        push(&format!("DTEND;VALUE=DATE:{}", ymd(e.end.succ_opt().unwrap_or(e.end))));
        push(&format!("SUMMARY:{}", escape(&e.summary)));
        if !e.description.is_empty() {
            push(&format!("DESCRIPTION:{}", escape(&e.description)));
        }
        if !e.location.is_empty() {
            push(&format!("LOCATION:{}", escape(&e.location)));
        }
        push("END:VEVENT");
    }
    push("END:VCALENDAR");
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::{Conference, ValidRound};

    #[test]
    fn renders_expected_events() {
        let cfp = Cfp {
            conference: Some(Conference {
                start: NaiveDate::from_ymd_opt(2026, 1, 11).unwrap(),
                end: NaiveDate::from_ymd_opt(2026, 1, 17).unwrap(),
                city: Some("Rennes".into()),
                country: Some("France".into()),
            }),
            rounds: vec![ValidRound {
                label: String::new(),
                submission: NaiveDate::from_ymd_opt(2025, 7, 10).unwrap(),
                submission_conflict: Some("Apply here by July 3".into()),
                response_start: NaiveDate::from_ymd_opt(2025, 9, 8),
                response_end: NaiveDate::from_ymd_opt(2025, 9, 11),
                notification: NaiveDate::from_ymd_opt(2025, 11, 6),
            }],
            submission_details: "Submit via HotCRP; 25 pages, double-blind.".into(),
            submission_url: Some("https://popl26.hotcrp.com".into()),
        };
        let prov = Provenance {
            source_url: "https://popl26.sigplan.org/dates".into(),
            codes: vec!["E003".into()],
            history: vec![Change { at: DateTime::parse_from_rfc3339("2026-09-20T00:00:00Z").unwrap().with_timezone(&Utc), field: "round 1 submission".into(), old: "2025-07-03".into(), new: "2025-07-10".into() }],
        };
        let stamp = DateTime::parse_from_rfc3339("2026-09-15T00:00:00Z").unwrap().with_timezone(&Utc);
        let ev = cfp_events("POPL/POPL/2026", &cfp, &prov, stamp);
        let ics = calendar("POPL 2026", &ev);
        let unfolded = ics.replace("\r\n ", "");
        assert!(ics.contains("SUMMARY:[POPL 26] Submission Deadline"));
        assert!(ics.contains("DTSTART;VALUE=DATE:20250710\r\nDTEND;VALUE=DATE:20250711"));
        assert!(ics.contains("SUMMARY:[POPL 26] Rebuttal"));
        assert!(ics.contains("SUMMARY:[POPL 26] Conference"));
        // The disclaimer with the source opens every description; the
        // details, links and notes follow; changes and codes close it.
        for e in &ev {
            assert!(e.description.starts_with("Extracted automatically by a small language model; please cross-check against the source before relying on it: https://popl26.sigplan.org/dates"), "{}", e.description);
        }
        let sub = &ev[0].description;
        let pos = |s: &str| sub.find(s).unwrap_or_else(|| panic!("{s} missing from {sub}"));
        assert!(pos("cross-check") < pos("Submit via HotCRP") && pos("Submit via HotCRP") < pos("Submit at:") && pos("Submit at:") < pos("Note: the page") && pos("Note: the page") < pos("Changed 2026-09-20") && pos("Changed 2026-09-20") < pos("Maintenance: E003") && pos("Maintenance: E003") < pos(REPO_URL));
        // Tags: the track for deadlines, the conference for its dates and
        // volunteers; when the two differ, the description says so.
        let splash = cfp_events("SPLASH/OOPSLA/2027", &cfp, &prov, stamp);
        assert_eq!(splash[0].summary, "[OOPSLA 27] Submission Deadline");
        assert!(splash[0].description.contains("\n\nOOPSLA is a track of SPLASH 2027.\n\n") && splash[1].description.contains("OOPSLA is a track of SPLASH 2027."), "{}", splash[0].description);
        assert_eq!(splash[3].summary, "[SPLASH 27] Conference");
        assert!(splash[3].description.contains("The deadlines of its OOPSLA track are filed as [OOPSLA 27]."), "{}", splash[3].description);
        assert!(!ev[0].description.contains("is a track of") && !ev[3].description.contains("filed as"), "nothing to explain when the names agree");
        let v = Volunteer { deadline: NaiveDate::from_ymd_opt(2027, 7, 1).unwrap(), deadline_conflict: None, how_to_apply: String::new(), application_url: None };
        let vol = volunteer_events("SPLASH/OOPSLA/2027", &["OOPSLA".to_string()], &v, &prov, stamp);
        assert_eq!(vol[0].summary, "[SPLASH 27] Volunteer Application Deadline");
        assert!(vol[0].description.contains("The deadlines of its OOPSLA track are filed as [OOPSLA 27]."));
        let etaps = conference_events("ETAPS/ESOP/2027", &["ESOP".to_string(), "TACAS".to_string()], cfp.conference.as_ref().unwrap(), &prov, stamp);
        assert_eq!(etaps[0].summary, "[ETAPS 27] Conference");
        assert!(etaps[0].description.contains("The deadlines of its ESOP and TACAS tracks are filed as [ESOP 27] and [TACAS 27]."), "{}", etaps[0].description);
        assert!(!conference_events("POPL/POPL/2027", &["POPL".to_string()], cfp.conference.as_ref().unwrap(), &prov, stamp)[0].description.contains("filed as"));
        // Renaming did not change the UIDs.
        assert_eq!(ev[0].uid, uid("POPL/POPL/2026", "Paper Submission Deadline"));
        assert_eq!(ev[3].uid, uid("POPL/POPL/2026", "Conference"));
        assert!(ics.contains("DTEND;VALUE=DATE:20250912"));
        assert!(ics.contains("LOCATION:Rennes\\, France"));
        assert!(unfolded.contains("Maintenance: E003"));
        assert!(unfolded.contains("Extracted automatically by a small language model"));
        assert!(unfolded.contains("Note: the page also states \"Apply here by July 3\"."));
        assert_eq!(unfolded.matches("cross-check against the source").count(), 4, "every event carries the disclaimer");
        assert!(unfolded.contains("Submit at: https://popl26.hotcrp.com"));
        assert!(unfolded.contains("Changed 2026-09-20: was 2025-07-03"));
        assert_eq!(unfolded.matches("Changed 2026-09-20").count(), 1, "only the submission event carries the change");
        assert_eq!(ev.len(), 4);
        // Stable UIDs.
        let ev2 = cfp_events("POPL/POPL/2026", &cfp, &prov, Utc::now());
        assert_eq!(ev[0].uid, ev2[0].uid);
        for line in ics.lines() {
            assert!(line.len() <= 75, "line too long: {line}");
        }
    }
}
