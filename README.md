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

## The site

Every run regenerates **https://jonasalaif.github.io/pl-conferences/** from
the data in this repository: upcoming dates, one row per conference-year with
its deadlines, conference dates, volunteer deadline and links (call for
papers, submission site, sign-up page, per-year `.ics` files), and the
calendar subscription. The page is in `docs/` and deployed by the workflow.

## How it works

1. `conferences.json` lists each conference, its research-paper track and the
   first year to collect. It is the only hand-edited file.
2. For every conference and every year up to next year that has no result yet
   (a bounded batch per run), the pipeline searches the web (DuckDuckGo, Brave and Bing HTML result pages,
   no API keys), lets the model pick the official page, fetches it, converts it
   to Markdown and asks the model for the dates as schema-constrained JSON.
   If the chosen page has no deadline, the model picks a link to follow.
   For every date the model must first quote the words on the page and copy
   the date exactly as written there; validation checks that the two sit in
   the same entry of the page (same or adjacent line), which stops a small
   model from pairing a label with a neighbouring row's date on dense tables.
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

## Things to know

- Every calendar entry says it was extracted by a language model and links
  the page it came from; treat the source as authoritative. If a page states
  two different dates for the same deadline, the entry quotes the other
  statement as a note.
- The workflow's commit favours the run's files over concurrent hand edits to
  the same files (`git pull --rebase -X theirs`); edit `state.json` or a record
  by hand only between runs.
- Runs are twice a week partly because GitHub evicts caches unused for seven
  days: a monthly cadence would re-download the model and Ollama every time.
- Renaming a conference or track in `conferences.json` changes the calendar
  UIDs, so subscribers see the events twice; prefer adding a new entry.
- Maintenance codes: see [MAINTENANCE.md](MAINTENANCE.md); E011 marks a
  conference-year that errored in two consecutive runs.

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
cargo test --test live -- --ignored --nocapture    # live accuracy harnesses against a local Ollama
```

The live harness has two parts: `extraction_accuracy` (nine saved call-for-papers
and dates pages with known deadlines) and `volunteer_accuracy` (four volunteer
pages with known deadlines plus a namesake page that must be rejected).
`PLC_RUNS` repeats each case, `PLC_CASE` filters by fixture name. Run it after
touching the schema, the prompts or the cleaning; a change is kept only if the
pass count does not drop.

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
