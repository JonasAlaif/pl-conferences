//! HTTP fetching with a headless-Chrome fallback for JavaScript-only pages.

use anyhow::{Context, Result, anyhow};
use std::process::{Command, Stdio};
use std::sync::LazyLock;
use std::time::{Duration, Instant};

pub const USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/128.0.0.0 Safari/537.36 pl-conferences/1.0 (+https://github.com/JonasAlaif/pl-conferences)";

static CLIENT: LazyLock<reqwest::blocking::Client> = LazyLock::new(|| {
    reqwest::blocking::Client::builder()
        .user_agent(USER_AGENT)
        .timeout(Duration::from_secs(40))
        .redirect(reqwest::redirect::Policy::limited(10))
        .build()
        .expect("http client")
});

/// Used only after a certificate error: academic hosts with expired or
/// self-signed certificates are common, and we only read public pages.
static INSECURE_CLIENT: LazyLock<reqwest::blocking::Client> = LazyLock::new(|| {
    reqwest::blocking::Client::builder()
        .user_agent(USER_AGENT)
        .timeout(Duration::from_secs(40))
        .redirect(reqwest::redirect::Policy::limited(10))
        .danger_accept_invalid_certs(true)
        .build()
        .expect("http client")
});

/// `PLC_FIXTURE_MAP=<json file>`: a map from URL to a recorded HTML file
/// (paths relative to the map file). With it set, every fetch is served
/// from the map and any other URL fails, so the discovery code (link
/// following, borrowing a submission site from a neighbouring page) can be
/// exercised offline on recorded pages with the real model. See the
/// discovery scenarios in tests/live.rs.
static FIXTURES: LazyLock<Option<std::collections::HashMap<String, std::path::PathBuf>>> = LazyLock::new(|| {
    let path = std::path::PathBuf::from(std::env::var("PLC_FIXTURE_MAP").ok()?);
    let text = std::fs::read_to_string(&path).ok()?;
    let map: std::collections::HashMap<String, String> = serde_json::from_str(&text).ok()?;
    let dir = path.parent().map(std::path::Path::to_path_buf).unwrap_or_default();
    Some(map.into_iter().map(|(u, f)| (u.trim_end_matches('/').to_string(), dir.join(f))).collect())
});

fn fixture_page(url: &str) -> Option<Result<Page>> {
    let map = FIXTURES.as_ref()?;
    Some(match map.get(url.trim_end_matches('/')) {
        Some(file) => std::fs::read_to_string(file).map(|html| Page { url: url.to_string(), html, via_chrome: false, insecure_tls: false }).with_context(|| format!("fixture {}", file.display())),
        None => Err(anyhow!("{url} is not in the fixture map (offline mode)")),
    })
}

#[derive(Debug, Clone)]
pub struct Page {
    /// Final URL after redirects.
    pub url: String,
    pub html: String,
    /// True when headless Chrome had to render the page.
    pub via_chrome: bool,
    /// True when the page could only be fetched by ignoring a bad TLS certificate.
    pub insecure_tls: bool,
}

pub fn client() -> &'static reqwest::blocking::Client {
    &CLIENT
}

/// GET a page as text. Non-2xx statuses are errors. Follows one
/// `<meta http-equiv="refresh">` redirect and retries once without
/// certificate verification after a TLS error.
pub fn get(url: &str) -> Result<Page> {
    get_inner(url, 0)
}

fn get_inner(url: &str, depth: u32) -> Result<Page> {
    let (resp, insecure) = match CLIENT.get(url).send() {
        Ok(r) => (r, false),
        Err(e) if is_cert_error(&e) => {
            log::warn!("{url}: certificate error ({e}); retrying without verification");
            (INSECURE_CLIENT.get(url).send().with_context(|| format!("GET {url}"))?, true)
        }
        Err(e) => return Err(anyhow::Error::new(e).context(format!("GET {url}"))),
    };
    let status = resp.status();
    let final_url = resp.url().to_string();
    if !status.is_success() {
        return Err(anyhow!("GET {url}: HTTP {status}"));
    }
    let is_pdf = resp.headers().get("content-type").and_then(|v| v.to_str().ok()).is_some_and(|t| t.contains("pdf"));
    let bytes = resp.bytes().with_context(|| format!("reading body of {url}"))?;
    if is_pdf || bytes.starts_with(b"%PDF") {
        // A call for papers published as a PDF: render its text as a
        // minimal HTML page so the rest of the pipeline is unchanged.
        let text = pdf_text(&bytes).with_context(|| format!("{final_url}: PDF without pdftotext available"))?;
        let html = format!("<html><body><pre>{}</pre></body></html>", text.replace('&', "&amp;").replace('<', "&lt;"));
        return Ok(Page { url: final_url, html, via_chrome: false, insecure_tls: insecure });
    }
    let html = String::from_utf8_lossy(&bytes).into_owned();
    if depth < 2 {
        if let Some(target) = meta_refresh(&html, &final_url) {
            if target != final_url {
                log::info!("{final_url}: meta refresh to {target}");
                let mut p = get_inner(&target, depth + 1)?;
                p.insecure_tls |= insecure;
                return Ok(p);
            }
        }
    }
    Ok(Page { url: final_url, html, via_chrome: false, insecure_tls: insecure })
}

