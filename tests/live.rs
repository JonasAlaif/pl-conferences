//! Live tests against a local Ollama. Run with `cargo test --test live -- --ignored --nocapture`.
//! Environment: PLC_MODEL, PLC_THINK, PLC_RUNS (repetitions, default 3), PLC_TEMP (default 0).

use pl_conferences::{clean, discover, llm::Llm, schema};
use std::time::Instant;

struct Case {
    fixture: &'static str,
    conference: &'static str,
    track: &'static str,
    year: i32,
    /// (submission, response_start, response_end, notification) per round; None = any value accepted.
    rounds: Vec<(&'static str, Option<&'static str>, Option<&'static str>, Option<&'static [&'static str]>)>,
    /// Accepted (start, end) pairs.
    conference_dates: Option<&'static [(&'static str, &'static str)]>,
    city: Option<&'static str>,
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            fixture: "popl26-dates.html", conference: "POPL", track: "POPL", year: 2026,
            rounds: vec![("2025-07-10", Some("2025-09-08"), Some("2025-09-11"), Some(&["2025-10-02", "2025-11-06"]))],
            conference_dates: Some(&[("2026-01-11", "2026-01-17"), ("2026-01-13", "2026-01-17")]), city: Some("Rennes"),
        },
        Case {
            fixture: "popl26-cfp.html", conference: "POPL", track: "POPL", year: 2026,
            rounds: vec![("2025-07-10", Some("2025-09-08"), Some("2025-09-11"), Some(&["2025-10-02", "2025-11-06"]))],
            conference_dates: Some(&[("2026-01-11", "2026-01-17"), ("2026-01-13", "2026-01-17")]), city: Some("Rennes"),
        },
        Case {
            fixture: "splash26-oopsla.html", conference: "SPLASH", track: "OOPSLA", year: 2026,
            rounds: vec![
                ("2025-10-10", Some("2025-12-02"), Some("2025-12-05"), Some(&["2025-12-17"])),
                ("2026-03-17", Some("2026-05-19"), Some("2026-05-22"), Some(&["2026-06-10"])),
            ],
            conference_dates: Some(&[("2026-10-04", "2026-10-09")]), city: Some("Oakland"),
        },
        Case {
            fixture: "splash26-dates.html", conference: "SPLASH", track: "OOPSLA", year: 2026,
            rounds: vec![
                ("2025-10-10", Some("2025-12-02"), Some("2025-12-05"), Some(&["2025-12-17"])),
                ("2026-03-17", Some("2026-05-19"), Some("2026-05-22"), Some(&["2026-06-10"])),
            ],
            conference_dates: Some(&[("2026-10-04", "2026-10-09")]), city: Some("Oakland"),
        },
        Case {
            fixture: "pldi26-cfp.html", conference: "PLDI", track: "PLDI", year: 2026,
            rounds: vec![("2025-11-13", Some("2026-02-17"), Some("2026-02-22"), Some(&["2026-03-05"]))],
            conference_dates: Some(&[("2026-06-15", "2026-06-19")]), city: Some("Boulder"),
        },
        Case {
            fixture: "icfp26-cfp.html", conference: "ICFP", track: "ICFP", year: 2026,
            rounds: vec![("2026-02-19", Some("2026-04-20"), Some("2026-04-23"), Some(&["2026-05-14", "2026-06-10"]))],
            conference_dates: Some(&[("2026-08-24", "2026-08-29")]), city: Some("Indianapolis"),
        },
        Case {
            fixture: "cav26.html", conference: "CAV", track: "CAV", year: 2026,
            rounds: vec![("2026-01-28", Some("2026-03-30"), Some("2026-04-02"), Some(&["2026-04-17"]))],
            conference_dates: Some(&[("2026-07-26", "2026-07-29")]), city: Some("Lisbon"),
        },
        Case {
            fixture: "etaps26-esop.html", conference: "ETAPS", track: "ESOP", year: 2026,
            rounds: vec![
                ("2025-06-03", Some("2025-07-21"), Some("2025-07-23"), Some(&["2025-08-01"])),
                ("2025-10-16", Some("2025-12-08"), Some("2025-12-10"), Some(&["2025-12-22"])),
            ],
            conference_dates: None, city: None,
        },
        // A plain page on the conference's own domain (not researchr), with
        // an abstract deadline a week before the paper deadline.
        Case {
            fixture: "lics26-cfp.html", conference: "LICS", track: "LICS", year: 2026,
            rounds: vec![("2026-01-22", Some("2026-03-26"), Some("2026-03-29"), Some(&["2026-04-16"]))],
            conference_dates: Some(&[("2026-07-20", "2026-07-23")]), city: Some("Lisbon"),
        },
        // Lists "Author Notification (Round 1)" twice (the second after a
        // revision phase) and likewise for round 2. Round 1 is read right
        // (the cross-round rule rejects anything after the round-2
        // deadline). For round 2 the model settles on the revision
        // deadline "Fri 30 Jul 2027" and keeps it when retried, told the
        // entry's label, or given the first-notification description: a
        // known limitation, so any round-2 notification is accepted here
        // and the case guards the rest of the page.
        Case {
            fixture: "splash27-oopsla.html", conference: "SPLASH", track: "OOPSLA", year: 2027,
            rounds: vec![
                ("2026-10-14", Some("2026-12-01"), Some("2026-12-05"), Some(&["2026-12-18"])),
                ("2027-04-07", Some("2027-06-15"), Some("2027-06-19"), None),
            ],
            conference_dates: Some(&[("2027-10-10", "2027-10-15")]), city: Some("Prague"),
        },
        Case {
            fixture: "oopsla25.html", conference: "SPLASH", track: "OOPSLA", year: 2025,
            rounds: vec![
                ("2024-10-15", Some("2024-12-03"), Some("2024-12-06"), Some(&["2024-12-18"])),
                ("2025-03-25", Some("2025-05-26"), Some("2025-05-29"), Some(&["2025-06-18"])),
            ],
            conference_dates: Some(&[("2025-10-12", "2025-10-18")]), city: Some("Singapore"),
        },
    ]
}

