//! Find the right page for a conference-year by web search, LLM choice among
//! hits, and LLM-guided link following; then extract and validate.

use crate::clean;
use crate::config::ConferenceCfg;
use crate::fetch;
use crate::llm::{Llm, LlmError};
use crate::schema::{self, Cfp, CfpExtraction, Choice, Conference, Deadlines, Extraction, UrlGuesses, Volunteer, VolunteerExtraction};
use crate::search::{Hit, Searcher, merge};
use crate::state::Code;
use anyhow::Result;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize, de::DeserializeOwned};

/// Pages with less text than this cannot describe a call for papers or a
/// volunteer programme; they are skipped without a model call.
pub const MIN_PAGE_CHARS: usize = 400;

pub const SYSTEM_PROMPT: &str = "You extract information from conference web pages. Use only facts stated on the page you are given. Never invent, guess or infer a date: every date you report must be written on the page, and you quote the words it comes from before converting it to YYYY-MM-DD. When something is not stated, answer null.";

const GUESS_PROMPT: &str = "You know the websites of academic conferences and how their URLs are usually formed from the conference name and year. Suggest plausible URLs; they will be checked.";

pub struct Ctx<'a> {
    pub llm: &'a Llm,
    pub searcher: &'a Searcher,
    /// Source URLs of earlier editions already in the repository, as (year, url).
    pub prior_urls: Vec<(i32, String)>,
}

/// How a page was found and processed.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Provenance {
    pub source_url: String,
    pub query: String,
    pub backend: Option<String>,
    pub hops: u32,
    pub via_chrome: bool,
    pub retried: bool,
    pub codes: Vec<Code>,
}

#[derive(Debug, Clone)]
pub struct Found<T> {
    pub value: T,
    pub raw: serde_json::Value,
    pub prov: Provenance,
    /// The page the data came from, as fetched and as shown to the model.
    pub html: String,
    pub md: String,
}

