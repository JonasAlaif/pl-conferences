//! Web search through plain HTML result pages, no API keys. Several engines
//! sit behind one trait so that one dying or changing its markup only costs a
//! maintenance code, not the pipeline.

use crate::fetch;
use anyhow::{Result, anyhow};
use scraper::{Html, Selector};
use std::sync::Mutex;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hit {
    pub title: String,
    pub url: String,
    pub snippet: String,
}

pub trait SearchBackend: Send + Sync {
    fn name(&self) -> &'static str;
    /// URL of the results page for a query.
    fn url(&self, query: &str) -> String;
    /// Parse a results page.
    fn parse(&self, html: &str) -> Vec<Hit>;
}

fn sel(s: &str) -> Selector {
    Selector::parse(s).expect("valid selector")
}

fn text_of(el: scraper::ElementRef) -> String {
    el.text().collect::<String>().split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Generic parser: one selector for result containers, then relative
/// selectors for the link, title and snippet inside each.
fn parse_with(html: &str, container: &str, link: &str, title: Option<&str>, snippet: &str, fix_url: fn(&str) -> Option<String>) -> Vec<Hit> {
    let doc = Html::parse_document(html);
    let (c, l, s) = (sel(container), sel(link), sel(snippet));
    let t = title.map(sel);
    let mut hits = vec![];
    for r in doc.select(&c) {
        let Some(a) = r.select(&l).next() else { continue };
        let Some(href) = a.value().attr("href") else { continue };
        let Some(url) = fix_url(href) else { continue };
        let title = match &t {
            Some(t) => r.select(t).next().map(text_of).unwrap_or_default(),
            None => text_of(a),
        };
        let snippet = r.select(&s).next().map(text_of).unwrap_or_default();
        if title.is_empty() && snippet.is_empty() {
            continue;
        }
        hits.push(Hit { title, url, snippet });
    }
    hits
}

fn plain_http(href: &str) -> Option<String> {
    let u = href.trim();
    u.starts_with("http").then(|| u.to_string())
}

/// DuckDuckGo's HTML endpoint wraps result URLs in a redirect with `uddg=`.
fn ddg_url(href: &str) -> Option<String> {
    let href = if href.starts_with("//") { format!("https:{href}") } else { href.to_string() };
    let u = url::Url::parse(&href).ok()?;
    if let Some((_, v)) = u.query_pairs().find(|(k, _)| k == "uddg") {
        return Some(v.into_owned());
    }
    plain_http(&href)
}

pub struct DdgHtml;
impl SearchBackend for DdgHtml {
    fn name(&self) -> &'static str {
        "ddg-html"
    }
    fn url(&self, q: &str) -> String {
        format!("https://html.duckduckgo.com/html/?q={}", urlencoding::encode(q))
    }
    fn parse(&self, html: &str) -> Vec<Hit> {
        parse_with(html, "div.result", "a.result__a", None, ".result__snippet", ddg_url)
    }
}

pub struct Brave;
impl SearchBackend for Brave {
    fn name(&self) -> &'static str {
        "brave"
    }
    fn url(&self, q: &str) -> String {
        format!("https://search.brave.com/search?q={}&source=web", urlencoding::encode(q))
    }
    fn parse(&self, html: &str) -> Vec<Hit> {
        parse_with(html, "div.snippet", "a[href^=http]", Some(".title"), ".generic-snippet, .snippet-description, .description", plain_http)
    }
}

pub struct Bing;
impl SearchBackend for Bing {
    fn name(&self) -> &'static str {
        "bing"
    }
    fn url(&self, q: &str) -> String {
        format!("https://www.bing.com/search?q={}", urlencoding::encode(q))
    }
    fn parse(&self, html: &str) -> Vec<Hit> {
        parse_with(html, "li.b_algo", "h2 a[href^=http]", None, ".b_caption p, p", bing_url)
    }
}

