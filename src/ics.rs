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

/// Provenance appended to descriptions.
#[derive(Debug, Clone, Default)]
pub struct Provenance {
    pub source_url: String,
    pub codes: Vec<String>,
    /// Recorded changes; entries whose `field` matches an event are listed in it.
    pub history: Vec<Change>,
}

impl Provenance {
    fn note(&self, field: &str) -> String {
        let mut s = String::new();
        for c in self.history.iter().filter(|c| c.field == field || c.field == "rounds") {
            s.push_str(&format!("Changed {}: was {} (now {}).\n", c.at.format("%Y-%m-%d"), c.old, c.new));
        }
        s.push_str(&format!("Source: {}", self.source_url));
        if !self.codes.is_empty() {
            s.push_str(&format!("\nMaintenance: {} - see {MAINTENANCE_URL}", self.codes.join(", ")));
        }
        s
    }
}

fn uid(key: &str, event: &str) -> String {
    let mut h = Sha1::new();
    h.update(format!("{key}/{event}").as_bytes());
    let hex: String = h.finalize().iter().map(|b| format!("{b:02x}")).collect();
    format!("{hex}@pl-conferences")
}

fn event(label: &str, key: &str, stamp: DateTime<Utc>, name: &str, desc: String, loc: &str, start: NaiveDate, end: NaiveDate) -> Event {
    Event { uid: uid(key, name), summary: format!("[{label}] {name}"), description: desc, location: loc.to_string(), start, end, stamp }
}

/// The single conference event (dates and location).
pub fn conference_events(label: &str, key: &str, c: &Conference, prov: &Provenance, stamp: DateTime<Utc>) -> Vec<Event> {
    let loc = match (&c.city, &c.country) {
        (Some(ci), Some(co)) => format!("{ci}, {co}"),
        (Some(x), None) | (None, Some(x)) => x.clone(),
        (None, None) => String::new(),
    };
    vec![event(label, key, stamp, "Conference", prov.note("conference"), &loc, c.start, c.end)]
}

/// Deadline events: submission, rebuttal and notification per round.
pub fn deadline_events(label: &str, key: &str, d: &Deadlines, prov: &Provenance, stamp: DateTime<Utc>) -> Vec<Event> {
    let mut events = vec![];
    let two = d.rounds.len() > 1;
    let mk = |name: &str, desc: String, loc: &str, start: NaiveDate, end: NaiveDate| event(label, key, stamp, name, desc, loc, start, end);
    for (i, r) in d.rounds.iter().enumerate() {
        let prefix = if two { format!("R{} ", i + 1) } else { String::new() };
        let n = i + 1;
        let mut parts = vec![];
        if !d.submission_details.is_empty() {
            parts.push(d.submission_details.clone());
        }
        if let Some(u) = &d.submission_url {
            parts.push(format!("Submit at: {u}"));
        }
        parts.push(prov.note(&format!("round {n} submission")));
        let desc = parts.join("\n\n");
        events.push(mk(&format!("{prefix}Paper Submission Deadline"), desc, "", r.submission, r.submission));
        if let (Some(s), Some(e)) = (r.response_start, r.response_end.or(r.response_start)) {
            events.push(mk(&format!("{prefix}Rebuttal"), prov.note(&format!("round {n} response")), "", s, e));
        }
        if let Some(nd) = r.notification {
            events.push(mk(&format!("{prefix}Notification"), prov.note(&format!("round {n} notification")), "", nd, nd));
        }
    }
    events
}

/// Both parts of a call for papers, for tests and the aggregate view.
pub fn cfp_events(label: &str, key: &str, cfp: &Cfp, prov: &Provenance, stamp: DateTime<Utc>) -> Vec<Event> {
    let mut events = vec![];
    if let Some(d) = cfp.deadlines() {
        events.extend(deadline_events(label, key, &d, prov, stamp));
    }
    if let Some(c) = &cfp.conference {
        events.extend(conference_events(label, key, c, prov, stamp));
    }
    events
}

pub fn volunteer_events(label: &str, key: &str, v: &Volunteer, prov: &Provenance, stamp: DateTime<Utc>) -> Vec<Event> {
    let mut parts = vec![];
    if !v.how_to_apply.is_empty() {
        parts.push(v.how_to_apply.clone());
    }
    if let Some(u) = &v.application_url {
        parts.push(format!("Apply at: {u}"));
    }
    parts.push(prov.note("volunteer deadline"));
    let desc = parts.join("\n\n");
    vec![Event {
        uid: uid(key, "Volunteer Application Deadline"),
        summary: format!("[{label}] Volunteer Application Deadline"),
        description: desc,
        location: String::new(),
        start: v.deadline,
        end: v.deadline,
        stamp,
    }]
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
        let ev = cfp_events("POPL 2026", "POPL/POPL/2026", &cfp, &prov, stamp);
        let ics = calendar("POPL 2026", &ev);
        let unfolded = ics.replace("\r\n ", "");
        assert!(ics.contains("SUMMARY:[POPL 2026] Paper Submission Deadline"));
        assert!(ics.contains("DTSTART;VALUE=DATE:20250710\r\nDTEND;VALUE=DATE:20250711"));
        assert!(ics.contains("SUMMARY:[POPL 2026] Rebuttal"));
        assert!(ics.contains("DTEND;VALUE=DATE:20250912"));
        assert!(ics.contains("LOCATION:Rennes\\, France"));
        assert!(unfolded.contains("Maintenance: E003"));
        assert!(unfolded.contains("Submit at: https://popl26.hotcrp.com"));
        assert!(unfolded.contains("Changed 2026-09-20: was 2025-07-03"));
        assert_eq!(unfolded.matches("Changed 2026-09-20").count(), 1, "only the submission event carries the change");
        assert_eq!(ev.len(), 4);
        // Stable UIDs.
        let ev2 = cfp_events("POPL 2026", "POPL/POPL/2026", &cfp, &prov, Utc::now());
        assert_eq!(ev[0].uid, ev2[0].uid);
        for line in ics.lines() {
            assert!(line.len() <= 75, "line too long: {line}");
        }
    }
}