/// What a call-for-papers attempt produced: the two parts can come from
/// different pages (home page for the dates, a later page for deadlines).
#[derive(Debug, Clone, Default)]
pub struct CfpFound {
    pub conference: Option<Found<Conference>>,
    pub deadlines: Option<Found<Deadlines>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Miss {
    NotFound,
    Invalid,
    /// Volunteer only: the conference page was found but has no programme.
    NoProgram,
}

/// What an attempt produced, plus everything worth recording.
#[derive(Debug)]
pub struct Attempted<T> {
    pub result: Result<T, Miss>,
    pub trail: Trail,
}

#[derive(Debug, Clone, Default)]
pub struct Trail {
    pub query: String,
    pub backend: Option<String>,
    pub codes: Vec<Code>,
    pub notes: Vec<String>,
    /// Number of pages fetched.
    pub pages: u32,
    pub llm_calls: u32,
    /// URLs already tried in this attempt (never fetched twice).
    pub visited: std::collections::HashSet<String>,
}

impl Trail {
    /// Already fetched (or rejected) in this attempt.
    fn tried(&self, url: &str) -> bool {
        self.visited.contains(url.trim_end_matches('/'))
    }
    /// Remember a URL as tried without fetching it (e.g. a pick that failed).
    fn mark_tried(&mut self, url: &str) {
        self.visited.insert(url.trim_end_matches('/').to_string());
    }
    fn code(&mut self, c: Code) {
        if !self.codes.contains(&c) {
            self.codes.push(c);
        }
    }
    fn note(&mut self, s: impl Into<String>) {
        let s = s.into();
        log::info!("{s}");
        self.notes.push(s);
    }
    pub fn note_text(&self) -> String {
        self.notes.join("; ")
    }
}

fn squash(s: &str) -> String {
    s.chars().filter(|c| c.is_alphanumeric()).collect::<String>().to_lowercase()
}

/// `name` appears as a whole word (short names like CAV would otherwise
/// match "excavation").
fn word_match(text: &str, name: &str) -> bool {
    let re = regex::Regex::new(&format!(r"(?i)(^|[^a-z0-9]){}([^a-z0-9]|$)", regex::escape(name))).unwrap();
    re.is_match(text)
}

/// A hit is worth considering if it mentions the conference or the track as
/// a word in its title or snippet, or anywhere in its URL (hosts such as
/// splashcon.org or popl26.sigplan.org glue the name to other text). Cheap
/// guard against engines that answer automation with unrelated pages.
pub fn relevant_hit(cfg: &ConferenceCfg, h: &Hit) -> bool {
    let text = format!("{} {}", h.title, h.snippet);
    let url = squash(&h.url);
    [cfg.conference.as_str(), cfg.track.as_str()].iter().any(|n| word_match(&text, n) || url.contains(&squash(n)))
}

/// Model call whose unusable output is a page-level failure, not a run-level one.
fn ask<X: DeserializeOwned + JsonSchema>(ctx: &Ctx, trail: &mut Trail, system: &str, prompt: &str) -> Result<Option<(X, String)>> {
    trail.llm_calls += 1;
    match ctx.llm.extract::<X>(system, prompt) {
        Ok((x, raw, _)) => Ok(Some((x, raw))),
        Err(LlmError::BadOutput(s)) => {
            trail.note(format!("model output unusable: {s}"));
            Ok(None)
        }
        Err(LlmError::Transport(e)) => Err(e),
    }
}

/// Candidate URLs for `year` derived from earlier editions' URLs by
/// substituting the year (`popl25.sigplan.org/track/POPL-2025-...` becomes
/// `popl26.sigplan.org/track/POPL-2026-...`). Needs no search engine at all.
pub fn derived_urls(prior: &[(i32, String)], year: i32) -> Vec<String> {
    let mut out = vec![];
    let mut sorted: Vec<&(i32, String)> = prior.iter().filter(|(y, _)| *y != year).collect();
    sorted.sort_by_key(|(y, _)| std::cmp::Reverse(*y));
    for (y, url) in sorted {
        let full = url.replace(&y.to_string(), &year.to_string());
        // Two-digit years only where they follow a letter (popl25, icfp25).
        let re = regex::Regex::new(&format!(r"(?i)([a-z]){:02}(\D|$)", y % 100)).unwrap();
        let short = re.replace_all(&full, format!("${{1}}{:02}${{2}}", year % 100)).into_owned();
        for c in [short, full] {
            if c != *url && !out.contains(&c) {
                // The site root too: track slugs change between years, and
                // link-following from the home page finds the rest.
                let origin = url::Url::parse(&c).ok().map(|u| format!("{}://{}/", u.scheme(), u.host_str().unwrap_or("")));
                out.push(c);
                if let Some(o) = origin {
                    if !out.contains(&o) {
                        out.push(o);
                    }
                }
            }
        }
    }
    out.truncate(8);
    out
}

/// Search with the queries in turn (later ones only if earlier ones gave
/// few relevant hits), merge, and fall back to URLs derived from earlier
/// editions, then to LLM URL guessing.
fn gather_hits(ctx: &Ctx, cfg: &ConferenceCfg, year: i32, queries: &[String], trail: &mut Trail) -> Result<Vec<Hit>> {
    let mut lists: Vec<Vec<Hit>> = vec![];
    let mut primary_failed = false;
    let mut primary_throttled = false;
    let relevant = |h: &Hit| relevant_hit(cfg, h);
    for q in queries {
        if lists.iter().map(Vec::len).sum::<usize>() >= 3 {
            break;
        }
        let out = ctx.searcher.search(q, &relevant);
        if let Some(f) = out.failures.iter().find(|f| Some(f.backend) == ctx.searcher.primary()) {
            primary_failed = true;
            primary_throttled |= f.throttled;
        }
        if let Some(b) = out.backend {
            if trail.backend.is_none() {
                trail.backend = Some(b.to_string());
            }
            lists.push(out.hits);
        } else {
            trail.note(format!("search failed for {q:?}: {}", out.failure_text()));
        }
    }
    let mut hits = merge(&lists, 8);
    if primary_failed && !hits.is_empty() {
        trail.code(if primary_throttled { Code::E008 } else { Code::E001 });
    }
    if hits.is_empty() {
        trail.code(Code::E002);
        let probe = |u: &str, trail: &mut Trail, hits: &mut Vec<Hit>| match fetch::get(u) {
            Ok(page) => {
                trail.pages += 1;
                let title = clean::title(&page.html).unwrap_or_default();
                if !hits.iter().any(|h| h.url == page.url) {
                    hits.push(Hit { title, url: page.url, snippet: String::from("(candidate URL)") });
                }
            }
            Err(e) => log::info!("candidate URL {u} failed: {e:#}"),
        };
        let derived = derived_urls(&ctx.prior_urls, year);
        if !derived.is_empty() {
            trail.note(format!("no search backend answered; trying URLs derived from earlier editions: {}", derived.join(", ")));
            for u in &derived {
                probe(u, trail, &mut hits);
            }
        }
        if hits.is_empty() {
            trail.note("guessing URLs with the model");
            let examples = if ctx.prior_urls.is_empty() {
                String::new()
            } else {
                let mut ex: Vec<&(i32, String)> = ctx.prior_urls.iter().collect();
                ex.sort_by_key(|(y, _)| std::cmp::Reverse(*y));
                format!(" Earlier editions used: {}.", ex.iter().take(3).map(|(y, u)| format!("{y}: {u}")).collect::<Vec<_>>().join("; "))
            };
            let prompt = format!("List up to five full URLs (with https://) that are most likely the official website or call for papers of the {} {year} conference (track: {}).{examples}", cfg.conference, cfg.track);
            if let Some((g, _)) = ask::<UrlGuesses>(ctx, trail, GUESS_PROMPT, &prompt)? {
                trail.note(format!("guessed URLs: {}", g.urls.join(", ")));
                for u in g.urls.iter().take(5) {
                    probe(u, trail, &mut hits);
                }
            }
        }
    }
    Ok(hits)
}

fn numbered<'a>(items: impl Iterator<Item = (&'a str, &'a str, &'a str)>) -> String {
    let mut s = String::new();
    for (i, (a, b, c)) in items.enumerate() {
        s.push_str(&format!("{i}. {a} | {b}"));
        if !c.is_empty() {
            s.push_str(&format!(" | {c}"));
        }
        s.push('\n');
    }
    s
}

