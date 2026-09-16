use anyhow::{Context, Result};
use pl_conferences::{clean, config, discover, fetch, ics, llm, schema, search, site, state};
use chrono::{Datelike, Utc};
use discover::{Attempted, Ctx, Miss};
use serde::{Deserialize, Serialize};
use state::{Outcome, State};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// Days before a negative volunteer result (not found / no programme) is retried.
const VOLUNTEER_RETRY_DAYS: i64 = 21;
/// Days before a call for papers that was not found (or did not validate) is looked for again.
const CFP_RETRY_DAYS: i64 = 7;
/// Days a stored record is trusted before its page is re-checked. Discovery
/// of missing data always goes first; re-validation fills the remaining slots.
const REVALIDATE_DAYS: i64 = 14;

const USAGE: &str = "usage: pl-conferences [--root DIR] [--conference NAME]... [--year YYYY] [--dry-run] [--no-volunteer]
       pl-conferences clean <file.html>
       pl-conferences fetch <url>
       pl-conferences extract <file.html> <conference> <year> <track>
       pl-conferences extract-volunteer <file.html> <conference> <year> <track> [site]
       pl-conferences sections <file.html>";

#[derive(Debug, Default)]
struct Args {
    root: PathBuf,
    conferences: Vec<String>,
    year: Option<i32>,
    dry_run: bool,
    no_volunteer: bool,
}

/// Stored as `cfp.json` / `volunteer.json`, next to the fetched page
/// (`cfp.html`) and the Markdown the model saw (`cfp.md`).
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Record<T> {
    conference: String,
    track: String,
    year: i32,
    label: String,
    /// When the current data was extracted.
    fetched_at: chrono::DateTime<Utc>,
    /// Last time a run re-checked the page and found the same data.
    #[serde(default)]
    last_verified: Option<chrono::DateTime<Utc>>,
    #[serde(default)]
    model: String,
    #[serde(default)]
    provenance: discover::Provenance,
    data: T,
    /// Dates that changed between runs, oldest first.
    #[serde(default)]
    history: Vec<schema::Change>,
    #[serde(default)]
    raw: serde_json::Value,
}

fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).format_timestamp_secs().init();
    let argv: Vec<String> = std::env::args().skip(1).collect();
    match argv.first().map(String::as_str) {
        Some("clean") => {
            let html = std::fs::read_to_string(argv.get(1).context(USAGE)?)?;
            print!("{}", clean::html_to_markdown(&html));
            return Ok(());
        }
        Some("sections") => {
            let html = std::fs::read_to_string(argv.get(1).context(USAGE)?)?;
            let md = clean::html_to_markdown_unbudgeted(&html);
            for s in clean::sections(&md) {
                let head = s.lines().next().unwrap_or("").chars().take(60).collect::<String>();
                println!("{:8.2} {:>7}  {head}", clean::score(&s), s.len());
            }
            return Ok(());
        }
        Some("fetch") => {
            let page = fetch::get_rendered(argv.get(1).context(USAGE)?)?;
            eprintln!("final url: {} (chrome: {})", page.url, page.via_chrome);
            print!("{}", clean::html_to_markdown(&page.html));
            return Ok(());
        }
        Some("extract") => {
            let html = std::fs::read_to_string(argv.get(1).context(USAGE)?)?;
            let cfg = config::ConferenceCfg { conference: argv.get(2).context(USAGE)?.clone(), track: argv.get(4).context(USAGE)?.clone(), since: 0 };
            let year: i32 = argv.get(3).context(USAGE)?.parse()?;
            let md = clean::html_to_markdown(&html);
            let llm = llm::Llm::from_env();
            llm.check()?;
            let prompt = discover::cfp_prompt(&cfg, year, "", &md);
            let t = std::time::Instant::now();
            let (x, raw, usage): (schema::CfpExtraction, _, _) = llm.extract(discover::SYSTEM_PROMPT, &prompt).map_err(anyhow::Error::new)?;
            eprintln!("{raw}\n--- {usage:?} wall {:.1}s", t.elapsed().as_secs_f64());
            match schema::validate_cfp(&x, year, &md, &cfg.track) {
                Ok(c) => println!("{}", serde_json::to_string_pretty(&c)?),
                Err(errs) => println!("INVALID: {errs:?}"),
            }
            return Ok(());
        }
        Some("extract-volunteer") => {
            // extract-volunteer <file.html> <conference> <year> <track> [site]
            let html = std::fs::read_to_string(argv.get(1).context(USAGE)?)?;
            let cfg = config::ConferenceCfg { conference: argv.get(2).context(USAGE)?.clone(), track: argv.get(4).context(USAGE)?.clone(), since: 0 };
            let year: i32 = argv.get(3).context(USAGE)?.parse()?;
            let md = clean::html_to_markdown(&html);
            let llm = llm::Llm::from_env();
            llm.check()?;
            let prompt = discover::volunteer_prompt(&cfg, year, argv.get(5).map(String::as_str), "", &md);
            let (x, raw, usage): (schema::VolunteerExtraction, _, _) = llm.extract(discover::SYSTEM_PROMPT, &prompt).map_err(anyhow::Error::new)?;
            eprintln!("{raw}\n--- {usage:?}");
            match schema::validate_volunteer(&x, year, &md, None) {
                Ok(v) => println!("{}", serde_json::to_string_pretty(&v)?),
                Err(errs) => println!("INVALID: {errs:?}"),
            }
            return Ok(());
        }
        _ => {}
    }
    let args = parse_args(&argv)?;
    run(&args)
}