/// Bing wraps result URLs as `bing.com/ck/a?...&u=a1<base64url of the URL>`.
fn bing_url(href: &str) -> Option<String> {
    use base64::Engine;
    let u = url::Url::parse(href).ok()?;
    if u.host_str().is_some_and(|h| h.ends_with("bing.com")) && u.path() == "/ck/a" {
        let (_, v) = u.query_pairs().find(|(k, _)| k == "u")?;
        let enc = v.strip_prefix("a1").unwrap_or(&v);
        let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(enc).ok()?;
        let s = String::from_utf8(bytes).ok()?;
        return s.starts_with("http").then_some(s);
    }
    plain_http(href)
}

pub fn default_backends() -> Vec<Box<dyn SearchBackend>> {
    // `PLC_NO_SEARCH=1` disables every engine, to exercise the fallbacks.
    if std::env::var("PLC_NO_SEARCH").is_ok_and(|v| v == "1") {
        return vec![];
    }
    // DuckDuckGo first: its results for conference queries are the most
    // relevant. Brave then Bing take over when it does not answer.
    vec![Box::new(DdgHtml), Box::new(Brave), Box::new(Bing)]
}

/// Search engines are asked as what this is: a script, named, with the
/// address of its repository. All three engines answer that.
fn search_user_agent() -> String {
    format!("pl-conferences/{} (+{})", env!("CARGO_PKG_VERSION"), crate::ics::REPO_URL)
}

/// curl writes the status code on a line of its own after the body.
fn split_status(out: &str) -> Option<(u16, String)> {
    let (body, code) = out.trim_end().rsplit_once('\n').unwrap_or(("", out.trim_end()));
    Some((code.trim().parse().ok()?, body.to_string()))
}

/// `Ok(None)` when there is no curl to run.
fn curl_get(url: &str) -> Result<Option<(u16, String)>> {
    let out = std::process::Command::new("curl")
        .args(["-sS", "--compressed", "-L", "--max-time", "40", "-A", &search_user_agent(), "-H", "Accept: text/html", "-H", "Accept-Language: en-US,en;q=0.9", "-w", "\n%{http_code}", url])
        .stdin(std::process::Stdio::null())
        .output();
    let out = match out {
        Ok(out) => out,
        Err(e) => {
            log::warn!("curl could not be run ({e}); searching with the built-in client");
            return Ok(None);
        }
    };
    if !out.status.success() {
        return Err(anyhow!("curl exited with {}: {}", out.status, String::from_utf8_lossy(&out.stderr).trim()));
    }
    split_status(&String::from_utf8_lossy(&out.stdout)).map(Some).ok_or_else(|| anyhow!("curl printed no status code"))
}

/// GET a results page: status and body. Through the system's curl, because
/// of who DuckDuckGo answers: measured from GitHub's runners (one client per
/// fresh runner, September 2026) it met this program's own HTTP client with
/// its "bots use DuckDuckGo too" challenge 36 times out of 36, whatever the
/// TLS library, HTTP version or User-Agent, and served curl 36 times out of
/// 36, including with the User-Agent above. A failed connection is tried
/// once more with curl; the program's own client is only for a machine
/// without curl.
fn get_page(url: &str) -> Result<(u16, String)> {
    let first = curl_get(url).or_else(|e| {
        log::warn!("{e:#}; trying once more");
        std::thread::sleep(Duration::from_secs(3));
        curl_get(url)
    })?;
    if let Some(r) = first {
        return Ok(r);
    }
    let resp = fetch::client().get(url).header("User-Agent", search_user_agent()).header("Accept", "text/html").header("Accept-Language", "en-US,en;q=0.9").send()?;
    Ok((resp.status().as_u16(), resp.text()?))
}

/// How long a backend that throttled us is left alone.
const COOL_DOWN: Duration = Duration::from_secs(15 * 60);

#[derive(Debug, Clone)]
pub struct Failure {
    pub backend: &'static str,
    pub reason: String,
    /// The engine blocked or throttled us, as opposed to answering with nothing usable.
    pub throttled: bool,
}

#[derive(Debug, Clone, Default)]
pub struct SearchOutcome {
    pub hits: Vec<Hit>,
    /// Backend that answered.
    pub backend: Option<&'static str>,
    /// Backends that failed before one answered, with the reason.
    pub failures: Vec<Failure>,
}