/// Ask the model to pick one entry; returns the chosen index if valid.
fn choose(ctx: &Ctx, trail: &mut Trail, question: &str, list: &str, n: usize) -> Result<Option<usize>> {
    let Some((c, _)) = ask::<Choice>(ctx, trail, SYSTEM_PROMPT, &format!("{question}\n\n{list}"))? else { return Ok(None) };
    Ok(c.index.map(|i| i as usize).filter(|i| *i < n))
}

/// Candidate hits in the order to try: the model's pick first, then the rest.
fn ordered_hits(ctx: &Ctx, trail: &mut Trail, hits: Vec<Hit>, what: &str) -> Result<Vec<Hit>> {
    let hits: Vec<Hit> = hits.into_iter().filter(|h| !trail.tried(&h.url)).collect();
    if hits.len() <= 1 {
        return Ok(hits);
    }
    let list = numbered(hits.iter().map(|h| (h.title.as_str(), h.url.as_str(), h.snippet.as_str())));
    let q = format!("Which of these search results is {what}? Prefer the conference's own website over aggregators, listings or social media. Answer with the index, or null if none fits.");
    let pick = choose(ctx, trail, &q, &list, hits.len())?;
    let mut out = hits;
    if let Some(i) = pick {
        let h = out.remove(i);
        out.insert(0, h);
    }
    Ok(out)
}

/// Page text plus every link target, so a URL the model reports can be
/// checked against both.
fn grounding_text(md: &str, links: &[(String, String)]) -> String {
    let mut s = String::with_capacity(md.len() + links.len() * 64);
    s.push_str(md);
    for (_, u) in links {
        s.push('\n');
        s.push_str(u);
    }
    s
}

/// Ask the model which of the page's links is `what`; None if it says none.
fn pick_link(ctx: &Ctx, trail: &mut Trail, links: &[(String, String)], page_url: &str, what: &str) -> Result<Option<String>> {
    if links.is_empty() {
        return Ok(None);
    }
    // Links from another host first: a submission system or a form lives
    // elsewhere more often than not, and this page's own navigation is noise.
    let host = url::Url::parse(page_url).ok().and_then(|u| u.host_str().map(String::from));
    let mut ranked: Vec<&(String, String)> = links.iter().filter(|(_, u)| !trail.tried(u)).collect();
    ranked.sort_by_key(|(_, u)| host.as_deref().is_some_and(|h| u.contains(h)));
    let shown: Vec<&(String, String)> = ranked.into_iter().take(MAX_LINKS).collect();
    let list = numbered(shown.iter().map(|(t, u)| (t.as_str(), u.as_str(), "")));
    let q = format!("Which of these links on the page is {what}? Answer with the index, or null if no link is that.");
    let pick = choose(ctx, trail, &q, &list, shown.len())?.map(|i| shown[i].1.clone());
    if let Some(u) = &pick {
        trail.note(format!("model picked link {u} as {what}"));
    }
    Ok(pick)
}