/// Text of a PDF via `pdftotext -layout` (poppler), which the workflow installs.
fn pdf_text(bytes: &[u8]) -> Result<String> {
    use std::io::Write;
    let mut child = Command::new("pdftotext")
        .args(["-layout", "-", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .context("pdftotext not found")?;
    child.stdin.take().context("pdftotext stdin")?.write_all(bytes)?;
    let out = child.wait_with_output()?;
    if !out.status.success() {
        return Err(anyhow!("pdftotext failed"));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

fn is_cert_error(e: &reqwest::Error) -> bool {
    let mut cur: Option<&(dyn std::error::Error + 'static)> = Some(e);
    while let Some(err) = cur {
        let s = err.to_string().to_lowercase();
        if s.contains("certificate") || s.contains("invalid peer") || s.contains("unknownissuer") || s.contains("handshake") {
            return true;
        }
        cur = err.source();
    }
    false
}

/// Target of a `<meta http-equiv="refresh" content="0; url=...">`, absolute.
pub fn meta_refresh(html: &str, base: &str) -> Option<String> {
    // Only the head matters; cut at a char boundary.
    let mut end = html.len().min(20_000);
    while !html.is_char_boundary(end) {
        end -= 1;
    }
    let doc = scraper::Html::parse_document(&html[..end]);
    let sel = scraper::Selector::parse("meta[http-equiv]").ok()?;
    for m in doc.select(&sel) {
        if !m.value().attr("http-equiv").is_some_and(|v| v.eq_ignore_ascii_case("refresh")) {
            continue;
        }
        // A malformed refresh must not stop the scan: another may be valid.
        let Some(content) = m.value().attr("content") else { continue };
        let Some((_, rest)) = content.split_once(';') else { continue };
        // "0; url=target", "0;URL='target'" or just "0;target".
        let rest = rest.trim();
        let target = match rest.get(..3).filter(|p| p.eq_ignore_ascii_case("url")) {
            Some(_) => rest[3..].trim_start().strip_prefix('=').unwrap_or(""),
            None => rest,
        };
        let target = target.trim().trim_matches('"').trim_matches('\'');
        if target.is_empty() {
            continue;
        }
        let abs = url::Url::parse(base).ok()?.join(target).ok()?;
        return Some(abs.to_string());
    }
    None
}

/// GET a page and, if it looks like an empty JavaScript shell, render it with
/// headless Chrome when one is installed.
pub fn get_rendered(url: &str) -> Result<Page> {
    if let Some(fixture) = fixture_page(url) {
        return fixture;
    }
    let page = get(url)?;
    let force = std::env::var("PLC_FORCE_CHROME").is_ok_and(|v| v == "1");
    if !force && !looks_js_only(&page.html) {
        return Ok(page);
    }
    log::info!("{url} looks JavaScript-rendered; trying headless Chrome");
    match chrome_dump(&page.url) {
        Ok(html) if !html.trim().is_empty() => Ok(Page { url: page.url, html, via_chrome: true, insecure_tls: page.insecure_tls }),
        Ok(_) => Ok(page),
        Err(e) => {
            log::warn!("headless Chrome unavailable: {e:#}");
            Ok(page)
        }
    }
}

/// Heuristic: almost no visible text but scripts present.
pub fn looks_js_only(html: &str) -> bool {
    let text = crate::clean::html_to_markdown(html);
    text.len() < 400 && html.contains("<script")
}

const CHROME_CANDIDATES: &[&str] = &[
    "google-chrome",
    "google-chrome-stable",
    "chromium",
    "chromium-browser",
    "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
];

/// Render a URL with headless Chrome and return the DOM.
///
/// Chrome prints the DOM and should exit, but on some systems helper
/// processes keep it alive; so the output is read as it arrives and the
/// process is stopped once the output has been quiet for a few seconds.
pub fn chrome_dump(url: &str) -> Result<String> {
    use std::io::Read;
    use std::sync::{Arc, Mutex};

    let bin = CHROME_CANDIDATES
        .iter()
        .find(|b| {
            Command::new(b).arg("--version").stdout(Stdio::null()).stderr(Stdio::null()).status().is_ok()
        })
        .ok_or_else(|| anyhow!("no Chrome/Chromium binary found"))?;
    // A private profile: without it Chrome hands the URL to an already
    // running instance and never returns.
    let profile = std::env::temp_dir().join(format!("plc-chrome-{}", std::process::id()));
    let mut child = Command::new(bin)
        .args([
            "--headless=new",
            "--disable-gpu",
            "--no-sandbox",
            "--no-first-run",
            "--no-default-browser-check",
            "--disable-extensions",
            "--disable-component-update",
            "--disable-background-networking",
            "--disable-sync",
            &format!("--user-data-dir={}", profile.display()),
            "--virtual-time-budget=8000",
            "--timeout=30000",
            &format!("--user-agent={USER_AGENT}"),
            "--dump-dom",
            url,
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .context("spawning Chrome")?;
    let mut stdout = child.stdout.take().context("no stdout")?;
    let buf: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(Vec::new()));
    let buf2 = Arc::clone(&buf);
    std::thread::spawn(move || {
        let mut chunk = [0u8; 8192];
        while let Ok(n) = stdout.read(&mut chunk) {
            if n == 0 {
                break;
            }
            buf2.lock().unwrap().extend_from_slice(&chunk[..n]);
        }
    });
    let start = Instant::now();
    let mut last_len = 0;
    let mut last_growth = Instant::now();
    let result = loop {
        if child.try_wait()?.is_some() {
            break Ok(());
        }
        let len = buf.lock().unwrap().len();
        if len != last_len {
            last_len = len;
            last_growth = Instant::now();
        }
        if len > 0 && last_growth.elapsed() > Duration::from_secs(3) {
            let _ = child.kill();
            break Ok(());
        }
        if start.elapsed() > Duration::from_secs(45) {
            let _ = child.kill();
            break if len > 0 { Ok(()) } else { Err(anyhow!("Chrome timed out")) };
        }
        std::thread::sleep(Duration::from_millis(200));
    };
    let _ = child.wait();
    let _ = std::fs::remove_dir_all(&profile);
    result?;
    let bytes = std::mem::take(&mut *buf.lock().unwrap());
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn meta_refresh_is_followed_relative_and_absolute() {
        let h = r#"<html><head><meta http-equiv="Refresh" content="0; URL='new/page.html'"></head></html>"#;
        assert_eq!(meta_refresh(h, "https://x.org/old/").as_deref(), Some("https://x.org/old/new/page.html"));
        let h = r#"<meta content="5;url=https://y.org/" http-equiv="refresh">"#;
        assert_eq!(meta_refresh(h, "https://x.org/").as_deref(), Some("https://y.org/"));
        assert_eq!(meta_refresh("<html><meta name=viewport></html>", "https://x.org/"), None);
        // No "url=" at all: the target starts with u/r/l and must survive intact.
        assert_eq!(meta_refresh(r#"<meta http-equiv="refresh" content="0;register.html">"#, "https://x.org/a/").as_deref(), Some("https://x.org/a/register.html"));
        assert_eq!(meta_refresh(r#"<meta http-equiv="refresh" content="0;upcoming.html">"#, "https://x.org/a/").as_deref(), Some("https://x.org/a/upcoming.html"));
        // A malformed refresh does not hide a valid one after it.
        assert_eq!(meta_refresh(r#"<meta http-equiv="refresh"><meta http-equiv="refresh" content="5"><meta http-equiv="refresh" content="0;url=https://y.org/">"#, "https://x.org/").as_deref(), Some("https://y.org/"));
        // A multibyte character straddling the 20 000-byte cut must not panic.
        let big = format!("<html><head></head><body>{}</body></html>", "–".repeat(9_000));
        assert_eq!(meta_refresh(&big, "https://x.org/"), None);
    }
}
