# Architecture map

Written for structural refactoring pass 1 (base `6e82e1f`). Labels: **documented** (README, design notes, tests that pin a requirement), **observed** (read in code or history), **inferred** (intent not written down anywhere).

## Products and entry points

| Product | Root | Entry point | Toolchain |
| :- | :- | :- | :- |
| Terminal app `hn` | repository root, crate `hn-reader` | `src/main.rs` (separate `[[bin]]` crate using `hn_lib`) | Rust 1.88, edition 2021 |
| Shared library `hn_lib` | `src/lib.rs` | Used by the binary and by `desktop/` through a path dependency; published to crates.io by `tui-release.yml` | Same as above |
| macOS app `hn-desktop` | `desktop/` (own Cargo workspace) | `desktop/src/main.rs` → `App` in `desktop/src/app/` | Rust 1.99.1, edition 2024, Rune (pinned Git) and wry |
| Browser preview | `preview/` | `server.mjs` (gateway), `app.js` (page), `publish.mjs`, `adopt.mjs`, `tools/` (host release tooling) | Node 22 |

`src/main.rs` also dispatches the snapshot commands: capture, preparation, `--validate-snapshot` and offline playback (observed).

## Dependency direction

- `desktop/` → `hn_lib` (path). `hn_lib` never depends on `desktop/` (observed).
- The terminal front end (`tui`, `terminal_session`, `app`, `ui`) is `pub` only so the binary links; a non-terminal front end uses `reader`'s documented boundary (documented, `src/lib.rs:1-12`, `src/reader.rs:3`). Desktop also uses `reader::saved` for storage and age labels, which the boundary inventory omits (observed).
- Preview: the Rust binary writes a versioned snapshot (`hn_lib::snapshot`); `publish.mjs` uploads it to R2 and advances `latest.json`; `adopt.mjs` installs it; the gateway serves the pinned bytes to the gVisor-isolated CLI and the browser companion. `tools/rollout.mjs` owns `releases/rollout.json`; `tools/update.mjs` owns each host's local release journal (observed).

## Where authoritative state and rules live

| Concern | Owner | Notes |
| :- | :- | :- |
| Feed paging, selection, tab switching | `hn_lib::feed_state` transitions | Both front ends rebuild an owned `FeedState` copy to call them (`src/app.rs:371`, `desktop/src/feed.rs:170`) |
| Comment tree navigation | `hn_lib::comments::Thread` | Both front ends; key routing and focus differ by design |
| Saved stories | `hn_lib::bookmarks` (`saved.json`) | Terminal repeats the shared toggle (`src/bookmarks.rs:69`) |
| Terminal summary lifecycle | `Reader` (job set, attempt IDs) and `Section` (`src/reader/jobs.rs`) | Settings code also writes `Progress` directly (`src/reader/settings.rs:138-195`) |
| Desktop summary lifecycle | `Story` (displayed results), `Bridge` (tasks, parking) | Dispatch eligibility decided in the window and twice in the bridge |
| Summary settings | `hn_lib::reader::settings::Config` → `summary.json`, owner-only | Key precedence documented in README.md:100-102 |
| Saved summaries | `hn_lib::reader::saved::SummaryStore`, one file per half, shared by both apps | Legacy single-file summaries ignored and left in place (documented by test, `src/reader/saved_tests.rs:134`) |
| Snapshot format and validation | `hn_lib::snapshot` | Read by the deployed preview |

## External integrations

Hacker News Firebase API; OpenRouter and Anthropic (paid); Ghostty and terminal-browser panes; WKWebView through wry; Cloudflare R2; GHCR signed images; Vercel page; GitHub Actions monitor that comments on and closes issues.

## Compatibility constraints

`summary.json` format, permissions and precedence; per-half summary files shared across apps; `saved.json`; snapshot format and `latest.json`/`rollout.json`; desktop goldens (PNG and element trees, macOS; trees only on Linux CI); CLI flags, keys, env vars; `hn_lib` public items (frozen until board item B47 is answered).

## Not examined in this pass

Desktop rendering internals and Rune; provider request internals; the gVisor patch in `preview/runtime/`; live hosts, R2 and registry state; Windows and Linux desktop behavior (CI only); the native webview (goldens bypass it, `desktop/src/app/live_page.rs`).
