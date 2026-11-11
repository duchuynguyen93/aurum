# M3 — Windows integration + phát hành

Mục tiêu: app chạy "như app Windows thật": phím media và khung Now Playing của hệ điều hành (SMTC trên Windows,
Now Playing trên macOS), thư viện tự cập nhật khi thư mục nhạc thay đổi, bìa album đọc được từ file `folder.jpg`
bên cạnh, exe có icon, không bật cửa sổ console, có bộ cài và bản portable phát hành theo tag `v*`.
Website giới thiệu (Firebase) **không** nằm trong M3, chờ kết quả chạy thật trên Windows.

## Khảo sát đã chốt (đọc từ source crate)

- `souvlaki 0.9.3`: `MediaControls::new(PlatformConfig { display_name, dbus_name, hwnd: Option<*mut c_void> })`.
  Windows **bắt buộc** `hwnd` (panic nếu `None`); macOS bỏ qua config. `attach(F: Fn(MediaControlEvent) + Send + 'static)`,
  callback chạy trên thread khác và giữ mutex nên chỉ được đẩy vào channel. Windows phát `Play/Pause/Stop/Next/Previous/
  Seek(dir)/SetPosition`, **không** `Toggle`; macOS phát `Toggle/Play/Pause/Next/Previous/SetPosition`.
  `set_metadata(MediaMetadata { title, album, artist, cover_url, duration })`, `set_playback(Playing|Paused { progress:
  Option<MediaPosition> } | Stopped)`. Windows: `progress = None` đặt vị trí về 0, `cover_url` phải là `file://C:\...`
  thô (crate chỉ `trim_start_matches("file://")` rồi `GetFileFromPathAsync`); macOS tạo `NSURL` nên cần
  `file:///Users/...` percent-encoded (`url::Url::from_file_path`). Không có `set_volume` trên hai nền tảng này.
- GPUI: `Window: HasWindowHandle` (raw-window-handle 0.6) → `RawWindowHandle::Win32(h)` → `h.hwnd.get() as *mut c_void`.
  Backend Windows nạp icon cửa sổ bằng `LoadImageW(module, MAKEINTRESOURCE(1))` → icon nhúng phải mang resource id **1**.
- `notify 8.2.2` + `notify-debouncer-full 0.7.0` (feature `crossbeam-channel`): `new_debouncer(timeout, None, tx)`,
  `debouncer.watch(path, RecursiveMode::Recursive)`, `unwatch`. Handler cho `crossbeam_channel::Sender<DebounceEventResult>`.
  `DebouncedEvent` deref tới `notify::Event { kind, paths }`. Root không tồn tại → `ErrorKind::PathNotFound`. Trên Windows
  tràn buffer 16KB thì backend tự unwatch mà không báo → khi nhận `Err` hoặc `need_rescan()` phải dựng lại watcher và quét lại.
  `Drop` chỉ đặt cờ stop, không join.
- Runner `windows-latest`: Inno Setup 6.7.1 (`iscc`), WiX 3.14, ImageMagick 7, Windows SDK (có `rc.exe`).
  `winresource 0.1.34` dùng `CARGO_CFG_TARGET_OS` trong build.rs. `softprops/action-gh-release@v3`.

## Đã làm trước khi giao việc

- `crates/aurum-app/Cargo.toml`: thêm `souvlaki 0.9`, `raw-window-handle 0.6`, `notify 8.2`, `notify-debouncer-full 0.7`
  (crossbeam-channel), `url 2`, `image` thêm `webp`, `[build-dependencies] winresource 0.1`, `build = "build.rs"`.
- `crates/aurum-app/build.rs`: nhúng `assets/icon/aurum.ico` với id 1 + ProductName/FileDescription, chỉ khi target Windows.
- `crates/aurum-app/assets/icon/aurum.png` (512) và `aurum.ico` (256/128/64/48/32/16): chữ A Fraunces vàng trên nền Obsidian.

