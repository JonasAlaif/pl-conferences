//! HTML → compact Markdown for the LLM.
//!
//! Boilerplate (navigation, scripts, media, forms) is dropped and links are
//! reduced to their anchor text. Content is only cut when a page exceeds the
//! budget, and then whole sections are dropped lowest-relevance first (a
//! conference program or accepted-papers list, never the call for papers).

use htmd::{Element, HtmlToMarkdown, element_handler::Handlers};
use regex::Regex;
use std::collections::HashSet;
use std::sync::LazyLock;

/// Tags whose whole subtree is dropped. `<header>` is kept: CAV 2027 puts
/// its dates and venue in a hero block inside `<header>`, and the site
/// chrome such a header carries on researchr pages is navigation, removed
/// by `STRIP_SELECTORS`. (`<aside>` was tried and adds 5-10K chars of
/// chrome on researchr pages.)
const SKIP_TAGS: &[&str] = &[
    "script", "style", "noscript", "template", "svg", "img", "picture", "video", "audio", "iframe",
    "canvas", "form", "input", "button", "select", "textarea", "nav", "footer", "aside",
];

/// Elements removed before conversion. Bootstrap-era class names are common
/// enough to be worth stripping generically; anything unmatched is harmless.
const STRIP_SELECTORS: &str = "[role=navigation], [role=banner], [role=contentinfo], [role=dialog], [role=status], [role=alert], \
    .navbar, .navigationbar, #navigationbar, .nav-menu, .menu-bar, .dropdown-menu, .breadcrumb, \
    .footer, .footer-box, .site-footer, .modal, .cookie, .cookie-banner, .cookie-consent, .skip-link, \
    .sr-only, .visually-hidden, [hidden], [aria-hidden=true]";

/// Upper bound on the text handed to the model (roughly 6K tokens). Prompt
/// processing dominates CPU time, so this is the main speed knob. At this
/// size the budget drops the programme, accepted-paper lists and committee
/// of the big researchr pages and keeps their call for papers and dates
/// sidebar whole (harness 33/33; it was 45K before the section splitter
/// recognised headings inside list items, and 24K then scored 18/33
/// because the sidebar hung off an oversize section and got truncated).
pub const MAX_CHARS: usize = 24_000;
/// Sections scoring at least this are treated as relevant when a page must be cut.
const RELEVANT_SCORE: f64 = 2.0;

/// Relevance signals used only when a page exceeds the budget. These are
/// English: a language-neutral variant (years plus "day number next to a
/// word") was tried and scored the conference programme (times, rooms,
/// "20 min") above the call for papers; boosting mentions of the track name
/// was tried too and let programme listings (which name the track on every
/// talk) fill the budget.
static KEYWORDS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)deadline|submission|submit|notification|notif|call for papers|important dates|rebuttal|author response|camera|aoe|anywhere on earth|round|volunteer|apply|application").unwrap()
});
static DATES: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(jan|feb|mar|apr|may|jun|jul|aug|sep|sept|oct|nov|dec)[a-z]*\.?\s+\d{1,2}\b|\b\d{1,2}\s+(jan|feb|mar|apr|may|jun|jul|aug|sep|sept|oct|nov|dec)[a-z]*\b|\b20\d\d\b").unwrap()
});

/// Convert a page to Markdown suitable for the model.
pub fn html_to_markdown(html: &str) -> String {
    budget(&html_to_markdown_unbudgeted(html), max_chars())
}

/// A `<th>` in a table body is a row's label ("Submission", "Rebuttal" at
/// the start of each row of ETAPS's dates table). The Markdown converter
/// drops header cells outside `<thead>`, and with them the one word that
/// says what the row's dates are; as plain cells they survive.
fn body_row_headers_to_cells(html: &str) -> String {
    static THEAD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?is)<thead\b.*?</thead>").unwrap());
    static TH: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)<(/?)th\b").unwrap());
    let mut out = String::with_capacity(html.len());
    let mut last = 0;
    for m in THEAD.find_iter(html) {
        out.push_str(&TH.replace_all(&html[last..m.start()], "<${1}td"));
        out.push_str(m.as_str());
        last = m.end();
    }
    out.push_str(&TH.replace_all(&html[last..], "<${1}td"));
    out
}