impl SearchOutcome {
    pub fn failure_text(&self) -> String {
        self.failures.iter().map(|f| format!("{}: {}", f.backend, f.reason)).collect::<Vec<_>>().join(" | ")
    }
}

/// Tries backends in order, pacing requests so engines do not block the IP.
pub struct Searcher {
    backends: Vec<Box<dyn SearchBackend>>,
    last_request: Mutex<Option<Instant>>,
    /// Backends that throttled us, with the time they may be tried again.
    cooling: Mutex<std::collections::HashMap<&'static str, Instant>>,
    /// Minimum gap between requests to search engines. Monthly volume is a
    /// few dozen queries, so generous spacing costs nothing.
    pub min_gap: Duration,
}

impl Searcher {
    pub fn new(backends: Vec<Box<dyn SearchBackend>>) -> Self {
        let secs = std::env::var("PLC_SEARCH_GAP").ok().and_then(|s| s.parse().ok()).unwrap_or(8);
        Self { backends, last_request: Mutex::new(None), cooling: Mutex::new(Default::default()), min_gap: Duration::from_secs(secs) }
    }

    fn pace(&self) {
        let mut last = self.last_request.lock().unwrap();
        if let Some(t) = *last {
            let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.subsec_nanos()).unwrap_or(0);
            let jitter = Duration::from_millis(u64::from(nanos % 2000));
            let wait = (self.min_gap + jitter).saturating_sub(t.elapsed());
            if !wait.is_zero() {
                std::thread::sleep(wait);
            }
        }
        *last = Some(Instant::now());
    }

    /// Name of the first (primary) backend, if any.
    pub fn primary(&self) -> Option<&'static str> {
        self.backends.first().map(|b| b.name())
    }

    fn fetch_results(&self, b: &dyn SearchBackend, query: &str, relevant: &dyn Fn(&Hit) -> bool) -> Result<Vec<Hit>> {
        let url = b.url(query);
        let mut delay = Duration::from_secs(20);
        for attempt in 0..3 {
            self.pace();
            let (status, html) = get_page(&url)?;
            // Exactly 200: DuckDuckGo's challenge page comes as 202, which
            // is a "success" status too.
            if status == 200 {
                let hits = b.parse(&html);
                if hits.is_empty() {
                    let title = crate::clean::title(&html).unwrap_or_default();
                    return Err(anyhow!("HTTP 200 but no results parsed ({} bytes, title {title:?})", html.len()));
                }
                // Engines that detect automation sometimes answer with
                // unrelated results; treat that like no answer at all.
                let n = hits.len();
                let hits: Vec<Hit> = hits.into_iter().filter(relevant).collect();
                if hits.is_empty() {
                    return Err(anyhow!("{n} results but none relevant"));
                }
                return Ok(hits);
            }
            // 202 (DuckDuckGo's challenge), 429 and 403 are the engine
            // saying no; back off once, then leave this backend alone for a
            // while. A challenge is never answered or worked around.
            if matches!(status, 202 | 403 | 429 | 503) {
                if attempt < 1 {
                    log::warn!("{} returned HTTP {status}; backing off {delay:?}", b.name());
                    std::thread::sleep(delay);
                    delay *= 2;
                    continue;
                }
                self.cooling.lock().unwrap().insert(b.name(), Instant::now() + COOL_DOWN);
                return Err(anyhow!("throttled: HTTP {status} (skipping this backend for {COOL_DOWN:?})"));
            }
            return Err(anyhow!("HTTP {status}"));
        }
        Err(anyhow!("throttled"))
    }

    /// `relevant` is a cheap sanity check on a hit (e.g. mentions the
    /// conference); backends whose results all fail it are skipped.
    pub fn search(&self, query: &str, relevant: &dyn Fn(&Hit) -> bool) -> SearchOutcome {
        let mut out = SearchOutcome::default();
        for b in &self.backends {
            if self.cooling.lock().unwrap().get(b.name()).is_some_and(|t| *t > Instant::now()) {
                out.failures.push(Failure { backend: b.name(), reason: "throttled: cooling down".into(), throttled: true });
                continue;
            }
            match self.fetch_results(b.as_ref(), query, relevant) {
                Ok(hits) => {
                    log::info!("search[{}] {query:?}: {} hits", b.name(), hits.len());
                    out.hits = hits;
                    out.backend = Some(b.name());
                    return out;
                }
                Err(e) => {
                    log::warn!("search[{}] {query:?} failed: {e:#}", b.name());
                    let reason = format!("{e:#}");
                    out.failures.push(Failure { backend: b.name(), throttled: reason.starts_with("throttled"), reason });
                }
            }
        }
        out
    }
}