## Đợt A — core + đóng gói (implementer A, không đụng `state/`)

1. **Bìa sidecar + WebP** (`crates/aurum-core/src/library/cover.rs`): `load(path)` giữ thứ tự: ảnh nhúng trước; không có thì
   tìm trong thư mục chứa file một lần, tên `cover|folder|front|album` × đuôi `jpg|jpeg|png|webp`, không phân biệt hoa thường,
   mime theo đuôi. Thêm `Cover::source` không cần; chỉ trả `Cover { mime, bytes }`. Test trong `crates/aurum-core/tests/`
   (hoặc unit test): copy fixture không có ảnh nhúng vào thư mục tạm kèm `Folder.PNG` → `load` trả bytes PNG; không có file nào
   → `None`. Bìa ở thư mục cha (multi-disc) để backlog.
2. **Console + log Windows** (`crates/aurum-app/src/main.rs`): thêm
   `#![cfg_attr(all(target_os = "windows", not(debug_assertions)), windows_subsystem = "windows")]`. Vì stderr mất ở bản
   release Windows, `env_logger` ghi thêm vào file `paths::data_dir()/aurum.log` (mở append, cắt khi > 2MB) khi
   `cfg!(windows)`; Mac giữ stderr. Dùng `env_logger::Builder::target(Target::Pipe(Box::new(file)))`.
3. **Bộ cài** `installer/aurum.iss`: `AppName=Aurum`, `AppVersion={#AppVersion}` (`#ifndef` mặc định 0.0.0),
   `AppPublisher=duchuynguyen93`, `AppPublisherURL=https://github.com/duchuynguyen93/aurum`, `DefaultDirName={autopf}\Aurum`,
   `PrivilegesRequired=lowest`, `ArchitecturesAllowed=x64compatible`, `ArchitecturesInstallIn64BitMode=x64compatible`,
   `SetupIconFile=..\crates\aurum-app\assets\icon\aurum.ico`, `UninstallDisplayIcon={app}\aurum.exe`, `OutputDir=..\dist`,
   `OutputBaseFilename=aurum-{#AppVersion}-windows-x64-setup`, LZMA2 solid; `[Files]` exe + LICENSE; `[Icons]` Start Menu
   luôn, Desktop là task unchecked; `[Run]` mở app sau cài (postinstall, unchecked tránh). Tiếng Anh.
4. **Release workflow** `.github/workflows/release.yml`: `on: push: tags: ['v*']` + `workflow_dispatch`.
   `permissions: contents: write`. Job `windows` (windows-latest): checkout, `dtolnay/rust-toolchain@1.99.1`, rust-cache,
   bước lấy version: tag → `${GITHUB_REF_NAME#v}`, dispatch → đọc `version` trong `Cargo.toml` (`[workspace.package]`);
   với tag thì **fail** nếu khác version Cargo. `cargo build -p aurum-app --release`, `iscc /DAppVersion=$ver installer\aurum.iss`,
   `Compress-Archive target\release\aurum.exe, LICENSE -> dist\aurum-$ver-windows-x64-portable.zip`.
   Luôn `upload-artifact` hai file (tên `aurum-release`, 14 ngày). Chỉ khi event là tag: `softprops/action-gh-release@v3`
   với `files: dist/*`, `generate_release_notes: true`. Không tạo tag trong plan này; user bấm.
5. **CI** `.github/workflows/build.yml`: job windows thêm `cargo test -p aurum-app` trước build release (job sẽ lâu hơn vì
   compile thêm profile dev của gpui; chấp nhận). `paths-ignore` thêm `docs/**`.
6. **README.md**: mục Download (link Releases, portable zip), mục Build, mục Known limitations cập nhật, ghi trạng thái M3.

Kiểm chứng đợt A: `cargo test -p aurum-core` xanh, `cargo build -p aurum-app` trên Mac xanh (build.rs không chạy phần
Windows), `iscc` chỉ chạy được trên CI. Không commit.

