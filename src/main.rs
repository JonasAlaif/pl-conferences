use anyhow::{Context, Result};
use pl_conferences::{clean, config, discover, fetch, ics, llm, schema, search, state};
use chrono::{Datelike, Utc};
use discover::{Attempted, Ctx, Miss};
use serde::{Deserialize, Serialize};
use state::{Outcome, State};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

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

/// Stored as `cfp.json` / `volunteer.json`.
#[derive(Debug, Serialize, Deserialize)]
struct Record<T> {
    conference: String,
    track: String,
    year: i32,
    label: String,
    fetched_at: chrono::DateTime<Utc>,
    #[serde(default)]
    model: String,
    #[serde(default)]
    provenance: discover::Provenance,
    data: T,
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
    items.sort_by_key(|(_, y)| *y > current_year);
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

/// Whether a conference-year still has something to attempt this run.
fn needs_work(args: &Args, root: &Path, state: &State, cfg: &config::ConferenceCfg, year: i32, current_year: i32) -> bool {
    let key = cfg.key(year);
    let dir = root.join("conferences").join(&key);
    let cfp_done = dir.join("cfp.json").exists() || skip_reason(state, &format!("{key}/cfp"), year, current_year).is_some();
    let vol_done = args.no_volunteer || dir.join("volunteer.json").exists() || skip_reason(state, &format!("{key}/volunteer"), year, current_year).is_some();
    !(cfp_done && vol_done)
}

/// One conference-year: call for papers, then student volunteers.
#[allow(clippy::too_many_arguments)]
fn process_year(args: &Args, ctx: &Ctx, vol_ctx: &Ctx, cfg: &config::ConferenceCfg, year: i32, state: &mut State, now: chrono::DateTime<Utc>, current_year: i32) -> Result<()> {
    let root = &args.root;
    let key = cfg.key(year);
    let dir = root.join("conferences").join(&key);
    let cfp_key = format!("{key}/cfp");
    let vol_key = format!("{key}/volunteer");
    let overdue = year <= current_year;
    let model = &ctx.llm.model;

    // --- Call for papers ---
    let mut cfp_url: Option<String> = None;
    if dir.join("cfp.json").exists() {
        if let Ok(r) = read_record::<schema::Cfp>(&dir.join("cfp.json")) {
            cfp_url = Some(r.provenance.source_url);
        }
    } else if let Some(reason) = skip_reason(state, &cfp_key, year, current_year) {
        log::info!("{cfp_key}: {reason}");
        if reason == "abandoned" && state.get(&cfp_key).map(|a| a.outcome) != Some(Outcome::Abandoned) {
            state.record(&cfp_key, Outcome::Abandoned, None, vec![], "year is in the past".into(), now, overdue);
        }
    } else {
        log::info!("=== {cfp_key}: searching");
        let t = Instant::now();
        let att = discover::find_cfp(ctx, cfg, year)?;
        let secs = t.elapsed().as_secs();
        let outcome = match &att.result {
            Ok(found) => {
                cfp_url = Some(found.prov.source_url.clone());
                log::info!("{cfp_key}: {}", serde_json::to_string(&found.value)?);
                if !args.dry_run {
                    write_cfp(&dir, cfg, year, found, model, now)?;
                }
                Outcome::Ok
            }
            Err(Miss::Invalid) => Outcome::Invalid,
            Err(_) => Outcome::NotFound,
        };
        log_attempt(&cfp_key, &att, outcome, secs);
        state.record(&cfp_key, outcome, att.trail.backend.clone(), att.trail.codes.clone(), att.trail.note_text(), now, overdue);
    }

    // --- Student volunteers ---
    if args.no_volunteer || dir.join("volunteer.json").exists() {
        return Ok(());
    }
    if let Some(reason) = skip_reason(state, &vol_key, year, current_year) {
        log::info!("{vol_key}: {reason}");
        if reason == "abandoned" && state.get(&vol_key).map(|a| a.outcome) != Some(Outcome::Abandoned) {
            state.record(&vol_key, Outcome::Abandoned, None, vec![], "year is in the past".into(), now, overdue);
        }
        return Ok(());
    }
    let Some(cfp_url) = cfp_url else {
        log::info!("{vol_key}: skipped (no call for papers found yet)");
        return Ok(());
    };
    log::info!("=== {vol_key}: searching");
    let t = Instant::now();
    let att = discover::find_volunteer(vol_ctx, cfg, year, Some(&cfp_url))?;
    let secs = t.elapsed().as_secs();
    let outcome = match &att.result {
        Ok(found) => {
            log::info!("{vol_key}: {}", serde_json::to_string(&found.value)?);
            if !args.dry_run {
                write_volunteer(&dir, cfg, year, found, model, now)?;
            }
            Outcome::Ok
        }
        Err(Miss::Invalid) => Outcome::Invalid,
        Err(Miss::NoProgram) => Outcome::NoVolunteerProgram,
        Err(Miss::NotFound) => Outcome::NotFound,
    };
    log_attempt(&vol_key, &att, outcome, secs);
    state.record(&vol_key, outcome, att.trail.backend.clone(), att.trail.codes.clone(), att.trail.note_text(), now, overdue);
    Ok(())
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

fn provenance_of(p: &discover::Provenance) -> ics::Provenance {
    ics::Provenance { source_url: p.source_url.clone(), codes: p.codes.iter().map(|c| format!("{c:?}")).collect() }
}

fn write_cfp(dir: &Path, cfg: &config::ConferenceCfg, year: i32, found: &discover::Found<schema::Cfp>, model: &str, now: chrono::DateTime<Utc>) -> Result<()> {
    let rec = Record { conference: cfg.conference.clone(), track: cfg.track.clone(), year, label: cfg.label(year), fetched_at: now, model: model.into(), provenance: found.prov.clone(), data: found.value.clone(), raw: found.raw.clone() };
    write_json(&dir.join("cfp.json"), &rec)?;
    let events = ics::cfp_events(&rec.label, &cfg.key(year), &rec.data, &provenance_of(&rec.provenance), now);
    std::fs::write(dir.join("cfp.ics"), ics::calendar(&rec.label, &events))?;
    log::info!("wrote {}", dir.join("cfp.ics").display());
    Ok(())
}

fn write_volunteer(dir: &Path, cfg: &config::ConferenceCfg, year: i32, found: &discover::Found<schema::Volunteer>, model: &str, now: chrono::DateTime<Utc>) -> Result<()> {
    let rec = Record { conference: cfg.conference.clone(), track: cfg.track.clone(), year, label: cfg.label(year), fetched_at: now, model: model.into(), provenance: found.prov.clone(), data: found.value.clone(), raw: found.raw.clone() };
    write_json(&dir.join("volunteer.json"), &rec)?;
    let events = ics::volunteer_events(&rec.label, &cfg.key(year), &rec.data, &provenance_of(&rec.provenance), now);
    std::fs::write(dir.join("volunteer.ics"), ics::calendar(&format!("{} volunteers", rec.label), &events))?;
    log::info!("wrote {}", dir.join("volunteer.ics").display());
    Ok(())
}

/// Rebuild `all.ics`, the README tables and `MAINTENANCE.md` from the JSON on disk.
fn regenerate_outputs(root: &Path, state: &State) -> Result<()> {
    let mut events = vec![];
    let mut rows: Vec<(chrono::NaiveDate, String, String)> = vec![];
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
                let ev = ics::cfp_events(&r.label, &key, &r.data, &provenance_of(&r.provenance), r.fetched_at);
                for e in &ev {
                    rows.push((e.start, r.label.clone(), e.summary.trim_start_matches(&format!("[{}] ", r.label)).to_string()));
                }
                events.extend(ev);
            } else if name == "volunteer.json" {
                let r = match read_record::<schema::Volunteer>(&entry) {
                    Ok(r) => r,
                    Err(e) => {
                        log::error!("skipping unreadable {}: {e:#}", entry.display());
                        continue;
                    }
                };
                let key = format!("{}/{}/{}", r.conference, r.track, r.year);
                let ev = ics::volunteer_events(&r.label, &key, &r.data, &provenance_of(&r.provenance), r.fetched_at);
                for e in &ev {
                    rows.push((e.start, r.label.clone(), "Volunteer Application Deadline".into()));
                }
                events.extend(ev);
            }
        }
    }
    std::fs::write(root.join("all.ics"), ics::calendar("PL conference deadlines", &events))?;
    std::fs::write(root.join("MAINTENANCE.md"), state.render_maintenance())?;

    // README: upcoming dates table + maintenance line.
    let today = Utc::now().date_naive();
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
