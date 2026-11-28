# hn v0.2.3

`hn` is a terminal reader for Hacker News. This is a maintenance release: the terminal app looks and behaves the same as 0.2.2.

## Changes

- Rules that the terminal and the macOS app share now live in one place in the `hn-reader` library: toggling a saved story, skipping X/Twitter links when the browser preview picks an example story, how changing summary settings affects pending summaries, and how discussion text is cleaned and trimmed before it is summarized.
- `[` and `]` still keep the selection on a comment you have re-rooted on. That rule now lives in the shared comment thread, which also fixes it in the macOS app.
- The crate no longer lists seven unused ICU dependencies directly. The dependencies that get built are unchanged.
- The library gains two public constructors, `hn_lib::reader::Source::from_comment` and `hn_lib::reader::DiscussionCapture::from_story`. No existing public item changed.

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
