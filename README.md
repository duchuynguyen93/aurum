# Aurum

A local music player for desktop, written in Rust with [GPUI](https://www.gpui.rs) (via
[gpui-kit](https://github.com/longbridge/gpui-kit)). Windows is the main target; development
happens on macOS.

**Status: M2 done.** Scans local folders into SQLite, browses albums / artists / tracks with cover art and search, plays FLAC, MP3, AAC/ALAC, Vorbis, WAV and AIFF gaplessly with ReplayGain. Verified by hand on macOS; Windows builds and passes tests in CI but has not been run by a person yet.

## Stack

- UI: `gpui-kit` 0.9 (pins `gpui-pre` 0.3.11)
- Workspace: `aurum-core` (model, queue), `aurum-audio` (engine trait), `aurum-app` (GPUI binary)

## Build

The toolchain is pinned in `rust-toolchain.toml` (1.99.1); rustup installs it on first build. Anything older than 1.96 fails because `gpui-pre-macos` 0.3.11 uses `Option::map_or_default`.

macOS:

```sh
cargo run -p aurum-app
cargo test --workspace
```

Windows (not yet tested by hand, built by CI):

- rustup with the MSVC toolchain
- Visual Studio Build Tools, "Desktop development with C++" workload
- Windows SDK 10.0.20348 or newer
- cmake

```sh
cargo build -p aurum-app --release
```

## License

MIT
