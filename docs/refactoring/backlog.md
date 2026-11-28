# Structural refactoring backlog

Pass 1, branch `refactor/complexity-pass-1`, base `6e82e1f`. Check results live in the run ledger, not here.

## Verification commands

Run from the repository root unless noted. A change to the root crate also reruns the desktop and preview checks, because both consume it.

| Area | Command |
| :- | :- |
| Root | `scripts/check-nested-crates.sh` |
| Root | `cargo build --locked` |
| Root | `cargo test --locked` |
| Root | `cargo clippy --locked --all-targets` (informational; 5 warnings at base) |
| Root reader | `scripts/verify-reader.sh` (fmt check, offline tests, recorded replay) |
| Root settings | `uv run --with pyte python scripts/verify-installed.py --binary target/debug/hn` |
| Desktop (in `desktop/`) | `cargo build --locked`, `cargo test --locked`, `cargo test --release --locked` |
| Desktop goldens (in `desktop/`, macOS) | `HN_GOLDEN_ACTUAL=$(mktemp -d) bash scripts/check_goldens.sh` |
| Preview | `HN_BIN=$PWD/target/debug/hn` then `npm ci`, `npm run format:check`, `npm test`, `npm run test:update`, `npm run test:rollout`, `npm run build`, each with `--prefix preview` |

Not run locally: Linux and Windows desktop CI, the preview image build, TUI release targets, the native webview. Never run: `preview/deploy/*`, `setup-rootless.sh`, publish/adopt/rollout against hosts, release workflows, the preview monitor, the paid live-provider check.

## Candidates

Status values: open, in progress, done, rejected (reason), blocked (what unblocks it), pending decision.

### C1. Terminal repeats two shared rules (bookmark toggle, social-host exclusion)
- **Problem.** `bookmarks::toggle` re-implements `toggle_saved` (`src/bookmarks.rs:52` vs `:69`). The X/Twitter host rule is written twice, in initial snapshot selection (`src/app.rs:584`) and paid preparation (`src/reader/snapshot.rs:77`); both came in `3394429`.
- **Benefit.** Each rule has one owner; two copies removed.
- **Risk and checks.** Saved-story tests (both views, reload, failed-write rollback); host-variant table (`src/app/app_tests.rs:26`); preparation fixture (`src/reader/snapshot_tests.rs:22`).
- **Status.** done (batch 1). `toggle` calls `toggle_saved`; the host rule is `snapshot::is_excluded_social_host` (crate-private), used by both callers, each keeping its own scheme rule.

### C2. Desktop decides deferred-summary dispatch in three places
- **Problem.** The window derives `configured && generating` per half (`desktop/src/app/reading_actions.rs:127`); both bridge branches then repeat the same paid-call predicate (`desktop/src/bridge/mod.rs:199`, `:214`). Fixes `bad0400` and `f4b464c` each touched app, bridge and story together.
- **Benefit.** One predicate decides whether a paid summary call starts, for both halves.
- **Risk.** `summary_skipped` has a narrower meaning than "not dispatched"; it must stay as is. Checks: `desktop/src/app/tests/summary_lifecycle.rs`, `summary_persistence.rs`, goldens.
- **Status.** done (batch 2, with C8). The bridge's `should_summarize` decides dispatch for both halves. The window's `configured()` gate stays; see justified exceptions.

### C3. Terminal settings writes summary progress directly, per half
- **Problem.** Applying settings repeats three decisions for article and discussion and sets `Progress::Idle` itself (`src/reader/settings.rs:138-195`), although `Section` owns queuing and cancellation (`src/reader/jobs.rs:15-79`).
- **Benefit.** Three paired branches become `Section` operations; `Section` is the only writer of its progress.
- **Risk.** Auto-off must keep submitted work; changed keys invalidate pending work; auth failures requeue only on the active story. Checks: `src/reader/tests/summary_settings.rs`, `scripts/verify-reader.sh`.
- **Status.** done (batch 3). `Section::cancel_automatic_preparation`, `invalidate_pending` and `retry_auth_failure` (`src/reader/jobs.rs`); settings no longer reads or writes `Progress`.

