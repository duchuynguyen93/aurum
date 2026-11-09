# M0 — Scaffold

Mục tiêu: có một workspace Rust build sạch trên macOS, cửa sổ GPUI hiện được chữ bằng gpui-kit,
CI build được trên `windows-latest`, repo public trên GitHub. Chưa có âm thanh, chưa có DB.

## Quyết định kỹ thuật (không đổi trong lúc implement)

- **UI stack**: `gpui-kit = "0.9"` (workspace cũ tên gpui-component, đã đổi tên repo thành longbridge/gpui-kit).
  gpui-kit exact-pin `gpui-pre = "=0.3.11"`. Nếu cần type gpui trực tiếp thì khai báo
  `gpui = { package = "gpui-pre", version = "=0.3.11" }`. **Không** dùng 0.3.12, không dùng crate `gpui` gốc
  (đứng ở 0.2.2 từ 2025-10), không dùng git dependency trỏ vào zed.
  Kiểm bằng `cargo tree -i gpui-pre` chỉ ra đúng một phiên bản.
- **macOS shader**: gpui-kit bật `runtime_shaders` trên gpui-pre-platform, kỳ vọng không cần Metal Toolchain
  (máy này thiếu `xcrun metal`). Nếu build chết ở build script nhắc metal/metallib thì chạy
  `xcodebuild -downloadComponent MetalToolchain` rồi thử lại, và ghi lại trong báo cáo.
- **Workspace** 3 crate, resolver 3, edition 2024:
  - `crates/aurum-core`: model + queue + library. Không phụ thuộc UI, không phụ thuộc audio device. M0 chỉ có `Track` struct và `Queue` với shuffle/repeat nhận `rand::Rng` từ ngoài, kèm test.
  - `crates/aurum-audio`: trait `AudioEngine` (play/pause/seek/set_volume/queue_next) và một `NullEngine`. M0 chưa có cpal.
  - `crates/aurum-app`: binary GPUI. M0: một cửa sổ 1200x760, tiêu đề "Aurum", nền `#0C0B0E`, sidebar 220px nền `#121114`, chữ "AURUM" tracking rộng màu `#D4AF6A`, vùng chính có tiêu đề "Albums" cỡ 22 và dòng "Library is empty" màu `#B9B2A6`. Dùng `gpui_kit::Root` / theme init theo README gpui-kit để component hoạt động. Màu lấy từ `docs/DESIGN.md`, khai báo thành const trong `crates/aurum-app/src/theme.rs`.
- **Profile**: `[profile.dev] opt-level = 1`, `[profile.dev.package."*"] opt-level = 3` để chạy thử không quá chậm; `[profile.release] lto = "thin", codegen-units = 1, strip = true`.
- **Target dir**: `.cargo/config.toml` với `[build] target-dir = "target"` (giữ trong repo, nhưng bật `incremental = true`). Không đặt RUSTFLAGS trong config vì tài liệu Zed cảnh báo nó ghi đè config của gpui trên Windows.
- **Windows manifest**: gpui-fast có feature `windows-manifest-provided-by-host`; M0 chưa đụng, để mặc định.
- **CI**: `.github/workflows/build.yml` với hai job: `macos-latest` chạy `cargo test --workspace` và `cargo build -p aurum-app`; `windows-latest` chạy `cargo build -p aurum-app --release` (runner có cmake, MSVC, Windows SDK sẵn). Cache bằng `Swatinem/rust-cache@v2`. `paths-ignore` cho `**.md`. Windows job upload `aurum.exe` làm artifact 7 ngày để user tải về chạy thử.
- **License** MIT, copyright `duchuynguyen93`. `.gitignore`: `target/`, `.DS_Store`, `*.log`.
- **README** tiếng Anh ngắn: là gì, stack, trạng thái "M0: window only", cách build trên macOS và Windows (yêu cầu VS Build Tools "Desktop development with C++", Windows SDK ≥ 10.0.20348, cmake, rustup), ghi rõ Windows mới chỉ qua CI.

## Việc phải kiểm tra trong source gpui-pre 0.3.11 sau `cargo fetch` (ghi kết quả vào docs/plans/M0-scaffold.md mục "Kết quả khảo sát API")

Grep trong `~/.cargo/registry/src/*/gpui-pre-0.3.11/`:
1. Variant của `enum ImageSource` (cần cho ảnh bìa từ bytes ở M2).
2. Có `Timer` hay `BackgroundExecutor::timer` không (cần cho tick progress ở M2).
3. Signature `uniform_list` (cần cho danh sách ảo).
4. `Window` có expose raw window handle không (`raw_window_handle`, `HasWindowHandle`, `hwnd`). Souvlaki trên Windows cần HWND cho SMTC ở M3. Không có thì ghi rủi ro.
5. Cách nạp font từ bytes: `text_system().add_fonts`.

## Định nghĩa xong

- `cargo test --workspace` xanh trên macOS, có ít nhất 4 test cho `Queue`.
- `cargo run -p aurum-app` mở cửa sổ và **chữ hiện lên** (chụp màn hình bằng `screencapture -x` vào scratchpad và xem lại, không tin build xanh).
- `cargo tree -i gpui-pre` ra một phiên bản.
- Repo khởi tạo, một commit đầu, push lên `duchuynguyen93/aurum` public, CI chạy; job Windows phải xanh hoặc báo lỗi cụ thể.

## Kết quả khảo sát API

Nguồn: `~/.cargo/registry/src/*/gpui-pre-0.3.11/src/`.

1. `ImageSource` (`elements/img.rs:42`): `Resource(Resource)`, `Render(Arc<RenderImage>)`, `Image(Arc<Image>)`, `Custom(Arc<dyn Fn(&mut Window, &mut App) -> Option<Result<Arc<RenderImage>, ImageCacheError>>>)`. Ảnh bìa từ bytes dùng `Image(Arc<Image>)`; `Image { format: ImageFormat, bytes: Vec<u8>, id: u64 }` (`platform.rs:3354`).
2. Có timer: `BackgroundExecutor::timer(&self, duration: Duration) -> Task<()>` (`executor.rs:200`), lấy qua `cx.background_executor()` (`app.rs:383`). Không có struct `Timer` riêng.
3. `uniform_list(id: impl Into<ElementId>, item_count: usize, f: impl 'static + Fn(Range<usize>, &mut Window, &mut App) -> Vec<R>) -> UniformList where R: IntoElement` (`elements/uniform_list.rs:22`); cuộn qua `.track_scroll(&UniformListScrollHandle)` (dòng 686).
4. Có: `impl HasWindowHandle for Window` và `impl HasDisplayHandle for Window` (`window.rs:7841`, `7847`), dùng `raw_window_handle`. Souvlaki lấy HWND qua `window.window_handle()`; không cần tự tìm. Rủi ro còn lại: chưa thử trên Windows thật.
5. `TextSystem::add_fonts(&self, fonts: Vec<Cow<'static, [u8]>>) -> Result<()>` (`text_system.rs:301`), lấy qua `cx.text_system()` (`app.rs:393`).

Phát hiện thêm: GPUI không có letter-spacing, nên chữ AURUM được dựng bằng một div mỗi chữ cái. `gpui_kit::*` re-export toàn bộ gpui nên không cần khai báo `gpui-pre` riêng. `gpui-pre-macos` 0.3.11 dùng `Option::map_or_default` (chỉ ổn định từ Rust 1.96), nên Rust 1.95 không build được; dùng 1.99.1.
