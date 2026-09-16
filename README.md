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
Last run: 2026-09-16. **Maintenance codes active: E003, E005** - see [MAINTENANCE.md](MAINTENANCE.md).
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
   Links work the same way: for the submission site and the volunteer
   sign-up form the model quotes the words that name them, and the URL is
   kept only if it is written in that quote or is the target of a link whose
   anchor text sits in it ("Apply here" resolves to the form behind "here").
   The model is never asked to pick a link from the page's link list.
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
   dates stay active until the conference is over, deadlines until the last
   rebuttal has ended. Missing parts are looked for first (a search that
   found nothing is repeated after a week); stored parts are re-checked
   every two weeks, each starting from its stored source page: if a value
   changed, the JSON keeps a history entry and the calendar event says
   "Changed <date>: was <old>". Archived parts are never touched again.
   Volunteer pages open late, so that pass stays active until the conference
   has happened or the application deadline has passed; a negative result is
   retried after three weeks.
5. Anything that worked only through a fallback is flagged with a maintenance
   code in [MAINTENANCE.md](MAINTENANCE.md) (regenerated every run) and in the
   affected calendar entry, so the repository can be fixed before the fallback
   also breaks. Failures are retried monthly until the year is over, then
   abandoned; nothing needs manual clean-up. Runs are twice weekly and
   bounded, so a failed attempt is retried within days. A model answer that cannot be
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

Model: [Qwen3.5-4B](https://huggingface.co/Qwen/Qwen3.5-4B) via
[Ollama](https://ollama.com), CPU only, structured output through Ollama's JSON
schema `format`. Override with `PLC_MODEL` (e.g. `qwen3.5:9b`), `PLC_NUM_CTX`
for the context size, `PLC_SEARCH_GAP` for the seconds between search-engine
requests, `PLC_NO_SEARCH=1` to exercise the no-search-engine fallbacks (URLs
derived from earlier editions, then model guesses, then link-following) and
`PLC_NUM_GPU=0` to force CPU inference locally. Each invocation attempts at
most `PLC_MAX_ITEMS` conference-years, discovery of missing data before
re-validation of stored data and the least recently attempted first, stops
starting new ones after `PLC_TIME_BUDGET_MIN` (default 75) minutes, and
fetches at most six pages per attempt; the rest waits for the next run. This
keeps every worker far from the job timeout, lets a backlog (a new conference
in `conferences.json`, a new year) drain over a few weeks, and means
re-checking this year's conferences can never crowd out next year's calls for
papers. The model is unloaded between conference-years: its server process
was seen growing to 13 GB over a long run.

The workflow runs one worker per conference in parallel (`--conference NAME`,
`PLC_MAX_ITEMS` 2: this year and next). Workers never touch git; each
uploads its own `conferences/NAME/` directory and its `state.json` as an
artifact, and a single gather job copies the (disjoint) directories together,
merges the state files newest-attempt-wins (`pl-conferences merge-state`),
rebuilds the outputs (`pl-conferences regenerate`) and makes the one commit.
A workflow-level concurrency group keeps two runs from overlapping, so the
only thing that can race the push is a person pushing during a run, which
the gather job handles by rebasing and retrying. `PLC_THINK=1`
enables the model's built-in reasoning; it is off by default because on CPU
the 4B model thinks for minutes per page and the harness showed no accuracy
gain.

Resources: on the runner the model server takes about 5 GB of RAM at a 16K
context with `OLLAMA_FLASH_ATTENTION=1` (10 GB without; an 8-bit KV cache
saves a little more but measurably changed answers on the harness, so it is
not used), so it fits the 16 GB of a public-repository GitHub runner. That
runner processes prompts at roughly 20-30 tokens/s and generates at 7
tokens/s, so a call-for-papers page (up to about 6K tokens after cleaning
and budgeting: programme, paper lists and committees are dropped first)
costs 3-5 minutes and a whole conference-year (search, page, link choices, volunteer
pass) 10-15 minutes; hence one worker per conference in parallel, twice a
week, each attempting at most two conference-years (up to about 30 minutes
per worker, against a 330-minute job timeout).

## Things to know

- Every calendar entry says it was extracted by a language model and links
  the page it came from; treat the source as authoritative. If a page states
  two different dates for the same deadline, the entry quotes the other
  statement as a note.
- A page that lists the same label twice can defeat the model: OOPSLA's
  dates list has two "Author Notification (Round 2)" entries (the second is
  the decision on revised papers) with the revision deadline between them,
  and the 4B model settles on the wrong one for the last round however it is
  asked. For earlier rounds a rule catches it (a round's notification must
  precede the next round's deadline); for the last round the stored value is
  whatever was read first, and a re-read that disagrees while the page still
  states the stored date keeps the stored date rather than flip-flopping.
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
