//! The GitHub Pages site: one static page listing every conference-year,
//! its dates and links, plus the calendar subscription. Generated on every
//! run into `docs/`; no JavaScript, no external assets.

use crate::schema::{Conference, Deadlines, Stage, Volunteer};
use chrono::{DateTime, NaiveDate, Utc};

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

fn stage_class(s: Stage) -> &'static str {
    match s {
        Stage::Future => "future",
        Stage::ConferenceAvailable => "conference",
        Stage::DeadlinesAvailable => "open",
        Stage::PostRebuttal => "closed",
        Stage::Happened => "past",
    }
}

/// Render the whole page. `upcoming` is (date, conference label, event),
/// already sorted; `maintenance` is the one-line summary from the state.
pub fn render(years: &[YearView], upcoming: &[(NaiveDate, String, String)], maintenance: &str, generated: DateTime<Utc>) -> String {
    let mut h = String::with_capacity(16_000);
    h.push_str(&format!(r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>PL conference deadlines</title>
<style>
  :root {{ color-scheme: light dark; --fg: #1a1a1a; --bg: #fff; --muted: #666; --line: #ddd; --accent: #0b57d0; --open: #e8f5e9; --closed: #fff3e0; --past: #f5f5f5; --future: #e3f2fd; --conference: #ede7f6; }}
  @media (prefers-color-scheme: dark) {{ :root {{ --fg: #e6e6e6; --bg: #121212; --muted: #9a9a9a; --line: #333; --accent: #8ab4f8; --open: #1b3a22; --closed: #3d2e14; --past: #1e1e1e; --future: #14283a; --conference: #2a2340; }} }}
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
  tr.open {{ background: var(--open); }} tr.closed {{ background: var(--closed); }} tr.past {{ background: var(--past); color: var(--muted); }} tr.future {{ background: var(--future); }} tr.conference {{ background: var(--conference); }}
  .stage {{ font-size: .78rem; text-transform: uppercase; letter-spacing: .03em; color: var(--muted); white-space: nowrap; }}
  .small {{ font-size: .85rem; color: var(--muted); }}
  ul.upcoming {{ list-style: none; padding: 0; margin: 0; }}
  ul.upcoming li {{ padding: 4px 0; border-bottom: 1px solid var(--line); }}
  ul.upcoming time {{ display: inline-block; min-width: 8.5em; font-variant-numeric: tabular-nums; }}
  .wrap {{ overflow-x: auto; }}
  footer {{ margin-top: 40px; color: var(--muted); font-size: .85rem; }}
</style>
</head>
<body>
<main>
<h1>PL conference deadlines</h1>
<p class="lead">Submission deadlines, rebuttals, notifications, conference dates and student-volunteer deadlines for programming-languages conferences, collected automatically twice a week by a small language model.</p>
<p>Subscribe in your calendar app: <code>{pages}/all.ics</code> (or <a href="webcal://jonasalaif.github.io/pl-conferences/all.ics">webcal link</a>). Every entry says which page it was extracted from; the source is authoritative, the calendar is a convenience.</p>
"#, pages = PAGES_URL));

    // Upcoming
    h.push_str("<h2>Upcoming</h2>\n");
    if upcoming.is_empty() {
        h.push_str("<p class=\"small\">Nothing upcoming yet.</p>\n");
    } else {
        h.push_str("<ul class=\"upcoming\">\n");
        for (d, label, what) in upcoming {
            h.push_str(&format!("<li><time datetime=\"{}\">{}</time> <strong>{}</strong> · {}</li>\n", d, date(*d), esc(label), esc(what)));
        }
        h.push_str("</ul>\n");
    }

    // Conferences
    h.push_str("<h2>Conferences</h2>\n<div class=\"wrap\"><table>\n<tr><th>Conference</th><th>Status</th><th>Paper deadlines</th><th>Conference</th><th>Volunteers</th><th>Links</th></tr>\n");
    let mut sorted: Vec<&YearView> = years.iter().collect();
    // Active editions first, then by label.
    sorted.sort_by_key(|y| (!y.stage.active(), y.label.clone()));
    for y in sorted {
        let deadlines = match &y.deadlines {
            Some((d, _)) => d.rounds.iter().enumerate().map(|(i, r)| {
                let prefix = if d.rounds.len() > 1 { format!("R{} ", i + 1) } else { String::new() };
                let mut s = format!("{prefix}<strong>{}</strong>", date(r.submission));
                if let (Some(a), Some(b)) = (r.response_start, r.response_end.or(r.response_start)) {
                    s.push_str(&format!("<br><span class=\"small\">rebuttal {}</span>", range(a, b)));
                }
                if let Some(n) = r.notification {
                    s.push_str(&format!("<br><span class=\"small\">notification {}</span>", date(n)));
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
                format!("{}{}", range(c.start, c.end), if loc.is_empty() { String::new() } else { format!("<br><span class=\"small\">{}</span>", esc(&loc)) })
            }
            None => "<span class=\"small\">not announced yet</span>".into(),
        };
        let volunteer = match &y.volunteer {
            Some((v, _)) => {
                let mut s = format!("apply by <strong>{}</strong>", date(v.deadline));
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
            stage_class(y.stage), esc(&y.label), verified, esc(y.stage.label()), deadlines, conference, volunteer, links.join("<br>")
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
        let html = render(&[y], &[(NaiveDate::from_ymd_opt(2026, 1, 11).unwrap(), "POPL 2026".into(), "Conference".into())], "No maintenance needed.", Utc::now());
        assert!(html.contains("POPL 2026 &lt;test&gt;"), "labels are escaped");
        assert!(html.contains("10 Jul 2025") && html.contains("rebuttal 8 Sep 2025 – 11 Sep 2025") && html.contains("notification 6 Nov 2025"));
        assert!(html.contains("Rennes, France") && html.contains("apply by <strong>10 Nov 2025"));
        assert!(html.contains("https://popl26.hotcrp.com") && html.contains("/conferences/POPL/POPL/2026/cfp.ics"));
        assert!(html.contains("all.ics") && html.contains("deadlines available"));
    }
}
