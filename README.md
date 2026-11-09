# Aurum

A local music player for desktop, written in Rust with [GPUI](https://www.gpui.rs) (via
[gpui-kit](https://github.com/longbridge/gpui-kit)). Windows is the main target; development
happens on macOS.

**Status: M0, window only.** No audio, no library yet. Windows is only verified through CI so far.

## Stack

- UI: `gpui-kit` 0.9 (pins `gpui-pre` 0.3.11)
- Workspace: `aurum-core` (model, queue), `aurum-audio` (engine trait), `aurum-app` (GPUI binary)

## Build

Requires a recent stable Rust (1.96 or newer; `gpui-pre-macos` 0.3.11 uses `Option::map_or_default`).

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
