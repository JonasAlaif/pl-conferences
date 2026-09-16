# Maintenance status

This file is regenerated on every run from `state.json`. Each code marks a fallback path that worked but suggests the primary mechanism needs a look. Codes disappear once the primary path works again.

Last run: 2026-09-16 02:37 UTC

## E003

The search hit did not contain the dates; they were found by following a link from it. Usually harmless, but check the query in `src/discover.rs` if it becomes common.

- `ICFP/ICFP/2026/cfp` (last seen 2026-09-15)
- `POPL/POPL/2026/cfp` (last seen 2026-09-15)

## Recent outcomes

| Conference-year | Outcome | Attempts | Last attempt | Note |
|---|---|---|---|---|
| `SPLASH/OOPSLA/2027/volunteer` | ok | 1 | 2026-09-16 | trying volunteer link https://2027.splashcon.org/signup from the conference page; https://stanfordesp.org/getinvolved/index.html is not about SPLASH 2027 (OOPSLA); model picked link https://cornell.learningu.org/volunteer/Splash/current/signup as the application form or sign-up page where students apply to be student volunteers at SPLASH 2027; not a general information page |
| `SPLASH/OOPSLA/2027/cfp` | ok | 1 | 2026-09-16 | validation failed: round 2 author_response_start 2027-01-22 is before an earlier date 2027-04-07; round 2 notification 2027-02-12 is before an earlier date 2027-03-05; validation failed again: round 2 author_response_start 2027-01-22 is before an earlier date 2027-04-07; round 2 notification 2027-02-12 is before an earlier date 2027-03-05 |
| `SPLASH/OOPSLA/2026/volunteer` | ok | 1 | 2026-09-15 | trying volunteer link https://2026.splashcon.org/track/splash-issta-2026-student-volunteers from the conference page |
| `SPLASH/OOPSLA/2026/cfp` | ok | 2 | 2026-09-16 | re-checking stored page https://2026.splashcon.org/track/oopsla-2026 |
| `POPL/POPL/2026/volunteer` | ok | 1 | 2026-09-15 | trying volunteer link https://popl26.sigplan.org/track/POPL-2026-student-volunteers from the conference page |
| `POPL/POPL/2026/cfp` | ok | 1 | 2026-09-15 | following link https://conf.researchr.org/track/POPL-2026/POPL-2026-popl-research-papers |
| `PLDI/PLDI/2027/volunteer` | notfound | 1 | 2026-09-16 | trying volunteer link https://pldi27.sigplan.org/signup from the conference page; https://pldi26.sigplan.org/track/pldi-2026-student-volunteering is not about PLDI 2027; https://conf.researchr.org/attending/pldi-2016/Student+Volunteers is not about PLDI 2027; https://pldi25.sigplan.org/track/pldi-2025-student-volunteering is not about PLDI 2027 |
| `PLDI/PLDI/2027/cfp` | ok | 1 | 2026-09-16 | model picked link https://conf.researchr.org/signin/pldi-2027/https%3A%5Es%5Espldi27.sigplan.org%5Es as the submission system where authors upload their papers for PLDI 2027 (PLDI track); not a call-for-papers or information page |
| `PLDI/PLDI/2026/volunteer` | ok | 1 | 2026-09-15 | trying volunteer link https://pldi26.sigplan.org/track/pldi-2026-student-volunteering from the conference page |
| `PLDI/PLDI/2026/cfp` | ok | 1 | 2026-09-15 | following link https://pldi26.sigplan.org/attending/Information-for-Presenters |
| `ICFP/ICFP/2026/volunteer` | ok | 1 | 2026-09-15 | trying volunteer link https://icfp26.sigplan.org/track/icfp-2026-icfp-volunteers from the conference page |
| `ICFP/ICFP/2026/cfp` | ok | 1 | 2026-09-15 | following link https://icfp26.sigplan.org/; following link https://icfp26.sigplan.org/track/icfp-2026-icfp-papers |
| `ETAPS/ESOP/2026/cfp` | ok | 1 | 2026-09-16 |  |
| `CAV/CAV/2026/volunteer` | notfound | 1 | 2026-09-15 | trying volunteer link https://submissions.floc26.org/cav/ from the conference page; fetch https://submissions.floc26.org/cav/ failed: GET https://submissions.floc26.org/cav/: HTTP 403 Forbidden; https://taoscav.org/volunteer is not about CAV 2026; fetch https://i-cav.org/cavlinks/who-are-we/ failed: GET https://i-cav.org/cavlinks/who-are-we/: HTTP 404 Not Found; fetch https://www.cavscare.org/champions-of-the-community failed: GET https://www.cavscare.org/champions-of-the-community: HTTP 404 Not Found |
| `CAV/CAV/2026/cfp` | ok | 1 | 2026-09-15 |  |
