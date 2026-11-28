# hn v0.3.0

`hn` is a terminal reader for Hacker News. The terminal app looks and behaves the same as 0.2.3. The minor version changes because the `hn-reader` library's feed API changed.

## Changes

- `hn --capture-snapshot`, which builds the browser preview's daily snapshot, now fetches each story from Hacker News once instead of twice.
- Library: the feed transitions in `hn_lib::feed_state` take a small `FeedNavigation` value instead of the full `FeedState`, which is removed along with `FeedStatus` and the unused `FeedAction::ToggleSaved` and `FeedAction::ApplyPage`. `switch_tab` takes the target tab's remembered page as a parameter. `HackerNewsCliServiceImpl::story_from_item` is new and public.

## Install

With Homebrew:

```sh
brew install shaance/tap/hn
```

Or download the `tar.gz` and matching `.sha256` for your system from the release assets. Verify the archive with `shasum -a 256 -c <archive>.sha256`, extract it, and put `hn` on your PATH.

To build from source with Rust 1.88 and Cargo:

```sh
cargo install hn-reader --locked
```

## Requirements and optional setup

- macOS binaries target macOS 13 or newer on Apple Silicon and Intel. macOS may need network access the first time it checks the notarization ticket.
- Linux binaries are built on Ubuntu 22.04 for x86-64 and ARM64, with glibc 2.35 and OpenSSL 3. Install the OpenSSL 3 runtime if it is missing.
- Browsing and saving stories work without an API key. Summaries need an OpenRouter or Anthropic API key in the `M` settings form, `OPENROUTER_API_KEY`, or `ANTHROPIC_API_KEY`.