/// Links on a page, most promising first (mention of the conference, track
/// or a call-for-papers word, then same host), capped so the list fits the
/// model's context.
fn candidate_links(html: &str, url: &str, keywords: &[&str]) -> Vec<(String, String)> {
    let host = url::Url::parse(url).ok().and_then(|u| u.host_str().map(String::from));
    let all = clean::links(html, url);
    let kws: Vec<String> = keywords.iter().map(|k| squash(k)).filter(|k| !k.is_empty()).collect();
    let mut scored: Vec<(i32, (String, String))> = all
        .into_iter()
        .map(|(t, u)| {
            let hay = format!("{}{}", squash(&t), squash(&u));
            let mut s = 0;
            if kws.iter().any(|k| hay.contains(k)) {
                s += 2;
            }
            if host.as_deref().is_some_and(|h| u.contains(h)) {
                s += 1;
            }
            (s, (t, u))
        })
        .collect();
    scored.sort_by_key(|(s, _)| std::cmp::Reverse(*s));
    scored.into_iter().map(|(_, l)| l).take(MAX_LINKS).collect()
}

/// Links shown to the model per choice. On the runner every 1K prompt
/// tokens costs about half a minute, so the list is kept short (ranked
/// links first) and same-host links are shown as paths.
const MAX_LINKS: usize = 80;

fn shown_url(u: &str, host: Option<&str>) -> String {
    match (url::Url::parse(u), host) {
        (Ok(p), Some(h)) if p.host_str() == Some(h) => format!("{}{}", p.path(), p.query().map(|q| format!("?{q}")).unwrap_or_default()),
        _ => u.to_string(),
    }
}

const LINK_WORDS: &[&str] = &["call for papers", "cfp", "dates", "deadline", "submission", "papers", "volunteer", "student"];

fn follow_link(ctx: &Ctx, trail: &mut Trail, html: &str, url: &str, what: &str, names: &[&str]) -> Result<Option<String>> {
    let mut kws: Vec<&str> = names.to_vec();
    kws.extend_from_slice(LINK_WORDS);
    // Links already tried in this attempt are not offered again: the model
    // would otherwise keep picking the same one.
    let links: Vec<(String, String)> = candidate_links(html, url, &kws).into_iter().filter(|(_, u)| !trail.tried(u)).collect();
    if links.is_empty() {
        return Ok(None);
    }
    let host = url::Url::parse(url).ok().and_then(|u| u.host_str().map(String::from));
    let shown: Vec<String> = links.iter().map(|(_, u)| shown_url(u, host.as_deref())).collect();
    let list = numbered(links.iter().zip(&shown).map(|((t, _), su)| (t.as_str(), su.as_str(), "")));
    let q = format!("Which link most likely leads to {what}? Answer with the index, or null if none fits.");
    Ok(choose(ctx, trail, &q, &list, links.len())?.map(|i| links[i].1.clone()))
}

fn fetch_page(trail: &mut Trail, url: &str) -> Option<fetch::Page> {
    if !trail.visited.insert(url.trim_end_matches('/').to_string()) {
        log::info!("{url}: already tried in this attempt");
        return None;
    }
    match fetch::get_rendered(url) {
        Ok(p) => {
            trail.pages += 1;
            // The page may have redirected: its final URL counts as tried too.
            trail.mark_tried(&p.url);
            if p.via_chrome {
                trail.code(Code::E004);
            }
            if p.insecure_tls {
                trail.code(Code::E009);
            }
            Some(p)
        }
        Err(e) => {
            trail.note(format!("fetch {url} failed: {e:#}"));
            None
        }
    }
}

/// Run the extraction; on validation failure retry once with the errors.
/// `None` means the model produced nothing usable for this page.
fn extract_validated<X, T>(ctx: &Ctx, trail: &mut Trail, prompt: &str, validate: impl Fn(&X) -> Result<T, Vec<String>>) -> Result<Option<(X, serde_json::Value, Result<T, Vec<String>>, bool)>>
where
    X: DeserializeOwned + JsonSchema + Serialize + Extraction,
{
    let Some((x, raw)) = ask::<X>(ctx, trail, SYSTEM_PROMPT, prompt)? else { return Ok(None) };
    match validate(&x) {
        Ok(v) => Ok(Some((x, serde_json::from_str(&raw)?, Ok(v), false))),
        // A retry can only help when the page is the right one and claims
        // to have the data; otherwise report and move on.
        Err(errs) if !(x.is_about() && x.claims_data()) => Ok(Some((x, serde_json::from_str(&raw)?, Err(errs), false))),
        Err(errs) => {
            trail.note(format!("validation failed: {}", errs.join("; ")));
            let retry = format!("{prompt}\n\nYour previous answer was:\n{raw}\n\nIt had these problems:\n- {}\n\nAnswer again, fixing these problems. Copy dates exactly as stated on the page.", errs.join("\n- "));
            let Some((x2, raw2)) = ask::<X>(ctx, trail, SYSTEM_PROMPT, &retry)? else {
                return Ok(Some((x, serde_json::from_str(&raw)?, Err(errs), true)));
            };
            let v2 = validate(&x2);
            if let Err(e) = &v2 {
                trail.note(format!("validation failed again: {}", e.join("; ")));
            }
            Ok(Some((x2, serde_json::from_str(&raw2)?, v2, true)))
        }
    }
}