fn parse_args(argv: &[String]) -> Result<Args> {
    let mut a = Args { root: PathBuf::from("."), ..Default::default() };
    let mut it = argv.iter();
    while let Some(x) = it.next() {
        match x.as_str() {
            "--root" => a.root = PathBuf::from(it.next().context(USAGE)?),
            "--conference" | "-c" => a.conferences.push(it.next().context(USAGE)?.clone()),
            "--year" | "-y" => a.year = Some(it.next().context(USAGE)?.parse()?),
            "--dry-run" | "-n" => a.dry_run = true,
            "--no-volunteer" => a.no_volunteer = true,
            "--help" | "-h" => anyhow::bail!("{USAGE}"),
            other => anyhow::bail!("unknown argument {other}\n{USAGE}"),
        }
    }
    Ok(a)
}

fn run(args: &Args) -> Result<()> {
    let root = &args.root;
    let cfgs = config::load(&root.join("conferences.json"))?;
    let state_path = root.join("state.json");
    let mut state = State::load(&state_path)?;
    let llm = llm::Llm::from_env();
    llm.check().context("Ollama is not ready")?;
    let searcher = search::Searcher::new(search::default_backends());
    let now = Utc::now();
    let current_year = now.year();
    let run_no = state.run_count;
    log::info!("run #{} with model {} (think: {}, ctx {})", run_no + 1, llm.model, llm.think, llm.num_ctx);

    // Rotate the order so a slow conference never starves the others.
    let n = cfgs.len().max(1);
    let offset = (run_no as usize) % n;
    let selected: Vec<&config::ConferenceCfg> = cfgs.iter().cycle().skip(offset).take(n).filter(|c| args.conferences.is_empty() || args.conferences.iter().any(|x| x.eq_ignore_ascii_case(&c.conference))).collect();

    let budget = Duration::from_secs(60 * std::env::var("PLC_TIME_BUDGET_MIN").ok().and_then(|s| s.parse().ok()).unwrap_or(75));
    let max_items: usize = std::env::var("PLC_MAX_ITEMS").ok().and_then(|s| s.parse().ok()).unwrap_or(5);
    let started = Instant::now();

    // Work list: every conference-year that still needs something. Only the
    // first `max_items` are attempted; the rest wait for the next run, which
    // keeps every run short. Missing data is looked for before stored data
    // is re-checked, so re-validation of this year's conferences can never
    // crowd out next year's calls for papers.
    let mut items: Vec<(&config::ConferenceCfg, i32, Need)> = vec![];
    for cfg in &selected {
        let years: Vec<i32> = match args.year {
            Some(y) => vec![y],
            None => (cfg.since..=current_year + 1).collect(),
        };
        for year in years {
            if let Some(need) = needs_work(args, root, &state, cfg, year, current_year, now) {
                items.push((cfg, year, need));
            }
        }
    }
    // Discovery before re-validation; within each, this year's editions
    // before next year's, never-attempted first, then least recently
    // attempted, so every pending item gets its turn.
    items.sort_by_key(|(cfg, y, need)| {
        let a = state.get(&format!("{}/cfp", cfg.key(*y)));
        (*need, *y > current_year, a.is_some(), a.map(|a| a.last_attempt))
    });
    let pending = items.len();
    items.truncate(max_items);
    log::info!("{pending} conference-years pending, attempting {} this run (budget {budget:?}): {}", items.len(), items.iter().map(|(c, y, n)| format!("{} ({})", c.key(*y), n.verb())).collect::<Vec<_>>().join(", "));

    let mut consecutive_errors = 0u32;
    let mut fatal: Option<anyhow::Error> = None;
    let mut ctxs: std::collections::HashMap<String, (Ctx, Ctx)> = std::collections::HashMap::new();
    for (cfg, year, _) in items {
        if started.elapsed() > budget {
            log::warn!("time budget of {budget:?} used up; remaining conference-years wait for the next run");
            break;
        }
        let (ctx, vol_ctx) = ctxs.entry(cfg.key(0)).or_insert_with(|| {
            let prior = prior_urls(root, cfg);
            (Ctx { llm: &llm, searcher: &searcher, prior_urls: prior.0 }, Ctx { llm: &llm, searcher: &searcher, prior_urls: prior.1 })
        });
        // A panic in one conference-year (a bug, an unexpected page) must not
        // take the run and its state down with it.
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| process_year(args, ctx, vol_ctx, cfg, year, &mut state, now, current_year)));
        let outcome = match outcome {
            Ok(r) => r,
            Err(p) => {
                let msg = p.downcast_ref::<String>().cloned().or_else(|| p.downcast_ref::<&str>().map(|s| s.to_string())).unwrap_or_else(|| "unknown panic".into());
                Err(anyhow::anyhow!("panic: {msg}"))
            }
        };
        match outcome {
            Ok(()) => consecutive_errors = 0,
            Err(e) => {
                consecutive_errors += 1;
                log::error!("{}: error: {e:#}", cfg.key(year));
                state.record(&format!("{}/cfp", cfg.key(year)), Outcome::Error, None, vec![], format!("{e:#}"), now, year <= current_year);
                if consecutive_errors >= 3 {
                    fatal = Some(e.context("three conference-years in a row failed with errors; giving up on this run"));
                    break;
                }
            }
        }
        llm.unload();
    }
    log::info!("run took {:.0?}", started.elapsed());

    state.last_run = Some(now);
    state.run_count += 1;
    if let Ok(v) = std::env::var("PLC_WORKFLOW_CODES") {
        state.last_run_codes = v.split(',').filter_map(|c| serde_json::from_value(serde_json::Value::String(c.trim().to_string())).ok()).collect();
    } else {
        state.last_run_codes.clear();
    }
    if args.dry_run {
        log::info!("dry run: not writing state, calendars or docs");
        return Ok(());
    }
    state.save(&state_path)?;
    regenerate_outputs(root, &state)?;
    match fatal {
        Some(e) => Err(e),
        None => Ok(()),
    }
}