### C4. Preview host tools classify a validator failure twice
- **Problem.** `tools/update.mjs:311` and `tools/rollout.mjs:177` run `hn --validate-snapshot` with the same timeout and the same "could not run vs rejected" predicate. `388cabf` copied the reader's rule after `7a59940` fixed it.
- **Benefit.** One owner, in the already-shared `tools/release.mjs`, for a decision that either aborts a good release or retries a rejected one when wrong.
- **Risk.** Callers keep their own rollback, abort and messages. Checks: `npm run test:update`, `npm run test:rollout`.
- **Status.** pending decision (P7). Any change to `preview/tools/*.mjs` changes the host-tools version that the production monitor compares with `main` (`.github/workflows/preview-monitor.yml:87-89`, `preview/deploy/tools-version.sh`), and a person must then run `sudo preview/deploy/install-tools.sh dev|prod` on each host (`preview/deploy/README.md`, "Automatic server releases"). For a two-line duplicated predicate that cost outweighs the benefit; bundle it with the next deliberate host-tools change.

### C5. Desktop starts a feed request from two owners
- **Problem.** `FeedModel::apply` computes the fill and writes `Loading` (`desktop/src/feed.rs:231`); `App::apply` re-derives the ranking and `request` recomputes the fill and writes `Loading` again (`desktop/src/app/feed_actions.rs:15`, `:111`).
- **Benefit.** The model establishes a request once; `App` dispatches exactly that.
- **Risk.** Deadline rules for new tab, in-flight reflow and pending page turn. Checks: `desktop/src/app/tests/feed_arrivals.rs`, `feed_paging.rs`.
- **Status.** done (batch 4). `FeedModel` establishes every request (`establish`, `start_request`, `refresh`, `reflow`) and returns its fill and ranking; `App::dispatch_feed` sends exactly that. Added `refresh_restarts_an_in_flight_deadline` (fails if refresh keeps the old clock).