pub fn cfp_prompt(cfg: &ConferenceCfg, year: i32, md: &str) -> String {
    format!(
        "Conference: {} {year}. Track: {} (the main research-paper track; ignore workshops, co-located events, artifact evaluation, camera-ready and revision deadlines). When the page lists dates for several tracks or events, use only the entries that name the {} track and read each date from the same entry as its label.\n\nPAGE:\n{md}",
        cfg.conference, cfg.track, cfg.track
    )
}

/// Extraction + validation + one corrective retry for a CFP page. Returns
/// the last raw answer, the validation result and whether a retry happened.
pub fn extract_cfp(llm: &Llm, cfg: &ConferenceCfg, year: i32, md: &str) -> Result<Option<(CfpExtraction, serde_json::Value, Result<Cfp, Vec<String>>, bool, Trail)>> {
    let ctx = Ctx { llm, searcher: &crate::search::Searcher::new(vec![]), prior_urls: vec![] };
    let mut trail = Trail::default();
    let prompt = cfp_prompt(cfg, year, md);
    Ok(extract_validated::<CfpExtraction, Cfp>(&ctx, &mut trail, &prompt, |x| schema::validate_cfp(x, year, md))?.map(|(x, raw, v, r)| (x, raw, v, r, trail)))
}

/// Extraction + validation + one corrective retry for a volunteer page, for
/// the harness (the pipeline goes through `try_volunteer_page`).
pub fn extract_volunteer(llm: &Llm, cfg: &ConferenceCfg, year: i32, site: Option<&str>, md: &str) -> Result<Option<(VolunteerExtraction, serde_json::Value, Result<Option<Volunteer>, Vec<String>>, bool, Trail)>> {
    let ctx = Ctx { llm, searcher: &crate::search::Searcher::new(vec![]), prior_urls: vec![] };
    let mut trail = Trail::default();
    let prompt = volunteer_prompt(cfg, year, site, md);
    Ok(extract_validated::<VolunteerExtraction, Option<Volunteer>>(&ctx, &mut trail, &prompt, |x| schema::validate_volunteer(x, year, md, None))?.map(|(x, raw, v, r)| (x, raw, v, r, trail)))
}

/// Try one page for the CFP, following at most two links. Conference dates
/// found on the way are kept in `acc`; `Some` is returned only once
/// deadlines were found.
#[allow(clippy::too_many_arguments)]
fn try_cfp_page(ctx: &Ctx, cfg: &ConferenceCfg, year: i32, trail: &mut Trail, url: &str, hops: u32, saw_invalid: &mut bool, acc: &mut CfpFound) -> Result<Option<Found<Deadlines>>> {
    let Some(page) = fetch_page(trail, url) else { return Ok(None) };
    let md = clean::html_to_markdown(&page.html);
    log::info!("page {} ({} chars, ~{} tokens, hops {hops})", page.url, md.len(), clean::estimate_tokens(&md));
    if md.len() < MIN_PAGE_CHARS {
        trail.note(format!("{} has almost no text ({} chars); skipped", page.url, md.len()));
        return Ok(None);
    }
    let links = clean::links(&page.html, &page.url);
    let grounding = grounding_text(&md, &links);
    let prompt = cfp_prompt(cfg, year, &md);
    let Some((x, raw, validated, retried)) = extract_validated::<CfpExtraction, Cfp>(ctx, trail, &prompt, |x| schema::validate_cfp(x, year, &grounding))? else { return Ok(None) };
    if !x.page_is_about_conference {
        trail.note(format!("{} is not about {}", page.url, cfg.label(year)));
        return Ok(None);
    }
    match validated {
        Ok(mut cfp) => {
            if hops > 0 {
                trail.code(Code::E003);
            }
            if retried {
                trail.code(Code::E005);
            }
            let prov = Provenance { source_url: page.url.clone(), query: trail.query.clone(), backend: trail.backend.clone(), hops, via_chrome: page.via_chrome, retried, codes: trail.codes.clone() };
            if let (Some(c), None) = (&cfp.conference, &acc.conference) {
                acc.conference = Some(Found { value: c.clone(), raw: raw.clone(), prov: prov.clone(), html: page.html.clone(), md: md.clone() });
            }
            if !cfp.rounds.is_empty() {
                if cfp.submission_url.is_none() {
                    cfp.submission_url = pick_link(ctx, trail, &links, &page.url, &format!("the submission system where authors upload their papers for {} {year} ({} track); not a call-for-papers or information page, and not a sign-in or account page of the conference website", cfg.conference, cfg.track))?;
                }
                let deadlines = cfp.deadlines().expect("rounds present");
                return Ok(Some(Found { value: deadlines, raw, prov, html: page.html.clone(), md }));
            }
            trail.note(format!("{} has the conference dates but no deadlines yet", page.url));
        }
        Err(_) if x.has_submission_deadline && !x.rounds.is_empty() => {
            *saw_invalid = true;
            return Ok(None);
        }
        Err(_) => {}
    }
    if hops >= 2 {
        return Ok(None);
    }
    let what = format!("the call for papers or important dates of the {} track of {} {year}", cfg.track, cfg.conference);
    // Up to two picks per page: a failed pick is excluded from the second.
    for _ in 0..2 {
        match follow_link(ctx, trail, &page.html, &page.url, &what, &[&cfg.track, &cfg.conference])? {
            Some(next) if !trail.tried(&next) => {
                trail.note(format!("following link {next}"));
                if let Some(found) = try_cfp_page(ctx, cfg, year, trail, &next, hops + 1, saw_invalid, acc)? {
                    return Ok(Some(found));
                }
                trail.mark_tried(&next);
            }
            _ => break,
        }
    }
    Ok(None)
}