/// What a run does for a conference-year: look for data that is missing, or
/// re-check stored data whose page has not been looked at for a while.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Need {
    Discover,
    Revalidate,
}

impl Need {
    fn verb(self) -> &'static str {
        match self {
            Need::Discover => "searching",
            Need::Revalidate => "re-validating",
        }
    }
}

/// Why a conference-year (or one of its parts) is left alone this run.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Idle {
    /// Over, or its deadline has passed: never touched again.
    Archived,
    /// Never found and its year is in the past.
    Abandoned,
    /// Recently attempted or recently verified.
    Waiting(String),
}

impl std::fmt::Display for Idle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Idle::Archived => write!(f, "archived"),
            Idle::Abandoned => write!(f, "abandoned"),
            Idle::Waiting(why) => write!(f, "{why}"),
        }
    }
}

/// Whether a conference-year has something to attempt this run. The same
/// per-part rules decide what the run then does, so the work list and the
/// run never disagree. A year with nothing stored is listed for its call for
/// papers only: the volunteer search starts from that page.
fn needs_work(args: &Args, root: &Path, state: &State, cfg: &config::ConferenceCfg, year: i32, current_year: i32, now: chrono::DateTime<Utc>) -> Option<Need> {
    let key = cfg.key(year);
    let dir = root.join("conferences").join(&key);
    let conf = read_record::<schema::Conference>(&dir.join("conference.json")).ok();
    let dl = read_record::<schema::Deadlines>(&dir.join("cfp.json")).ok();
    let vol = read_record::<schema::Volunteer>(&dir.join("volunteer.json")).ok();
    let stage = schema::stage(conf.as_ref().map(|r| &r.data), dl.as_ref().map(|r| &r.data), now.date_naive());
    let cfp = cfp_need(state, &format!("{key}/cfp"), conf.as_ref(), dl.as_ref(), year, current_year, now).ok();
    let vol = if args.no_volunteer || (conf.is_none() && dl.is_none()) {
        None
    } else {
        volunteer_need(state, &format!("{key}/volunteer"), vol.as_ref(), stage, year, current_year, now).ok()
    };
    cfp.into_iter().chain(vol).min()
}

/// Conference dates and deadlines: discovery while a part is missing (not
/// retried within `CFP_RETRY_DAYS` of a failed search), re-validation once a
/// stored part is older than `REVALIDATE_DAYS`; archived when the conference
/// is over (dates) or the last rebuttal has ended (deadlines).
fn cfp_need(state: &State, key: &str, conf: Option<&Record<schema::Conference>>, dl: Option<&Record<schema::Deadlines>>, year: i32, current_year: i32, now: chrono::DateTime<Utc>) -> Result<Need, Idle> {
    let today = now.date_naive();
    let stage = schema::stage(conf.map(|r| &r.data), dl.map(|r| &r.data), today);
    let conf_active = schema::conference_active(conf.map(|r| &r.data), today) && stage != schema::Stage::Happened;
    let dl_active = schema::deadlines_active(dl.map(|r| &r.data), today) && stage != schema::Stage::Happened;
    if !conf_active && !dl_active {
        return Err(Idle::Archived);
    }
    if conf.is_none() && dl.is_none() && skip_reason(state, key, year, current_year).is_some() {
        return Err(Idle::Abandoned);
    }
    let missing = (conf_active && conf.is_none()) || (dl_active && dl.is_none());
    let due = (conf_active && conf.is_some_and(|r| stale(r, now))) || (dl_active && dl.is_some_and(|r| stale(r, now)));
    if missing {
        return match recent_negative(state, key, now, CFP_RETRY_DAYS) {
            Some(wait) if !due => Err(wait),
            _ => Ok(Need::Discover),
        };
    }
    if due {
        Ok(Need::Revalidate)
    } else {
        Err(Idle::Waiting(format!("verified within the last {REVALIDATE_DAYS} days")))
    }
}

/// Volunteer applications open late (after rebuttals), so this part stays
/// active until the conference has happened or its own deadline has passed.
fn volunteer_need(state: &State, key: &str, vol: Option<&Record<schema::Volunteer>>, stage: schema::Stage, year: i32, current_year: i32, now: chrono::DateTime<Utc>) -> Result<Need, Idle> {
    if stage == schema::Stage::Happened {
        return Err(Idle::Archived);
    }
    match vol {
        Some(r) if now.date_naive() > r.data.deadline => Err(Idle::Archived),
        Some(r) if stale(r, now) => Ok(Need::Revalidate),
        Some(_) => Err(Idle::Waiting(format!("verified within the last {REVALIDATE_DAYS} days"))),
        None if skip_reason(state, key, year, current_year).is_some() => Err(Idle::Abandoned),
        None => match recent_negative(state, key, now, VOLUNTEER_RETRY_DAYS) {
            Some(wait) => Err(wait),
            None => Ok(Need::Discover),
        },
    }
}

