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
Last run: 2026-09-16. **Maintenance codes active: E003** - see [MAINTENANCE.md](MAINTENANCE.md).
<!-- maintenance:end -->

## Upcoming dates

<!-- dates:start -->
| Date | Conference | Event |
|---|---|---|
| 2026-10-04 | SPLASH 2026 (OOPSLA) | Conference |
| 2026-10-14 | SPLASH 2027 (OOPSLA) | R1 Paper Submission Deadline |
| 2026-10-15 | ETAPS 2027 (ESOP) | R2 Paper Submission Deadline |
| 2026-11-06 | SPLASH 2027 (OOPSLA) | Volunteer Application Deadline |
| 2026-11-12 | PLDI 2027 | Paper Submission Deadline |
| 2026-12-01 | SPLASH 2027 (OOPSLA) | R1 Rebuttal |
| 2026-12-07 | ETAPS 2027 (ESOP) | R2 Rebuttal |
| 2026-12-18 | SPLASH 2027 (OOPSLA) | R1 Notification |
| 2026-12-22 | ETAPS 2027 (ESOP) | R2 Notification |
| 2027-02-16 | PLDI 2027 | Rebuttal |
| 2027-04-01 | PLDI 2027 | Notification |
| 2027-04-07 | SPLASH 2027 (OOPSLA) | R2 Paper Submission Deadline |
| 2027-04-12 | ETAPS 2027 (ESOP) | Conference |
| 2027-06-05 | PLDI 2027 | Conference |
| 2027-06-15 | SPLASH 2027 (OOPSLA) | R2 Rebuttal |
| 2027-08-13 | SPLASH 2027 (OOPSLA) | R2 Notification |
| 2027-10-10 | SPLASH 2027 (OOPSLA) | Conference |

<!-- dates:end -->

## Status

<!-- status:start -->
| Conference | Stage | Submission deadline(s) | Conference dates | Last verified |
|---|---|---|---|---|
| CAV 2026 | happened | 2026-01-28 | 2026-07-26..2026-07-29 | 2026-09-15 |
| ETAPS 2026 (ESOP) | happened | 2025-06-03, 2025-10-16 | 2026-04-13..2026-04-16 | 2026-09-16 |
| ETAPS 2027 (ESOP) | deadlines available | 2026-05-28, 2026-10-15 | 2027-04-12..2027-04-15 | 2026-09-16 |
| ICFP 2026 | happened | 2026-02-19 | 2026-08-24..2026-08-29 | 2026-09-15 |
| PLDI 2026 | happened | 2025-11-13 | 2026-06-15..2026-06-19 | 2026-09-15 |
| PLDI 2027 | deadlines available | 2026-11-12 | 2027-06-05..2027-06-11 | 2026-09-16 |
| POPL 2026 | happened | 2025-07-10 | 2026-01-11..2026-01-17 | 2026-09-15 |
| SPLASH 2026 (OOPSLA) | post-rebuttal | 2025-10-10, 2026-03-17 | 2026-10-04..2026-10-09 | 2026-09-16 |
| SPLASH 2027 (OOPSLA) | deadlines available | 2026-10-14, 2027-04-07 | 2027-10-10..2027-10-15 | 2026-09-16 |

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

### Vault release (not set up)

The workflow can fall back to assets of a GitHub release tagged `vault` in
this repository (`ollama-linux-amd64.tar.zst` and the model GGUF split into
`model.gguf.part-aa`, `-ab`, ...) if both the Ollama release and the model
registry ever disappear. Those assets are about 5 GB and are deliberately not
uploaded for now; until they are, the fallback is a no-op and a missing
upstream shows up as a failed run with an issue.

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