/// Find (or re-validate) the call for papers: conference dates and paper
/// deadlines. `known_url` is the page the stored data came from; it is
/// tried first and a web search only happens when it is gone or no longer
/// yields deadlines. The result is `Ok` when at least one part was found.
pub fn find_cfp(ctx: &Ctx, cfg: &ConferenceCfg, year: i32, known_url: Option<&str>) -> Result<Attempted<CfpFound>> {
    let mut trail = Trail::default();
    let mut saw_invalid = false;
    let mut acc = CfpFound::default();
    let finish = |acc: CfpFound, trail: Trail, saw_invalid: bool| {
        if acc.conference.is_some() || acc.deadlines.is_some() {
            Ok(Attempted { result: Ok(acc), trail })
        } else {
            Ok(Attempted { result: Err(if saw_invalid { Miss::Invalid } else { Miss::NotFound }), trail })
        }
    };
    if let Some(u) = known_url {
        trail.query = format!("stored URL {u}");
        trail.note(format!("re-checking stored page {u}"));
        if let Some(found) = try_cfp_page(ctx, cfg, year, &mut trail, u, 0, &mut saw_invalid, &mut acc)? {
            acc.deadlines = Some(found);
            return finish(acc, trail, saw_invalid);
        }
        trail.note("stored page yields no deadlines; searching");
    }
    let same = cfg.track.eq_ignore_ascii_case(&cfg.conference);
    let queries = if same {
        vec![format!("{} {year} call for papers", cfg.conference), format!("{} {year}", cfg.conference)]
    } else {
        vec![format!("{} {year} {} call for papers", cfg.conference, cfg.track), format!("{} {year}", cfg.conference)]
    };
    trail.query = queries[0].clone();
    let hits = gather_hits(ctx, cfg, year, &queries, &mut trail)?;
    if hits.is_empty() {
        trail.note("no candidate pages");
        return finish(acc, trail, saw_invalid);
    }
    let what = format!("the official website or call for papers of {} {year}", cfg.conference);
    let hits = ordered_hits(ctx, &mut trail, hits, &what)?;
    for h in hits.iter().take(3) {
        if let Some(found) = try_cfp_page(ctx, cfg, year, &mut trail, &h.url, 0, &mut saw_invalid, &mut acc)? {
            acc.deadlines = Some(found);
            return finish(acc, trail, saw_invalid);
        }
    }
    if acc.conference.is_some() {
        trail.note("only conference dates found so far");
    }
    finish(acc, trail, saw_invalid)
}