fn stale<T>(r: &Record<T>, now: chrono::DateTime<Utc>) -> bool {
    now - r.last_verified.unwrap_or(r.fetched_at) >= chrono::Duration::days(REVALIDATE_DAYS)
}

/// A failed search (not found, invalid, no programme) is not repeated for
/// `days`; fetch failures and errors are retried on the next run.
fn recent_negative(state: &State, key: &str, now: chrono::DateTime<Utc>, days: i64) -> Option<Idle> {
    let a = state.get(key)?;
    let age = now - a.last_attempt;
    let negative = matches!(a.outcome, Outcome::NotFound | Outcome::Invalid | Outcome::NoVolunteerProgram);
    (negative && age < chrono::Duration::days(days)).then(|| Idle::Waiting(format!("{} {} days ago; retried after {days} days", format!("{:?}", a.outcome).to_lowercase(), age.num_days())))
}

/// One conference-year: call for papers, then student volunteers. Stored
/// data is re-validated from its source page; differences are kept as
/// history and noted in the calendar.
#[allow(clippy::too_many_arguments)]
fn process_year(args: &Args, ctx: &Ctx, vol_ctx: &Ctx, cfg: &config::ConferenceCfg, year: i32, state: &mut State, now: chrono::DateTime<Utc>, current_year: i32) -> Result<()> {
    let root = &args.root;
    let today = now.date_naive();
    let key = cfg.key(year);
    let dir = root.join("conferences").join(&key);
    let cfp_key = format!("{key}/cfp");
    let vol_key = format!("{key}/volunteer");
    let overdue = year <= current_year;
    let model = &ctx.llm.model;

    // --- Conference dates and paper deadlines (one extraction, two records) ---
    let conf_rec = read_record::<schema::Conference>(&dir.join("conference.json")).ok();
    let dl_rec = read_record::<schema::Deadlines>(&dir.join("cfp.json")).ok();
    let have_any = conf_rec.is_some() || dl_rec.is_some();
    let mut stage = schema::stage(conf_rec.as_ref().map(|r| &r.data), dl_rec.as_ref().map(|r| &r.data), today);
    let conf_active = schema::conference_active(conf_rec.as_ref().map(|r| &r.data), today) && stage != schema::Stage::Happened;
    let dl_active = schema::deadlines_active(dl_rec.as_ref().map(|r| &r.data), today) && stage != schema::Stage::Happened;
    // The deadlines' source page, else the conference dates' source page.
    let mut cfp_url: Option<String> = dl_rec.as_ref().map(|r| r.provenance.source_url.clone()).or_else(|| conf_rec.as_ref().map(|r| r.provenance.source_url.clone()));
    match cfp_need(state, &cfp_key, conf_rec.as_ref(), dl_rec.as_ref(), year, current_year, now) {
        Err(idle) => {
            log::info!("{cfp_key}: {idle} ({})", stage.label());
            if idle == Idle::Abandoned && state.get(&cfp_key).map(|a| a.outcome) != Some(Outcome::Abandoned) {
                state.record(&cfp_key, Outcome::Abandoned, None, vec![], "year is in the past".into(), now, overdue);
            }
        }
        Ok(need) => {
        let wanted = match (conf_active, dl_active) {
            (true, true) => "conference dates and deadlines",
            (true, false) => "conference dates",
            _ => "deadlines",
        };
        log::info!("=== {cfp_key}: {} {wanted} ({})", need.verb(), stage.label());
        let t = Instant::now();
        let att = discover::find_cfp(ctx, cfg, year, cfp_url.as_deref())?;
        let secs = t.elapsed().as_secs();
        let mut got_something = false;
        let outcome = match &att.result {
            Ok(found) => {
                if let (true, Some(f)) = (conf_active, &found.conference) {
                    log::info!("{cfp_key}: conference {}", serde_json::to_string(&f.value)?);
                    let rec = merge_record(conf_rec.clone(), cfg, year, f, model, now, |o, n| schema::diff_conference(Some(o), Some(n), now));
                    if !args.dry_run {
                        write_part(&dir, "conference", &rec, f, conf_rec.is_none() || rec.fetched_at == now, |r, p| ics::conference_events(&r.label, &cfg.key(year), &r.data, p, r.fetched_at))?;
                    }
                    got_something = true;
                }
                if let (true, Some(f)) = (dl_active, &found.deadlines) {
                    log::info!("{cfp_key}: deadlines {}", serde_json::to_string(&f.value)?);
                    let rec = merge_record(dl_rec.clone(), cfg, year, f, model, now, |o, n| schema::diff_deadlines(o, n, now));
                    cfp_url = Some(rec.provenance.source_url.clone());
                    if !args.dry_run {
                        write_part(&dir, "cfp", &rec, f, dl_rec.is_none() || rec.fetched_at == now, |r, p| ics::deadline_events(&r.label, &cfg.key(year), &r.data, p, r.fetched_at))?;
                    }
                    got_something = true;
                }
                if found.conference.is_none() && conf_active && conf_rec.is_none() {
                    log::info!("{cfp_key}: conference dates not found yet");
                }
                if found.deadlines.is_none() && dl_active && dl_rec.is_none() {
                    log::info!("{cfp_key}: deadlines not found yet");
                }
                if got_something { Outcome::Ok } else { Outcome::NotFound }
            }
            Err(Miss::Invalid) => Outcome::Invalid,
            Err(_) => Outcome::NotFound,
        };
        if have_any && outcome != Outcome::Ok {
            log::warn!("{cfp_key}: re-validation found nothing new ({outcome:?}); keeping the stored data");
        }
        log_attempt(&cfp_key, &att, outcome, secs);
        state.record(&cfp_key, outcome, att.trail.backend.clone(), att.trail.codes.clone(), att.trail.note_text(), now, overdue);
        let conf_now = read_record::<schema::Conference>(&dir.join("conference.json")).ok();
        let dl_now = read_record::<schema::Deadlines>(&dir.join("cfp.json")).ok();
        stage = schema::stage(conf_now.as_ref().map(|r| &r.data), dl_now.as_ref().map(|r| &r.data), today);
        }
    }

    // --- Student volunteers ---
    if args.no_volunteer {
        return Ok(());
    }
    let existing_vol = read_record::<schema::Volunteer>(&dir.join("volunteer.json")).ok();
    let need = match volunteer_need(state, &vol_key, existing_vol.as_ref(), stage, year, current_year, now) {
        Ok(need) => need,
        Err(idle) => {
            log::info!("{vol_key}: {idle}");
            if idle == Idle::Abandoned && state.get(&vol_key).map(|a| a.outcome) != Some(Outcome::Abandoned) {
                state.record(&vol_key, Outcome::Abandoned, None, vec![], "year is in the past".into(), now, overdue);
            }
            return Ok(());
        }
    };
    let Some(cfp_url) = cfp_url else {
        log::info!("{vol_key}: skipped (no call for papers found yet)");
        return Ok(());
    };
    log::info!("=== {vol_key}: {}", need.verb());
    let t = Instant::now();
    let known = existing_vol.as_ref().map(|r| r.provenance.source_url.clone());
    let conference_end = read_record::<schema::Conference>(&dir.join("conference.json")).ok().map(|r| r.data.end);
    let site = own_site(root, cfg, year);
    let att = discover::find_volunteer(vol_ctx, cfg, year, known.as_deref(), Some(&cfp_url), conference_end, site.as_deref())?;
    let secs = t.elapsed().as_secs();
    let outcome = match &att.result {
        Ok(found) => {
            log::info!("{vol_key}: {}", serde_json::to_string(&found.value)?);
            let rec = merge_record(existing_vol.clone(), cfg, year, found, model, now, |old, new| {
                if old.deadline != new.deadline {
                    vec![schema::Change { at: now, field: "volunteer deadline".into(), old: old.deadline.to_string(), new: new.deadline.to_string() }]
                } else {
                    vec![]
                }
            });
            if !args.dry_run {
                write_volunteer(&dir, cfg, year, &rec, found, existing_vol.is_none() || rec.fetched_at == now)?;
            }
            Outcome::Ok
        }
        Err(Miss::Invalid) => Outcome::Invalid,
        Err(Miss::NoProgram) => Outcome::NoVolunteerProgram,
        Err(Miss::NotFound) => Outcome::NotFound,
    };
    if existing_vol.is_some() && outcome != Outcome::Ok {
        log::warn!("{vol_key}: re-validation failed ({outcome:?}); keeping the stored data");
    }
    log_attempt(&vol_key, &att, outcome, secs);
    state.record(&vol_key, outcome, att.trail.backend.clone(), att.trail.codes.clone(), att.trail.note_text(), now, overdue);
    Ok(())
}

