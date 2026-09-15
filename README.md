# pl-conferences

Important dates for programming-languages conferences, collected automatically
every week by a small local language model running on a GitHub Actions runner,
and published as calendar files.

**Subscribe to everything:** add this URL to your calendar app as a subscription:

```
https://raw.githubusercontent.com/JonasAlaif/pl-conferences/main/all.ics
```

Per-conference calendars live under `conferences/<CONF>/<TRACK>/<YEAR>/` as
`conference.ics` (conference dates and location), `cfp.ics` (submission
deadline, rebuttal, notification per round, with the submission site linked)
and `volunteer.ics` (student-volunteer application deadline with the sign-up
page linked), each next to the JSON the model produced and the page it came
from.

<!-- maintenance:start -->
<!-- maintenance:end -->

## Upcoming dates

<!-- dates:start -->
<!-- dates:end -->

## Status

<!-- status:start -->
<!-- status:end -->

## How it works

1. `conferences.json` lists each conference, its research-paper track and the
   first year to collect. It is the only hand-edited file.
2. For every conference and every year up to next year that has no result yet
   (a bounded batch per run), the pipeline searches the web (DuckDuckGo, Brave and Bing HTML result pages,
   no API keys), lets the model pick the official page, fetches it, converts it
   to Markdown and asks the model for the dates as schema-constrained JSON.
   If the chosen page has no deadline, the model picks a link to follow.
3. Dates are checked for consistency (ordering, year, calendar validity); on
   failure the model gets one corrective retry. One extraction yields two
   records with separate lifecycles: `conference.json` (dates, location) and
   `cfp.json` (deadlines, submission details and link). Each is written with
   its calendar, the fetched page (`*.html`) and the Markdown the model saw
   (`*.md`), and committed. A page with only the conference dates still
   produces the conference record; deadlines keep being looked for.
4. Every conference-year has a stage derived from its data and the date:
   *future* (nothing known), *conference available* (dates and location, no
   deadlines yet), *deadlines available*, *post-rebuttal* (the last round's
   rebuttal has ended, so the deadlines are final) and *happened*. Conference
   dates are re-collected on every run until the conference is over, deadlines
   until the last rebuttal has ended, each starting from its stored source
   page: if a value changed, the JSON keeps a history entry and the calendar
   event says "Changed <date>: was <old>". Archived parts are never touched
   again. Volunteer pages open late, so that pass stays active until the
   conference has happened or the application deadline has passed.
5. Anything that worked only through a fallback is flagged with a maintenance
   code in [MAINTENANCE.md](MAINTENANCE.md) (regenerated every run) and in the
   affected calendar entry, so the repository can be fixed before the fallback
   also breaks. Failures are retried monthly until the year is over, then
   abandoned; nothing needs manual clean-up. Runs are weekly and bounded, so
   a failed attempt is retried within days. A model answer that cannot be
   parsed, or an error on one conference, only skips that conference-year;
   three errors in a row abort the run, and whatever was written is still
   committed.

Maintenance codes: E001 fallback search backend used (primary parsed
nothing), E002 no search backend answered (URLs derived from earlier
editions or guessed by the model), E003 page found by link-following, E004
headless Chrome needed, E005 corrective retry needed, E006 conference-year
not found three runs in a row after its year began, E007 Ollama or model
installed from a fallback source, E008 primary search backend throttled the
runner, E009 invalid TLS certificate ignored, E010 the pinned model tag now
resolves to a different upstream build.

### Vault release (optional hardening)

If the Ollama release or the model ever disappear upstream, the workflow
falls back to assets of a GitHub release tagged `vault` in this repository:
`ollama-linux-amd64.tar.zst` and the model GGUF split into `< 2 GB` parts
named `model.gguf.part-aa`, `model.gguf.part-ab`, ... To create it:

```bash
curl -fsSLO https://github.com/ollama/ollama/releases/download/v0.34.1/ollama-linux-amd64.tar.zst
curl -fsSL -o model.gguf https://huggingface.co/unsloth/Qwen3.5-4B-GGUF/resolve/main/Qwen3.5-4B-Q4_K_M.gguf
split -b 1900m model.gguf model.gguf.part-
gh release create vault --title "Vault: pinned Ollama + model" --notes "Fallback binaries" ollama-linux-amd64.tar.zst model.gguf.part-*
```

Model: [Qwen3.5-4B](https://huggingface.co/Qwen/Qwen3.5-4B) via
[Ollama](https://ollama.com), CPU only, structured output through Ollama's JSON
schema `format`. Override with `PLC_MODEL` (e.g. `qwen3.5:9b`), `PLC_NUM_CTX` for the context
size, `PLC_SEARCH_GAP` for the seconds between search-engine requests,
`PLC_NO_SEARCH=1` to exercise the no-search-engine fallbacks (URLs derived
from earlier editions, then model guesses, then link-following) and
`PLC_NUM_GPU=0` to force CPU inference locally. Each run attempts at most
`PLC_MAX_ITEMS` (default 5) conference-years, current year first, and stops
starting new ones after `PLC_TIME_BUDGET_MIN` (default 75) minutes; the rest
wait for the next weekly run. This keeps every run far from the job timeout
and lets a backlog (a new conference in `conferences.json`, a new year) drain
over a few weeks.

Resources: the 4B model at a 32K context takes about 6 GB of RAM on CPU
(`ollama ps` reports 4.7 GB for the model plus cache), so it fits the 16 GB of
a public-repository GitHub runner with room to spare; a call-for-papers page is
8-13K tokens after cleaning.
`PLC_THINK=1` enables the model's built-in reasoning; it is off by default
because on CPU the 4B model thinks for minutes per page and the harness showed
no accuracy gain on the call-for-papers pages (see `tests/live.rs`).

## Running locally

```bash
ollama pull qwen3.5:4b
cargo run --release -- --conference POPL --year 2026 --dry-run
```

`--dry-run` searches, extracts and logs everything but writes nothing.
Without it the run writes into `conferences/`, `all.ics`, `state.json`,
`MAINTENANCE.md` and the tables in this README. `--root DIR` points at a
different checkout (handy for experiments), `--no-volunteer` skips the
volunteer pass.

Debugging helpers:

```bash
cargo run -- clean page.html                       # the Markdown the model sees
cargo run -- extract page.html POPL 2026 POPL      # run extraction on a saved page
cargo run -- fetch https://example.org/cfp         # fetch (with Chrome fallback) and clean
cargo run -- sections page.html                    # relevance scores used when a page must be cut
cargo test                                         # offline tests on saved fixtures
cargo test -- --ignored                            # live tests against a local Ollama
```

## Layout

```
conferences.json        input
state.json              per conference-year attempt log (drives retries and maintenance codes)
all.ics                 aggregate calendar
MAINTENANCE.md          active maintenance codes and recent outcomes
conferences/…/conference.{json,ics,html,md}  dates and location + provenance, calendar, source page
conferences/…/cfp.{json,ics,html,md}         deadlines, submission details and link
conferences/…/volunteer.{json,ics,html,md}   student-volunteer deadline and sign-up link
src/                    the Rust pipeline (search, discover, fetch, clean, llm, schema, ics, state)
tests/fixtures/         saved pages for offline tests
```