/// `site` is the conference's own website (host of the page its dates came
/// from). It lets the model tell the conference apart from namesakes: a page
/// on another site is about this conference only if it refers to this event.
pub fn volunteer_prompt(cfg: &ConferenceCfg, year: i32, site: Option<&str>, md: &str) -> String {
    let site_note = site.map(|s| format!(" The conference's own website is {s}; a page elsewhere is about this conference only if it explicitly refers to this event.")).unwrap_or_default();
    format!("Conference: {} {year} (the academic conference; its {} track has paper deadlines).{site_note} Topic: the student volunteer programme (students helping at the conference), not paper submissions. Only report a deadline if the page states one.\n\nPAGE:\n{md}", cfg.conference, cfg.track)
}

/// `https://host` of a URL, for prompts.
pub fn site_of(url: &str) -> Option<String> {
    let u = url::Url::parse(url).ok()?;
    Some(format!("{}://{}", u.scheme(), u.host_str()?))
}

#[allow(clippy::too_many_arguments)]
fn try_volunteer_page(ctx: &Ctx, cfg: &ConferenceCfg, year: i32, trail: &mut Trail, url: &str, hops: u32, saw: &mut (bool, bool), conference_end: Option<chrono::NaiveDate>, site: Option<&str>) -> Result<Option<Found<Volunteer>>> {
    let Some(page) = fetch_page(trail, url) else { return Ok(None) };
    let md = clean::html_to_markdown(&page.html);
    log::info!("page {} ({} chars, hops {hops})", page.url, md.len());
    if md.len() < MIN_PAGE_CHARS {
        trail.note(format!("{} has almost no text ({} chars); skipped", page.url, md.len()));
        return Ok(None);
    }
    let links = clean::links(&page.html, &page.url);
    let grounding = grounding_text(&md, &links);
    let prompt = volunteer_prompt(cfg, year, site, &md);
    let Some((x, raw, validated, retried)) = extract_validated::<VolunteerExtraction, Option<Volunteer>>(ctx, trail, &prompt, |x| schema::validate_volunteer(x, year, &grounding, conference_end))? else { return Ok(None) };
    if !x.page_is_about_conference {
        trail.note(format!("{} is not about {}", page.url, cfg.label(year)));
        return Ok(None);
    }
    if x.has_volunteer_program {
        saw.0 = true;
        match validated {
            Ok(Some(mut v)) => {
                if hops > 0 {
                    trail.code(Code::E003);
                }
                if retried {
                    trail.code(Code::E005);
                }
                if v.application_url.is_none() {
                    v.application_url = pick_link(ctx, trail, &links, &page.url, &format!("the application form or sign-up page where students apply to be student volunteers at {} {year}; not a general information page", cfg.conference))?;
                }
                let prov = Provenance { source_url: page.url.clone(), query: trail.query.clone(), backend: trail.backend.clone(), hops, via_chrome: page.via_chrome, retried, codes: trail.codes.clone() };
                return Ok(Some(Found { value: v, raw, prov, html: page.html.clone(), md }));
            }
            Ok(None) => {
                trail.note(format!("{} describes a volunteer programme but states no deadline", page.url));
            }
            Err(_) => {
                saw.1 = true;
                return Ok(None);
            }
        }
    }
    if hops >= 2 {
        return Ok(None);
    }
    let what = format!("the page explaining how students apply to be student volunteers at {} {year}", cfg.conference);
    for _ in 0..2 {
        match follow_link(ctx, trail, &page.html, &page.url, &what, &["volunteer"])? {
            Some(next) if !trail.tried(&next) => {
                trail.note(format!("following link {next}"));
                if let Some(found) = try_volunteer_page(ctx, cfg, year, trail, &next, hops + 1, saw, conference_end, site)? {
                    return Ok(Some(found));
                }
                trail.mark_tried(&next);
            }
            _ => break,
        }
    }
    Ok(None)
}