## Đợt B — media controls + theo dõi thư mục (implementer B, chỉ `state/`, `ui/settings.rs`, `ui/shell.rs` nếu cần)

1. **`state/media.rs`** — `pub struct Media { controls: Option<MediaControls>, rx: Receiver<MediaControlEvent>,
   last_timeline: Instant }`.
   - `Media::new(window: &Window) -> Media`: lấy `window.window_handle()`; `Ok(h)` với `RawWindowHandle::Win32(w)` →
     `hwnd = Some(w.hwnd.get() as *mut c_void)`; nền tảng khác `None`. Trên Windows nếu không lấy được Win32 handle thì
     **không** tạo controls (tránh panic). `MediaControls::new(PlatformConfig { display_name: "Aurum", dbus_name: "aurum", hwnd })`
     lỗi → log warn, `controls = None`. `attach(move |e| { tx.send(e).ok(); })`.
   - `pub fn poll(&mut self) -> Vec<MediaControlEvent>` (try_recv hết).
   - `pub fn set_track(&mut self, track: Option<&Track>, cover: Option<&Path>)`: `set_metadata` với title (fallback tên file),
     artist, album, duration, `cover_url` qua `cover_url(path)`: Windows `format!("file://{}", path.display())`; khác
     `url::Url::from_file_path(path).ok().map(|u| u.to_string())`.
   - `pub fn set_playback(&mut self, playing: bool, position: Duration, force: bool)`: `Playing|Paused { progress: Some(..) }`
     luôn `Some`; `force` bỏ qua rate limit, còn lại tối đa 1 lần/giây. `Stopped` khi `current == None`.
   - Mọi lỗi từ souvlaki chỉ log debug, không bao giờ panic.
2. **Thumbnail file cho OS** (`state/cover_cache.rs`): `load_art(path)` thêm tham số `cache_dir: &Path`; sau khi làm
   `thumb` (trước khi đổi BGRA) ghi JPEG chất lượng 85 vào `cache_dir/<hex hash của CoverKey>.jpg` nếu chưa có
   (RGBA → RGB). `CoverArt` thêm `pub file: Option<PathBuf>`. `AppState::ensure_cover_for` truyền `paths::data_dir().join("covers")`
   (tạo thư mục). Hash: `std::hash::DefaultHasher` của key là đủ.
3. **Nối vào `AppState`**: field `media: Media` tạo trong `new(window, cx)`. Trong `tick`: drain `media.poll()` →
   `Play` → nếu chưa phát thì `play()`; `Pause` → nếu đang phát thì `pause()`; `Toggle` → `toggle_play`; `Next`/`Previous`;
   `Stop` → pause + seek 0; `SetPosition(p)` → `seek_to(p.0)`; `Seek(Forward|Backward)` → ±10s kẹp [0, duration];
   các variant khác bỏ qua (`_ =>`, enum non_exhaustive). Tách `play()`/`pause()` idempotent ra khỏi `toggle_play`.
   Gọi `media.set_track` khi `playback.current` đổi (start_current, TrackStarted) và lại khi cover của bài hiện tại vừa load
   xong (`finish_load` của key hiện tại → cập nhật metadata kèm cover). `media.set_playback(.., force: true)` ở play/pause/
   seek/đổi bài/ended; trong `tick` mỗi Position event gọi `set_playback(.., force: false)` (rate limit 1s nằm trong Media).