### C6. Feed transitions take an owned copy of the whole feed
- **Problem.** Every navigation call builds a `FeedState` by cloning stories, ID snapshot and caches (`src/app.rs:371`, `desktop/src/feed.rs:170`), while the six transitions read only scalars (`src/feed_state.rs:108`). The two-column destination is computed separately in each front end (`src/app.rs:305`, `desktop/src/app/input.rs:129`; `324d008`).
- **Benefit.** Zero owned `FeedState` builds on navigation paths; one destination calculation.
- **Risk.** Changing the transitions' signatures breaks `hn_lib` public items; keeping both signatures would leave two entry points with no outside consumer.
- **Status.** blocked by board item B47 (may `hn_lib`'s public API change?)

### C7. Desktop reply activation probes retry availability at three sites
- **Problem.** Row label, pointer/Enter activation and Right key each check exhausted batches and failed IDs (`desktop/src/story_view/discussion.rs:286`, `desktop/src/app/reading_actions.rs:243`, `desktop/src/app/input.rs:350`); `60f0f0e` touched all three.
- **Benefit.** One `Story` decision, three callers.
- **Checks.** `desktop/src/app/tests/discussion.rs:623`, `src/comments.rs:1167`.
- **Status.** open

### C8. Desktop restores saved summaries through synthetic completions
- **Problem.** Restore manufactures successful `StoryUpdate`s and the window re-interprets results to clear age markers (`desktop/src/app/reading_actions.rs:42`, `:135`; `desktop/src/story.rs:575`).
- **Benefit.** `Story` owns restoration and its markers; two synthetic completions removed.
- **Checks.** `summary_persistence.rs:444`, restart request counts, goldens.
- **Status.** done (batch 2, with C2). `Story::restore_summary` installs saved halves; `Story` alone writes `restored_summary`.

### C9. Discussion evidence is built separately by each app
- **Problem.** Both clean comment text and apply the 1,800 and 6,000 character limits (`src/reader/sources.rs:201`, `desktop/src/service/mod.rs:155`).
- **Benefit.** Each limit has one owner; selection stays per app.
- **Risk.** Needs additive public methods on reader types.
- **Status.** open

### C10. Operator runbook still describes the retired image owner
- **Problem.** `preview/deploy/README.md:13` and `:32` assign `PREVIEW_IMAGE` to `gateway.env` and tell operators to restore the old pin; the updater writes it to `current/release.env` (`tools/update.mjs:116`) and requires `PREVIEW_ROLE` (`:191`), which the instructions omit.
- **Benefit.** Finishes the #88 migration in the active instructions.
- **Status.** open, partly blocked. Removing `PREVIEW_IMAGE` from the `gateway.env` list is verifiable from the repo (the unit loads `current/release.env` second, `preview/deploy/hn-preview.service:11`). Where `PREVIEW_ROLE` is set is not: the update unit loads both `gateway.env` and `storage.env` (`preview/deploy/hn-preview-update.service`), and only the hosts show which one holds it.

### C11. Browser reconnect regression lives inside the publisher harness
- **Problem.** About 90 lines of `preview/publisher-check.mjs:202-290` test `app.js` but need the native binary, simulated R2 and systemctl to run.
- **Benefit.** The browser check runs on its own; the sensitivity mutation targets it.
- **Status.** open

## Pending decisions

| ID | Decision | Blocks |
| :- | :- | :- |
| P1 | May `hn_lib`'s public API change? (board B47) crates.io lists 0 dependents, 22 downloads | C6; P5 |
| P2 | Desktop copies an environment-only key into `summary.json` on an unrelated Apply (`desktop/src/settings.rs:99` compares with the stored field; terminal compares with the effective key, `src/reader/settings.rs:105`). README.md:103 says this must not happen. Fix as a behavior correction outside this pass? | Sharing key-edit logic between apps |
| P3 | Desktop sibling movement at a re-rooted comment can leave the displayed subtree; terminal pins the opposite (`src/app/comment_view_state_tests.rs:175`, `desktop/src/app/input.rs:337`) | Consolidating the sibling guard |
| P4 | Preview monitor and page gate still accept health without `running` (`.github/workflows/preview-monitor.yml:51`, `preview-ui.yml:76`). Is that recovery mode retired? | Removing the exception (CI change) |
| P5 | Capture fetches each story twice (`src/snapshot_capture.rs:31`, `src/hn_cli_service.rs:265`); fixing it widens `hn_lib`'s surface and changes request counts | — |
| P7 | Share the snapshot-validator failure rule in `preview/tools/release.mjs` (C4), accepting a host-tools reinstall on both hosts after merge? | C4 |
| P6 | Remove the seven unused direct `icu_*` dependencies from the root manifest (`Cargo.toml:39-45`; `418816e`, `f4950e1`, `251babc`) | Dependency change |

## Rejected

- Split `src/comments.rs` by size alone: no duplicated decision removed.
- Unify the comment key handlers: cancellation, citations, scrolling and focus have different owners and behavior.
- Delete unused `FeedAction` variants: breaks a frozen public enum.
- Share adopter and rollout manifest checks: one wire rule across a signed image and manually installed tools; a shared import couples two delivery paths for three stable checks.
- Remove the `HN_BIN` override: documented test contract (`preview/README.md:27`).
- Shared terminal/desktop summary scheduler, citation replacement transition, browser controller: lifecycles and behavior differ by platform.

## Completed batches

| Batch | Candidate | Commit | Change |
| :- | :- | :- | :- |
| 1 | C1 | `60f61d0` | 11 lines added, 14 removed in 4 files; two duplicated rules now have one owner each |
| 2 | C2, C8 | `15e677b` | 39 added, 45 removed in 3 desktop files; one dispatch predicate (was two), one writer of the restored-summary marker (was three sites in two types), no synthetic `StoryUpdate` on restore |
| 3 | C3 | `0a593d3` | 33 added, 47 removed in 2 files; settings no longer writes summary progress; three paired branches became `Section` operations |
| 4 | C5 | the commit that adds this row | 84 added, 86 removed in 3 desktop files; fill computed and `Loading` written once per request (were twice), ranking decided once, reflow decision moved from `poll.rs` into the model |

## Active transitions

None.

## Justified exceptions

- **Two credential checks before a desktop summary call.** The window's `configured()` (`desktop/src/app/mod.rs:224`, `headless || key`) and the bridge's `should_summarize` (`!source.is_live() || key`) agree in production, where headless means the fixture source. The test constructor `desktop/src/app/tests/support.rs:57` always uses the fixture source and some tests set `headless = false` (`feed_paging.rs:125`, `story_lifecycle.rs:259`), so dropping the window gate would change those tests' behavior. Removing it would need the test harness to pair source and headless the way `init` does; not worth a batch on its own.
