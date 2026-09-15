use anyhow::{Context, Result};
use pl_conferences::{clean, config, discover, fetch, ics, llm, schema, search, state};
use chrono::{Datelike, Utc};
use discover::{Attempted, Ctx, Miss};
use serde::{Deserialize, Serialize};
use state::{Outcome, State};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// Days before a negative volunteer result (not found / no programme) is retried.
const VOLUNTEER_RETRY_DAYS: i64 = 21;

const USAGE: &str = "usage: pl-conferences [--root DIR] [--conference NAME]... [--year YYYY] [--dry-run] [--no-volunteer]
       pl-conferences clean <file.html>
       pl-conferences fetch <url>
       pl-conferences extract <file.html> <conference> <year> <track>
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
    /// Informational; recomputed on every write.
    #[serde(default)]
    stage: Option<schema::Stage>,
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
            let prompt = discover::cfp_prompt(&cfg, year, &md);
            let t = std::time::Instant::now();
            let (x, raw, usage): (schema::CfpExtraction, _, _) = llm.extract(discover::SYSTEM_PROMPT, &prompt).map_err(anyhow::Error::new)?;
            eprintln!("{raw}\n--- {usage:?} wall {:.1}s", t.elapsed().as_secs_f64());
            match schema::validate_cfp(&x, year, &md) {
                Ok(c) => println!("{}", serde_json::to_string_pretty(&c)?),
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

    // Work list: every conference-year that still needs something, current
    // and past years before next year, conferences rotated between runs.
    // Only the first `max_items` are attempted; the rest wait for the next
    // (weekly) run, which keeps every run short.
    let mut items: Vec<(&config::ConferenceCfg, i32)> = vec![];
    for cfg in &selected {
        let years: Vec<i32> = match args.year {
            Some(y) => vec![y],
            None => (cfg.since..=current_year + 1).collect(),
        };
        for year in years {
            if needs_work(args, root, &state, cfg, year, current_year) {
                items.push((cfg, year));
            }
        }
    }
    // Never-attempted first, then least recently attempted; next year's
    // editions after this year's.
    items.sort_by_key(|(cfg, y)| {
        let a = state.get(&format!("{}/cfp", cfg.key(*y)));
        (*y > current_year, a.is_some(), a.map(|a| a.last_attempt))
    });
    let pending = items.len();
    items.truncate(max_items);
    log::info!("{pending} conference-years pending, attempting {} this run (budget {budget:?})", items.len());

    let mut consecutive_errors = 0u32;
    let mut fatal: Option<anyhow::Error> = None;
    let mut ctxs: std::collections::HashMap<String, (Ctx, Ctx)> = std::collections::HashMap::new();
    for (cfg, year) in items {
        if started.elapsed() > budget {
            log::warn!("time budget of {budget:?} used up; remaining conference-years wait for the next run");
            break;
        }
        let (ctx, vol_ctx) = ctxs.entry(cfg.key(0)).or_insert_with(|| {
            let prior = prior_urls(root, cfg);
            (Ctx { llm: &llm, searcher: &searcher, prior_urls: prior.0 }, Ctx { llm: &llm, searcher: &searcher, prior_urls: prior.1 })
        });
        match process_year(args, ctx, vol_ctx, cfg, year, &mut state, now, current_year) {
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

/// Whether a conference-year still has something to attempt this run:
/// active stages are re-collected every run, archived ones never.
fn needs_work(args: &Args, root: &Path, state: &State, cfg: &config::ConferenceCfg, year: i32, current_year: i32) -> bool {
    let today = Utc::now().date_naive();
    let key = cfg.key(year);
    let dir = root.join("conferences").join(&key);
    let cfp = read_record::<schema::Cfp>(&dir.join("cfp.json")).ok();
    let stage = schema::stage(cfp.as_ref().map(|r| &r.data), today);
    let cfp_work = stage.active() && (cfp.is_some() || skip_reason(state, &format!("{key}/cfp"), year, current_year).is_none());
    let vol_recent_negative = state.get(&format!("{key}/volunteer")).is_some_and(|a| matches!(a.outcome, Outcome::NotFound | Outcome::NoVolunteerProgram) && Utc::now() - a.last_attempt < chrono::Duration::days(VOLUNTEER_RETRY_DAYS));
    let vol_has_data = dir.join("volunteer.json").exists();
    let vol_work = !args.no_volunteer && volunteer_active(root, cfg, year, stage, today) && (vol_has_data || (!vol_recent_negative && (cfp.is_some() || skip_reason(state, &format!("{key}/volunteer"), year, current_year).is_none())));
    cfp_work || vol_work
}

/// Volunteer applications open late (after rebuttals), so the volunteer
/// pass stays active until the conference has happened or its own
/// deadline has passed.
fn volunteer_active(root: &Path, cfg: &config::ConferenceCfg, year: i32, stage: schema::Stage, today: chrono::NaiveDate) -> bool {
    if stage == schema::Stage::Happened {
        return false;
    }
    match read_record::<schema::Volunteer>(&root.join("conferences").join(cfg.key(year)).join("volunteer.json")) {
        Ok(r) => today <= r.data.deadline,
        Err(_) => true,
    }
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

    // --- Call for papers ---
    let existing = read_record::<schema::Cfp>(&dir.join("cfp.json")).ok();
    let mut stage = schema::stage(existing.as_ref().map(|r| &r.data), today);
    let mut cfp_url: Option<String> = existing.as_ref().map(|r| r.provenance.source_url.clone());
    if !stage.active() {
        log::info!("{cfp_key}: archived ({})", stage.label());
    } else if existing.is_none() && skip_reason(state, &cfp_key, year, current_year).is_some() {
        log::info!("{cfp_key}: abandoned");
        if state.get(&cfp_key).map(|a| a.outcome) != Some(Outcome::Abandoned) {
            state.record(&cfp_key, Outcome::Abandoned, None, vec![], "year is in the past".into(), now, overdue);
        }
    } else {
        log::info!("=== {cfp_key}: {} ({})", if existing.is_some() { "re-validating" } else { "searching" }, stage.label());
        let t = Instant::now();
        let att = discover::find_cfp(ctx, cfg, year, cfp_url.as_deref())?;
        let secs = t.elapsed().as_secs();
        let outcome = match &att.result {
            Ok(found) => {
                log::info!("{cfp_key}: {}", serde_json::to_string(&found.value)?);
                let rec = merge_record(existing.clone(), cfg, year, found, model, now, |old, new| schema::diff_cfp(old, new, now));
                cfp_url = Some(rec.provenance.source_url.clone());
                stage = schema::stage(Some(&rec.data), today);
                if !args.dry_run {
                    write_cfp(&dir, cfg, year, &rec, found, existing.is_none() || !rec.history.is_empty() && rec.fetched_at == now)?;
                }
                Outcome::Ok
            }
            Err(Miss::Invalid) => Outcome::Invalid,
            Err(_) => Outcome::NotFound,
        };
        if existing.is_some() && outcome != Outcome::Ok {
            log::warn!("{cfp_key}: re-validation failed ({outcome:?}); keeping the stored data");
        }
        log_attempt(&cfp_key, &att, outcome, secs);
        state.record(&cfp_key, outcome, att.trail.backend.clone(), att.trail.codes.clone(), att.trail.note_text(), now, overdue);
    }

    // --- Student volunteers ---
    if args.no_volunteer || !volunteer_active(root, cfg, year, stage, today) {
        return Ok(());
    }
    let existing_vol = read_record::<schema::Volunteer>(&dir.join("volunteer.json")).ok();
    if existing_vol.is_none() && skip_reason(state, &vol_key, year, current_year).is_some() {
        log::info!("{vol_key}: abandoned");
        if state.get(&vol_key).map(|a| a.outcome) != Some(Outcome::Abandoned) {
            state.record(&vol_key, Outcome::Abandoned, None, vec![], "year is in the past".into(), now, overdue);
        }
        return Ok(());
    }
    let Some(cfp_url) = cfp_url else {
        log::info!("{vol_key}: skipped (no call for papers found yet)");
        return Ok(());
    };
    // Volunteer pages appear late; a negative result is not retried for a while.
    if existing_vol.is_none() {
        if let Some(a) = state.get(&vol_key) {
            if matches!(a.outcome, Outcome::NotFound | Outcome::NoVolunteerProgram) && now - a.last_attempt < chrono::Duration::days(VOLUNTEER_RETRY_DAYS) {
                log::info!("{vol_key}: {:?} {} days ago; not retried yet", a.outcome, (now - a.last_attempt).num_days());
                return Ok(());
            }
        }
    }
    log::info!("=== {vol_key}: {}", if existing_vol.is_some() { "re-validating" } else { "searching" });
    let t = Instant::now();
    let known = existing_vol.as_ref().map(|r| r.provenance.source_url.clone());
    let conference_end = read_record::<schema::Cfp>(&dir.join("cfp.json")).ok().and_then(|r| r.data.conference.map(|c| c.end));
    let att = discover::find_volunteer(vol_ctx, cfg, year, known.as_deref(), Some(&cfp_url), conference_end)?;
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

/// Combine a fresh extraction with the stored record: unchanged data only
/// bumps `last_verified`; changed data replaces it and extends the history.
fn merge_record<T: Clone + Serialize>(existing: Option<Record<T>>, cfg: &config::ConferenceCfg, year: i32, found: &discover::Found<T>, model: &str, now: chrono::DateTime<Utc>, diff: impl Fn(&T, &T) -> Vec<schema::Change>) -> Record<T> {
    match existing {
        Some(mut old) => {
            let changes = diff(&old.data, &found.value);
            old.last_verified = Some(now);
            if changes.is_empty() {
                log::info!("{}: unchanged", cfg.key(year));
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
        None => Record { conference: cfg.conference.clone(), track: cfg.track.clone(), year, label: cfg.label(year), fetched_at: now, last_verified: Some(now), stage: None, model: model.into(), provenance: found.prov.clone(), data: found.value.clone(), history: vec![], raw: found.raw.clone() },
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
        if let Ok(r) = read_record::<schema::Cfp>(&e.path().join("cfp.json")) {
            cfp.push((year, r.provenance.source_url));
        }
        if let Ok(r) = read_record::<schema::Volunteer>(&e.path().join("volunteer.json")) {
            vol.push((year, r.provenance.source_url));
        }
    }
    (cfp, vol)
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

/// Write the record, its calendar and (when the data is new) the page it
/// came from. Pages are not rewritten for unchanged data, so weekly
/// re-validation does not churn the repository.
fn write_cfp(dir: &Path, cfg: &config::ConferenceCfg, year: i32, rec: &Record<schema::Cfp>, found: &discover::Found<schema::Cfp>, write_pages: bool) -> Result<()> {
    let mut rec = rec.clone();
    rec.stage = Some(schema::stage(Some(&rec.data), Utc::now().date_naive()));
    write_json(&dir.join("cfp.json"), &rec)?;
    if write_pages || !dir.join("cfp.html").exists() {
        std::fs::write(dir.join("cfp.html"), &found.html)?;
        std::fs::write(dir.join("cfp.md"), &found.md)?;
    }
    let events = ics::cfp_events(&rec.label, &cfg.key(year), &rec.data, &provenance_of(&rec.provenance, &rec.history), rec.fetched_at);
    std::fs::write(dir.join("cfp.ics"), ics::calendar(&rec.label, &events))?;
    log::info!("wrote {} ({} events, stage {})", dir.join("cfp.ics").display(), events.len(), rec.stage.map(|s| s.label()).unwrap_or("?"));
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
    let mut status: Vec<(String, String, String, String, String)> = vec![];
    let today = Utc::now().date_naive();
    let conf_dir = root.join("conferences");
    if conf_dir.exists() {
        for entry in walk(&conf_dir)? {
            let name = entry.file_name().unwrap().to_string_lossy().to_string();
            if name == "cfp.json" {
                let r = match read_record::<schema::Cfp>(&entry) {
                    Ok(r) => r,
                    Err(e) => {
                        log::error!("skipping unreadable {}: {e:#}", entry.display());
                        continue;
                    }
                };
                let key = format!("{}/{}/{}", r.conference, r.track, r.year);
                let ev = ics::cfp_events(&r.label, &key, &r.data, &provenance_of(&r.provenance, &r.history), r.fetched_at);
                for e in &ev {
                    rows.push((e.start, r.label.clone(), e.summary.trim_start_matches(&format!("[{}] ", r.label)).to_string()));
                }
                events.extend(ev);
                let stage = schema::stage(Some(&r.data), today);
                let deadlines = r.data.rounds.iter().map(|x| x.submission.to_string()).collect::<Vec<_>>().join(", ");
                let conf = r.data.conference.as_ref().map(|c| format!("{}..{}", c.start, c.end)).unwrap_or_default();
                let verified = r.last_verified.unwrap_or(r.fetched_at).format("%Y-%m-%d").to_string();
                status.push((r.label.clone(), stage.label().to_string(), deadlines, conf, verified));
            } else if name == "volunteer.json" {
                let r = match read_record::<schema::Volunteer>(&entry) {
                    Ok(r) => r,
                    Err(e) => {
                        log::error!("skipping unreadable {}: {e:#}", entry.display());
                        continue;
                    }
                };
                let key = format!("{}/{}/{}", r.conference, r.track, r.year);
                let ev = ics::volunteer_events(&r.label, &key, &r.data, &provenance_of(&r.provenance, &r.history), r.fetched_at);
                for e in &ev {
                    rows.push((e.start, r.label.clone(), "Volunteer Application Deadline".into()));
                }
                events.extend(ev);
            }
        }
    }
    std::fs::write(root.join("all.ics"), ics::calendar("PL conference deadlines", &events))?;
    std::fs::write(root.join("MAINTENANCE.md"), state.render_maintenance())?;

    // README: upcoming dates table, status table, maintenance line.
    rows.sort();
    let mut table = String::from("| Date | Conference | Event |\n|---|---|---|\n");
    let mut count = 0;
    for (d, label, what) in rows.iter().filter(|(d, _, _)| *d >= today) {
        table.push_str(&format!("| {d} | {label} | {what} |\n"));
        count += 1;
    }
    if count == 0 {
        table.push_str("| | *(nothing upcoming yet)* | |\n");
    }
    let readme_path = root.join("README.md");
    let readme = std::fs::read_to_string(&readme_path).unwrap_or_default();
    let readme = state::replace_marked(&readme, "dates", &table);
    let mut st = String::from("| Conference | Stage | Submission deadline(s) | Conference dates | Last verified |\n|---|---|---|---|---|\n");
    status.sort();
    for (l, s, d, c, v) in &status {
        st.push_str(&format!("| {l} | {s} | {d} | {c} | {v} |\n"));
    }
    if status.is_empty() {
        st.push_str("| *(nothing collected yet)* | | | | |\n");
    }
    let readme = state::replace_marked(&readme, "status", &st);
    let readme = state::replace_marked(&readme, "maintenance", &state.summary_line());
    std::fs::write(&readme_path, readme)?;
    log::info!("wrote all.ics ({} events), README.md, MAINTENANCE.md", events.len());
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

    fn cfp(sub: &str) -> schema::Cfp {
        schema::Cfp {
            conference: None,
            rounds: vec![schema::ValidRound { label: String::new(), submission: NaiveDate::parse_from_str(sub, "%Y-%m-%d").unwrap(), response_start: None, response_end: None, notification: None }],
            submission_details: "prose".into(),
        }
    }

    fn found(sub: &str) -> discover::Found<schema::Cfp> {
        discover::Found { value: cfp(sub), raw: serde_json::Value::Null, prov: discover::Provenance { source_url: "https://x.org/new".into(), ..Default::default() }, html: String::new(), md: String::new() }
    }

    #[test]
    fn merge_keeps_unchanged_data_and_records_changes() {
        let cfg = config::ConferenceCfg { conference: "X".into(), track: "X".into(), since: 2027 };
        let t0 = Utc::now() - chrono::Duration::days(7);
        let t1 = Utc::now();
        let first = merge_record(None, &cfg, 2027, &found("2026-07-01"), "m", t0, |a, b| schema::diff_cfp(a, b, t0));
        assert!(first.history.is_empty() && first.last_verified == Some(t0));

        // Same dates, different prose: only last_verified moves.
        let mut same = found("2026-07-01");
        same.value.submission_details = "other prose".into();
        same.prov.source_url = "https://x.org/other".into();
        let second = merge_record(Some(first.clone()), &cfg, 2027, &same, "m", t1, |a, b| schema::diff_cfp(a, b, t1));
        assert!(second.history.is_empty());
        assert_eq!(second.last_verified, Some(t1));
        assert_eq!(second.fetched_at, t0, "unchanged data keeps its original fetch time");
        assert_eq!(second.provenance.source_url, "https://x.org/new");
        assert_eq!(second.data.submission_details, "prose");

        // A moved deadline replaces the data and is remembered.
        let third = merge_record(Some(second), &cfg, 2027, &found("2026-07-08"), "m", t1, |a, b| schema::diff_cfp(a, b, t1));
        assert_eq!(third.history.len(), 1);
        assert_eq!((third.history[0].field.as_str(), third.history[0].old.as_str(), third.history[0].new.as_str()), ("round 1 submission", "2026-07-01", "2026-07-08"));
        assert_eq!(third.data.rounds[0].submission.to_string(), "2026-07-08");
        assert_eq!(third.fetched_at, t1);
    }
}