/// Merge hit lists, dropping duplicates by host + path, keeping order.
pub fn merge(lists: &[Vec<Hit>], max: usize) -> Vec<Hit> {
    let mut seen = std::collections::HashSet::new();
    let mut out = vec![];
    let mut idx = 0;
    // Interleave so that both queries contribute to the top of the list.
    loop {
        let mut any = false;
        for l in lists {
            if let Some(h) = l.get(idx) {
                any = true;
                let key = url::Url::parse(&h.url).map(|u| format!("{}{}", u.host_str().unwrap_or(""), u.path().trim_end_matches('/'))).unwrap_or(h.url.clone());
                if seen.insert(key.to_lowercase()) {
                    out.push(h.clone());
                    if out.len() >= max {
                        return out;
                    }
                }
            }
        }
        if !any {
            return out;
        }
        idx += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> String {
        std::fs::read_to_string(format!("{}/tests/fixtures/search/{name}.html", env!("CARGO_MANIFEST_DIR"))).unwrap()
    }

    #[test]
    fn curl_output_splits_into_status_and_body() {
        assert_eq!(split_status("<html>\nresults\n</html>\n200"), Some((200, "<html>\nresults\n</html>".to_string())));
        assert_eq!(split_status("challenge\n202\n"), Some((202, "challenge".to_string())));
        assert_eq!(split_status("\n429"), Some((429, String::new())), "an empty body still carries its status");
        assert_eq!(split_status("<html>no status</html>"), None);
        assert!(search_user_agent().starts_with("pl-conferences/") && search_user_agent().contains("github.com"), "the engines are told who is asking");
    }

    #[test]
    fn ddg_html_parses_and_unwraps_redirects() {
        let hits = DdgHtml.parse(&fixture("ddg-html"));
        assert!(hits.len() >= 5, "{hits:?}");
        assert!(hits.iter().any(|h| h.url.starts_with("https://popl27.sigplan.org/")), "{hits:?}");
        assert!(hits.iter().all(|h| !h.url.contains("duckduckgo.com/l/")));
        assert!(hits[0].snippet.len() > 10);
    }

    #[test]
    fn brave_parses() {
        let hits = Brave.parse(&fixture("brave"));
        assert!(hits.len() >= 5, "{hits:?}");
        assert!(hits.iter().any(|h| h.url.contains("popl27.sigplan.org")), "{hits:?}");
        assert!(hits.iter().all(|h| !h.url.contains("brave.com")));
    }

    #[test]
    fn bing_parses_and_decodes() {
        let hits = Bing.parse(&fixture("bing"));
        assert!(hits.len() >= 5, "{hits:?}");
        assert!(hits.iter().all(|h| !h.url.contains("bing.com/ck")), "{hits:?}");
        assert!(hits.iter().all(|h| h.url.starts_with("http")));
    }

    #[test]
    fn merge_dedupes_by_host_and_path() {
        let a = vec![Hit { title: "a".into(), url: "https://x.org/p/".into(), snippet: String::new() }];
        let b = vec![Hit { title: "b".into(), url: "https://x.org/p".into(), snippet: String::new() }, Hit { title: "c".into(), url: "https://y.org".into(), snippet: String::new() }];
        let m = merge(&[a, b], 8);
        assert_eq!(m.len(), 2);
    }
}