/// `MAX_CHARS`, overridable with `PLC_MAX_CHARS` for experiments.
fn max_chars() -> usize {
    std::env::var("PLC_MAX_CHARS").ok().and_then(|s| s.parse().ok()).unwrap_or(MAX_CHARS)
}

/// Full conversion without the length budget.
pub fn html_to_markdown_unbudgeted(html: &str) -> String {
    let convert = |html: &str| {
        let converter = HtmlToMarkdown::builder()
            .skip_tags(SKIP_TAGS.to_vec())
            // Anchor text only: URLs are pure token waste for extraction.
            .add_handler(vec!["a"], |handlers: &dyn Handlers, el: Element| {
                Some(handlers.walk_children(el.node))
            })
            .build();
        tidy(&converter.convert(html).unwrap_or_default())
    };
    let html = &body_row_headers_to_cells(html);
    let stripped = convert(&strip_elements(html));
    // Safety net: if stripping boilerplate by selector left almost nothing,
    // the selectors hit real content (a site that puts its body in a
    // `.modal`, say); fall back to the unstripped conversion. The absolute
    // floor matters: on researchr pages the navigation alone is several
    // times the content, so a ratio on its own fires on every page there.
    if stripped.len() < 1_500 {
        let full = convert(html);
        if full.len() > 3 * stripped.len() {
            return full;
        }
    }
    stripped
}

/// Remove boilerplate containers by CSS selector and re-serialize.
fn strip_elements(html: &str) -> String {
    let mut doc = scraper::Html::parse_document(html);
    let Ok(sel) = scraper::Selector::parse(STRIP_SELECTORS) else {
        return html.to_string();
    };
    let ids: Vec<_> = doc.select(&sel).map(|e| e.id()).collect();
    for id in ids {
        if let Some(mut node) = doc.tree.get_mut(id) {
            node.detach();
        }
    }
    doc.html()
}

/// Collapse whitespace and drop exact duplicate lines.
pub fn tidy(md: &str) -> String {
    let mut seen: HashSet<String> = HashSet::new();
    let mut out = String::with_capacity(md.len());
    let mut blank_run = 0;
    for raw in md.lines() {
        let line = raw.trim_end();
        let trimmed = line.trim();
        if trimmed.is_empty() {
            blank_run += 1;
            if blank_run == 1 {
                out.push('\n');
            }
            continue;
        }
        blank_run = 0;
        // Only boilerplate repeats verbatim; lines carrying numbers (dates)
        // and table rows are never dropped.
        let key = trimmed.to_string();
        if key.len() > 3 && !key.starts_with('|') && !key.chars().any(|c| c.is_ascii_digit()) && !seen.insert(key) {
            continue;
        }
        // Table rows come column-aligned with long runs of spaces; the
        // padding is pure noise for the model and pushes cells far apart.
        if trimmed.starts_with('|') {
            out.push_str(&compact_row(trimmed));
        } else {
            out.push_str(line);
        }
        out.push('\n');
    }
    out.trim().to_string()
}

/// `| a   |  b |` becomes `| a | b |`; separator rows keep one dash per cell.
fn compact_row(row: &str) -> String {
    let cells: Vec<String> = row
        .trim_matches('|')
        .split('|')
        .map(|c| {
            let c = c.split_whitespace().collect::<Vec<_>>().join(" ");
            if !c.is_empty() && c.chars().all(|ch| matches!(ch, '-' | ':')) { "---".to_string() } else { c }
        })
        .collect();
    format!("| {} |", cells.join(" | "))
}