/// Fields that may be refreshed without counting as a change (links).
trait SoftUpdate {
    fn soft_update(&mut self, from: &Self);
}
impl SoftUpdate for schema::Deadlines {
    fn soft_update(&mut self, from: &Self) {
        // Always take the fresh value: a wrong link must be replaceable, and
        // a link that vanished from the page should not linger.
        self.submission_url = from.submission_url.clone();
    }
}
impl SoftUpdate for schema::Conference {
    fn soft_update(&mut self, from: &Self) {
        // Same dates and city (see `diff_conference`): take the fresh wording.
        self.city = from.city.clone();
        self.country = from.country.clone();
    }
}
impl SoftUpdate for schema::Volunteer {
    fn soft_update(&mut self, from: &Self) {
        self.application_url = from.application_url.clone();
    }
}

/// Combine a fresh extraction with the stored record: unchanged data only
/// bumps `last_verified`; changed data replaces it and extends the history.
fn merge_record<T: Clone + Serialize + SoftUpdate>(existing: Option<Record<T>>, cfg: &config::ConferenceCfg, year: i32, found: &discover::Found<T>, model: &str, now: chrono::DateTime<Utc>, diff: impl Fn(&T, &T) -> Vec<schema::Change>) -> Record<T> {
    match existing {
        Some(mut old) => {
            let changes = diff(&old.data, &found.value);
            old.last_verified = Some(now);
            if changes.is_empty() {
                log::info!("{}: unchanged", cfg.key(year));
                old.data.soft_update(&found.value);
            } else {
                for c in &changes {
                    log::warn!("{}: {} changed: {} -> {}", cfg.key(year), c.field, c.old, c.new);
                }
                old.history.extend(changes);
                old.data = found.value.clone();
                old.raw = found.raw.clone();
                old.provenance = found.prov.clone();
                old.fetched_at = now;
                old.model = model.into();
            }
            old
        }
        None => Record { conference: cfg.conference.clone(), track: cfg.track.clone(), year, label: cfg.label(year), fetched_at: now, last_verified: Some(now), model: model.into(), provenance: found.prov.clone(), data: found.value.clone(), history: vec![], raw: found.raw.clone() },
    }
}

