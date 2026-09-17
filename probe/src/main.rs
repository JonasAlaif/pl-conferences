//! Temporary diagnostic: which property of the pipeline's HTTP client makes
//! DuckDuckGo answer with its challenge page. `ddg-probe <rustls|native>
//! <h2|http1> <honest|chrome> [bare]` asks three queries with one client;
//! `bare` leaves out the Accept and Accept-Language headers.

use std::time::Duration;

const HONEST: &str = "pl-conferences/1.0 (+https://github.com/JonasAlaif/pl-conferences)";
const CHROME: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/154.0.0.0 Safari/537.36";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let has = |s: &str| args.iter().any(|a| a == s);
    let mut b = reqwest::blocking::Client::builder().user_agent(if has("chrome") { CHROME } else { HONEST }).timeout(Duration::from_secs(40));
    b = if has("native") { b.tls_backend_native() } else { b.tls_backend_rustls() };
    if has("http1") {
        b = b.http1_only();
    }
    let client = b.build().expect("client");
    for q in ["POPL 2027 student volunteers", "CAV 2027 call for papers", "LICS 2027 call for papers"] {
        let url = format!("https://html.duckduckgo.com/html/?q={}", q.replace(' ', "+"));
        let mut req = client.get(&url);
        if !has("bare") {
            req = req.header("Accept", "text/html").header("Accept-Language", "en-US,en;q=0.9");
        }
        match req.send() {
            Ok(resp) => {
                let (status, version) = (resp.status(), resp.version());
                let body = resp.text().unwrap_or_default();
                println!("== reqwest {:28} | {status} {version:?} results={} challenge={}", args.join(" "), body.matches("result__a").count(), body.contains("bots use DuckDuckGo"));
            }
            Err(e) => println!("== reqwest {:28} | error {e:#}", args.join(" ")),
        }
        std::thread::sleep(Duration::from_secs(6));
    }
}