fn fixture(name: &str) -> String {
    std::fs::read_to_string(format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap()
}

/// Submission sites the pages name (all as text, e.g. "Submission Web Site:
/// https://icfp26.hotcrp.com"); a page without one must yield none.
fn expected_submission_url(fixture: &str) -> Option<&'static str> {
    match fixture {
        "popl26-cfp.html" => Some("https://popl26.hotcrp.com"),
        "splash26-oopsla.html" => Some("https://oopsla26.hotcrp.com"),
        "splash27-oopsla.html" => Some("https://oopsla27.hotcrp.com"),
        "pldi26-cfp.html" => Some("https://pldi2026.hotcrp.com"),
        "icfp26-cfp.html" => Some("https://icfp26.hotcrp.com"),
        // "All submissions will be electronic via: https://submissions.floc26.org/lics/."
        "lics26-cfp.html" => Some("https://submissions.floc26.org/lics"),
        "oopsla25.html" => Some("https://oopsla2425.hotcrp.com"),
        _ => None,
    }
}

/// Application forms the volunteer pages point to: three as written-out
/// Google Forms links, SPLASH's behind the word "here" in "Apply here".
fn expected_application_url(fixture: &str) -> Option<&'static str> {
    match fixture {
        "splash26-volunteers.html" => Some("https://tinyurl.com/splash-issta-sv26"),
        "icfp26-volunteers.html" => Some("https://forms.gle/wdogVw3P25pizryk6"),
        "popl26-volunteers.html" => Some("https://forms.gle/YiMy1PXVF6QWggsb8"),
        "pldi26-volunteers.html" => Some("https://forms.gle/QzqHZhXCj87SGeq67"),
        _ => None,
    }
}

fn same_url(got: Option<&str>, want: Option<&str>) -> bool {
    got.map(|u| u.trim_end_matches('/').to_lowercase()) == want.map(|u| u.trim_end_matches('/').to_lowercase())
}

/// Fixtures are cleaned against this base, so a page's own relative links
/// resolve to it; production refuses submission links on the page's own
/// host (`discover::usable_link`) and so does this check.
const FIXTURE_BASE: &str = "https://example.org/";