/// `known_url` is the volunteers page the stored data came from (tried
/// first). `hint_url` is the CFP page: its links usually include the
/// volunteers page, so it is tried next and a web search only happens if
/// that fails.
/// `site` is the conference's own website when one is known (see
/// `main::own_site`); a shared host such as conf.researchr.org is never
/// passed here, since it would mark the real volunteers page as "elsewhere".
pub fn find_volunteer(ctx: &Ctx, cfg: &ConferenceCfg, year: i32, known_url: Option<&str>, hint_url: Option<&str>, conference_end: Option<chrono::NaiveDate>, site: Option<&str>) -> Result<Attempted<Found<Volunteer>>> {
    let mut trail = Trail::default();
    // (has_program seen, invalid seen)
    let mut saw = (false, false);
    if let Some(u) = known_url {
        trail.query = format!("stored URL {u}");
        trail.note(format!("re-checking stored page {u}"));
        if let Some(found) = try_volunteer_page(ctx, cfg, year, &mut trail, u, 0, &mut saw, conference_end, site)? {
            return Ok(Attempted { result: Ok(found), trail });
        }
    }
    if let Some(h) = hint_url {
        trail.query = format!("links of {h}");
        if let Some(page) = fetch_page(&mut trail, h) {
            let what = format!("the page explaining how students apply to be student volunteers at {} {year}", cfg.conference);
            for _ in 0..2 {
                let Some(next) = follow_link(ctx, &mut trail, &page.html, &page.url, &what, &["volunteer"])? else { break };
                if trail.tried(&next) {
                    break;
                }
                trail.note(format!("trying volunteer link {next} from the conference page"));
                if let Some(found) = try_volunteer_page(ctx, cfg, year, &mut trail, &next, 0, &mut saw, conference_end, site)? {
                    return Ok(Attempted { result: Ok(found), trail });
                }
                if saw.0 {
                    let miss = if saw.1 { Miss::Invalid } else { Miss::NoProgram };
                    return Ok(Attempted { result: Err(miss), trail });
                }
                trail.mark_tried(&next);
            }
        }
    }
    let queries = vec![format!("{} {year} student volunteers", cfg.conference)];
    trail.query = queries[0].clone();
    let mut hits = gather_hits(ctx, cfg, year, &queries, &mut trail)?;
    // Pages on the conference's own site first: namesakes (a school
    // programme called "Splash", a company called "CAV") live elsewhere.
    if let Some(s) = site {
        hits.sort_by_key(|h| !h.url.starts_with(s));
    }
    let site_note = site.map(|s| format!(" The conference's own website is {s}.")).unwrap_or_default();
    let what = format!("the page explaining how students apply to be student volunteers at the {} {year} academic conference (not a list of committee members or volunteers' names, and not an unrelated event with a similar name).{site_note}", cfg.conference);
    let hits = ordered_hits(ctx, &mut trail, hits, &what)?;
    for h in hits.iter().take(3) {
        if let Some(found) = try_volunteer_page(ctx, cfg, year, &mut trail, &h.url, 0, &mut saw, conference_end, site)? {
            return Ok(Attempted { result: Ok(found), trail });
        }
    }
    let miss = if saw.1 { Miss::Invalid } else if saw.0 { Miss::NoProgram } else { Miss::NotFound };
    Ok(Attempted { result: Err(miss), trail })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relevance_filter_needs_conference_or_track_mention() {
        let cfg = ConferenceCfg { conference: "SPLASH".into(), track: "OOPSLA".into(), since: 2026 };
        let ok = Hit { title: "Call for Papers".into(), url: "https://2026.splashcon.org/track/oopsla-2026".into(), snippet: String::new() };
        let junk = Hit { title: "Speed test".into(), url: "https://speedtest.example.com/".into(), snippet: "fast internet".into() };
        assert!(relevant_hit(&cfg, &ok));
        assert!(!relevant_hit(&cfg, &junk));
        let cav = ConferenceCfg { conference: "CAV".into(), track: "CAV".into(), since: 2026 };
        let excavation = Hit { title: "Excavation services".into(), url: "https://digging.example.com/".into(), snippet: "caveat emptor".into() };
        let real = Hit { title: "CAV 2026".into(), url: "https://conferences.i-cav.org/2026/".into(), snippet: String::new() };
        assert!(!relevant_hit(&cav, &excavation));
        assert!(relevant_hit(&cav, &real));
    }

    #[test]
    fn derives_next_year_urls_from_prior_editions() {
        let prior = vec![
            (2025, "https://popl25.sigplan.org/track/POPL-2025-popl-research-papers".to_string()),
            (2024, "https://popl24.sigplan.org/track/POPL-2024-popl-research-papers".to_string()),
        ];
        let d = derived_urls(&prior, 2026);
        assert_eq!(d[0], "https://popl26.sigplan.org/track/POPL-2026-popl-research-papers");
        assert_eq!(d[1], "https://popl26.sigplan.org/");
        assert!(d.contains(&"https://popl25.sigplan.org/track/POPL-2026-popl-research-papers".to_string()));
        let d = derived_urls(&[(2025, "https://2025.splashcon.org/track/oopsla-2025".into())], 2026);
        assert_eq!(d[0], "https://2026.splashcon.org/track/oopsla-2026");
        let d = derived_urls(&[(2025, "https://etaps.org/2025/conferences/esop/".into())], 2026);
        assert_eq!(d[0], "https://etaps.org/2026/conferences/esop/");
    }
}