/// Source URLs of earlier editions on disk: (cfp, volunteer) lists of (year, url).
fn prior_urls(root: &Path, cfg: &config::ConferenceCfg) -> (Vec<(i32, String)>, Vec<(i32, String)>) {
    let mut cfp = vec![];
    let mut vol = vec![];
    let base = root.join("conferences").join(&cfg.conference).join(&cfg.track);
    let Ok(rd) = std::fs::read_dir(&base) else { return (cfp, vol) };
    for e in rd.flatten() {
        let Ok(year) = e.file_name().to_string_lossy().parse::<i32>() else { continue };
        if let Ok(r) = read_record::<schema::Deadlines>(&e.path().join("cfp.json")) {
            cfp.push((year, r.provenance.source_url));
        }
        if let Ok(r) = read_record::<schema::Conference>(&e.path().join("conference.json")) {
            cfp.push((year, r.provenance.source_url));
        }
        if let Ok(r) = read_record::<schema::Volunteer>(&e.path().join("volunteer.json")) {
            vol.push((year, r.provenance.source_url));
        }
    }
    (cfp, vol)
}

/// The conference's own website for `year`, derived from the data: a host
/// that this conference's stored pages use and no other conference's do. A
/// host shared between conferences (conf.researchr.org hosts hundreds) is
/// never "own", so it cannot mislabel the real pages as foreign. Newest
/// year first; a host that carries its edition's year (2026.splashcon.org,
/// popl26.sigplan.org) is moved to `year`.
fn own_site(root: &Path, cfg: &config::ConferenceCfg, year: i32) -> Option<String> {
    let mut hosts: std::collections::HashMap<String, std::collections::HashSet<String>> = Default::default();
    let mut mine: Vec<(i32, String)> = vec![];
    let conf_dir = root.join("conferences");
    for entry in walk(&conf_dir).unwrap_or_default() {
        let name = entry.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        if !matches!(name.as_str(), "conference.json" | "cfp.json" | "volunteer.json") {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&entry) else { continue };
        let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else { continue };
        let (Some(conference), Some(url)) = (v["conference"].as_str(), v["provenance"]["source_url"].as_str()) else { continue };
        let Some(site) = discover::site_of(url) else { continue };
        hosts.entry(site.clone()).or_default().insert(conference.to_string());
        if conference == cfg.conference {
            mine.push((v["year"].as_i64().unwrap_or(0) as i32, site));
        }
    }
    mine.sort_by_key(|(y, _)| std::cmp::Reverse(*y));
    let (y, site) = mine.into_iter().find(|(_, s)| hosts.get(s).is_some_and(|c| c.len() == 1))?;
    let moved = discover::derived_urls(&[(y, site.clone())], year).into_iter().next().unwrap_or(site);
    Some(moved.trim_end_matches('/').to_string())
}

/// Why a conference-year without data is not attempted: abandoned once its
/// year is in the past.
fn skip_reason(state: &State, key: &str, year: i32, current_year: i32) -> Option<&'static str> {
    match state.get(key) {
        Some(a) if a.outcome == Outcome::Abandoned => Some("abandoned"),
        _ if year < current_year => Some("abandoned"),
        _ => None,
    }
}

fn log_attempt<T: std::fmt::Debug>(key: &str, att: &Attempted<T>, outcome: Outcome, secs: u64) {
    log::info!(
        "{key}: {outcome:?} in {secs}s ({} pages, {} llm calls, backend {:?}, codes {:?}) {}",
        att.trail.pages, att.trail.llm_calls, att.trail.backend, att.trail.codes, att.trail.note_text()
    );
}

fn read_or_skip<T: serde::de::DeserializeOwned>(path: &Path) -> Option<Record<T>> {
    match read_record::<T>(path) {
        Ok(r) => Some(r),
        Err(e) => {
            log::error!("skipping unreadable {}: {e:#}", path.display());
            None
        }
    }
}

fn read_record<T: serde::de::DeserializeOwned>(path: &Path) -> Result<Record<T>> {
    let text = std::fs::read_to_string(path)?;
    serde_json::from_str(&text).with_context(|| format!("parsing {}", path.display()))
}

fn write_json<T: Serialize>(path: &Path, v: &T) -> Result<()> {
    std::fs::create_dir_all(path.parent().unwrap())?;
    std::fs::write(path, serde_json::to_string_pretty(v)? + "\n")?;
    Ok(())
}

fn provenance_of(p: &discover::Provenance, history: &[schema::Change]) -> ics::Provenance {
    ics::Provenance { source_url: p.source_url.clone(), codes: p.codes.iter().map(|c| format!("{c:?}")).collect(), history: history.to_vec() }
}

/// Write one record (`conference` or `cfp`), its calendar and (when the
/// data is new) the page it came from. Pages are not rewritten for
/// unchanged data, so weekly re-validation does not churn the repository.
fn write_part<T: Serialize + Clone>(dir: &Path, name: &str, rec: &Record<T>, found: &discover::Found<T>, write_pages: bool, events: impl Fn(&Record<T>, &ics::Provenance) -> Vec<ics::Event>) -> Result<()> {
    let rec = rec.clone();
    write_json(&dir.join(format!("{name}.json")), &rec)?;
    if write_pages || !dir.join(format!("{name}.html")).exists() {
        std::fs::write(dir.join(format!("{name}.html")), &found.html)?;
        std::fs::write(dir.join(format!("{name}.md")), &found.md)?;
    }
    let ev = events(&rec, &provenance_of(&rec.provenance, &rec.history));
    std::fs::write(dir.join(format!("{name}.ics")), ics::calendar(&format!("{} {name}", rec.label), &ev))?;
    log::info!("wrote {} ({} events)", dir.join(format!("{name}.ics")).display(), ev.len());
    Ok(())
}

