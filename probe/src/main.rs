//! Temporary diagnostic: which property of the pipeline's HTTP client makes
//! DuckDuckGo answer with its challenge page. One thing varies per request.

use std::time::Duration;

const HONEST: &str = "pl-conferences/1.0 (+https://github.com/JonasAlaif/pl-conferences)";
const CHROME: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/154.0.0.0 Safari/537.36";

fn client(native: bool, http1: bool, ua: &str) -> reqwest::blocking::Client {
    let mut b = reqwest::blocking::Client::builder().user_agent(ua).timeout(Duration::from_secs(40));
    b = if native { b.tls_backend_native() } else { b.tls_backend_rustls() };
    if http1 {
        b = b.http1_only();
    }
    b.build().expect("client")
}

fn main() {
    let queries = ["POPL 2027 student volunteers", "CAV 2027 call for papers", "LICS 2027 call for papers"];
    let variants: [(&str, bool, bool, &str); 6] = [
        ("rustls h2 chrome-ua (production)", false, false, CHROME),
        ("rustls h2 honest-ua", false, false, HONEST),
        ("rustls http1 honest-ua", false, true, HONEST),
        ("native-tls h2 honest-ua", true, false, HONEST),
        ("native-tls http1 honest-ua", true, true, HONEST),
        ("native-tls h2 chrome-ua", true, false, CHROME),
    ];
    for q in queries {
        for (name, native, http1, ua) in variants {
            let url = format!("https://html.duckduckgo.com/html/?q={}", q.replace(' ', "+"));
            let r = client(native, http1, ua).get(&url).header("Accept", "text/html").header("Accept-Language", "en-US,en;q=0.9").send();
            match r {
                Ok(resp) => {
                    let (status, version) = (resp.status(), resp.version());
                    let body = resp.text().unwrap_or_default();
                    println!(
                        "== {name:34} | {q:30} | {status} {version:?} bytes={} results={} challenge={}",
                        body.len(),
                        body.matches("result__a").count(),
                        body.contains("bots use DuckDuckGo")
                    );
                }
                Err(e) => println!("== {name:34} | {q:30} | error {e:#}"),
            }
            std::thread::sleep(Duration::from_secs(6));
        }
    }
}