4. **`state/watch.rs`** — `pub struct Watch { debouncer: Option<Debouncer<RecommendedWatcher, RecommendedCache>>,
   rx: Receiver<DebounceEventResult>, roots: Vec<PathBuf> }`. `Watch::new()`, `rebuild(&mut self, roots: &[PathBuf])`:
   drop debouncer cũ, `new_debouncer(Duration::from_secs(2), None, tx)`; với từng root `watch(root, Recursive)`,
   lỗi → `log::warn!` và bỏ qua root đó. `pub fn poll(&mut self) -> WatchOutcome { Quiet, Changed, Broken }`:
   `Changed` khi có event mà `paths` có file đuôi audio (`aurum_core::library::has_audio_extension`, làm pub trong `scan.rs`)
   hoặc kind là `Create/Remove/Modify(Name)` của thư mục / `Any`; `Broken` khi nhận `Err(..)` hoặc event `need_rescan()`.
   Trong `AppState::tick`: `Changed` → `watch_dirty = true`; `Broken` → `rebuild` + dirty. Nếu dirty và `scan.is_none()` →
   `run_scan` (đã có `rescan_pending` cho trường hợp đang quét). `rebuild` gọi ở `new`, `add_roots`, `remove_root`.
   Settings thêm `watch: bool` (default true) + toggle "Watch folders for changes" trong `ui/settings.rs`; tắt → drop
   debouncer. Thư mục `data_dir` không nằm trong roots nên ghi cache bìa không tự kích hoạt scan.
5. **Unit test** (không wildcard `use gpui_kit::*` trong module test): `cover_url` trên nền tảng hiện tại ra chuỗi hợp lệ
   với đường dẫn có khoảng trắng; `Watch` với thư mục tạm: tạo file `.flac` → `poll` ra `Changed` trong ≤ 5s; file `.txt` →
   `Quiet`.

Kiểm chứng đợt B trên Mac: `cargo test -p aurum-app` xanh; chạy app, phát một bài, bấm F8 (play/pause) và F9 (next) trên
bàn phím Mac → app phản hồi; mở Control Center → Now Playing hiện tên bài, nghệ sĩ, ảnh bìa (chụp màn hình); copy một file
nhạc vào `~/Music/_watchtest/` → vài giây sau lưới Albums có album mới, xoá đi → biến mất (log `scan started`). Không commit.

## Ngoài phạm vi / backlog

- Cover sidecar đổi sau khi đã cache → chỉ thấy sau khi mở lại app.
- Bìa ở thư mục cha (multi-disc), `.webp` nhúng trong tag chưa test thật.
- Shuffle/repeat lên SMTC, tên app trong SMTC (souvlaki không hỗ trợ).
- Ký số exe: SmartScreen vẫn cảnh báo bản chưa ký; cân nhắc SignPath cho OSS sau.
- Bản macOS: không phát hành (chỉ để dev).

## Những thứ chỉ test được trên máy Windows (user)

SMTC hiện khi bấm phím volume/media, không có cửa sổ console, icon taskbar + Explorer, bộ cài không hỏi UAC, shortcut
Start Menu, file `%APPDATA%\aurum\aurum.log` có nội dung, thumbnail bìa trong SMTC.

## Kết quả (2026-11-11)

- Đợt A và B xong trên Mac: 63 test xanh toàn workspace, 1 bỏ qua (AAC priming, từ M1). `cargo fmt`, `clippy` sạch.
- Now Playing macOS hiện tên bài, nghệ sĩ, bìa, tiến trình (ảnh chụp widget Control Center); nút Pause/Play/Next trong
  widget điều khiển được app (log `media control: Pause/Play/Next`). Phím F8/F9 bàn phím thật chưa bấm thử.
- Theo dõi thư mục: copy FLAC vào `~/Music/_watchtest` → `scan started ... added 1` sau 3–8s; xoá thư mục → `removed 1`.
  Phát hiện và xử lý: FSEvents gộp việc xoá cả thư mục thành một `Modify(Metadata)` trên thư mục, nên
  `Modify(Metadata)` với path đã biến mất hoặc là thư mục cũng tính là thay đổi.
- Thumbnail bìa JPEG ghi ở `data_dir/covers/<hash>.jpg` (35 file sau một phiên).
- `~/Music` không có file sidecar nào nên lưới Albums trên Mac không đổi; đường sidecar có 3 test trong `tests/cover.rs`.
- Chưa kiểm được trên Mac: toàn bộ phần Windows (SMTC, thumbnail qua `file://C:\...`, icon, console, bộ cài, log file).