fn write_volunteer(dir: &Path, cfg: &config::ConferenceCfg, year: i32, rec: &Record<schema::Volunteer>, found: &discover::Found<schema::Volunteer>, write_pages: bool) -> Result<()> {
    write_json(&dir.join("volunteer.json"), rec)?;
    if write_pages || !dir.join("volunteer.html").exists() {
        std::fs::write(dir.join("volunteer.html"), &found.html)?;
        std::fs::write(dir.join("volunteer.md"), &found.md)?;
    }
    let events = ics::volunteer_events(&rec.label, &cfg.key(year), &rec.data, &provenance_of(&rec.provenance, &rec.history), rec.fetched_at);
    std::fs::write(dir.join("volunteer.ics"), ics::calendar(&format!("{} volunteers", rec.label), &events))?;
    log::info!("wrote {}", dir.join("volunteer.ics").display());
    Ok(())
}

/// Rebuild `all.ics`, the README tables and `MAINTENANCE.md` from the JSON on disk.
fn regenerate_outputs(root: &Path, state: &State) -> Result<()> {
    let mut events = vec![];
    let mut rows: Vec<(chrono::NaiveDate, String, String)> = vec![];
    let now = Utc::now();
    let today = now.date_naive();
    let conf_dir = root.join("conferences");
    // One row per conference-year on the site, assembled from all three records.
    type Parts = (String, Option<Record<schema::Conference>>, Option<Record<schema::Deadlines>>, Option<Record<schema::Volunteer>>);
    let mut by_year: std::collections::BTreeMap<String, Parts> = Default::default();
    if conf_dir.exists() {
        for entry in walk(&conf_dir)? {
            let name = entry.file_name().unwrap().to_string_lossy().to_string();
            if name == "conference.json" {
                let Some(r) = read_or_skip::<schema::Conference>(&entry) else { continue };
                let key = format!("{}/{}/{}", r.conference, r.track, r.year);
                let ev = ics::conference_events(&r.label, &key, &r.data, &provenance_of(&r.provenance, &r.history), r.fetched_at);
                rows.push((r.data.start, r.label.clone(), "Conference".into()));
                // The calendar next to the JSON is derived from it, so it is
                // rebuilt here too and never drifts from an edited record.
                std::fs::write(entry.with_file_name("conference.ics"), ics::calendar(&format!("{} conference", r.label), &ev))?;
                events.extend(ev);
                let label = r.label.clone();
                by_year.entry(key).or_insert_with(|| (label, None, None, None)).1 = Some(r);
            } else if name == "cfp.json" {
                let Some(r) = read_or_skip::<schema::Deadlines>(&entry) else { continue };
                let key = format!("{}/{}/{}", r.conference, r.track, r.year);
                let ev = ics::deadline_events(&r.label, &key, &r.data, &provenance_of(&r.provenance, &r.history), r.fetched_at);
                for e in &ev {
                    rows.push((e.start, r.label.clone(), e.summary.trim_start_matches(&format!("[{}] ", r.label)).to_string()));
                }
                std::fs::write(entry.with_file_name("cfp.ics"), ics::calendar(&format!("{} cfp", r.label), &ev))?;
                events.extend(ev);
                let label = r.label.clone();
                by_year.entry(key).or_insert_with(|| (label, None, None, None)).2 = Some(r);
            } else if name == "volunteer.json" {
                let Some(r) = read_or_skip::<schema::Volunteer>(&entry) else { continue };
                let key = format!("{}/{}/{}", r.conference, r.track, r.year);
                let ev = ics::volunteer_events(&r.label, &key, &r.data, &provenance_of(&r.provenance, &r.history), r.fetched_at);
                for e in &ev {
                    rows.push((e.start, r.label.clone(), "Volunteer Application Deadline".into()));
                }
                std::fs::write(entry.with_file_name("volunteer.ics"), ics::calendar(&format!("{} volunteers", r.label), &ev))?;
                events.extend(ev);
                let label = r.label.clone();
                by_year.entry(key).or_insert_with(|| (label, None, None, None)).3 = Some(r);
            }
        }
    }
    let views: Vec<site::YearView> = by_year
        .into_iter()
        .map(|(key, (label, conf, dl, vol))| {
            let stage = schema::stage(conf.as_ref().map(|r| &r.data), dl.as_ref().map(|r| &r.data), today);
            let last_verified = [conf.as_ref().map(|r| r.last_verified.unwrap_or(r.fetched_at)), dl.as_ref().map(|r| r.last_verified.unwrap_or(r.fetched_at))].into_iter().flatten().max();
            site::YearView {
                key,
                label,
                stage,
                conference: conf.map(|r| (r.data, r.provenance.source_url)),
                deadlines: dl.map(|r| (r.data, r.provenance.source_url)),
                volunteer: vol.map(|r| (r.data, r.provenance.source_url)),
                last_verified,
            }
        })
        .collect();
    let calendar = ics::calendar("PL conference deadlines", &events);
    std::fs::write(root.join("all.ics"), &calendar)?;
    std::fs::write(root.join("MAINTENANCE.md"), state.render_maintenance())?;

    // The site: docs/index.html plus the calendar at a Pages URL.
    rows.sort();
    let upcoming: Vec<_> = rows.iter().filter(|(d, _, _)| *d >= today).cloned().collect();
    let docs = root.join("docs");
    std::fs::create_dir_all(&docs)?;
    std::fs::write(docs.join("index.html"), site::render(&views, &upcoming, &state.summary_line(), now))?;
    std::fs::write(docs.join("all.ics"), &calendar)?;
    std::fs::write(docs.join(".nojekyll"), "")?;

    // README: only the maintenance line is generated.
    let readme_path = root.join("README.md");
    // A missing README is bootstrapped; any other read error must not end
    // in overwriting the file with just the generated blocks.
    let readme = match std::fs::read_to_string(&readme_path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(anyhow::Error::new(e).context("reading README.md")),
    };
    let readme = state::replace_marked(&readme, "maintenance", &state.summary_line());
    std::fs::write(&readme_path, readme)?;
    log::info!("wrote all.ics ({} events), docs/index.html ({} conference-years), README.md, MAINTENANCE.md", events.len(), views.len());
    Ok(())
}