/// Split at headings; the first element is the preamble.
pub fn sections(md: &str) -> Vec<String> {
    let mut sections: Vec<String> = vec![String::new()];
    for line in md.lines() {
        if is_heading(line) {
            sections.push(String::new());
        }
        let cur = sections.last_mut().unwrap();
        cur.push_str(line);
        cur.push('\n');
    }
    sections
}

/// A Markdown heading, also when it sits inside a list item ("*   #####
/// Name"): researchr renders each committee member that way, and without
/// this the whole committee (and the dates sidebar before it) hung off
/// whatever heading came last, as one oversize section that the budget
/// could only truncate from the tail, where the dates were.
fn is_heading(line: &str) -> bool {
    line.trim_start().trim_start_matches(|c: char| matches!(c, '*' | '-' | '+' | '.' | ' ') || c.is_ascii_digit()).starts_with('#')
}

/// Relevance of a section for date extraction, per unit length.
pub fn score(s: &str) -> f64 {
    let kw = KEYWORDS.find_iter(s).count() as f64;
    let dates = DATES.find_iter(s).count() as f64;
    (5.0 * kw + dates + 1.0) / (s.len() as f64 / 1000.0 + 1.0)
}

/// Keep the whole text when it fits; otherwise keep the preamble plus the
/// most relevant heading-delimited sections that fit, in document order.
/// The result stays within `max` up to the few bytes of the `[…]` markers
/// that mark dropped sections.
pub fn budget(md: &str, max: usize) -> String {
    if md.len() <= max {
        return md.to_string();
    }
    let mut sections = sections(md);
    let mut keep = vec![false; sections.len()];
    let mut used = 0;
    // Preamble (title, tagline with dates/location) is always kept, capped
    // unless it is the whole page (no headings at all).
    let preamble_len = if sections.len() == 1 { sections[0].len().min(max) } else { sections[0].len().min(4000) };
    keep[0] = true;
    used += preamble_len;
    let mut order: Vec<usize> = (1..sections.len()).collect();
    order.sort_by(|&a, &b| score(&sections[b]).partial_cmp(&score(&sections[a])).unwrap());
    for i in order {
        let len = sections[i].len();
        // Relevant sections fill the whole budget; filler (programs, paper
        // lists) is only admitted while the text is still small.
        let cap = if score(&sections[i]) >= RELEVANT_SCORE { max } else { max / 3 };
        if used + len <= cap {
            keep[i] = true;
            used += len;
        } else if cap > used + 8_000 {
            // A relevant section too big for what is left (e.g. a page with
            // no headings at all): keep its head rather than nothing.
            let room = cap - used - 16;
            let cut = truncate_at_boundary(&sections[i], room).len();
            sections[i].truncate(cut);
            sections[i].push_str("\n[…]\n");
            used += sections[i].len();
            keep[i] = true;
        }
    }
    let mut out = String::with_capacity(used + 64);
    let mut dropped = false;
    for (i, s) in sections.iter().enumerate() {
        if keep[i] {
            if dropped {
                out.push_str("[…]\n\n");
                dropped = false;
            }
            if i == 0 {
                out.push_str(truncate_at_boundary(s, preamble_len));
                if preamble_len < s.len() {
                    out.push_str("\n[…]");
                }
                out.push('\n');
            } else {
                out.push_str(s);
            }
        } else {
            dropped = true;
        }
    }
    out.trim().to_string()
}

fn truncate_at_boundary(s: &str, mut n: usize) -> &str {
    n = n.min(s.len());
    while !s.is_char_boundary(n) {
        n -= 1;
    }
    &s[..n]
}

/// Rough token estimate (4 chars per token) for logging.
pub fn estimate_tokens(text: &str) -> usize {
    text.len().div_ceil(4)
}

/// The page's `<title>`, if any.
pub fn title(html: &str) -> Option<String> {
    let doc = scraper::Html::parse_document(html);
    let sel = scraper::Selector::parse("title").ok()?;
    let t = doc.select(&sel).next()?.text().collect::<String>();
    let t = t.split_whitespace().collect::<Vec<_>>().join(" ");
    (!t.is_empty()).then_some(t)
}

