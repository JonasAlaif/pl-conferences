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

/// Tags whose whole subtree is dropped. Keeping `<header>`/`<aside>` was
/// tried: on researchr pages it adds 5-10K chars of site chrome, pushing
/// the big track pages over the budget, and the harness regressed.
const SKIP_TAGS: &[&str] = &[
    "script", "style", "noscript", "template", "svg", "img", "picture", "video", "audio", "iframe",
    "canvas", "form", "input", "button", "select", "textarea", "nav", "header", "footer", "aside",
];

/// Elements removed before conversion. Bootstrap-era class names are common
/// enough to be worth stripping generically; anything unmatched is harmless.
const STRIP_SELECTORS: &str = "[role=navigation], [role=banner], [role=contentinfo], [role=dialog], \
    .navbar, .navigationbar, #navigationbar, .nav-menu, .menu-bar, .dropdown-menu, .breadcrumb, \
    .footer, .footer-box, .site-footer, .modal, .cookie, .cookie-banner, .cookie-consent, .skip-link, \
    .sr-only, .visually-hidden, [hidden], [aria-hidden=true]";

/// Upper bound on the text handed to the model (roughly 11K tokens). Prompt
/// processing dominates CPU time, so this is the main speed knob.
pub const MAX_CHARS: usize = 45_000;
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
    budget(&html_to_markdown_unbudgeted(html), MAX_CHARS)
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
        if line.starts_with('#') {
            sections.push(String::new());
        }
        let cur = sections.last_mut().unwrap();
        cur.push_str(line);
        cur.push('\n');
    }
    sections
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
    links_with_context(html, base).into_iter().map(|(t, u, _)| (t, u)).collect()
}

/// `(anchor text, absolute URL, context)`: the context is the text of the
/// element around the link, cut to a window around the anchor, so that a
/// link reading "here" comes with "Apply here by July 12". Empty when the
/// element adds nothing to the anchor text.
pub fn links_with_context(html: &str, base: &str) -> Vec<(String, String, String)> {
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
        if text.is_empty() || !seen.insert(abs.to_string()) {
            continue;
        }
        let around = a.parent().and_then(scraper::ElementRef::wrap).map(|p| p.text().collect::<String>()).unwrap_or_default();
        let around = around.split_whitespace().collect::<Vec<_>>().join(" ");
        let context = if around == text { String::new() } else { window_around(&around, &text, 60) };
        out.push((text, abs.to_string(), context));
    }
    out
}

/// `text` with up to `radius` characters on each side of `needle`.
fn window_around(text: &str, needle: &str, radius: usize) -> String {
    let pos = text.find(needle).unwrap_or(0);
    let mut start = pos.saturating_sub(radius);
    while !text.is_char_boundary(start) {
        start -= 1;
    }
    let mut end = (pos + needle.len() + radius).min(text.len());
    while !text.is_char_boundary(end) {
        end += 1;
    }
    let mut s = String::new();
    if start > 0 {
        s.push('…');
    }
    s.push_str(&text[start..end]);
    if end < text.len() {
        s.push('…');
    }
    s
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
    fn links_are_absolute_and_deduped() {
        let l = links(&fixture("icfp26-volunteers.html"), "https://icfp26.sigplan.org/track/x");
        assert!(l.iter().all(|(_, u)| u.starts_with("http")));
        let urls: HashSet<_> = l.iter().map(|(_, u)| u).collect();
        assert_eq!(urls.len(), l.len());
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
