//! The GitHub Pages site: one static page listing every conference-year,
//! its dates and links, plus the calendar subscription. Generated on every
//! run into `docs/`; no JavaScript, no external assets.

use crate::schema::{Conference, Deadlines, Stage, Volunteer};
use chrono::{DateTime, Datelike, NaiveDate, Utc};

pub const PAGES_URL: &str = "https://jonasalaif.github.io/pl-conferences";
pub const REPO_URL: &str = "https://github.com/JonasAlaif/pl-conferences";
const RAW_URL: &str = "https://raw.githubusercontent.com/JonasAlaif/pl-conferences/main";

/// What the page shows for one conference-year.
pub struct YearView {
    pub key: String,
    pub label: String,
    pub stage: Stage,
    pub conference: Option<(Conference, String)>,
    pub deadlines: Option<(Deadlines, String)>,
    pub volunteer: Option<(Volunteer, String)>,
    pub last_verified: Option<DateTime<Utc>>,
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn link(url: &str, text: &str) -> String {
    format!(r#"<a href="{}">{}</a>"#, esc(url), esc(text))
}

fn date(d: NaiveDate) -> String {
    d.format("%-d %b %Y").to_string()
}

fn range(a: NaiveDate, b: NaiveDate) -> String {
    if a == b { date(a) } else { format!("{} – {}", date(a), date(b)) }
}

/// A date that has passed is grayed out, so the eye lands on what is still
/// ahead. `last_day` is the last day the entry matters.
fn gone(html: String, last_day: NaiveDate, today: NaiveDate) -> String {
    if last_day < today { format!("<span class=\"gone\">{html}</span>") } else { html }
}

/// What a reader wants to know about an edition today, as (label, row
/// class). With several rounds the last one decides: an edition whose
/// second round is still open has its submission upcoming.
fn status(y: &YearView, today: NaiveDate) -> (&'static str, &'static str) {
    if y.conference.as_ref().is_some_and(|(c, _)| today > c.end) {
        return ("happened", "past");
    }
    match (&y.deadlines, &y.conference) {
        (Some((d, _)), _) => match d.rounds.last() {
            Some(last) if today <= last.submission => ("submission upcoming", "open"),
            _ if crate::schema::deadlines_active(Some(d), today) => ("under review", "review"),
            _ => ("conference upcoming", "closed"),
        },
        (None, Some(_)) => ("conference announced", "conference"),
        (None, None) => ("not announced", "future"),
    }
}

/// Every dated event of an edition.
fn all_dates(y: &YearView) -> Vec<NaiveDate> {
    let mut v = vec![];
    if let Some((d, _)) = &y.deadlines {
        for r in &d.rounds {
            v.push(r.submission);
            v.extend(r.response_start);
            v.extend(r.notification);
        }
    }
    if let Some((c, _)) = &y.conference {
        v.push(c.start);
        v.push(c.end);
    }
    if let Some((vol, _)) = &y.volunteer {
        v.push(vol.deadline);
    }
    v
}

/// Reading order: editions one can still submit to, soonest deadline
/// first; then editions with something else coming up, soonest first; then
/// editions announced without dates yet; then past editions, most recent
/// first. Ties break on the label.
fn sort_key(y: &YearView, today: NaiveDate) -> (u8, i64, String) {
    if status(y, today).0 == "submission upcoming" {
        let next_deadline = y.deadlines.iter().flat_map(|(d, _)| d.rounds.iter().map(|r| r.submission)).filter(|s| *s >= today).min();
        if let Some(n) = next_deadline {
            return (0, n.num_days_from_ce() as i64, y.label.clone());
        }
    }
    let dates = all_dates(y);
    let next = dates.iter().filter(|d| **d >= today).min();
    let last = dates.iter().max();
    match (next, last) {
        (Some(n), _) => (1, n.num_days_from_ce() as i64, y.label.clone()),
        (None, None) => (2, 0, y.label.clone()),
        (None, Some(l)) => (3, -(l.num_days_from_ce() as i64), y.label.clone()),
    }
}

/// One thing on the timeline.
struct Mark {
    date: NaiveDate,
    /// Conference dates are a span, not a point.
    end: Option<NaiveDate>,
    /// Short text next to the tick ("SPLASH'27 R1 deadline").
    text: String,
    /// Full text for the tooltip.
    full: String,
    /// Track, for the colour.
    conference: String,
    /// "deadline", "rebuttal", "notification", "conference" or "volunteers".
    kind: &'static str,
}

/// "OOPSLA'27" from the key "SPLASH/OOPSLA/2027": the track is what one
/// submits to.
fn short_label(key: &str) -> String {
    let (track, year) = track_year(key);
    format!("{track}'{}", year.get(2..).unwrap_or(year))
}

fn track_year(key: &str) -> (&str, &str) {
    let mut parts = key.split('/').skip(1);
    (parts.next().unwrap_or(key), parts.next().unwrap_or(""))
}

/// Every dated event of every edition on or after `today`, soonest first.
fn marks(years: &[YearView], today: NaiveDate) -> Vec<Mark> {
    let mut v = vec![];
    for y in years {
        let short = short_label(&y.key);
        let conference = track_year(&y.key).0.to_string();
        let mut push = |date: NaiveDate, end: Option<NaiveDate>, what: &str, kind: &'static str| {
            if end.unwrap_or(date) >= today {
                v.push(Mark { date, end, text: format!("{short} {what}"), full: format!("{} · {what}", y.label), conference: conference.clone(), kind });
            }
        };
        if let Some((d, _)) = &y.deadlines {
            for (i, r) in d.rounds.iter().enumerate() {
                let prefix = if d.rounds.len() > 1 { format!("R{} ", i + 1) } else { String::new() };
                push(r.submission, None, &format!("{prefix}deadline"), "deadline");
                if let Some(a) = r.response_start {
                    push(a, r.response_end, &format!("{prefix}rebuttal"), "rebuttal");
                }
                if let Some(n) = r.notification {
                    push(n, None, &format!("{prefix}notification"), "notification");
                }
            }
        }
        if let Some((c, _)) = &y.conference {
            push(c.start, Some(c.end), "conference", "conference");
        }
        if let Some((vol, _)) = &y.volunteer {
            push(vol.deadline, None, "volunteers", "volunteers");
        }
    }
    v.sort_by(|a, b| (a.date, &a.text).cmp(&(b.date, &b.text)));
    v
}

/// A stable hue per track name, so that one track keeps its colour from
/// run to run and from year to year.
fn hue(name: &str) -> u32 {
    let h = name.bytes().fold(5381u32, |h, b| h.wrapping_mul(33) ^ u32::from(b));
    // Spread over the wheel in golden-angle steps so that a handful of
    // conferences get clearly different hues.
    (h % 360).wrapping_mul(137) % 360
}

/// Lanes for labels that would otherwise overlap: each label occupies
/// `[x, x + width]` (or `[x - width, x]` when flipped to stay inside `w`),
/// and takes the lowest lane whose last label ends before it starts.
/// Returns (lane, flipped) per label, in the order given (sorted by x).
fn lanes(labels: &[(f64, f64)], w: f64) -> Vec<(usize, bool)> {
    let mut ends: Vec<f64> = vec![];
    let mut out = vec![];
    for &(x, width) in labels {
        let flipped = x + width > w;
        let (start, end) = if flipped { (x - width, x) } else { (x, x + width) };
        let lane = match ends.iter().position(|e| *e + 6.0 <= start) {
            Some(i) => i,
            None => {
                ends.push(f64::MIN);
                ends.len() - 1
            }
        };
        ends[lane] = end;
        out.push((lane, flipped));
    }
    out
}

/// The months from the first of `from`'s month up to and including the
/// month of `to`.
fn months(from: NaiveDate, to: NaiveDate) -> Vec<NaiveDate> {
    let mut v = vec![];
    let mut m = from.with_day(1).expect("day 1");
    while m <= to {
        v.push(m);
        m = if m.month() == 12 { NaiveDate::from_ymd_opt(m.year() + 1, 1, 1) } else { NaiveDate::from_ymd_opt(m.year(), m.month() + 1, 1) }.expect("next month");
    }
    v
}

/// The upcoming year and a bit as one horizontal timeline (inline SVG, no
/// script): months along the axis, a tick and a short label per event,
/// labels stacked into lanes where they would overlap, conference dates as
/// a bar. Everything further than `MONTHS_AHEAD` months out is left to the
/// table below.
fn timeline(years: &[YearView], today: NaiveDate) -> String {
    const MONTHS_AHEAD: u32 = 15;
    const W: f64 = 1100.0;
    const PAD: f64 = 10.0;
    const LANE: f64 = 17.0;
    const FONT: f64 = 6.3; // average glyph width at the label size, for overlap estimates
    let horizon = today.checked_add_months(chrono::Months::new(MONTHS_AHEAD)).expect("horizon");
    let marks: Vec<Mark> = marks(years, today).into_iter().filter(|m| m.date <= horizon).collect();
    if marks.is_empty() {
        return "<p class=\"small\">Nothing upcoming yet.</p>\n".into();
    }
    let last = marks.iter().map(|m| m.end.unwrap_or(m.date)).max().expect("non-empty");
    let months = months(today, last);
    let start = months[0];
    let end = months.last().expect("non-empty").checked_add_months(chrono::Months::new(1)).expect("end");
    let days = f64::from((end - start).num_days() as i32);
    let x = |d: NaiveDate| PAD + f64::from((d.min(end) - start).num_days() as i32) / days * (W - 2.0 * PAD);
    let placed = lanes(&marks.iter().map(|m| (x(m.date), FONT * m.text.chars().count() as f64 + 8.0)).collect::<Vec<_>>(), W - PAD);
    let top_lanes = placed.iter().map(|(l, _)| l + 1).max().unwrap_or(1);
    let axis = 12.0 + top_lanes as f64 * LANE;
    let height = axis + 36.0;
    let mut s = format!("<div class=\"wrap\"><svg class=\"timeline\" viewBox=\"0 0 {W} {height}\" role=\"img\" aria-label=\"Upcoming dates on a timeline\">\n");
    // Months.
    for (i, m) in months.iter().enumerate() {
        let xm = x(*m);
        let name = if m.month() == 1 || i == 0 { m.format("%b %Y") } else { m.format("%b") };
        s.push_str(&format!("<line class=\"month\" x1=\"{xm:.1}\" y1=\"{:.1}\" x2=\"{xm:.1}\" y2=\"{:.1}\"/><text class=\"month\" x=\"{:.1}\" y=\"{:.1}\">{name}</text>\n", axis - 4.0, axis + 10.0, xm + 3.0, axis + 22.0));
    }
    s.push_str(&format!("<line class=\"axis\" x1=\"{PAD}\" y1=\"{axis:.1}\" x2=\"{:.1}\" y2=\"{axis:.1}\"/>\n", W - PAD));
    let xt = x(today);
    s.push_str(&format!("<line class=\"today\" x1=\"{xt:.1}\" y1=\"4\" x2=\"{xt:.1}\" y2=\"{:.1}\"/><text class=\"today\" x=\"{:.1}\" y=\"{:.1}\">today</text>\n", axis + 12.0, xt + 3.0, axis + 33.0));
    // Events.
    for (m, (lane, flipped)) in marks.iter().zip(&placed) {
        let xm = x(m.date);
        let y = axis - 6.0 - *lane as f64 * LANE;
        s.push_str(&format!("<g class=\"ev {}\" style=\"--h:{}\"><title>{}: {}</title>", m.kind, hue(&m.conference), esc(&m.full), if let Some(e) = m.end { range(m.date, e) } else { date(m.date) }));
        if let Some(e) = m.end {
            s.push_str(&format!("<rect x=\"{xm:.1}\" y=\"{:.1}\" width=\"{:.1}\" height=\"5\" rx=\"1\"/>", axis - 2.5, (x(e) - xm).max(3.0)));
        }
        s.push_str(&format!("<line x1=\"{xm:.1}\" y1=\"{:.1}\" x2=\"{xm:.1}\" y2=\"{axis:.1}\"/>", y - 9.0));
        let (tx, anchor) = if *flipped { (xm - 3.0, " text-anchor=\"end\"") } else { (xm + 3.0, "") };
        s.push_str(&format!("<text x=\"{tx:.1}\" y=\"{y:.1}\"{anchor}>{}</text></g>\n", esc(&m.text)));
    }
    s.push_str("</svg></div>\n");
    s
}

/// Render the whole page. `maintenance` is the one-line summary from the state.
pub fn render(years: &[YearView], maintenance: &str, generated: DateTime<Utc>) -> String {
    let mut h = String::with_capacity(16_000);
    h.push_str(&format!(r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>PL conference deadlines</title>
<style>
  :root {{ color-scheme: light dark; --fg: #1a1a1a; --bg: #fff; --muted: #666; --line: #ddd; --accent: #0b57d0; --open: #e8f5e9; --review: #fffde7; --closed: #fff3e0; --past: #f5f5f5; --future: #e3f2fd; --conference: #ede7f6; }}
  @media (prefers-color-scheme: dark) {{ :root {{ --fg: #e6e6e6; --bg: #121212; --muted: #9a9a9a; --line: #333; --accent: #8ab4f8; --open: #1b3a22; --review: #33301a; --closed: #3d2e14; --past: #1e1e1e; --future: #14283a; --conference: #2a2340; }} }}
  body {{ font: 15px/1.5 system-ui, -apple-system, "Segoe UI", sans-serif; color: var(--fg); background: var(--bg); margin: 0; padding: 0 16px 48px; }}
  main {{ max-width: 1100px; margin: 0 auto; }}
  h1 {{ font-size: 1.6rem; margin: 32px 0 4px; }}
  h2 {{ font-size: 1.15rem; margin: 32px 0 8px; }}
  p.lead {{ color: var(--muted); margin: 0 0 16px; }}
  code {{ background: var(--past); padding: 2px 6px; border-radius: 4px; font-size: .9em; word-break: break-all; }}
  a {{ color: var(--accent); }}
  table {{ border-collapse: collapse; width: 100%; font-size: .93rem; }}
  th, td {{ text-align: left; vertical-align: top; padding: 8px 10px; border-bottom: 1px solid var(--line); }}
  th {{ font-weight: 600; color: var(--muted); font-size: .8rem; text-transform: uppercase; letter-spacing: .03em; }}
  .gone {{ color: var(--muted); opacity: .55; }} .gone strong {{ font-weight: normal; }}
  tr.open {{ background: var(--open); }} tr.review {{ background: var(--review); }} tr.closed {{ background: var(--closed); }} tr.past {{ background: var(--past); color: var(--muted); }} tr.future {{ background: var(--future); }} tr.conference {{ background: var(--conference); }}
  .stage {{ font-size: .78rem; text-transform: uppercase; letter-spacing: .03em; color: var(--muted); white-space: nowrap; }}
  .small {{ font-size: .85rem; color: var(--muted); }}
  .wrap {{ overflow-x: auto; }}
  svg.timeline {{ display: block; width: 100%; min-width: 860px; font-size: 11px; }}
  svg.timeline line.axis {{ stroke: var(--fg); stroke-width: 1.5; }}
  svg.timeline line.month {{ stroke: var(--line); }} svg.timeline text.month {{ fill: var(--muted); font-size: 10px; text-transform: uppercase; letter-spacing: .03em; }}
  svg.timeline line.today {{ stroke: var(--accent); stroke-dasharray: 3 3; }} svg.timeline text.today {{ fill: var(--accent); font-size: 10px; }}
  svg.timeline .ev {{ --c: hsl(var(--h) 60% 36%); }}
  @media (prefers-color-scheme: dark) {{ svg.timeline .ev {{ --c: hsl(var(--h) 65% 72%); }} }}
  svg.timeline .ev line {{ stroke: var(--c); stroke-width: 1; opacity: .5; }} svg.timeline .ev rect {{ fill: var(--c); }}
  svg.timeline .ev text {{ fill: var(--c); paint-order: stroke; stroke: var(--bg); stroke-width: 3px; stroke-linejoin: round; }} svg.timeline .ev.deadline text {{ font-weight: 700; }}
  svg.timeline .ev.rebuttal text, svg.timeline .ev.notification text, svg.timeline .ev.volunteers text {{ opacity: .75; }}
  svg.timeline .ev:hover text {{ opacity: 1; text-decoration: underline; }} svg.timeline .ev:hover line {{ opacity: 1; stroke-width: 2; }}
  footer {{ margin-top: 40px; color: var(--muted); font-size: .85rem; }}
</style>
</head>
<body>
<main>
<h1>PL conference deadlines</h1>
<p class="lead">Submission deadlines, rebuttals, notifications, conference dates and student-volunteer deadlines for programming-languages conferences, collected automatically twice a week by a small language model.</p>
<p>Subscribe in your calendar app: <code>{pages}/all.ics</code> (or <a href="webcal://jonasalaif.github.io/pl-conferences/all.ics">webcal link</a>). Every entry says which page it was extracted from; the source is authoritative, the calendar is a convenience.</p>
"#, pages = PAGES_URL));

    let today = generated.date_naive();

    // Upcoming
    h.push_str("<h2>Upcoming</h2>\n");
    h.push_str(&timeline(years, today));

    // Conferences
    h.push_str("<h2>Conferences</h2>\n<div class=\"wrap\"><table>\n<tr><th>Conference</th><th>Status</th><th>Paper deadlines</th><th>Conference</th><th>Volunteers</th><th>Links</th></tr>\n");
    let mut sorted: Vec<&YearView> = years.iter().collect();
    sorted.sort_by_key(|y| sort_key(y, today));
    for y in sorted {
        let deadlines = match &y.deadlines {
            Some((d, _)) => d.rounds.iter().enumerate().map(|(i, r)| {
                let prefix = if d.rounds.len() > 1 { format!("R{} ", i + 1) } else { String::new() };
                let mut s = gone(format!("{prefix}<strong>{}</strong>", date(r.submission)), r.submission, today);
                if let (Some(a), Some(b)) = (r.response_start, r.response_end.or(r.response_start)) {
                    s.push_str(&format!("<br>{}", gone(format!("<span class=\"small\">rebuttal {}</span>", range(a, b)), b, today)));
                }
                if let Some(n) = r.notification {
                    s.push_str(&format!("<br>{}", gone(format!("<span class=\"small\">notification {}</span>", date(n)), n, today)));
                }
                if let Some(c) = &r.submission_conflict {
                    s.push_str(&format!("<br><span class=\"small\">page also states “{}”</span>", esc(c)));
                }
                s
            }).collect::<Vec<_>>().join("<br>"),
            None => "<span class=\"small\">not announced yet</span>".into(),
        };
        let conference = match &y.conference {
            Some((c, _)) => {
                let loc = [c.city.clone(), c.country.clone()].into_iter().flatten().collect::<Vec<_>>().join(", ");
                gone(format!("{}{}", range(c.start, c.end), if loc.is_empty() { String::new() } else { format!("<br><span class=\"small\">{}</span>", esc(&loc)) }), c.end, today)
            }
            None => "<span class=\"small\">not announced yet</span>".into(),
        };
        let volunteer = match &y.volunteer {
            Some((v, _)) => {
                let mut s = gone(format!("apply by <strong>{}</strong>", date(v.deadline)), v.deadline, today);
                if let Some(u) = &v.application_url {
                    s.push_str(&format!("<br>{}", link(u, "sign-up page")));
                }
                s
            }
            None => "<span class=\"small\">–</span>".into(),
        };
        let mut links = vec![];
        if let Some((d, src)) = &y.deadlines {
            links.push(link(src, "call for papers"));
            if let Some(u) = &d.submission_url {
                links.push(link(u, "submission site"));
            }
            links.push(link(&format!("{RAW_URL}/conferences/{}/cfp.ics", y.key), "deadlines .ics"));
        }
        if let Some((_, src)) = &y.conference {
            if y.deadlines.as_ref().map(|(_, s)| s) != Some(src) {
                links.push(link(src, "conference page"));
            }
            links.push(link(&format!("{RAW_URL}/conferences/{}/conference.ics", y.key), "conference .ics"));
        }
        if let Some((_, src)) = &y.volunteer {
            links.push(link(src, "volunteers page"));
            links.push(link(&format!("{RAW_URL}/conferences/{}/volunteer.ics", y.key), "volunteer .ics"));
        }
        let verified = y.last_verified.map(|t| format!("<br><span class=\"small\">checked {}</span>", t.format("%-d %b %Y"))).unwrap_or_default();
        h.push_str(&format!(
            "<tr class=\"{}\"><td><strong>{}</strong>{}</td><td class=\"stage\">{}</td><td>{}</td><td>{}</td><td>{}</td><td class=\"small\">{}</td></tr>\n",
            status(y, today).1, esc(&y.label), verified, status(y, today).0, deadlines, conference, volunteer, links.join("<br>")
        ));
    }
    h.push_str("</table></div>\n");

    h.push_str(&format!(r#"<footer>
<p>Status: {}. Dates are extracted by a small language model (Qwen3.5-4B via Ollama on a GitHub Actions runner) and validated for consistency, but read the linked pages before relying on them. Generated {}.</p>
<p><a href="{repo}">Source, data and how it works</a> · <a href="{repo}/blob/main/MAINTENANCE.md">Maintenance status</a></p>
</footer>
</main>
</body>
</html>
"#, esc(maintenance), generated.format("%-d %b %Y %H:%M UTC"), repo = REPO_URL));
    h
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::ValidRound;

    #[test]
    fn renders_a_year_with_all_parts_and_escapes_text() {
        let y = YearView {
            key: "POPL/POPL/2026".into(),
            label: "POPL 2026 <test>".into(),
            stage: Stage::DeadlinesAvailable,
            conference: Some((Conference { start: NaiveDate::from_ymd_opt(2026, 1, 11).unwrap(), end: NaiveDate::from_ymd_opt(2026, 1, 17).unwrap(), city: Some("Rennes".into()), country: Some("France".into()) }, "https://popl26.sigplan.org/".into())),
            deadlines: Some((Deadlines { rounds: vec![ValidRound { label: String::new(), submission: NaiveDate::from_ymd_opt(2025, 7, 10).unwrap(), submission_conflict: None, response_start: NaiveDate::from_ymd_opt(2025, 9, 8), response_end: NaiveDate::from_ymd_opt(2025, 9, 11), notification: NaiveDate::from_ymd_opt(2025, 11, 6) }], submission_details: String::new(), submission_url: Some("https://popl26.hotcrp.com".into()) }, "https://popl26.sigplan.org/track/x".into())),
            volunteer: Some((Volunteer { deadline: NaiveDate::from_ymd_opt(2025, 11, 10).unwrap(), deadline_conflict: None, how_to_apply: String::new(), application_url: None }, "https://popl26.sigplan.org/track/sv".into())),
            last_verified: None,
        };
        let html = render(&[y], "No maintenance needed.", Utc::now());
        assert!(html.contains("POPL 2026 &lt;test&gt;"), "labels are escaped");
        assert!(html.contains("10 Jul 2025") && html.contains("rebuttal 8 Sep 2025 – 11 Sep 2025") && html.contains("notification 6 Nov 2025"));
        assert!(html.contains("Rennes, France") && html.contains("apply by <strong>10 Nov 2025"));
        assert!(html.contains("https://popl26.hotcrp.com") && html.contains("/conferences/POPL/POPL/2026/cfp.ics"));
        assert!(html.contains("all.ics"));
    }

    #[test]
    fn the_status_follows_the_last_round_and_open_submissions_come_first() {
        let d = |y, m, dd| NaiveDate::from_ymd_opt(y, m, dd).unwrap();
        let round = |sub: NaiveDate, notif: NaiveDate| ValidRound { label: String::new(), submission: sub, submission_conflict: None, response_start: None, response_end: None, notification: Some(notif) };
        let mk = |label: &str, rounds: Vec<ValidRound>, conf: Option<(NaiveDate, NaiveDate)>| YearView {
            key: label.into(),
            label: label.into(),
            stage: Stage::Future,
            conference: conf.map(|(s, e)| (Conference { start: s, end: e, city: None, country: None }, String::new())),
            deadlines: (!rounds.is_empty()).then(|| (Deadlines { rounds, submission_details: String::new(), submission_url: None }, String::new())),
            volunteer: None,
            last_verified: None,
        };
        let today = d(2026, 9, 17);
        // Round 1 notified, round 2 still open: the last round decides.
        let two_rounds = mk("two rounds", vec![round(d(2026, 5, 28), d(2026, 8, 6)), round(d(2026, 10, 15), d(2026, 12, 22))], Some((d(2027, 4, 12), d(2027, 4, 15))));
        let review = mk("under review", vec![round(d(2026, 7, 9), d(2026, 10, 5))], Some((d(2027, 1, 10), d(2027, 1, 16))));
        let decided = mk("decided", vec![round(d(2026, 3, 17), d(2026, 6, 10))], Some((d(2026, 10, 4), d(2026, 10, 9))));
        let announced = mk("announced", vec![], Some((d(2027, 6, 21), d(2027, 6, 24))));
        let unknown = mk("unknown", vec![], None);
        let over = mk("over", vec![round(d(2025, 7, 10), d(2025, 10, 2))], Some((d(2026, 1, 11), d(2026, 1, 17))));
        let later_open = mk("later open", vec![round(d(2027, 1, 20), d(2027, 4, 23))], None);
        // Past dates are grayed out, upcoming ones are not.
        let html = render(&[mk("two rounds", vec![round(d(2020, 5, 28), d(2020, 8, 6)), round(d(2999, 10, 15), d(2999, 12, 22))], None)], "", Utc::now());
        assert!(html.contains("<span class=\"gone\">R1 <strong>28 May 2020</strong></span>") && html.contains("R2 <strong>15 Oct 2999</strong>") && !html.contains("<span class=\"gone\">R2"), "{html}");
        assert_eq!(status(&two_rounds, today).0, "submission upcoming");
        assert_eq!(status(&review, today).0, "under review");
        assert_eq!(status(&decided, today).0, "conference upcoming");
        assert_eq!(status(&announced, today).0, "conference announced");
        assert_eq!(status(&unknown, today).0, "not announced");
        assert_eq!(status(&over, today).0, "happened");
        // Open submissions first (soonest deadline first), though the
        // edition under review has an earlier next date (5 Oct).
        let mut v = vec![&over, &unknown, &announced, &decided, &review, &later_open, &two_rounds];
        v.sort_by_key(|y| sort_key(y, today));
        let order: Vec<&str> = v.iter().map(|y| y.label.as_str()).collect();
        assert_eq!(order, vec!["two rounds", "later open", "decided", "under review", "announced", "unknown", "over"]);
    }

    #[test]
    fn the_timeline_stacks_overlapping_labels_and_stops_at_the_horizon() {
        let d = |y, m, dd| NaiveDate::from_ymd_opt(y, m, dd).unwrap();
        // Two labels at nearly the same x go to different lanes; a far one
        // returns to the first lane; one near the right edge is flipped.
        assert_eq!(lanes(&[(100.0, 80.0), (110.0, 80.0), (300.0, 80.0), (1080.0, 80.0)], 1090.0), vec![(0, false), (1, false), (0, false), (0, true)]);
        assert_eq!(short_label("SPLASH/OOPSLA/2027"), "OOPSLA'27", "the track, which is what one submits to");
        assert_eq!(short_label("POPL/POPL/2027"), "POPL'27");
        assert_eq!(months(d(2026, 9, 17), d(2027, 1, 3)).len(), 5, "September to January");
        let mk = |key: &str, sub: NaiveDate, conf: (NaiveDate, NaiveDate)| YearView {
            key: key.into(),
            label: key.replace('/', " "),
            stage: Stage::Future,
            conference: Some((Conference { start: conf.0, end: conf.1, city: None, country: None }, String::new())),
            deadlines: Some((Deadlines { rounds: vec![ValidRound { label: String::new(), submission: sub, submission_conflict: None, response_start: None, response_end: None, notification: None }], submission_details: String::new(), submission_url: None }, String::new())),
            volunteer: None,
            last_verified: None,
        };
        let today = d(2026, 9, 17);
        let years = [mk("POPL/POPL/2027", d(2026, 7, 9), (d(2027, 1, 10), d(2027, 1, 16))), mk("ICFP/ICFP/2028", d(2028, 2, 25), (d(2028, 9, 26), d(2028, 10, 1)))];
        let svg = timeline(&years, today);
        assert!(svg.contains("POPL'27 conference") && svg.contains("<rect"), "the conference is a bar: {svg}");
        assert!(svg.contains("style=\"--h:") && svg.contains("<title>POPL POPL 2027 · conference: 10 Jan 2027 – 16 Jan 2027</title>"), "colour and tooltip: {svg}");
        assert!(!svg.contains("POPL'27 deadline"), "a passed deadline is not shown");
        assert!(!svg.contains("ICFP'28"), "beyond the horizon is left to the table");
        assert!(svg.contains(">Sep 2026<") && svg.contains(">Jan 2027<") && !svg.contains(">Mar<"), "months from today's to the last event's: {svg}");
        assert!(svg.contains("today"));
        assert_eq!(timeline(&[], today), "<p class=\"small\">Nothing upcoming yet.</p>\n");
    }

    #[test]
    fn editions_are_ordered_by_next_date_then_unknown_then_past() {
        let d = |y, m, dd| NaiveDate::from_ymd_opt(y, m, dd).unwrap();
        let today = d(2026, 9, 16);
        let mk = |label: &str, conf: Option<(NaiveDate, NaiveDate)>| YearView {
            key: label.into(),
            label: label.into(),
            stage: Stage::Future,
            conference: conf.map(|(s, e)| (Conference { start: s, end: e, city: None, country: None }, String::new())),
            deadlines: None,
            volunteer: None,
            last_verified: None,
        };
        let later = mk("B later", Some((d(2027, 6, 1), d(2027, 6, 5))));
        let soon = mk("C soon", Some((d(2026, 10, 4), d(2026, 10, 9))));
        let unknown = mk("A unknown", None);
        let past_old = mk("D old", Some((d(2026, 1, 11), d(2026, 1, 17))));
        let past_recent = mk("E recent", Some((d(2026, 7, 26), d(2026, 7, 29))));
        let mut v = vec![&later, &past_old, &unknown, &soon, &past_recent];
        v.sort_by_key(|y| sort_key(y, today));
        let order: Vec<&str> = v.iter().map(|y| y.label.as_str()).collect();
        assert_eq!(order, vec!["C soon", "B later", "A unknown", "E recent", "D old"]);
    }
}