/// All links on the page as (anchor text, absolute URL), deduplicated, in
/// document order. Used for LLM-guided link following.
pub fn links(html: &str, base: &str) -> Vec<(String, String)> {
    let doc = scraper::Html::parse_document(html);
    let Ok(sel) = scraper::Selector::parse("a[href]") else {
        return vec![];
    };
    let base_url = url::Url::parse(base).ok();
    let mut seen = HashSet::new();
    let mut out = vec![];
    for a in doc.select(&sel) {
        let Some(href) = a.value().attr("href") else { continue };
        let href = href.trim();
        if href.is_empty() || href.starts_with('#') || href.starts_with("javascript:") || href.starts_with("mailto:") {
            continue;
        }
        let abs = match &base_url {
            Some(b) => b.join(href),
            None => url::Url::parse(href),
        };
        let Ok(mut abs) = abs else { continue };
        if !matches!(abs.scheme(), "http" | "https") {
            continue;
        }
        abs.set_fragment(None);
        let text = a.text().collect::<String>();
        let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
        // Deduplicated by (text, URL), not by URL alone: the same target
        // linked twice with different words ("https://esop27.hotcrp.com/"
        // in a list, then "here" in "The papers can be submitted here")
        // must keep both, since a quote is grounded by the anchor text.
        if text.is_empty() || !seen.insert((text.clone(), abs.to_string())) {
            continue;
        }
        out.push((text, abs.to_string()));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    pub fn fixture(name: &str) -> String {
        std::fs::read_to_string(format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap()
    }

    #[test]
    fn dates_table_survives_and_nav_is_gone() {
        let md = html_to_markdown(&fixture("popl26-dates.html"));
        assert!(md.contains("Thu 10 Jul 2025"), "submission date row missing:\n{md}");
        assert!(md.contains("Rennes"), "location missing");
        assert!(!md.contains("http"), "URLs should be stripped");
        assert!(!md.contains("Requesting a Visa"), "navigation should be stripped:\n{md}");
        assert!(md.len() < 20_000, "dates page should be compact, got {}", md.len());
    }

    #[test]
    fn cfp_survives_budgeting() {
        let md = html_to_markdown(&fixture("splash26-oopsla.html"));
        assert!(md.len() <= MAX_CHARS + 100);
        let lower = md.to_lowercase();
        assert!(lower.contains("important dates"), "important dates section missing");
        assert!(lower.contains("double-blind") || lower.contains("double blind"), "cfp prose missing");
        assert!(lower.contains("round"), "round info missing");
    }

    #[test]
    fn headingless_page_is_truncated_not_dropped() {
        let line = "Submission deadline is 1 March 2027 and notification 1 May 2027. ";
        let big = line.repeat(2_000);
        let out = budget(&big, 60_000);
        assert!(out.len() > 30_000, "got {}", out.len());
        assert!(out.ends_with("[…]"));
    }

    #[test]
    fn duplicate_lines_with_digits_are_kept() {
        let md = "*   Notification: 14 May 2026\n\n*   Notification: 14 May 2026\n*   Program\n*   Program\n";
        let t = tidy(md);
        assert_eq!(t.matches("14 May 2026").count(), 2);
        assert_eq!(t.matches("Program").count(), 1);
    }

    #[test]
    fn headings_inside_list_items_split_sections_so_a_committee_can_be_dropped() {
        // A researchr sidebar: dates, then a committee of "*   ##### Name" items.
        let mut md = String::from("# Call for Papers\n\nSubmission deadline is firm.\n\n### FAQ\n\nMay I post on arXiv? Yes.\n\nImportant Dates\n\n**Wed 14 Oct 2026**\n**Submission (Round 1)**\n\nFri 18 Dec 2026\nAuthor Notification (Round 1)\n\nOOPSLA Review Committee\n\n");
        for i in 0..300 {
            md.push_str(&format!("*   ##### Person Number {i}\n\n    ##### Some University\n\n    ##### Some Country\n\n"));
        }
        let secs = sections(&md);
        assert!(secs.len() > 300, "each committee member is its own section, got {}", secs.len());
        let out = budget(&md, 6_000);
        assert!(out.contains("Submission (Round 1)") && out.contains("Author Notification (Round 1)") && out.contains("May I post on arXiv"), "{out}");
        assert!(!out.contains("Person Number 250"), "the committee is filler and goes first");
    }

    #[test]
    fn a_rows_header_cell_keeps_its_label() {
        // ETAPS 2027's dates table: <th> in the body names the row.
        let html = r#"<table><thead><tr><th>Event / Date</th><th>ESOP–round 1</th><th>TACAS</th></tr></thead><tbody><tr><th>Submission</th><td>May 28</td><td>Oct 15</td></tr><tr><th>Mandatory Artifact Submission</th><td>—</td><td>Oct 29</td></tr></tbody></table>"#;
        let md = html_to_markdown(html);
        assert!(md.contains("| Submission | May 28 | Oct 15 |"), "{md}");
        assert!(md.contains("| Mandatory Artifact Submission | — | Oct 29 |"), "{md}");
        assert!(md.contains("Event / Date"), "the header row is still the header: {md}");
    }

    #[test]
    fn page_header_content_is_kept_but_navigation_is_not() {
        // CAV 2027: dates and venue live in a hero block inside <header>.
        let html = r#"<html><body><header class="header-section"><nav><a href="/x">Menu item</a></nav><div class="navbar">Site menu</div><div class="hero"><h1>39th CAV</h1><span>July 19-23</span> <span>Amsterdam</span></div></header><main><p>Submission deadline 20 January 2027</p></main></body></html>"#;
        let md = html_to_markdown(html);
        assert!(md.contains("July 19-23") && md.contains("Amsterdam") && md.contains("39th CAV"), "{md}");
        assert!(!md.contains("Menu item") && !md.contains("Site menu"), "{md}");
    }

    #[test]
    fn the_same_target_linked_with_different_words_keeps_both_anchors() {
        let html = r#"<a href="https://esop27.hotcrp.com/">Submit paper</a><p>The papers can be submitted <a href="https://esop27.hotcrp.com/">here</a>.</p><a href="https://esop27.hotcrp.com/">Submit paper</a>"#;
        let links = links(html, "https://etaps.org/2027/conferences/esop/");
        assert_eq!(links.len(), 2, "{links:?}");
        assert!(links.iter().any(|(t, _)| t == "here"));
    }

    #[test]
    fn links_are_absolute_and_deduped() {
        let l = links(&fixture("icfp26-volunteers.html"), "https://icfp26.sigplan.org/track/x");
        assert!(l.iter().all(|(_, u)| u.starts_with("http")));
        // Deduplicated by (anchor text, URL): no pair twice.
        let pairs: HashSet<_> = l.iter().collect();
        assert_eq!(pairs.len(), l.len());
    }

    #[test]
    fn sizes_report() {
        for name in [
            "popl26-dates.html", "popl26-cfp.html", "splash26-dates.html", "splash26-oopsla.html",
            "pldi26-cfp.html", "icfp26-cfp.html", "icfp26-volunteers.html", "splash26-volunteers.html",
            "cav26.html", "etaps26.html", "etaps26-esop.html", "oopsla25.html",
        ] {
            let html = fixture(name);
            let raw = html_to_markdown_unbudgeted(&html);
            let md = budget(&raw, MAX_CHARS);
            eprintln!("{name:26} html={:>8} unbudgeted={:>7} md={:>7} ~tokens={:>6}", html.len(), raw.len(), md.len(), estimate_tokens(&md));
        }
    }
}