fn walk(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut out = vec![];
    let mut entries: Vec<_> = std::fs::read_dir(dir)?.filter_map(|e| e.ok()).map(|e| e.path()).collect();
    entries.sort();
    for p in entries {
        if p.is_dir() {
            out.extend(walk(&p)?);
        } else {
            out.push(p);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn cfp(sub: &str) -> schema::Deadlines {
        schema::Deadlines {
            rounds: vec![schema::ValidRound { label: String::new(), submission: NaiveDate::parse_from_str(sub, "%Y-%m-%d").unwrap(), submission_conflict: None, response_start: None, response_end: None, notification: None }],
            submission_details: "prose".into(),
            submission_url: None,
        }
    }

    fn found(sub: &str) -> discover::Found<schema::Deadlines> {
        discover::Found { value: cfp(sub), raw: serde_json::Value::Null, prov: discover::Provenance { source_url: "https://x.org/new".into(), ..Default::default() }, html: String::new(), md: String::new() }
    }

    #[test]
    fn own_site_ignores_hosts_shared_between_conferences() {
        let root = std::env::temp_dir().join(format!("plc-own-site-{}", std::process::id()));
        let write = |conf: &str, year: i32, kind: &str, url: &str| {
            let dir = root.join("conferences").join(conf).join(conf).join(year.to_string());
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join(format!("{kind}.json")), format!(r#"{{"conference":"{conf}","track":"{conf}","year":{year},"label":"x","fetched_at":"2026-01-01T00:00:00Z","provenance":{{"source_url":"{url}"}},"data":{{}}}}"#)).unwrap();
        };
        write("POPL", 2026, "cfp", "https://conf.researchr.org/track/POPL-2026/x");
        write("POPL", 2026, "volunteer", "https://popl26.sigplan.org/track/sv");
        write("POPL", 2025, "cfp", "https://popl25.sigplan.org/track/x");
        write("SPLASH", 2027, "cfp", "https://conf.researchr.org/track/splash-2027/x");
        let popl = config::ConferenceCfg { conference: "POPL".into(), track: "POPL".into(), since: 2025 };
        let splash = config::ConferenceCfg { conference: "SPLASH".into(), track: "OOPSLA".into(), since: 2027 };
        assert_eq!(own_site(&root, &popl, 2026).as_deref(), Some("https://popl26.sigplan.org"));
        assert_eq!(own_site(&root, &popl, 2027).as_deref(), Some("https://popl27.sigplan.org"), "the host follows the edition's year");
        assert_eq!(own_site(&root, &splash, 2027), None, "only a shared host is known");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn merge_keeps_unchanged_data_and_records_changes() {
        let cfg = config::ConferenceCfg { conference: "X".into(), track: "X".into(), since: 2027 };
        let t0 = Utc::now() - chrono::Duration::days(7);
        let t1 = Utc::now();
        let first = merge_record(None, &cfg, 2027, &found("2026-07-01"), "m", t0, |a, b| schema::diff_deadlines(a, b, t0));
        assert!(first.history.is_empty() && first.last_verified == Some(t0));

        // Same dates, different prose: only last_verified moves.
        let mut same = found("2026-07-01");
        same.value.submission_details = "other prose".into();
        same.prov.source_url = "https://x.org/other".into();
        let second = merge_record(Some(first.clone()), &cfg, 2027, &same, "m", t1, |a, b| schema::diff_deadlines(a, b, t1));
        assert!(second.history.is_empty());
        assert_eq!(second.last_verified, Some(t1));
        assert_eq!(second.fetched_at, t0, "unchanged data keeps its original fetch time");
        assert_eq!(second.provenance.source_url, "https://x.org/new");
        assert_eq!(second.data.submission_details, "prose");

        // A moved deadline replaces the data and is remembered.
        let third = merge_record(Some(second), &cfg, 2027, &found("2026-07-08"), "m", t1, |a, b| schema::diff_deadlines(a, b, t1));
        assert_eq!(third.history.len(), 1);
        assert_eq!((third.history[0].field.as_str(), third.history[0].old.as_str(), third.history[0].new.as_str()), ("round 1 submission", "2026-07-01", "2026-07-08"));
        assert_eq!(third.data.rounds[0].submission.to_string(), "2026-07-08");
        assert_eq!(third.fetched_at, t1);
    }
}
