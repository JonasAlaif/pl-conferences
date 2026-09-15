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

fn check(case: &Case, cfp: &schema::Cfp) -> Vec<String> {
    let mut errs = vec![];
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
        let md = clean::html_to_markdown(&fixture(case.fixture));
        let cfg = pl_conferences::config::ConferenceCfg { conference: case.conference.into(), track: case.track.into(), since: 0 };
        for r in 0..runs {
            let t = Instant::now();
            let Some((_x, raw, validated, retried, trail)) = discover::extract_cfp(&llm, &cfg, case.year, &md).unwrap() else {
                total += 1;
                eprintln!("FAIL {:24} run {r}: model output unusable", case.fixture);
                continue;
            };
            let secs = t.elapsed().as_secs_f64();
            total_secs += secs;
            total += 1;
            let errs = match validated {
                Ok(cfp) => check(&case, &cfp),
                Err(e) => vec![format!("INVALID: {}", e.join("; "))],
            };
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