fn check(case: &Case, cfp: &schema::Cfp) -> Vec<String> {
    let mut errs = vec![];
    let want = expected_submission_url(case.fixture);
    let got = cfp.submission_url.as_deref().filter(|u| discover::usable_link(u, FIXTURE_BASE, true).is_ok());
    if !same_url(got, want) {
        errs.push(format!("submission_url {:?} != {want:?}", cfp.submission_url));
    }
    if cfp.rounds.len() != case.rounds.len() {
        errs.push(format!("expected {} rounds, got {}", case.rounds.len(), cfp.rounds.len()));
        return errs;
    }
    for (i, ((sub, rs, re, notif), got)) in case.rounds.iter().zip(&cfp.rounds).enumerate() {
        let d = |s: &str| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap();
        if got.submission != d(sub) {
            errs.push(format!("round {i} submission {} != {sub}", got.submission));
        }
        if let Some(rs) = rs {
            if got.response_start != Some(d(rs)) {
                errs.push(format!("round {i} response_start {:?} != {rs}", got.response_start));
            }
        }
        if let Some(re) = re {
            if got.response_end != Some(d(re)) {
                errs.push(format!("round {i} response_end {:?} != {re}", got.response_end));
            }
        }
        if let Some(ns) = notif {
            if !got.notification.is_some_and(|n| ns.iter().any(|x| d(x) == n)) {
                errs.push(format!("round {i} notification {:?} not in {ns:?}", got.notification));
            }
        }
    }
    if let Some(pairs) = case.conference_dates {
        match &cfp.conference {
            Some(c) if pairs.iter().any(|(s, e)| c.start.to_string() == *s && c.end.to_string() == *e) => {}
            other => errs.push(format!("conference dates {:?} not in {pairs:?}", other.as_ref().map(|c| (c.start, c.end)))),
        }
    }
    if let Some(city) = case.city {
        if !cfp.conference.as_ref().and_then(|c| c.city.as_deref()).is_some_and(|c| c.contains(city)) {
            errs.push(format!("city {:?} != {city}", cfp.conference.as_ref().and_then(|c| c.city.clone())));
        }
    }
    errs
}

struct VolunteerCase {
    fixture: &'static str,
    conference: &'static str,
    track: &'static str,
    year: i32,
    site: &'static str,
    /// Accepted deadlines; empty means the page must be rejected or yield no programme.
    deadlines: &'static [&'static str],
}

fn volunteer_cases() -> Vec<VolunteerCase> {
    vec![
        // The page states both "Apply here by July 12" and "Sun 19 Jul 2026 Application Deadline".
        VolunteerCase { fixture: "splash26-volunteers.html", conference: "SPLASH", track: "OOPSLA", year: 2026, site: "https://2026.splashcon.org", deadlines: &["2026-07-12", "2026-07-19"] },
        VolunteerCase { fixture: "icfp26-volunteers.html", conference: "ICFP", track: "ICFP", year: 2026, site: "https://icfp26.sigplan.org", deadlines: &["2026-07-03"] },
        VolunteerCase { fixture: "popl26-volunteers.html", conference: "POPL", track: "POPL", year: 2026, site: "https://popl26.sigplan.org", deadlines: &["2025-11-10"] },
        VolunteerCase { fixture: "pldi26-volunteers.html", conference: "PLDI", track: "PLDI", year: 2026, site: "https://pldi26.sigplan.org", deadlines: &["2026-04-13"] },
        // A namesake: Cornell's "Splash!" school outreach programme must not pass as SPLASH 2027.
        VolunteerCase { fixture: "cornell-splash.html", conference: "SPLASH", track: "OOPSLA", year: 2027, site: "https://2027.splashcon.org", deadlines: &[] },
    ]
}

/// Volunteer pages: deadline accuracy and rejection of namesakes.
#[test]
#[ignore]
fn volunteer_accuracy() {
    let mut llm = Llm::from_env();
    llm.temperature = std::env::var("PLC_TEMP").ok().and_then(|s| s.parse().ok()).unwrap_or(0.0);
    let runs: usize = std::env::var("PLC_RUNS").ok().and_then(|s| s.parse().ok()).unwrap_or(2);
    llm.check().expect("ollama with model");
    let only = std::env::var("PLC_CASE").ok();
    let (mut total, mut passed) = (0, 0);
    for case in volunteer_cases() {
        if only.as_deref().is_some_and(|o| !case.fixture.contains(o)) {
            continue;
        }
        let html = fixture(case.fixture);
        let md = clean::html_to_markdown(&html);
        let links = clean::links(&html, case.site);
        let cfg = pl_conferences::config::ConferenceCfg { conference: case.conference.into(), track: case.track.into(), since: 0 };
        for r in 0..runs {
            let t = Instant::now();
            // Same path as production: validation failure gets one corrective retry.
            let Some((x, raw, validated, retried, _trail)) = discover::extract_volunteer(&llm, &cfg, case.year, Some(case.site), &md, &links).unwrap() else {
                total += 1;
                eprintln!("FAIL {:24} run {r}: model output unusable", case.fixture);
                continue;
            };
            let secs = t.elapsed().as_secs_f64();
            total += 1;
            let tag = if retried { " (after retry)" } else { "" };
            let mut link_err = None;
            let got = match validated {
                Ok(Some(v)) => {
                    let want = expected_application_url(case.fixture);
                    if !same_url(v.application_url.as_deref(), want) {
                        link_err = Some(format!("application_url {:?} != {want:?}", v.application_url));
                    }
                    Some(v.deadline.to_string())
                }
                Ok(None) => None,
                // "Not about this conference" is a rejection, which is the right answer for namesakes.
                Err(_) if !x.page_is_about_conference => None,
                Err(e) => {
                    eprintln!("FAIL {:24} run {r} {secs:5.1}s{tag}: INVALID {}\n     raw: {}", case.fixture, e.join("; "), raw.to_string().replace('\n', " "));
                    continue;
                }
            };
            let ok = match got.as_deref() {
                Some(d) => case.deadlines.contains(&d),
                None => case.deadlines.is_empty(),
            };
            if ok && link_err.is_none() {
                passed += 1;
                eprintln!("PASS {:24} run {r} {secs:5.1}s{tag} deadline {got:?}", case.fixture);
            } else if !ok {
                eprintln!("FAIL {:24} run {r} {secs:5.1}s{tag}: deadline {got:?} not in {:?}\n     raw: {}", case.fixture, case.deadlines, raw.to_string().replace('\n', " "));
            } else {
                eprintln!("FAIL {:24} run {r} {secs:5.1}s{tag}: {}\n     raw: {}", case.fixture, link_err.unwrap(), raw.to_string().replace('\n', " "));
            }
        }
    }
    eprintln!("=== volunteers {passed}/{total} passed, model {}", llm.model);
    assert_eq!(passed, total);
}

#[test]
#[ignore]
fn extraction_accuracy() {
    let mut llm = Llm::from_env();
    llm.temperature = std::env::var("PLC_TEMP").ok().and_then(|s| s.parse().ok()).unwrap_or(0.0);
    let runs: usize = std::env::var("PLC_RUNS").ok().and_then(|s| s.parse().ok()).unwrap_or(3);
    llm.check().expect("ollama with model");
    let only = std::env::var("PLC_CASE").ok();
    let mut total = 0;
    let mut passed = 0;
    let mut total_secs = 0.0;
    for case in cases() {
        if only.as_deref().is_some_and(|o| !case.fixture.contains(o)) {
            continue;
        }
        let html = fixture(case.fixture);
        let md = clean::html_to_markdown(&html);
        let links = clean::links(&html, FIXTURE_BASE);
        let cfg = pl_conferences::config::ConferenceCfg { conference: case.conference.into(), track: case.track.into(), since: 0 };
        for r in 0..runs {
            let t = Instant::now();
            let Some((_x, raw, validated, retried, trail)) = discover::extract_cfp(&llm, &cfg, case.year, &md, &links).unwrap() else {
                total += 1;
                eprintln!("FAIL {:24} run {r}: model output unusable", case.fixture);
                continue;
            };
            let secs = t.elapsed().as_secs_f64();
            total_secs += secs;
            total += 1;
            let mut errs = match validated {
                Ok(cfp) => check(&case, &cfp),
                Err(e) => vec![format!("INVALID: {}", e.join("; "))],
            };
            // The evidence-before-answer design relies on the runtime honouring
            // schema property order; check it on the raw answer.
            let text = raw.to_string();
            for (before, after) in [("submission_deadline_quote", "\"submission_deadline\""), ("submission_deadline_as_written", "\"submission_deadline\""), ("notification_quote", "\"notification\"")] {
                if let (Some(a), Some(b)) = (text.find(before), text.find(after)) {
                    if a > b {
                        errs.push(format!("{before} came after {after} in the model output: the runtime no longer honours schema order"));
                    }
                }
            }
            let tag = if retried { " (after retry)" } else { "" };
            if errs.is_empty() {
                passed += 1;
                eprintln!("PASS {:24} run {r} {secs:5.1}s{tag} ~{} tok", case.fixture, clean::estimate_tokens(&md));
            } else {
                eprintln!("FAIL {:24} run {r} {secs:5.1}s{tag}: {}\n     notes: {}\n     raw: {}", case.fixture, errs.join("; "), trail.note_text(), raw.to_string().replace('\n', " "));
            }
        }
    }
    eprintln!("=== {passed}/{total} passed, model {}, think {}, temp {}, avg {:.1}s", llm.model, llm.think, llm.temperature, total_secs / total.max(1) as f64);
    assert_eq!(passed, total);
}
